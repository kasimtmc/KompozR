# KompozR for RustRover and CLion

For [KompozR](https://github.com/kasimtmc/kompozr)'s UI code, written in Kotlin's syntax: inside `kompoz! { … }`
blocks of Rust sources, and in whole `.kt` files read by `kompoz_file!("screen.kt")`.

- **kompoz-lsp**, KompozR's language server, is started for `.kt` files and for Rust sources with `kompoz!` (or
  `kompoz_file!`) in them, as `kompoz-lsp --lsp --no-rust-analyzer`: the IDE's own Rust support keeps the Rust, the
  server the Kotlin (the generator's errors on the Kotlin's lines, and rustc's through its shadow-project bridge;
  hovers, completion, going to definitions). The IDE asks it for its colors (semantic tokens) in these files, the
  Kotlin's.
- **`.kt` files** are highlighted with the plugin's Kotlin grammar, a TextMate bundle (the VS Code extension's
  grammar, `editors/vscode-kompozr/syntaxes/kotlin.tmLanguage.json`): RustRover and CLion have no Kotlin of their
  own. The VS Code extension's injection for `kompoz!` has no counterpart here: a `.rs` file is the Rust plugin's,
  highlighted by its own lexer, not TextMate, so the Kotlin in a `kompoz!` block takes its colors from the server's
  semantic tokens.

## Setting up

Build `kompoz-lsp` (`cargo install --path crates/kompoz-lsp` in KompozR's repository, or `cargo build -p kompoz-lsp`
in it). The plugin finds it in this order:

1. Settings, Tools, KompozR;
2. `KOMPOZ_LSP` in the environment;
3. `target/debug/kompoz-lsp` or `target/release/kompoz-lsp` in the project, or in `CARGO_TARGET_DIR` (the newest);
4. `kompoz-lsp` on `PATH`, else in `~/.cargo/bin` (an IDE started from a desktop may not have it on `PATH`).

Settings, Tools, KompozR also has the server's extra arguments, and the shadow-project bridge: on or off
(`--no-shadow`), and its rust-analyzer (`--shadow-rust-analyzer`; empty: `rust-analyzer` on `PATH`,
`rustup component add rust-analyzer`). Applied, they restart the server.

## Building

Gradle needs a JDK (the IDE's own, its `jbr/`, will do) and an IDE to build against: a local one (no download) with
`-PidePath`, else RustRover 2026.3 is downloaded.

```sh
JAVA_HOME=/path/to/rustrover/jbr ./gradlew buildPlugin -PidePath=/path/to/rustrover
```

`build/distributions/kompozr-jetbrains-<version>.zip` is then installed from disk (Settings, Plugins). `./gradlew test`
runs the unit tests (where the server is found, its arguments); `./gradlew verifyPlugin` runs JetBrains' Plugin
Verifier against the IDE built with (or RustRover and CLion 2026.3 downloaded).
