use std::io::{Read, Write};

fn main() -> anyhow::Result<()> {
    let mut input = Vec::new();
    std::io::stdin()
        .take(70 * 1024 * 1024)
        .read_to_end(&mut input)?;
    anyhow::ensure!(
        input.len() < 70 * 1024 * 1024,
        "compiler input exceeds limit"
    );
    let source: covenant_knowledge::SourceSet = serde_json::from_slice(&input)?;
    let snapshot = covenant_knowledge::compile_source(source)?;
    serde_json::to_writer(std::io::stdout().lock(), &snapshot)?;
    std::io::stdout().write_all(b"\n")?;
    Ok(())
}
