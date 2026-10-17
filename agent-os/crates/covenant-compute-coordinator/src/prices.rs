//! Historical prices projected from the durable lease journal. No backfilled
//! prices, interpolation or live asks are passed off as historical settlement.
use crate::{CoordinatorState, JobRecord};
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use covenant_compute_protocol::{JobKind, SettlementProof};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct PriceObservation {
    pub gpu_model: String,
    pub provider: String,
    pub at_ms: u64,
    pub gpu_count: u16,
    pub amount_micros: u64,
    pub duration_ms: u64,
    pub source: &'static str,
    pub currency: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_path: Option<String>,
}

/// Full RPC genesis hash, not the truncated CAIP-2 identifier. Sources:
/// https://solana.com/docs/payments/agentic-payments/x402
/// https://developers.circle.com/stablecoins/usdc-contract-addresses
pub const SOLANA_MAINNET_GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
pub const SOLANA_MAINNET_USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

/// An immutable deployment boundary for *new* settlement observations. Old
/// source metadata has no persisted chain provenance and is never relabeled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriceHistoryIdentity {
    pub version: u8,
    pub coordinator_pubkey_b58: String,
    pub genesis_hash: String,
    pub settlement_mint_b58: String,
    pub recording_started_at_ms: u64,
}
impl PriceHistoryIdentity {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || bs58::decode(&self.coordinator_pubkey_b58)
                .into_vec()
                .map_or(true, |v| v.len() != 32)
            || self.genesis_hash != SOLANA_MAINNET_GENESIS
            || self.settlement_mint_b58 != SOLANA_MAINNET_USDC
            || self.recording_started_at_ms == 0
        {
            return Err("price settlement identity must bind a coordinator, verified Solana mainnet and native USDC".into());
        }
        Ok(())
    }

    pub fn matches_runtime(&self, coordinator: &str, mint: Option<&str>, now_ms: u64) -> bool {
        self.validate().is_ok()
            && self.coordinator_pubkey_b58 == coordinator
            && mint == Some(self.settlement_mint_b58.as_str())
            && self.recording_started_at_ms <= now_ms
    }
}

/// Call only after independently fetching the RPC genesis and verifying its
/// configured mint. A missing RPC or bad pin disables public settlement prices;
/// it must not stop existing settlement/cleanup workers. Existing files are
/// never overwritten, so an interrupted first write fails closed on restart.
pub fn bind_identity(
    path: &Path,
    coordinator_pubkey_b58: &str,
    verified_genesis_hash: &str,
    settlement_mint_b58: &str,
    now_ms: u64,
) -> Result<PriceHistoryIdentity, String> {
    let current = PriceHistoryIdentity {
        version: 1,
        coordinator_pubkey_b58: coordinator_pubkey_b58.into(),
        genesis_hash: verified_genesis_hash.into(),
        settlement_mint_b58: settlement_mint_b58.into(),
        recording_started_at_ms: now_ms,
    };
    current.validate()?;
    let load = || -> Result<PriceHistoryIdentity, String> {
        let saved: PriceHistoryIdentity =
            serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("invalid durable price identity: {e}"))?;
        saved.validate()?;
        if saved.coordinator_pubkey_b58 != current.coordinator_pubkey_b58
            || saved.genesis_hash != current.genesis_hash
            || saved.settlement_mint_b58 != current.settlement_mint_b58
            || saved.recording_started_at_ms > now_ms
        {
            return Err("durable price identity disagrees with this deployment or clock".into());
        }
        Ok(saved)
    };
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(&serde_json::to_vec(&current).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            fs::File::open(path.parent().unwrap_or(Path::new(".")))
                .and_then(|p| p.sync_all())
                .map_err(|e| e.to_string())?;
            Ok(current)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => load(),
        Err(e) => Err(e.to_string()),
    }
}

/// Minimal projection copied under the job mutex. Signature checks happen only
/// after that mutex has been released. Never clones buyer/input/output/access.
pub(crate) struct PriceProjection {
    quote: Option<PriceObservation>,
    settlement: Option<(PriceObservation, SettlementProof)>,
}
impl PriceProjection {
    pub(crate) fn from_record(
        id: Uuid,
        record: &JobRecord,
        identity: Option<&PriceHistoryIdentity>,
        since_ms: u64,
        until_ms: u64,
    ) -> Option<Self> {
        if record.pinned || record.envelope.payload.kind != JobKind::LeaseSession {
            return None;
        }
        let source = record.lease_access.as_ref()?.source_price.as_ref()?;
        if source.validate().is_err() {
            return None;
        }
        let in_range = |at| at >= since_ms && at <= until_ms;
        let quote = in_range(source.observed_at_ms).then(|| PriceObservation {
            gpu_model: source.gpu_model.clone(),
            provider: source.provider.clone(),
            at_ms: source.observed_at_ms,
            gpu_count: source.gpu_count,
            amount_micros: source.instance_micro_usd_per_hour,
            duration_ms: 3_600_000,
            source: "operator_reported_provider_quote",
            currency: "USD",
            proof_path: None,
        });
        let settlement = (|| {
            let identity = identity.filter(|i| i.validate().is_ok())?;
            // Price metadata recorded before this deployment pin has unknown
            // mint/cluster provenance even if its payout completed afterwards.
            if record.price_history_identity.as_ref() != Some(identity)
                || source.observed_at_ms < identity.recording_started_at_ms
            {
                return None;
            }
            let elapsed = record.metered_elapsed_ms.filter(|v| *v > 0)?;
            let at_ms = record.payout.as_ref()?.recorded_at_ms;
            if !in_range(at_ms) || at_ms < source.observed_at_ms {
                return None;
            }
            let proof = record.settlement_proof(id, &identity.settlement_mint_b58)?;
            let observation = PriceObservation {
                gpu_model: source.gpu_model.clone(),
                provider: source.provider.clone(),
                at_ms,
                gpu_count: source.gpu_count,
                amount_micros: proof.gross_micro_usdc,
                duration_ms: elapsed,
                source: "settled_lease_charge",
                currency: "USDC",
                proof_path: Some(format!("/prices/proofs/{id}")),
            };
            Some((observation, proof))
        })();
        (quote.is_some() || settlement.is_some()).then_some(Self { quote, settlement })
    }

    pub(crate) fn verified_observations(self) -> impl Iterator<Item = PriceObservation> {
        let settlement = self
            .settlement
            .and_then(|(row, proof)| valid_settlement(&proof).then_some(row));
        self.quote.into_iter().chain(settlement)
    }

    fn verified_proof(self) -> Option<SettlementProof> {
        self.settlement
            .and_then(|(_, proof)| valid_settlement(&proof).then_some(proof))
    }
}

fn valid_settlement(proof: &SettlementProof) -> bool {
    let signature_shape = bs58::decode(&proof.tx_signature)
        .into_vec()
        .is_ok_and(|v| v.len() == 64 && v.iter().any(|b| *b != 0));
    signature_shape && proof.verify_offline().is_ok()
}

#[derive(Debug, Serialize)]
pub struct PricePoint {
    pub gpu_model: String,
    pub provider: String,
    pub source: &'static str,
    pub currency: &'static str,
    pub day_start_ms: u64,
    pub first_observed_at_ms: u64,
    pub last_observed_at_ms: u64,
    pub sample_count: u64,
    /// Duration-weighted mean, normalized per GPU. Integer millionths.
    pub mean_micros_per_gpu_hour: u64,
    pub min_micros_per_gpu_hour: u64,
    pub max_micros_per_gpu_hour: u64,
    pub proof_paths: Vec<String>,
}

type PriceBucketKey = (String, String, &'static str, &'static str, u64);

#[derive(Default)]
struct PriceBucket {
    gpu_ms: u128,
    amount: u128,
    count: u64,
    first: u64,
    last: u64,
    min_rate: u64,
    max_rate: u64,
    proof_paths: Vec<String>,
}

/// Retains one accumulator per source/asset/GPU/day rather than all provider
/// offers. A full year can contain over half a million hourly offer samples.
struct PriceSeries {
    since_ms: u64,
    until_ms: u64,
    buckets: BTreeMap<PriceBucketKey, PriceBucket>,
}
impl PriceSeries {
    fn new(since_ms: u64, until_ms: u64) -> Self {
        Self {
            since_ms,
            until_ms,
            buckets: BTreeMap::new(),
        }
    }

    fn observe(&mut self, row: PriceObservation) {
        if row.at_ms < self.since_ms
            || row.at_ms > self.until_ms
            || row.gpu_count == 0
            || row.duration_ms == 0
        {
            return;
        }
        let gpu_ms = row.duration_ms as u128 * row.gpu_count as u128;
        let rate = (row.amount_micros as u128 * 3_600_000 / gpu_ms).min(u64::MAX as u128) as u64;
        let key = (
            row.gpu_model,
            row.provider,
            row.source,
            row.currency,
            row.at_ms / 86_400_000 * 86_400_000,
        );
        let bucket = self.buckets.entry(key).or_insert_with(|| PriceBucket {
            first: row.at_ms,
            last: row.at_ms,
            min_rate: rate,
            max_rate: rate,
            ..PriceBucket::default()
        });
        bucket.gpu_ms += gpu_ms;
        bucket.amount += row.amount_micros as u128;
        bucket.count += 1;
        bucket.first = bucket.first.min(row.at_ms);
        bucket.last = bucket.last.max(row.at_ms);
        bucket.min_rate = bucket.min_rate.min(rate);
        bucket.max_rate = bucket.max_rate.max(rate);
        if let Some(path) = row.proof_path {
            bucket.proof_paths.push(path);
        }
    }

    fn finish(self) -> Vec<PricePoint> {
        self.buckets
            .into_iter()
            .map(
                |((gpu_model, provider, source, currency, day_start_ms), mut bucket)| {
                    bucket.proof_paths.sort();
                    PricePoint {
                        gpu_model,
                        provider,
                        source,
                        currency,
                        day_start_ms,
                        first_observed_at_ms: bucket.first,
                        last_observed_at_ms: bucket.last,
                        sample_count: bucket.count,
                        mean_micros_per_gpu_hour: (bucket.amount * 3_600_000 / bucket.gpu_ms)
                            .min(u64::MAX as u128)
                            as u64,
                        min_micros_per_gpu_hour: bucket.min_rate,
                        max_micros_per_gpu_hour: bucket.max_rate,
                        proof_paths: bucket.proof_paths,
                    }
                },
            )
            .collect()
    }
}

pub fn series(rows: Vec<PriceObservation>, since_ms: u64, until_ms: u64) -> Vec<PricePoint> {
    let mut series = PriceSeries::new(since_ms, until_ms);
    for row in rows {
        series.observe(row);
    }
    series.finish()
}

#[derive(Default, Deserialize)]
pub struct HistoryQuery {
    pub since_ms: Option<u64>,
    pub until_ms: Option<u64>,
}
#[derive(Serialize)]
pub struct History {
    pub version: u8,
    pub coordinator_pubkey_b58: String,
    pub settlement_mint_b58: Option<String>,
    pub settlement_identity: Option<PriceHistoryIdentity>,
    pub settlement_evidence: &'static str,
    pub generated_at_ms: u64,
    pub unit: &'static str,
    pub coverage: &'static str,
    pub provider_sampling: Option<crate::provider_prices::ProviderPriceStatus>,
    pub points: Vec<PricePoint>,
}
pub async fn history(
    State(state): State<CoordinatorState>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<History>, (StatusCode, &'static str)> {
    if !state.config().public_price_history {
        return Err((StatusCode::NOT_FOUND, "public data is disabled"));
    }
    let now = crate::epoch_ms();
    let until = q.until_ms.unwrap_or(now).min(now);
    let since = q.since_ms.unwrap_or(until.saturating_sub(30 * 86_400_000));
    if since > until || until - since > 366 * 86_400_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            "time range must be ordered and at most 366 days",
        ));
    }
    let permit = HISTORY_REQUESTS.try_acquire().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "price history is busy; retry shortly",
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        build_history(&state, since, until, now)
    })
    .await
    .map(Json)
    .map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "price history is temporarily unavailable",
        )
    })
}

/// A price-history-specific proof surface. Opting into prices does not publish
/// the deployment's older private receipt feed. Exactly the same provenance,
/// payout and signature predicates admit this proof and its historical point.
pub async fn scoped_proof(
    State(state): State<CoordinatorState>,
    AxumPath(job_id): AxumPath<Uuid>,
) -> Result<Json<SettlementProof>, (StatusCode, &'static str)> {
    if !state.config().public_price_history {
        return Err((StatusCode::NOT_FOUND, "public price history is disabled"));
    }
    let permit = HISTORY_REQUESTS.try_acquire().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "price history is busy; retry shortly",
        )
    })?;
    let proof = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let identity = state.verified_price_history_identity()?;
        state
            .jobs()
            .price_projection(job_id, identity, crate::epoch_ms())?
            .verified_proof()
    })
    .await
    .map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "price proof is temporarily unavailable",
        )
    })?;
    proof
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "no qualifying public price proof"))
}

static HISTORY_REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

fn build_history(state: &CoordinatorState, since: u64, until: u64, now: u64) -> History {
    let coordinator = state.escrow().coordinator_pubkey_b58();
    let identity = state
        .config()
        .price_history_identity
        .as_ref()
        .filter(|identity| {
            identity.matches_runtime(&coordinator, state.payout().payout_mint_b58(), now)
        });
    let mut aggregate = PriceSeries::new(since, until);
    for row in state.jobs().price_observations(identity, since, until) {
        aggregate.observe(row);
    }
    let provider_sampling = state.provider_prices().map(|store| {
        store.for_each_observation(since, until, |row| aggregate.observe(row));
        store.status()
    });
    let points = aggregate.finish();
    History {
        version: 1,
        coordinator_pubkey_b58: coordinator,
        settlement_mint_b58: identity.map(|i| i.settlement_mint_b58.clone()),
        settlement_identity: identity.cloned(),
        settlement_evidence: "valid signed receipts and recorded payout transaction citations; independently verify cited transactions for chain finality; timestamps record payout observation, not block time; no cross-deployment aggregation",
        generated_at_ms: now,
        unit: "millionths of the named currency per GPU-hour",
        coverage: "provider market quotes are a limited hourly price-ordered sample, not every offer; lease quotes are sampled at successful provisioning; market asks include the configured 16 GB storage and exclude bandwidth/additional usage; quotes are not invoices; history begins with recording; settled series requires the same durable verified identity at source observation and payout; linked proofs cover only qualifying price-history leases",
        provider_sampling,
        points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(amount: u64, duration: u64, count: u16) -> PriceObservation {
        PriceObservation {
            gpu_model: "A100".into(),
            provider: "fixture".into(),
            at_ms: 42,
            gpu_count: count,
            amount_micros: amount,
            duration_ms: duration,
            source: "settled_lease_charge",
            currency: "USDC",
            proof_path: None,
        }
    }
    #[test]
    fn duration_weighting_and_multigpu_units_are_exact() {
        let points = series(
            vec![row(2_000_000, 3_600_000, 2), row(500_000, 1_800_000, 1)],
            0,
            100,
        );
        assert_eq!(points[0].mean_micros_per_gpu_hour, 1_000_000);
        assert_eq!(points[0].sample_count, 2);
    }
    #[test]
    fn sources_assets_days_and_missing_history_are_never_conflated() {
        let mut quote = row(500_000, 3_600_000, 1);
        quote.currency = "USD";
        quote.source = "operator_reported_provider_quote";
        let mut tomorrow = row(9, 1000, 1);
        tomorrow.at_ms = 86_400_001;
        let points = series(
            vec![quote, row(500_000, 3_600_000, 1), tomorrow],
            0,
            90_000_000,
        );
        assert_eq!(points.len(), 3);
        assert!(series(vec![row(1, 1, 1)], 100, 200).is_empty());
        assert!(series(vec![row(1, 0, 1), row(1, 1, 0)], 0, 100).is_empty());
    }
    #[test]
    fn identity_replay_keeps_original_boundary_and_rejects_changes_or_partial_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("price-identity.json");
        let key = bs58::encode([9u8; 32]).into_string();
        let first = bind_identity(
            &path,
            &key,
            SOLANA_MAINNET_GENESIS,
            SOLANA_MAINNET_USDC,
            100,
        )
        .unwrap();
        assert_eq!(
            bind_identity(
                &path,
                &key,
                SOLANA_MAINNET_GENESIS,
                SOLANA_MAINNET_USDC,
                200
            )
            .unwrap(),
            first
        );
        let other = bs58::encode([8u8; 32]).into_string();
        assert!(bind_identity(
            &path,
            &other,
            SOLANA_MAINNET_GENESIS,
            SOLANA_MAINNET_USDC,
            200
        )
        .is_err());
        assert!(
            bind_identity(&path, &key, SOLANA_MAINNET_GENESIS, SOLANA_MAINNET_USDC, 99).is_err()
        );
        assert_eq!(
            serde_json::from_slice::<PriceHistoryIdentity>(&fs::read(&path).unwrap()).unwrap(),
            first
        );
        fs::write(&path, b"{\"version\":").unwrap();
        assert!(bind_identity(
            &path,
            &key,
            SOLANA_MAINNET_GENESIS,
            SOLANA_MAINNET_USDC,
            300
        )
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{\"version\":");
    }

    #[test]
    fn identity_refuses_unknown_mint_network_or_runtime_relabeling() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("identity.json");
        let key = bs58::encode([9u8; 32]).into_string();
        assert!(bind_identity(&path, &key, "unknown-genesis", SOLANA_MAINNET_USDC, 100).is_err());
        assert!(bind_identity(&path, &key, SOLANA_MAINNET_GENESIS, &key, 100).is_err());
        assert!(!path.exists());
        let identity = bind_identity(
            &path,
            &key,
            SOLANA_MAINNET_GENESIS,
            SOLANA_MAINNET_USDC,
            100,
        )
        .unwrap();
        assert!(identity.matches_runtime(&key, Some(SOLANA_MAINNET_USDC), 101));
        assert!(!identity.matches_runtime(&key, Some(&key), 101));
        assert!(!identity.matches_runtime(&key, None, 101));
        assert!(!identity.matches_runtime(&key, Some(SOLANA_MAINNET_USDC), 99));
    }
    #[test]
    fn streaming_daily_accumulator_preserves_weighting_bounds_and_proof_order() {
        let mut stream = PriceSeries::new(10, 86_400_010);
        let mut first = row(2_000_000, 3_600_000, 2);
        first.at_ms = 10;
        first.proof_path = Some("/proof/receipts/z".into());
        stream.observe(first);
        let mut second = row(2_000_000, 1_800_000, 1);
        second.at_ms = 86_399_999;
        second.proof_path = Some("/proof/receipts/a".into());
        stream.observe(second);
        let mut excluded = row(99_000_000, 1, 1);
        excluded.at_ms = 9;
        stream.observe(excluded);
        let mut next_day = row(500_000, 3_600_000, 1);
        next_day.at_ms = 86_400_010;
        stream.observe(next_day);
        // Thousands of market observations retain one bucket's counters, not
        // one vector element per quote. This also checks long-running counts.
        for _ in 0..10_000 {
            let mut market = row(1_000_000, 3_600_000, 1);
            market.currency = "USD";
            market.source = "provider_market_quote";
            stream.observe(market);
        }
        assert_eq!(stream.buckets.len(), 3);
        assert_eq!(
            stream
                .buckets
                .values()
                .map(|b| b.proof_paths.len())
                .sum::<usize>(),
            2
        );
        let points = stream.finish();
        let paid = points
            .iter()
            .find(|p| p.currency == "USDC" && p.day_start_ms == 0)
            .unwrap();
        assert_eq!(paid.sample_count, 2);
        assert_eq!(paid.mean_micros_per_gpu_hour, 1_600_000);
        assert_eq!(paid.min_micros_per_gpu_hour, 1_000_000);
        assert_eq!(paid.max_micros_per_gpu_hour, 4_000_000);
        assert_eq!(
            (paid.first_observed_at_ms, paid.last_observed_at_ms),
            (10, 86_399_999)
        );
        assert_eq!(paid.proof_paths, ["/proof/receipts/a", "/proof/receipts/z"]);
        assert_eq!(
            points
                .iter()
                .find(|p| p.currency == "USD")
                .unwrap()
                .sample_count,
            10_000
        );
    }
}
