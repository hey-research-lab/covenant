use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use covenant_a2a::{A2ADuplicateSafety, A2AIdempotency};
use covenant_compute_protocol::phala::{
    phala_lease_input, phala_workload, reviewed_phala_wrapper, PHALA_CPU_MODEL,
};
use covenant_compute_protocol::ssh::SshHostKey;
use covenant_compute_protocol::{lease_input, CapabilityRequirement, JobKind, LeaseTerms};
use covenant_types::AgentId;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Closed(AtomicBool);
#[async_trait]
impl LeaseCloseSource for Closed {
    async fn is_closed(&self, _: Uuid) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
struct Evidence;
#[async_trait]
impl PhalaEvidence for Evidence {
    async fn after_access(
        &self,
        _: &JobEnvelopePayload,
        _: &LeaseAccess,
        _: &str,
    ) -> Result<(), ExecutorError> {
        Ok(())
    }
}
struct Provider {
    directory: PathBuf,
    provision_count: AtomicUsize,
    create_count: AtomicUsize,
    delete_count: AtomicUsize,
    create_response_lost: AtomicBool,
    machine: Mutex<Option<Machine>>,
    preserve_after_delete: AtomicBool,
    wrong_key: AtomicBool,
    create_hangs: AtomicBool,
    creating: tokio::sync::Notify,
}
impl Provider {
    fn new(path: &Path) -> Arc<Self> {
        Arc::new(Self {
            directory: path.into(),
            provision_count: AtomicUsize::new(0),
            create_count: AtomicUsize::new(0),
            delete_count: AtomicUsize::new(0),
            create_response_lost: AtomicBool::new(false),
            machine: Mutex::new(None),
            preserve_after_delete: AtomicBool::new(false),
            wrong_key: AtomicBool::new(false),
            create_hangs: AtomicBool::new(false),
            creating: tokio::sync::Notify::new(),
        })
    }
    fn durable(&self, id: Uuid) -> Row {
        let snapshot: Snapshot =
            serde_json::from_slice(&std::fs::read(self.directory.join("leases.json")).unwrap())
                .unwrap();
        snapshot.rows[&id].clone()
    }
}
#[async_trait]
impl PhalaProvider for Provider {
    async fn health(&self) -> Result<(), ExecutorError> {
        Ok(())
    }
    async fn provision(&self, plan: &LeasePlan) -> Result<Provision, ExecutorError> {
        assert_eq!(self.durable(plan.job_id).phase, Phase::ProvisionIntent);
        self.provision_count.fetch_add(1, Ordering::SeqCst);
        Ok(Provision {
            app_id: "ab".repeat(20),
            compose_hash: plan.spec.workload_sha256.clone(),
            env_public_key: "01".repeat(32),
            os_image_hash: OS_IMAGE_SHA256.into(),
            instance_type: plan.allowance.instance_type.clone(),
            hourly_micro_usd: 9000,
            teepod_id: 17,
            kms_id: "phala".into(),
        })
    }
    async fn create(&self, plan: &LeasePlan, provision: &Provision) -> Result<Uuid, ExecutorError> {
        let row = self.durable(plan.job_id);
        assert_eq!(row.phase, Phase::CreateIntent);
        assert_eq!(row.plan.payload_sha256, plan.payload_sha256);
        self.create_count.fetch_add(1, Ordering::SeqCst);
        let id = Uuid::new_v4();
        *self.machine.lock() = Some(Machine {
            vm_uuid: id,
            app_id: provision.app_id.clone(),
            name: plan.name.clone(),
            compose_hash: plan.spec.workload_sha256.clone(),
            instance_id: Some("cd".repeat(20)),
            running: true,
        });
        self.creating.notify_one();
        if self.create_hangs.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if self.create_response_lost.load(Ordering::SeqCst) {
            Err(failed("lost create response"))
        } else {
            Ok(id)
        }
    }
    async fn find(&self, _: &str) -> Result<Option<Machine>, ExecutorError> {
        Ok(self.machine.lock().clone())
    }
    async fn get(&self, _: Uuid) -> Result<Option<Machine>, ExecutorError> {
        Ok(self.machine.lock().clone())
    }
    async fn identity(
        &self,
        plan: &LeasePlan,
        machine: &Machine,
    ) -> Result<SshAccess, ExecutorError> {
        let mut wire = b"\0\0\0\x0bssh-ed25519\0\0\0\x20".to_vec();
        wire.extend_from_slice(
            &[if self.wrong_key.load(Ordering::SeqCst) {
                9
            } else {
                8
            }; 32],
        );
        Ok(SshAccess {
            host: format!(
                "{}-1022.{PHALA_GATEWAY}",
                machine.instance_id.as_ref().unwrap()
            ),
            port: 443,
            username: "covenant".into(),
            lease_generation: plan.spec.lease_generation.clone(),
            observed_host_keys: vec![SshHostKey {
                algorithm: "ssh-ed25519".into(),
                key_base64: STANDARD.encode(wire),
            }],
            transport: Some(SshTransport::Tls),
        })
    }
    async fn delete(&self, _: Uuid) -> Result<(), ExecutorError> {
        let snapshot: Snapshot =
            serde_json::from_slice(&std::fs::read(self.directory.join("leases.json")).unwrap())
                .unwrap();
        let row = snapshot.rows.values().next().unwrap();
        assert_eq!(row.phase, Phase::DeletePending);
        assert!(row.close_requested);
        self.delete_count.fetch_add(1, Ordering::SeqCst);
        if !self.preserve_after_delete.load(Ordering::SeqCst) {
            *self.machine.lock() = None;
        }
        Ok(())
    }
}

fn operator() -> String {
    bs58::encode([3; 32]).into_string()
}
fn config(enabled: bool) -> PhalaConfig {
    PhalaConfig {
        operator_pubkey_b58: operator(),
        wrapper_template: reviewed_phala_wrapper(),
        image_digest: format!("sha256:{}", "12".repeat(32)),
        allow_managed_kms_creation: enabled,
        creation_allowance: Some(CreationAllowance {
            schema_version: 1,
            review_id: "disposable-test-review".into(),
            operator_pubkey_b58: operator(),
            instance_type: "tdx.test".into(),
            max_hourly_micro_usd: 10000,
            max_runtime_secs: 60,
            max_new_resources: 1,
            planning_reserve_micro_usd: 10000,
            valid_until_ms: now_ms() + 120_000,
        }),
        ready_timeout: Duration::from_millis(50),
        poll_interval: Duration::from_millis(1),
    }
}
fn job() -> JobEnvelopePayload {
    let mut wire = b"\0\0\0\x0bssh-ed25519\0\0\0\x20".to_vec();
    wire.extend_from_slice(&[7; 32]);
    let mut job = JobEnvelopePayload {
        job_id: Uuid::new_v4(),
        buyer: AgentId::new("buyer@local", [2; 32]),
        kind: JobKind::LeaseSession,
        capability_requirement: CapabilityRequirement {
            inference_tariff: None,
            gpu_class: Some("cpu".into()),
            min_vram_gb: None,
            model_id: Some(PHALA_CPU_MODEL.into()),
            kind: JobKind::LeaseSession,
            max_duration_secs: 60,
            min_reputation_bps: None,
        },
        input: vec![lease_input(LeaseTerms {
            max_duration_secs: 60,
            rate_micro_usdc_per_sec: 10,
            client_public_key: Some(format!("ssh-ed25519 {}", STANDARD.encode(wire))),
        })
        .unwrap()],
        price_micro_usdc: 600,
        deadline_ms: 120_000,
        idempotency: A2AIdempotency::new(A2ADuplicateSafety::Idempotent, "phala-test"),
        issued_at_ms: now_ms(),
        referral_code: None,
        stream: true,
    };
    let c = config(true);
    let (_, spec) =
        phala_workload(&job, &operator(), &c.wrapper_template, &c.image_digest).unwrap();
    job.input.push(phala_lease_input(spec).unwrap());
    job
}
fn backend(provider: Arc<Provider>, path: &Path, enabled: bool) -> Arc<PhalaSessionBackend> {
    PhalaSessionBackend::open_journal(
        provider,
        config(enabled),
        path,
        Arc::new(Closed(AtomicBool::new(false))),
        Arc::new(Evidence),
    )
    .unwrap()
}

#[tokio::test]
async fn signed_lease_creates_once_and_delete_requires_observed_absence() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    let backend = backend(provider.clone(), directory.path(), true);
    let job = job();
    assert!(backend.shutdown_status().is_resolved());
    let first = backend.open(&job).await.unwrap();
    assert_eq!(backend.shutdown_status().pending_leases, 1);
    assert!(!backend.shutdown_status().is_resolved());
    assert_eq!(
        first.ssh.as_ref().unwrap().transport,
        Some(SshTransport::Tls)
    );
    assert_eq!(backend.open(&job).await.unwrap().ssh, first.ssh);
    assert_eq!(provider.create_count.load(Ordering::SeqCst), 1);
    provider.preserve_after_delete.store(true, Ordering::SeqCst);
    backend.close(job.job_id).await;
    assert_eq!(provider.durable(job.job_id).phase, Phase::DeletePending);
    assert_eq!(backend.shutdown_status().pending_deletes, 1);
    assert!(!backend.shutdown_status().is_resolved());
    assert!(backend.admit(&super::tests::job()).is_err());
    provider
        .preserve_after_delete
        .store(false, Ordering::SeqCst);
    backend.reconcile().await.unwrap();
    assert_eq!(provider.durable(job.job_id).phase, Phase::Deleted);
    assert!(backend.shutdown_status().is_resolved());
    assert_eq!(provider.delete_count.load(Ordering::SeqCst), 2);
    assert!(backend.open(&job).await.is_err());
    assert!(
        backend.open(&super::tests::job()).await.is_err(),
        "confirmed deletion must not silently refund the planning reservation"
    );
}

#[tokio::test]
async fn lost_create_response_restarts_by_exact_app_without_second_create() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    provider.create_response_lost.store(true, Ordering::SeqCst);
    let job = job();
    {
        let backend = backend(provider.clone(), directory.path(), true);
        assert!(backend.open(&job).await.is_err());
        assert_eq!(provider.durable(job.job_id).phase, Phase::CreateIntent);
    }
    let restarted = backend(provider.clone(), directory.path(), false);
    let access = restarted.open(&job).await.unwrap();
    assert_eq!(access.job_id, job.job_id);
    assert_eq!(provider.create_count.load(Ordering::SeqCst), 1);
    restarted.close(job.job_id).await;
    assert_eq!(provider.durable(job.job_id).phase, Phase::Deleted);
}

#[tokio::test]
async fn unknown_create_absence_never_releases_capacity_or_recreates() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    provider.create_response_lost.store(true, Ordering::SeqCst);
    let original = job();
    {
        let backend = backend(provider.clone(), directory.path(), true);
        assert!(backend.open(&original).await.is_err());
    }
    *provider.machine.lock() = None;
    let backend = backend(provider.clone(), directory.path(), true);
    assert!(backend.open(&original).await.is_err());
    assert!(backend.reconcile().await.is_err());
    assert!(backend.open(&job()).await.is_err());
    assert_eq!(backend.shutdown_status().unknown_creates, 1);
    assert!(!backend.shutdown_status().is_resolved());
    assert_eq!(provider.create_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_count.load(Ordering::SeqCst), 0);
    assert!(provider.durable(original.job_id).close_requested);
}

#[tokio::test]
async fn foreign_resource_is_neither_adopted_nor_deleted_after_unknown_create() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    provider.create_response_lost.store(true, Ordering::SeqCst);
    let job = job();
    {
        let backend = backend(provider.clone(), directory.path(), true);
        assert!(backend.open(&job).await.is_err());
    }
    provider.machine.lock().as_mut().unwrap().name = "existing-diagnostic-cvm".into();
    let backend = backend(provider.clone(), directory.path(), true);
    assert!(backend.open(&job).await.is_err());
    assert!(backend.reconcile().await.is_err());
    assert_eq!(provider.delete_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn owned_uuid_cleanup_survives_workload_and_name_drift() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    let backend = backend(provider.clone(), directory.path(), true);
    let job = job();
    backend.open(&job).await.unwrap();
    {
        let mut state = provider.machine.lock();
        let machine = state.as_mut().unwrap();
        machine.compose_hash = String::new();
        machine.name = "changed-by-control-plane".into();
    }
    backend.close(job.job_id).await;
    assert_eq!(provider.durable(job.job_id).phase, Phase::Deleted);
    assert_eq!(provider.delete_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn disabled_creation_still_retries_owned_cleanup_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    let job = job();
    {
        let backend = backend(provider.clone(), directory.path(), true);
        backend.open(&job).await.unwrap();
        provider.preserve_after_delete.store(true, Ordering::SeqCst);
        backend.close(job.job_id).await;
    }
    provider
        .preserve_after_delete
        .store(false, Ordering::SeqCst);
    let backend = backend(provider.clone(), directory.path(), false);
    backend.reconcile().await.unwrap();
    assert_eq!(provider.durable(job.job_id).phase, Phase::Deleted);
    assert!(backend.open(&super::tests::job()).await.is_err());
    assert_eq!(provider.create_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn changed_payload_or_key_and_unsigned_leases_fail_closed() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    let backend = backend(provider.clone(), directory.path(), true);
    let mut unsigned = job();
    unsigned.input.pop();
    assert!(backend.open(&unsigned).await.is_err());
    assert_eq!(provider.provision_count.load(Ordering::SeqCst), 0);
    let original = job();
    backend.open(&original).await.unwrap();
    let mut changed = original.clone();
    changed.price_micro_usdc += 1;
    assert!(backend.open(&changed).await.is_err());
    provider.wrong_key.store(true, Ordering::SeqCst);
    assert!(backend.open(&original).await.is_err());
    assert_eq!(provider.create_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_during_create_reconciles_and_deletes_without_second_create() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    provider.create_hangs.store(true, Ordering::SeqCst);
    let backend = backend(provider.clone(), directory.path(), true);
    let executor = PhalaExecutor::new(backend.clone(), Arc::new(Closed(AtomicBool::new(false))));
    let job = job();
    let id = job.job_id;
    let (sink, _receiver) = tokio::sync::mpsc::channel(2);
    let task = tokio::spawn(async move {
        executor
            .execute_streaming(&job, Duration::from_secs(60), sink)
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), provider.creating.notified())
        .await
        .unwrap();
    task.abort();
    let _ = task.await;
    backend.reconcile().await.unwrap();
    assert_eq!(provider.durable(id).phase, Phase::Deleted);
    assert_eq!(provider.create_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn journal_lock_and_invalid_history_stop_before_provider_access() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    let first = backend(provider.clone(), directory.path(), true);
    assert!(PhalaSessionBackend::open_journal(
        provider.clone(),
        config(true),
        directory.path(),
        Arc::new(Closed(AtomicBool::new(false))),
        Arc::new(Evidence)
    )
    .is_err());
    drop(first);
    std::fs::write(directory.path().join("leases.json"), b"not-json").unwrap();
    assert!(PhalaSessionBackend::open_journal(
        provider.clone(),
        config(true),
        directory.path(),
        Arc::new(Closed(AtomicBool::new(false))),
        Arc::new(Evidence)
    )
    .is_err());
    assert_eq!(provider.provision_count.load(Ordering::SeqCst), 0);
}

#[test]
fn io_failure_poison_prevents_later_mutations() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    let backend = backend(provider, directory.path(), true);
    let mut plan = backend.plan(&job()).unwrap();
    plan.collector_token = "01".repeat(32);
    std::fs::create_dir(directory.path().join("leases.json")).unwrap();
    let row = Row {
        plan,
        phase: Phase::ProvisionIntent,
        provision: None,
        vm_uuid: None,
        access: None,
        close_requested: false,
        last_observed_ms: now_ms(),
        delete_attempts: 0,
    };
    assert!(backend.save(row.clone()).is_err());
    std::fs::remove_dir(directory.path().join("leases.json")).unwrap();
    assert!(backend.save(row).is_err());
    assert!(backend.admit(&job()).is_err());
    assert!(backend.shutdown_status().journal_poisoned);
    assert!(!backend.shutdown_status().is_resolved());
}

#[tokio::test]
async fn expired_signed_lease_refuses_without_provider_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    let backend = backend(provider.clone(), directory.path(), true);
    let mut job = job();
    job.input.pop();
    job.issued_at_ms = now_ms() - 61_000;
    let c = config(true);
    let (_, spec) =
        phala_workload(&job, &operator(), &c.wrapper_template, &c.image_digest).unwrap();
    job.input.push(phala_lease_input(spec).unwrap());
    assert!(backend.open(&job).await.is_err());
    assert_eq!(provider.provision_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn future_signed_lease_and_bool_without_allowance_refuse_before_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::new(directory.path());
    let backend = backend(provider.clone(), directory.path(), true);
    let mut future = job();
    future.input.pop();
    future.issued_at_ms = now_ms() + 60_000;
    let c = config(true);
    let (_, spec) =
        phala_workload(&future, &operator(), &c.wrapper_template, &c.image_digest).unwrap();
    future.input.push(phala_lease_input(spec).unwrap());
    assert!(backend.open(&future).await.is_err());
    drop(backend);
    let mut c = config(true);
    c.creation_allowance = None;
    let backend = PhalaSessionBackend::open_journal(
        provider.clone(),
        c,
        directory.path(),
        Arc::new(Closed(AtomicBool::new(false))),
        Arc::new(Evidence),
    )
    .unwrap();
    assert!(backend.open(&job()).await.is_err());
    assert_eq!(provider.provision_count.load(Ordering::SeqCst), 0);
}

#[test]
fn allowance_requires_one_resource_and_covers_reviewed_runtime() {
    let mut a = config(true).creation_allowance.unwrap();
    a.max_new_resources = 2;
    assert!(a.validate(&operator()).is_err());
    a.max_new_resources = 1;
    a.planning_reserve_micro_usd = 1;
    assert!(a.validate(&operator()).is_err());
    a.planning_reserve_micro_usd = 10000;
    a.max_runtime_secs = 3601;
    assert!(a.validate(&operator()).is_err());
}
