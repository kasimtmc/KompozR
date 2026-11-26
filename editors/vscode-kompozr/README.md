# KompozR for VS Code

For [KompozR](https://github.com/kasimtmc/kompozr)'s UI code, written in Kotlin's syntax: inside `kompoz! { … }`
blocks of Rust sources, and in whole `.kt` files read by `kompoz_file!("screen.kt")`.

- **Highlighting**: `.kt` files are Kotlin, with a grammar of their own (`source.kotlin.kompozr`). In Rust sources
  the Kotlin inside `kompoz! { … }`, `kompoz!( … )` and `kompoz![ … ]` is highlighted as Kotlin (a grammar injected
  into VS Code's Rust one), up to the delimiter that closes the macro's; the Rust after it is Rust again.
  `kompoz_file!` is an ordinary macro call.
- **kompoz-lsp**, KompozR's language server: the generator's errors on the Kotlin's own lines, and rustc's through
  its shadow-project bridge (a second rust-analyzer over the Rust the Kotlin translates to, for type errors and
  type-accurate hovers); hovers, completion and going to definitions in the Kotlin. It stands in front of the
  toolchain's rust-analyzer, which answers for the Rust around the Kotlin: no rust-analyzer extension is needed.

## Setting up

Install the toolchain's rust-analyzer (`rustup component add rust-analyzer`) and build `kompoz-lsp`
(`cargo install --path crates/kompoz-lsp` in KompozR's repository, or `cargo build -p kompoz-lsp` in it). The
extension finds `kompoz-lsp` in this order:

1. the `kompozr.server.path` setting;
2. `KOMPOZ_LSP` in the environment;
3. `target/debug/kompoz-lsp` or `target/release/kompoz-lsp` in a workspace folder, or in `CARGO_TARGET_DIR` (the
   newest);
4. `kompoz-lsp` on `PATH`.

The server starts when a `.kt` file or a Rust source with `kompoz!` (or `kompoz_file!`) in it is opened, or any Rust
source while it is the one in front of rust-analyzer. **KompozR: Restart kompoz-lsp** starts it again (after a
rebuild of `kompoz-lsp`, say).

## Settings

| Setting | Default | |
| --- | --- | --- |
| `kompozr.server.path` | `""` | The `kompoz-lsp` to run (see above). |
| `kompozr.server.extraArgs` | `[]` | Arguments after `kompoz-lsp`'s own: rust-analyzer's, passed on to it. |
| `kompozr.rustAnalyzer.path` | `""` | The rust-analyzer `kompoz-lsp` runs (`--rust-analyzer`), and its shadow bridge (`--shadow-rust-analyzer`). Empty: `rust-analyzer` on `PATH`. |
| `kompozr.rustAnalyzer.mode` | `auto` | `auto`: `kompoz-lsp` runs rust-analyzer unless the rust-analyzer extension (`rust-lang.rust-analyzer`) is installed, which then has the Rust to itself (`--no-rust-analyzer`). `kompozr`: always (turn the extension off then). `off`: never. |
| `kompozr.shadow.enabled` | `true` | The shadow-project bridge; off: `--no-shadow`. |
| `kompozr.trace.server` | `off` | The messages between VS Code and the server, in the KompozR output channel. |

With the rust-analyzer extension installed and no rust-analyzer set or on `PATH`, the shadow bridge runs the
extension's own server.

## With other extensions

- **A Kotlin extension** also claims `.kt` files as `kotlin`; VS Code then uses one of the two grammars, and both
  language servers answer. KompozR's `.kt` files aren't JVM Kotlin: turn the other extension off in KompozR's
  workspaces.
- **rust-analyzer's extension**: in the `auto` mode it keeps the Rust, `kompoz-lsp` the Kotlin. Its semantic
  highlighting may cover the Kotlin's in `kompoz!` blocks; `"editor.semanticHighlighting.enabled": false` for Rust
  (`"[rust]"`) leaves them to the grammar.

## Building

```sh
npm ci --offline     # or npm ci
npm run build        # dist/extension.js (esbuild)
npm test             # unit tests (node --test) and grammar tests (vscode-tmgrammar-test)
npm run package      # kompozr-<version>.vsix (vsce)
```

`test/grammar/rust.tmLanguage.json` is VS Code's own Rust grammar (MIT, from `extensions/rust`), for the grammar
tests of the injection.
