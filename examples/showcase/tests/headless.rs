//! The showcase: each of the Material tab's sections composes (the test rule, in the showcase's theme, its title
//! there), the shell shows the Material tab when its navigation item is clicked; and run headless as its binary runs
//! (`KOMPOZ_HEADLESS=1`, a GPU device: lavapipe, the validation layers' errors failing it where
//! `KOMPOZ_GPU_VALIDATION=1` turns them on, as CI does), the Material tab shot page by page in every config matches the C++ showcase's pictures
//! (examples/showcase/reference) outside the regions reference/regions.txt lists with their reasons. The drawing
//! needs a GPU device: without one it is skipped, unless `KOMPOZ_GPU_REQUIRED=1`.

#![allow(non_snake_case)]

use kompoz::gpu::device::{self, Backend, Surface};
use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::strings::{LocalStrings, Strings};
use showcase::shots::{self, Config, Showcase};
use showcase::*;
use std::path::Path;

// A section in the showcase's theme and strings, as the Material screen shows it (by its index, MaterialScreen's
// order; the dynamic color one with the app's colors chosen).
kompoz::kompoz! {
    @Composable
    fun SectionInTheme(index: Int) {
        val source = remember { mutableStateOf(ColorSource.App) }
        MaterialTheme(colorScheme = lightColorScheme()) {
            CompositionLocalProvider(LocalStrings provides Strings.turkish()) {
                Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                    when (index) {
                        0 -> SearchSection()
                        1 -> TooltipSection()
                        2 -> DropdownSection()
                        3 -> DatePickerSection()
                        4 -> TimePickerSection()
                        5 -> InteractionSection()
                        6 -> AnchoredDraggableSection()
                        7 -> RippleSection()
                        8 -> SelectableRowsSection()
                        9 -> SemanticsSection()
                        10 -> DynamicColorSection(source)
                        11 -> InsetsSection()
                        12 -> ImeNestedScrollSection()
                        13 -> FrameRateSection()
                        14 -> TextFieldsSection()
                        15 -> AnimateItemSection()
                        16 -> ButtonsSection()
                        17 -> ChipsSection()
                        18 -> SelectionControlsSection()
                        19 -> TabsSection()
                        20 -> ListItemSection()
                        21 -> DialogsSection()
                        22 -> FabMenuSection()
                        23 -> ShortNavigationBarSection()
                        24 -> WideNavigationRailSection()
                        25 -> FloatingToolbarsSection()
                        else -> CardsSection()
                    }
                }
            }
        }
    }
}

/// Every section composes, its title there (and the dynamic color one its choices).
fn every_section_composes() {
    let titles = [
        "SearchBar · SearchBarState",
        "TooltipBox · PlainTooltip · RichTooltip · TooltipAnchorPosition",
        "ExposedDropdownMenuBox",
        "DatePicker · DateRangePicker",
        "TimePicker",
        "InteractionSource · collectIsPressedAsState · collectIsDraggedAsState",
        "anchoredDraggable · AnchoredDraggableState",
        "Ripple · indication · LocalIndication",
        "selectable · toggleable · selectableGroup · matchParentSize",
        "Erişilebilirlik: merge · clearAndSet · hide · customActions · liveRegion",
        "Dinamik renk (Material You)",
        "WindowInsets (dp: sol, üst, sağ, alt)",
        "Modifier.imeNestedScroll",
        "Modifier.preferredFrameRate",
        "Metin alanları ve odak",
        "animateItem · SwipeToDismissBox",
        "Butonlar",
        "Chip'ler",
        "Seçim kontrolleri",
        "Sekmeler",
        "ListItem ve Badge",
        "Dialog, menü ve alt sayfa",
        "FloatingActionButtonMenu",
        "ShortNavigationBar",
        "WideNavigationRail",
        "Kayan araç çubukları",
        "Kartlar ve BottomAppBar",
    ];
    for (index, title) in titles.into_iter().enumerate() {
        let rule = KompozTestRule::new(kompozable_lambda!(move || SectionInTheme(index as i32)));
        rule.on_node_with_text(title).assert_exists();
        if index == 10 {
            rule.on_node_with_text("Duvar kağıdı").assert_exists();
            rule.on_node_with_text("lightColorScheme / darkColorScheme: uygulamanın kendi renkleri").assert_exists();
        }
    }
}

/// The shell starts on its first tab (a screen that comes with stage 16); its Material item shows the Material screen.
fn the_shell_opens_the_material_tab() {
    let rule = KompozTestRule::new(showcase::ShowcaseApp);
    rule.on_node_with_text("Ana sayfa ekranı KompozR'da 16. aşamada gelir; Material sekmesi hazır.").assert_exists();
    rule.on_all_nodes_with_text("Material").on_first().perform_click();
    rule.on_node_with_text("SearchBar · SearchBarState").assert_exists();
}

/// The showcase's binary, headless, through every config's pages: no validation errors (the layers on), and the
/// pictures the C++ showcase's outside the listed regions.
fn it_draws_as_the_cpp_showcase() {
    let binary = Path::new(env!("CARGO_BIN_EXE_showcase"));
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("showcase-shots");
    let reference = shots::reference_dir();
    let regions = shots::read_regions(&reference.join("regions.txt")).expect("the regions");
    let mut comparisons = Vec::new();
    for config in Config::all() {
        let dir = out.join(config.name());
        shots::shoot(binary, Showcase::KompozR, config, &dir).unwrap_or_else(|e| panic!("{e}"));
        let cut = shots::cut(&shots::load_pages(&dir).expect("its pages"), config.scale);
        comparisons.extend(shots::compare(config, &shots::parts(&cut, config.scale), &reference, &regions, &dir));
    }
    let report = shots::report(&comparisons);
    assert!(
        shots::all_match(&comparisons),
        "the shots differ from the C++ showcase's (in {}):\n{report}",
        out.display()
    );
}

fn main() {
    kompoz::runtime::set_ui_thread();
    let mut tests: Vec<(&str, fn())> = vec![
        ("every_section_composes", every_section_composes),
        ("the_shell_opens_the_material_tab", the_shell_opens_the_material_tab),
    ];
    let required = std::env::var_os("KOMPOZ_GPU_REQUIRED").is_some_and(|v| v == "1");
    match device::create(Backend::Vulkan, Surface::default()) {
        Ok(d) => {
            println!("device: {}", d.device_name());
            tests.push(("it_draws_as_the_cpp_showcase", it_draws_as_the_cpp_showcase));
        }
        Err(why) if required => panic!("no GPU device (KOMPOZ_GPU_REQUIRED=1): {why}"),
        Err(why) => println!("skipped it_draws_as_the_cpp_showcase: {why}"),
    }
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = 0;
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        let ok = std::panic::catch_unwind(test).is_ok();
        failed += usize::from(!ok);
        println!("{} {name}", if ok { "ok    " } else { "FAILED" });
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
