//! Opt-in tariff discovery and pre-signing pricing. No guessed/default rates.
use crate::{capacity_filtered, epoch_ms, BuyerConfig, BuyerError, InferArgs};
use covenant_compute_protocol::{
    canonical_model, parse_generation_params, tariff_input, InferenceTariffQuote, JobKind,
};
use covenant_mcp::Content;

/// Choose the cheapest currently live revision of an explicit tariff ID. Hardware
/// is filtered on each offer rather than the directory row's union/max fields.
/// This read is not a reservation: matching/admission recheck the exact snapshot.
#[allow(clippy::too_many_arguments)]
pub async fn quote_inference_tariff(
    http: &reqwest::Client,
    config: &BuyerConfig,
    tariff_id: &str,
    model: &str,
    output_cap: u32,
    gpu_class: Option<&str>,
    min_vram_gb: Option<u32>,
    min_reputation_bps: Option<u32>,
    ceiling_micro_usdc: u64,
) -> Result<InferenceTariffQuote, BuyerError> {
    let view = capacity_filtered(http, config, min_reputation_bps).await?;
    let mut quotes: Vec<_> = view
        .entries
        .iter()
        .filter(|row| row.kind == JobKind::InferenceCall)
        .flat_map(|row| &row.inference_tariffs)
        .filter(|offer| {
            offer.tariff.id == tariff_id
                && canonical_model(&offer.tariff.model) == canonical_model(model)
        })
        .filter(|offer| gpu_class.is_none_or(|gpu| offer.gpu_class.eq_ignore_ascii_case(gpu)))
        .filter(|offer| min_vram_gb.is_none_or(|vram| offer.vram_gb >= vram))
        .filter_map(|offer| offer.tariff.quote(output_cap, epoch_ms()).ok())
        .filter(|quote| quote.price_micro_usdc <= ceiling_micro_usdc)
        .collect();
    quotes.sort_by(|a, b| {
        a.price_micro_usdc
            .cmp(&b.price_micro_usdc)
            .then(b.tariff.revision.cmp(&a.tariff.revision))
    });
    quotes.into_iter().next().ok_or_else(|| BuyerError::Protocol(
        "no currently live inference tariff satisfies the requested ID, model, output cap, hardware, reputation and price ceiling".into()))
}

#[allow(clippy::too_many_arguments)]
pub async fn price_input(
    http: &reqwest::Client,
    config: &BuyerConfig,
    input: &mut Vec<Content>,
    tariff_id: &str,
    model: Option<&str>,
    gpu_class: Option<&str>,
    min_vram_gb: Option<u32>,
    min_reputation_bps: Option<u32>,
    explicit_price: Option<u64>,
    ceiling_micro_usdc: u64,
) -> Result<u64, BuyerError> {
    let model = model.ok_or_else(|| {
        BuyerError::Protocol("an inference tariff requires an explicit model".into())
    })?;
    let cap = parse_generation_params(input)
        .map_err(|e| BuyerError::Protocol(e.to_string()))?
        .and_then(|params| params.max_tokens)
        .ok_or_else(|| {
            BuyerError::Protocol(
                "an inference tariff requires an explicit max_tokens output cap".into(),
            )
        })?;
    if covenant_compute_protocol::parse_tariff_input(input)
        .map_err(|e| BuyerError::Protocol(e.to_string()))?
        .is_some()
    {
        return Err(BuyerError::Protocol(
            "input already contains a tariff quote".into(),
        ));
    }
    let quote = quote_inference_tariff(
        http,
        config,
        tariff_id,
        model,
        cap,
        gpu_class,
        min_vram_gb,
        min_reputation_bps,
        ceiling_micro_usdc,
    )
    .await?;
    if explicit_price.is_some_and(|price| price != quote.price_micro_usdc) {
        return Err(BuyerError::Protocol(
            "explicit price must equal the selected inference tariff quote".into(),
        ));
    }
    let price = quote.price_micro_usdc;
    input.push(tariff_input(&quote).map_err(|e| BuyerError::Protocol(e.to_string()))?);
    Ok(price)
}

/// Resolves before any spend-cap reservation or signature. Non-tariff calls follow
/// the existing pricing path exactly. Native clients can call this same function.
pub async fn prepare_infer_args(
    http: &reqwest::Client,
    config: &BuyerConfig,
    args: &mut InferArgs,
    ceiling_micro_usdc: u64,
) -> Result<Vec<Content>, BuyerError> {
    prepare_infer_args_for_purchase(http, config, args, ceiling_micro_usdc, None).await
}

pub async fn prepare_infer_args_for_purchase(
    http: &reqwest::Client,
    config: &BuyerConfig,
    args: &mut InferArgs,
    ceiling_micro_usdc: u64,
    existing: Option<&crate::SignedJobEnvelope>,
) -> Result<Vec<Content>, BuyerError> {
    let tariff_id = args.tariff_id.take();
    let mut input = args.input().map_err(BuyerError::Protocol)?;
    if let Some(id) = tariff_id {
        if let Some(existing) = existing {
            args.price_micro_usdc = Some(replay_tariff_quote(
                &mut input,
                existing,
                &id,
                args.price_micro_usdc,
            )?);
            return Ok(input);
        }
        args.price_micro_usdc = Some(
            price_input(
                http,
                config,
                &mut input,
                &id,
                args.model.as_deref(),
                args.gpu_class.as_deref(),
                args.min_vram_gb,
                args.min_reputation_bps,
                args.price_micro_usdc,
                ceiling_micro_usdc,
            )
            .await?,
        );
    }
    Ok(input)
}

/// A retry retains the original signed snapshot even after its validity window
/// has elapsed. The purchase book still checks all original input/routing fields;
/// this function does not authorize a new dispatch using an expired quote.
pub fn replay_tariff_quote(
    input: &mut Vec<Content>,
    existing: &crate::SignedJobEnvelope,
    id: &str,
    explicit_price: Option<u64>,
) -> Result<u64, BuyerError> {
    let quote = existing
        .payload
        .capability_requirement
        .inference_tariff
        .as_ref()
        .ok_or_else(|| {
            BuyerError::Protocol("idempotency key names a legacy purchase without a tariff".into())
        })?;
    if quote.tariff.id != id
        || explicit_price.is_some_and(|p| p != quote.price_micro_usdc)
        || parse_generation_params(input)
            .map_err(|e| BuyerError::Protocol(e.to_string()))?
            .and_then(|p| p.max_tokens)
            != Some(quote.requested_output_cap)
    {
        return Err(BuyerError::Protocol(
            "idempotency key's tariff, price or requested output cap differs".into(),
        ));
    }
    input.push(tariff_input(quote).map_err(|e| BuyerError::Protocol(e.to_string()))?);
    Ok(quote.price_micro_usdc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_compute_protocol::{generation_input, GenerationParams};
    #[tokio::test]
    async fn missing_model_or_output_cap_fails_before_any_network() {
        let config = BuyerConfig {
            coordinator_url: "http://127.0.0.1:1".into(),
            poll_interval: std::time::Duration::from_millis(1),
            referral_code: None,
            rpc_url: None,
        };
        let http = reqwest::Client::new();
        let mut input = vec![Content::text("prompt")];
        let err = price_input(
            &http,
            &config,
            &mut input,
            "measured",
            Some("model"),
            None,
            None,
            None,
            None,
            100,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("max_tokens"));
        input.push(
            generation_input(GenerationParams {
                max_tokens: Some(10),
                ..Default::default()
            })
            .unwrap(),
        );
        assert!(price_input(
            &http, &config, &mut input, "measured", None, None, None, None, None, 100
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("model"));
    }
}

#[cfg(test)]
mod discovery_tests {
    use super::*;
    use axum::{routing::get, Json, Router};
    use serde_json::json;
    #[tokio::test]
    async fn live_quote_keeps_real_hardware_pairing_and_never_falls_back_to_ceiling() {
        let now = epoch_ms();
        let tariff = |base| json!({"schema_version":1,"id":"measured","revision":1,"model":"model","base_micro_usdc":base,"output_cap_micro_usdc_per_million":1_000_000,"max_output_tokens":1000,"valid_from_ms":now-1000,"valid_until_ms":now+60_000});
        let body = json!({"registered_operators":2,"matchable_operators":2,"liveness_window_ms":10000,"min_score_bps":0,"min_bond_micro_usdc":0,"entries":[{"kind":"inference_call","model":"model","operators":2,"min_ask_micro_usdc":1,"max_ask_micro_usdc":999,"min_ask_unit":"per_job","max_vram_gb":80,"gpu_classes":["cpu","h100"],"tee_capable":false,"inference_tariffs":[{"tariff":tariff(1),"gpu_class":"cpu","vram_gb":0},{"tariff":tariff(100),"gpu_class":"h100","vram_gb":80}]}]});
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/federation/capacity",
                    get(move || {
                        let body = body.clone();
                        async move { Json(body) }
                    }),
                ),
            )
            .await
            .unwrap()
        });
        let config = BuyerConfig {
            coordinator_url: format!("http://{address}"),
            poll_interval: std::time::Duration::from_millis(1),
            referral_code: None,
            rpc_url: None,
        };
        let http = reqwest::Client::new();
        let q = quote_inference_tariff(
            &http,
            &config,
            "measured",
            "model:latest",
            32,
            Some("H100"),
            Some(80),
            None,
            200,
        )
        .await
        .unwrap();
        assert_eq!(q.price_micro_usdc, 132);
        assert!(quote_inference_tariff(
            &http,
            &config,
            "measured",
            "model",
            32,
            Some("H100"),
            Some(80),
            None,
            100
        )
        .await
        .is_err());
        assert!(quote_inference_tariff(
            &http, &config, "unknown", "model", 32, None, None, None, 9999
        )
        .await
        .is_err());
        server.abort();
    }
}
