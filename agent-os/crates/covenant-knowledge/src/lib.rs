//! Offline source knowledge. Evidence is a frozen source observation, not a proof
//! of semantic correctness. No repository code, macros or build hooks execute.
mod parser;
mod source;
mod store;

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

pub use source::{capture_source, default_home, CompileOptions};
pub use store::{Project, Store};

pub const SCHEMA_VERSION: u32 = 1;
pub const COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CompileConfig {
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
}
impl Default for CompileConfig {
    fn default() -> Self {
        Self {
            max_files: 20_000,
            max_file_bytes: 1024 * 1024,
            max_total_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GitMetadata {
    pub revision: Option<String>,
    pub dirty: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub path: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceSet {
    pub files: Vec<SourceFile>,
    pub git: GitMetadata,
    pub diagnostics: Vec<Diagnostic>,
    pub config: CompileConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub language: String,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub source_sha256: String,
    pub signature: String,
    pub exported: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub from: String,
    pub to: Option<String>,
    pub kind: String,
    pub target: String,
    pub resolution: String,
    pub path: String,
    pub line: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u32,
    pub compiler_version: String,
    pub parser_versions: BTreeMap<String, String>,
    pub id: String,
    pub source: SourceSet,
    pub objects: Vec<Object>,
    pub relationships: Vec<Relationship>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn parser_versions() -> BTreeMap<String, String> {
    [
        ("tree-sitter", "0.25.10"),
        ("rust", "0.24.2"),
        ("javascript", "0.25.0"),
        ("typescript", "0.23.2"),
        ("python", "0.25.0"),
        ("markdown", "covenant-sections-v1"),
        ("extractor", "covenant-syntax-v2"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}

pub fn validate_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty() && path.len() <= 4096,
        "invalid source path length"
    );
    ensure!(
        !path.contains('\\') && !path.contains('\0') && !path.contains(':'),
        "source path must be a portable relative path"
    );
    ensure!(
        Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "source path escapes snapshot"
    );
    Ok(())
}

pub fn validate_source(source: &SourceSet) -> Result<()> {
    let hard = CompileConfig::default();
    ensure!(
        source.config.max_files > 0 && source.config.max_files <= hard.max_files,
        "file limit outside supported bounds"
    );
    ensure!(
        source.config.max_file_bytes > 0 && source.config.max_file_bytes <= hard.max_file_bytes,
        "per-file limit outside supported bounds"
    );
    ensure!(
        source.config.max_total_bytes > 0 && source.config.max_total_bytes <= hard.max_total_bytes,
        "total source limit outside supported bounds"
    );
    ensure!(
        source.files.len() <= source.config.max_files,
        "too many source files"
    );
    let mut paths = BTreeSet::new();
    let mut total = 0usize;
    for f in &source.files {
        validate_path(&f.path)?;
        ensure!(paths.insert(&f.path), "duplicate source path");
        ensure!(
            !source::sensitive_path(&f.path),
            "sensitive source path excluded: {}",
            f.path
        );
        ensure!(
            f.content.len() <= source.config.max_file_bytes,
            "source file too large: {}",
            f.path
        );
        ensure!(!f.content.contains('\0'), "binary source is unsupported");
        ensure!(
            !source::contains_secret(&f.content),
            "credential material detected: {}",
            f.path
        );
        total = total
            .checked_add(f.content.len())
            .context("source size overflow")?;
        ensure!(
            total <= source.config.max_total_bytes,
            "source exceeds total byte limit"
        );
    }
    if let Some(rev) = &source.git.revision {
        ensure!(
            (rev.len() == 40 || rev.len() == 64) && rev.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid Git revision"
        );
    }
    Ok(())
}

/// Canonical, portable compilation, also used inside the remote compiler image.
pub fn compile_source(source: SourceSet) -> Result<Snapshot> {
    compile_cached(source, &BTreeMap::new()).map(|(s, _, _)| s)
}

pub(crate) fn compile_cached(
    mut source: SourceSet,
    cache: &BTreeMap<String, parser::ParsedFile>,
) -> Result<(Snapshot, usize, BTreeMap<String, parser::ParsedFile>)> {
    validate_source(&source)?;
    source.files.sort_by(|a, b| a.path.cmp(&b.path));
    source.diagnostics.sort();
    source.diagnostics.dedup();
    let mut objects = Vec::new();
    let mut relationships = Vec::new();
    let mut diagnostics = source.diagnostics.clone();
    let mut reused = 0;
    let mut fresh = BTreeMap::new();
    for file in &source.files {
        let key = parser::cache_key(file);
        let parsed = match cache.get(&key) {
            Some(p) => {
                reused += 1;
                p.clone()
            }
            None => {
                let parsed = parser::parse(file)?;
                fresh.insert(key, parsed.clone());
                parsed
            }
        };
        ensure!(
            objects.len() + parsed.objects.len() <= 200_000
                && relationships.len() + parsed.relationships.len() <= 400_000,
            "snapshot structural limit exceeded; compile a smaller source set"
        );
        objects.extend(parsed.objects);
        relationships.extend(parsed.relationships);
        diagnostics.extend(parsed.diagnostics);
    }
    parser::resolve(&objects, &mut relationships);
    objects.sort_by(|a, b| {
        (&a.path, a.start_byte, &a.kind, &a.name).cmp(&(&b.path, b.start_byte, &b.kind, &b.name))
    });
    relationships.sort();
    relationships.dedup();
    diagnostics.sort();
    diagnostics.dedup();
    let mut snapshot = Snapshot {
        schema_version: SCHEMA_VERSION,
        compiler_version: COMPILER_VERSION.into(),
        parser_versions: parser_versions(),
        id: String::new(),
        source,
        objects,
        relationships,
        diagnostics,
    };
    snapshot.id = snapshot_digest(&snapshot)?;
    Ok((snapshot, reused, fresh))
}

pub fn snapshot_digest(snapshot: &Snapshot) -> Result<String> {
    let mut value = snapshot.clone();
    value.id.clear();
    Ok(sha256(&serde_json::to_vec(&value)?))
}

/// Fully recompile an imported artifact. A self-consistent hash alone does not
/// establish that its relationships were derived by this compiler.
pub fn validate_snapshot(snapshot: &Snapshot) -> Result<()> {
    ensure!(
        snapshot.schema_version == SCHEMA_VERSION,
        "unsupported knowledge schema"
    );
    ensure!(
        snapshot.compiler_version == COMPILER_VERSION
            && snapshot.parser_versions == parser_versions(),
        "unsupported compiler/parser version"
    );
    ensure!(
        snapshot.id == snapshot_digest(snapshot)?,
        "snapshot hash mismatch"
    );
    let rebuilt = compile_source(snapshot.source.clone())?;
    if &rebuilt != snapshot {
        bail!("snapshot knowledge does not match source compilation");
    }
    Ok(())
}

pub(crate) fn evidence(snapshot: &Snapshot, object: &Object) -> Result<serde_json::Value> {
    let file = snapshot
        .source
        .files
        .iter()
        .find(|f| f.path == object.path)
        .context("evidence source missing")?;
    ensure!(
        sha256(file.content.as_bytes()) == object.source_sha256,
        "source digest mismatch"
    );
    let fragment = file
        .content
        .get(object.start_byte..object.end_byte)
        .context("invalid evidence byte range")?;
    let truncated = fragment.len() > 16 * 1024;
    let fragment: String = fragment.chars().take(16 * 1024).collect();
    Ok(serde_json::json!({
        "object_id":object.id, "snapshot_id":snapshot.id, "path":object.path,
        "revision":snapshot.source.git.revision, "dirty":snapshot.source.git.dirty,
        "source_sha256":object.source_sha256, "start_line":object.start_line,
        "end_line":object.end_line, "start_byte":object.start_byte,"end_byte":object.end_byte,
        "fragment":fragment,
        "truncated":truncated,
        "limitations":["Static source observation; does not prove runtime behavior or semantic correctness."]
    }))
}
