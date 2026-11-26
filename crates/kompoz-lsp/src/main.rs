//! `kompoz-lsp`: KompozR's language server on stdin/stdout (see the library's doc), or `--kate` to install Kate's
//! support.
//!
//! ```text
//! kompoz-lsp [--lsp] [--rust-analyzer PATH | --no-rust-analyzer] [rust-analyzer's arguments…]
//! kompoz-lsp --kate
//! ```

use kompoz_lsp::server::{self, Config};

const USAGE: &str = "\
usage: kompoz-lsp [--lsp] [--rust-analyzer PATH | --no-rust-analyzer] [rust-analyzer's arguments...]
       kompoz-lsp --kate

  (no option), --lsp       the language server, on stdin/stdout: the Kotlin of kompoz! blocks and .kt files its own,
                           the Rust rust-analyzer's (the toolchain's, started when first needed)
  --rust-analyzer PATH     the rust-analyzer to run (default: rust-analyzer on PATH)
  --no-rust-analyzer       none: the Kotlin's answers only
  --kate                   Kate's highlighting for Kompoz Rust and this server in its LSP client's settings
  --version, --help
";

fn main() {
    let mut config = Config::default();
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
            _ => match a.strip_prefix("--rust-analyzer=") {
                Some(p) => config.rust_analyzer = Some(p.into()),
                // (rust-analyzer's own, passed on)
                None => config.analyzer_args.push(a),
            },
        }
    }
    std::process::exit(server::run(config));
}
