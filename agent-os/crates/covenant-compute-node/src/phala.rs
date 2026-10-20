//! Per-lease Phala CPU resources. The local journal is the mutation authority:
//! neither a provider listing nor a remembered diagnostic CVM grants ownership.
//!
//! The Cloud API and collector are control-plane observations, not hardware
//! attestation. This backend deliberately leaves the served trust tier Open.
//! Provider contract: Phala-Network/phala-cloud@b2599d6ce6a0d806015e407131d3adba2b1d0b02,
//! js/src/actions/cvms/{provision_cvm,commit_cvm_provision,get_cvm_info}.ts.

mod http;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use covenant_compute_protocol::phala::{
    parse_phala_ephemeral_lease_spec, parse_phala_lease_spec, render_phala_ephemeral_manifest,
    render_phala_manifest, PhalaEphemeralLeaseSpec, PhalaLeaseSpec, PHALA_GATEWAY,
};
use covenant_compute_protocol::ssh::{SshAccess, SshTransport};
use covenant_compute_protocol::{JobEnvelopePayload, LeaseAccess};
use parking_lot::Mutex;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::executor::{ChunkSink, ExecutionOutcome, ExecutorError, JobExecutor};
use crate::lease::{LeaseCloseSource, LeaseControl, LeaseExecutor, SessionBackend};
use crate::phala_evidence::PhalaEvidence;

pub use http::PhalaCloud;
pub const OS_IMAGE_SHA256: &str =
    "ec43cb8c5e04b6bb6c53b33b16076a01d83d01f7f4b703f2f78a0d947d6247e6";
const RECOVERY_INTERVAL: Duration = Duration::from_secs(5);
const MAX_JOURNAL_BYTES: u64 = 32 * 1024 * 1024;

fn failed(message: impl Into<String>) -> ExecutorError {
    ExecutorError::Failed(message.into())
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// New creation is off by default. Enabling managed KMS is an operator risk
/// acknowledgement, never a claim that the KMS release policy is attested.
pub struct PhalaConfig {
    pub operator_pubkey_b58: String,
    pub wrapper_template: Value,
    pub image_digest: String,
    pub allow_managed_kms_creation: bool,
    /// Allocation-only enablement; never authorizes metadata-only readiness.
    pub allow_ephemeral_creation: bool,
    pub creation_allowance: Option<CreationAllowance>,
    pub ready_timeout: Duration,
    pub poll_interval: Duration,
}

/// Single-resource planning authorization, not a provider billing cap.
/// A deletion outage can exceed this reserve. No automatic invoice refund.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationAllowance {
    pub schema_version: u8,
    pub review_id: String,
    pub operator_pubkey_b58: String,
    pub instance_type: String,
    pub max_hourly_micro_usd: u64,
    pub max_runtime_secs: u64,
    pub max_new_resources: u8,
    pub planning_reserve_micro_usd: u64,
    pub valid_until_ms: u64,
}
impl CreationAllowance {
    pub fn validate(&self, operator: &str) -> Result<(), ExecutorError> {
        let identifier = |value: &str| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        };
        let planned = self
            .max_hourly_micro_usd
            .checked_mul(self.max_runtime_secs)
            .and_then(|v| v.checked_add(3599))
            .map(|v| v / 3600)
            .ok_or_else(|| failed("Phala planning reserve overflow"))?;
        if self.schema_version != 1
            || !identifier(&self.review_id)
            || !identifier(&self.instance_type)
            || self.operator_pubkey_b58 != operator
            || self.max_hourly_micro_usd == 0
            || self.max_runtime_secs == 0
            || self.max_runtime_secs > 3600
            || self.max_new_resources != 1
            || self.planning_reserve_micro_usd < planned
            || self.valid_until_ms == 0
        {
            return Err(failed("invalid reviewed Phala creation allowance"));
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeasePlan {
    pub job_id: Uuid,
    pub issued_at_ms: u64,
    pub payload_sha256: String,
    pub buyer_pubkey_b58: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<PhalaLeaseSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral_spec: Option<PhalaEphemeralLeaseSpec>,
    pub name: String,
    pub manifest: String,
    pub allowance: CreationAllowance,
    // Owner-only journal; never include this plan in logs or receipts.
    pub collector_token: String,
}

impl LeasePlan {
    fn validate_spec(&self) -> Result<(), ExecutorError> {
        let (id, operator) = match (&self.spec, &self.ephemeral_spec) {
            (Some(spec), None) => {
                spec.validate().map_err(|error| failed(error.to_string()))?;
                if !lower_hex(&self.collector_token, 64) {
                    return Err(failed("managed Phala plan requires a collector secret"));
                }
                (spec.job_id, &spec.operator_pubkey_b58)
            }
            (None, Some(spec)) => {
                spec.validate().map_err(|error| failed(error.to_string()))?;
                if !self.collector_token.is_empty() {
                    return Err(failed("ephemeral Phala plan forbids collector secrets"));
                }
                (spec.job_id, &spec.operator_pubkey_b58)
            }
            _ => return Err(failed("Phala plan requires exactly one explicit profile")),
        };
        if id != self.job_id || operator != &self.allowance.operator_pubkey_b58 {
            return Err(failed("Phala plan profile identity mismatch"));
        }
        Ok(())
    }

    fn is_ephemeral(&self) -> bool {
        self.ephemeral_spec.is_some()
    }
    fn expires_at_ms(&self) -> u64 {
        self.ephemeral_spec
            .as_ref()
            .map(|spec| spec.lease_expires_at_ms)
            .or_else(|| self.spec.as_ref().map(|spec| spec.lease_expires_at_ms))
            .unwrap_or(0)
    }
    fn workload_sha256(&self) -> &str {
        self.ephemeral_spec
            .as_ref()
            .map(|spec| spec.workload_sha256.as_str())
            .or_else(|| self.spec.as_ref().map(|spec| spec.workload_sha256.as_str()))
            .unwrap_or("")
    }
    fn generation(&self) -> &str {
        self.ephemeral_spec
            .as_ref()
            .map(|spec| spec.lease_generation.as_str())
            .or_else(|| {
                self.spec
                    .as_ref()
                    .map(|spec| spec.lease_generation.as_str())
            })
            .unwrap_or("")
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provision {
    pub app_id: String,
    pub compose_hash: String,
    pub env_public_key: String,
    pub os_image_hash: String,
    pub instance_type: String,
    pub hourly_micro_usd: u64,
    pub teepod_id: u64,
    pub kms_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    // Owner-only provision recovery token; never log or include in receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provision_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Machine {
    /// Canonical provider UUID, not a name or a mutable alias.
    pub vm_uuid: Uuid,
    pub app_id: String,
    pub name: String,
    pub compose_hash: String,
    pub instance_id: Option<String>,
    pub running: bool,
}

/// Mutating methods are invoked only after the corresponding journal intent
/// has reached stable storage. `create` is invoked at most once per job.
#[async_trait]
pub trait PhalaProvider: Send + Sync {
    async fn health(&self) -> Result<(), ExecutorError>;
    async fn provision(&self, plan: &LeasePlan) -> Result<Provision, ExecutorError>;
    async fn create(&self, plan: &LeasePlan, provision: &Provision) -> Result<Uuid, ExecutorError>;
    async fn find(&self, app_id: &str) -> Result<Option<Machine>, ExecutorError>;
    async fn get(&self, vm_uuid: Uuid) -> Result<Option<Machine>, ExecutorError>;
    async fn identity(
        &self,
        plan: &LeasePlan,
        machine: &Machine,
    ) -> Result<SshAccess, ExecutorError>;
    /// Initiates deletion; only a subsequent GET 404 confirms completion.
    async fn delete(&self, vm_uuid: Uuid) -> Result<(), ExecutorError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    ProvisionIntent,
    Provisioned,
    CreateIntent,
    Created,
    Ready,
    DeletePending,
    Deleted,
    /// A provision request can allocate cached configuration, but cannot
    /// create a billed CVM. Never repeat it or progress after an unknown result.
    ProvisionAbandoned,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    plan: LeasePlan,
    phase: Phase,
    provision: Option<Provision>,
    vm_uuid: Option<Uuid>,
    access: Option<SshAccess>,
    close_requested: bool,
    last_observed_ms: u64,
    delete_attempts: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    observed_instance_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    version: u8,
    operator: String,
    rows: BTreeMap<Uuid, Row>,
}

struct Journal {
    path: PathBuf,
    _lock: File,
    snapshot: Snapshot,
    poisoned: bool,
}

impl Journal {
    fn open(directory: &Path, operator: &str) -> Result<Self, ExecutorError> {
        let parent = directory
            .parent()
            .ok_or_else(|| failed("Phala journal needs a parent directory"))?;
        if !parent.is_dir() {
            return Err(failed("Phala journal parent must already exist"));
        }
        match std::fs::create_dir(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(failed("create Phala journal directory")),
        }
        let meta = std::fs::symlink_metadata(directory)
            .map_err(|_| failed("inspect Phala journal directory"))?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(failed("Phala journal directory must not be a symlink"));
        }
        File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(|_| failed("sync Phala journal parent"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| failed("secure Phala journal directory"))?;
        }
        let lock_path = directory.join("owner.lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|_| failed("open Phala journal lock"))?;
        lock.try_lock()
            .map_err(|_| failed("Phala journal is already owned by another process"))?;
        let path = directory.join("leases.json");
        let snapshot = match File::open(&path) {
            Ok(file) => {
                if file
                    .metadata()
                    .map_err(|_| failed("read Phala journal"))?
                    .len()
                    > MAX_JOURNAL_BYTES
                {
                    return Err(failed("Phala journal exceeds bounded size"));
                }
                serde_json::from_reader(file).map_err(|_| failed("invalid Phala journal"))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Snapshot {
                version: 1,
                operator: operator.into(),
                rows: BTreeMap::new(),
            },
            Err(_) => return Err(failed("read Phala journal")),
        };
        let result = Self {
            path,
            _lock: lock,
            snapshot,
            poisoned: false,
        };
        result.validate(operator)?;
        Ok(result)
    }

    fn validate(&self, operator: &str) -> Result<(), ExecutorError> {
        if self.snapshot.version != 1 || self.snapshot.operator != operator {
            return Err(failed("Phala journal version or operator mismatch"));
        }
        let mut review_ids = std::collections::BTreeSet::new();
        for (id, row) in &self.snapshot.rows {
            row.plan.validate_spec()?;
            row.plan.allowance.validate(operator)?;
            if !review_ids.insert(&row.plan.allowance.review_id) {
                return Err(failed("Phala journal reuses a single-resource allowance"));
            }
            if *id != row.plan.job_id
                || hash(row.plan.manifest.as_bytes()) != row.plan.workload_sha256()
                || row.plan.name != format!("covenant-{}", id.simple())
                || !lower_hex(&row.plan.payload_sha256, 64)
                || row.plan.issued_at_ms >= row.plan.expires_at_ms()
                || row.plan.expires_at_ms() > row.plan.allowance.valid_until_ms
                || row
                    .plan
                    .expires_at_ms()
                    .saturating_sub(row.plan.issued_at_ms)
                    > row.plan.allowance.max_runtime_secs * 1000
                || row.vm_uuid.is_some_and(|v| v.is_nil())
                || (matches!(
                    row.phase,
                    Phase::Provisioned | Phase::CreateIntent | Phase::Created | Phase::Ready
                ) && row.provision.is_none())
                || (matches!(row.phase, Phase::Created | Phase::Ready) && row.vm_uuid.is_none())
                || (row.phase == Phase::Ready && row.access.is_none())
                || (row.plan.is_ephemeral() && (row.phase == Phase::Ready || row.access.is_some()))
                || row
                    .observed_instance_id
                    .as_ref()
                    .is_some_and(|id| !lower_hex(id, 40))
            {
                return Err(failed("Phala journal binding is invalid"));
            }
            if let Some(provision) = &row.provision {
                validate_provision(&row.plan, provision)?;
            }
            if let Some(access) = &row.access {
                access.validate().map_err(|e| failed(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Replace on disk before changing the live copy. A failed fsync or rename
    /// never grants authority for a provider mutation.
    fn save(&mut self, row: Row) -> Result<(), ExecutorError> {
        if self.poisoned {
            return Err(failed(
                "Phala journal write failed; restart and reconcile before further operations",
            ));
        }
        let result = self.save_inner(row);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn save_inner(&mut self, row: Row) -> Result<(), ExecutorError> {
        let mut next = self.snapshot.clone();
        next.rows.insert(row.plan.job_id, row);
        let data = serde_json::to_vec(&next).map_err(|_| failed("serialize Phala journal"))?;
        if data.len() as u64 > MAX_JOURNAL_BYTES {
            return Err(failed(
                "Phala journal full; preserve history before admitting more leases",
            ));
        }
        let directory = self.path.parent().expect("journal has parent");
        let mut temporary = tempfile::NamedTempFile::new_in(directory)
            .map_err(|_| failed("stage Phala journal"))?;
        temporary
            .write_all(&data)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| failed("sync Phala journal"))?;
        temporary
            .persist(&self.path)
            .map_err(|_| failed("replace Phala journal"))?;
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|_| failed("sync Phala journal directory"))?;
        self.snapshot = next;
        Ok(())
    }
}

fn validate_provision(plan: &LeasePlan, provision: &Provision) -> Result<(), ExecutorError> {
    plan.validate_spec()?;
    let invalid_profile = if plan.is_ephemeral() {
        !provision.env_public_key.is_empty()
            || !provision.kms_id.is_empty()
            || !provision
                .device_id
                .as_deref()
                .is_some_and(|id| lower_hex(id, 64))
    } else {
        !lower_hex(&provision.env_public_key, 64)
            || provision.kms_id.is_empty()
            || provision.device_id.is_some()
            || provision.provision_token.is_some()
    };
    if !lower_hex(&provision.app_id, 40)
        || invalid_profile
        || provision.provision_token.as_ref().is_some_and(|token| {
            token.is_empty() || token.len() > 65_536 || token.bytes().any(|b| b.is_ascii_control())
        })
        || provision.compose_hash != plan.workload_sha256()
        || provision.os_image_hash != OS_IMAGE_SHA256
        || provision.instance_type != plan.allowance.instance_type
        || provision.hourly_micro_usd == 0
        || provision.hourly_micro_usd > plan.allowance.max_hourly_micro_usd
        || provision.teepod_id == 0
    {
        return Err(failed(
            "provider provision differs from signed workload or qualified OS",
        ));
    }
    Ok(())
}

fn validate_machine(row: &Row, machine: &Machine) -> Result<(), ExecutorError> {
    validate_ownership(row, machine)?;
    if machine.name != row.plan.name || machine.compose_hash != row.plan.workload_sha256() {
        return Err(failed(
            "provider ready machine differs from signed workload",
        ));
    }
    Ok(())
}
fn validate_ownership(row: &Row, machine: &Machine) -> Result<(), ExecutorError> {
    let provision = row
        .provision
        .as_ref()
        .ok_or_else(|| failed("missing provision"))?;
    if machine.vm_uuid.is_nil()
        || machine.app_id != provision.app_id
        || (row.vm_uuid.is_none() && machine.name != row.plan.name)
        || row.vm_uuid.is_some_and(|id| id != machine.vm_uuid)
    {
        return Err(failed(
            "provider machine ownership or workload mismatch; no mutation permitted",
        ));
    }
    Ok(())
}

fn observe_ephemeral_instance(row: &mut Row, machine: &Machine) -> Result<(), ExecutorError> {
    if !row.plan.is_ephemeral() {
        return Ok(());
    }
    if let Some(instance) = &machine.instance_id {
        if !lower_hex(instance, 40)
            || row
                .observed_instance_id
                .as_ref()
                .is_some_and(|old| old != instance)
        {
            return Err(failed(
                "ephemeral provider instance changed; destroy without restart or replacement",
            ));
        }
        row.observed_instance_id = Some(instance.clone());
    } else if row.observed_instance_id.is_some() {
        return Err(failed(
            "ephemeral provider instance disappeared; destroy without replacement",
        ));
    }
    Ok(())
}

/// Allocation receipt only. This contains no provider or routing credential and
/// deliberately has no SSH endpoint, key, hardware verdict or readiness flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhalaAllocation {
    pub job_id: Uuid,
    pub vm_uuid: Uuid,
    pub app_id: String,
    pub workload_sha256: String,
    pub expires_at_ms: u64,
}

/// Local recovery obligations, without provider credentials or guest secrets.
/// A deleted row keeps its planning reservation but no longer needs cleanup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhalaShutdownStatus {
    pub pending_leases: usize,
    pub unknown_creates: usize,
    pub pending_deletes: usize,
    pub journal_poisoned: bool,
}

impl PhalaShutdownStatus {
    pub fn is_resolved(&self) -> bool {
        self.pending_leases == 0 && !self.journal_poisoned
    }
}

pub struct PhalaSessionBackend {
    provider: Arc<dyn PhalaProvider>,
    config: PhalaConfig,
    journal: Mutex<Journal>,
    operations: tokio::sync::Mutex<()>,
    close_requests: Mutex<Vec<Uuid>>,
    close_source: Arc<dyn LeaseCloseSource>,
    evidence: Arc<dyn PhalaEvidence>,
}

impl PhalaSessionBackend {
    /// Read-only snapshot: shutdown never starts a second cleanup operation
    /// alongside a serving future or the existing recovery task.
    pub fn shutdown_status(&self) -> PhalaShutdownStatus {
        let journal = self.journal.lock();
        let mut status = PhalaShutdownStatus {
            journal_poisoned: journal.poisoned,
            ..PhalaShutdownStatus::default()
        };
        for row in journal.snapshot.rows.values() {
            if !matches!(row.phase, Phase::Deleted | Phase::ProvisionAbandoned) {
                status.pending_leases += 1;
            }
            if row.phase == Phase::CreateIntent {
                status.unknown_creates += 1;
            }
            if row.phase == Phase::DeletePending {
                status.pending_deletes += 1;
            }
        }
        status
    }

    pub fn open_journal(
        provider: Arc<dyn PhalaProvider>,
        config: PhalaConfig,
        directory: &Path,
        close_source: Arc<dyn LeaseCloseSource>,
        evidence: Arc<dyn PhalaEvidence>,
    ) -> Result<Arc<Self>, ExecutorError> {
        covenant_compute_protocol::validate_address_b58(
            "Phala operator",
            &config.operator_pubkey_b58,
        )
        .map_err(|e| failed(e.to_string()))?;
        if config.ready_timeout.is_zero() || config.poll_interval.is_zero() {
            return Err(failed("Phala readiness intervals must be positive"));
        }
        if !config
            .image_digest
            .strip_prefix("sha256:")
            .is_some_and(|value| lower_hex(value, 64))
        {
            return Err(failed(
                "Phala guest image must be an immutable SHA256 digest",
            ));
        }
        if let Some(allowance) = &config.creation_allowance {
            allowance.validate(&config.operator_pubkey_b58)?;
        }
        let journal = Journal::open(directory, &config.operator_pubkey_b58)?;
        Ok(Arc::new(Self {
            provider,
            config,
            journal: Mutex::new(journal),
            operations: tokio::sync::Mutex::new(()),
            close_requests: Mutex::new(Vec::new()),
            close_source,
            evidence,
        }))
    }

    fn plan(&self, job: &JobEnvelopePayload) -> Result<LeasePlan, ExecutorError> {
        let spec = parse_phala_lease_spec(&job.input).map_err(|e| failed(e.to_string()))?;
        let ephemeral_spec =
            parse_phala_ephemeral_lease_spec(&job.input).map_err(|e| failed(e.to_string()))?;
        let (manifest, expires_at_ms) = match (&spec, &ephemeral_spec) {
            (Some(spec), None) => (
                render_phala_manifest(
                    job,
                    &self.config.operator_pubkey_b58,
                    spec,
                    &self.config.wrapper_template,
                    &self.config.image_digest,
                )
                .map_err(|e| failed(e.to_string()))?,
                spec.lease_expires_at_ms,
            ),
            (None, Some(spec)) => (
                render_phala_ephemeral_manifest(
                    job,
                    &self.config.operator_pubkey_b58,
                    spec,
                    &self.config.wrapper_template,
                    &self.config.image_digest,
                )
                .map_err(|e| failed(e.to_string()))?,
                spec.lease_expires_at_ms,
            ),
            _ => {
                return Err(failed(
                    "Phala requires exactly one buyer-signed profile and workload",
                ))
            }
        };
        let now = now_ms();
        if now < job.issued_at_ms || now >= expires_at_ms {
            return Err(failed("signed Phala lease is future-issued or expired"));
        }
        let payload =
            serde_json::to_vec(job).map_err(|_| failed("serialize signed lease payload"))?;
        let allowance = self
            .journal
            .lock()
            .snapshot
            .rows
            .get(&job.job_id)
            .map(|row| row.plan.allowance.clone())
            .or_else(|| self.config.creation_allowance.clone())
            .ok_or_else(|| {
                failed("Phala creation requires an explicit reviewed planning allowance")
            })?;
        allowance.validate(&self.config.operator_pubkey_b58)?;
        if expires_at_ms > allowance.valid_until_ms
            || expires_at_ms.saturating_sub(job.issued_at_ms) > allowance.max_runtime_secs * 1000
        {
            return Err(failed(
                "signed Phala lease exceeds the reviewed creation allowance",
            ));
        }
        Ok(LeasePlan {
            job_id: job.job_id,
            issued_at_ms: job.issued_at_ms,
            payload_sha256: hash(&payload),
            buyer_pubkey_b58: job.buyer.pubkey_base58(),
            name: format!("covenant-{}", job.job_id.simple()),
            manifest,
            allowance,
            spec,
            ephemeral_spec,
            collector_token: String::new(),
        })
    }

    fn creation_enabled(&self, plan: &LeasePlan) -> bool {
        if plan.is_ephemeral() {
            self.config.allow_ephemeral_creation
        } else {
            self.config.allow_managed_kms_creation
        }
    }

    fn row(&self, id: Uuid) -> Result<Row, ExecutorError> {
        self.journal
            .lock()
            .snapshot
            .rows
            .get(&id)
            .cloned()
            .ok_or_else(|| failed("missing Phala lease"))
    }
    fn save(&self, row: Row) -> Result<(), ExecutorError> {
        self.journal.lock().save(row)
    }
    fn request_close(&self, id: Uuid) {
        self.close_requests.lock().push(id);
    }
    fn admit(&self, job: &JobEnvelopePayload) -> Result<(), ExecutorError> {
        let plan = self.plan(job)?;
        if plan.is_ephemeral() && self.close_requests.lock().contains(&job.job_id) {
            return Err(failed(
                "ephemeral allocation cancellation is already pending",
            ));
        }
        let journal = self.journal.lock();
        if journal.poisoned {
            return Err(failed("Phala journal requires restart and reconciliation"));
        }
        if let Some(row) = journal.snapshot.rows.get(&job.job_id) {
            if row.plan.payload_sha256 != plan.payload_sha256 || row.close_requested {
                return Err(failed("Phala lease replay differs or is closing"));
            }
            return Ok(());
        }
        if !self.creation_enabled(&plan) {
            return Err(failed(
                "new Phala resources disabled for this explicit profile",
            ));
        }
        if journal
            .snapshot
            .rows
            .values()
            .any(|row| row.plan.allowance.review_id == plan.allowance.review_id)
        {
            return Err(failed("Phala creation allowance already reserved; deletion does not refund planned exposure"));
        }
        if journal
            .snapshot
            .rows
            .values()
            .any(|row| !matches!(row.phase, Phase::Deleted | Phase::ProvisionAbandoned))
        {
            return Err(failed(
                "Phala capacity reserved by a live or unresolved lease",
            ));
        }
        Ok(())
    }

    /// Start once before accepting jobs. This task never provisions or creates;
    /// it only reconciles journal-owned resources and retries their teardown.
    pub fn start_recovery(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                let Some(backend) = weak.upgrade() else {
                    return;
                };
                if let Err(error) = backend.reconcile().await {
                    tracing::warn!(%error, "Phala reconciliation incomplete; journal retained");
                }
                drop(backend);
                tokio::time::sleep(RECOVERY_INTERVAL).await;
            }
        });
    }

    pub async fn reconcile(&self) -> Result<(), ExecutorError> {
        let _operation = self.operations.lock().await;
        let requests = self.close_requests.lock().clone();
        // Do not remove a cancellation request until its durable write succeeds.
        for id in requests {
            if let Ok(mut row) = self.row(id) {
                row.close_requested = true;
                self.save(row)?;
            }
            self.close_requests.lock().retain(|queued| *queued != id);
        }
        let ids: Vec<_> = self.journal.lock().snapshot.rows.keys().copied().collect();
        let mut any_error = false;
        for id in ids {
            let mut row = self.row(id)?;
            if matches!(row.phase, Phase::Deleted | Phase::ProvisionAbandoned) {
                continue;
            }
            let now = now_ms();
            if now < row.last_observed_ms {
                return Err(failed("Phala journal clock moved backwards"));
            }
            row.last_observed_ms = now;
            if now >= row.plan.expires_at_ms() || self.close_source.is_closed(id).await {
                row.close_requested = true;
            }
            if row.plan.is_ephemeral()
                && !row.close_requested
                && matches!(row.phase, Phase::Created | Phase::Ready)
            {
                // The provider may omit instance_id for none-mode. If it does
                // expose one, a later change is terminal, never a new boot grant.
                match self
                    .provider
                    .get(row.vm_uuid.expect("created row has VM"))
                    .await
                {
                    Ok(Some(machine)) => {
                        if validate_machine(&row, &machine).is_err()
                            || observe_ephemeral_instance(&mut row, &machine).is_err()
                        {
                            row.close_requested = true;
                        }
                    }
                    Ok(None) => row.close_requested = true,
                    Err(_) => any_error = true,
                }
            }
            self.save(row.clone())?;
            if row.close_requested || row.phase == Phase::DeletePending {
                if let Err(error) = self.cleanup(row).await {
                    tracing::warn!(job_id=%id, %error, "Phala cleanup pending");
                    any_error = true;
                }
            }
        }
        if any_error {
            Err(failed("Phala cleanup remains unresolved"))
        } else {
            Ok(())
        }
    }

    async fn cleanup(&self, mut row: Row) -> Result<(), ExecutorError> {
        row.close_requested = true;
        if matches!(row.phase, Phase::ProvisionIntent | Phase::Provisioned) {
            row.phase = Phase::ProvisionAbandoned;
            return self.save(row);
        }
        // No provider ID after a lost create response: find only the newly
        // provisioned app. A 404 is not proof that the earlier create failed.
        let machine = if let Some(id) = row.vm_uuid {
            self.provider.get(id).await?
        } else {
            let provision = row
                .provision
                .as_ref()
                .ok_or_else(|| failed("missing app ownership"))?;
            self.provider.find(&provision.app_id).await?
        };
        let Some(machine) = machine else {
            if row.vm_uuid.is_some() {
                row.phase = Phase::Deleted;
                return self.save(row);
            }
            return Err(failed(
                "create outcome unknown; absent app is not authority to create again",
            ));
        };
        validate_ownership(&row, &machine)?;
        row.vm_uuid = Some(machine.vm_uuid);
        row.phase = Phase::DeletePending;
        row.delete_attempts = row
            .delete_attempts
            .checked_add(1)
            .ok_or_else(|| failed("Phala deletion attempt counter overflow"))?;
        self.save(row.clone())?;
        self.provider.delete(machine.vm_uuid).await?;
        if self.provider.get(machine.vm_uuid).await?.is_none() {
            row.phase = Phase::Deleted;
            self.save(row)?;
            Ok(())
        } else {
            Err(failed("provider deletion initiated but not confirmed"))
        }
    }

    // Caller holds operations. A persisted CreateIntent is reconciled, never retried.
    async fn allocate_inner(&self, job: &JobEnvelopePayload) -> Result<Row, ExecutorError> {
        self.admit(job)?;
        let mut row = match self.row(job.job_id) {
            Ok(row) => row,
            Err(_) => {
                let mut plan = self.plan(job)?;
                if !plan.is_ephemeral() {
                    let mut random = [0u8; 32];
                    SystemRandom::new()
                        .fill(&mut random)
                        .map_err(|_| failed("generate collector secret"))?;
                    plan.collector_token = random.iter().map(|b| format!("{b:02x}")).collect();
                }
                plan.validate_spec()?;
                let mut row = Row {
                    plan,
                    phase: Phase::ProvisionIntent,
                    provision: None,
                    vm_uuid: None,
                    access: None,
                    close_requested: false,
                    last_observed_ms: now_ms(),
                    delete_attempts: 0,
                    observed_instance_id: None,
                };
                self.save(row.clone())?;
                let provision = self.provider.provision(&row.plan).await?;
                validate_provision(&row.plan, &provision)?;
                row.provision = Some(provision);
                row.phase = Phase::Provisioned;
                self.save(row.clone())?;
                row
            }
        };
        if matches!(
            row.phase,
            Phase::ProvisionIntent
                | Phase::ProvisionAbandoned
                | Phase::Deleted
                | Phase::DeletePending
        ) {
            return Err(failed("Phala lease cannot restart this provider operation"));
        }
        if row.phase == Phase::Provisioned {
            if !self.creation_enabled(&row.plan)
                || self.config.creation_allowance.as_ref() != Some(&row.plan.allowance)
                || now_ms() < row.plan.issued_at_ms
                || now_ms() < row.last_observed_ms
                || now_ms() >= row.plan.expires_at_ms()
                || self.close_source.is_closed(job.job_id).await
            {
                return Err(failed("Phala create disabled, expired or buyer-closed"));
            }
            row.phase = Phase::CreateIntent;
            self.save(row.clone())?;
            let id = self
                .provider
                .create(&row.plan, row.provision.as_ref().unwrap())
                .await?;
            if id.is_nil() {
                return Err(failed("provider returned invalid machine identity"));
            }
            row.vm_uuid = Some(id);
            row.phase = Phase::Created;
            self.save(row.clone())?;
        }
        Ok(row)
    }

    /// Allocate only one new single-boot VM. Returned provider observations are
    /// not attestation or SSH access. A later verifier must bind an exact quote
    /// before any native ephemeral readiness API can be added.
    pub async fn allocate_ephemeral(
        &self,
        job: &JobEnvelopePayload,
    ) -> Result<PhalaAllocation, ExecutorError> {
        let _operation = self.operations.lock().await;
        let plan = self.plan(job)?;
        if !plan.is_ephemeral() {
            return Err(failed(
                "ephemeral allocation requires the explicit single-boot profile",
            ));
        }
        let mut cleanup = AllocationOnDrop {
            backend: self,
            job_id: job.job_id,
            armed: true,
        };
        let result = async {
            let mut row = self.allocate_inner(job).await?;
            let machine = if let Some(id) = row.vm_uuid {
                self.provider.get(id).await?
            } else {
                self.provider
                    .find(&row.provision.as_ref().unwrap().app_id)
                    .await?
            }
            .ok_or_else(|| {
                failed("ephemeral resource is not visible; reconcile without recreating")
            })?;
            validate_machine(&row, &machine)?;
            observe_ephemeral_instance(&mut row, &machine)?;
            let closed = self.close_source.is_closed(job.job_id).await;
            let observed_at_ms = now_ms();
            if observed_at_ms < row.last_observed_ms
                || observed_at_ms >= row.plan.expires_at_ms()
                || closed
            {
                return Err(failed("ephemeral allocation expired or was buyer-closed"));
            }
            row.vm_uuid = Some(machine.vm_uuid);
            row.phase = Phase::Created;
            row.last_observed_ms = observed_at_ms;
            self.save(row.clone())?;
            Ok(PhalaAllocation {
                job_id: job.job_id,
                vm_uuid: machine.vm_uuid,
                app_id: machine.app_id,
                workload_sha256: row.plan.workload_sha256().into(),
                expires_at_ms: row.plan.expires_at_ms(),
            })
        }
        .await;
        if result.is_ok() {
            cleanup.armed = false;
        }
        result
    }

    async fn open_inner(&self, job: &JobEnvelopePayload) -> Result<LeaseAccess, ExecutorError> {
        let _operation = self.operations.lock().await;
        if self.plan(job)?.is_ephemeral() {
            return Err(failed(
                "native ephemeral readiness requires independently verified relay attestation",
            ));
        }
        let mut row = self.allocate_inner(job).await?;
        let started = tokio::time::Instant::now();
        loop {
            let now = now_ms();
            if now < row.last_observed_ms
                || now >= row.plan.expires_at_ms()
                || started.elapsed() >= self.config.ready_timeout
                || self.close_source.is_closed(job.job_id).await
            {
                return Err(failed("Phala readiness expired or buyer closed the lease"));
            }
            let machine = if let Some(id) = row.vm_uuid {
                self.provider.get(id).await?
            } else {
                self.provider
                    .find(&row.provision.as_ref().unwrap().app_id)
                    .await?
            };
            if let Some(machine) = machine {
                validate_machine(&row, &machine)?;
                row.vm_uuid = Some(machine.vm_uuid);
                self.save(row.clone())?;
                if machine.running {
                    // Transient collector unavailability is expected at boot;
                    // identity mismatches never become an access grant.
                    if let Ok(access) = self.provider.identity(&row.plan, &machine).await {
                        validate_access(&row, &machine, &access)?;
                        if row.access.as_ref().is_some_and(|old| old != &access) {
                            return Err(failed(
                                "Phala SSH key or instance changed after readiness",
                            ));
                        }
                        if now_ms() >= row.plan.expires_at_ms() {
                            return Err(failed("Phala signed lease expired during readiness"));
                        }
                        row.phase = Phase::Ready;
                        row.access = Some(access.clone());
                        row.last_observed_ms = now_ms();
                        self.save(row)?;
                        return Ok(LeaseAccess {
                            job_id: job.job_id,
                            endpoint: format!(
                                "ssh+tls://{}@{}:{}",
                                access.username, access.host, access.port
                            ),
                            ssh: Some(access),
                            source_price: None,
                            ready_at_ms: now_ms(),
                            note: Some(
                                "Per-lease Phala CPU; observed SSH key; served trust remains Open"
                                    .into(),
                            ),
                        });
                    }
                }
            }
            tokio::time::sleep(self.config.poll_interval).await;
        }
    }
}

fn validate_access(row: &Row, machine: &Machine, access: &SshAccess) -> Result<(), ExecutorError> {
    let instance = machine
        .instance_id
        .as_deref()
        .filter(|id| lower_hex(id, 40))
        .ok_or_else(|| failed("provider instance identity not ready"))?;
    access.validate().map_err(|e| failed(e.to_string()))?;
    if access.host != format!("{instance}-1022.{PHALA_GATEWAY}")
        || access.port != 443
        || access.username != "covenant"
        || access.transport != Some(SshTransport::Tls)
        || access.lease_generation != row.plan.generation()
        || access.observed_host_keys.len() != 1
        || access.observed_host_keys[0].algorithm != "ssh-ed25519"
    {
        return Err(failed(
            "collector SSH identity does not bind this Phala lease",
        ));
    }
    Ok(())
}

#[async_trait]
impl SessionBackend for PhalaSessionBackend {
    async fn open(&self, job: &JobEnvelopePayload) -> Result<LeaseAccess, ExecutorError> {
        let result = self.open_inner(job).await;
        if result.is_err() {
            self.request_close(job.job_id);
        }
        result
    }
    async fn close(&self, id: Uuid) {
        self.request_close(id);
        if let Err(error) = self.reconcile().await {
            tracing::warn!(job_id=%id, %error, "Phala close remains in durable reconciliation");
        }
    }
    async fn after_access(
        &self,
        job: &JobEnvelopePayload,
        access: &LeaseAccess,
    ) -> Result<(), ExecutorError> {
        let row = self.row(job.job_id)?;
        if row.phase != Phase::Ready
            || row.close_requested
            || row.access.as_ref() != access.ssh.as_ref()
        {
            return Err(failed(
                "hardware evidence access differs from ready Phala lease",
            ));
        }
        self.evidence
            .after_access(job, access, &row.plan.collector_token)
            .await
    }
}

/// Keeps the ordinary lease transport untouched while adding Phala-specific
/// admission, cancellation cleanup and honest backend health.
pub struct PhalaExecutor {
    lease: LeaseExecutor,
    backend: Arc<PhalaSessionBackend>,
}
impl PhalaExecutor {
    pub fn new(backend: Arc<PhalaSessionBackend>, source: Arc<dyn LeaseCloseSource>) -> Self {
        backend.start_recovery();
        Self {
            lease: LeaseExecutor::new(backend.clone(), LeaseControl::new()).watching(source),
            backend,
        }
    }
}
struct AllocationOnDrop<'a> {
    backend: &'a PhalaSessionBackend,
    job_id: Uuid,
    armed: bool,
}
impl Drop for AllocationOnDrop<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.backend.request_close(self.job_id);
        }
    }
}

struct CloseOnDrop(Arc<PhalaSessionBackend>, Uuid);
impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        self.0.request_close(self.1);
    }
}
#[async_trait]
impl JobExecutor for PhalaExecutor {
    fn admit(&self, job: &JobEnvelopePayload, _remaining: Duration) -> Result<(), String> {
        if self
            .backend
            .plan(job)
            .map_err(|e| e.to_string())?
            .is_ephemeral()
        {
            return Err("ephemeral allocation is not yet a native verified lease executor".into());
        }
        self.backend.admit(job).map_err(|e| e.to_string())
    }
    async fn execute(
        &self,
        job: &JobEnvelopePayload,
        deadline: Duration,
    ) -> Result<ExecutionOutcome, ExecutorError> {
        self.lease.execute(job, deadline).await
    }
    async fn execute_streaming(
        &self,
        job: &JobEnvelopePayload,
        deadline: Duration,
        sink: ChunkSink,
    ) -> Result<ExecutionOutcome, ExecutorError> {
        let _cleanup = CloseOnDrop(self.backend.clone(), job.job_id);
        self.lease.execute_streaming(job, deadline, sink).await
    }
    async fn health(&self) -> Result<(), ExecutorError> {
        if self.backend.config.allow_ephemeral_creation
            || self.backend.config.wrapper_template["key_provider"] == "none"
        {
            return Err(failed(
                "ephemeral allocation is not yet a native verified lease executor",
            ));
        }
        if !self.backend.config.allow_managed_kms_creation {
            return Err(failed("Phala new leases disabled; recovery remains active"));
        }
        let Some(allowance) = &self.backend.config.creation_allowance else {
            return Err(failed("Phala creation allowance is not configured"));
        };
        {
            let journal = self.backend.journal.lock();
            if now_ms() >= allowance.valid_until_ms
                || journal.poisoned
                || journal
                    .snapshot
                    .rows
                    .values()
                    .any(|row| row.plan.allowance.review_id == allowance.review_id)
            {
                return Err(failed(
                    "Phala creation allowance expired, reserved or journal unhealthy",
                ));
            }
        }
        if self
            .backend
            .journal
            .lock()
            .snapshot
            .rows
            .values()
            .any(|row| !matches!(row.phase, Phase::Deleted | Phase::ProvisionAbandoned))
        {
            return Err(failed("Phala capacity occupied or unresolved"));
        }
        self.backend.provider.health().await
    }
}
