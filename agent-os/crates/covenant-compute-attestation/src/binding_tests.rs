use super::*;
use p384::pkcs8::DecodePrivateKey;
fn key() -> SshHostKey {
    let mut bytes = vec![];
    bytes.extend_from_slice(&11u32.to_be_bytes());
    bytes.extend_from_slice(b"ssh-ed25519");
    bytes.extend_from_slice(&32u32.to_be_bytes());
    bytes.extend_from_slice(&[0x9e; 32]);
    SshHostKey {
        algorithm: "ssh-ed25519".into(),
        key_base64: STANDARD.encode(bytes),
    }
}
fn challenge() -> HardwareChallenge {
    let now = 1_780_315_200_000;
    HardwareChallenge::new(
        HardwareScope {
            job_id: Uuid::new_v4(),
            operator_pubkey_b58: LocalIdentity::generate("operator@local")
                .agent_id()
                .pubkey_base58(),
            buyer_pubkey_b58: LocalIdentity::generate("buyer@local")
                .agent_id()
                .pubkey_base58(),
            lease_generation: "provider-instance-1".into(),
            host: "127.0.0.1".into(),
            port: 22,
            username: "worker".into(),
            guest_channel_key: key(),
            workload_sha256: hex::encode([0x3c; 32]),
            lease_expires_at_ms: now + 600_000,
        },
        now,
    )
    .unwrap()
}
fn evidence(challenge: HardwareChallenge) -> HardwareEvidence {
    let refs: serde_json::Value =
        serde_json::from_str(include_str!("../reference/snp-launch-measurements.json")).unwrap();
    let measurement: [u8; 48] = hex::decode(refs["measurements"][0]["sha384"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let platform: serde_json::Value =
        serde_json::from_str(include_str!("../reference/snp-platform.json")).unwrap();
    let tcb = &platform["tcb_floor"];
    let signing = p384::ecdsa::SigningKey::from_pkcs8_der(include_bytes!(
        "../tests/fixtures/snp/vcek-key.pkcs8.der"
    ))
    .unwrap();
    let report = crate::SnpReportBuilder::genoa(
        challenge.guest_report_data().unwrap(),
        measurement,
        [0x5a; 64],
    )
    .host_data([0x3c; 32])
    .tcb(
        tcb["bootloader"].as_u64().unwrap() as u8,
        tcb["tee"].as_u64().unwrap() as u8,
        tcb["snp"].as_u64().unwrap() as u8,
        tcb["microcode"].as_u64().unwrap() as u8,
    )
    .signed_with(&signing);
    HardwareEvidence {
        challenge,
        guest: GuestEvidence::SevSnp {
            report_base64: STANDARD.encode(report),
            certificate_chain_base64: vec![
                STANDARD.encode(include_bytes!("../tests/fixtures/snp/vcek.der")),
                STANDARD.encode(include_bytes!("../tests/fixtures/snp/ask.der")),
                STANDARD.encode(include_bytes!("../tests/fixtures/snp/test-ark.der")),
            ],
        },
        gpu_cc: None,
    }
}
#[test]
fn saved_verdict_survives_challenge_expiry_but_not_vendor_certificate_expiry() {
    let policy = Policy::for_tests();
    let c = challenge();
    let e = evidence(c.clone());
    let verdict = verify_with_policy(&e, c.issued_at_ms, &policy).unwrap();
    assert!(verify_saved_with_policy(&e, &verdict, &c.scope, c.expires_at_ms + 1, &policy).is_ok());

    // The fixture VCEK expires at 2036-01-01, inside this verdict's TTL.
    // An originally valid signature cannot extend a vendor validity window.
    let expiry = DateTime::parse_from_rfc3339("2036-01-01T00:00:00Z")
        .unwrap()
        .timestamp_millis() as u64;
    let mut scope = c.scope;
    scope.lease_expires_at_ms = expiry + 600_000;
    let c = HardwareChallenge::new(scope, expiry - 60_000).unwrap();
    let e = evidence(c.clone());
    let verdict = verify_with_policy(&e, c.issued_at_ms, &policy).unwrap();
    assert!(matches!(
        verify_saved_with_policy(&e, &verdict, &c.scope, expiry + 1_000, &policy),
        Err(BindingError::Verification(
            VerificationError::SnpCertificateExpired
        ))
    ));
}
#[test]
fn full_uuid_endpoint_key_workload_and_nonce_are_hardware_bound() {
    let c = challenge();
    let e = evidence(c.clone());
    let policy = Policy::for_tests();
    let v = verify_with_policy(&e, c.issued_at_ms, &policy).unwrap();
    assert_eq!(v.evidence_class, TrustClass::Attested);
    assert_eq!(v.served_class, TrustClass::Open);
    assert!(
        v.require_floor(&c.scope, TrustClass::Attested, c.issued_at_ms)
            .is_err()
    );
    let mut mutations = vec![];
    let mut m = c.clone();
    m.scope.job_id = Uuid::new_v4();
    mutations.push(m);
    let mut m = c.clone();
    m.scope.lease_generation.push('x');
    mutations.push(m);
    let mut m = c.clone();
    m.scope.host = "another.example".into();
    mutations.push(m);
    let mut m = c.clone();
    m.nonce_hex = hex::encode([7; 32]);
    mutations.push(m);
    let mut m = c.clone();
    m.scope.guest_channel_key.key_base64 = key().key_base64.replace("np6e", "p6en");
    mutations.push(m);
    let mut m = c.clone();
    m.scope.workload_sha256 = hex::encode([8; 32]);
    mutations.push(m);
    for modified in mutations {
        let mut wrong = e.clone();
        wrong.challenge = modified;
        assert!(verify_with_policy(&wrong, c.issued_at_ms, &policy).is_err());
    }
    assert!(verify_with_policy(&e, c.expires_at_ms, &policy).is_err());
}
#[test]
fn synthetic_roots_never_verify_in_production_or_after_signature_tampering() {
    let c = challenge();
    let mut e = evidence(c.clone());
    assert!(verify_evidence(&e, c.issued_at_ms).is_err());
    if let GuestEvidence::SevSnp { report_base64, .. } = &mut e.guest {
        let mut bytes = STANDARD.decode(&*report_base64).unwrap();
        bytes[200] ^= 1;
        *report_base64 = STANDARD.encode(bytes);
    }
    assert!(verify_with_policy(&e, c.issued_at_ms, &Policy::for_tests()).is_err());
}
#[test]
fn request_signature_binds_method_path_body_and_fresh_time() {
    let identity = LocalIdentity::generate("operator@local");
    let key = identity.agent_id().pubkey_base58();
    let signed = sign_request(&identity, "POST", "/hardware/job", b"evidence", 1000).unwrap();
    assert!(
        verify_request(
            &key,
            &signed,
            "POST",
            "/hardware/job",
            b"evidence",
            1000,
            2000
        )
        .is_ok()
    );
    for (method, path, body) in [
        ("GET", "/hardware/job", b"evidence".as_slice()),
        ("POST", "/hardware/other", b"evidence".as_slice()),
        ("POST", "/hardware/job", b"changed".as_slice()),
    ] {
        assert!(verify_request(&key, &signed, method, path, body, 1000, 2000).is_err());
    }
    assert!(
        verify_request(
            &key,
            &signed,
            "POST",
            "/hardware/job",
            b"evidence",
            1000,
            31001
        )
        .is_err()
    );
}

#[test]
fn a_gpu_report_cannot_be_grafted_onto_an_independent_guest_report() {
    let c = challenge();
    let mut e = evidence(c.clone());
    let gpu = GpuEvidence {
        report_base64: STANDARD.encode([7u8; 16]),
        certificate_chain_base64: vec![STANDARD.encode([8u8; 16])],
    };
    let combined = c.guest_report_data_for_gpu(Some(&gpu)).unwrap();
    assert_ne!(combined, c.guest_report_data().unwrap());
    let mut changed = gpu.clone();
    changed.report_base64 = STANDARD.encode([9u8; 16]);
    assert_ne!(
        combined,
        c.guest_report_data_for_gpu(Some(&changed)).unwrap()
    );
    e.gpu_cc = Some(gpu);
    let error = verify_with_policy(&e, c.issued_at_ms, &Policy::for_tests()).unwrap_err();
    assert!(matches!(
        error,
        BindingError::Verification(VerificationError::SnpReportDataMismatch)
    ));
}
