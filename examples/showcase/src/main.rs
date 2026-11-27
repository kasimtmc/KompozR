//! KompozR's showcase (the C++ Kompoz showcase's shell and Material screen, in Kotlin's syntax: see lib.rs), run on
//! the platform's window (KOMPOZ_HEADLESS=1: without one), 440 × 900 dp as the C++ one.
//!
//! ```text
//! cargo run -p showcase
//! KOMPOZ_HEADLESS=1 KOMPOZ_SCRIPT="wait 10;click 385 830;wait 60;shot shot.png;quit" cargo run -p showcase
//! ```

use kompoz::prelude::*;
use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    web_fonts();
    App::new("Kompoz Showcase").size(440.dp(), 900.dp()).run(showcase::ShowcaseApp)
}

/// On the web the showcase brings its fonts (fonts/: the C++ showcase's, which its shots are compared in; natively
/// fontconfig's configuration of that directory), sans-serif Noto Sans and monospace Noto Sans Mono, as that
/// configuration names them: the web platform's bundled fonts then aren't added.
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
fn web_fonts() {
    use kompoz::text::font::{FontFamilyResolverImpl, GenericFontFamily};
    use std::sync::Arc;
    let collection = FontFamilyResolverImpl::system().collection().clone();
    let fonts: [&'static [u8]; 7] = [
        include_bytes!("../fonts/NotoSans-Regular.ttf"),
        include_bytes!("../fonts/NotoSans-Medium.ttf"),
        include_bytes!("../fonts/NotoSans-Bold.ttf"),
        include_bytes!("../fonts/NotoSansMono-Regular.ttf"),
        include_bytes!("../fonts/NotoSansArabic-Regular.ttf"),
        include_bytes!("../fonts/NotoSansSymbols-Regular.ttf"),
        include_bytes!("../fonts/NotoSansSymbols2-Regular.ttf"),
    ];
    for bytes in fonts {
        collection.register(Arc::new(bytes));
    }
    for generic in [GenericFontFamily::SansSerif, GenericFontFamily::Serif, GenericFontFamily::Cursive] {
        collection.set_generic_families(generic, &["Noto Sans"]);
    }
    collection.set_generic_families(GenericFontFamily::Monospace, &["Noto Sans Mono"]);
}
