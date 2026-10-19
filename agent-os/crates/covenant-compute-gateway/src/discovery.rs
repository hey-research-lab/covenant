//! Live, read-only payment discovery. An observation is not a reservation:
//! each new quote rechecks availability before signing its native request.
use crate::{Gateway, GatewayError, Result};
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use covenant_compute_protocol::InferenceTariff;
use serde_json::{json, Value};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

mod openapi;
pub use openapi::openapi_http;

#[derive(Clone, Debug)]
pub enum DiscoveryPricing {
    Legacy {
        retail_micro_usdc: u64,
        native_micro_usdc: u64,
    },
    ModelTariff {
        retail: InferenceTariff,
        native: InferenceTariff,
    },
}

#[derive(Clone, Debug)]
pub struct DiscoveryOffer {
    pub model: String,
    pub max_output_tokens: u32,
    pub max_input_bytes: usize,
    pub pricing: DiscoveryPricing,
}

impl DiscoveryOffer {
    pub fn native_cost(&self, cap: u32) -> Result<u64> {
        if cap == 0 || cap > self.max_output_tokens {
            return Err(GatewayError::Invalid(
                "output cap outside advertised range".into(),
            ));
        }
        match &self.pricing {
            DiscoveryPricing::Legacy {
                native_micro_usdc, ..
            } => Ok(*native_micro_usdc),
            DiscoveryPricing::ModelTariff { native, .. } => native
                .price(cap)
                .map_err(|e| GatewayError::Invalid(e.to_string())),
        }
    }
    pub fn constrain_to_budget(&mut self, remaining: u64) -> bool {
        if self.max_output_tokens == 0
            || !self
                .native_cost(1)
                .is_ok_and(|cost| cost > 0 && cost <= remaining)
        {
            return false;
        }
        let (mut low, mut high) = (1, self.max_output_tokens);
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            if self.native_cost(middle).is_ok_and(|cost| cost <= remaining) {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        // Never modify the tariff itself: its identity is an exact quote binding.
        self.max_output_tokens = low;
        true
    }
    pub fn matches_request(&self, request: &Value) -> bool {
        request["model"].as_str() == Some(self.model.as_str())
            && request["max_tokens"]
                .as_u64()
                .is_some_and(|n| n > 0 && n <= u64::from(self.max_output_tokens))
            && match &self.pricing {
                DiscoveryPricing::Legacy { .. } => request.get("pricing").is_none(),
                DiscoveryPricing::ModelTariff { .. } => request["pricing"] == "model_tariff_v1",
            }
    }
    fn current(&self, now: u64) -> bool {
        match &self.pricing {
            DiscoveryPricing::Legacy {
                retail_micro_usdc,
                native_micro_usdc,
            } => *native_micro_usdc > 0 && retail_micro_usdc >= native_micro_usdc,
            DiscoveryPricing::ModelTariff { retail, native } => {
                retail.valid_at(now)
                    && native.valid_at(now)
                    && retail.valid_until_ms > now.saturating_add(45_000)
                    && native.valid_until_ms > now.saturating_add(45_000)
            }
        }
    }
    fn pricing_record(&self) -> Value {
        match &self.pricing {
            DiscoveryPricing::Legacy {
                retail_micro_usdc, ..
            } => json!({
                "currency":"USDC", "unit":"micro_usdc", "billing":"fixed_request",
                "price_micro_usdc":retail_micro_usdc, "max_output_tokens":self.max_output_tokens
            }),
            DiscoveryPricing::ModelTariff { retail, .. } => json!({
                "currency":"USDC", "unit":"micro_usdc", "billing":"fixed_requested_output_cap",
                "request_pricing":"model_tariff_v1", "tariff":retail,
                "max_output_tokens":self.max_output_tokens,
                "maximum_price_micro_usdc":retail.price(self.max_output_tokens).ok()
            }),
        }
    }
    fn input_schema(&self) -> Value {
        let mut schema = json!({"type":"object", "additionalProperties":false,
        "required":["model","messages","max_tokens"], "properties":{
            "model":{"type":"string","const":self.model},
            "messages":{"type":"array","minItems":1,"maxItems":32,
                "description":format!("Normalized JSON message array must not exceed {} UTF-8 bytes.",self.max_input_bytes),
                "items":{"type":"object","additionalProperties":false,"required":["role","content"],"properties":{
                    "role":{"enum":["system","user","assistant"]},"content":{"type":"string","minLength":1}
                }}},
            "max_tokens":{"type":"integer","minimum":1,"maximum":self.max_output_tokens},
            "temperature":{"type":"number","minimum":0,"maximum":2},
            "stream":{"const":false}
        }});
        if matches!(self.pricing, DiscoveryPricing::ModelTariff { .. }) {
            schema["required"]
                .as_array_mut()
                .unwrap()
                .push(json!("pricing"));
            schema["properties"]["pricing"] = json!({"const":"model_tariff_v1"});
        }
        schema
    }
}

fn model_records(offers: &[DiscoveryOffer]) -> Vec<Value> {
    let mut models = std::collections::BTreeMap::<String, Value>::new();
    for offer in offers {
        let model = models.entry(offer.model.clone()).or_insert_with(|| json!({
            "id":offer.model,"object":"model","owned_by":"covenant", "streaming":false,"tools":false,"vision":false,
            "max_input_bytes":offer.max_input_bytes,"max_output_tokens":offer.max_output_tokens
        }));
        match offer.pricing {
            DiscoveryPricing::Legacy { .. } => {
                model["legacy_pricing"] = offer.pricing_record();
                if model.get("pricing").is_none() {
                    model["pricing"] = offer.pricing_record();
                    model["max_output_tokens"] = json!(offer.max_output_tokens);
                }
            }
            DiscoveryPricing::ModelTariff { .. } => {
                model["pricing"] = offer.pricing_record();
                model["max_output_tokens"] = json!(offer.max_output_tokens);
            }
        }
    }
    models.into_values().collect()
}

impl Gateway {
    async fn live_discovery_offers(&self) -> Result<(u64, Vec<DiscoveryOffer>)> {
        let _permit = self
            .discovery_reads
            .try_acquire()
            .map_err(|_| GatewayError::Unavailable("catalog busy".into()))?;
        let observed = crate::now_ms();
        let started = Instant::now();
        if !self.admissions_open() {
            return Ok((observed, Vec::new()));
        }
        let mut offers = tokio::time::timeout(
            Duration::from_secs(5),
            self.executor
                .discovery_offers(observed, self.sale_price_atomic),
        )
        .await
        .map_err(|_| GatewayError::Unavailable("catalog lookup timed out".into()))??;
        if offers.len() > 64 {
            return Err(GatewayError::Unavailable("invalid catalog size".into()));
        }
        let now = crate::after_wait(observed, started);
        offers.retain(|offer| offer.current(now));
        if !offers.is_empty() {
            tokio::time::timeout(Duration::from_secs(5), self.rail.check_offers(&offers, now))
                .await
                .map_err(|_| GatewayError::Unavailable("payment readiness timed out".into()))??;
        }
        let disk = tokio::time::timeout(Duration::from_secs(2), self.disk.lock())
            .await
            .map_err(|_| GatewayError::Unavailable("catalog busy".into()))?;
        let now = crate::after_wait(observed, started);
        if !disk.healthy || now < disk.journal.watermark_ms {
            return Err(GatewayError::Unavailable(
                "catalog journal unavailable".into(),
            ));
        }
        if !self.admissions_open()
            || disk.journal.entries.len() >= 100_000
            || disk
                .journal
                .entries
                .values()
                .filter(|e| matches!(e.phase, crate::Phase::Quoted) && e.terms.expires_at_ms > now)
                .count()
                >= 1024
        {
            return Ok((now, Vec::new()));
        }
        let remaining = disk
            .journal
            .reserve_micro_usdc
            .saturating_sub(disk.used_reserve());
        offers.retain_mut(|offer| offer.current(now) && offer.constrain_to_budget(remaining));
        Ok((now, offers))
    }
    pub async fn discovery(&self) -> Result<Value> {
        let (observed, offers) = self.live_discovery_offers().await?;
        let payment = json!({"scheme":"exact","network":self.rail.network(),"asset":self.rail.asset(),
            "assetDecimals":self.rail.asset_decimals(),"payTo":self.rail.pay_to(),"feePayer":self.rail.fee_payer()});
        let mut endpoints = vec![json!({"path":"/v1/models","method":"GET","price":"free"})];
        for offer in &offers {
            endpoints.push(json!({"path":"/v1/chat/completions","url":self.rail.resource_url(),"method":"POST",
                "description":"Request a fresh HTTP 402 quote, then submit the identical body with Payment-Signature. Prices are fixed upfront; availability is an observation, not a reservation.",
                "payment":payment,"price":offer.pricing_record(),"inputSchema":offer.input_schema(),
                "max_input_bytes":offer.max_input_bytes,
                "paymentRequiredHeader":"Payment-Required","paymentSignatureHeader":"Payment-Signature",
                "idempotencyHeader":"Idempotency-Key"}));
        }
        Ok(
            json!({"schemaVersion":1,"x402Version":2,"name":"Covenant Compute",
            "description":"Available text inference and its current exact Solana USDC payment terms.",
            "observedAtMs":observed,"available":!offers.is_empty(),"models":model_records(&offers),"endpoints":endpoints}),
        )
    }
    pub async fn discovery_models(&self) -> Result<Value> {
        let (_, offers) = self.live_discovery_offers().await?;
        Ok(json!({"object":"list","data":model_records(&offers)}))
    }
}

fn respond(result: Result<Value>) -> Response {
    let mut response = match result {
        Ok(value) => Json(value).into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"live discovery unavailable"})),
        )
            .into_response(),
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}
pub async fn manifest_http(State(gateway): State<Arc<Gateway>>) -> Response {
    respond(gateway.discovery().await)
}
pub async fn models_http(State(gateway): State<Arc<Gateway>>) -> Response {
    respond(gateway.discovery_models().await)
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
