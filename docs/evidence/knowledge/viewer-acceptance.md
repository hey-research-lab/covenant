# Local knowledge viewer acceptance

The actual `covenant-compute knowledge serve` binary was exercised on 20 October
2026 against two compiled snapshots of a copy of `examples/knowledge`. The only
fixture edit was the Python example's unit price changing from 12 to 14. No data
was mocked and no inference, wallet, network purchase, or production deployment
was involved.

The browser check covered the current snapshot/coverage view, snapshot integrity
verification, source search, opening the stored source evidence, navigating its
dependency trace, and comparing the two recorded snapshots. The trace exposed
unresolved relationships alongside resolved ones. The snapshot comparison found
the changed Python source file.

All four views were exercised at 1440, 768, and 390 CSS pixels. The run recorded
no JavaScript page errors and no horizontal page overflow. Desktop overview and
source inspection, and mobile source inspection, dependencies, and changes were
also inspected visually for clipping, overlaps, and readable source metadata.
The viewer uses the official Covenant mark and the actual Archivo and JetBrains
Mono fonts, with their license notices embedded in the binary.

Local browser evidence is retained under `output/compute-knowledge-qa/`:
`browser-qa.cjs`, `browser-qa.json`, `canonical-before.json`,
`canonical-after.json`, and the desktop/390px/768px PNGs. These are local QA
artifacts, not public availability evidence. Parser changes can alter later
object counts and snapshot IDs; the recorded runs describe their exact fixture
snapshots only.

The released-entrypoint integration test exercises real loopback HTTP requests:
unknown session paths, foreign Host and Origin, forwarding headers, cross-site
fetch metadata, mutation operations, cross-project selectors, and oversized
request bodies are refused. MCP checks cover all six read-only tools,
notifications, project isolation, mutation rejection and bounded input frames.

The separate public Next proxy check executes the actual route with side-effect
sentinels. It passed 88 cases across GET/POST/PUT/PATCH/DELETE, demo mode on/off,
encoded knowledge paths and knowledge tool names, with no operator-token reads
or upstream requests:

```sh
node agent-os/covenant-web/scripts/check-knowledge-proxy.mjs
```

To repeat the Rust boundary checks and launch a new local session:

```sh
cd agent-os
cargo test -p covenant-compute-buyer --test knowledge_cli
./target/debug/covenant-compute knowledge compile ../examples/knowledge --json
# Use the returned project ID. A fresh private session URL is printed.
./target/debug/covenant-compute knowledge serve --project PROJECT_ID
```
