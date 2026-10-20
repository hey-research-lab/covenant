//! Opt-in acceptance against a real Docker daemon and a deliberately faulty
//! pinned fixture image. The successful compiler/receipt flow lives in buyer.
use super::*;
use covenant_compute_protocol::knowledge::{
    knowledge_digest, knowledge_input, KnowledgeCompileRequest,
};
use covenant_identity::LocalIdentity;
use serde_json::{json, Value};
use std::sync::Arc;

async fn docker(runtime: &str, args: &[&str]) -> std::process::Output {
    tokio::time::timeout(
        Duration::from_secs(15),
        Command::new(runtime)
            .args(args)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("Docker command timeout")
    .expect("Docker command failed to start")
}

async fn inspect(runtime: &str, name: &str) -> Option<Value> {
    let output = docker(runtime, &["inspect", name]).await;
    if output.status.success() {
        Some(
            serde_json::from_slice::<Vec<Value>>(&output.stdout)
                .unwrap()
                .remove(0),
        )
    } else {
        None
    }
}

async fn wait_running(runtime: &str, name: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(value) = inspect(runtime, name).await {
                if value["State"]["Running"] == true {
                    return value;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("fixture container did not start")
}

async fn wait_removed(runtime: &str, name: &str) {
    tokio::time::timeout(Duration::from_secs(12), async {
        while inspect(runtime, name).await.is_some() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("executor left a container behind");
}

fn job(config: &KnowledgeConfig, content: &str) -> JobEnvelopePayload {
    let commit = "a".repeat(40);
    let source = json!({
        "files": [{"path":"src/lib.rs", "content":content}],
        "git": {"revision":commit, "dirty":false},
        "diagnostics":[], "config":{}
    });
    let request = KnowledgeCompileRequest {
        schema_version: 1,
        repository: "https://github.com/example/public".into(),
        commit,
        compiler_image_sha256: config.image_sha256.clone(),
        source_sha256: knowledge_digest(&source).unwrap(),
        source,
    };
    covenant_compute_buyer::sign_envelope(
        &covenant_compute_buyer::BuyerConfig {
            coordinator_url: "http://127.0.0.1:1".into(),
            poll_interval: Duration::from_millis(10),
            referral_code: None,
            rpc_url: None,
        },
        &LocalIdentity::generate("docker-fixture-buyer@local"),
        covenant_compute_buyer::JobRequest {
            kind: covenant_compute_protocol::JobKind::BatchJob,
            input: knowledge_input(&request).unwrap(),
            model: Some(request.model().unwrap()),
            gpu_class: None,
            min_vram_gb: None,
            min_reputation_bps: None,
            price_micro_usdc: 10,
            deadline_ms: 60_000,
        },
    )
    .unwrap()
    .payload
}

#[tokio::test]
#[ignore = "requires real Docker and pinned deploy/knowledge-compiler lifecycle fixture image"]
async fn real_docker_isolation_timeout_output_cancel_and_scoped_orphans() {
    let runtime =
        std::env::var("COVENANT_KNOWLEDGE_TEST_DOCKER_RUNTIME").unwrap_or_else(|_| "docker".into());
    let image = std::env::var("COVENANT_KNOWLEDGE_TEST_FAULT_IMAGE")
        .expect("set a pinned lifecycle fixture image; never use an arbitrary production image");
    let config = KnowledgeConfig::new(runtime.clone(), image.clone()).unwrap();
    let compiler = Arc::new(KnowledgeExecutor::new(config.clone()));
    compiler.health().await.unwrap();
    assert_eq!(compiler.reap_orphans().await.unwrap(), 0);

    let waiting_job = job(&config, "WAIT");
    let waiting_name = format!(
        "covenant-knowledge-{}-{}",
        compiler.instance_tag, waiting_job.job_id
    );
    let task_compiler = compiler.clone();
    let timeout_task = tokio::spawn(async move {
        task_compiler
            .execute(&waiting_job, Duration::from_secs(12))
            .await
    });
    let inspection = wait_running(&runtime, &waiting_name).await;
    assert_eq!(inspection["HostConfig"]["NetworkMode"], "none");
    assert_eq!(inspection["HostConfig"]["ReadonlyRootfs"], true);
    assert_eq!(inspection["HostConfig"]["Memory"], 512 * 1024 * 1024);
    assert_eq!(inspection["HostConfig"]["NanoCpus"], 1_000_000_000u64);
    assert_eq!(inspection["HostConfig"]["PidsLimit"], 64);
    assert_eq!(inspection["HostConfig"]["CapDrop"], json!(["ALL"]));
    assert!(inspection["HostConfig"]["SecurityOpt"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str().unwrap().starts_with("no-new-privileges")));
    assert_eq!(inspection["Config"]["User"], "65532:65532");
    assert_eq!(inspection["Mounts"], json!([]));
    assert_eq!(inspection["Config"]["Image"], image);
    let probe = docker(
        &runtime,
        &[
            "exec",
            &waiting_name,
            "/bin/sh",
            "-c",
            "test \"$(id -u)\" = 65532 && ! touch /knowledge-write-probe && test \"$(wc -l < /proc/net/route)\" -eq 1 && test \"$(awk '/^CapEff:/{print $2}' /proc/self/status)\" = 0000000000000000 && test \"$(awk '/^NoNewPrivs:/{print $2}' /proc/self/status)\" = 1",
        ],
    )
    .await;
    assert!(
        probe.status.success(),
        "runtime isolation probe: {}",
        String::from_utf8_lossy(&probe.stderr)
    );
    assert!(matches!(
        timeout_task.await.unwrap(),
        Err(ExecutorError::Timeout(_))
    ));
    wait_removed(&runtime, &waiting_name).await;

    let overflow_job = job(&config, "OVERFLOW");
    let overflow_name = format!(
        "covenant-knowledge-{}-{}",
        compiler.instance_tag, overflow_job.job_id
    );
    let overflow = tokio::time::timeout(
        Duration::from_secs(12),
        compiler.execute(&overflow_job, Duration::from_secs(30)),
    )
    .await
    .expect("output ceiling must terminate before the job deadline")
    .unwrap_err();
    assert!(overflow.to_string().contains("output exceeded"));
    wait_removed(&runtime, &overflow_name).await;

    let cancelled_job = job(&config, "WAIT");
    let cancelled_name = format!(
        "covenant-knowledge-{}-{}",
        compiler.instance_tag, cancelled_job.job_id
    );
    let task_compiler = compiler.clone();
    let cancelled_task = tokio::spawn(async move {
        task_compiler
            .execute(&cancelled_job, Duration::from_secs(60))
            .await
    });
    wait_running(&runtime, &cancelled_name).await;
    cancelled_task.abort();
    let _ = cancelled_task.await;
    wait_removed(&runtime, &cancelled_name).await;

    let own_name = format!("knowledge-test-orphan-{}", uuid::Uuid::new_v4());
    let other_name = format!("knowledge-test-other-{}", uuid::Uuid::new_v4());
    for (name, tag) in [
        (&own_name, compiler.instance_tag.as_str()),
        (&other_name, "other-operator"),
    ] {
        let label = format!("covenant.knowledge.operator={tag}");
        let output = docker(
            &runtime,
            &[
                "run",
                "-d",
                "--rm",
                "--pull",
                "never",
                "--name",
                name,
                "--network",
                "none",
                "--label",
                &label,
                "--entrypoint",
                "/bin/sleep",
                &image,
                "60",
            ],
        )
        .await;
        assert!(output.status.success(), "start test orphan");
    }
    assert_eq!(compiler.reap_orphans().await.unwrap(), 1);
    assert!(inspect(&runtime, &own_name).await.is_none());
    assert!(inspect(&runtime, &other_name).await.is_some());
    assert!(docker(&runtime, &["rm", "-f", &other_name])
        .await
        .status
        .success());
    assert_eq!(compiler.reap_orphans().await.unwrap(), 0);

    if let Ok(path) = std::env::var("COVENANT_KNOWLEDGE_DOCKER_LIFECYCLE_EVIDENCE_PATH") {
        use std::io::Write;
        assert!(std::path::Path::new(&path).is_absolute());
        let evidence = json!({
            "schema":"covenant.knowledge.real-docker-lifecycle.v1",
            "local_test_only":true,
            "funded_acceptance":false,
            "fixture_image":image,
            "inspection":inspection,
            "runtime_probe_stdout":String::from_utf8_lossy(&probe.stdout),
            "runtime_probe_stderr":String::from_utf8_lossy(&probe.stderr),
            "passed":["nonroot","read_only_rootfs","no_network_routes","no_mounts",
                "no_capabilities","no_new_privileges","memory_cpu_pid_limits",
                "timeout_cleanup","output_ceiling_cleanup","cancellation_cleanup",
                "operator_scoped_orphan_cleanup"]
        });
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(&serde_json::to_vec_pretty(&evidence).unwrap())
            .unwrap();
    }
}
