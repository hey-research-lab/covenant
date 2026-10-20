//! Dedicated compiler executor. Source is data on stdin, never a command,
//! filesystem mount, network destination or Docker argument.
use std::process::Stdio;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use covenant_compute_protocol::knowledge::{
    knowledge_model, knowledge_output, validate_knowledge_job, KnowledgeCompileResult,
};
use covenant_compute_protocol::JobEnvelopePayload;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::executor::{drain_stderr_tail, read_stdout_capped};
use crate::{ExecutionOutcome, ExecutorError, JobExecutor};

#[derive(Debug, Clone)]
pub struct KnowledgeConfig {
    pub runtime: String,
    pub image: String,
    pub image_sha256: String,
}

impl KnowledgeConfig {
    pub fn new(runtime: String, image: String) -> anyhow::Result<Self> {
        let (name, digest) = image.rsplit_once("@sha256:").ok_or_else(|| {
            anyhow::anyhow!("knowledge compiler image must be pinned as name@sha256:<digest>")
        })?;
        anyhow::ensure!(
            !name.is_empty() && !name.starts_with('-') && !name.chars().any(char::is_whitespace),
            "invalid compiler image name"
        );
        knowledge_model(digest)?;
        anyhow::ensure!(!runtime.is_empty(), "container runtime is required");
        Ok(Self {
            runtime,
            image_sha256: digest.into(),
            image,
        })
    }
    pub fn from_environment() -> anyhow::Result<Self> {
        Self::new(
            std::env::var("COVENANT_COMPUTE_NODE_CONTAINER_RUNTIME")
                .unwrap_or_else(|_| "docker".into()),
            std::env::var("COVENANT_COMPUTE_KNOWLEDGE_IMAGE").map_err(|_| {
                anyhow::anyhow!(
                    "COVENANT_COMPUTE_KNOWLEDGE_IMAGE must pin the compiler image digest"
                )
            })?,
        )
    }
    pub fn model(&self) -> String {
        format!("knowledge-compile-v1:{}", self.image_sha256)
    }
}

pub struct KnowledgeExecutor {
    config: KnowledgeConfig,
    instance_tag: String,
}

// Dropping a Docker CLI only detaches it. Cancellation must also remove the
// named container; a process crash is covered by the scoped boot sweep.
struct ContainerCleanup {
    runtime: String,
    name: String,
    armed: bool,
}
impl Drop for ContainerCleanup {
    fn drop(&mut self) {
        if self.armed {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let runtime = self.runtime.clone();
                let name = self.name.clone();
                handle.spawn(async move { remove_container(&runtime, &name).await });
            }
        }
    }
}
async fn remove_container(runtime: &str, name: &str) {
    let mut command = Command::new(runtime);
    command
        .args(["rm", "-f", name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let _ = tokio::time::timeout(Duration::from_secs(10), command.status()).await;
}

impl KnowledgeExecutor {
    pub fn new(config: KnowledgeConfig) -> Self {
        Self {
            config,
            instance_tag: uuid::Uuid::new_v4().simple().to_string(),
        }
    }

    pub fn with_operator(mut self, operator: &covenant_types::AgentId) -> Self {
        self.instance_tag = covenant_compute_protocol::sha256_hex(&operator.pubkey)[..16].into();
        self
    }

    /// Remove only this operator's containers from an interrupted process.
    /// Never pull an image or inspect containers belonging to another operator.
    pub async fn reap_orphans(&self) -> anyhow::Result<usize> {
        let label = format!("label=covenant.knowledge.operator={}", self.instance_tag);
        let mut command = Command::new(&self.config.runtime);
        command
            .args(["ps", "-aq", "--filter", &label])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(10), command.output()).await??;
        anyhow::ensure!(output.status.success(), "knowledge orphan listing failed");
        let output = std::str::from_utf8(&output.stdout)?;
        let ids: Vec<_> = output.lines().filter(|line| !line.is_empty()).collect();
        anyhow::ensure!(
            ids.len() <= 1024
                && ids
                    .iter()
                    .all(|id| (12..=64).contains(&id.len())
                        && id.bytes().all(|b| b.is_ascii_hexdigit())),
            "knowledge orphan listing returned invalid container identifiers"
        );
        if ids.is_empty() {
            return Ok(0);
        }
        let mut command = Command::new(&self.config.runtime);
        command
            .args(["rm", "-f"])
            .args(&ids)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let status = tokio::time::timeout(Duration::from_secs(10), command.status()).await??;
        anyhow::ensure!(status.success(), "knowledge orphan cleanup failed");
        Ok(ids.len())
    }

    fn command(&self, name: &str) -> Command {
        let mut cmd = Command::new(&self.config.runtime);
        cmd.args([
            "run",
            "--rm",
            "--pull",
            "never",
            "--name",
            name,
            "--label",
            &format!("covenant.knowledge.operator={}", self.instance_tag),
            "--network",
            "none",
            "--read-only",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--pids-limit",
            "64",
            "--memory",
            "512m",
            "--cpus",
            "1",
            "--user",
            "65532:65532",
            "--entrypoint",
            "/usr/local/bin/covenant-knowledge-compiler",
            "-i",
            &self.config.image,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
        cmd
    }
    async fn cleanup(&self, name: &str) {
        remove_container(&self.config.runtime, name).await;
    }
}

#[async_trait]
impl JobExecutor for KnowledgeExecutor {
    fn admit(&self, job: &JobEnvelopePayload, _remaining: Duration) -> Result<(), String> {
        let request = validate_knowledge_job(job).map_err(|e| e.to_string())?;
        if request.compiler_image_sha256 != self.config.image_sha256 {
            return Err("requested compiler digest is not installed on this operator".into());
        }
        Ok(())
    }

    async fn health(&self) -> Result<(), ExecutorError> {
        let mut command = Command::new(&self.config.runtime);
        command
            .args(["image", "inspect", &self.config.image])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let status = tokio::time::timeout(Duration::from_secs(10), command.status())
            .await
            .map_err(|_| ExecutorError::Failed("compiler image inspection timed out".into()))?
            .map_err(|e| ExecutorError::Failed(format!("compiler image inspection: {e}")))?;
        if !status.success() {
            return Err(ExecutorError::Failed("pinned compiler image is not installed; pull and verify it before starting the operator".into()));
        }
        Ok(())
    }

    async fn execute(
        &self,
        job: &JobEnvelopePayload,
        deadline: Duration,
    ) -> Result<ExecutionOutcome, ExecutorError> {
        self.admit(job, deadline).map_err(ExecutorError::Failed)?;
        let request =
            validate_knowledge_job(job).map_err(|e| ExecutorError::Failed(e.to_string()))?;
        let source = serde_json::to_vec(&request.source)
            .map_err(|e| ExecutorError::Failed(e.to_string()))?;
        let name = format!("covenant-knowledge-{}-{}", self.instance_tag, job.job_id);
        let started = Instant::now();
        let mut child = self
            .command(&name)
            .spawn()
            .map_err(|e| ExecutorError::Failed(format!("start compiler container: {e}")))?;
        let mut cleanup = ContainerCleanup {
            runtime: self.config.runtime.clone(),
            name: name.clone(),
            armed: true,
        };
        let mut stdin = child.stdin.take().expect("piped stdin");
        let mut stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let run = async {
            let write = async {
                stdin
                    .write_all(&source)
                    .await
                    .map_err(|e| ExecutorError::Failed(e.to_string()))?;
                stdin
                    .shutdown()
                    .await
                    .map_err(|e| ExecutorError::Failed(e.to_string()))?;
                drop(stdin);
                Ok::<_, ExecutorError>(())
            };
            let wait = async {
                child
                    .wait()
                    .await
                    .map_err(|e| ExecutorError::Failed(e.to_string()))
            };
            // Cancel the other readers/waiter immediately when a source write
            // or output ceiling fails so cleanup can preempt the container.
            let (_, output, error, status) = tokio::try_join!(
                write,
                read_stdout_capped(&mut stdout),
                async { Ok::<_, ExecutorError>(drain_stderr_tail(stderr).await) },
                wait
            )?;
            if !status.success() {
                return Err(ExecutorError::Failed(format!(
                    "knowledge compiler failed: {}",
                    error
                )));
            }
            Ok(output)
        };
        let output = match tokio::time::timeout(deadline, run).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                self.cleanup(&name).await;
                cleanup.armed = false;
                let _ = child.kill().await;
                return Err(error);
            }
            Err(_) => {
                self.cleanup(&name).await;
                cleanup.armed = false;
                let _ = child.kill().await;
                return Err(ExecutorError::Timeout(deadline));
            }
        };
        // A successful --rm container has exited before the child wait returns.
        cleanup.armed = false;
        let snapshot = serde_json::from_slice(&output).map_err(|e| {
            ExecutorError::Failed(format!("compiler returned invalid snapshot JSON: {e}"))
        })?;
        let result = KnowledgeCompileResult::new(&request, snapshot)
            .map_err(|e| ExecutorError::Failed(e.to_string()))?;
        Ok(ExecutionOutcome {
            output: knowledge_output(&result),
            wall_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            tokens_in: None,
            tokens_out: None,
            finish_reason: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_image_and_isolation_arguments() {
        assert!(KnowledgeConfig::new("docker".into(), "compiler:latest".into()).is_err());
        let e = KnowledgeExecutor::new(
            KnowledgeConfig::new(
                "docker".into(),
                format!("compiler@sha256:{}", "a".repeat(64)),
            )
            .unwrap(),
        );
        let args = e
            .command("job")
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(args.windows(2).any(|a| a == ["--network", "none"]));
        assert!(args.windows(2).any(|a| a == ["--pull", "never"]));
        assert!(args.windows(2).any(|a| a == ["--user", "65532:65532"]));
        assert!(!args.iter().any(|a| a == "sh" || a == "-c" || a == "-v"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn knowledge_executor_transports_typed_stdin_without_source_execution() {
        use covenant_compute_protocol::knowledge::{
            knowledge_digest, knowledge_input, parse_knowledge_output, KnowledgeCompileRequest,
        };
        use covenant_identity::LocalIdentity;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("fixture-runtime");
        std::fs::write(
            &runtime,
            r#"#!/usr/bin/env python3
import json, sys, time, pathlib
root = pathlib.Path(__file__).parent
if sys.argv[1] == "image":
    sys.exit(0)
if sys.argv[1] == "ps":
    assert sys.argv[-1].startswith("label=covenant.knowledge.operator=")
    print("a" * 12)
    sys.exit(0)
if sys.argv[1] == "rm":
    with (root / "removed").open("a") as log:
        log.write(" ".join(sys.argv[2:]) + "\n")
    sys.exit(0)
source = json.load(sys.stdin)
assert "--network" in sys.argv and "none" in sys.argv
assert "--read-only" in sys.argv and "--cap-drop" in sys.argv
assert not any(x in sys.argv for x in ("sh", "-c", "-v"))
if source["files"][0]["content"] == "WAIT":
    (root / "waiting").write_text("started")
    time.sleep(30)
if source["files"][0]["content"] == "OVERFLOW":
    sys.stdout.write("x" * (5 * 1024 * 1024))
    sys.stdout.flush()
    time.sleep(30)
json.dump({"source": source, "objects": []}, sys.stdout)
"#,
        )
        .unwrap();
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
        let compiler = KnowledgeExecutor::new(
            KnowledgeConfig::new(
                runtime.to_string_lossy().into_owned(),
                format!("compiler@sha256:{}", "b".repeat(64)),
            )
            .unwrap(),
        );
        let commit = "a".repeat(40);
        let source = serde_json::json!({"files":[{"path":"src/lib.rs","content":"// $(touch NEVER_EXECUTED)\npub fn hello() {}"}],"git":{"revision":commit,"dirty":false},"diagnostics":[],"config":{}});
        let request = KnowledgeCompileRequest {
            schema_version: 1,
            repository: "https://github.com/example/public".into(),
            commit,
            compiler_image_sha256: "b".repeat(64),
            source_sha256: knowledge_digest(&source).unwrap(),
            source,
        };
        let envelope = covenant_compute_buyer::sign_envelope(
            &covenant_compute_buyer::BuyerConfig {
                coordinator_url: "http://127.0.0.1:1".into(),
                poll_interval: Duration::from_millis(10),
                referral_code: None,
                rpc_url: None,
            },
            &LocalIdentity::generate("buyer@local"),
            covenant_compute_buyer::JobRequest {
                kind: covenant_compute_protocol::JobKind::BatchJob,
                input: knowledge_input(&request).unwrap(),
                model: Some(request.model().unwrap()),
                gpu_class: None,
                min_vram_gb: None,
                min_reputation_bps: None,
                price_micro_usdc: 10,
                deadline_ms: 5_000,
            },
        )
        .unwrap();
        compiler.health().await.unwrap();
        assert_eq!(compiler.reap_orphans().await.unwrap(), 1);
        assert!(std::fs::read_to_string(dir.path().join("removed"))
            .unwrap()
            .contains(&"a".repeat(12)));
        let result = compiler
            .execute(&envelope.payload, Duration::from_secs(5))
            .await
            .unwrap();
        let result = parse_knowledge_output(&result.output, &request).unwrap();
        assert_eq!(result.snapshot["source"], request.source);
        assert!(
            crate::EchoExecutor
                .admit(&envelope.payload, Duration::from_secs(5))
                .is_err(),
            "generic executors must reject compiler contracts before acceptance"
        );
        let mut overflow_request = request.clone();
        overflow_request.source["files"][0]["content"] = "OVERFLOW".into();
        overflow_request.source_sha256 = knowledge_digest(&overflow_request.source).unwrap();
        let mut overflow_job = envelope.payload.clone();
        overflow_job.input = knowledge_input(&overflow_request).unwrap();
        let overflow = tokio::time::timeout(
            Duration::from_secs(3),
            compiler.execute(&overflow_job, Duration::from_secs(10)),
        )
        .await
        .expect("output ceiling must cancel blocked child before its deadline")
        .unwrap_err();
        assert!(overflow.to_string().contains("output exceeded"));
        let mut cancelled_request = request.clone();
        cancelled_request.source["files"][0]["content"] = "WAIT".into();
        cancelled_request.source_sha256 = knowledge_digest(&cancelled_request.source).unwrap();
        let mut cancelled_job = envelope.payload.clone();
        cancelled_job.job_id = uuid::Uuid::new_v4();
        cancelled_job.input = knowledge_input(&cancelled_request).unwrap();
        let cancelled_name = cancelled_job.job_id.to_string();
        let compiler = std::sync::Arc::new(compiler);
        let task_compiler = compiler.clone();
        let task = tokio::spawn(async move {
            task_compiler
                .execute(&cancelled_job, Duration::from_secs(10))
                .await
        });
        tokio::time::timeout(Duration::from_secs(3), async {
            while !dir.path().join("waiting").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("fixture started");
        task.abort();
        let _ = task.await;
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if std::fs::read_to_string(dir.path().join("removed"))
                    .unwrap_or_default()
                    .contains(&cancelled_name)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("cancelled work removes its named container");
        let mut wrong = envelope.payload;
        wrong.capability_requirement.model_id =
            Some(format!("knowledge-compile-v1:{}", "c".repeat(64)));
        assert!(compiler.admit(&wrong, Duration::from_secs(5)).is_err());
    }
}
