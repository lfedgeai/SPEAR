# wasm-js samples

These samples are JS-first: you write JavaScript (e.g. `src/entry.mjs`), and it runs in SPEAR as a WASM executable.

Under the hood, a small Rust “Boa JS runner” is compiled to WASM (`wasm32-wasip1`) and embeds/loads the JS entry.

So the focus here is JS, even though the runner itself is written in Rust.
