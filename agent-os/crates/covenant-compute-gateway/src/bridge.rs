//! Private, authenticated adapter to the pinned official x402 SVM verifier.
//! The fully signed transaction is returned before broadcast and fsynced by
//! the gateway. Settlement always retries those exact bytes and checks finality.
use crate::{
    payment_payload, GatewayError, PaymentRail, PaymentTerms, Result, SettlementStatus,
    VerifiedPayment,
};
use async_trait::async_trait;
use base64::Engine;
use covenant_compute_protocol::InferenceTariff;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

const MAX_READINESS_BYTES: usize = 65_536;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Readiness {
    schema_version: u32,
    observed_at_ms: u64,
    pricing_valid: bool,
    scheme: String,
    network: String,
    asset: String,
    asset_decimals: u8,
    pay_to: String,
    fee_payer: String,
    resource_url: String,
    legacy_price_atomic: String,
    retail_tariffs: Vec<InferenceTariff>,
    #[serde(default)]
    batch_enabled: bool,
}

impl Readiness {
    fn legacy_price(&self) -> Option<u64> {
        self.legacy_price_atomic.parse::<u64>().ok().filter(|n| {
            *n > 0 && *n <= MAX_SAFE_INTEGER && n.to_string() == self.legacy_price_atomic
        })
    }
    fn allows(&self, amount: u64, now: u64) -> bool {
        if Some(amount) == self.legacy_price() {
            return true;
        }
        self.retail_tariffs.iter().any(|tariff| {
            if !tariff.valid_at(now) || amount < tariff.base_micro_usdc {
                return false;
            }
            let variable = u128::from(amount - tariff.base_micro_usdc);
            let rate = u128::from(tariff.output_cap_micro_usdc_per_million);
            if rate == 0 {
                return variable == 0;
            }
            if variable == 0 {
                return false;
            }
            let first = (variable - 1) * 1_000_000 / rate + 1;
            let last = variable * 1_000_000 / rate;
            first <= last && first <= u128::from(tariff.max_output_tokens) && last >= 1
        })
    }
    fn allows_batch(&self, manifest: &crate::batch::BatchPricingManifest, now: u64) -> bool {
        self.batch_enabled
            && manifest.children.len() <= 2
            && manifest.total().is_ok()
            && manifest.children.iter().all(|child| {
                if child.model.trim().is_empty()
                    || child.model == "any"
                    || child.model.len() > 128
                    || child.model.chars().any(char::is_control)
                    || child.max_output_tokens > 32_768
                {
                    return false;
                }
                match child.pricing.as_deref() {
                    None => Some(child.amount_atomic) == self.legacy_price(),
                    Some("model_tariff_v1") => self.retail_tariffs.iter().any(|tariff| {
                        tariff.model == child.model
                            && tariff.valid_at(now)
                            && tariff.price(child.max_output_tokens).ok()
                                == Some(child.amount_atomic)
                    }),
                    Some(_) => false,
                }
            })
    }
}

pub struct SvmBridge {
    http: reqwest::Client,
    url: String,
    secret: String,
    network: String,
    asset: String,
    pay_to: String,
    fee_payer: String,
    resource_url: String,
}
impl SvmBridge {
    pub fn new(
        url: String,
        secret: String,
        network: String,
        asset: String,
        pay_to: String,
        fee_payer: String,
        resource_url: String,
    ) -> Result<Self> {
        let parsed = reqwest::Url::parse(&url)
            .map_err(|_| GatewayError::Invalid("invalid bridge URL".into()))?;
        if parsed.scheme() != "http"
            || parsed.host_str() != Some("127.0.0.1")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || secret.len() < 32
        {
            return Err(GatewayError::Invalid(
                "bridge must use authenticated loopback HTTP".into(),
            ));
        }
        let asset_expected = match network.as_str() {
            "solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp" => {
                "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
            }
            "solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1" => {
                "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
            }
            _ => return Err(GatewayError::Invalid("unsupported Solana network".into())),
        };
        let resource = reqwest::Url::parse(&resource_url)
            .map_err(|_| GatewayError::Invalid("invalid resource URL".into()))?;
        if asset != asset_expected
            || resource.scheme() != "https"
            || !resource.username().is_empty()
            || resource.password().is_some()
            || resource.path() != "/v1/chat/completions"
            || resource.query().is_some()
            || resource.fragment().is_some()
        {
            return Err(GatewayError::Invalid(
                "rail asset/resource configuration mismatch".into(),
            ));
        }
        for key in [&pay_to, &fee_payer] {
            if bs58::decode(key)
                .into_vec()
                .map_or(true, |bytes| bytes.len() != 32)
            {
                return Err(GatewayError::Invalid("invalid payment address".into()));
            }
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| GatewayError::Unavailable(e.to_string()))?;
        Ok(Self {
            http,
            url: url.trim_end_matches('/').into(),
            secret,
            network,
            asset,
            pay_to,
            fee_payer,
            resource_url,
        })
    }
    async fn readiness(&self, now: u64) -> Result<(Readiness, u64)> {
        let started = Instant::now();
        let unavailable =
            || GatewayError::Unavailable("payment bridge readiness unavailable".into());
        let mut response = self
            .http
            .get(format!("{}/readiness", self.url))
            .bearer_auth(&self.secret)
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map_err(|_| unavailable())?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n > MAX_READINESS_BYTES as u64)
        {
            return Err(unavailable());
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            if chunk.len() > MAX_READINESS_BYTES.saturating_sub(body.len()) {
                return Err(unavailable());
            }
            body.extend_from_slice(&chunk);
        }
        let snapshot: Readiness = serde_json::from_slice(&body).map_err(|_| unavailable())?;
        let now = crate::after_wait(now, started);
        if snapshot.schema_version != 1
            || !snapshot.pricing_valid
            || snapshot.observed_at_ms.abs_diff(now) > 5_000
            || snapshot.scheme != "exact"
            || snapshot.asset_decimals != 6
            || snapshot.network != self.network
            || snapshot.asset != self.asset
            || snapshot.pay_to != self.pay_to
            || snapshot.fee_payer != self.fee_payer
            || snapshot.resource_url != self.resource_url
            || snapshot.legacy_price().is_none()
            || snapshot.retail_tariffs.len() > 32
        {
            return Err(unavailable());
        }
        let mut models = BTreeSet::new();
        for tariff in &snapshot.retail_tariffs {
            if tariff.validate().is_err()
                || tariff.max_output_tokens > 32_768
                || !models.insert(&tariff.model)
                || [
                    tariff.revision,
                    tariff.base_micro_usdc,
                    tariff.output_cap_micro_usdc_per_million,
                    tariff.valid_from_ms,
                    tariff.valid_until_ms,
                    tariff.price(tariff.max_output_tokens).unwrap_or(u64::MAX),
                ]
                .iter()
                .any(|n| *n > MAX_SAFE_INTEGER)
            {
                return Err(unavailable());
            }
        }
        Ok((snapshot, now))
    }
    async fn call(&self, path: &str, body: Value) -> Result<Value> {
        let response = self
            .http
            .post(format!("{}{path}", self.url))
            .bearer_auth(&self.secret)
            .json(&body)
            .send()
            .await
            .map_err(|_| GatewayError::Unavailable("payment bridge unavailable".into()))?;
        if response.status().is_client_error() {
            return Err(GatewayError::Payment(
                "authorization rejected by payment verifier".into(),
            ));
        }
        if !response.status().is_success() {
            return Err(GatewayError::Unavailable(
                "payment bridge unavailable".into(),
            ));
        }
        response
            .json()
            .await
            .map_err(|_| GatewayError::Unavailable("invalid payment bridge response".into()))
    }
}

#[async_trait]
impl PaymentRail for SvmBridge {
    fn network(&self) -> &str {
        &self.network
    }
    fn asset(&self) -> &str {
        &self.asset
    }
    fn pay_to(&self) -> &str {
        &self.pay_to
    }
    fn fee_payer(&self) -> &str {
        &self.fee_payer
    }
    fn resource_url(&self) -> &str {
        &self.resource_url
    }
    async fn check_offers(
        &self,
        offers: &[crate::discovery::DiscoveryOffer],
        now: u64,
    ) -> Result<()> {
        let (snapshot, now) = self.readiness(now).await?;
        for offer in offers {
            let matches = match &offer.pricing {
                crate::discovery::DiscoveryPricing::Legacy {
                    retail_micro_usdc, ..
                } => Some(*retail_micro_usdc) == snapshot.legacy_price(),
                crate::discovery::DiscoveryPricing::ModelTariff { retail, .. } => {
                    retail.valid_at(now) && snapshot.retail_tariffs.contains(retail)
                }
            };
            if !matches {
                return Err(GatewayError::Unavailable(
                    "payment bridge pricing differs from advertised offer".into(),
                ));
            }
        }
        Ok(())
    }
    async fn check_price(&self, amount_atomic: u64, now: u64) -> Result<()> {
        let (snapshot, now) = self.readiness(now).await?;
        if !snapshot.allows(amount_atomic, now) {
            return Err(GatewayError::Unavailable(
                "payment amount is not configured for admission".into(),
            ));
        }
        Ok(())
    }
    async fn check_batch_price(
        &self,
        manifest: &crate::batch::BatchPricingManifest,
        now: u64,
    ) -> Result<()> {
        let (snapshot, now) = self.readiness(now).await?;
        if !snapshot.allows_batch(manifest, now) {
            return Err(GatewayError::Unavailable(
                "batch pricing is not configured for admission".into(),
            ));
        }
        Ok(())
    }
    fn authorization_id(&self, header: &str) -> Result<String> {
        let payload = payment_payload(header)?;
        let encoded = payload["payload"]["transaction"]
            .as_str()
            .ok_or_else(|| GatewayError::Payment("transaction missing".into()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| GatewayError::Payment("transaction encoding invalid".into()))?;
        if bytes.len() > 1232 || bytes.is_empty() {
            return Err(GatewayError::Payment("transaction size invalid".into()));
        }
        Ok(Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect())
    }
    async fn verify(&self, header: &str, terms: &PaymentTerms) -> Result<VerifiedPayment> {
        let payload = payment_payload(header)?;
        if payload["x402Version"] != 2
            || payload["accepted"] != terms.accepted()
            || payload["resource"]["url"] != terms.resource_url
        {
            return Err(GatewayError::Payment(
                "payment requirements or resource changed".into(),
            ));
        }
        let result = self.call("/verify", json!({"paymentPayload":payload,"paymentRequirements":terms.accepted(),"resourceUrl":terms.resource_url})).await?;
        let payment: VerifiedPayment = serde_json::from_value(result)?;
        if payment.authorization_id != self.authorization_id(header)?
            || payment.transaction_id.is_empty()
            || payment.encoded_transaction.is_empty()
            || payment.payer.is_empty()
        {
            return Err(GatewayError::Unavailable(
                "payment bridge binding mismatch".into(),
            ));
        }
        Ok(payment)
    }
    async fn settle(
        &self,
        payment: &VerifiedPayment,
        terms: &PaymentTerms,
    ) -> Result<SettlementStatus> {
        let value = self
            .call(
                "/settle",
                json!({"payment":payment,"paymentRequirements":terms.accepted()}),
            )
            .await?;
        Ok(match value["status"].as_str() {
            Some("finalized") if value["transaction_id"] == payment.transaction_id => {
                SettlementStatus::Finalized {
                    transaction_id: payment.transaction_id.clone(),
                }
            }
            Some("failed") => SettlementStatus::Failed,
            _ => SettlementStatus::Pending,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{DiscoveryOffer, DiscoveryPricing};
    use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::sync::Mutex;

    const SECRET: &str = "readiness-test-secret-at-least-32-bytes";
    const NETWORK: &str = "solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1";
    const ASSET: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
    const ADDRESS: &str = "11111111111111111111111111111111";
    const RESOURCE: &str = "https://example.com/v1/chat/completions";

    fn tariff() -> InferenceTariff {
        InferenceTariff {
            schema_version: 1,
            id: "retail".into(),
            revision: 1,
            model: "haiku".into(),
            base_micro_usdc: 12_000,
            output_cap_micro_usdc_per_million: 9_000_000,
            max_output_tokens: 2048,
            valid_from_ms: 1000,
            valid_until_ms: 200_000,
        }
    }
    fn snapshot() -> Value {
        json!({"schemaVersion":1,"observedAtMs":1000,"pricingValid":true,"scheme":"exact",
            "network":NETWORK,"asset":ASSET,"assetDecimals":6,"payTo":ADDRESS,"feePayer":ADDRESS,
            "resourceUrl":RESOURCE,"legacyPriceAtomic":"50000","retailTariffs":[tariff()]})
    }
    fn bridge(url: String, resource: &str) -> Result<SvmBridge> {
        SvmBridge::new(
            url,
            SECRET.into(),
            NETWORK.into(),
            ASSET.into(),
            ADDRESS.into(),
            ADDRESS.into(),
            resource.into(),
        )
    }
    struct Fixture {
        value: Mutex<Value>,
        reads: AtomicUsize,
    }
    async fn serve(State(state): State<Arc<Fixture>>, headers: HeaderMap) -> Json<Value> {
        assert_eq!(headers["authorization"], format!("Bearer {SECRET}"));
        state.reads.fetch_add(1, Ordering::SeqCst);
        Json(state.value.lock().await.clone())
    }
    async fn fixture() -> (SvmBridge, Arc<Fixture>, tokio::task::JoinHandle<()>) {
        let state = Arc::new(Fixture {
            value: Mutex::new(snapshot()),
            reads: AtomicUsize::new(0),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/readiness", get(serve))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (bridge(url, RESOURCE).unwrap(), state, task)
    }
    fn offers() -> Vec<DiscoveryOffer> {
        vec![
            DiscoveryOffer {
                model: "haiku".into(),
                max_output_tokens: 2048,
                max_input_bytes: 4096,
                pricing: DiscoveryPricing::Legacy {
                    retail_micro_usdc: 50000,
                    native_micro_usdc: 30000,
                },
            },
            DiscoveryOffer {
                model: "haiku".into(),
                max_output_tokens: 256,
                max_input_bytes: 4096,
                pricing: DiscoveryPricing::ModelTariff {
                    retail: tariff(),
                    native: tariff(),
                },
            },
        ]
    }

    #[tokio::test]
    async fn readiness_binds_complete_offers_and_exact_integer_cap_prices_with_get_only() {
        let (bridge, state, task) = fixture().await;
        bridge.check_offers(&offers(), 1000).await.unwrap();
        assert_eq!(
            state.reads.load(Ordering::SeqCst),
            1,
            "one policy read for all offers"
        );
        for amount in [14_304, 30_432, 50_000] {
            bridge.check_price(amount, 1000).await.unwrap();
        }
        for amount in [0, 12_000, 14_305, 30_441, 50_001] {
            assert!(bridge.check_price(amount, 1000).await.is_err());
        }
        let mut changed = snapshot();
        changed["retailTariffs"][0]["revision"] = json!(2);
        *state.value.lock().await = changed;
        assert!(
            bridge.check_offers(&offers(), 1000).await.is_err(),
            "same price with changed definition must not advertise"
        );
        bridge.check_price(14_304, 1000).await.unwrap();
        *state.value.lock().await = snapshot();
        let mut changed_offers = offers();
        if let DiscoveryPricing::ModelTariff { retail, .. } = &mut changed_offers[1].pricing {
            retail.max_output_tokens = 256;
        }
        assert!(
            bridge.check_offers(&changed_offers, 1000).await.is_err(),
            "admitted cap must not replace signed tariff cap"
        );
        task.abort();
    }

    #[tokio::test]
    async fn batch_readiness_requires_opt_in_and_each_exact_child_price() {
        use crate::batch::{BatchPrice, BatchPricingManifest};
        let (bridge, state, task) = fixture().await;
        let manifest = BatchPricingManifest {
            version: 1,
            batch_id: uuid::Uuid::new_v4(),
            children: [256, 2048]
                .map(|cap| BatchPrice {
                    job_id: uuid::Uuid::new_v4(),
                    model: "haiku".into(),
                    max_output_tokens: cap,
                    pricing: Some("model_tariff_v1".into()),
                    amount_atomic: tariff().price(cap).unwrap(),
                })
                .into(),
        };
        assert!(bridge.check_batch_price(&manifest, 1000).await.is_err());
        state.value.lock().await["batchEnabled"] = json!(true);
        bridge.check_batch_price(&manifest, 1000).await.unwrap();
        let mut changed = manifest.clone();
        changed.children[0].amount_atomic += 1;
        changed.children[1].amount_atomic -= 1;
        assert_eq!(changed.total().unwrap(), manifest.total().unwrap());
        assert!(bridge.check_batch_price(&changed, 1000).await.is_err());
        for field in ["model", "max_output_tokens", "job_id", "amount_atomic"] {
            let mut changed = serde_json::to_value(&manifest).unwrap();
            changed["children"][0][field] = match field {
                "model" => json!("other"),
                "max_output_tokens" => json!(257),
                "job_id" => changed["children"][1][field].clone(),
                _ => json!(0),
            };
            let changed = serde_json::from_value(changed).unwrap();
            assert!(
                bridge.check_batch_price(&changed, 1000).await.is_err(),
                "accepted {field}"
            );
        }
        let mut legacy = manifest.clone();
        legacy.children[0].pricing = None;
        legacy.children[0].amount_atomic = 50_000;
        bridge.check_batch_price(&legacy, 1000).await.unwrap();
        assert_eq!(legacy.total().unwrap(), 80_432);
        task.abort();
    }

    #[tokio::test]
    async fn readiness_refuses_identity_drift_malformed_stale_or_oversized_policy() {
        let (bridge, state, task) = fixture().await;
        for (field, value) in [
            ("network", json!("other")),
            ("asset", json!("other")),
            ("payTo", json!("other")),
            ("feePayer", json!("other")),
            (
                "resourceUrl",
                json!("https://other.example/v1/chat/completions"),
            ),
            ("scheme", json!("upto")),
            ("assetDecimals", json!(9)),
            ("schemaVersion", json!(2)),
            ("pricingValid", json!(false)),
            ("observedAtMs", json!(20_000)),
            ("legacyPriceAtomic", json!("050000")),
            ("legacyPriceAtomic", json!("9007199254740992")),
            ("retailTariffs", json!([tariff(), tariff()])),
            ("extra", json!("x".repeat(MAX_READINESS_BYTES))),
        ] {
            let mut changed = snapshot();
            changed[field] = value;
            *state.value.lock().await = changed;
            assert!(
                bridge.check_price(50_000, 1000).await.is_err(),
                "accepted changed {field}"
            );
        }
        *state.value.lock().await = snapshot();
        assert!(
            bridge.check_offers(&offers(), 10_000).await.is_err(),
            "stale bridge clock must not advertise"
        );
        let mut expired = snapshot();
        expired["observedAtMs"] = json!(200_000);
        *state.value.lock().await = expired;
        assert!(bridge.check_price(14_304, 200_000).await.is_err());
        bridge.check_price(50_000, 200_000).await.unwrap();
        task.abort();
    }

    #[test]
    fn readiness_price_grid_covers_zero_rates_rounding_and_rejects_resource_credentials() {
        for rate in [0, 1, 500_000, 1_000_001, 1_500_000, 9_000_000] {
            let mut value = snapshot();
            value["retailTariffs"][0]["base_micro_usdc"] = json!(100);
            value["retailTariffs"][0]["output_cap_micro_usdc_per_million"] = json!(rate);
            let policy: Readiness = serde_json::from_value(value).unwrap();
            let tariff = &policy.retail_tariffs[0];
            let prices: BTreeSet<_> = (1..=2048).map(|cap| tariff.price(cap).unwrap()).collect();
            for amount in 99..=tariff.price(2048).unwrap() + 1 {
                assert_eq!(
                    policy.allows(amount, 1000),
                    prices.contains(&amount),
                    "rate {rate}, amount {amount}"
                );
            }
        }
        assert!(bridge(
            "http://127.0.0.1:4043".into(),
            "https://user:secret@example.com/v1/chat/completions"
        )
        .is_err());
        assert!(bridge(
            "http://127.0.0.1:4043".into(),
            "https://user@example.com/v1/chat/completions"
        )
        .is_err());
    }
}
