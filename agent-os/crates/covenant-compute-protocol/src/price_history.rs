//! Versioned source quote metadata. Quotes are sampled at provisioning, not
//! proof of provider payment, and USD must not be conflated with settled USDC.
use crate::ProtocolError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePrice {
    pub version: u8,
    pub provider: String,
    pub gpu_model: String,
    pub gpu_count: u16,
    /// Price of the entire offered instance, in millionths of USD per hour.
    pub instance_micro_usd_per_hour: u64,
    pub observed_at_ms: u64,
}

impl SourcePrice {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.version != 1
            || self.provider.is_empty()
            || self.provider.len() > 64
            || self.gpu_model.trim().is_empty()
            || self.gpu_model.len() > 128
            || self.gpu_count == 0
            || self.gpu_count > 1024
            || self.instance_micro_usd_per_hour == 0
            || self.observed_at_ms == 0
        {
            return Err(ProtocolError::Invalid(
                "invalid source price observation".into(),
            ));
        }
        Ok(())
    }
}
