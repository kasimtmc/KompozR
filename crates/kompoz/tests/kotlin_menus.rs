//! Material's menus in Kotlin's syntax: material3 1.5's MenuSamples.kt and ExposedDropdownMenuSamples.kt from a
//! `.kt` file (kompoz_file!) — a DropdownMenu of items with icons, a divider and a trailing text; one scrolled to its
//! end by its scroll state; checkable items in groups of a DropdownMenuPopup (MenuDefaults' group and item shapes);
//! the exposed dropdown menus (a read-only anchor and selectable items, an editable one filtering its options as one
//! types, a multi-autocomplete one completing the token at the cursor) and ExposedDropdownMenuBox's documentation
//! form — each composed and used as a user does. (The samples' TooltipBox, WP7's, and GroupedMenuSample's ButtonGroup,
//! WP1b's, wait for those packages.) Density 1: dp are px.

// (GroupedMenuSample leaves its `homeChecked` state unused, as Compose's sample does; OldAnchorMenu uses the deprecated
// `MenuAnchorType`)
#![allow(non_snake_case, non_upper_case_globals, unused_variables, deprecated)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::input::PointerType;
use kompoz::ui::semantics::SemanticsProperties;

kompoz_file!("tests/kt/menus.kt");

fn in_theme(content: impl Fn() + 'static) -> KompozTestRule {
    let content = std::rc::Rc::new(content);
    KompozTestRule::new(kompozable_lambda!(move || {
        let content = content.clone();
        MaterialTheme(content = move || content());
    }))
}

/// A text's own bounds (an item merges its text: the unmerged tree's).
fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    named_args!(rule.on_node_with_text(text, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

/// A mouse's click at `p` (a finger's tap on a field would show its cursor handle: another overlay).
fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Mouse);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

/// A text field's node (a read-only one's too: its EditableText).
fn editable() -> kompoz::test::SemanticsMatcher {
    kompoz::test::SemanticsMatcher::key_is_defined(SemanticsProperties::EditableText)
}

/// The text the (only) text field holds.
fn field_text(rule: &KompozTestRule) -> String {
    let node = rule.on_all_nodes_with(editable(), true).on_first().fetch_semantics_node();
    node.config().get(SemanticsProperties::EditableText).map(|t| t.text().to_string()).unwrap_or_default()
}

fn field_bounds(rule: &KompozTestRule) -> Rect {
    rule.on_all_nodes_with(editable(), true).on_first().get_unclipped_bounds_in_root()
}

/// Whether the item of `text` is selected (a checked item too: selected, to accessibility).
fn is_selected(rule: &KompozTestRule, text: &str) -> bool {
    let item = kompoz::test::has_text(text, false, false).and(kompoz::test::is_selectable());
    let node = rule.on_node(item).fetch_semantics_node();
    node.config().get(SemanticsProperties::Selected) == Some(true)
}

/// MenuSample: the icon button opens the menu below it, its items' text 12 dp in from its start, the leading icons,
/// the divider and the trailing "F11"; a tap outside closes it.
fn menu_sample() {
    let rule = in_theme(kompozable_lambda!(|| MenuSample()));
    rule.on_node_with_text("Edit").assert_does_not_exist();
    let anchor = rule.on_node_with_content_description("Localized description").get_unclipped_bounds_in_root();
    rule.on_node_with_content_description("Localized description").perform_click();
    assert_eq!(rule.host().overlay_count(), 1);
    for text in ["Edit", "Settings", "Send Feedback", "F11"] {
        rule.on_node_with_text(text).assert_exists();
    }
    let edit = text_bounds(&rule, "Edit");
    let settings = text_bounds(&rule, "Settings");
    // (below the anchor's 48 dp room — its 40 dp container at 4 —, at the room's start: the text after the 12 dp
    // padding, the 24 dp leading icon and 12 dp, 48 in)
    assert!(edit.top > anchor.bottom - 1.0 && settings.top > edit.bottom, "{anchor:?} {edit:?} {settings:?}");
    assert!((edit.left - (anchor.left - 4.0 + 48.0)).abs() < 0.5, "{anchor:?} {edit:?}");
    // (the F11 trailing text at the item's end, after "Send Feedback")
    assert!(text_bounds(&rule, "F11").left > text_bounds(&rule, "Send Feedback").right);
    tap(&rule, Offset::new(700.0, 700.0));
    rule.wait_for_idle();
    rule.on_node_with_text("Edit").assert_does_not_exist();
    assert_eq!(rule.host().overlay_count(), 0);
}

/// GroupedMenuSample: two groups under their labels, MenuDefaults' group shapes and spacing, the items' supporting
/// texts; a checkable item turns checked (selected, to accessibility) as it is clicked, its leading icon the checked
/// one (the filled Check for Home, which had none) and its trailing icon the filled one; the menu stays open.
fn grouped_menu_sample() {
    let rule = in_theme(kompozable_lambda!(|| GroupedMenuSample()));
    rule.on_node_with_content_description("Localized description").perform_click();
    for text in ["Modification", "Navigation", "Edit", "Settings", "Home", "More Options", "Edit mode", "Opens menu"] {
        rule.on_node_with_text(text).assert_exists();
    }
    // (the second group below the first, MenuDefaults.GroupSpacing apart at least)
    let (modification, navigation) = (text_bounds(&rule, "Modification"), text_bounds(&rule, "Navigation"));
    assert!(navigation.top > text_bounds(&rule, "Settings").bottom && modification.top < navigation.top);
    assert!(!is_selected(&rule, "Home"));
    let icons_before = rule.draw().vectors.len();
    rule.on_node_with_text("Home").perform_click();
    rule.wait_for_idle();
    assert!(is_selected(&rule, "Home"));
    rule.on_node_with_text("Modification").assert_exists();
    // (Home had no leading icon: its checked one, the filled Check, is drawn now; its trailing icon swapped)
    assert_eq!(rule.draw().vectors.len(), icons_before + 1);
    rule.on_node_with_text("Home").perform_click();
    rule.wait_for_idle();
    assert!(!is_selected(&rule, "Home"));
    assert_eq!(rule.draw().vectors.len(), icons_before);
    rule.on_node_with_text("Edit").perform_click();
    rule.wait_for_idle();
    assert!(is_selected(&rule, "Edit") && !is_selected(&rule, "Settings"));
}

/// MenuWithScrollStateSample: 30 items in a menu taller than the window, scrolled to its end by the sample's effect
/// as it opens (its scroll state given to DropdownMenu): the last item in view, the first above it.
fn menu_with_scroll_state_sample() {
    let rule = in_theme(kompozable_lambda!(|| MenuWithScrollStateSample()));
    rule.on_node_with_content_description("Localized description").perform_click();
    rule.wait_for_idle();
    let last = text_bounds(&rule, "Item 30");
    let first = text_bounds(&rule, "Item 1");
    let window = rule.host().viewport_size();
    assert!(last.bottom <= window.height as f32 + 0.5 && last.top > 0.0, "{last:?}");
    assert!(first.bottom < 0.5, "{first:?}");
}

/// ExposedDropdownMenuSample: a read-only field showing the first option; a tap opens the menu (the standalone group
/// shape, the item shapes by position) under it; picking an option shows it in the field and selects it (its Check
/// icon), the menu staying open as the sample leaves it; the trailing icon's tap closes it.
fn exposed_dropdown_menu_sample() {
    let rule = in_theme(kompozable_lambda!(|| ExposedDropdownMenuSample()));
    assert_eq!(field_text(&rule), "Android");
    let field = field_bounds(&rule);
    tap(&rule, field.center());
    assert_eq!(rule.host().overlay_count(), 1);
    // (Android the field's text too)
    rule.on_all_nodes_with_text("Android").assert_count_equals(2);
    for option in ["Base", "Cupcake", "Donut", "Eclair"] {
        rule.on_node_with_text(option).assert_exists();
    }
    rule.on_node_with_text("Froyo").assert_does_not_exist();
    // (the menu as wide as the field, under it)
    let cupcake = text_bounds(&rule, "Cupcake");
    assert!(cupcake.top > field.bottom - 1.0 && cupcake.right < field.right, "{field:?} {cupcake:?}");
    assert!(!is_selected(&rule, "Cupcake"));
    let checks = rule.draw().vectors.len();
    rule.on_node_with_text("Cupcake").perform_click();
    rule.wait_for_idle();
    assert_eq!(field_text(&rule), "Cupcake");
    assert!(is_selected(&rule, "Cupcake") && !is_selected(&rule, "Base"));
    // (the selected item's leading Check)
    assert_eq!(rule.draw().vectors.len(), checks + 1);
    assert_eq!(rule.host().overlay_count(), 1);
    tap(&rule, field.center());
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 0);
}

/// EditableExposedDropdownMenuSample: the field takes typing (PrimaryEditable: the menu doesn't take the focus); a
/// tap opens all the options, typing filters them by subsequence (underlined), a pick sets the field and closes the
/// menu; nothing matching closes it.
fn editable_exposed_dropdown_menu_sample() {
    let rule = in_theme(kompozable_lambda!(|| EditableExposedDropdownMenuSample()));
    let field = field_bounds(&rule);
    assert!((field.width() - 280.0).abs() < 0.5, "{field:?}");
    tap(&rule, field.center());
    assert_eq!(rule.host().overlay_count(), 1);
    rule.on_node_with_text("Android").assert_exists();
    assert!(rule.host().text_input("ice"));
    rule.wait_for_idle();
    assert_eq!(field_text(&rule), "ice");
    for shown in ["Ice Cream Sandwich", "Quince Tart", "Upside Down Cake", "Vanilla Ice Cream"] {
        rule.on_node_with_text(shown).assert_exists();
    }
    for hidden in ["Android", "Pie", "Eclair", "Cupcake"] {
        rule.on_node_with_text(hidden).assert_does_not_exist();
    }
    // (the matched letters underlined: an annotated option)
    let node = rule.on_node_with_text("Quince Tart").fetch_semantics_node();
    let texts = node.config().get(SemanticsProperties::Text).unwrap_or_default();
    assert!(texts.iter().any(|t| !t.span_styles().is_empty()), "{texts:?}");
    rule.on_node_with_text("Quince Tart").perform_click();
    rule.wait_for_idle();
    assert_eq!(field_text(&rule), "Quince Tart");
    assert_eq!(rule.host().overlay_count(), 0);
    // (nothing matching: the menu stays closed though allowed)
    tap(&rule, field.center());
    assert_eq!(rule.host().overlay_count(), 1);
    assert!(rule.host().text_input("zz"));
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 0);
}

/// MultiAutocompleteExposedDropdownMenuSample: the token at the cursor (between commas) filters the options; a pick
/// replaces the token and adds ", " (the cursor after it), so the next token starts empty (no options) until typed.
fn multi_autocomplete_exposed_dropdown_menu_sample() {
    let rule = in_theme(kompozable_lambda!(|| MultiAutocompleteExposedDropdownMenuSample()));
    let field = field_bounds(&rule);
    tap(&rule, field.center());
    // (an empty token: no options, no menu)
    assert_eq!(rule.host().overlay_count(), 0);
    assert!(rule.host().text_input("cup"));
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 1);
    rule.on_node_with_text("Cupcake").assert_exists();
    rule.on_node_with_text("Upside Down Cake").assert_does_not_exist();
    rule.on_node_with_text("Donut").assert_does_not_exist();
    rule.on_node_with_text("Cupcake").perform_click();
    rule.wait_for_idle();
    assert_eq!(field_text(&rule), "Cupcake, ");
    assert_eq!(rule.host().overlay_count(), 0);
    assert!(rule.host().text_input("don"));
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 1);
    rule.on_node_with_text("Donut").perform_click();
    rule.wait_for_idle();
    assert_eq!(field_text(&rule), "Cupcake, Donut, ");
}

/// ExposedDropdownMenuBox's documentation form: a filling read-only String field and items given `forEach`'s `it`,
/// each item's padding ExposedDropdownMenuDefaults.ItemContentPadding; a pick sets the value and closes the menu.
fn exposed_dropdown_menu_of_strings() {
    let rule = in_theme(kompozable_lambda!(|| ExposedDropdownMenuOfStrings(vec![
        "Bir".to_string(),
        "İki".to_string(),
        "Üç".to_string()
    ])));
    let field = rule.on_node_with_tag("dizgi").get_unclipped_bounds_in_root();
    assert!((field.width() - 300.0).abs() < 0.5, "{field:?}");
    assert_eq!(field_text(&rule), "Bir");
    tap(&rule, field.center());
    let iki = text_bounds(&rule, "İki");
    // (the items' text 16 dp in: the exposed menu's item padding)
    assert!((iki.left - (field.left + 16.0)).abs() < 0.5 && iki.top > field.bottom - 1.0, "{field:?} {iki:?}");
    rule.on_node_with_text("Üç").perform_click();
    rule.wait_for_idle();
    assert_eq!(field_text(&rule), "Üç");
    assert_eq!(rule.host().overlay_count(), 0);
}

/// The deprecated `MenuAnchorType` anchors the menu as ExposedDropdownMenuAnchorType does; a shaped item in its
/// colors (MenuDefaults.itemColors) closes it, a checkable one turns checked and keeps it open.
fn old_anchor_menu() {
    let rule = in_theme(kompozable_lambda!(|| OldAnchorMenu()));
    tap(&rule, field_bounds(&rule).center());
    assert_eq!(rule.host().overlay_count(), 1);
    let c = rule.draw();
    let at = c.texts.iter().position(|t| t == "Biçimli").expect("the shaped item");
    assert_eq!(c.colors[at], Color::Red);
    rule.on_node_with_text("İşaretli").perform_click();
    rule.wait_for_idle();
    assert!(is_selected(&rule, "İşaretli"));
    rule.on_node_with_text("Biçimli").perform_click();
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 0);
}

fn main() {
    common::run(&[
        ("menu_sample", menu_sample),
        ("grouped_menu_sample", grouped_menu_sample),
        ("menu_with_scroll_state_sample", menu_with_scroll_state_sample),
        ("exposed_dropdown_menu_sample", exposed_dropdown_menu_sample),
        ("editable_exposed_dropdown_menu_sample", editable_exposed_dropdown_menu_sample),
        ("multi_autocomplete_exposed_dropdown_menu_sample", multi_autocomplete_exposed_dropdown_menu_sample),
        ("exposed_dropdown_menu_of_strings", exposed_dropdown_menu_of_strings),
        ("old_anchor_menu", old_anchor_menu),
    ]);
}
