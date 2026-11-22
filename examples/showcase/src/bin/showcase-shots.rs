//! The showcase's shots against the C++ Kompoz showcase's (see showcase::shots).
//!
//! ```text
//! showcase-shots compare [--showcase <binary>] [--out <dir>]
//!     KompozR's showcase (by default the `showcase` beside this binary) shot into <dir> (target/showcase-shots),
//!     cut into its bars and sections and compared with examples/showcase/reference: a report (also
//!     <dir>/report.txt; what differs beside it: <dir>/kompozr/<config>/<part>.diff.png), and a failure if
//!     anything differs outside the regions reference/regions.txt lists.
//! showcase-shots reference --cpp <C++ showcase binary> --revision <C++ Kompoz's commit> [--out <dir>]
//!     The C++ showcase shot, its menu bar cut off, cut into its bars and sections, into
//!     examples/showcase/reference (look at them before committing them), the revision in reference/REVISION.
//! ```

use showcase::shots::{self, Config, Showcase};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let here = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default();
    let out = arg(&args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/showcase-shots"));
    let result = match args.first().map(String::as_str) {
        Some("compare") => {
            let binary = arg(&args, "--showcase").map(PathBuf::from).unwrap_or_else(|| here.join("showcase"));
            compare(&binary, &out)
        }
        Some("reference") => match (arg(&args, "--cpp"), arg(&args, "--revision")) {
            (Some(cpp), Some(revision)) => reference(Path::new(&cpp), &revision, &out),
            _ => Err("reference needs --cpp <binary> and --revision <commit>".to_string()),
        },
        _ => Err("usage: showcase-shots compare [--showcase <binary>] [--out <dir>] | showcase-shots reference --cpp \
                  <binary> --revision <commit> [--out <dir>]"
            .to_string()),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("showcase-shots: {e}");
            ExitCode::FAILURE
        }
    }
}

fn compare(binary: &Path, out: &Path) -> Result<bool, String> {
    let shots_dir = out.join("kompozr");
    let reference = shots::reference_dir();
    let regions = shots::read_regions(&reference.join("regions.txt"))?;
    let mut comparisons = Vec::new();
    for config in Config::all() {
        let dir = shots_dir.join(config.name());
        shots::shoot(binary, Showcase::KompozR, config, &dir)?;
        let cut = shots::cut(&shots::load_pages(&dir)?, config.scale);
        let _ = cut.content.save(&dir.join("content.png"));
        let parts = shots::parts(&cut, config.scale);
        comparisons.extend(shots::compare(config, &parts, &reference, &regions, &dir));
    }
    let report = shots::report(&comparisons);
    print!("{report}");
    let _ = std::fs::write(out.join("report.txt"), &report);
    let ok = shots::all_match(&comparisons);
    println!(
        "{}",
        if ok { "the shots match the C++ showcase's (outside the listed regions)" } else { "the shots differ" }
    );
    Ok(ok)
}

fn reference(cpp: &Path, revision: &str, out: &Path) -> Result<bool, String> {
    let reference = shots::reference_dir();
    for config in Config::all() {
        let dir = out.join("cpp").join(config.name());
        shots::shoot(cpp, Showcase::Cpp, config, &dir)?;
        let pages: Vec<_> =
            shots::load_pages(&dir)?.iter().map(|p| p.without_top(shots::MENU_BAR * config.scale as i32)).collect();
        let cut = shots::cut(&pages, config.scale);
        let _ = cut.content.save(&dir.join("content.png"));
        let to = reference.join(config.name());
        let _ = std::fs::remove_dir_all(&to);
        shots::save_parts(&shots::parts(&cut, config.scale), &to)?;
    }
    let note = format!(
        "C++ Kompoz {revision}: examples/showcase built Release in a separate build directory, shot by \
         `showcase-shots reference` (its menu bar, the top {} dp, cut off; cut into the top app bar, the navigation \
         bar and the Material tab's sections).\n",
        shots::MENU_BAR
    );
    std::fs::write(reference.join("REVISION"), note).map_err(|e| e.to_string())?;
    Ok(true)
}
