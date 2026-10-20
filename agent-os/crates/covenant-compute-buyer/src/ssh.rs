//! Lease-bound SSH with an optional, independently authenticated TLS outer
//! channel. Provider metadata never becomes a command. OpenSSH verifies the
//! exact hardware-bound host key even when dialing our single-use local relay.
//! TLS authenticates a gateway; it does not promote the lease's hardware trust.
use crate::{hardware::HardwareClient, BuyerError};
use covenant_compute_attestation::{
    binding::{HardwareScope, HardwareVerdict},
    vendor_types::TrustClass,
};
use covenant_compute_protocol::ssh::{SshAccess, SshTransport};
use rustls::{pki_types::ServerName, ClientConfig, RootCertStore};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::Command,
};
use tokio_rustls::{client::TlsStream, TlsConnector};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(120);
const OUTPUT_LIMIT: u64 = 16 * 1024;
/// A transfer is a single bounded attempt. An uncertain/partial remote write
/// must be reconciled by its original destination; it is never auto-retried.
pub const MAX_TRANSFER_BYTES: u64 = 64 * 1024 * 1024;

fn invalid(error: impl ToString) -> BuyerError {
    BuyerError::Coordinator(format!("lease SSH: {}", error.to_string()))
}

#[derive(Debug)]
pub struct SshOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub struct SshClient {
    tls: Arc<ClientConfig>,
}

impl Default for SshClient {
    fn default() -> Self {
        let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Self {
            tls: Arc::new(tls_config(roots)),
        }
    }
}

fn tls_config(roots: RootCertStore) -> ClientConfig {
    // Choose the provider explicitly: other workspace crates may enable aws-lc.
    ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

/// Construct channel metadata from an independently pinned scope, never from
/// a human-readable provider endpoint or an observed list of multiple keys.
fn scoped_access(scope: &HardwareScope) -> Result<SshAccess, BuyerError> {
    let access = SshAccess {
        host: scope.host.clone(),
        port: scope.port,
        username: scope.username.clone(),
        lease_generation: scope.lease_generation.clone(),
        transport: scope.transport,
        observed_host_keys: vec![scope.guest_channel_key.clone()],
    };
    access.validate().map_err(invalid)?;
    Ok(access)
}

fn remaining(verdict: &HardwareVerdict, scope: &HardwareScope) -> Result<Duration, BuyerError> {
    let now = crate::epoch_ms();
    // verify_lease already checked the floor and cryptographic evidence. Recheck
    // the clock and all deadlines immediately before any connection/payload.
    if now < verdict.verified_at_ms
        || now >= verdict.expires_at_ms
        || now >= scope.lease_expires_at_ms
    {
        return Err(invalid("hardware verification expired before channel use"));
    }
    Ok(
        Duration::from_millis(verdict.expires_at_ms.min(scope.lease_expires_at_ms) - now)
            .min(OPERATION_TIMEOUT),
    )
}

impl SshClient {
    /// Fetch the CURRENT lease verdict and re-run vendor verification locally,
    /// then perform an authenticated, noninteractive id/pwd probe. No saved
    /// verdict or runtime verification bypass is accepted by this API.
    pub async fn probe(
        &self,
        hardware: &HardwareClient<'_>,
        scope: &HardwareScope,
        floor: TrustClass,
        private_key: &Path,
    ) -> Result<SshOutput, BuyerError> {
        let access = scoped_access(scope)?;
        let verdict = hardware.verify_lease(scope, floor).await?;
        self.run(
            &access,
            private_key,
            "id -u; pwd",
            None,
            remaining(&verdict, scope)?,
        )
        .await
    }

    /// Upload one file into a new, flat /workspace path. Only the buyer chooses
    /// the basename; no command, path expansion, directory traversal, overwrite,
    /// shell configuration, forwarding or agent authentication is accepted.
    /// The fresh current-lease/hardware check completes before source bytes are
    /// read. OpenSSH sends them only after the exact pinned key authenticates.
    pub async fn upload_file(
        &self,
        hardware: &HardwareClient<'_>,
        scope: &HardwareScope,
        floor: TrustClass,
        private_key: &Path,
        source: &Path,
        basename: &str,
    ) -> Result<SshOutput, BuyerError> {
        validate_basename(basename)?;
        let access = scoped_access(scope)?;
        let verdict = hardware.verify_lease(scope, floor).await?;
        let file = open_source(source)?;
        let length = file.metadata().map_err(invalid)?.len();
        if length > MAX_TRANSFER_BYTES {
            return Err(invalid("workspace transfer exceeds the 64 MiB limit"));
        }
        self.run(
            &access,
            private_key,
            &upload_command(&format!("/workspace/{basename}")),
            Some((file, length)),
            remaining(&verdict, scope)?,
        )
        .await
    }

    async fn run(
        &self,
        access: &SshAccess,
        private_key: &Path,
        remote_command: &str,
        input: Option<(std::fs::File, u64)>,
        duration: Duration,
    ) -> Result<SshOutput, BuyerError> {
        access.validate().map_err(invalid)?;
        if access.observed_host_keys.len() != 1 || duration.is_zero() {
            return Err(invalid(
                "exactly one host-key pin and a live deadline are required",
            ));
        }
        let private_key = private_key_path(private_key)?;
        let mut known_hosts = tempfile::NamedTempFile::new().map_err(invalid)?;
        known_hosts
            .write_all(access.known_hosts().map_err(invalid)?.as_bytes())
            .map_err(invalid)?;
        known_hosts.flush().map_err(invalid)?;
        let result = tokio::time::timeout(duration, async {
            let (host, port, _relay) = match access.transport {
                None => (access.host.clone(), access.port, None),
                Some(SshTransport::OutboundRelay) => return Err(invalid("outbound relay requires the dedicated verified relay client")),
                Some(SshTransport::Tls) => {
                    // The certificate is validated BEFORE OpenSSH is started.
                    let tls = connect_tls(access, self.tls.clone()).await?;
                    let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(invalid)?;
                    let port = listener.local_addr().map_err(invalid)?.port();
                    let task = tokio::spawn(async move {
                        let (mut local, _) = listener.accept().await?;
                        drop(listener); // exactly one connection, never a general proxy
                        let mut tls = tls;
                        tokio::io::copy_bidirectional(&mut local, &mut tls).await?;
                        Ok::<(), std::io::Error>(())
                    });
                    ("127.0.0.1".into(), port, Some(Relay(task)))
                }
            };
            let mut command = ssh_command(access, &host, port, &private_key, known_hosts.path())?;
            command.arg(remote_command).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
            let mut child = command.spawn().map_err(invalid)?;
            let mut stdin = child.stdin.take().ok_or_else(|| invalid("missing SSH stdin"))?;
            let stdout = child.stdout.take().ok_or_else(|| invalid("missing SSH stdout"))?;
            let stderr = child.stderr.take().ok_or_else(|| invalid("missing SSH stderr"))?;
            let writer = async move {
                if let Some((file, length)) = input {
                    let mut source = tokio::fs::File::from_std(file).take(length);
                    let count = tokio::io::copy(&mut source, &mut stdin).await.map_err(invalid)?;
                    if count != length {
                        return Err(invalid("workspace source changed during transfer"));
                    }
                }
                stdin.shutdown().await.map_err(invalid)?;
                drop(stdin);
                Ok(())
            };
            let tasks = tokio::try_join!(
                writer,
                bounded_output(stdout),
                bounded_output(stderr),
                async { child.wait().await.map_err(invalid) },
            );
            match tasks {
                Ok(((), stdout, stderr, status)) if status.success() => Ok(SshOutput { stdout, stderr }),
                Ok((_, _, _, status)) => Err(invalid(format!("SSH exited with {status}; remote write may be partial, do not automatically retry"))),
                Err(error) => {
                    let _ = child.kill().await;
                    Err(error)
                }
            }
        }).await;
        result.map_err(|_| {
            invalid(
                "channel deadline expired; remote write may be partial, do not automatically retry",
            )
        })?
    }
}

// Abort the relay on success, error, deadline, or cancellation. OpenSSH's
// kill_on_drop closes the matching child; no detached tunnel survives.
struct Relay(tokio::task::JoinHandle<std::io::Result<()>>);
impl Drop for Relay {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn connect_tls(
    access: &SshAccess,
    config: Arc<ClientConfig>,
) -> Result<TlsStream<TcpStream>, BuyerError> {
    SshTransport::Tls
        .validate_host(&access.host)
        .map_err(invalid)?;
    let name = ServerName::try_from(access.host.clone()).map_err(invalid)?;
    tokio::time::timeout(CONNECT_TIMEOUT, async {
        let tcp = TcpStream::connect((access.host.as_str(), access.port))
            .await
            .map_err(invalid)?;
        tcp.set_nodelay(true).map_err(invalid)?;
        TlsConnector::from(config)
            .connect(name, tcp)
            .await
            .map_err(|_| invalid("TLS certificate or gateway handshake failed"))
    })
    .await
    .map_err(|_| invalid("TLS gateway connection timed out"))?
}

async fn bounded_output(reader: impl AsyncRead + Unpin) -> Result<Vec<u8>, BuyerError> {
    let mut bytes = Vec::new();
    reader
        .take(OUTPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(invalid)?;
    if bytes.len() as u64 > OUTPUT_LIMIT {
        return Err(invalid("SSH output exceeds limit"));
    }
    Ok(bytes)
}

fn validate_basename(name: &str) -> Result<(), BuyerError> {
    if name.is_empty()
        || name.len() > 128
        || name.starts_with('.')
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(invalid(
            "workspace destination must be a 1..128 byte plain basename",
        ));
    }
    Ok(())
}

fn upload_command(destination: &str) -> String {
    // The measured Phala image includes /usr/bin/python3. O_EXCL rejects every
    // preexisting object, including FIFOs and symlinks (shell noclobber alone
    // can still open non-regular files). An acknowledged upload is fsynced.
    let path = serde_json::to_string(destination).expect("path serializes");
    let script = format!(
        "import os,shutil,sys; f=os.fdopen(os.open({path},os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),'wb'); shutil.copyfileobj(sys.stdin.buffer,f); f.flush(); os.fsync(f.fileno()); f.close()"
    );
    format!("/usr/bin/python3 -I -c '{}'", script.replace('\'', "'\\''"))
}

fn open_source(path: &Path) -> Result<std::fs::File, BuyerError> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(invalid)?;
    if !file.metadata().map_err(invalid)?.is_file() {
        return Err(invalid("workspace source must be a regular file"));
    }
    Ok(file)
}

fn private_key_path(path: &Path) -> Result<PathBuf, BuyerError> {
    let metadata = std::fs::symlink_metadata(path).map_err(invalid)?;
    if !metadata.file_type().is_file() {
        return Err(invalid(
            "SSH identity must be a regular file, not a symlink",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(invalid(
                "SSH identity must be private (mode 0600 or stricter)",
            ));
        }
    }
    path.canonicalize().map_err(invalid)
}

fn ssh_command(
    access: &SshAccess,
    host: &str,
    port: u16,
    private_key: &Path,
    known_hosts: &Path,
) -> Result<Command, BuyerError> {
    let mut command = Command::new(if cfg!(unix) {
        "/usr/bin/ssh"
    } else {
        "ssh.exe"
    });
    // No shell is involved locally. Ignore user/system SSH config, agents,
    // commands, proxies, global/DNS trust and connection multiplexing.
    command.args(["-F", if cfg!(unix) { "/dev/null" } else { "NUL" }, "-T"]);
    for option in [
        "BatchMode=yes",
        "IdentitiesOnly=yes",
        "IdentityAgent=none",
        "StrictHostKeyChecking=yes",
        "GlobalKnownHostsFile=none",
        "KnownHostsCommand=none",
        "VerifyHostKeyDNS=no",
        "UpdateHostKeys=no",
        "CanonicalizeHostname=no",
        "CheckHostIP=no",
        "ClearAllForwardings=yes",
        "ForwardAgent=no",
        "ForwardX11=no",
        "PermitLocalCommand=no",
        "ProxyCommand=none",
        "ProxyJump=none",
        "ControlMaster=no",
        "ControlPath=none",
        "PasswordAuthentication=no",
        "KbdInteractiveAuthentication=no",
        "PreferredAuthentications=publickey",
        "HostbasedAuthentication=no",
        "GSSAPIAuthentication=no",
        "NumberOfPasswordPrompts=0",
        "ConnectionAttempts=1",
        "ConnectTimeout=10",
        "ServerAliveInterval=10",
        "ServerAliveCountMax=1",
        "EscapeChar=none",
        "LogLevel=ERROR",
    ] {
        command.args(["-o", option]);
    }
    let alias = if access.port == 22 && !access.host.contains(':') {
        access.host.clone()
    } else {
        format!("[{}]:{}", access.host, access.port)
    };
    let path = known_hosts
        .to_str()
        .ok_or_else(|| invalid("known-hosts path is not UTF-8"))?;
    if path.chars().any(char::is_control) {
        return Err(invalid("known-hosts path contains control characters"));
    }
    let algorithms = match access.observed_host_keys[0].algorithm.as_str() {
        "ssh-rsa" => "rsa-sha2-512,rsa-sha2-256",
        other => other,
    };
    command.arg("-o").arg(format!("HostKeyAlias={alias}"));
    command
        .arg("-o")
        .arg(format!("HostKeyAlgorithms={algorithms}"));
    command.arg("-o").arg(format!(
        "UserKnownHostsFile=\"{}\"",
        path.replace('\\', "\\\\").replace('"', "\\\"")
    ));
    command
        .arg("-i")
        .arg(private_key)
        .arg("-l")
        .arg(&access.username)
        .arg("-p")
        .arg(port.to_string())
        .arg("--")
        .arg(host);
    Ok(command)
}

#[cfg(test)]
mod tests;
