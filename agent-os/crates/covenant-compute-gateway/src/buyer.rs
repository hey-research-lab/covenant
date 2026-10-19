//! The real gateway-to-Covenant execution adapter. Its budget is mandatory;
//! successful output includes the receipt already verified by the buyer SDK.
use crate::pricing::TariffBook;
use crate::{ExecutionFailure, ExecutionQuote, Executor, GatewayError, PreparedExecution, Result};
use async_trait::async_trait;
use covenant_compute_buyer::{
    dispatch_signed, durable_budget::DurableBudget, sign_envelope, BuyerConfig, JobRequest,
};
use covenant_compute_protocol::{
    chat_input, generation_input, ChatMessage, ChatRole, GenerationParams, JobKind,
};
use covenant_identity::LocalIdentity;
use covenant_mcp::Content;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

const DELIVERY_MARGIN_MS: u64 = 45_000;

pub struct BuyerExecutor {
    pub config: BuyerConfig,
    pub identity: Arc<LocalIdentity>,
    pub http: reqwest::Client,
    pub budget: DurableBudget,
    pub model: String,
    pub offer_micro_usdc: u64,
    pub max_tokens: u32,
    /// UTF-8 bytes of the normalized JSON message array, matching the provider.
    pub max_input_bytes: usize,
    /// Operator receipt keys approved for this gateway deployment.
    pub approved_operators: Vec<String>,
    pub tariffs: Option<Arc<TariffBook>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    model: String,
    messages: Vec<TextMessage>,
    max_tokens: u32,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    pricing: Option<PricingMode>,
}
#[derive(Deserialize)]
enum PricingMode {
    #[serde(rename = "model_tariff_v1")]
    ModelTariffV1,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TextMessage {
    role: ChatRole,
    content: String,
}

#[async_trait]
impl Executor for BuyerExecutor {
    fn prepare(&self, request: &Value) -> Result<PreparedExecution> {
        self.prepare_with_pricing(request, true, crate::now_ms())
    }
    fn prepare_legacy(&self, request: &Value) -> Result<PreparedExecution> {
        self.prepare_with_pricing(request, false, crate::now_ms())
    }
    fn quote(&self, request: &Value, legacy_price: u64, now: u64) -> Result<ExecutionQuote> {
        let prepared = self.prepare_with_pricing(request, true, now)?;
        let (amount_atomic, mut expires_at_ms) = if request.get("pricing").is_some() {
            let book = self
                .tariffs
                .as_ref()
                .ok_or_else(|| GatewayError::Unavailable("model tariffs are disabled".into()))?;
            let model = request["model"]
                .as_str()
                .ok_or_else(|| GatewayError::Invalid("model required".into()))?;
            let cap = request["max_tokens"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| GatewayError::Invalid("output cap required".into()))?;
            let (retail, _) = book.quote(model, cap, now)?;
            let expires = now.saturating_add(90_000).min(
                retail
                    .tariff
                    .valid_until_ms
                    .saturating_sub(DELIVERY_MARGIN_MS),
            );
            (retail.price_micro_usdc, expires)
        } else {
            (legacy_price, now.saturating_add(90_000))
        };
        for envelope in &prepared.job_envelopes {
            expires_at_ms = expires_at_ms.min(
                envelope
                    .payload
                    .issued_at_ms
                    .saturating_add(covenant_compute_protocol::JOB_FRESHNESS_WINDOW_MS)
                    .saturating_sub(DELIVERY_MARGIN_MS),
            );
        }
        Ok(ExecutionQuote {
            prepared,
            amount_atomic,
            expires_at_ms,
        })
    }
    fn validate_admission(&self, prepared: &PreparedExecution, now: u64) -> Result<()> {
        for envelope in &prepared.job_envelopes {
            if envelope.payload.issued_at_ms.saturating_add(
                envelope
                    .payload
                    .deadline_ms
                    .min(covenant_compute_protocol::JOB_FRESHNESS_WINDOW_MS),
            ) <= now.saturating_add(DELIVERY_MARGIN_MS)
            {
                return Err(GatewayError::Unavailable(
                    "native execution freshness or deadline too close".into(),
                ));
            }
            if let Some(quote) =
                covenant_compute_protocol::parse_tariff_input(&envelope.payload.input)
                    .map_err(|e| GatewayError::Invalid(e.to_string()))?
            {
                if !quote.tariff.valid_at(now)
                    || quote.tariff.valid_until_ms <= now.saturating_add(DELIVERY_MARGIN_MS)
                {
                    return Err(GatewayError::Unavailable(
                        "quoted native tariff expired or too close to expiry".into(),
                    ));
                }
            }
        }
        Ok(())
    }
    fn models(&self, now: u64, legacy_price: u64) -> Vec<Value> {
        self.tariffs.as_ref().map(|book| book.models(now)).unwrap_or_else(|| vec![json!({
            "id":self.model,"object":"model","owned_by":"covenant",
            "pricing":{"currency":"USDC","unit":"micro_usdc","billing":"fixed_request", "price_micro_usdc":legacy_price,"max_output_tokens":self.max_tokens}
        })])
    }
    async fn execute(
        &self,
        prepared: &PreparedExecution,
    ) -> std::result::Result<Value, ExecutionFailure> {
        let uncertain = |message: String| ExecutionFailure {
            message,
            definitely_unpaid: false,
            terminal_seller_loss: false,
        };
        if prepared.buyer != self.identity.agent_id().pubkey_base58()
            || prepared.coordinator != self.config.coordinator_url
            || prepared.job_envelopes.len() != 1
        {
            return Err(uncertain("persisted execution identity mismatch".into()));
        }
        let envelope = prepared.job_envelopes[0].clone();
        self.budget
            .reserve(&envelope, crate::now_ms())
            .map_err(|e| uncertain(e.to_string()))?;
        let outcome = dispatch_signed(&self.http, &self.config, &self.identity, envelope)
            .await
            .map_err(|e| ExecutionFailure {
                definitely_unpaid: e.concludes_purchase_unpaid(),
                terminal_seller_loss: false,
                message: e.to_string(),
            })?;
        self.require_approved_operator(&outcome.receipt.signer_pubkey_b58)?;
        completed_response(outcome, &self.model).map_err(|e| uncertain(e.to_string()))
    }
}
impl BuyerExecutor {
    fn prepare_with_pricing(
        &self,
        request: &Value,
        use_tariffs: bool,
        now: u64,
    ) -> Result<PreparedExecution> {
        if request.get("pricing").is_some_and(Value::is_null) {
            return Err(GatewayError::Invalid(
                "pricing must be omitted or model_tariff_v1".into(),
            ));
        }
        let request:Request=serde_json::from_value(request.clone()).map_err(|_|GatewayError::Invalid("expected text chat: model, messages, max_tokens, optional temperature/pricing; streaming unsupported".into()))?;
        if request.model != self.model
            || request.max_tokens == 0
            || request.max_tokens > self.max_tokens
            || request.stream
            || request.messages.is_empty()
            || request.messages.len() > 32
            || serde_json::to_vec(&request.messages)?.len() > self.max_input_bytes
            || request
                .messages
                .iter()
                .any(|m| m.role == ChatRole::Tool || m.content.is_empty())
        {
            return Err(GatewayError::Invalid(
                "unsupported model, output cap or message type".into(),
            ));
        }
        let mut input = chat_input(
            request
                .messages
                .into_iter()
                .map(|m| ChatMessage {
                    role: m.role,
                    content: m.content,
                    images: vec![],
                    tool_calls: vec![],
                    tool_call_id: None,
                })
                .collect(),
        );
        input.push(
            generation_input(GenerationParams {
                max_tokens: Some(request.max_tokens),
                temperature: request.temperature,
                ..Default::default()
            })
            .map_err(|e| GatewayError::Invalid(e.to_string()))?,
        );
        let offer = if use_tariffs && request.pricing.is_some() {
            let book = self
                .tariffs
                .as_ref()
                .ok_or_else(|| GatewayError::Unavailable("model tariffs are disabled".into()))?;
            {
                let (_, native) = book.quote(&request.model, request.max_tokens, now)?;
                input.push(
                    covenant_compute_protocol::tariff_input(&native)
                        .map_err(|e| GatewayError::Invalid(e.to_string()))?,
                );
                native.price_micro_usdc
            }
        } else {
            self.offer_micro_usdc
        };
        let envelope = sign_envelope(
            &self.config,
            &self.identity,
            JobRequest {
                kind: JobKind::InferenceCall,
                input,
                model: Some(request.model),
                gpu_class: None,
                min_vram_gb: None,
                min_reputation_bps: None,
                price_micro_usdc: offer,
                // Includes authorization and dispatch; quote expiry separately
                // reserves time within the node's signed-request freshness bound.
                deadline_ms: 180_000,
            },
        )
        .map_err(|e| GatewayError::Invalid(e.to_string()))?;
        Ok(PreparedExecution {
            buyer: self.identity.agent_id().pubkey_base58(),
            coordinator: self.config.coordinator_url.clone(),
            job_envelopes: vec![envelope],
            maximum_cost_micro_usdc: offer,
        })
    }

    pub(crate) fn require_approved_operator(
        &self,
        signer_pubkey_b58: &str,
    ) -> std::result::Result<(), ExecutionFailure> {
        if !self
            .approved_operators
            .iter()
            .any(|key| key == signer_pubkey_b58)
        {
            return Err(ExecutionFailure {
                message: "verified native result used an unapproved inference operator; customer payment not settled".into(),
                definitely_unpaid: false,
                terminal_seller_loss: true,
            });
        }
        Ok(())
    }
}
pub fn completed_response(
    outcome: covenant_compute_buyer::DispatchOutcome,
    model: &str,
) -> Result<Value> {
    let text = outcome
        .output
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let finish_reason = outcome
        .receipt
        .receipt
        .meter
        .finish_reason
        .map(|reason| reason.as_openai())
        .unwrap_or("stop");
    Ok(json!({
        "id":format!("chatcmpl-{}",outcome.envelope.payload.job_id), "object":"chat.completion", "created":crate::now_ms()/1000,
        "model":model, "choices":[{"index":0,"message":{"role":"assistant","content":text},"finish_reason":finish_reason}],
        "covenant":{"job_id":outcome.envelope.payload.job_id,"envelope":outcome.envelope,"receipt":outcome.receipt,"output_json":serde_json::to_string(&outcome.output)?,"output":outcome.output}
    }))
}
