//! Historical prices projected from the durable lease journal. No backfilled
//! prices, interpolation or live asks are passed off as historical settlement.
use crate::{CoordinatorState, JobRecord};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use covenant_compute_protocol::{price_history::SourcePrice, JobKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

pub(crate) fn observations(
    id: Uuid,
    record: &JobRecord,
    mint: Option<&str>,
) -> Vec<PriceObservation> {
    if record.pinned || record.envelope.payload.kind != JobKind::LeaseSession {
        return vec![];
    }
    let Some(SourcePrice {
        provider,
        gpu_model,
        gpu_count,
        instance_micro_usd_per_hour,
        observed_at_ms,
        ..
    }) = record
        .lease_access
        .as_ref()
        .and_then(|access| access.source_price.as_ref())
        .filter(|s| s.validate().is_ok())
    else {
        return vec![];
    };
    let mut rows = vec![PriceObservation {
        gpu_model: gpu_model.clone(),
        provider: provider.clone(),
        at_ms: *observed_at_ms,
        gpu_count: *gpu_count,
        amount_micros: *instance_micro_usd_per_hour,
        duration_ms: 3_600_000,
        source: "operator_reported_provider_quote",
        currency: "USD",
        proof_path: None,
    }];
    // Only actual on-chain payouts with a known metered duration can enter this
    // series. Legacy records without GPU identity/duration remain unclassified.
    if let (Some(mint), Some(elapsed), Some(at_ms)) =
        (mint, record.metered_elapsed_ms, record.concluded_at_ms)
    {
        if elapsed > 0 {
            if let Some(proof) = record
                .settlement_proof(id, mint)
                .filter(|p| p.verify_offline().is_ok())
            {
                rows.push(PriceObservation {
                    gpu_model: gpu_model.clone(),
                    provider: provider.clone(),
                    at_ms,
                    gpu_count: *gpu_count,
                    amount_micros: proof.gross_micro_usdc,
                    duration_ms: elapsed,
                    source: "settled_lease_charge",
                    currency: "USDC",
                    proof_path: Some(format!("/proof/receipts/{id}")),
                });
            }
        }
    }
    rows
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

pub fn series(rows: Vec<PriceObservation>, since_ms: u64, until_ms: u64) -> Vec<PricePoint> {
    let mut buckets =
        BTreeMap::<(String, String, &'static str, &'static str, u64), Vec<PriceObservation>>::new();
    for row in rows {
        if row.at_ms < since_ms
            || row.at_ms > until_ms
            || row.gpu_count == 0
            || row.duration_ms == 0
        {
            continue;
        }
        buckets
            .entry((
                row.gpu_model.clone(),
                row.provider.clone(),
                row.source,
                row.currency,
                row.at_ms / 86_400_000 * 86_400_000,
            ))
            .or_default()
            .push(row);
    }
    buckets
        .into_iter()
        .map(
            |((gpu_model, provider, source, currency, day_start_ms), rows)| {
                let gpu_ms: u128 = rows
                    .iter()
                    .map(|r| r.duration_ms as u128 * r.gpu_count as u128)
                    .sum();
                let amount: u128 = rows.iter().map(|r| r.amount_micros as u128).sum();
                let rate = |r: &PriceObservation| {
                    ((r.amount_micros as u128 * 3_600_000)
                        / (r.duration_ms as u128 * r.gpu_count as u128))
                        .min(u64::MAX as u128) as u64
                };
                PricePoint {
                    gpu_model,
                    provider,
                    source,
                    currency,
                    day_start_ms,
                    first_observed_at_ms: rows.iter().map(|r| r.at_ms).min().unwrap(),
                    last_observed_at_ms: rows.iter().map(|r| r.at_ms).max().unwrap(),
                    sample_count: rows.len() as u64,
                    mean_micros_per_gpu_hour: (amount * 3_600_000 / gpu_ms).min(u64::MAX as u128)
                        as u64,
                    min_micros_per_gpu_hour: rows.iter().map(rate).min().unwrap(),
                    max_micros_per_gpu_hour: rows.iter().map(rate).max().unwrap(),
                    proof_paths: rows.into_iter().filter_map(|r| r.proof_path).collect(),
                }
            },
        )
        .collect()
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
    pub generated_at_ms: u64,
    pub unit: &'static str,
    pub coverage: &'static str,
    pub points: Vec<PricePoint>,
}
pub async fn history(
    State(state): State<CoordinatorState>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<History>, (StatusCode, &'static str)> {
    if !state.config().public_proof_feed {
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
    let mint = state.payout().payout_mint_b58();
    Ok(Json(History {version: 1, coordinator_pubkey_b58: state.escrow().coordinator_pubkey_b58(),
        settlement_mint_b58: mint.map(str::to_owned), generated_at_ms: now,
        unit: "millionths of the named currency per GPU-hour",
        coverage: "provider quotes sampled at successful provisioning; settled charges require an on-chain payout and known GPU/time; historical source coverage begins with this deployment; provider quotes are not invoices",
        points: series(state.jobs().price_observations(mint), since, until),
    }))
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
}
