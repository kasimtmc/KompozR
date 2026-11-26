# Changes

## 0.1.0

- `.kt` files as Kotlin, with a grammar of their own (`source.kotlin.kompozr`, from the C++ Kompoz's).
- The Kotlin inside `kompoz! { … }`, `kompoz!( … )` and `kompoz![ … ]` in Rust sources highlighted as Kotlin (a
  grammar injected into Rust's), nested delimiters balanced, the Rust after the block Rust again.
- `kompoz-lsp` started for `.kt` files and Rust sources with `kompoz!`: found in the `kompozr.server.path` setting,
  `KOMPOZ_LSP`, a workspace's `target/` or on `PATH`. It runs the toolchain's rust-analyzer, unless the
  rust-analyzer extension is installed (`kompozr.rustAnalyzer.mode`); `kompozr.rustAnalyzer.path`,
  `kompozr.shadow.enabled` for the shadow-project bridge, `kompozr.server.extraArgs`, `kompozr.trace.server`, and a
  command to restart it.
