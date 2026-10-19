//! Versioned fixed inference prices: base + requested output-token cap, rounded up.
//! This is a pre-agreed whole-job price, not a claim of measured token usage.
use crate::sign::ProtocolError;
use covenant_mcp::Content;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceTariff {
    pub schema_version: u32,
    pub id: String,
    pub revision: u64,
    pub model: String,
    pub base_micro_usdc: u64,
    pub output_cap_micro_usdc_per_million: u64,
    pub max_output_tokens: u32,
    pub valid_from_ms: u64,
    pub valid_until_ms: u64,
}
impl InferenceTariff {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != 1
            || self.revision == 0
            || self.id.is_empty()
            || self.id.len() > 128
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || self.model.trim().is_empty()
            || self.model == "any"
            || self.model.len() > 128
            || self.model.chars().any(char::is_control)
            || self.max_output_tokens == 0
            || self.valid_until_ms <= self.valid_from_ms
        {
            return Err(ProtocolError::Invalid(
                "invalid inference tariff version, identity, output cap, or validity window".into(),
            ));
        }
        self.price(self.max_output_tokens)?;
        Ok(())
    }
    pub fn valid_at(&self, now_ms: u64) -> bool {
        self.valid_from_ms <= now_ms && now_ms < self.valid_until_ms
    }
    pub fn price(&self, output_cap: u32) -> Result<u64, ProtocolError> {
        if output_cap == 0 || output_cap > self.max_output_tokens {
            return Err(ProtocolError::Invalid(
                "requested output cap is outside the tariff range".into(),
            ));
        }
        let variable = (u128::from(output_cap)
            * u128::from(self.output_cap_micro_usdc_per_million))
        .div_ceil(1_000_000);
        let price = u64::try_from(u128::from(self.base_micro_usdc) + variable).map_err(|_| {
            ProtocolError::Invalid("inference tariff price overflows micro-USDC".into())
        })?;
        if price == 0 {
            return Err(ProtocolError::Invalid(
                "inference tariff price must be positive".into(),
            ));
        }
        Ok(price)
    }
    pub fn quote(
        &self,
        output_cap: u32,
        now_ms: u64,
    ) -> Result<InferenceTariffQuote, ProtocolError> {
        self.validate()?;
        if !self.valid_at(now_ms) {
            return Err(ProtocolError::Invalid(
                "inference tariff is not currently valid".into(),
            ));
        }
        Ok(InferenceTariffQuote {
            tariff: self.clone(),
            requested_output_cap: output_cap,
            price_micro_usdc: self.price(output_cap)?,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceTariffQuote {
    pub tariff: InferenceTariff,
    pub requested_output_cap: u32,
    pub price_micro_usdc: u64,
}
impl InferenceTariffQuote {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.tariff.validate()?;
        if self.tariff.price(self.requested_output_cap)? != self.price_micro_usdc {
            return Err(ProtocolError::Invalid(
                "inference quote price disagrees with its signed tariff".into(),
            ));
        }
        Ok(())
    }
}
/// Each offer keeps a real operator's hardware and tariff together. It is an
/// anonymous discovery claim, not hardware attestation or a capacity reservation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferenceTariffOffer {
    pub tariff: InferenceTariff,
    pub gpu_class: String,
    pub vram_gb: u32,
}
pub fn tariff_input(quote: &InferenceTariffQuote) -> Result<Content, ProtocolError> {
    quote.validate()?;
    Ok(Content::Json {
        value: serde_json::json!({"inference_tariff": quote}),
    })
}
pub fn parse_tariff_input(
    input: &[Content],
) -> Result<Option<InferenceTariffQuote>, ProtocolError> {
    let mut found = None;
    for block in input {
        if let Content::Json { value } = block {
            if let Some(value) = value.get("inference_tariff") {
                if found.is_some() {
                    return Err(ProtocolError::Invalid(
                        "duplicate inference tariff block".into(),
                    ));
                }
                let quote: InferenceTariffQuote = serde_json::from_value(value.clone())
                    .map_err(|e| ProtocolError::Invalid(format!("inference tariff: {e}")))?;
                quote.validate()?;
                found = Some(quote);
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tariff() -> InferenceTariff {
        InferenceTariff {
            schema_version: 1,
            id: "measured-model".into(),
            revision: 1,
            model: "model".into(),
            base_micro_usdc: 100,
            output_cap_micro_usdc_per_million: 1001,
            max_output_tokens: 4096,
            valid_from_ms: 10,
            valid_until_ms: 100,
        }
    }
    #[test]
    fn fixed_output_cap_price_rounds_up_and_is_bounded() {
        let t = tariff();
        assert_eq!(t.price(1000).unwrap(), 102);
        assert_eq!(t.price(1).unwrap(), 101);
        assert!(t.quote(0, 20).is_err());
        assert!(t.quote(4097, 20).is_err());
        assert!(t.quote(1, 9).is_err());
        assert!(t.quote(1, 100).is_err());
        let mut t = t;
        t.base_micro_usdc = u64::MAX;
        assert!(t.quote(1, 20).is_err());
    }
    #[test]
    fn tampered_prices_versions_and_duplicate_blocks_fail() {
        let q = tariff().quote(1000, 20).unwrap();
        let block = tariff_input(&q).unwrap();
        assert_eq!(
            parse_tariff_input(std::slice::from_ref(&block)).unwrap(),
            Some(q.clone())
        );
        assert!(parse_tariff_input(&[block.clone(), block]).is_err());
        let mut changed = q;
        changed.price_micro_usdc += 1;
        assert!(changed.validate().is_err());
        let mut t = tariff();
        t.schema_version = 2;
        assert!(t.quote(1000, 20).is_err());
    }
}

#[cfg(test)]
mod binding_tests {
    use super::*;
    use crate::*;
    use covenant_a2a::{A2ADuplicateSafety, A2AIdempotency};
    use covenant_identity::LocalIdentity;
    fn payload() -> JobEnvelopePayload {
        let tariff = InferenceTariff {
            schema_version: 1,
            id: "measured".into(),
            revision: 1,
            model: "model".into(),
            base_micro_usdc: 100,
            output_cap_micro_usdc_per_million: 1_000_000,
            max_output_tokens: 1000,
            valid_from_ms: 10,
            valid_until_ms: 100,
        };
        let quote = tariff.quote(32, 20).unwrap();
        JobEnvelopePayload {
            job_id: uuid::Uuid::nil(),
            buyer: LocalIdentity::generate("buyer@local").agent_id(),
            kind: JobKind::InferenceCall,
            capability_requirement: CapabilityRequirement {
                inference_tariff: Some(quote.clone()),
                gpu_class: None,
                min_vram_gb: None,
                model_id: Some("model".into()),
                kind: JobKind::InferenceCall,
                max_duration_secs: 60,
                min_reputation_bps: None,
            },
            input: vec![
                Content::text("prompt"),
                generation_input(GenerationParams {
                    max_tokens: Some(32),
                    ..Default::default()
                })
                .unwrap(),
                tariff_input(&quote).unwrap(),
            ],
            price_micro_usdc: 132,
            deadline_ms: 60_000,
            idempotency: A2AIdempotency::new(A2ADuplicateSafety::Idempotent, "test"),
            issued_at_ms: 20,
            referral_code: None,
            stream: false,
        }
    }
    #[test]
    fn signed_tariff_binds_model_output_cap_price_version_and_time() {
        let p = payload();
        p.validate_input().unwrap();
        let mut changed = p.clone();
        changed.price_micro_usdc += 1;
        assert!(changed.validate_input().is_err());
        let mut changed = p.clone();
        changed.capability_requirement.model_id = Some("other".into());
        assert!(changed.validate_input().is_err());
        let mut changed = p.clone();
        changed.input[1] = generation_input(GenerationParams {
            max_tokens: Some(33),
            ..Default::default()
        })
        .unwrap();
        assert!(changed.validate_input().is_err());
        let mut changed = p.clone();
        changed.issued_at_ms = 100;
        assert!(changed.validate_input().is_err());
        let mut changed = p.clone();
        changed.capability_requirement.inference_tariff = None;
        assert!(changed.validate_input().is_err());
    }
    #[test]
    fn real_rust_signed_tariff_fixture() {
        // Reuse the accepted legacy output bytes, then sign the new native
        // request contract and its matching receipt through production Rust.
        let mut fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../sdk/compute-x402-client/test/fixtures/native-response.json"
        ))
        .unwrap();
        fixture["request"]["pricing"] = serde_json::json!("model_tariff_v1");
        let buyer = LocalIdentity::generate("tariff-fixture-buyer@local");
        let operator = LocalIdentity::generate("tariff-fixture-operator@local");
        let mut payload: JobEnvelopePayload =
            serde_json::from_value(fixture["response"]["covenant"]["envelope"]["payload"].clone())
                .unwrap();
        let tariff = InferenceTariff {
            schema_version: 1,
            id: "fixture-model-v1".into(),
            revision: 1,
            model: "fixture-model".into(),
            base_micro_usdc: 100,
            output_cap_micro_usdc_per_million: 1_000_001,
            max_output_tokens: 4096,
            valid_from_ms: payload.issued_at_ms - 1000,
            valid_until_ms: payload.issued_at_ms + 1_000_000,
        };
        let quote = tariff.quote(64, payload.issued_at_ms).unwrap();
        payload.buyer = buyer.agent_id();
        payload.price_micro_usdc = quote.price_micro_usdc;
        payload.capability_requirement.inference_tariff = Some(quote.clone());
        payload.input.push(tariff_input(&quote).unwrap());
        payload.validate_input().unwrap();
        let envelope = SignedJobEnvelope::sign(payload, &buyer).unwrap();
        envelope.verify().unwrap();
        let mut receipt: WorkReceiptPayload =
            serde_json::from_value(fixture["response"]["covenant"]["receipt"]["receipt"].clone())
                .unwrap();
        receipt.operator = operator.agent_id();
        receipt.job_hash_hex = crate::sha256_hex(envelope.payload_json.as_bytes());
        receipt.price_micro_usdc = quote.price_micro_usdc;
        let receipt = SignedWorkReceipt::sign(receipt, &operator).unwrap();
        receipt.verify().unwrap();
        let request_hash = crate::sha256_hex(&serde_json::to_vec(&fixture["request"]).unwrap());
        fixture["response"]["covenant"]["envelope"] = serde_json::to_value(envelope).unwrap();
        fixture["response"]["covenant"]["receipt"] = serde_json::to_value(receipt).unwrap();
        fixture["response"]["covenant"]["request_sha256"] = serde_json::json!(request_hash);
        if let Ok(path) = std::env::var("COVENANT_TARIFF_FIXTURE_OUTPUT") {
            std::fs::write(path, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
        }
    }

    #[test]
    fn legacy_requirement_bytes_and_signature_are_unchanged() {
        const LEGACY: &str = r#"{"gpu_class":null,"min_vram_gb":null,"model_id":"model","kind":"inference_call","max_duration_secs":60,"min_reputation_bps":null}"#;
        let req: CapabilityRequirement = serde_json::from_str(LEGACY).unwrap();
        assert!(req.inference_tariff.is_none());
        assert_eq!(crate::sign::to_canonical_json(&req).unwrap(), LEGACY);
        let id = LocalIdentity::generate("legacy@local");
        let mut p = payload();
        p.buyer = id.agent_id();
        p.capability_requirement = req;
        p.input = vec![Content::text("prompt")];
        let signed = SignedJobEnvelope::sign(p, &id).unwrap();
        assert!(!signed.payload_json.contains("inference_tariff"));
        let decoded: SignedJobEnvelope =
            serde_json::from_str(&serde_json::to_string(&signed).unwrap()).unwrap();
        decoded.verify().unwrap();
        assert_eq!(signed.payload_json, decoded.payload_json);
    }
    #[test]
    fn profile_requires_exact_snapshot_and_expiry_at_admission() {
        let p = payload();
        let quote = p.capability_requirement.inference_tariff.clone().unwrap();
        let mut profile = CapabilityProfile {
            inference_tariffs: vec![quote.tariff.clone()],
            operator: p.buyer.clone(),
            hardware: HardwareClass::CpuOnly,
            vram_gb: 0,
            models_served: vec!["model".into()],
            job_kinds: vec![JobKind::InferenceCall],
            price: PriceAsk {
                unit: PriceUnit::PerMillionTokens,
                micro_usdc: 9999,
            },
            tee_capable: false,
            kind_prices: vec![],
            kind_models: vec![],
        };
        profile.validate_labels().unwrap();
        assert!(profile.satisfies_at(&p.capability_requirement, 20));
        assert!(!profile.satisfies_at(&p.capability_requirement, 100));
        assert_eq!(profile.job_floor_micro_usdc(&p.capability_requirement), 132);
        let mut legacy = p.capability_requirement.clone();
        legacy.inference_tariff = None;
        assert_eq!(profile.job_floor_micro_usdc(&legacy), 9999);
        profile.inference_tariffs[0].base_micro_usdc += 1;
        assert!(!profile.satisfies(&p.capability_requirement));
        profile.inference_tariffs.clear();
        let bytes = crate::sign::to_canonical_json(&profile).unwrap();
        assert!(!bytes.contains("inference_tariffs"));
        assert_eq!(
            crate::sign::to_canonical_json(
                &serde_json::from_str::<CapabilityProfile>(&bytes).unwrap()
            )
            .unwrap(),
            bytes
        );
    }
}

#[cfg(test)]
mod sdk_vectors {
    use super::*;
    #[test]
    fn cross_language_versioned_tariff_vectors() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../fixtures/compute/inference-tariff-v1.json"
        ))
        .unwrap();
        for row in fixture["cases"].as_array().unwrap() {
            let tariff: Option<InferenceTariff> =
                serde_json::from_value(row["tariff"].clone()).unwrap();
            let expected_revision = row
                .get("expected_revision")
                .and_then(serde_json::Value::as_u64);
            let outcome: Result<Option<InferenceTariffQuote>, ProtocolError> = (|| {
                let Some(tariff) = tariff else {
                    return if expected_revision.is_some() {
                        Err(ProtocolError::Invalid("missing pinned tariff".into()))
                    } else {
                        Ok(None)
                    };
                };
                if expected_revision.is_some_and(|revision| revision != tariff.revision) {
                    return Err(ProtocolError::Invalid("tariff revision mismatch".into()));
                }
                tariff
                    .quote(
                        row["requested_output_cap"]
                            .as_u64()
                            .unwrap()
                            .try_into()
                            .unwrap(),
                        row["now_ms"].as_u64().unwrap(),
                    )
                    .map(Some)
            })();
            if row["error"].as_bool() == Some(true) {
                assert!(outcome.is_err(), "{}", row["name"]);
            } else {
                let quote = outcome.unwrap();
                assert_eq!(
                    quote.as_ref().map(|q| q.price_micro_usdc),
                    row["price_micro_usdc"].as_u64(),
                    "{}",
                    row["name"]
                );
                if let Some(quote) = quote {
                    assert_eq!(serde_json::to_value(quote.tariff).unwrap(), row["tariff"]);
                }
            }
        }
    }
}
