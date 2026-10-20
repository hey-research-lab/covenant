use super::*;
use base64::Engine as _;
use covenant_compute_attestation::binding::GuestEvidence;
use covenant_compute_protocol::{
    CapabilityRequirement, EscrowHoldAttestation, FundingSource, JobEnvelopePayload, LeaseTerms,
    SignedJobEnvelope,
};
use covenant_identity::LocalIdentity;
fn fixture() -> (
    Arc<CoordinatorState>,
    LocalIdentity,
    LocalIdentity,
    Uuid,
    u64,
) {
    let buyer = LocalIdentity::generate("buyer@hardware");
    let operator = LocalIdentity::generate("operator@hardware");
    let now = epoch_ms();
    let id = Uuid::new_v4();
    let state = Arc::new(CoordinatorState::new(
        LocalIdentity::generate("coordinator@hardware"),
        crate::CoordinatorConfig::default(),
        Arc::new(crate::reputation::NoReputation),
        Arc::new(crate::MockPayout::new()),
        Arc::new(covenant_audit::InMemoryAuditLog::new()),
    ));
    let envelope = SignedJobEnvelope::sign(
        JobEnvelopePayload {
            job_id: id,
            buyer: buyer.agent_id(),
            kind: JobKind::LeaseSession,
            capability_requirement: CapabilityRequirement {
                inference_tariff: None,
                gpu_class: None,
                min_vram_gb: None,
                model_id: None,
                kind: JobKind::LeaseSession,
                max_duration_secs: 600,
                min_reputation_bps: None,
            },
            input: vec![covenant_compute_protocol::lease_input(LeaseTerms {
                max_duration_secs: 600,
                rate_micro_usdc_per_sec: 1,
                client_public_key: None,
            })
            .unwrap()],
            price_micro_usdc: 600,
            deadline_ms: 900_000,
            idempotency: covenant_a2a::A2AIdempotency::new(
                covenant_a2a::A2ADuplicateSafety::Idempotent,
                "hardware-fixture",
            ),
            issued_at_ms: now,
            referral_code: None,
            stream: false,
        },
        &buyer,
    )
    .unwrap();
    let hold =
        EscrowHoldAttestation::sign(id, 600, FundingSource::Organic, now, &operator).unwrap();
    let mut key = vec![0, 0, 0, 11];
    key.extend_from_slice(b"ssh-ed25519");
    key.extend_from_slice(&[0, 0, 0, 32]);
    key.extend_from_slice(&[7; 32]);
    let record=serde_json::from_value(serde_json::json!({"operator_pubkey_b58":operator.agent_id().pubkey_base58(),"envelope":envelope,"escrow_hold":hold,"phase":"accepted","receipt":null,"output":null,"fee_micro_usdc":0,"accepted_at_ms":now,"lease_access":{"job_id":id,"endpoint":"ssh://root@gpu.example:2222","ready_at_ms":now,"ssh":{"host":"gpu.example","port":2222,"username":"root","lease_generation":"vm-1","observed_host_keys":[{"algorithm":"ssh-ed25519","key_base64":base64::engine::general_purpose::STANDARD.encode(key)}]}}})).unwrap();
    state.jobs().insert(id, record).unwrap();
    (state, buyer, operator, id, now)
}
fn bad_evidence(challenge: HardwareChallenge) -> HardwareEvidence {
    HardwareEvidence {
        challenge,
        guest: GuestEvidence::SevSnp {
            report_base64: "AA==".into(),
            certificate_chain_base64: Vec::new(),
        },
        gpu_cc: None,
    }
}
#[tokio::test]
async fn evidence_is_one_shot_durable_and_never_releases_a_stronger_floor() {
    let (state, buyer, operator, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("hardware.json");
    let pin = "ab".repeat(32);
    let mut service = HardwareService::open(state.clone(), &path, pin.clone()).unwrap();
    service.clock = Arc::new(move || now + 2);
    let owner = buyer.agent_id().pubkey_base58();
    let op = operator.agent_id().pubkey_base58();
    assert!(service.challenge(id, "stranger", 0, now).is_err());
    let challenge = service.challenge(id, &owner, 0, now).unwrap();
    assert_eq!(challenge, service.challenge(id, &op, 0, now + 1).unwrap());
    assert!(service
        .submit(id, &owner, bad_evidence(challenge.clone()), now + 1)
        .await
        .is_err());
    assert!(!service.book.lock().data.rows[&id].consumed);
    assert!(service
        .submit(id, &op, bad_evidence(challenge.clone()), now + 2)
        .await
        .is_err());
    assert!(service.book.lock().data.rows[&id].consumed);
    assert!(service
        .require_floor(id, &owner, TrustClass::Attested, now + 3)
        .is_err());
    drop(service);
    let mut service = HardwareService::open(state.clone(), &path, pin.clone()).unwrap();
    service.clock = Arc::new(move || now + 3);
    assert!(service
        .submit(id, &op, bad_evidence(challenge), now + 3)
        .await
        .unwrap_err()
        .to_string()
        .contains("consumed"));
    assert!(HardwareService::open(state.clone(), &path, pin.clone()).is_err());
    drop(service);
    assert!(HardwareService::open(state.clone(), &path, "cd".repeat(32)).is_err());
    std::fs::write(&path, b"{broken").unwrap();
    assert!(HardwareService::open(state, &path, pin).is_err());
}
#[test]
fn challenges_bind_the_current_lease_generation_endpoint_key_and_paid_window() {
    let (state, buyer, _, id, now) = fixture();
    let dir = tempfile::tempdir().unwrap();
    let service =
        HardwareService::open(state.clone(), &dir.path().join("book"), "ab".repeat(32)).unwrap();
    let owner = buyer.agent_id().pubkey_base58();
    let challenge = service.challenge(id, &owner, 0, now).unwrap();
    service.match_live_scope(&challenge.scope, now).unwrap();
    let mut changed = challenge.scope.clone();
    changed.lease_generation = "vm-2".into();
    assert!(service.match_live_scope(&changed, now).is_err());
    changed = challenge.scope.clone();
    changed.host = "other.example".into();
    assert!(service.match_live_scope(&changed, now).is_err());
    assert!(service.challenge(id, &owner, 0, now - 1).is_err());
    assert!(service.challenge(id, &owner, 1, now).is_err());
    assert!(service.challenge(id, &owner, 0, now + 600_000).is_err());
    state.jobs().request_lease_close(id, now + 1).unwrap();
    assert!(service.match_live_scope(&challenge.scope, now + 1).is_err());
}
#[test]
fn signed_http_authentication_cannot_be_replayed_for_a_different_route() {
    let actor = LocalIdentity::generate("actor@hardware");
    let now = epoch_ms();
    let path = format!("/federation/hardware/{}/challenge", Uuid::new_v4());
    let body = b"{}";
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-covenant-hardware-actor",
        actor.agent_id().pubkey_base58().parse().unwrap(),
    );
    headers.insert(
        "x-covenant-hardware-signed-at",
        now.to_string().parse().unwrap(),
    );
    headers.insert(
        "x-covenant-hardware-signature",
        binding::sign_request(&actor, "POST", &path, body, now)
            .unwrap()
            .parse()
            .unwrap(),
    );
    assert!(authenticate(&headers, "POST", &path, body, now).is_ok());
    assert!(authenticate(&headers, "POST", &path, b"{\"host_key_index\":1}", now).is_err());
    assert!(authenticate(&headers, "POST", &path, body, now + 30_001).is_err());
    assert!(authenticate(&headers, "GET", &path, body, now).is_err());
    assert!(authenticate(&headers, "POST", &(path + "/other"), body, now).is_err());
}

#[tokio::test]
async fn overload_does_not_consume_or_queue_a_challenge() {
    let (state, buyer, operator, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let mut service =
        HardwareService::open(state, &directory.path().join("hardware"), "ab".repeat(32)).unwrap();
    service.clock = Arc::new(move || now);
    let challenge = service
        .challenge(id, &buyer.agent_id().pubkey_base58(), 0, now)
        .unwrap();
    let busy = service
        .verify_slots
        .clone()
        .acquire_many_owned(2)
        .await
        .unwrap();
    let error = service
        .submit(
            id,
            &operator.agent_id().pubkey_base58(),
            bad_evidence(challenge.clone()),
            now,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, HardwareError::Unavailable(_)));
    assert!(error.to_string().contains("capacity"));
    assert!(!service.book.lock().data.rows[&id].consumed);
    drop(busy);
    assert!(service
        .submit(
            id,
            &operator.agent_id().pubkey_base58(),
            bad_evidence(challenge),
            now,
        )
        .await
        .is_err());
    assert!(service.book.lock().data.rows[&id].consumed);
    assert_eq!(service.verify_slots.available_permits(), 2);
}

#[test]
fn verdict_reads_persist_clock_progress_and_refuse_rollback_after_restart() {
    let (state, buyer, _, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("hardware");
    let pin = "ab".repeat(32);
    let service = HardwareService::open(state.clone(), &path, pin.clone()).unwrap();
    let owner = buyer.agent_id().pubkey_base58();
    service.challenge(id, &owner, 0, now).unwrap();
    // Even a read with no verified evidence records the observed clock.
    assert!(service.evidence(id, &owner, now + 100).is_err());
    assert!(service
        .evidence(id, &owner, now + 99)
        .unwrap_err()
        .to_string()
        .contains("backwards"));
    drop(service);
    let service = HardwareService::open(state, &path, pin).unwrap();
    assert!(service
        .challenge(id, &owner, 0, now + 99)
        .unwrap_err()
        .to_string()
        .contains("backwards"));
}

#[test]
fn completed_verification_cannot_extend_collection_or_lease_time() {
    let (state, buyer, _, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let service = HardwareService::open(
        state.clone(),
        &directory.path().join("hardware"),
        "ab".repeat(32),
    )
    .unwrap();
    let challenge = service
        .challenge(id, &buyer.agent_id().pubkey_base58(), 0, now)
        .unwrap();
    // This synthetic verdict tests only the post-verification timing gate;
    // it is never treated as cryptographically verified evidence.
    let verdict = HardwareVerdict {
        challenge,
        evidence_sha256: "00".repeat(32),
        reference_sha256: binding::reference_digest(),
        guest_identity: "test-only".into(),
        gpu_identity: None,
        channel_key_fingerprint: "test-only".into(),
        evidence_class: TrustClass::Attested,
        served_class: TrustClass::Open,
        verifier_version: "test-only".into(),
        verified_at_ms: now + 1,
        expires_at_ms: now + binding::VERDICT_TTL_MS,
    };
    assert!(service.validate_completion(&verdict, now + 2).is_ok());
    assert!(service.validate_completion(&verdict, now).is_err());
    assert!(service
        .validate_completion(&verdict, now + binding::CHALLENGE_TTL_MS)
        .is_err());
    state.jobs().request_lease_close(id, now + 3).unwrap();
    assert!(service.validate_completion(&verdict, now + 4).is_err());
}

#[tokio::test]
async fn worker_rechecks_clock_and_challenge_before_vendor_verification() {
    let (state, buyer, operator, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let mut service =
        HardwareService::open(state, &directory.path().join("hardware"), "ab".repeat(32)).unwrap();
    let owner = buyer.agent_id().pubkey_base58();
    let actor = operator.agent_id().pubkey_base58();
    let challenge = service.challenge(id, &owner, 0, now).unwrap();
    service.clock = Arc::new(move || now + binding::CHALLENGE_TTL_MS);
    let error = service
        .submit(id, &actor, bad_evidence(challenge), now)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("stale or invalid challenge"));
    assert!(!service.book.lock().data.rows[&id].consumed);

    let next = now + binding::CHALLENGE_TTL_MS + 1;
    let challenge = service.challenge(id, &owner, 0, next).unwrap();
    service.clock = Arc::new(move || next - 1);
    let error = service
        .submit(id, &actor, bad_evidence(challenge), next)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("clock moved backwards"));
    assert!(!service.book.lock().data.rows[&id].consumed);
}

#[test]
fn all_stronger_workspace_floors_remain_unreleased() {
    let (state, buyer, _, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let service =
        HardwareService::open(state, &directory.path().join("hardware"), "ab".repeat(32)).unwrap();
    assert_eq!(binding::SERVED_TRUST_CEILING, TrustClass::Open);
    for floor in [
        TrustClass::Isolated,
        TrustClass::Attested,
        TrustClass::Confidential,
    ] {
        assert!(service
            .require_floor(id, &buyer.agent_id().pubkey_base58(), floor, now)
            .unwrap_err()
            .to_string()
            .contains("not released"));
    }
}

#[test]
fn challenges_refuse_legacy_or_mismatched_ssh_access_without_fallback() {
    let (state, buyer, _, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let service = HardwareService::open(
        state.clone(),
        &directory.path().join("hardware"),
        "ab".repeat(32),
    )
    .unwrap();
    let original = state.jobs().get(id).unwrap();
    let owner = buyer.agent_id().pubkey_base58();
    for mode in 0..4 {
        let mut changed = original.clone();
        let access = changed.lease_access.as_mut().unwrap();
        match mode {
            0 => access.ssh = None,
            1 => access.job_id = Uuid::new_v4(),
            2 => access.ready_at_ms = now + 1,
            3 => access.ssh.as_mut().unwrap().observed_host_keys[0].key_base64 = "AA==".into(),
            _ => unreachable!(),
        }
        state.jobs().insert(id, changed).unwrap();
        assert!(
            service.challenge(id, &owner, 0, now).is_err(),
            "mode {mode}"
        );
        assert!(service.book.lock().data.rows.is_empty());
    }
    state.jobs().insert(id, original).unwrap();
    assert!(service.challenge(id, &owner, 0, now).is_ok());
}

#[tokio::test]
async fn hardware_http_rejects_unauthenticated_requests_without_state_changes() {
    let (state, _, _, id, _) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let service = Arc::new(
        HardwareService::open(state, &directory.path().join("hardware"), "ab".repeat(32)).unwrap(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = router(service.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = reqwest::Client::new();
    for (method, route) in [
        (reqwest::Method::POST, "challenge"),
        (reqwest::Method::POST, "evidence"),
        (reqwest::Method::GET, "verdict"),
    ] {
        let response = client
            .request(method, format!("{base}/federation/hardware/{id}/{route}"))
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(service.book.lock().data.rows.is_empty());
    }
    assert_eq!(service.verify_slots.available_permits(), 2);
    server.abort();
}

// Deliberately invalid vendor evidence: these rows exercise admission and
// worker timing only, and must never pass the production verifier.
fn cache_unverified_fixture(
    service: &HardwareService,
    challenge: HardwareChallenge,
    now: u64,
    expires_at_ms: u64,
) -> HardwareEvidence {
    let evidence = bad_evidence(challenge.clone());
    let verdict = HardwareVerdict {
        challenge: challenge.clone(),
        evidence_sha256: "00".repeat(32),
        reference_sha256: binding::reference_digest(),
        guest_identity: "test-only".into(),
        gpu_identity: None,
        channel_key_fingerprint: "test-only".into(),
        evidence_class: TrustClass::Attested,
        served_class: TrustClass::Open,
        verifier_version: "test-only".into(),
        verified_at_ms: now,
        expires_at_ms,
    };
    let mut book = service.book.lock();
    let row = book.data.rows.get_mut(&challenge.scope.job_id).unwrap();
    row.consumed = true;
    row.evidence = Some(evidence.clone());
    row.verdict = Some(verdict);
    book.save().unwrap();
    evidence
}

#[tokio::test]
async fn saturated_reads_challenges_and_cached_submits_never_schedule_or_touch_journal() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (state, buyer, operator, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("hardware");
    let mut service = HardwareService::open(state, &path, "ab".repeat(32)).unwrap();
    let owner = buyer.agent_id().pubkey_base58();
    let actor = operator.agent_id().pubkey_base58();
    let challenge = service.challenge(id, &owner, 0, now).unwrap();
    let evidence = cache_unverified_fixture(&service, challenge, now, now + 1000);
    let worker_calls = Arc::new(AtomicUsize::new(0));
    let observed = worker_calls.clone();
    service.clock = Arc::new(move || {
        observed.fetch_add(1, Ordering::SeqCst);
        now + 1
    });
    let before = std::fs::read(&path).unwrap();
    let busy = service
        .verify_slots
        .clone()
        .acquire_many_owned(2)
        .await
        .unwrap();
    assert!(service
        .evidence_blocking(id, &owner, now + 1)
        .await
        .unwrap_err()
        .to_string()
        .contains("capacity"));
    assert!(service
        .challenge_blocking(id, &owner, 0, now + 1)
        .await
        .unwrap_err()
        .to_string()
        .contains("capacity"));
    assert!(service
        .submit(id, &actor, evidence, now + 1)
        .await
        .unwrap_err()
        .to_string()
        .contains("capacity"));
    assert_eq!(
        worker_calls.load(Ordering::SeqCst),
        0,
        "overload must fail before a blocking worker starts"
    );
    assert_eq!(service.book.lock().data.watermark_ms, now);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(busy);
    // A correctly signed stranger still cannot enqueue work for this lease.
    assert!(service
        .evidence_blocking(id, "stranger", now + 1)
        .await
        .unwrap_err()
        .to_string()
        .contains("lease operator or buyer"));
    assert_eq!(worker_calls.load(Ordering::SeqCst), 0);
    assert_eq!(service.verify_slots.available_permits(), 2);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[tokio::test]
async fn cached_reads_use_worker_time_instead_of_request_arrival_time() {
    let (state, buyer, _, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("hardware");
    let mut service = HardwareService::open(state, &path, "ab".repeat(32)).unwrap();
    let owner = buyer.agent_id().pubkey_base58();
    let challenge = service.challenge(id, &owner, 0, now).unwrap();
    cache_unverified_fixture(&service, challenge, now, now + 50);
    service.clock = Arc::new(move || now + 100);
    let error = service
        .evidence_blocking(id, &owner, now)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("verdict expired"),
        "must reject worker-time expiry before attempting the invalid vendor evidence: {error}"
    );
    assert_eq!(service.book.lock().data.watermark_ms, now + 100);
    assert_eq!(service.verify_slots.available_permits(), 2);
}

#[tokio::test]
async fn challenge_fresh_submit_and_cached_retry_run_on_blocking_workers() {
    let (state, buyer, operator, id, now) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let mut service =
        HardwareService::open(state, &directory.path().join("hardware"), "ab".repeat(32)).unwrap();
    let requester_thread = std::thread::current().id();
    let worker_threads = Arc::new(Mutex::new(Vec::new()));
    let observed = worker_threads.clone();
    service.clock = Arc::new(move || {
        observed.lock().push(std::thread::current().id());
        now
    });
    let owner = buyer.agent_id().pubkey_base58();
    let actor = operator.agent_id().pubkey_base58();
    let challenge = service
        .challenge_blocking(id, &owner, 0, now)
        .await
        .unwrap();
    assert!(service
        .submit(id, &actor, bad_evidence(challenge.clone()), now)
        .await
        .unwrap_err()
        .to_string()
        .contains("certificate chain exceeds limits"));
    let evidence = cache_unverified_fixture(&service, challenge, now, now + 1000);
    assert!(service
        .submit(id, &actor, evidence, now)
        .await
        .unwrap_err()
        .to_string()
        .contains("certificate chain exceeds limits"));
    assert!(service
        .evidence_blocking(id, &owner, now)
        .await
        .unwrap_err()
        .to_string()
        .contains("certificate chain exceeds limits"));
    let observed = worker_threads.lock();
    assert!(observed.len() >= 4);
    assert!(
        observed.iter().all(|thread| *thread != requester_thread),
        "hardware filesystem and crypto workflow must leave the async requester thread"
    );
    assert_eq!(service.verify_slots.available_permits(), 2);
}
