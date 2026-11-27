//! The showcase: each of the Material tab's sections composes (the test rule, in the showcase's theme, its title
//! there), the shell shows the Material tab when its navigation item is clicked, the dialogs section's map button
//! opens the places screen (kept on the Material tab's back stack while another tab is shown), the C++ showcase's
//! deep links open their screens; and run headless as its binary runs
//! (`KOMPOZ_HEADLESS=1`, a GPU device: lavapipe, the validation layers' errors failing it where
//! `KOMPOZ_GPU_VALIDATION=1` turns them on, as CI does), the Material tab shot page by page (and the places screen
//! opened by its deep link) in every config matches the C++ showcase's pictures
//! (examples/showcase/reference) outside the regions reference/regions.txt lists with their reasons. The drawing
//! needs a GPU device: without one it is skipped, unless `KOMPOZ_GPU_REQUIRED=1`.

#![allow(non_snake_case)]

use kompoz::gpu::device::{self, NATIVE_BACKEND, Surface};
use kompoz::prelude::*;
use kompoz::test::{InjectionScope, KompozTestRule};
use kompoz::ui::input::{Key, KeyModifiers};
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
                        21 -> DialogsSection(rememberNavController())
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

const HOME: &str = "Ana sayfa ekranı KompozR'da 16. aşamada gelir; Material sekmesi hazır.";

/// The shell starts on its first tab (a screen that comes with stage 16); its Material item shows the Material screen.
fn the_shell_opens_the_material_tab() {
    let rule = KompozTestRule::new(showcase::ShowcaseApp);
    rule.on_node_with_text(HOME).assert_exists();
    rule.on_all_nodes_with_text("Material").on_first().perform_click();
    rule.on_node_with_text("SearchBar · SearchBarState").assert_exists();
    rule.on_node_with_text(HOME).assert_does_not_exist();
}

/// The dialogs section's "Harita" opens the places screen (C++ containers.cpp:92): its sheet peeking, dragged open,
/// closed by its button. It is on the Material tab's back stack: the home tab saves it (popUpTo the start with saveState), the
/// Material tab brings it back (restoreState), and back leads to the Material screen, then home.
fn the_map_button_opens_the_places_screen() {
    let rule = KompozTestRule::new(showcase::ShowcaseApp);
    rule.on_all_nodes_with_text("Material").on_first().perform_click();
    rule.on_node_with_text("Harita (BottomSheetScaffold)").perform_scroll_to().perform_click();
    rule.on_node_with_text("Yakındaki yerler").assert_exists();
    rule.on_node_with_text("Sayfa: kısmen açık").assert_exists();
    rule.on_node_with_text("SearchBar · SearchBarState").assert_does_not_exist();
    // (dragged up by its title: its buttons are below the 128 dp it peeks)
    rule.on_node_with_text("Yakındaki yerler").perform_touch_input(|t| {
        let y = t.center_y();
        t.swipe_up_with(y, y - 600.0, 300)
    });
    rule.wait_for_idle();
    rule.on_node_with_text("Sayfa: açık").assert_exists();
    rule.on_node_with_text("Daralt").perform_click();
    rule.on_node_with_text("Sayfa: kısmen açık").assert_exists();
    rule.on_all_nodes_with_text("Ana sayfa").on_first().perform_click();
    rule.on_node_with_text(HOME).assert_exists();
    rule.on_node_with_text("Yakındaki yerler").assert_does_not_exist();
    rule.on_all_nodes_with_text("Material").on_first().perform_click();
    rule.on_node_with_text("Yakındaki yerler").assert_exists();
    assert!(rule.host().key_down(Key::Escape, KeyModifiers::default()));
    rule.wait_for_idle();
    rule.on_node_with_text("SearchBar · SearchBarState").assert_exists();
    assert!(rule.host().key_down(Key::Escape, KeyModifiers::default()));
    rule.wait_for_idle();
    rule.on_node_with_text(HOME).assert_exists();
}

/// The C++ showcase's deep links (`showcase kompoz://item/42`): an item of the list graph (its typed route's path
/// argument), the components, Material and places screens.
fn deep_links_open_their_screens() {
    let rule = KompozTestRule::new(showcase::ShowcaseApp);
    rule.host().open_deep_link("kompoz://item/42");
    rule.wait_for_idle();
    rule.on_node_with_text("Öğe 42 ekranı KompozR'da 16. aşamada gelir; Material sekmesi hazır.").assert_exists();
    rule.host().open_deep_link("kompoz://places");
    rule.wait_for_idle();
    rule.on_node_with_text("Yakındaki yerler").assert_exists();
    rule.host().open_deep_link("kompoz://components");
    rule.wait_for_idle();
    rule.on_node_with_text("Bileşenler ekranı KompozR'da 16. aşamada gelir; Material sekmesi hazır.").assert_exists();
    rule.host().open_deep_link("kompoz://material");
    rule.wait_for_idle();
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
        let (_, parts) = shots::run_parts(&dir, Showcase::KompozR, config.scale).expect("its pages");
        comparisons.extend(shots::compare(config, &parts, &reference, &regions, &dir));
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
        ("the_map_button_opens_the_places_screen", the_map_button_opens_the_places_screen),
        ("deep_links_open_their_screens", deep_links_open_their_screens),
    ];
    let required = std::env::var_os("KOMPOZ_GPU_REQUIRED").is_some_and(|v| v == "1");
    match device::create(NATIVE_BACKEND, Surface::default()) {
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
