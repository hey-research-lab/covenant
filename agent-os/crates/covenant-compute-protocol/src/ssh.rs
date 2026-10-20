//! Structured SSH observations for lease-bound hardware evidence.
//! A discovered key does not authenticate a guest. Hardware verification must
//! bind it to the measured workload and its live lease before it earns trust.
use crate::sign::ProtocolError;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshHostKey {
    pub algorithm: String,
    pub key_base64: String,
}
impl SshHostKey {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if !["ssh-ed25519", "ecdsa-sha2-nistp256", "ssh-rsa"].contains(&self.algorithm.as_str())
            || self.key_base64.is_empty()
            || self.key_base64.len() > 16384
            || !self
                .key_base64
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
        {
            return Err(ProtocolError::Invalid(
                "invalid SSH host key encoding".into(),
            ));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.key_base64)
            .map_err(|_| ProtocolError::Invalid("invalid SSH host key base64".into()))?;
        let mut wire = bytes.as_slice();
        let algorithm_matches = ssh_string(&mut wire) == Some(self.algorithm.as_bytes());
        let key_matches = match self.algorithm.as_str() {
            "ssh-ed25519" => ssh_string(&mut wire).is_some_and(|key| key.len() == 32),
            "ecdsa-sha2-nistp256" => {
                ssh_string(&mut wire) == Some(b"nistp256".as_slice())
                    && ssh_string(&mut wire).is_some_and(|point| point.len() == 65 && point[0] == 4)
            }
            "ssh-rsa" => {
                ssh_string(&mut wire).is_some_and(positive_mpint)
                    && ssh_string(&mut wire).is_some_and(positive_mpint)
            }
            _ => false,
        };
        if !algorithm_matches || !key_matches || !wire.is_empty() {
            return Err(ProtocolError::Invalid(
                "SSH host key bytes do not match the algorithm".into(),
            ));
        }
        Ok(())
    }
}
fn ssh_string<'a>(wire: &mut &'a [u8]) -> Option<&'a [u8]> {
    let length = u32::from_be_bytes(wire.get(..4)?.try_into().ok()?) as usize;
    let end = 4usize.checked_add(length)?;
    let value = wire.get(4..end)?;
    *wire = wire.get(end..)?;
    Some(value)
}
fn positive_mpint(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes.len() <= 1025
        && bytes[0] & 0x80 == 0
        && bytes.iter().any(|b| *b != 0)
        && (bytes[0] != 0 || bytes.get(1).is_some_and(|b| b & 0x80 != 0))
}
pub fn validate_ssh_host(host: &str) -> Result<(), ProtocolError> {
    if host.is_empty()
        || host.len() > 253
        || host.starts_with('-')
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b))
    {
        return Err(ProtocolError::Invalid(
            "invalid SSH hostname/address".into(),
        ));
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshAccess {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub lease_generation: String,
    /// Unauthenticated discovery from the remote SSH service. Never an attestation.
    pub observed_host_keys: Vec<SshHostKey>,
}
impl SshAccess {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_ssh_host(&self.host)?;
        if self.port == 0
            || self.username.is_empty()
            || self.username.len() > 64
            || !self
                .username
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || self.lease_generation.is_empty()
            || self.lease_generation.len() > 128
            || self.lease_generation.chars().any(char::is_control)
            || self.observed_host_keys.len() > 8
        {
            return Err(ProtocolError::Invalid(
                "invalid structured SSH access".into(),
            ));
        }
        for key in &self.observed_host_keys {
            key.validate()?;
        }
        Ok(())
    }
    /// A host-key pin usable by OpenSSH after the caller chooses its trust policy.
    /// Formatting this does not authenticate an observed key.
    pub fn known_hosts(&self) -> Result<String, ProtocolError> {
        self.validate()?;
        let host = if self.port == 22 && !self.host.contains(':') {
            self.host.clone()
        } else {
            format!("[{}]:{}", self.host, self.port)
        };
        Ok(self
            .observed_host_keys
            .iter()
            .map(|key| format!("{host} {} {}\n", key.algorithm, key.key_base64))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access() -> SshAccess {
        let mut bytes = vec![0, 0, 0, 11];
        bytes.extend_from_slice(b"ssh-ed25519");
        bytes.extend_from_slice(&[0, 0, 0, 32]);
        bytes.extend_from_slice(&[7; 32]);
        SshAccess {
            host: "gpu.example".into(),
            port: 2222,
            username: "worker".into(),
            lease_generation: "instance-1".into(),
            observed_host_keys: vec![SshHostKey {
                algorithm: "ssh-ed25519".into(),
                key_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            }],
        }
    }

    #[test]
    fn known_hosts_preserves_exact_key_and_endpoint_without_claiming_trust() {
        let a = access();
        assert_eq!(
            a.known_hosts().unwrap(),
            format!(
                "[gpu.example]:2222 ssh-ed25519 {}\n",
                a.observed_host_keys[0].key_base64
            )
        );
        let mut ipv6 = a;
        ipv6.host = "2001:db8::1".into();
        ipv6.port = 22;
        assert!(ipv6.known_hosts().unwrap().starts_with("[2001:db8::1]:22 "));
    }

    #[test]
    fn endpoint_and_key_injection_or_malformed_wire_keys_are_rejected() {
        for host in [
            "-oProxyCommand=evil",
            "gpu\nother",
            "$(evil)",
            "gpu;evil",
            "user@host",
        ] {
            let mut a = access();
            a.host = host.into();
            assert!(a.known_hosts().is_err());
        }
        let mut a = access();
        a.observed_host_keys[0].algorithm = "ssh-rsa".into();
        assert!(a.validate().is_err());
        let mut a = access();
        a.observed_host_keys[0].key_base64 = "AAAA".into();
        assert!(a.validate().is_err());
        let mut a = access();
        a.lease_generation = "instance\nother".into();
        assert!(a.validate().is_err());
    }
}
