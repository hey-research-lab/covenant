//! Read-only market observations, independent of rental provisioning. The only
//! upstream operation is Vast's public offer search; this module has no buyer,
//! provider credentials, instance creation, or payment API.
//!
//! Every successful hourly sample is retained (including unchanged prices), so
//! the daily mean is not biased toward offers whose prices change frequently.
//! Snapshots are atomically persisted before becoming visible. A file lock keeps
//! two coordinator processes from silently overwriting the same history.

use crate::prices::PriceObservation;
use anyhow::{ensure, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{BufReader, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub const SAMPLE_INTERVAL_SECONDS: u64 = 3_600;
pub const RETENTION_DAYS: u64 = 366;
const INTERVAL_MS: u64 = SAMPLE_INTERVAL_SECONDS * 1_000;
const RETENTION_MS: u64 = RETENTION_DAYS * 86_400_000;
const MAX_SNAPSHOTS: usize = (RETENTION_DAYS * 24 + 1) as usize;
const MAX_STORE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const OFFERS_PER_MODEL: usize = 8;
const ENDPOINT: &str = "https://console.vast.ai/api/v0/bundles/";
pub const GPU_MODELS: [&str; 8] = [
    "L40S",
    "L40",
    "RTX 6000Ada",
    "RTX A6000",
    "A40",
    "A100 PCIE",
    "A100 SXM4",
    "H100 SXM",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderPriceIdentity {
    pub provider: String,
    pub endpoint: String,
    pub query_version: u8,
    pub gpu_models: Vec<String>,
    pub allocated_storage_gb: u16,
}

impl Default for ProviderPriceIdentity {
    fn default() -> Self {
        Self {
            provider: "vast.ai".into(),
            endpoint: ENDPOINT.into(),
            query_version: 1,
            gpu_models: GPU_MODELS.iter().map(|s| (*s).into()).collect(),
            allocated_storage_gb: 16,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderQuote {
    pub offer_id: u64,
    pub gpu_model: String,
    pub gpu_count: u16,
    /// Whole-instance asking price, rounded to the nearest millionth of USD.
    pub instance_micro_usd_per_hour: u64,
}

impl ProviderQuote {
    fn valid(&self) -> bool {
        self.offer_id > 0
            && GPU_MODELS.contains(&self.gpu_model.as_str())
            && (1..=1024).contains(&self.gpu_count)
            && (1..=100_000_000_000).contains(&self.instance_micro_usd_per_hour)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Sample {
    observed_at_ms: u64,
    offers: Vec<ProviderQuote>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailure {
    Transport,
    HttpStatus,
    InvalidResponse,
    ResponseTooLarge,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredHistory {
    version: u8,
    identity: ProviderPriceIdentity,
    recording_started_at_ms: u64,
    last_attempt_at_ms: Option<u64>,
    last_success_at_ms: Option<u64>,
    last_failure: Option<ProviderFailure>,
    samples: Vec<Sample>,
}

#[derive(Debug, Serialize)]
pub struct ProviderPriceStatus {
    pub identity: ProviderPriceIdentity,
    pub state: &'static str,
    pub recording_started_at_ms: u64,
    pub first_retained_sample_at_ms: Option<u64>,
    pub last_attempt_at_ms: Option<u64>,
    pub last_success_at_ms: Option<u64>,
    pub last_failure: Option<ProviderFailure>,
    pub last_sample_offer_count: usize,
    pub last_sample_counts_by_model: std::collections::BTreeMap<String, usize>,
    pub stale: bool,
    pub persistence_healthy: bool,
    pub sample_interval_seconds: u64,
    pub retention_days: u64,
    pub maximum_offers_per_model: usize,
    pub sample_scope: &'static str,
}

pub struct ProviderPriceStore {
    path: PathBuf,
    identity: ProviderPriceIdentity,
    history: Mutex<StoredHistory>,
    healthy: AtomicBool,
    // Keep the OS lock alive for the lifetime of this store. The persistent lock
    // file itself is harmless after a crash; the OS releases its lock.
    _lock: File,
}

impl ProviderPriceStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let parent = parent_directory(&path);
        fs::create_dir_all(parent).context("create provider price directory")?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))
            .context("open provider price lock")?;
        lock.try_lock()
            .context("provider price store is already in use")?;
        let identity = ProviderPriceIdentity::default();
        let history = if path.exists() {
            let file = File::open(&path).context("open provider price history")?;
            ensure!(
                file.metadata()?.len() <= MAX_STORE_BYTES,
                "provider price history exceeds size limit"
            );
            let stored: StoredHistory = serde_json::from_reader(BufReader::new(file))
                .context("decode provider price history")?;
            validate_history(&stored, &identity)?;
            stored
        } else {
            StoredHistory {
                version: 1,
                identity: identity.clone(),
                recording_started_at_ms: crate::epoch_ms(),
                last_attempt_at_ms: None,
                last_success_at_ms: None,
                last_failure: None,
                samples: vec![],
            }
        };
        let store = Self {
            path,
            identity,
            history: Mutex::new(history),
            healthy: AtomicBool::new(true),
            _lock: lock,
        };
        if !store.path.exists() {
            store.persist(&store.history.lock())?;
        }
        Ok(store)
    }

    pub fn identity(&self) -> &ProviderPriceIdentity {
        &self.identity
    }

    pub fn status(&self) -> ProviderPriceStatus {
        self.status_at(crate::epoch_ms())
    }

    fn status_at(&self, now: u64) -> ProviderPriceStatus {
        let history = self.history.lock();
        let stale = history
            .last_success_at_ms
            .is_none_or(|last| last > now || now.saturating_sub(last) > INTERVAL_MS * 2);
        let healthy = self.healthy.load(Ordering::Acquire);
        let last = history.samples.last();
        let count = last.map_or(0, |sample| sample.offers.len());
        let mut counts = GPU_MODELS
            .iter()
            .map(|s| (s.to_string(), 0))
            .collect::<std::collections::BTreeMap<_, _>>();
        if let Some(sample) = last {
            for offer in &sample.offers {
                *counts.get_mut(&offer.gpu_model).expect("validated model") += 1;
            }
        }
        ProviderPriceStatus {
            identity: self.identity.clone(),
            state: if !healthy { "persistence_failed" } else if history.last_failure.is_some() { "provider_unavailable" }
                else if history.last_success_at_ms.is_none() { "pending" } else if stale { "stale" }
                else if count == 0 { "empty_sample" } else { "available" },
            recording_started_at_ms: history.recording_started_at_ms,
            first_retained_sample_at_ms: history.samples.first().map(|s| s.observed_at_ms),
            last_attempt_at_ms: history.last_attempt_at_ms,
            last_success_at_ms: history.last_success_at_ms,
            last_failure: history.last_failure,
            last_sample_offer_count: count,
            last_sample_counts_by_model: counts,
            stale,
            persistence_healthy: healthy,
            sample_interval_seconds: SAMPLE_INTERVAL_SECONDS,
            retention_days: RETENTION_DAYS,
            maximum_offers_per_model: OFFERS_PER_MODEL,
            sample_scope: "up to eight lowest whole-instance USD hourly asks per listed GPU model; verified, rentable, unrented on-demand offers; provider dph_total with 16 GB storage; bandwidth and additional usage charges excluded; does not assert Covenant admission eligibility",
        }
    }

    pub fn observations(&self, since_ms: u64, until_ms: u64) -> Vec<PriceObservation> {
        let mut observations = Vec::new();
        self.for_each_observation(since_ms, until_ms, |row| observations.push(row));
        observations
    }

    /// Stream one quote at a time into an aggregate. Even a full retention-window
    /// request does not allocate a second copy of the complete offer history.
    /// The callback runs under the history lock and must not re-enter this store.
    pub fn for_each_observation(
        &self,
        since_ms: u64,
        until_ms: u64,
        mut consume: impl FnMut(PriceObservation),
    ) {
        let cutoff = since_ms.max(crate::epoch_ms().saturating_sub(RETENTION_MS));
        for sample in
            self.history.lock().samples.iter().filter(|sample| {
                sample.observed_at_ms >= cutoff && sample.observed_at_ms <= until_ms
            })
        {
            for offer in &sample.offers {
                consume(PriceObservation {
                    gpu_model: offer.gpu_model.clone(),
                    provider: "vast.ai".into(),
                    at_ms: sample.observed_at_ms,
                    gpu_count: offer.gpu_count,
                    amount_micros: offer.instance_micro_usd_per_hour,
                    duration_ms: 3_600_000,
                    source: "provider_market_quote",
                    currency: "USD",
                    proof_path: None,
                });
            }
        }
    }

    pub fn due(&self, now_ms: u64) -> bool {
        self.healthy.load(Ordering::Acquire)
            && self
                .history
                .lock()
                .last_attempt_at_ms
                .is_none_or(|last| now_ms.saturating_sub(last) >= INTERVAL_MS)
    }

    /// Returns false when this interval was already sampled (including restart
    /// and concurrent retries). Provider failures retain all prior observations.
    pub fn record(
        &self,
        at_ms: u64,
        result: std::result::Result<Vec<ProviderQuote>, ProviderFailure>,
    ) -> Result<bool> {
        ensure!(
            self.healthy.load(Ordering::Acquire),
            "provider price persistence is unhealthy"
        );
        let mut current = self.history.lock();
        ensure!(
            at_ms >= current.recording_started_at_ms,
            "provider price observation predates recording"
        );
        if let Some(last) = current.last_attempt_at_ms {
            ensure!(at_ms >= last, "provider price clock moved backwards");
            if at_ms - last < INTERVAL_MS {
                return Ok(false);
            }
        }
        let mut next = current.clone();
        next.last_attempt_at_ms = Some(at_ms);
        match result {
            Ok(offers) => {
                validate_offers(&offers)?;
                next.samples.push(Sample {
                    observed_at_ms: at_ms,
                    offers,
                });
                next.last_success_at_ms = Some(at_ms);
                next.last_failure = None;
            }
            Err(failure) => next.last_failure = Some(failure),
        }
        next.samples
            .retain(|sample| sample.observed_at_ms >= at_ms.saturating_sub(RETENTION_MS));
        ensure!(
            next.samples.len() <= MAX_SNAPSHOTS,
            "provider price history exceeds sample limit"
        );
        self.persist(&next)?;
        *current = next;
        Ok(true)
    }

    fn persist(&self, next: &StoredHistory) -> Result<()> {
        let result = (|| -> Result<()> {
            let bytes = serde_json::to_vec(next)?;
            ensure!(
                bytes.len() as u64 <= MAX_STORE_BYTES,
                "provider price history exceeds byte limit"
            );
            let pending = self.path.with_extension("pending");
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&pending)
                .context("open pending provider price snapshot")?;
            file.write_all(&bytes)
                .context("write provider price snapshot")?;
            file.sync_all().context("sync provider price snapshot")?;
            fs::rename(&pending, &self.path).context("replace provider price snapshot")?;
            File::open(parent_directory(&self.path))?
                .sync_all()
                .context("sync provider price directory")?;
            Ok(())
        })();
        if result.is_err() {
            self.healthy.store(false, Ordering::Release);
        }
        result
    }
}

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn validate_offers(offers: &[ProviderQuote]) -> Result<()> {
    ensure!(
        offers.len() <= GPU_MODELS.len() * OFFERS_PER_MODEL,
        "provider price sample exceeds offer limit"
    );
    let mut ids = HashSet::new();
    for offer in offers {
        ensure!(offer.valid(), "provider price offer is invalid");
        ensure!(
            ids.insert(offer.offer_id),
            "provider price offer ID duplicated"
        );
    }
    for model in GPU_MODELS {
        ensure!(
            offers.iter().filter(|o| o.gpu_model == model).count() <= OFFERS_PER_MODEL,
            "provider price model exceeds offer limit"
        );
    }
    Ok(())
}

fn validate_history(history: &StoredHistory, identity: &ProviderPriceIdentity) -> Result<()> {
    ensure!(
        history.version == 1 && &history.identity == identity,
        "provider price source identity or schema changed"
    );
    ensure!(
        history.recording_started_at_ms > 0,
        "provider price start time is invalid"
    );
    ensure!(
        history.samples.len() <= MAX_SNAPSHOTS,
        "provider price history exceeds sample limit"
    );
    let mut previous = None;
    for sample in &history.samples {
        ensure!(
            sample.observed_at_ms >= history.recording_started_at_ms,
            "provider sample predates recording"
        );
        if let Some(last) = previous {
            ensure!(
                sample.observed_at_ms.saturating_sub(last) >= INTERVAL_MS,
                "provider samples are duplicated or unordered"
            );
        }
        ensure!(
            Some(sample.observed_at_ms) <= history.last_success_at_ms,
            "provider sample follows last success"
        );
        validate_offers(&sample.offers)?;
        previous = Some(sample.observed_at_ms);
    }
    ensure!(
        history.last_success_at_ms <= history.last_attempt_at_ms,
        "provider success follows last attempt"
    );
    if let Some(last) = history.last_attempt_at_ms {
        ensure!(
            last >= history.recording_started_at_ms,
            "provider attempt predates recording"
        );
    }
    if let Some(sample) = history.samples.last() {
        ensure!(
            Some(sample.observed_at_ms) == history.last_success_at_ms,
            "provider last success missing its sample"
        );
    }
    Ok(())
}

pub struct VastPriceSampler {
    client: reqwest::Client,
}

impl VastPriceSampler {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("CovenantPriceHistory/1.0")
                .build()
                .context("build provider price HTTP client")?,
        })
    }

    pub async fn sample(&self) -> std::result::Result<Vec<ProviderQuote>, ProviderFailure> {
        let mut offers = Vec::new();
        for model in GPU_MODELS {
            let mut response = self
                .client
                .post(ENDPOINT)
                .json(&serde_json::json!({
                    "gpu_name": {"eq": model}, "verified": {"eq": true},
                    "rentable": {"eq": true}, "rented": {"eq": false},
                "type": "ondemand", "allocated_storage": 16,
                "order": [["dph_total", "asc"]], "limit": OFFERS_PER_MODEL,
                }))
                .send()
                .await
                .map_err(|_| ProviderFailure::Transport)?;
            if !response.status().is_success() {
                return Err(ProviderFailure::HttpStatus);
            }
            if response
                .content_length()
                .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
            {
                return Err(ProviderFailure::ResponseTooLarge);
            }
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| ProviderFailure::Transport)?
            {
                if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err(ProviderFailure::ResponseTooLarge);
                }
                body.extend_from_slice(&chunk);
            }
            offers.extend(parse_response(&body, model)?);
        }
        validate_offers(&offers).map_err(|_| ProviderFailure::InvalidResponse)?;
        Ok(offers)
    }
}

fn parse_response(
    body: &[u8],
    model: &str,
) -> std::result::Result<Vec<ProviderQuote>, ProviderFailure> {
    #[derive(Deserialize)]
    struct Response {
        offers: Vec<RawOffer>,
    }
    #[derive(Deserialize)]
    struct RawOffer {
        id: u64,
        gpu_name: String,
        num_gpus: u16,
        dph_total: f64,
        verification: String,
        rentable: bool,
        rented: bool,
    }
    let response: Response =
        serde_json::from_slice(body).map_err(|_| ProviderFailure::InvalidResponse)?;
    if response.offers.len() > OFFERS_PER_MODEL {
        return Err(ProviderFailure::InvalidResponse);
    }
    let mut offers = Vec::new();
    for raw in response.offers {
        if raw.gpu_name != model
            || raw.verification != "verified"
            || !raw.rentable
            || raw.rented
            || !raw.dph_total.is_finite()
            || raw.dph_total <= 0.0
            || raw.dph_total > 100_000.0
        {
            return Err(ProviderFailure::InvalidResponse);
        }
        let offer = ProviderQuote {
            offer_id: raw.id,
            gpu_model: raw.gpu_name,
            gpu_count: raw.num_gpus,
            instance_micro_usd_per_hour: (raw.dph_total * 1_000_000.0).round() as u64,
        };
        if !offer.valid() {
            return Err(ProviderFailure::InvalidResponse);
        }
        offers.push(offer);
    }
    validate_offers(&offers).map_err(|_| ProviderFailure::InvalidResponse)?;
    Ok(offers)
}

/// Sampling starts immediately if due, then hourly. A failed query never adds
/// zero-priced capacity. Persistence failure stops new observations until a
/// restart/replay; settlement and existing history remain independent.
pub fn spawn_periodic_provider_prices(
    store: Arc<ProviderPriceStore>,
) -> Result<tokio::task::JoinHandle<()>> {
    let sampler = VastPriceSampler::new()?;
    Ok(tokio::spawn(async move {
        // Recheck once a minute: a tick anchored to process startup can arrive
        // just before the previous (completed) poll's one-hour boundary and
        // accidentally halve the sampling cadence. No HTTP call occurs until
        // the durable last-attempt timestamp is at least an hour old.
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if !store.due(crate::epoch_ms()) {
                continue;
            }
            let result = sampler.sample().await;
            if let Err(failure) = &result {
                tracing::warn!(
                    ?failure,
                    "provider price sample unavailable; retaining previous prices"
                );
            }
            let target = Arc::clone(&store);
            let at_ms = crate::epoch_ms();
            match tokio::task::spawn_blocking(move || target.record(at_ms, result)).await {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    tracing::error!(%error, "provider price sample could not be persisted")
                }
                Err(error) => tracing::error!(%error, "provider price persistence task failed"),
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote(price: u64) -> ProviderQuote {
        ProviderQuote {
            offer_id: 12,
            gpu_model: "L40S".into(),
            gpu_count: 2,
            instance_micro_usd_per_hour: price,
        }
    }

    #[test]
    fn changes_unchanged_prices_and_multi_gpu_units_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prices.json");
        let store = ProviderPriceStore::open(&path).unwrap();
        let at = crate::epoch_ms();
        store.record(at, Ok(vec![quote(1_000_000)])).unwrap();
        assert!(!store.record(at, Ok(vec![quote(2_000_000)])).unwrap());
        store
            .record(at + INTERVAL_MS, Ok(vec![quote(1_000_000)]))
            .unwrap();
        store
            .record(at + INTERVAL_MS * 2, Ok(vec![quote(2_000_000)]))
            .unwrap();
        drop(store);
        let reopened = ProviderPriceStore::open(path).unwrap();
        let rows = reopened.observations(at, at + INTERVAL_MS * 2);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].gpu_count, 2);
        assert_eq!(rows[2].amount_micros, 2_000_000);
        assert_eq!(rows[0].source, "provider_market_quote");
        assert_eq!(rows[0].currency, "USD");
        assert!(rows.iter().all(|r| r.proof_path.is_none()));
    }

    #[test]
    fn failure_is_not_zero_and_empty_capacity_is_not_failure() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProviderPriceStore::open(dir.path().join("prices.json")).unwrap();
        let at = crate::epoch_ms();
        store.record(at, Ok(vec![quote(1_000_000)])).unwrap();
        store
            .record(at + INTERVAL_MS, Err(ProviderFailure::HttpStatus))
            .unwrap();
        assert_eq!(store.status().state, "provider_unavailable");
        assert_eq!(store.status().last_success_at_ms, Some(at));
        assert_eq!(store.observations(at, at + INTERVAL_MS).len(), 1);
        store.record(at + INTERVAL_MS * 2, Ok(vec![])).unwrap();
        assert_eq!(store.status_at(at + INTERVAL_MS * 2).state, "empty_sample");
        assert_eq!(store.status().last_failure, None);
        assert_eq!(store.observations(at, at + INTERVAL_MS * 2).len(), 1);
    }

    #[test]
    fn concurrent_writers_corrupt_history_and_changed_source_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prices.json");
        let store = ProviderPriceStore::open(&path).unwrap();
        assert!(ProviderPriceStore::open(&path).is_err());
        drop(store);
        let mut data: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        data["identity"]["endpoint"] = serde_json::json!("https://different.example/");
        fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(ProviderPriceStore::open(&path).is_err());
        fs::write(&path, b"{partial").unwrap();
        assert!(ProviderPriceStore::open(&path).is_err());
    }

    #[test]
    fn failed_write_does_not_publish_and_abandoned_pending_file_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prices.json");
        let store = ProviderPriceStore::open(&path).unwrap();
        let at = crate::epoch_ms();
        store.record(at, Ok(vec![quote(1_000_000)])).unwrap();
        fs::create_dir(path.with_extension("pending")).unwrap();
        assert!(store
            .record(at + INTERVAL_MS, Ok(vec![quote(2_000_000)]))
            .is_err());
        assert_eq!(store.status().state, "persistence_failed");
        assert_eq!(store.observations(at, at + INTERVAL_MS).len(), 1);
        drop(store);
        fs::remove_dir(path.with_extension("pending")).unwrap();
        fs::write(path.with_extension("pending"), b"{partial").unwrap();
        let recovered = ProviderPriceStore::open(path).unwrap();
        assert_eq!(recovered.observations(at, at + INTERVAL_MS).len(), 1);
        assert!(recovered.status().persistence_healthy);
    }

    #[test]
    fn retention_duplicate_ids_and_clock_rollback_are_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProviderPriceStore::open(dir.path().join("prices.json")).unwrap();
        let at = crate::epoch_ms();
        assert!(store.record(at, Ok(vec![quote(1), quote(2)])).is_err());
        store.record(at, Ok(vec![quote(1)])).unwrap();
        assert!(store.record(at - 1, Ok(vec![quote(2)])).is_err());
        store
            .record(at + RETENTION_MS + INTERVAL_MS, Ok(vec![quote(3)]))
            .unwrap();
        assert_eq!(store.observations(0, u64::MAX).len(), 1);
    }

    #[test]
    fn real_provider_shape_and_invalid_or_ambiguous_prices() {
        let valid = serde_json::json!({"offers":[{"id":54550274,"gpu_name":"L40S","num_gpus":2,"dph_total":0.04777777777777778,"rentable":true,"rented":false,"verification":"verified"}],"truncated":true});
        let decoded = parse_response(&serde_json::to_vec(&valid).unwrap(), "L40S").unwrap();
        assert_eq!(decoded[0].instance_micro_usd_per_hour, 47_778);
        assert_eq!(decoded[0].gpu_count, 2);
        for (field, value) in [
            ("dph_total", serde_json::json!(0)),
            ("dph_total", serde_json::json!(-1)),
            ("num_gpus", serde_json::json!(0)),
            ("rented", serde_json::json!(true)),
            ("gpu_name", serde_json::json!("H100 SXM")),
            ("verification", serde_json::json!("unverified")),
        ] {
            let mut bad = valid.clone();
            bad["offers"][0][field] = value;
            assert!(
                parse_response(&serde_json::to_vec(&bad).unwrap(), "L40S").is_err(),
                "{field}"
            );
        }
        assert!(parse_response(b"{}", "L40S").is_err());
        assert!(parse_response(b"{\"offers\":[]}", "L40S")
            .unwrap()
            .is_empty());
    }
}
