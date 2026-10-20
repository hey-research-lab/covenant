use covenant_knowledge::*;
use serde_json::json;
use std::{fs, path::Path, process::Command};

fn file(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn fixtures(root: &Path) {
    file(
        root,
        "src/lib.rs",
        "pub mod math;\npub fn ready() -> bool { true }\npub fn run() { ready(); }\n",
    );
    file(
        root,
        "src/math.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    );
    file(
        root,
        "web/main.ts",
        "import { size } from './size';\nexport function runTask(x: number) { return size(x); }\n",
    );
    file(
        root,
        "web/size.ts",
        "export const size = (n: number) => n + 1;\n",
    );
    file(
        root,
        "web/card.tsx",
        "export function Card() { return <div>Evidence</div>; }\n",
    );
    file(
        root,
        "web/plain.js",
        "export class Queue { next() { return 1; } }\n",
    );
    file(root,"worker/jobs.py","from worker.utils import scale\nclass Worker:\n    def run(self, n):\n        return scale(n)\n");
    file(root, "worker/utils.py", "def scale(n):\n    return n * 2\n");
    file(
        root,
        "README.md",
        "# Example\n\n## Source evidence\nNo runtime code executes.\n",
    );
}
fn setup() -> (tempfile::TempDir, tempfile::TempDir, Store) {
    let source = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let store = Store::open(home.path()).unwrap();
    (source, home, store)
}
fn git(root: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn expression_references_exports_and_multiple_imports_are_precise() {
    let (source, _home, store) = setup();
    file(
        source.path(),
        "main.py",
        "import first as f, second\nfrom first import value\ndef run(x):\n    return x + value\n",
    );
    file(source.path(), "first.py", "value = 1\n");
    file(source.path(), "second.py", "value = 2\n");
    file(
        source.path(),
        "main.ts",
        "export function outer() { function inner() { return 1; } return inner(); }\n",
    );
    file(
        source.path(),
        "lib.rs",
        "pub fn outer() { fn inner() {} inner(); }\n",
    );
    let r = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let snapshot = store
        .snapshot(r["project"]["id"].as_str().unwrap(), None)
        .unwrap();
    for target in ["first", "second"] {
        assert!(snapshot.relationships.iter().any(|r| r.path == "main.py"
            && r.target == target
            && r.kind == "imports"
            && r.resolution == "resolved"));
    }
    for path in ["main.ts", "lib.rs"] {
        assert!(snapshot
            .objects
            .iter()
            .any(|o| o.path == path && o.name == "outer" && o.exported));
        assert!(snapshot
            .objects
            .iter()
            .any(|o| o.path == path && o.name == "inner" && o.kind == "function" && !o.exported));
    }
    assert!(snapshot.relationships.iter().any(|r| r.path == "main.py"
        && r.target == "value"
        && r.kind == "references"
        && r.resolution == "unresolved"));
    validate_snapshot(&snapshot).unwrap();
}

#[test]
fn markdown_sections_include_body_and_respect_fence_markers() {
    let (source, _home, store) = setup();
    file(source.path(), "README.md", "# Guide\nOpening text\n## Evidence\nCaptured bytes\n````rust\n```\n# Not a heading\n````\n## Next\nEnd\n");
    let r = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let project = r["project"]["id"].as_str().unwrap();
    let snapshot = store.snapshot(project, None).unwrap();
    assert!(!snapshot.objects.iter().any(|o| o.name == "Not a heading"));
    let section = snapshot
        .objects
        .iter()
        .find(|o| o.name == "Evidence")
        .unwrap();
    let evidence = store
        .query(project, None, "evidence", &json!({"id":section.id}))
        .unwrap();
    let fragment = evidence["evidence"]["fragment"].as_str().unwrap();
    assert!(fragment.contains("Captured bytes"));
    assert!(!fragment.contains("## Next"));
    assert_eq!(section.start_line, 3);
    assert_eq!(section.end_line, 8);
}

#[test]
fn committed_capture_refuses_subdirectory_scope_expansion() {
    let (source, _home, store) = setup();
    file(source.path(), "inside/main.py", "def local(): pass\n");
    file(source.path(), "outside.py", "def unrelated(): pass\n");
    git(source.path(), &["init", "-q"]);
    git(source.path(), &["add", "."]);
    git(
        source.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    let output = Command::new("git")
        .arg("-C")
        .arg(source.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let sha = String::from_utf8(output.stdout).unwrap().trim().to_string();
    assert!(store
        .compile(
            &source.path().join("inside"),
            CompileOptions {
                force_full: false,
                commit: Some(sha)
            }
        )
        .unwrap_err()
        .to_string()
        .contains("repository root"));
    assert!(store.projects().unwrap().is_empty());
}

#[test]
fn storage_upgrade_preserves_snapshots_and_failed_commit_is_atomic() {
    let (source, home, store) = setup();
    file(source.path(), "main.py", "def before(): pass\n");
    let initial = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let project = initial["project"]["id"].as_str().unwrap();
    drop(store);
    let db = rusqlite::Connection::open(home.path().join("knowledge.sqlite3")).unwrap();
    db.execute_batch("ALTER TABLE parse_cache DROP COLUMN checksum; DROP TABLE relationship_index; PRAGMA user_version=1;")
        .unwrap();
    let store = Store::open(home.path()).unwrap();
    assert_eq!(
        store.snapshot(project, None).unwrap().id,
        initial["snapshot_id"]
    );
    assert_eq!(
        store
            .compile(source.path(), CompileOptions::default())
            .unwrap()["snapshot_id"],
        initial["snapshot_id"]
    );
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
    db.execute_batch("CREATE TRIGGER test_fail_commit BEFORE UPDATE ON projects BEGIN SELECT RAISE(ABORT,'simulated interrupted commit'); END;").unwrap();
    file(source.path(), "main.py", "def after(): pass\n");
    assert!(store
        .compile(source.path(), CompileOptions::default())
        .is_err());
    drop(store);
    db.execute_batch("DROP TRIGGER test_fail_commit;").unwrap();
    let store = Store::open(home.path()).unwrap();
    assert_eq!(
        store.snapshot(project, None).unwrap().id,
        initial["snapshot_id"]
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM snapshots", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_ne!(
        store
            .compile(source.path(), CompileOptions::default())
            .unwrap()["snapshot_id"],
        initial["snapshot_id"]
    );
}

#[test]
fn all_languages_have_resolvable_exact_source_evidence() {
    let (source, _home, store) = setup();
    fixtures(source.path());
    let report = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let project = report["project"]["id"].as_str().unwrap();
    let snapshot = store.snapshot(project, None).unwrap();
    for (name, lang) in [
        ("ready", "rust"),
        ("runTask", "typescript"),
        ("Card", "tsx"),
        ("Queue", "javascript"),
        ("scale", "python"),
        ("Source evidence", "markdown"),
    ] {
        let o = snapshot
            .objects
            .iter()
            .find(|o| o.name == name && o.language == lang)
            .unwrap();
        let e = store
            .query(project, None, "evidence", &json!({"id":o.id}))
            .unwrap();
        let f = snapshot
            .source
            .files
            .iter()
            .find(|f| f.path == o.path)
            .unwrap();
        assert_eq!(
            e["evidence"]["fragment"],
            &f.content[o.start_byte..o.end_byte]
        );
        assert_eq!(e["evidence"]["source_sha256"], sha256(f.content.as_bytes()));
        assert!(o.start_line > 0);
    }
    assert!(snapshot
        .relationships
        .iter()
        .any(|r| r.kind == "imports" && r.path == "web/main.ts" && r.resolution == "resolved"));
    assert!(snapshot
        .relationships
        .iter()
        .any(|r| r.kind == "imports" && r.path == "worker/jobs.py" && r.resolution == "resolved"));
    assert!(snapshot
        .relationships
        .iter()
        .any(|r| r.kind == "imports" && r.path == "src/lib.rs" && r.resolution == "resolved"));
    let search = store
        .query(project, None, "search", &json!({"query":"runTask"}))
        .unwrap();
    assert!(search["total"].as_u64().unwrap() > 0);
    validate_snapshot(&snapshot).unwrap();
}

#[test]
fn same_source_and_incremental_rebuild_are_canonical() {
    let (source, _home, store) = setup();
    fixtures(source.path());
    let a = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let b = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    assert_eq!(a["snapshot_id"], b["snapshot_id"]);
    assert_eq!(b["reused_files"], 9);
    file(
        source.path(),
        "worker/utils.py",
        "def scale(n):\n    return n * 3\n",
    );
    let inc = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    assert_eq!(inc["reused_files"], 8);
    let full = store
        .compile(
            source.path(),
            CompileOptions {
                force_full: true,
                commit: None,
            },
        )
        .unwrap();
    assert_eq!(inc["snapshot_id"], full["snapshot_id"]);
    assert_eq!(full["reused_files"], 0);
}

#[test]
fn revisiting_a_snapshot_orders_history_by_last_capture() {
    let (source, _home, store) = setup();
    file(source.path(), "main.py", "def first(): pass\n");
    let first = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    file(source.path(), "main.py", "def second(): pass\n");
    store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    file(source.path(), "main.py", "def first(): pass\n");
    let revisit = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    assert_eq!(first["snapshot_id"], revisit["snapshot_id"]);
    let status = store
        .query(
            first["project"]["id"].as_str().unwrap(),
            None,
            "status",
            &json!({}),
        )
        .unwrap();
    assert_eq!(status["snapshots"][0]["id"], first["snapshot_id"]);
    assert_eq!(status["snapshot_id"], first["snapshot_id"]);
    assert_eq!(status["snapshots"].as_array().unwrap().len(), 2);
}

#[test]
fn deleted_and_renamed_files_disappear_only_from_new_snapshots() {
    let (source, _home, store) = setup();
    fixtures(source.path());
    let a = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    fs::remove_file(source.path().join("web/plain.js")).unwrap();
    fs::rename(
        source.path().join("web/size.ts"),
        source.path().join("web/length.ts"),
    )
    .unwrap();
    let b = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let p = a["project"]["id"].as_str().unwrap();
    assert_eq!(
        store
            .query(p, None, "search", &json!({"query":"Queue"}))
            .unwrap()["total"],
        0
    );
    assert!(
        store
            .query(
                p,
                a["snapshot_id"].as_str(),
                "search",
                &json!({"query":"Queue"})
            )
            .unwrap()["total"]
            .as_u64()
            .unwrap()
            > 0
    );
    let diff = store
        .query(
            p,
            None,
            "changes",
            &json!({"from":a["snapshot_id"],"to":b["snapshot_id"]}),
        )
        .unwrap();
    assert!(diff["removed"]
        .as_array()
        .unwrap()
        .contains(&json!("web/plain.js")));
    assert!(diff["added"]
        .as_array()
        .unwrap()
        .contains(&json!("web/length.ts")));
    let now = store.snapshot(p, None).unwrap();
    assert!(now
        .relationships
        .iter()
        .any(|r| r.path == "web/main.ts" && r.kind == "imports" && r.resolution == "unresolved"));
}

#[test]
fn ignored_sensitive_linked_and_binary_files_are_reported() {
    let (source, _home, store) = setup();
    file(source.path(), ".gitignore", "ignored.py\nhidden/\n");
    file(source.path(), ".covenantignore", "local.py\n");
    for path in [
        "ignored.py",
        "hidden/no.py",
        "local.py",
        ".env",
        "key.pem",
        "node_modules/pkg/index.js",
    ] {
        file(source.path(), path, "def should_not_exist(): pass\n");
    }
    file(source.path(), "safe.py", "def useful(): pass\n");
    file(
        source.path(),
        "credential.txt",
        "-----BEGIN PRIVATE KEY-----\nsecret\n",
    );
    fs::write(source.path().join("bytes.bin"), [0, 255]).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/passwd", source.path().join("escaped.py")).unwrap();
    let r = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let snapshot = store
        .snapshot(r["project"]["id"].as_str().unwrap(), None)
        .unwrap();
    assert!(snapshot.objects.iter().any(|o| o.name == "useful"));
    assert!(!snapshot
        .objects
        .iter()
        .any(|o| o.name == "should_not_exist"));
    assert!(!snapshot
        .source
        .files
        .iter()
        .any(|f| f.path == "credential.txt"));
    for code in ["ignored", "excluded", "binary", "credential_detected"] {
        assert!(
            snapshot.diagnostics.iter().any(|d| d.code == code),
            "{code}"
        );
    }
}

#[test]
fn malformed_syntax_and_dynamic_calls_are_explicit() {
    let (source, _home, store) = setup();
    file(source.path(), "broken.rs", "pub fn broken( {\n");
    file(
        source.path(),
        "run.py",
        "def run(obj):\n    return obj.do_something()\n",
    );
    let r = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    assert_eq!(r["coverage"]["partial"], true);
    let s = store
        .snapshot(r["project"]["id"].as_str().unwrap(), None)
        .unwrap();
    assert!(s.diagnostics.iter().any(|d| d.code == "parse_error"));
    assert!(s
        .relationships
        .iter()
        .any(|r| r.kind == "calls" && r.target == "obj.do_something" && r.to.is_none()));
}

#[test]
fn local_candidate_calls_are_not_runtime_proofs() {
    let (source, _home, store) = setup();
    file(
        source.path(),
        "call.py",
        "def helper(): pass\ndef run(helper):\n    helper()\n",
    );
    let r = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let s = store
        .snapshot(r["project"]["id"].as_str().unwrap(), None)
        .unwrap();
    let edge = s.relationships.iter().find(|r| r.kind == "calls").unwrap();
    assert_eq!(edge.resolution, "candidate");
}

#[test]
fn snapshot_and_project_isolation_survive_reopen() {
    let (source, home, store) = setup();
    fixtures(source.path());
    let first = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let other = tempfile::tempdir().unwrap();
    file(other.path(), "private.py", "def private_code(): pass\n");
    let second = store
        .compile(other.path(), CompileOptions::default())
        .unwrap();
    drop(store);
    let store = Store::open(home.path()).unwrap();
    assert!(store
        .snapshot(
            first["project"]["id"].as_str().unwrap(),
            second["snapshot_id"].as_str()
        )
        .is_err());
    assert!(
        store
            .query(
                second["project"]["id"].as_str().unwrap(),
                None,
                "search",
                &json!({"query":"private_code"})
            )
            .unwrap()["total"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn index_reconstruction_and_append_only_storage() {
    let (source, home, store) = setup();
    fixtures(source.path());
    let r = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let db = rusqlite::Connection::open(home.path().join("knowledge.sqlite3")).unwrap();
    assert!(db.execute("DELETE FROM snapshots", []).is_err());
    assert!(db.execute("UPDATE events SET kind='changed'", []).is_err());
    db.execute("DELETE FROM object_search", []).unwrap();
    let p = r["project"]["id"].as_str().unwrap();
    let snapshot = store.snapshot(p, None).unwrap();
    let root = snapshot
        .objects
        .iter()
        .find(|o| o.name == "runTask" && o.kind == "function")
        .unwrap();
    let before = store
        .query(p, None, "dependencies", &json!({"id":root.id}))
        .unwrap();
    db.execute("DELETE FROM relationship_index", []).unwrap();
    assert!(store
        .query(p, None, "dependencies", &json!({"id":root.id}))
        .unwrap_err()
        .to_string()
        .contains("index is incomplete"));
    store.rebuild_indexes(p, None).unwrap();
    assert_eq!(
        before,
        store
            .query(p, None, "dependencies", &json!({"id":root.id}))
            .unwrap()
    );
    assert!(
        store
            .query(p, None, "search", &json!({"query":"runTask"}))
            .unwrap()["total"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn import_rejects_hash_consistent_fabricated_facts() {
    let (source, _home, store) = setup();
    fixtures(source.path());
    let source_set = capture_source(source.path(), &CompileOptions::default()).unwrap();
    let mut snapshot = compile_source(source_set).unwrap();
    snapshot.objects[0].name = "invented".into();
    snapshot.id = snapshot_digest(&snapshot).unwrap();
    assert!(validate_snapshot(&snapshot).is_err());
    assert!(store.import_snapshot(source.path(), snapshot).is_err());
}

#[test]
fn invalid_paths_duplicate_paths_and_resource_overruns_refuse() {
    let base = SourceSet {
        files: vec![],
        git: GitMetadata::default(),
        diagnostics: vec![],
        config: CompileConfig::default(),
    };
    for path in ["../oops.py", "/etc/passwd", "a/../../x", ".env", "a\\x"] {
        let mut s = base.clone();
        s.files.push(SourceFile {
            path: path.into(),
            content: "hi".into(),
        });
        assert!(compile_source(s).is_err(), "{path}");
    }
    let mut s = base.clone();
    s.files = vec![
        SourceFile {
            path: "x.py".into(),
            content: "a".into()
        };
        2
    ];
    assert!(compile_source(s).is_err());
    let mut s = base;
    s.config.max_file_bytes = 1;
    s.files.push(SourceFile {
        path: "x.py".into(),
        content: "abc".into(),
    });
    assert!(compile_source(s).is_err());
}

#[test]
fn committed_and_dirty_bytes_have_distinct_citations() {
    let (source, _home, store) = setup();
    git(source.path(), &["init", "-q"]);
    file(source.path(), "main.py", "def committed(): pass\n");
    git(source.path(), &["add", "."]);
    git(
        source.path(),
        &[
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "user.name=Fixture",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    let clean = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let p = clean["project"]["id"].as_str().unwrap();
    let revision = store
        .snapshot(p, None)
        .unwrap()
        .source
        .git
        .revision
        .unwrap();
    file(source.path(), "main.py", "def changed(): pass\n");
    let dirty = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    assert_ne!(dirty["snapshot_id"], clean["snapshot_id"]);
    let snapshot = store.snapshot(p, None).unwrap();
    assert!(snapshot.source.git.dirty);
    assert!(snapshot.objects.iter().any(|o| o.name == "changed"));
    let pinned = store
        .compile(
            source.path(),
            CompileOptions {
                force_full: false,
                commit: Some(revision),
            },
        )
        .unwrap();
    assert_eq!(
        store
            .snapshot(p, pinned["snapshot_id"].as_str())
            .unwrap()
            .source
            .git
            .dirty,
        false
    );
    assert_eq!(
        fs::read_to_string(source.path().join("main.py")).unwrap(),
        "def changed(): pass\n"
    );
}

#[test]
fn unsupported_database_version_is_not_reset() {
    let dir = tempfile::tempdir().unwrap();
    let db = rusqlite::Connection::open(dir.path().join("knowledge.sqlite3")).unwrap();
    db.execute_batch("PRAGMA user_version=99; CREATE TABLE sentinel(value TEXT); INSERT INTO sentinel VALUES('preserve');").unwrap();
    assert!(Store::open(dir.path()).is_err());
    assert_eq!(
        db.query_row("SELECT value FROM sentinel", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "preserve"
    );
}

#[test]
fn failed_capture_does_not_publish_partial_snapshot() {
    let (source, home, store) = setup();
    file(source.path(), "ok.py", "def ok(): pass\n");
    let r = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    file(
        source.path(),
        ".covenantignore",
        &"#".repeat(1024 * 1024 + 1),
    );
    assert!(store
        .compile(source.path(), CompileOptions::default())
        .is_err());
    drop(store);
    let store = Store::open(home.path()).unwrap();
    assert_eq!(
        store
            .resolve_project(r["project"]["id"].as_str(), source.path())
            .unwrap()
            .latest
            .as_deref(),
        r["snapshot_id"].as_str()
    );
}

#[test]
fn corrupted_valid_json_cache_falls_back_to_source() {
    let (source, home, store) = setup();
    file(source.path(), "main.py", "def reliable(): pass\n");
    let a = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let db = rusqlite::Connection::open(home.path().join("knowledge.sqlite3")).unwrap();
    let (key, artifact): (String, String) = db
        .query_row("SELECT key,artifact FROM parse_cache LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&artifact).unwrap();
    value["objects"][0]["name"] = json!("invented");
    db.execute(
        "UPDATE parse_cache SET artifact=?1 WHERE key=?2",
        rusqlite::params![serde_json::to_string(&value).unwrap(), key],
    )
    .unwrap();
    let b = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    assert_eq!(a["snapshot_id"], b["snapshot_id"]);
    assert_eq!(b["reused_files"], 0);
}

#[test]
fn missing_rust_import_is_not_resolved_to_its_parent() {
    let (source, _home, store) = setup();
    file(
        source.path(),
        "src/foo.rs",
        "use self::missing;\npub fn hello() {}\n",
    );
    let report = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let snapshot = store
        .snapshot(report["project"]["id"].as_str().unwrap(), None)
        .unwrap();
    let import = snapshot
        .relationships
        .iter()
        .find(|r| r.kind == "imports")
        .unwrap();
    assert!(import.to.is_none());
    assert_eq!(import.resolution, "unresolved");
}

#[cfg(unix)]
#[test]
fn raw_git_source_status_never_runs_repository_filters() {
    let root = tempfile::tempdir().unwrap();
    let probe = tempfile::tempdir().unwrap();
    let marker = probe.path().join("FILTER_EXECUTED");
    let script = probe.path().join("filter.py");
    let marker_literal = serde_json::to_string(marker.to_str().unwrap()).unwrap();
    fs::write(&script, format!("import pathlib,sys\npathlib.Path({marker_literal}).write_text('ran')\nsys.stdout.buffer.write(sys.stdin.buffer.read())\n")).unwrap();
    git(root.path(), &["init", "-q"]);
    file(
        root.path(),
        ".gitattributes",
        "*.py filter=knowledge_probe\n",
    );
    file(root.path(), "main.py", "def original(): pass\n");
    git(root.path(), &["add", "."]);
    git(
        root.path(),
        &[
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "user.name=Fixture",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    let command = format!(
        "python3 '{}'",
        script.to_str().unwrap().replace('\'', "'\\''")
    );
    git(
        root.path(),
        &["config", "filter.knowledge_probe.clean", &command],
    );
    git(
        root.path(),
        &["config", "filter.knowledge_probe.process", &command],
    );
    git(
        root.path(),
        &["config", "filter.knowledge_probe.required", "true"],
    );
    git(root.path(), &["config", "core.fsmonitor", &command]);
    let clean = capture_source(root.path(), &CompileOptions::default()).unwrap();
    assert!(!clean.git.dirty);
    assert!(clean
        .diagnostics
        .iter()
        .any(|d| d.code == "git_source_scope"));
    // Equal lengths force Git status to read/filter bytes, exposing the original bug.
    file(root.path(), "main.py", "def modified(): pass\n");
    let changed = capture_source(root.path(), &CompileOptions::default()).unwrap();
    assert!(changed.git.dirty);
    assert!(changed
        .files
        .iter()
        .any(|f| f.content == "def modified(): pass\n"));
    fs::remove_file(root.path().join("main.py")).unwrap();
    assert!(
        capture_source(root.path(), &CompileOptions::default())
            .unwrap()
            .git
            .dirty
    );
    file(root.path(), "main.py", "def original(): pass\n");
    file(root.path(), "added.py", "def added(): pass\n");
    assert!(
        capture_source(root.path(), &CompileOptions::default())
            .unwrap()
            .git
            .dirty
    );
    assert!(
        !marker.exists(),
        "source capture must never execute filter/fsmonitor programs"
    );
}

#[test]
fn raw_git_blob_comparison_supports_sha256_and_subdirectory_scope() {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "-q", "--object-format=sha256"]);
    file(root.path(), "nested/main.py", "def original(): pass\n");
    file(root.path(), "outside.py", "def outside(): pass\n");
    git(root.path(), &["add", "."]);
    git(
        root.path(),
        &[
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "user.name=Fixture",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    let source = capture_source(&root.path().join("nested"), &CompileOptions::default()).unwrap();
    assert_eq!(source.git.revision.as_deref().unwrap().len(), 64);
    assert_eq!(source.files.len(), 1);
    assert_eq!(source.files[0].path, "main.py");
    assert!(!source.git.dirty);
    file(root.path(), "outside.py", "def changed(): pass\n");
    assert!(
        !capture_source(&root.path().join("nested"), &CompileOptions::default())
            .unwrap()
            .git
            .dirty,
        "source metadata compares only the selected directory"
    );
}
