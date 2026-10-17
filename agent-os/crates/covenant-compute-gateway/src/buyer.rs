//! The real gateway-to-Covenant execution adapter. Its budget is mandatory;
//! successful output includes the receipt already verified by the buyer SDK.
use crate::{ExecutionFailure, Executor, GatewayError, PreparedExecution, Result};
use async_trait::async_trait;
use covenant_compute_buyer::{
    dispatch_signed, durable_budget::DurableBudget, sign_envelope, BuyerConfig, JobRequest,
};
use covenant_compute_protocol::{
    chat_input, generation_input, ChatMessage, ChatRole, GenerationParams, JobKind,
};
use covenant_identity::LocalIdentity;
use covenant_mcp::Content;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct BuyerExecutor {
    pub config: BuyerConfig,
    pub identity: Arc<LocalIdentity>,
    pub http: reqwest::Client,
    pub budget: DurableBudget,
    pub model: String,
    pub offer_micro_usdc: u64,
    pub max_tokens: u32,
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
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextMessage {
    role: ChatRole,
    content: String,
}

#[async_trait]
impl Executor for BuyerExecutor {
    fn prepare(&self, request: &Value) -> Result<PreparedExecution> {
        let request:Request=serde_json::from_value(request.clone()).map_err(|_|GatewayError::Invalid("expected text chat: model, messages, max_tokens, optional temperature; streaming unsupported".into()))?;
        if request.model != self.model
            || request.max_tokens == 0
            || request.max_tokens > self.max_tokens
            || request.stream
            || request.messages.is_empty()
            || request.messages.len() > 64
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
                price_micro_usdc: self.offer_micro_usdc,
                deadline_ms: 45_000,
            },
        )
        .map_err(|e| GatewayError::Invalid(e.to_string()))?;
        Ok(PreparedExecution {
            buyer: self.identity.agent_id().pubkey_base58(),
            coordinator: self.config.coordinator_url.clone(),
            job_envelopes: vec![envelope],
            maximum_cost_micro_usdc: self.offer_micro_usdc,
        })
    }
    async fn execute(
        &self,
        prepared: &PreparedExecution,
    ) -> std::result::Result<Value, ExecutionFailure> {
        let uncertain = |message: String| ExecutionFailure {
            message,
            definitely_unpaid: false,
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
                message: e.to_string(),
            })?;
        completed_response(outcome, &self.model).map_err(|e| uncertain(e.to_string()))
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
