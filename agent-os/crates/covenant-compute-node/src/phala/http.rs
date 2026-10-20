//! Bounded Cloud API transport. No automatic HTTP mutation retries, redirects,
//! implicit resource selection, caller-supplied callback URLs or shell bridge.
use super::*;
use covenant_compute_protocol::ssh::SshHostKey;
use reqwest::{Method, StatusCode};
use ring::{aead, agreement};
use serde_json::json;

const MAX_RESPONSE: usize = 512 * 1024;
const BASE: &str = "https://cloud-api.phala.com/api/v1";

pub struct PhalaCloud {
    client: reqwest::Client,
    base: String,
    api_key: String,
    teepod_id: u64,
    image: String,
    kms_id: String,
}

impl PhalaCloud {
    pub fn new(
        api_key: String,
        teepod_id: u64,
        image: String,
        kms_id: String,
    ) -> Result<Self, ExecutorError> {
        if api_key.trim() != api_key
            || api_key.is_empty()
            || api_key.len() > 4096
            || api_key.bytes().any(|b| b.is_ascii_control())
            || teepod_id == 0
            || image.is_empty()
            || image.len() > 128
            || !image
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b))
            || kms_id.is_empty()
            || kms_id.len() > 128
            || !kms_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err(failed(
                "invalid explicit Phala API, node, OS or KMS configuration",
            ));
        }
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("Covenant-Compute/1")
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(20))
                .connect_timeout(Duration::from_secs(8))
                .build()
                .map_err(|_| failed("build Phala HTTP client"))?,
            base: BASE.into(),
            api_key,
            teepod_id,
            image,
            kms_id,
        })
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Option<Value>, ExecutorError> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base))
            .header("X-API-Key", &self.api_key)
            .header("X-Phala-Version", "2026-05-22")
            .header("Cache-Control", "no-cache");
        if let Some(body) = body {
            request = request.json(&body);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| failed("Phala API transport outcome unknown"))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(failed(format!(
                "Phala API refused request (HTTP {})",
                response.status().as_u16()
            )));
        }
        if response.status() == StatusCode::NO_CONTENT {
            return Ok(Some(Value::Null));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| failed("Phala response incomplete"))?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE {
                return Err(failed("oversized Phala response"));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| failed("invalid Phala response JSON"))
    }

    async fn machine(&self, reference: &str) -> Result<Option<Machine>, ExecutorError> {
        let Some(value) = self
            .request(Method::GET, &format!("/cvms/{reference}"), None)
            .await?
        else {
            return Ok(None);
        };
        let id = value
            .get("vm_uuid")
            .and_then(Value::as_str)
            .and_then(|v| Uuid::parse_str(v).ok())
            .filter(|v| !v.is_nil())
            .ok_or_else(|| failed("provider returned no immutable VM UUID"))?;
        let string = |key| {
            value
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| failed("provider machine fields missing"))
        };
        Ok(Some(Machine {
            vm_uuid: id,
            app_id: string("app_id")?.trim_start_matches("0x").into(),
            name: string("name")?,
            compose_hash: value
                .get("compose_hash")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            instance_id: value
                .get("instance_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
            running: value.get("status").and_then(Value::as_str) == Some("running")
                && value.get("in_progress").and_then(Value::as_bool) == Some(false),
        }))
    }
    async fn checked_rate(&self, plan: &LeasePlan) -> Result<u64, ExecutorError> {
        let value = self
            .request(Method::GET, "/instance-types/cpu", None)
            .await?
            .ok_or_else(|| failed("Phala CPU pricing unavailable"))?;
        let items = value["items"]
            .as_array()
            .ok_or_else(|| failed("invalid Phala CPU price catalog"))?;
        let mut matching = items
            .iter()
            .filter(|item| item["id"] == plan.allowance.instance_type);
        let item = matching
            .next()
            .ok_or_else(|| failed("reviewed Phala CPU type unavailable"))?;
        if matching.next().is_some()
            || item["vcpu"].as_u64() != Some(1)
            || item["memory_mb"].as_u64() != Some(2048)
            || item["requires_gpu"] != false
            || item["default_disk_size_gb"].as_u64() != Some(20)
        {
            return Err(failed(
                "Phala CPU type differs from the qualified resource profile",
            ));
        }
        let rate = decimal_micro_usd(
            item["hourly_rate"]
                .as_str()
                .ok_or_else(|| failed("Phala hourly rate must be a decimal string"))?,
        )?;
        if rate == 0 || rate > plan.allowance.max_hourly_micro_usd {
            return Err(failed("Phala hourly rate exceeds the reviewed allowance"));
        }
        Ok(rate)
    }
}

#[async_trait]
impl PhalaProvider for PhalaCloud {
    async fn health(&self) -> Result<(), ExecutorError> {
        // Authentication only. Never provision a boot benchmark.
        self.request(Method::GET, "/cvms", None)
            .await?
            .filter(Value::is_array)
            .ok_or_else(|| failed("Phala account listing unavailable"))?;
        Ok(())
    }
    async fn provision(&self, plan: &LeasePlan) -> Result<Provision, ExecutorError> {
        let hourly_micro_usd = self.checked_rate(plan).await?;
        let mut compose: Value =
            serde_json::from_str(&plan.manifest).map_err(|_| failed("invalid manifest"))?;
        // API AppComposeV2 is not the measured v3 wrapper. These are its typed
        // carrier fields; only an exactly matching returned hash can authorize
        // create. Provider normalization cannot silently change the buyer pin.
        compose["manifest_version"] = json!(3);
        compose.as_object_mut().unwrap().remove("init_script");
        let response = self
            .request(
                Method::POST,
                "/cvms/provision",
                Some(json!({
                        "name": plan.name, "teepod_id": self.teepod_id, "image": self.image,
                "instance_type": plan.allowance.instance_type,
                        "prefer_dev": false, "vcpu": 1, "memory": 2048, "disk_size": 20,
                        "kms": "PHALA", "kms_id": self.kms_id, "listed": false,
                        "env_keys": ["COVENANT_COLLECTOR_TOKEN"], "compose_file": compose
                    })),
            )
            .await?
            .ok_or_else(|| failed("Phala provision endpoint unavailable"))?;
        let string = |key| {
            response
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| failed("Phala provision field missing"))
        };
        // Legacy provision uses Teepod.id, which is not Node.id. Never infer
        // their equivalence or accept the provider's automatic placement.
        let returned_kms = response.get("kms_id").and_then(Value::as_str);
        let info_kms = response
            .get("kms_info")
            .and_then(|v| v.get("id"))
            .and_then(Value::as_str);
        if response.get("teepod_id").and_then(Value::as_u64) != Some(self.teepod_id)
            || (returned_kms.is_none() && info_kms.is_none())
            || returned_kms.is_some_and(|id| id != self.kms_id)
            || info_kms.is_some_and(|id| id != self.kms_id)
        {
            return Err(failed("Phala provision selected another teepod or KMS"));
        }
        let provision = Provision {
            app_id: string("app_id")?.trim_start_matches("0x").into(),
            compose_hash: string("compose_hash")?,
            env_public_key: string("app_env_encrypt_pubkey")?
                .trim_start_matches("0x")
                .into(),
            os_image_hash: string("os_image_hash")?,
            instance_type: string("instance_type")?,
            hourly_micro_usd,
            teepod_id: self.teepod_id,
            kms_id: self.kms_id.clone(),
        };
        validate_provision(plan, &provision)?;
        Ok(provision)
    }
    async fn create(&self, plan: &LeasePlan, provision: &Provision) -> Result<Uuid, ExecutorError> {
        validate_provision(plan, provision)?;
        if provision.teepod_id != self.teepod_id || provision.kms_id != self.kms_id {
            return Err(failed(
                "Phala provider configuration changed after provision",
            ));
        }
        if self.checked_rate(plan).await? != provision.hourly_micro_usd {
            return Err(failed("Phala rate changed after provision; create refused"));
        }
        let now = now_ms();
        if now < plan.issued_at_ms
            || now >= plan.spec.lease_expires_at_ms
            || now >= plan.allowance.valid_until_ms
        {
            return Err(failed(
                "Phala creation authorization expired during price verification",
            ));
        }
        let encrypted = encrypt_token(&plan.collector_token, &provision.env_public_key)?;
        let value = self
            .request(
                Method::POST,
                "/cvms",
                Some(json!({
                    "app_id": provision.app_id, "compose_hash": provision.compose_hash,
                    "encrypted_env": encrypted, "env_keys": ["COVENANT_COLLECTOR_TOKEN"],
                    "kms_id": self.kms_id,
                })),
            )
            .await?
            .ok_or_else(|| failed("Phala create endpoint unavailable"))?;
        if value
            .get("app_id")
            .and_then(Value::as_str)
            .map(|v| v.trim_start_matches("0x"))
            != Some(&provision.app_id)
            || value.get("name").and_then(Value::as_str) != Some(&plan.name)
        {
            return Err(failed(
                "Phala create response identity mismatch; reconcile original app",
            ));
        }
        value
            .get("vm_uuid")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            .filter(|id| !id.is_nil())
            .ok_or_else(|| failed("Phala create returned no VM UUID"))
    }
    async fn find(&self, app_id: &str) -> Result<Option<Machine>, ExecutorError> {
        if !lower_hex(app_id, 40) {
            return Err(failed("invalid app reference"));
        }
        // Official SDK CvmIdSchema canonicalizes app IDs to this exact prefix.
        self.machine(&format!("app_{app_id}")).await
    }
    async fn get(&self, vm_uuid: Uuid) -> Result<Option<Machine>, ExecutorError> {
        let machine = self.machine(&vm_uuid.simple().to_string()).await?;
        if machine.as_ref().is_some_and(|m| m.vm_uuid != vm_uuid) {
            return Err(failed("Phala immutable VM reference changed"));
        }
        Ok(machine)
    }
    async fn identity(
        &self,
        plan: &LeasePlan,
        machine: &Machine,
    ) -> Result<SshAccess, ExecutorError> {
        let instance = machine
            .instance_id
            .as_deref()
            .filter(|s| lower_hex(s, 40))
            .ok_or_else(|| failed("Phala instance not ready"))?;
        let url = format!("https://{instance}-18090.{PHALA_GATEWAY}/identity");
        let mut response = self
            .client
            .get(url)
            .bearer_auth(&plan.collector_token)
            .header("Cache-Control", "no-cache")
            .send()
            .await
            .map_err(|_| failed("collector unavailable"))?;
        if response.status() != StatusCode::OK {
            return Err(failed("collector not ready"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| failed("collector response incomplete"))?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE {
                return Err(failed("oversized collector response"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| failed("invalid collector identity"))?;
        let identity = &value["identity"];
        let info = &identity["guest_info"];
        if value["verification_status"] != "unverified"
            || info["app_id"] != machine.app_id
            || info["instance_id"] != instance
            || info["compose_hash"] != plan.spec.workload_sha256
            || info["os_image_hash"] != OS_IMAGE_SHA256
            || identity["transport"] != json!({"kind":"tls"})
        {
            return Err(failed("collector guest identity differs from this lease"));
        }
        let key: SshHostKey = serde_json::from_value(identity["guest_channel_key"].clone())
            .map_err(|_| failed("invalid collector host key"))?;
        Ok(SshAccess {
            host: identity["host"]
                .as_str()
                .ok_or_else(|| failed("collector host missing"))?
                .into(),
            port: identity["port"]
                .as_u64()
                .and_then(|v| u16::try_from(v).ok())
                .ok_or_else(|| failed("collector port missing"))?,
            username: identity["username"]
                .as_str()
                .ok_or_else(|| failed("collector username missing"))?
                .into(),
            lease_generation: plan.spec.lease_generation.clone(),
            observed_host_keys: vec![key],
            transport: Some(SshTransport::Tls),
        })
    }
    async fn delete(&self, vm_uuid: Uuid) -> Result<(), ExecutorError> {
        self.request(Method::DELETE, &format!("/cvms/{}", vm_uuid.simple()), None)
            .await?;
        Ok(())
    }
}

fn decimal_micro_usd(value: &str) -> Result<u64, ExecutorError> {
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || whole.is_empty()
        || whole.len() > 20
        || fraction.len() > 18
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || fraction.bytes().skip(6).any(|b| b != b'0')
    {
        return Err(failed("Phala hourly rate is not an exact micro-USD amount"));
    }
    let whole: u64 = whole.parse().map_err(|_| failed("Phala price overflow"))?;
    let fraction = format!("{:0<6}", &fraction[..fraction.len().min(6)]);
    whole
        .checked_mul(1_000_000)
        .and_then(|v| v.checked_add(fraction.parse::<u64>().ok()?))
        .ok_or_else(|| failed("Phala price overflow"))
}

/// Official dstack encrypted-env-spec.md at 7f143148cc49f4a09799958835d310e050137cb3:
/// X25519 ephemeral public key || 12-byte IV || AES-256-GCM ciphertext and tag.
/// The KMS key comes through authenticated Cloud TLS; this is not a proof of
/// the KMS release policy and must not be described as one.
fn encrypt_token(token: &str, public_key: &str) -> Result<String, ExecutorError> {
    if !lower_hex(token, 64) || !lower_hex(public_key, 64) {
        return Err(failed("invalid environment encryption input"));
    }
    let remote: Vec<u8> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&public_key[i..i + 2], 16).unwrap())
        .collect();
    let rng = SystemRandom::new();
    let private = agreement::EphemeralPrivateKey::generate(&agreement::X25519, &rng)
        .map_err(|_| failed("generate environment encryption key"))?;
    let public = private
        .compute_public_key()
        .map_err(|_| failed("derive environment public key"))?;
    let peer = agreement::UnparsedPublicKey::new(&agreement::X25519, remote);
    let mut iv = [0u8; 12];
    rng.fill(&mut iv)
        .map_err(|_| failed("generate environment nonce"))?;
    let mut plaintext =
        serde_json::to_vec(&json!({"env":[{"key":"COVENANT_COLLECTOR_TOKEN","value":token}]}))
            .map_err(|_| failed("encode environment"))?;
    agreement::agree_ephemeral(private, &peer, |shared| {
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, shared)
            .map_err(|_| failed("derive encryption key"))?;
        aead::LessSafeKey::new(key)
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(iv),
                aead::Aad::empty(),
                &mut plaintext,
            )
            .map_err(|_| failed("encrypt environment"))
    })
    .map_err(|_| failed("invalid environment recipient key"))??;
    Ok(public
        .as_ref()
        .iter()
        .chain(iv.iter())
        .chain(plaintext.iter())
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn cloud(server: &MockServer) -> PhalaCloud {
        let mut cloud = PhalaCloud::new(
            "disposable-test-key".into(),
            17,
            "dstack-0.6.1".into(),
            "phala".into(),
        )
        .unwrap();
        cloud.base = server.uri();
        cloud
    }
    fn plan() -> LeasePlan {
        let id = Uuid::new_v4();
        let manifest =
            json!({"manifest_version":"3","init_script":[],"docker_compose_file":"test"})
                .to_string();
        LeasePlan {
            job_id: id,
            issued_at_ms: now_ms(),
            payload_sha256: "01".repeat(32),
            buyer_pubkey_b58: bs58::encode([2; 32]).into_string(),
            spec: PhalaLeaseSpec {
                schema_version: 1,
                job_id: id,
                operator_pubkey_b58: bs58::encode([3; 32]).into_string(),
                lease_generation: format!("phala-{id}-1"),
                lease_expires_at_ms: now_ms() + 60_000,
                workload_sha256: hash(manifest.as_bytes()),
            },
            name: format!("covenant-{}", id.simple()),
            manifest,
            allowance: CreationAllowance {
                schema_version: 1,
                review_id: "http-test-review".into(),
                operator_pubkey_b58: bs58::encode([3; 32]).into_string(),
                instance_type: "tdx.test".into(),
                max_hourly_micro_usd: 10000,
                max_runtime_secs: 60,
                max_new_resources: 1,
                planning_reserve_micro_usd: 10000,
                valid_until_ms: now_ms() + 120_000,
            },
            collector_token: "09".repeat(32),
        }
    }
    fn provision(plan: &LeasePlan) -> Provision {
        Provision {
            app_id: "ab".repeat(20),
            compose_hash: plan.spec.workload_sha256.clone(),
            env_public_key: "01".repeat(32),
            os_image_hash: OS_IMAGE_SHA256.into(),
            instance_type: "tdx.test".into(),
            hourly_micro_usd: 9000,
            teepod_id: 17,
            kms_id: "phala".into(),
        }
    }
    async fn pricing(server: &MockServer, rate: &str) {
        Mock::given(method("GET"))
            .and(path("/instance-types/cpu"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"items":[{
                "id":"tdx.test", "vcpu":1, "memory_mb":2048, "requires_gpu":false,
                "default_disk_size_gb":20, "hourly_rate":rate
            }]})))
            .mount(server)
            .await;
    }
    #[test]
    fn encrypted_env_roundtrips_official_x25519_aes_gcm_wire_format() {
        let rng = SystemRandom::new();
        let receiver = agreement::EphemeralPrivateKey::generate(&agreement::X25519, &rng).unwrap();
        let public = receiver.compute_public_key().unwrap();
        let public_hex: String = public.as_ref().iter().map(|b| format!("{b:02x}")).collect();
        let token = "09".repeat(32);
        let encrypted = encrypt_token(&token, &public_hex).unwrap();
        assert!(!encrypted.contains(&token));
        let bytes: Vec<u8> = (0..encrypted.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&encrypted[i..i + 2], 16).unwrap())
            .collect();
        let peer = agreement::UnparsedPublicKey::new(&agreement::X25519, &bytes[..32]);
        let mut ciphertext = bytes[44..].to_vec();
        let decoded = agreement::agree_ephemeral(receiver, &peer, |shared| {
            let key = aead::UnboundKey::new(&aead::AES_256_GCM, shared).unwrap();
            let raw = aead::LessSafeKey::new(key)
                .open_in_place(
                    aead::Nonce::assume_unique_for_key(bytes[32..44].try_into().unwrap()),
                    aead::Aad::empty(),
                    &mut ciphertext,
                )
                .unwrap();
            serde_json::from_slice::<Value>(raw).unwrap()
        })
        .unwrap();
        assert_eq!(
            decoded,
            json!({"env":[{"key":"COVENANT_COLLECTOR_TOKEN","value":token}]})
        );
        assert!(
            encrypt_token(&token, &"00".repeat(32)).is_err(),
            "low order recipient must be refused"
        );
    }
    #[tokio::test]
    async fn provision_is_pinned_and_normalized_hash_mismatch_refuses() {
        let server = MockServer::start().await;
        let cloud = cloud(&server);
        let plan = plan();
        pricing(&server, "0.009000000").await;
        Mock::given(method("POST"))
            .and(path("/cvms/provision"))
            .and(header("X-Phala-Version", "2026-05-22"))
            .and(header("X-API-Key", "disposable-test-key"))
            .and(body_partial_json(
                json!({"name":plan.name,"teepod_id":17,"image":"dstack-0.6.1",
                "vcpu":1,"memory":2048,"listed":false,"prefer_dev":false,
                "compose_file":{"manifest_version":3}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "app_id":"ab".repeat(20), "compose_hash":"00".repeat(32),
                "app_env_encrypt_pubkey":"01".repeat(32), "os_image_hash":OS_IMAGE_SHA256
                ,"instance_type":"tdx.test"
                ,"teepod_id":17,"kms_id":"phala"
            })))
            .expect(1)
            .mount(&server)
            .await;
        assert!(cloud.provision(&plan).await.is_err());
        assert_eq!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|r| r.method == "POST")
                .count(),
            1
        );
    }
    #[tokio::test]
    async fn create_uses_bound_app_hash_and_ciphertext_never_plaintext_token() {
        let server = MockServer::start().await;
        let cloud = cloud(&server);
        let plan = plan();
        pricing(&server, "0.009").await;
        let provision = provision(&plan);
        let id = Uuid::new_v4();
        Mock::given(method("POST"))
            .and(path("/cvms"))
            .and(body_partial_json(json!({
                "app_id":provision.app_id,"compose_hash":provision.compose_hash,"kms_id":"phala",
                "env_keys":["COVENANT_COLLECTOR_TOKEN"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "app_id":provision.app_id,"name":plan.name,"vm_uuid":id
            })))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(cloud.create(&plan, &provision).await.unwrap(), id);
        let requests = server.received_requests().await.unwrap();
        let post = requests.iter().find(|r| r.method == "POST").unwrap();
        let request: Value = serde_json::from_slice(&post.body).unwrap();
        assert!(request["encrypted_env"]
            .as_str()
            .is_some_and(|s| s.len() > 88 && lower_hex(s, s.len())));
        assert!(!String::from_utf8_lossy(&post.body).contains(&plan.collector_token));
    }
    #[tokio::test]
    async fn immutable_uuid_get_checks_observed_compose_and_no_redirect_credentials() {
        let server = MockServer::start().await;
        let cloud = cloud(&server);
        let plan = plan();
        let id = Uuid::new_v4();
        Mock::given(method("GET"))
            .and(path(format!("/cvms/{}", id.simple())))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "app_id":"ab".repeat(20),"vm_uuid":id,"name":plan.name,
                "instance_id":"cd".repeat(20),"status":"running","in_progress":false
                ,"compose_hash":plan.spec.workload_sha256
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/cvms/{}/compose_file", id.simple())))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        assert_eq!(
            cloud.get(id).await.unwrap().unwrap().compose_hash,
            plan.spec.workload_sha256
        );
        assert!(server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|r| !r.url.path().ends_with("/compose_file")));
        let foreign = Uuid::new_v4();
        Mock::given(method("DELETE"))
            .and(path(format!("/cvms/{}", foreign.simple())))
            .respond_with(ResponseTemplate::new(307).insert_header(
                "Location",
                format!("{}/must-not-receive-credentials", server.uri()),
            ))
            .mount(&server)
            .await;
        assert!(cloud.delete(foreign).await.is_err());
        assert!(!server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.url.path() == "/must-not-receive-credentials"));
    }
    #[test]
    fn hourly_rates_use_exact_decimal_micros_without_rounding() {
        assert_eq!(decimal_micro_usd("0.009000000").unwrap(), 9000);
        assert_eq!(decimal_micro_usd("1.000001").unwrap(), 1_000_001);
        for bad in [
            "NaN",
            "-1",
            "1e-2",
            "0.0000001",
            ".5",
            "18446744073709551615.0",
        ] {
            assert!(decimal_micro_usd(bad).is_err(), "{bad}");
        }
    }
    #[tokio::test]
    async fn rate_above_allowance_refuses_before_provision_or_create_posts() {
        let server = MockServer::start().await;
        let cloud = cloud(&server);
        let plan = plan();
        pricing(&server, "0.011").await;
        assert!(cloud.provision(&plan).await.is_err());
        assert!(cloud.create(&plan, &provision(&plan)).await.is_err());
        assert!(server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|r| r.method == "GET"));
    }
    #[tokio::test]
    async fn provision_rejects_automatic_placement_or_a_different_kms() {
        for (teepod, kms) in [(18, "phala"), (17, "other-kms")] {
            let server = MockServer::start().await;
            let cloud = cloud(&server);
            let plan = plan();
            pricing(&server, "0.009").await;
            Mock::given(method("POST"))
                .and(path("/cvms/provision"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "app_id":"ab".repeat(20),"compose_hash":plan.spec.workload_sha256,
                    "app_env_encrypt_pubkey":"01".repeat(32),"os_image_hash":OS_IMAGE_SHA256,
                    "instance_type":"tdx.test","teepod_id":teepod,"kms_info":{"id":kms}
                })))
                .expect(1)
                .mount(&server)
                .await;
            let error = cloud.provision(&plan).await.err().unwrap().to_string();
            assert!(error.contains("another teepod or KMS"), "{error}");
            assert!(!server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .any(|r| r.method == "POST" && r.url.path() == "/cvms"));
        }
    }
}
