# Knowledge example

An original, offline example for Covenant Compute's knowledge compiler.
The Rust, Python, JavaScript and TypeScript files demonstrate declarations,
module dependencies and source citations. The code need not be executed.

## Try it

    covenant-compute knowledge compile examples/knowledge --json
    cd examples/knowledge
    covenant-compute knowledge query calculate_total --json
    covenant-compute knowledge status --json

Use the returned project ID with `knowledge mcp --project ID` or
`knowledge serve --project ID`. The index lives outside the repository.
The source snapshot preserves the bytes captured when it was compiled.

## Limits

Static module relationships and call candidates describe source observations.
Dynamic dispatch, external package resolution and runtime behavior are not
evaluated. No model key, wallet, code execution or network service is required.
