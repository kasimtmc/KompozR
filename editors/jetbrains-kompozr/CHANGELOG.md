# Changes

## 0.1.0

- `kompoz-lsp`, KompozR's language server, for `.kt` files and Rust sources with `kompoz!` (or `kompoz_file!`) in
  them, in RustRover and CLion (2026.3 and later): `kompoz-lsp --lsp --no-rust-analyzer`, the IDE's own Rust support
  keeping the Rust; its colors (semantic tokens) for the Kotlin asked for in these files.
- `kompoz-lsp` found in Settings, Tools, KompozR, in `KOMPOZ_LSP`, built in the project's `target/` (or
  `CARGO_TARGET_DIR`), on `PATH` or in `~/.cargo/bin`; extra arguments, and the shadow-project bridge turned off
  (`--no-shadow`) or given its rust-analyzer (`--shadow-rust-analyzer`) there. Applied, the settings restart it.
- `.kt` files highlighted with the plugin's Kotlin grammar (a TextMate bundle: the VS Code extension's grammar).
