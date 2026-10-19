//! Operator-approved upfront prices. Retail and native prices are fixed by the
//! requested output cap, never by the eventual generated token count.
use crate::{GatewayError, Result};
use covenant_compute_protocol::{InferenceTariff, InferenceTariffQuote};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelTariff {
    pub retail: InferenceTariff,
    pub native: InferenceTariff,
}

#[derive(Clone, Debug)]
pub struct TariffBook(BTreeMap<String, ModelTariff>);

impl TariffBook {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        if bytes.len() > 262_144 {
            return Err(GatewayError::Invalid("tariff file too large".into()));
        }
        Self::new(serde_json::from_slice(&bytes)?)
    }

    pub fn new(rows: Vec<ModelTariff>) -> Result<Self> {
        if rows.is_empty() || rows.len() > 32 {
            return Err(GatewayError::Invalid(
                "one to 32 model tariffs required".into(),
            ));
        }
        let mut models = BTreeMap::new();
        for row in rows {
            row.retail.validate().map_err(invalid)?;
            row.native.validate().map_err(invalid)?;
            // These quotes and receipts cross JSON into both supported SDKs.
            // Native-only tariffs can use u64; public x402 values must survive
            // JavaScript's integer representation without rounding.
            const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
            for tariff in [&row.retail, &row.native] {
                if [
                    tariff.revision,
                    tariff.base_micro_usdc,
                    tariff.output_cap_micro_usdc_per_million,
                    tariff.valid_from_ms,
                    tariff.valid_until_ms,
                    tariff.price(tariff.max_output_tokens).map_err(invalid)?,
                ]
                .iter()
                .any(|value| *value > MAX_SAFE_INTEGER)
                {
                    return Err(GatewayError::Invalid(
                        "tariff exceeds SDK-safe integer range".into(),
                    ));
                }
            }
            if row.retail.model != row.native.model
                || row.retail.max_output_tokens != row.native.max_output_tokens
                || row.retail.max_output_tokens > 32_768
                || row.retail.valid_from_ms != row.native.valid_from_ms
                || row.retail.valid_until_ms != row.native.valid_until_ms
            {
                return Err(GatewayError::Invalid(
                    "retail/native tariff scope differs".into(),
                ));
            }
            // Checking every supported cap also covers asymmetric rounding at
            // microunit boundaries; checking endpoints alone is insufficient.
            for cap in 1..=row.retail.max_output_tokens {
                if row.retail.price(cap).map_err(invalid)?
                    < row.native.price(cap).map_err(invalid)?
                {
                    return Err(GatewayError::Invalid(
                        "retail tariff cannot cover native price".into(),
                    ));
                }
            }
            if models.insert(row.retail.model.clone(), row).is_some() {
                return Err(GatewayError::Invalid("duplicate model tariff".into()));
            }
        }
        Ok(Self(models))
    }

    pub fn quote(
        &self,
        model: &str,
        cap: u32,
        now: u64,
    ) -> Result<(InferenceTariffQuote, InferenceTariffQuote)> {
        let row = self
            .0
            .get(model)
            .ok_or_else(|| GatewayError::Invalid("model has no approved tariff".into()))?;
        Ok((
            row.retail.quote(cap, now).map_err(invalid)?,
            row.native.quote(cap, now).map_err(invalid)?,
        ))
    }

    pub fn validate_single_provider(&self, model: &str, max_tokens: u32) -> Result<()> {
        if self.0.len() != 1
            || self
                .0
                .get(model)
                .is_none_or(|row| row.retail.max_output_tokens != max_tokens)
        {
            return Err(GatewayError::Invalid(
                "tariffs must match the qualified provider model and output limit".into(),
            ));
        }
        Ok(())
    }

    pub fn models(&self, now: u64) -> Vec<Value> {
        self.0.values().filter(|row| row.retail.valid_at(now)).map(|row| json!({
            "id":row.retail.model,"object":"model","owned_by":"covenant",
            "pricing":{"currency":"USDC","unit":"micro_usdc","billing":"fixed_requested_output_cap",
                "request_pricing":"model_tariff_v1","tariff":row.retail,"maximum_price_micro_usdc":row.retail.price(row.retail.max_output_tokens).ok()}
        })).collect()
    }
}

fn invalid(error: impl std::fmt::Display) -> GatewayError {
    GatewayError::Invalid(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn row() -> ModelTariff {
        let native = InferenceTariff {
            schema_version: 1,
            id: "haiku-native".into(),
            revision: 1,
            model: "haiku".into(),
            base_micro_usdc: 100,
            output_cap_micro_usdc_per_million: 1001,
            max_output_tokens: 2048,
            valid_from_ms: 10,
            valid_until_ms: 1_000_000,
        };
        let mut retail = native.clone();
        retail.id = "haiku-retail".into();
        retail.base_micro_usdc = 120;
        ModelTariff { native, retail }
    }
    #[test]
    fn quotes_bind_model_cap_revision_and_exact_rounding() {
        let book = TariffBook::new(vec![row()]).unwrap();
        let (retail, native) = book.quote("haiku", 1000, 20).unwrap();
        assert_eq!(
            (retail.price_micro_usdc, native.price_micro_usdc),
            (122, 102)
        );
        assert_eq!(book.quote("haiku", 1, 20).unwrap().0.price_micro_usdc, 121);
        assert!(book.quote("another-model", 1000, 20).is_err());
        assert!(book.quote("haiku", 2049, 20).is_err());
        assert!(book.quote("haiku", 1, 1_000_000).is_err());
        assert!(book.validate_single_provider("haiku", 2048).is_ok());
        assert!(book.validate_single_provider("haiku", 4096).is_err());
    }
    #[test]
    fn refuses_underfunded_conflicting_or_unqualified_tariffs() {
        let mut r = row();
        r.retail.base_micro_usdc = 1;
        assert!(TariffBook::new(vec![r]).is_err());
        assert!(TariffBook::new(vec![row(), row()]).is_err());
        let mut r = row();
        r.native.model = "other".into();
        assert!(TariffBook::new(vec![r]).is_err());
        let book = TariffBook::new(vec![row()]).unwrap();
        assert!(book.models(1_000_000).is_empty());
    }

    #[test]
    fn rejects_tariffs_that_clients_cannot_verify_exactly() {
        let mut r = row();
        r.native.revision = 9_007_199_254_740_992;
        assert!(TariffBook::new(vec![r]).is_err());
        let mut r = row();
        r.retail.base_micro_usdc = 9_007_199_254_740_991;
        assert!(TariffBook::new(vec![r]).is_err());
    }
}
