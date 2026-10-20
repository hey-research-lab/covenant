//! Exercise the released offline entrypoint, stdio boundary, and real HTTP
//! viewer without a coordinator, daemon, wallet, model key, or network purchase.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_covenant-compute"));
    command.env_clear();
    command
}
fn run(args: &[&str]) -> Output {
    command().args(args).output().unwrap()
}
fn read_json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn compile(source: &Path, store: &Path) -> Value {
    read_json(run(&[
        "knowledge",
        "compile",
        source.to_str().unwrap(),
        "--store",
        store.to_str().unwrap(),
        "--json",
    ]))
}
fn fixture(root: &Path) -> std::path::PathBuf {
    let source = root.join("repository");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("engine.py"),"def total_price(quantity, price):\n    return quantity * price\n\ndef invoice(quantity):\n    return total_price(quantity, 3)\n").unwrap();
    std::fs::write(
        source.join("README.md"),
        "# Sample project\n\nAn invoice example.\n",
    )
    .unwrap();
    source
}

#[test]
fn knowledge_commands_compile_query_diff_and_verify_without_identity() {
    let root = tempfile::tempdir().unwrap();
    let source = fixture(root.path());
    let store = root.path().join("knowledge");
    let first = compile(&source, &store);
    let project = first["project"]["id"].as_str().unwrap();
    let snapshot = first["snapshot_id"].as_str().unwrap();
    let shared = [
        "--store",
        store.to_str().unwrap(),
        "--project",
        project,
        "--json",
    ];
    let call = |args: &[&str]| {
        let mut all = vec!["knowledge"];
        all.extend_from_slice(args);
        all.extend_from_slice(&shared);
        read_json(run(&all))
    };
    let search = call(&["query", "total_price"]);
    assert!(!search["results"].as_array().unwrap().is_empty());
    let item = &search["results"][0];
    let id = item["object"]["id"].as_str().unwrap();
    let detail = call(&["show", id]);
    assert_eq!(detail["evidence"]["path"], "engine.py");
    assert!(detail["evidence"]["fragment"]
        .as_str()
        .unwrap()
        .contains("total_price"));
    let status = call(&["status"]);
    assert_eq!(status["snapshot_id"], snapshot);
    let verify = call(&["verify", snapshot]);
    assert_eq!(verify["valid"], true);
    std::fs::write(
        source.join("engine.py"),
        "def total_price(quantity, price):\n    return quantity * price + 1\n",
    )
    .unwrap();
    let second = compile(&source, &store);
    let newer = second["snapshot_id"].as_str().unwrap();
    assert_ne!(snapshot, newer);
    let diff = call(&["diff", snapshot, newer]);
    assert!(diff["modified"]
        .as_array()
        .unwrap()
        .contains(&json!("engine.py")));
    assert!(!root.path().join(".covenant-compute-mcp").exists());
    assert!(!store.join("identity.json").exists());
}

#[test]
fn knowledge_help_and_invalid_syntax_do_not_need_home_or_network() {
    let help = run(&["knowledge", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("without a wallet"));
    for args in [
        vec!["knowledge", "serve"],
        vec!["knowledge", "mcp"],
        vec!["knowledge", "query"],
        vec!["knowledge", "compile"],
        vec!["knowledge", "query", "x", "--limit", "1000"],
        vec!["knowledge", "status", "--path", "/etc/passwd"],
        vec!["knowledge", "compile", ".", "--remote"],
        vec![
            "knowledge",
            "compile",
            ".",
            "--remote",
            "--repository",
            "https://github.com/example/example",
            "--compiler-image-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "--dry-run",
            "--deadline-ms",
            "999",
        ],
        vec![
            "knowledge",
            "compile",
            ".",
            "--remote",
            "--repository",
            "https://github.com/example/example",
            "--compiler-image-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "--dry-run",
            "--deadline-ms",
            "600001",
        ],
    ] {
        let out = run(&args);
        assert!(!out.status.success(), "accepted {args:?}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(!error.contains("COORDINATOR_URL"), "{error}");
        assert!(!error.contains("HOME"), "{error}");
    }
}

#[test]
fn knowledge_mcp_is_project_bound_readonly_and_bounds_frames() {
    let root = tempfile::tempdir().unwrap();
    let source = fixture(root.path());
    let store = root.path().join("knowledge");
    let compiled = compile(&source, &store);
    let project = compiled["project"]["id"].as_str().unwrap();
    let mut child = command()
        .args([
            "knowledge",
            "mcp",
            "--project",
            project,
            "--store",
            store.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"knowledge.search","arguments":{"query":"total_price"}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"knowledge.status","arguments":{"project":"other"}}}),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"knowledge.compile","arguments":{}}}),
    ];
    {
        let mut stdin = child.stdin.take().unwrap();
        for message in messages {
            writeln!(stdin, "{message}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[1]["result"]["tools"].as_array().unwrap().len(), 6);
    assert_eq!(rows[2]["result"]["isError"], false);
    assert_eq!(rows[3]["error"]["code"], -32602);
    assert_eq!(rows[4]["error"]["code"], -32602);
    let mut oversized = command()
        .args([
            "knowledge",
            "mcp",
            "--project",
            project,
            "--store",
            store.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = oversized.stdin.take().unwrap();
        let _ = stdin.write_all(&vec![b' '; 64 * 1024 + 2]);
    }
    let output = oversized.wait_with_output().unwrap();
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], -32600);
}

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[tokio::test]
async fn knowledge_viewer_requires_private_session_and_exact_origin_without_proxy_headers() {
    let root = tempfile::tempdir().unwrap();
    let source = fixture(root.path());
    let store = root.path().join("knowledge");
    let compiled = compile(&source, &store);
    let project = compiled["project"]["id"].as_str().unwrap();
    let mut child = command()
        .args([
            "knowledge",
            "serve",
            "--project",
            project,
            "--store",
            store.to_str().unwrap(),
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let _server = Server(child);
    let ready: Value = serde_json::from_str(&line).unwrap();
    let url = ready["url"].as_str().unwrap();
    assert!(url.starts_with("http://127.0.0.1:"));
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let page = http.get(url).send().await.unwrap();
    assert!(page.status().is_success());
    assert_eq!(page.headers()["cache-control"], "no-store, private");
    assert!(page.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .contains("frame-ancestors 'none'"));
    assert!(page.text().await.unwrap().contains("Covenant"));
    let api = format!("{url}api/query");
    let request = json!({"operation":"status","arguments":{}});
    assert_eq!(
        http.post(&api)
            .json(&request)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    for (name, value) in [
        ("origin", "http://evil.example"),
        ("host", "evil.example"),
        ("x-forwarded-for", "127.0.0.1"),
        ("sec-fetch-site", "cross-site"),
    ] {
        assert_eq!(
            http.post(&api)
                .header(name, value)
                .json(&request)
                .send()
                .await
                .unwrap()
                .status(),
            403,
            "accepted {name}"
        );
    }
    let wrong = url.replace("/session/", "/wrong/");
    assert_eq!(http.get(wrong).send().await.unwrap().status(), 404);
    assert_eq!(
        http.post(&api)
            .json(&json!({"operation":"status","arguments":{"project":"other"}}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        http.post(&api)
            .json(&json!({"operation":"compile","arguments":{}}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        http.post(&api)
            .header("content-type", "application/json")
            .body(" ".repeat(40 * 1024))
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
}
