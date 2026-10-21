use crate::*;
use ignore::gitignore::GitignoreBuilder;
use sha1::Digest as _;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct CompileOptions {
    pub force_full: bool,
    pub commit: Option<String>,
}

pub fn default_home() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("COVENANT_KNOWLEDGE_HOME") {
        return Ok(p.into());
    }
    Ok(PathBuf::from(
        std::env::var_os("HOME").context("HOME is unset; set COVENANT_KNOWLEDGE_HOME")?,
    )
    .join(".covenant/knowledge"))
}

pub(crate) fn sensitive_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.split('/').any(|p| {
        matches!(
            p,
            ".git"
                | ".ssh"
                | ".aws"
                | ".azure"
                | ".gnupg"
                | ".kube"
                | "node_modules"
                | "target"
                | "__pycache__"
                | ".venv"
                | "venv"
                | ".next"
                | "dist"
                | "build"
                | ".covenant"
                | "credentials"
                | "credentials.json"
                | "secrets.json"
                | "secrets.yaml"
                | "secrets.yml"
                | "id_rsa"
                | "id_ed25519"
                | "wallet.json"
                | "keypair.json"
                | "identity.json"
                | ".netrc"
                | ".npmrc"
                | ".pypirc"
                | "operator.token"
        ) || p == ".env"
            || p.starts_with(".env.")
            || p.ends_with(".pem")
            || p.ends_with(".key")
            || p.ends_with(".p12")
            || p.ends_with(".pfx")
    })
}

pub(crate) fn contains_secret(text: &str) -> bool {
    [
        "-----BEGIN PRIVATE KEY-----",
        "-----BEGIN RSA PRIVATE KEY-----",
        "-----BEGIN OPENSSH PRIVATE KEY-----",
        "-----BEGIN EC PRIVATE KEY-----",
    ]
    .iter()
    .any(|p| text.contains(p))
        || text
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .any(|word| {
                (word.starts_with("ghp_") && word.len() >= 36)
                    || (word.starts_with("github_pat_") && word.len() >= 50)
                    || (word.starts_with("AKIA") && word.len() == 20)
                    || (word.starts_with("sk-proj-") && word.len() >= 35)
            })
}

fn git(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut command = Command::new("git");
    // Metadata reads must not inherit a caller's repository, injected config,
    // replacement objects or a credentialed lazy-fetch transport. No command
    // below asks Git to inspect working-tree content or run content filters.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .args(["--no-pager", "--no-optional-locks", "--no-replace-objects"])
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ALLOW_PROTOCOL", "")
        .output()
        .context("run offline Git metadata reader")
}

fn git_blob_digest(content: &str, oid_bytes: usize) -> Result<String> {
    let header = format!("blob {}\0", content.len());
    let digest = match oid_bytes {
        40 => {
            let mut hash = sha1::Sha1::new();
            hash.update(header.as_bytes());
            hash.update(content.as_bytes());
            hash.finalize().to_vec()
        }
        64 => {
            let mut hash = Sha256::new();
            hash.update(header.as_bytes());
            hash.update(content.as_bytes());
            hash.finalize().to_vec()
        }
        _ => bail!("unsupported Git object identifier"),
    };
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

fn compare_captured_source_to_head(root: &Path, source: &mut SourceSet) -> Result<()> {
    let Some(revision) = source.git.revision.as_deref() else {
        return Ok(());
    };
    // Without --full-tree, Git limits listing to this directory and returns
    // paths relative to it. Raw tree metadata never runs clean/process filters.
    let listing = git(root, &["ls-tree", "-rz", "-l", revision, "--", "."])?;
    ensure!(listing.status.success(), "cannot inspect raw HEAD tree");
    let mut tracked = BTreeMap::<String, String>::new();
    for row in listing.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let row = std::str::from_utf8(row).context("non-UTF8 Git path unsupported")?;
        let (header, path) = row.split_once('\t').context("invalid HEAD tree entry")?;
        validate_path(path)?;
        let fields: Vec<_> = header.split_whitespace().collect();
        ensure!(fields.len() == 4, "invalid HEAD tree metadata");
        if fields[1] != "blob" || fields[0] == "120000" || sensitive_path(path) {
            continue;
        }
        let size: usize = fields[3].parse().context("invalid HEAD blob size")?;
        if size > source.config.max_file_bytes {
            continue;
        }
        if source.diagnostics.iter().any(|diagnostic| {
            path == diagnostic.path
                || path
                    .strip_prefix(&diagnostic.path)
                    .is_some_and(|p| p.starts_with('/'))
        }) {
            continue;
        }
        ensure!(
            fields[2].len() == revision.len() && fields[2].bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid HEAD blob identifier"
        );
        tracked.insert(path.into(), fields[2].into());
    }
    let mut dirty = false;
    for file in &source.files {
        let expected = tracked.remove(&file.path);
        if expected.as_deref() != Some(git_blob_digest(&file.content, revision.len())?.as_str()) {
            dirty = true;
        }
    }
    // A removed eligible tracked file has no current exclusion diagnostic.
    // Conservatively count it as a source change rather than call it clean.
    source.git.dirty = dirty || !tracked.is_empty();
    source.diagnostics.push(Diagnostic {
        path: String::new(), code: "git_source_scope".into(),
        message: "Git dirty compares captured raw source bytes with HEAD, including additions and missing admissible tracked files. Capture exclusions, Git index/filter transformations and submodule internals are outside this comparison.".into(),
    });
    Ok(())
}

pub fn capture_source(root: &Path, options: &CompileOptions) -> Result<SourceSet> {
    let root = root
        .canonicalize()
        .context("resolve repository directory")?;
    ensure!(root.is_dir(), "source must be a directory");
    if let Some(commit) = &options.commit {
        let git_root = git(&root, &["rev-parse", "--show-toplevel"])?;
        ensure!(
            git_root.status.success(),
            "--commit requires a Git repository"
        );
        let git_root = PathBuf::from(String::from_utf8(git_root.stdout)?.trim()).canonicalize()?;
        ensure!(root == git_root, "--commit requires the repository root; a subdirectory must not silently capture the whole tree");
        ensure!(
            (commit.len() == 40 || commit.len() == 64)
                && commit.bytes().all(|b| b.is_ascii_hexdigit()),
            "--commit requires a full commit SHA"
        );
        let verify = git(
            &root,
            &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
        )?;
        ensure!(verify.status.success(), "commit does not exist");
        let revision = String::from_utf8(verify.stdout)?.trim().to_string();
        let listing = git(&root, &["ls-tree", "-rz", "--full-tree", &revision])?;
        ensure!(listing.status.success(), "could not read committed tree");
        let stage = tempfile::tempdir()?;
        let mut diagnostics = Vec::new();
        let limits = CompileConfig::default();
        let mut count = 0;
        let mut total = 0;
        for row in listing.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let row = std::str::from_utf8(row).context("non-UTF8 Git path unsupported")?;
            let (header, path) = row.split_once('\t').context("invalid tree entry")?;
            validate_path(path)?;
            let fields: Vec<_> = header.split_whitespace().collect();
            if fields.len() != 3
                || fields[1] != "blob"
                || fields[0] == "120000"
                || sensitive_path(path)
            {
                diagnostics.push(Diagnostic {
                    path: path.into(),
                    code: "excluded".into(),
                    message: "Excluded sensitive, linked, or non-file committed entry.".into(),
                });
                continue;
            }
            count += 1;
            ensure!(
                count <= limits.max_files,
                "committed tree exceeds file limit"
            );
            let size = git(&root, &["cat-file", "-s", fields[2]])?;
            ensure!(size.status.success(), "cannot inspect committed file");
            let size: usize = String::from_utf8(size.stdout)?.trim().parse()?;
            if size > limits.max_file_bytes {
                diagnostics.push(Diagnostic {
                    path: path.into(),
                    code: "oversized".into(),
                    message: "Excluded file exceeding per-file byte limit.".into(),
                });
                continue;
            }
            total += size;
            ensure!(
                total <= limits.max_total_bytes,
                "committed source exceeds byte limit"
            );
            let blob = git(&root, &["cat-file", "blob", fields[2]])?;
            ensure!(blob.status.success(), "cannot read committed file");
            let dest = stage.path().join(path);
            fs::create_dir_all(dest.parent().unwrap())?;
            fs::write(dest, blob.stdout)?;
        }
        let mut source = capture_worktree(stage.path())?;
        source.git = GitMetadata {
            revision: Some(revision),
            dirty: false,
        };
        source.diagnostics.extend(diagnostics);
        source.diagnostics.sort();
        source.diagnostics.dedup();
        return Ok(source);
    }
    capture_worktree(&root)
}

fn read_bounded(root: &Path, path: &Path, max: usize) -> Result<Vec<u8>> {
    #[cfg(unix)]
    let file = {
        use std::ffi::CString;
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::OpenOptionsExt;
        let mut dir: OwnedFd = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(root)?
            .into();
        let relative = path.strip_prefix(root)?;
        let components: Vec<_> = relative.components().collect();
        ensure!(!components.is_empty(), "source path is empty");
        for (index, component) in components.iter().enumerate() {
            let Component::Normal(name) = component else {
                bail!("source path escapes repository");
            };
            let name = CString::new(name.as_bytes())?;
            let last = index + 1 == components.len();
            let flags = libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | if last {
                    libc::O_NONBLOCK
                } else {
                    libc::O_DIRECTORY
                };
            // Walk from the opened root descriptor; concurrent replacement of a
            // parent directory with a symlink cannot redirect captured bytes.
            let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            dir = unsafe { OwnedFd::from_raw_fd(fd) };
        }
        fs::File::from(dir)
    };
    #[cfg(not(unix))]
    let file = {
        ensure!(
            path.canonicalize()?.starts_with(root),
            "source path escapes repository"
        );
        ensure!(
            !path.symlink_metadata()?.file_type().is_symlink(),
            "symbolic link excluded"
        );
        OpenOptions::new().read(true).open(path)?
    };
    ensure!(file.metadata()?.is_file(), "not a regular source file");
    let mut bytes = Vec::new();
    file.take((max + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= max, "file exceeds byte limit");
    Ok(bytes)
}

fn capture_worktree(root: &Path) -> Result<SourceSet> {
    let canonical = root.canonicalize()?;
    capture_canonical_worktree(&canonical, true)
}

/// A status request is already bound to a registered canonical directory.
/// Never follow a replacement root symlink to expand that session's scope.
pub(crate) fn capture_registered_source(root: &Path) -> Result<SourceSet> {
    let before = root.symlink_metadata()?;
    ensure!(
        before.is_dir() && !before.file_type().is_symlink(),
        "registered source is not a directory"
    );
    ensure!(
        root.canonicalize()? == root,
        "registered source changed path"
    );
    // Git metadata is not a source-freshness signal. Skipping Git also keeps
    // status bounded by admitted files rather than the repository's tree size.
    let source = capture_canonical_worktree(root, false)?;
    let after = root.symlink_metadata()?;
    ensure!(
        after.is_dir() && !after.file_type().is_symlink() && root.canonicalize()? == root,
        "registered source changed while captured"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            before.dev() == after.dev() && before.ino() == after.ino(),
            "registered source directory was replaced while captured"
        );
    }
    Ok(source)
}

fn capture_canonical_worktree(root: &Path, include_git: bool) -> Result<SourceSet> {
    let config = CompileConfig::default();
    let mut source = SourceSet {
        files: vec![],
        git: GitMetadata::default(),
        diagnostics: vec![],
        config,
    };
    if include_git {
        if let Ok(output) = git(root, &["rev-parse", "HEAD"]) {
            if output.status.success() {
                source.git.revision = Some(String::from_utf8_lossy(&output.stdout).trim().into());
            }
        }
    }
    let mut stack = vec![(root.to_path_buf(), Vec::<PathBuf>::new())];
    let mut seen = 0usize;
    let mut total = 0usize;
    while let Some((dir, mut rules)) = stack.pop() {
        for name in [".gitignore", ".covenantignore"] {
            let rule = dir.join(name);
            if rule
                .symlink_metadata()
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
            {
                // Ignore configuration is also bounded untrusted input.
                ensure!(
                    fs::metadata(&rule)?.len() <= 1024 * 1024,
                    "ignore file too large"
                );
                rules.push(rule);
            }
        }
        let mut builder = GitignoreBuilder::new(root);
        for p in &rules {
            let contents = String::from_utf8(read_bounded(root, p, 1024 * 1024)?)
                .context("ignore file is not UTF8")?;
            for line in contents.lines() {
                builder
                    .add_line(Some(p.clone()), line)
                    .with_context(|| format!("invalid ignore rules {}", p.display()))?;
            }
        }
        let ignores = builder.build()?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&dir)? {
            seen += 1;
            ensure!(
                seen <= source.config.max_files * 4,
                "directory entry limit exceeded"
            );
            entries.push(entry?);
        }
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)?
                .to_str()
                .context("non-UTF8 path unsupported")?
                .replace('\\', "/");
            validate_path(&relative)?;
            let meta = path.symlink_metadata()?;
            let excluded = if sensitive_path(&relative) {
                Some(("excluded", "Sensitive or generated path excluded."))
            } else if meta.file_type().is_symlink() {
                Some(("symlink", "Symbolic links are not followed."))
            } else if ignores
                .matched_path_or_any_parents(&path, meta.is_dir())
                .is_ignore()
            {
                Some(("ignored", "Excluded by .gitignore or .covenantignore."))
            } else {
                None
            };
            if let Some((code, message)) = excluded {
                source.diagnostics.push(Diagnostic {
                    path: relative,
                    code: code.into(),
                    message: message.into(),
                });
                continue;
            }
            ensure!(
                path.canonicalize()?.starts_with(root),
                "source path escapes repository"
            );
            if meta.is_dir() {
                ensure!(
                    path.components().count() - root.components().count() <= 64,
                    "directory depth limit exceeded"
                );
                stack.push((path, rules.clone()));
                continue;
            }
            if !meta.is_file() {
                source.diagnostics.push(Diagnostic {
                    path: relative,
                    code: "special_file".into(),
                    message: "Non-regular file excluded.".into(),
                });
                continue;
            }
            if meta.len() > source.config.max_file_bytes as u64 {
                source.diagnostics.push(Diagnostic {
                    path: relative,
                    code: "oversized".into(),
                    message: "Excluded file exceeding per-file byte limit.".into(),
                });
                continue;
            }
            let bytes = read_bounded(root, &path, source.config.max_file_bytes)?;
            let content = match String::from_utf8(bytes) {
                Ok(s) if !s.contains('\0') => s,
                _ => {
                    source.diagnostics.push(Diagnostic {
                        path: relative,
                        code: "binary".into(),
                        message: "Binary or non-UTF8 file excluded.".into(),
                    });
                    continue;
                }
            };
            if contains_secret(&content) {
                source.diagnostics.push(Diagnostic {path:relative,code:"credential_detected".into(),
                    message:"File excluded by credential signature detection; detection is not exhaustive.".into()});
                continue;
            }
            total += content.len();
            ensure!(
                total <= source.config.max_total_bytes,
                "source exceeds total byte limit"
            );
            source.files.push(SourceFile {
                path: relative,
                content,
            });
            ensure!(
                source.files.len() <= source.config.max_files,
                "source exceeds file count limit"
            );
        }
    }
    source.files.sort_by(|a, b| a.path.cmp(&b.path));
    if include_git {
        compare_captured_source_to_head(root, &mut source)?;
    }
    source.diagnostics.sort();
    validate_source(&source)?;
    Ok(source)
}
