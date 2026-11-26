//! `kompoz-lsp`: KompozR's language server on stdin/stdout (see the library's doc), or `--kate` to install Kate's
//! support.
//!
//! ```text
//! kompoz-lsp [--lsp] [--rust-analyzer PATH | --no-rust-analyzer] [--shadow-rust-analyzer PATH | --no-shadow]
//!            [rust-analyzer's arguments…]
//! kompoz-lsp --kate
//! ```

use kompoz_lsp::server::{self, Config};

const USAGE: &str = "\
usage: kompoz-lsp [--lsp] [--rust-analyzer PATH | --no-rust-analyzer] [--shadow-rust-analyzer PATH | --no-shadow]
                  [rust-analyzer's arguments...]
       kompoz-lsp --kate

  (no option), --lsp       the language server, on stdin/stdout: the Kotlin of kompoz! blocks and .kt files its own,
                           the Rust rust-analyzer's (the toolchain's, started when first needed)
  --rust-analyzer PATH     the rust-analyzer to run (default: rust-analyzer on PATH)
  --no-rust-analyzer       none: the Kotlin's answers only
  --shadow-rust-analyzer PATH
                           the rust-analyzer of the shadow project (rustc's errors on the Kotlin's lines, hovers of
                           the Rust it becomes; default: --rust-analyzer's, else rust-analyzer on PATH), started when
                           a package using KompozR has Kotlin open
  --no-shadow              no shadow project
  --kate                   Kate's highlighting for Kompoz Rust and this server in its LSP client's settings
  --version, --help
";

fn main() {
    let mut config = Config::default();
    let (mut shadow, mut shadow_path): (bool, Option<std::path::PathBuf>) = (true, None);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--kate" => {
                let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("kompoz-lsp"));
                match kompoz_lsp::kate::install(&exe) {
                    Ok(done) => {
                        print!("{done}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("kompoz-lsp: {e}");
                        std::process::exit(1);
                    }
                }
            }
            "--lsp" | "--stdio" => {}
            "--no-rust-analyzer" => config.rust_analyzer = None,
            "--no-shadow" => shadow = false,
            "--shadow-rust-analyzer" => match args.next() {
                Some(p) => shadow_path = Some(p.into()),
                None => {
                    eprint!("kompoz-lsp: --shadow-rust-analyzer takes a path\n{USAGE}");
                    std::process::exit(2);
                }
            },
            "--rust-analyzer" => match args.next() {
                Some(p) => config.rust_analyzer = Some(p.into()),
                None => {
                    eprint!("kompoz-lsp: --rust-analyzer takes a path\n{USAGE}");
                    std::process::exit(2);
                }
            },
            "--version" => {
                println!("kompoz-lsp {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--help" | "-h" => {
                print!("{USAGE}");
                return;
            }
            _ => {
                if let Some(p) = a.strip_prefix("--rust-analyzer=") {
                    config.rust_analyzer = Some(p.into());
                } else if let Some(p) = a.strip_prefix("--shadow-rust-analyzer=") {
                    shadow_path = Some(p.into());
                } else {
                    // (rust-analyzer's own, passed on)
                    config.analyzer_args.push(a);
                }
            }
        }
    }
    if shadow {
        config.shadow =
            Some(shadow_path.or_else(|| config.rust_analyzer.clone()).unwrap_or_else(|| "rust-analyzer".into()));
    }
    std::process::exit(server::run(config));
}
