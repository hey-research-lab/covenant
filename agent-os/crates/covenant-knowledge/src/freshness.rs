//! Compare saved evidence with a bounded, read-only capture of its own source.
//! This does not compile, update the parser cache, or advance project history.
use crate::*;
use serde_json::{json, Value};

fn is_ignore_file(path: &str) -> bool {
    matches!(
        path.rsplit('/').next(),
        Some(".gitignore" | ".covenantignore")
    )
}

pub(crate) fn freshness(root: &Path, saved: &SourceSet) -> Value {
    let unavailable = |reason: &str| {
        json!({"state":"unavailable", "reason":reason,
            "scope":"Admitted source files, current default capture limits, and captured repository .gitignore/.covenantignore rules. Git metadata and excluded content are not freshness signals.",
            "changes":null,"config_changed":null,"ignore_rules_changed":null,
            "git_metadata_checked":false,"git_metadata_changed":null})
    };
    // Project roots are canonical when registered. A later symlink replacement
    // must not let a project-bound MCP session inspect a different directory.
    let Ok(metadata) = root.symlink_metadata() else {
        return unavailable(
            "The registered source directory is unavailable. Saved evidence remains readable.",
        );
    };
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || root.canonicalize().ok().as_deref() != Some(root)
    {
        return unavailable("The registered source directory moved or changed identity. Saved evidence remains readable.");
    }
    let current = match crate::source::capture_registered_source(root) {
        Ok(source) => source,
        Err(_) => return unavailable("Current source could not be captured within the supported file, byte, path and ignore-rule limits."),
    };
    // Ignore files can exclude themselves, contain NULs or credential signatures,
    // or otherwise be skipped after rules were read. Without their saved bytes
    // a policy-equivalence claim is unsafe, regardless of the exclusion reason.
    if [&current, saved]
        .iter()
        .any(|source| source.diagnostics.iter().any(|d| is_ignore_file(&d.path)))
    {
        return unavailable("An ignore file was excluded from captured evidence; its capture policy cannot be compared reliably.");
    }
    let before: BTreeMap<_, _> = saved.files.iter().map(|f| (&f.path, &f.content)).collect();
    let after: BTreeMap<_, _> = current
        .files
        .iter()
        .map(|f| (&f.path, &f.content))
        .collect();
    let added: Vec<_> = after
        .keys()
        .filter(|p| !before.contains_key(*p))
        .map(|p| (*p).clone())
        .collect();
    let removed: Vec<_> = before
        .keys()
        .filter(|p| !after.contains_key(*p))
        .map(|p| (*p).clone())
        .collect();
    let modified: Vec<_> = before
        .iter()
        .filter(|(p, content)| after.get(*p).is_some_and(|next| next != *content))
        .map(|(p, _)| (*p).clone())
        .collect();
    let config_changed = saved.config != current.config;
    let ignore_rules_changed = added
        .iter()
        .chain(&removed)
        .chain(&modified)
        .any(|p| is_ignore_file(p));
    let stale = config_changed || !added.is_empty() || !removed.is_empty() || !modified.is_empty();
    const EXAMPLES: usize = 20;
    json!({
        "state":if stale {"stale"} else {"current"},
        "reason":if stale {"Current admitted source or capture policy differs from this saved snapshot. Compile explicitly to record an update."}
            else {"Current admitted source and capture policy match this saved snapshot."},
        "scope":"Admitted source files, current default capture limits, and captured repository .gitignore/.covenantignore rules. Git metadata and excluded content are not freshness signals.",
        "changes":{"added":added.len(),"modified":modified.len(),"removed":removed.len(),
            "examples":{"added":added.iter().take(EXAMPLES).collect::<Vec<_>>(),
                "modified":modified.iter().take(EXAMPLES).collect::<Vec<_>>(),
                "removed":removed.iter().take(EXAMPLES).collect::<Vec<_>>()},
            "truncated":added.len()>EXAMPLES || modified.len()>EXAMPLES || removed.len()>EXAMPLES},
        "config_changed":config_changed,"ignore_rules_changed":ignore_rules_changed,
        "git_metadata_checked":false,"git_metadata_changed":null,
    })
}
