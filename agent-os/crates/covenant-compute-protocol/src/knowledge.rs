//! Bounded, deterministic knowledge compilation over the existing batch wire.
//! The image digest is part of routing and of the signed input. This contract
//! never accepts a shell command or an arbitrary source URL to fetch on a node.
use covenant_mcp::Content;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{sha256_hex, JobEnvelopePayload, JobKind, ProtocolError};

pub const KNOWLEDGE_MODEL_PREFIX: &str = "knowledge-compile-v1:";
pub const MAX_KNOWLEDGE_SOURCE_BYTES: usize = 512 * 1024;
pub const MAX_KNOWLEDGE_RESULT_BYTES: usize = 3 * 1024 * 1024;
pub const MAX_KNOWLEDGE_FILES: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeCompileRequest {
    pub schema_version: u32,
    /// Public GitHub repository, verified anonymously by the buyer before buying.
    pub repository: String,
    pub commit: String,
    pub compiler_image_sha256: String,
    pub source_sha256: String,
    /// Portable covenant-knowledge SourceSet. No host filesystem paths.
    pub source: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeCompileResult {
    pub schema_version: u32,
    pub compiler_image_sha256: String,
    pub source_sha256: String,
    pub snapshot_sha256: String,
    /// Portable covenant-knowledge Snapshot, independently validated on import.
    pub snapshot: Value,
}

pub fn knowledge_digest(value: &Value) -> Result<String, ProtocolError> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
}

pub fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn knowledge_model(image_sha256: &str) -> Result<String, ProtocolError> {
    if !is_sha256(image_sha256) {
        return Err(ProtocolError::Invalid(
            "compiler image must be a lowercase SHA-256 digest".into(),
        ));
    }
    Ok(format!("{KNOWLEDGE_MODEL_PREFIX}{image_sha256}"))
}

/// Only canonical GitHub repository origins, never credentials, ports, query
/// strings, redirects or caller-supplied network hosts.
pub fn knowledge_repository_parts(repository: &str) -> Result<(&str, &str), ProtocolError> {
    let path = repository
        .strip_prefix("https://github.com/")
        .ok_or_else(|| {
            ProtocolError::Invalid(
                "knowledge pilot requires https://github.com/owner/repository".into(),
            )
        })?;
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|s| {
            s.is_empty()
                || *s == "."
                || *s == ".."
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
        || parts[1].ends_with(".git")
    {
        return Err(ProtocolError::Invalid(
            "repository must be a canonical GitHub origin without .git".into(),
        ));
    }
    Ok((parts[0], parts[1]))
}

pub fn valid_knowledge_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
        && !path.split('/').any(|s| s == ".git")
}

impl KnowledgeCompileRequest {
    pub fn model(&self) -> Result<String, ProtocolError> {
        knowledge_model(&self.compiler_image_sha256)
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != 1 {
            return Err(ProtocolError::Invalid(
                "unsupported knowledge request version".into(),
            ));
        }
        self.model()?;
        knowledge_repository_parts(&self.repository)?;
        if self.commit.len() != 40
            || !self
                .commit
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ProtocolError::Invalid(
                "knowledge compile requires a full lowercase Git commit".into(),
            ));
        }
        let bytes = serde_json::to_vec(&self.source)?;
        if bytes.len() > MAX_KNOWLEDGE_SOURCE_BYTES || sha256_hex(&bytes) != self.source_sha256 {
            return Err(ProtocolError::Invalid(
                "knowledge source exceeds 512 KiB or fails its digest".into(),
            ));
        }
        if self.source.pointer("/git/dirty") != Some(&Value::Bool(false))
            || self.source.pointer("/git/revision").and_then(Value::as_str) != Some(&self.commit)
        {
            return Err(ProtocolError::Invalid(
                "remote knowledge compilation requires a clean pinned commit".into(),
            ));
        }
        let files = self
            .source
            .get("files")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ProtocolError::Invalid("knowledge source requires a files array".into())
            })?;
        if files.is_empty() || files.len() > MAX_KNOWLEDGE_FILES {
            return Err(ProtocolError::Invalid(
                "knowledge source must contain 1..=256 files".into(),
            ));
        }
        let mut previous: Option<&str> = None;
        for file in files {
            let path = file
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ProtocolError::Invalid("knowledge file path is required".into()))?;
            if !valid_knowledge_path(path)
                || previous.is_some_and(|p| p >= path)
                || file.get("content").and_then(Value::as_str).is_none()
            {
                return Err(ProtocolError::Invalid(
                    "knowledge files must have safe unique sorted paths and UTF-8 content".into(),
                ));
            }
            previous = Some(path);
        }
        Ok(())
    }
}

impl KnowledgeCompileResult {
    pub fn new(request: &KnowledgeCompileRequest, snapshot: Value) -> Result<Self, ProtocolError> {
        let result = Self {
            schema_version: 1,
            compiler_image_sha256: request.compiler_image_sha256.clone(),
            source_sha256: request.source_sha256.clone(),
            snapshot_sha256: knowledge_digest(&snapshot)?,
            snapshot,
        };
        result.verify(request)?;
        Ok(result)
    }

    pub fn verify(&self, request: &KnowledgeCompileRequest) -> Result<(), ProtocolError> {
        request.validate()?;
        if self.schema_version != 1
            || self.compiler_image_sha256 != request.compiler_image_sha256
            || self.source_sha256 != request.source_sha256
            || knowledge_digest(&self.snapshot)? != self.snapshot_sha256
            || self.snapshot.get("source") != Some(&request.source)
            || serde_json::to_vec(self)?.len() > MAX_KNOWLEDGE_RESULT_BYTES
        {
            return Err(ProtocolError::Invalid(
                "knowledge result version, source, compiler, size or snapshot digest mismatch"
                    .into(),
            ));
        }
        Ok(())
    }
}

pub fn knowledge_input(request: &KnowledgeCompileRequest) -> Result<Vec<Content>, ProtocolError> {
    request.validate()?;
    Ok(vec![Content::json(
        serde_json::json!({"knowledge_compile": request}),
    )])
}

pub fn parse_knowledge_input(input: &[Content]) -> Result<KnowledgeCompileRequest, ProtocolError> {
    let [Content::Json { value }] = input else {
        return Err(ProtocolError::Invalid(
            "knowledge compile requires exactly one typed JSON block".into(),
        ));
    };
    if value.as_object().is_none_or(|v| v.len() != 1) {
        return Err(ProtocolError::Invalid(
            "knowledge compile contains unexpected input fields".into(),
        ));
    }
    let request: KnowledgeCompileRequest = serde_json::from_value(
        value
            .get("knowledge_compile")
            .cloned()
            .ok_or_else(|| ProtocolError::Invalid("missing knowledge_compile".into()))?,
    )?;
    request.validate()?;
    Ok(request)
}

pub fn validate_knowledge_job(
    job: &JobEnvelopePayload,
) -> Result<KnowledgeCompileRequest, ProtocolError> {
    let request = parse_knowledge_input(&job.input)?;
    if job.kind != JobKind::BatchJob
        || job.stream
        || job.capability_requirement.model_id.as_deref() != Some(request.model()?.as_str())
    {
        return Err(ProtocolError::Invalid(
            "knowledge compile requires its exact image model, non-streaming batch job".into(),
        ));
    }
    Ok(request)
}

pub fn knowledge_output(result: &KnowledgeCompileResult) -> Vec<Content> {
    vec![Content::json(
        serde_json::json!({"knowledge_result": result}),
    )]
}

pub fn parse_knowledge_output(
    output: &[Content],
    request: &KnowledgeCompileRequest,
) -> Result<KnowledgeCompileResult, ProtocolError> {
    let [Content::Json { value }] = output else {
        return Err(ProtocolError::Invalid(
            "knowledge output requires exactly one typed JSON block".into(),
        ));
    };
    if value.as_object().is_none_or(|v| v.len() != 1) {
        return Err(ProtocolError::Invalid(
            "knowledge output contains unexpected fields".into(),
        ));
    }
    let result: KnowledgeCompileResult = serde_json::from_value(
        value
            .get("knowledge_result")
            .cloned()
            .ok_or_else(|| ProtocolError::Invalid("missing knowledge_result".into()))?,
    )?;
    result.verify(request)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> KnowledgeCompileRequest {
        let commit = "a".repeat(40);
        let source = serde_json::json!({"files":[{"path":"src/lib.rs","content":"fn f() {}"}],"git":{"revision":commit,"dirty":false},"diagnostics":[],"config":{}});
        KnowledgeCompileRequest {
            schema_version: 1,
            repository: "https://github.com/example/project".into(),
            commit,
            compiler_image_sha256: "b".repeat(64),
            source_sha256: knowledge_digest(&source).unwrap(),
            source,
        }
    }
    #[test]
    fn roundtrip_and_tamper_controls() {
        let r = request();
        assert_eq!(
            parse_knowledge_input(&knowledge_input(&r).unwrap()).unwrap(),
            r
        );
        let result =
            KnowledgeCompileResult::new(&r, serde_json::json!({"source":r.source,"objects":[]}))
                .unwrap();
        assert_eq!(
            parse_knowledge_output(&knowledge_output(&result), &r).unwrap(),
            result
        );
        let mut forged = result.clone();
        forged.snapshot = serde_json::json!({"objects":["forged"]});
        assert!(forged.verify(&r).is_err());
        let mut dirty = r.clone();
        dirty.source["git"]["dirty"] = true.into();
        dirty.source_sha256 = knowledge_digest(&dirty.source).unwrap();
        assert!(dirty.validate().is_err());
        let mut traversal = r.clone();
        traversal.source["files"][0]["path"] = "../secret".into();
        traversal.source_sha256 = knowledge_digest(&traversal.source).unwrap();
        assert!(traversal.validate().is_err());
        assert!(parse_knowledge_input(&[Content::text("echo run")]).is_err());
    }
    #[test]
    fn private_network_and_wildcard_routes_refused() {
        for url in [
            "http://github.com/a/b",
            "https://127.0.0.1/a/b",
            "https://github.com/a/b.git",
            "https://github.com/a/b?token=x",
            "https://github.com/a/b/",
        ] {
            assert!(knowledge_repository_parts(url).is_err(), "{url}");
        }
        let model = request().model().unwrap();
        assert!(!crate::model_matches("any", &model));
        assert!(crate::model_matches(&model, &model));
        let alias = format!("{model}:latest");
        assert!(!crate::model_matches(&alias, &model));
        assert_eq!(crate::canonical_model(&alias), alias);
        assert_eq!(crate::canonical_model(&model), model);
    }
}
