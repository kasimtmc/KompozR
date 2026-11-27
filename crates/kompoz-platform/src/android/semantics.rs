//! What Android's accessibility, content capture, translation, autofill and scroll capture read from the host and
//! do to it, beyond the shared accessibility tree (what Compose's Android delegates read from a node's
//! configuration themselves): each node's [`Extras`]; the actions Android asks for, done as Compose's
//! `performActionHelper` does them (scrolls by direction and by page, a slider's steps, scrolling to a collection's
//! item or a node onto the screen, selections in UTF-16 offsets); a text's characters' bounds; the unmerged tree's
//! shown nodes for content capture and their translations; the autofill structure and what a service filled in;
//! the scrollable that scroll capture captures, scrolled by offsets.
//!
//! Plain Rust over kompoz-ui's semantics, tested on every host (tests/android.rs).

use super::accessibility::{Extras, Request, ScrollAxis, byte_at, has, progress_step, scroll_amount};
use super::autofill::{AutofillNode, AutofillValue, type_of};
use super::capture::CaptureNode;
use kompoz_graphics::{IntRect, LayoutDirection, Offset, Rect};
use kompoz_text::TextLayoutResult;
use kompoz_ui::KompozHost;
use kompoz_ui::semantics::{
    AccessibilityNode, ContentDataType, Role, ScrollAxisRange, SemanticsAction, SemanticsActions,
    SemanticsConfiguration, SemanticsNode, SemanticsProperties,
};
use kompoz_ui::state::ToggleableState;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// The merged tree's nodes by id (what `accessibility_tree`'s nodes are of).
fn merged_nodes(host: &KompozHost) -> HashMap<u64, SemanticsNode> {
    let mut out = HashMap::new();
    for n in host.semantics_owner().all_semantics_nodes(true) {
        for fake in n.replaced_children().into_iter().filter(SemanticsNode::is_fake) {
            out.insert(fake.id(), fake);
        }
        out.insert(n.id(), n);
    }
    out
}

/// The node `id` of the merged tree.
fn merged(host: &KompozHost, id: u64) -> Option<SemanticsNode> {
    host.semantics_owner().find(id, true)
}

fn axis(range: Option<ScrollAxisRange>) -> Option<ScrollAxis> {
    range.map(|r| ScrollAxis { value: (r.value)(), max: (r.max_value)(), reverse: r.reverse_scrolling })
}

/// Its text layout, if it hands one out (Compose's `getTextLayoutResult`).
fn text_layout(config: &SemanticsConfiguration) -> Option<TextLayoutResult> {
    let get = config.get(SemanticsActions::GetTextLayoutResult)?.action?;
    let mut out = Vec::new();
    if get(&mut out) { out.into_iter().next() } else { None }
}

/// Whether `config` has the action `key` (set, whatever its function).
macro_rules! has_action {
    ($config:expr, $key:ident) => {
        $config.contains(SemanticsActions::$key)
    };
}

/// Each node's [`Extras`] (by semantics id), for the nodes of `tree` (the host's accessibility tree).
pub fn extras(host: &KompozHost, tree: &[AccessibilityNode]) -> HashMap<u64, Extras> {
    let nodes = merged_nodes(host);
    let mut out = HashMap::with_capacity(tree.len());
    for a in tree {
        let Some(n) = nodes.get(&a.id) else { continue };
        out.insert(a.id, extras_of(n, a));
    }
    out
}

fn extras_of(n: &SemanticsNode, a: &AccessibilityNode) -> Extras {
    let c = n.config();
    let strings = |key| c.get(key).unwrap_or_default();
    let texts: Vec<String> = c
        .get(SemanticsProperties::Text)
        .unwrap_or_default()
        .iter()
        .map(|t| t.text().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let descriptions: Vec<String> =
        strings(SemanticsProperties::ContentDescription).into_iter().filter(|d: &String| !d.is_empty()).collect();
    let mut bits = 0;
    let mut set = |bit: u32, on: bool| {
        if on {
            bits |= bit;
        }
    };
    set(has::SCROLL_BY, has_action!(c, ScrollBy));
    set(has::SCROLL_TO_INDEX, has_action!(c, ScrollToIndex));
    set(has::PAGE_UP, has_action!(c, PageUp));
    set(has::PAGE_DOWN, has_action!(c, PageDown));
    set(has::PAGE_LEFT, has_action!(c, PageLeft));
    set(has::PAGE_RIGHT, has_action!(c, PageRight));
    set(has::COPY, has_action!(c, CopyText));
    set(has::CUT, has_action!(c, CutText));
    set(has::PASTE, has_action!(c, PasteText));
    set(has::IME_ACTION, has_action!(c, OnImeAction));
    set(has::SET_TEXT, has_action!(c, SetText));
    set(has::SET_SELECTION, has_action!(c, SetSelection));
    set(has::DISMISS, has_action!(c, Dismiss));
    set(has::SET_PROGRESS, has_action!(c, SetProgress));
    set(has::TEXT_LAYOUT, has_action!(c, GetTextLayoutResult));
    set(has::REQUEST_FOCUS, has_action!(c, RequestFocus));
    let focused = c.get(SemanticsProperties::Focused) == Some(true) || a.focused;
    // Where the lines start, where the layout's text is the text moved over (no description before it).
    let layout = if bits & has::TEXT_LAYOUT != 0 && descriptions.is_empty() { text_layout(&c) } else { None };
    let iterable = if a.editable { Some(a.value.as_str()) } else { texts.first().map(String::as_str) };
    let lines = layout
        .as_ref()
        .filter(|l| Some(l.layout_input.text.text()) == iterable)
        .map(|l| (0..l.line_count()).map(|i| (l.get_line_start(i), l.get_line_top(i))).collect::<Vec<(usize, f32)>>());
    Extras {
        texts,
        descriptions,
        merging: n.is_merging_semantics_of_descendants(),
        long_clickable: has_action!(c, OnLongClick),
        focusable: c.contains(SemanticsProperties::Focused),
        selection: c
            .get(SemanticsProperties::TextSelectionRange)
            .filter(|_| a.editable)
            .map(|r| (r.start.min(a.value.len()), r.end.min(a.value.len()))),
        horizontal: axis(c.get(SemanticsProperties::HorizontalScrollAxisRange)),
        vertical: axis(c.get(SemanticsProperties::VerticalScrollAxisRange)),
        has: bits,
        multi_line: layout.as_ref().is_some_and(|l| l.line_count() > 1),
        lines,
        max_text_length: c.get(SemanticsProperties::MaxTextLength),
        text_entry_key: c.contains(SemanticsProperties::TextEntryKey),
        sensitive: c.get(SemanticsProperties::IsSensitiveData) == Some(true),
        rtl: n.layout_node().layout_direction() == LayoutDirection::Rtl,
        empty_bounds: !n.is_root() && n.touch_bounds_in_root().is_empty(),
        // (A field without the focus: TalkBack's lines and pages aren't for a field it isn't typing into.)
        exclude_lines: a.editable && !focused,
    }
}

// ---- Actions ----

/// Does what Android asked of node `id` (Compose's `performActionHelper`; accessibility focus and moves by
/// granularity are KompozAccessibility's own): whether it was done. A disabled node takes none but setting its
/// selection and copying. Selections are UTF-16 offsets of the text moved over; a node without a selection action
/// of its own takes none (KompozAccessibility keeps its cursor then). Showing a node with no scrollable above it:
/// false (the view then asks its own parents to).
pub fn perform(host: &KompozHost, id: u64, request: Request) -> bool {
    let Some(node) = merged(host, id) else { return false };
    let c = node.config();
    let enabled = !c.contains(SemanticsProperties::Disabled);
    let semantic = |action: SemanticsAction| host.perform_semantics_action(id, action);
    match request {
        Request::SetSelection { start, end, traversal } => {
            if !(enabled && has_action!(c, SetSelection)) {
                return false;
            }
            // (In traversal, relative to the text as typed; else to the text shown.)
            let key = if traversal { SemanticsProperties::InputText } else { SemanticsProperties::EditableText };
            let text = c.get(key).or_else(|| c.get(SemanticsProperties::EditableText));
            let text = text.map(|t| t.text().to_string()).unwrap_or_default();
            let at = |unit: i32| if unit < 0 { -1 } else { byte_at(&text, unit) as i32 };
            semantic(SemanticsAction::SetSelection {
                start: at(start),
                end: at(end),
                relative_to_original_text: traversal,
            })
        }
        Request::Copy => semantic(SemanticsAction::CopyText),
        _ if !enabled => false,
        Request::Click => semantic(SemanticsAction::Click),
        Request::LongClick => semantic(SemanticsAction::LongClick),
        Request::Focus => semantic(SemanticsAction::Focus),
        Request::ClearFocus => c.get(SemanticsProperties::Focused) == Some(true) && host.clear_focus(false),
        Request::ScrollForward
        | Request::ScrollBackward
        | Request::ScrollUp
        | Request::ScrollDown
        | Request::ScrollLeft
        | Request::ScrollRight => scroll(&node, &c, &request),
        Request::ScrollToPosition { row, column } => {
            let Some(to) = c.get(SemanticsActions::ScrollToIndex).and_then(|a| a.action) else { return false };
            let info = c.get(SemanticsProperties::CollectionInfo);
            let index = match info {
                Some(i) if i.row_count == 1 && i.column_count > 1 => column,
                Some(i) if i.row_count > 1 && i.column_count > 1 => row * i.column_count + column,
                _ => row,
            };
            to(index)
        }
        Request::SetProgress(v) => semantic(SemanticsAction::SetProgress(v)),
        Request::SetText(t) => semantic(SemanticsAction::SetText(t)),
        Request::Expand => semantic(SemanticsAction::Expand),
        Request::Collapse => semantic(SemanticsAction::Collapse),
        Request::Dismiss => semantic(SemanticsAction::Dismiss),
        Request::Cut => semantic(SemanticsAction::CutText),
        Request::Paste => semantic(SemanticsAction::PasteText),
        Request::ImeEnter => semantic(SemanticsAction::ImeAction),
        Request::PageUp => semantic(SemanticsAction::PageUp),
        Request::PageDown => semantic(SemanticsAction::PageDown),
        Request::PageLeft => semantic(SemanticsAction::PageLeft),
        Request::PageRight => semantic(SemanticsAction::PageRight),
        Request::ShowOnScreen => show_on_screen(&node),
        Request::Custom(i) => semantic(SemanticsAction::CustomAction(i)),
    }
}

/// A scroll by direction (Compose's): a slider's step forward or back, else a viewport's length along each axis
/// that scrolls that way (a page where it pages), if it can go there.
fn scroll(node: &SemanticsNode, c: &SemanticsConfiguration, request: &Request) -> bool {
    let forward = matches!(request, Request::ScrollForward);
    if (forward || matches!(request, Request::ScrollBackward))
        && let (Some(r), Some(set)) = (
            c.get(SemanticsProperties::ProgressBarRangeInfo),
            c.get(SemanticsActions::SetProgress).and_then(|a| a.action),
        )
    {
        return set(progress_step(r.current, *r.range.start(), *r.range.end(), r.steps, forward));
    }
    let Some(scroll_by) = c.get(SemanticsActions::ScrollBy).and_then(|a| a.action) else { return false };
    let mut lengths = Vec::new();
    let viewport = c
        .get(SemanticsActions::GetScrollViewportLength)
        .and_then(|a| a.action)
        .filter(|f| f(&mut lengths))
        .and_then(|_| lengths.first().copied());
    let size = node.coordinates().bounds_in_parent();
    let rtl = node.layout_node().layout_direction() == LayoutDirection::Rtl;
    let can = |a: ScrollAxis, amount: f32| (amount < 0.0 && a.value > 0.0) || (amount > 0.0 && a.value < a.max);
    if let Some(x) = axis(c.get(SemanticsProperties::HorizontalScrollAxisRange))
        && let Some(amount) = scroll_amount(request, viewport.unwrap_or(size.width()), true, x.reverse, rtl)
        && can(x, amount)
    {
        if has_action!(c, PageLeft) || has_action!(c, PageRight) {
            let page = if amount > 0.0 { SemanticsActions::PageRight } else { SemanticsActions::PageLeft };
            return c.get(page).and_then(|a| a.action).is_some_and(|f| f());
        }
        return scroll_by(amount, 0.0);
    }
    if let Some(y) = axis(c.get(SemanticsProperties::VerticalScrollAxisRange))
        && let Some(amount) = scroll_amount(request, viewport.unwrap_or(size.height()), false, y.reverse, rtl)
        && can(y, amount)
    {
        if has_action!(c, PageUp) || has_action!(c, PageDown) {
            let page = if amount > 0.0 { SemanticsActions::PageDown } else { SemanticsActions::PageUp };
            return c.get(page).and_then(|a| a.action).is_some_and(|f| f());
        }
        return scroll_by(0.0, amount);
    }
    false
}

/// The scrollables above `node` scrolled just enough to show it whole (Compose's `scrollOntoScreen`), each by what
/// is left after those below it; whether one scrolled. None above it: false.
fn show_on_screen(node: &SemanticsNode) -> bool {
    let mut ancestor = node.parent();
    while let Some(a) = &ancestor {
        if a.unmerged_config().contains(SemanticsActions::ScrollBy) {
            break;
        }
        ancestor = a.parent();
    }
    if ancestor.is_none() {
        return false;
    }
    let size = node.size();
    let rtl = node.layout_node().layout_direction() == LayoutDirection::Rtl;
    let mut scrolled = false;
    let mut accumulated = Offset::Zero;
    while let Some(a) = ancestor {
        let config = a.unmerged_config();
        if let Some(scroll_by) = config.get(SemanticsActions::ScrollBy).and_then(|s| s.action) {
            let viewport = a.unclipped_bounds_in_root();
            let p = node.position_in_root();
            let target = Rect {
                left: p.x + accumulated.x,
                top: p.y + accumulated.y,
                right: p.x + accumulated.x + size.width as f32,
                bottom: p.y + accumulated.y + size.height as f32,
            };
            let (dx, dy) = scroll_into(viewport, target);
            let mut fx = dx;
            let mut fy = dy;
            if config.get(SemanticsProperties::HorizontalScrollAxisRange).is_some_and(|r| r.reverse_scrolling) {
                fx = -fx;
            }
            if rtl {
                fx = -fx;
            }
            if config.get(SemanticsProperties::VerticalScrollAxisRange).is_some_and(|r| r.reverse_scrolling) {
                fy = -fy;
            }
            if (dx, dy) != (0.0, 0.0) {
                scrolled = scroll_by(fx, fy) || scrolled;
            }
            accumulated = Offset::new(accumulated.x - dx, accumulated.y - dy);
        }
        ancestor = a.parent();
    }
    scrolled
}

/// How far `viewport` scrolls to show `target` whole (Compose's `scrollDxDyForNodeVisible`): of the moves that
/// line up either edge, the smaller; none where the edges would go opposite ways.
pub fn scroll_into(viewport: Rect, target: Rect) -> (f32, f32) {
    // (Kotlin's `sign`: 0 for 0.)
    let sign = |v: f32| {
        if v > 0.0 {
            1
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    };
    let delta = |a: f32, b: f32| {
        if sign(a) != sign(b) {
            0.0
        } else if a.abs() < b.abs() {
            a
        } else {
            b
        }
    };
    (
        delta(target.left - viewport.left, target.right - viewport.right),
        delta(target.top - viewport.top, target.bottom - viewport.bottom),
    )
}

/// Where the characters `start..start + length` (UTF-16 offsets of its text) of node `id`'s text are, px in the
/// view (EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY; Compose's `getBoundingBoxes`): each one's box cut to the node's
/// bounds, none for one outside them or past the text.
pub fn character_bounds(host: &KompozHost, id: u64, start: i32, length: i32) -> Vec<Option<Rect>> {
    let Some(node) = merged(host, id) else { return Vec::new() };
    let Some(layout) = text_layout(&node.config()) else { return Vec::new() };
    let coordinates = node.layout_node().inner_coordinates();
    if !coordinates.is_attached() {
        return Vec::new();
    }
    let origin = coordinates.position_in_window();
    let bounds = node.bounds_in_window();
    let text = layout.layout_input.text.text().to_string();
    let units = text.encode_utf16().count() as i32;
    (start..start.saturating_add(length.max(0)))
        .map(|unit| {
            if unit < 0 || unit >= units {
                return None;
            }
            let b = layout.get_bounding_box(byte_at(&text, unit));
            let r = Rect {
                left: b.left + origin.x,
                top: b.top + origin.y,
                right: b.right + origin.x,
                bottom: b.bottom + origin.y,
            };
            r.overlaps(bounds).then(|| r.intersect(bounds))
        })
        .collect()
}

// ---- Content capture and translation ----

/// The texts of a config, joined by lines.
fn joined_texts(c: &SemanticsConfiguration) -> Option<String> {
    let texts = c.get(SemanticsProperties::Text)?;
    Some(texts.iter().map(|t| t.text()).collect::<Vec<_>>().join("\n"))
}

/// The unmerged tree's shown nodes (Compose's uncovered nodes, for content capture): every node placed in a window
/// that isn't transparent, parents first, with its place among its parent's shown children.
pub fn capture_nodes(host: &KompozHost) -> Vec<CaptureNode> {
    let mut out = Vec::new();
    for (i, root) in host.semantics_owner().root_semantics_nodes(false).into_iter().enumerate() {
        visit_capture(&root, None, i as i32, &mut out);
    }
    out
}

fn shown(n: &SemanticsNode) -> bool {
    !n.is_fake() && n.is_placed() && n.layout_node().is_attached() && !n.is_transparent()
}

fn visit_capture(n: &SemanticsNode, parent: Option<u64>, index: i32, out: &mut Vec<CaptureNode>) {
    if !shown(n) {
        return;
    }
    let c = n.unmerged_config();
    let mut class_name = None;
    if c.get(SemanticsProperties::IsTraversalGroup).is_some() {
        class_name = Some("android.view.ViewGroup");
    }
    let text = joined_texts(c);
    if text.is_some() {
        class_name = Some("android.widget.TextView");
    }
    let editable_text = c.get(SemanticsProperties::EditableText).map(|t| t.text().to_string());
    if editable_text.is_some() {
        class_name = Some("android.widget.EditText");
    }
    if let Some(role) = c.get(SemanticsProperties::Role).and_then(compose_legacy_class_name) {
        class_name = Some(role);
    }
    // (Its style's size: resolved by then, in sp; another kind of size isn't said.)
    let text_size = text_layout(c).and_then(|l| {
        let input = &l.layout_input;
        input.style.font_size.is_sp().then(|| input.density.text_to_px(input.style.font_size))
    });
    let b = if parent.is_none() { n.bounds_in_window() } else { n.bounds_in_parent() };
    out.push(CaptureNode {
        id: n.id(),
        parent,
        index,
        bounds: [b.left, b.top, b.width(), b.height()],
        class_name,
        text,
        editable_text,
        description: c.get(SemanticsProperties::ContentDescription).map(|d| d.join("\n")),
        test_tag: c.get(SemanticsProperties::TestTag),
        password: c.contains(SemanticsProperties::Password),
        text_size: text_size.filter(|s| s.is_finite()),
    });
    let mut i = 0;
    for child in n.children() {
        if shown(&child) {
            visit_capture(&child, Some(n.id()), i, out);
            i += 1;
        }
    }
}

/// Compose's `toLegacyClassName` (content capture and autofill keep Compose's own: no Switch there).
fn compose_legacy_class_name(role: Role) -> Option<&'static str> {
    match role {
        Role::Switch => None,
        role => super::accessibility::legacy_class_name(role),
    }
}

/// The node of the unmerged tree with `id`.
fn unmerged(host: &KompozHost, id: u64) -> Option<SemanticsNode> {
    host.semantics_owner().find(id, false)
}

/// What node `id` asks to be translated (its texts, a line each; Compose's `onCreateVirtualViewTranslationRequests`).
pub fn translation_text(host: &KompozHost, id: u64) -> Option<String> {
    joined_texts(unmerged(host, id)?.unmerged_config())
}

/// Node `id`'s translation set (Compose's `doTranslation`: its `SetTextSubstitution`); whether it took it.
pub fn set_translation(host: &KompozHost, id: u64, text: &str) -> bool {
    let Some(node) = unmerged(host, id) else { return false };
    let action = node.unmerged_config().get(SemanticsActions::SetTextSubstitution).and_then(|a| a.action);
    action.is_some_and(|f| f(text.into()))
}

/// Every shown node's translation shown, hidden or cleared (Compose's `showTranslatedText`, `hideTranslatedText`,
/// `clearTranslatedText`): `show` true, false, or none to clear.
pub fn show_translations(host: &KompozHost, show: Option<bool>) {
    for n in host.semantics_owner().all_semantics_nodes(false) {
        if !shown(&n) {
            continue;
        }
        apply_translation(&n, show);
    }
}

/// A node's translation shown or hidden as `show` says (none: cleared), where it is the other way.
fn apply_translation(n: &SemanticsNode, show: Option<bool>) {
    let c = n.unmerged_config();
    let showing = c.get(SemanticsProperties::IsShowingTextSubstitution);
    match show {
        Some(on) if showing == Some(!on) => {
            if let Some(f) = c.get(SemanticsActions::ShowTextSubstitution).and_then(|a| a.action) {
                f(on);
            }
        }
        None if showing.is_some() => {
            if let Some(f) = c.get(SemanticsActions::ClearTextSubstitution).and_then(|a| a.action) {
                f();
            }
        }
        _ => {}
    }
}

/// Nodes `ids` (just come) showing their translations as the others do (Compose's `updateTranslationOnAppeared`).
pub fn translate_appeared(host: &KompozHost, ids: &[u64], translated: bool) {
    for &id in ids {
        if let Some(n) = unmerged(host, id) {
            apply_translation(&n, Some(translated));
        }
    }
}

// ---- Autofill ----

/// Whether a node is the autofill structure's (Compose's `isRelatedToAutofill`).
fn related_to_autofill(c: &SemanticsConfiguration) -> bool {
    c.contains(SemanticsActions::OnFillData)
        || c.contains(SemanticsProperties::ContentType)
        || c.contains(SemanticsProperties::ContentDataType)
}

/// The view's autofill structure (Compose's `populateViewStructure`): the unmerged root (the view's own
/// structure), then each node related to autofill as a child of the nearest such node above it, parents first.
pub fn autofill_nodes(host: &KompozHost) -> Vec<AutofillNode> {
    let mut out = Vec::new();
    let Some(root) = host.semantics_owner().unmerged_root_semantics_node() else { return out };
    out.push(autofill_node(&root, true, -1));
    let mut stack = vec![(root, 0i32)];
    while let Some((node, parent)) = stack.pop() {
        for child in node.children() {
            let layout = child.layout_node();
            if layout.is_deactivated() || !layout.is_attached() || !child.is_placed() {
                continue;
            }
            if !related_to_autofill(child.unmerged_config()) {
                stack.push((child, parent));
                continue;
            }
            out.push(autofill_node(&child, false, parent));
            let index = out.len() as i32 - 1;
            stack.push((child, index));
        }
    }
    out
}

fn autofill_node(n: &SemanticsNode, root: bool, parent: i32) -> AutofillNode {
    let c = n.unmerged_config();
    let merged = n.with_merging_enabled().config();
    let data_type = c.get(SemanticsProperties::ContentDataType);
    let has_set_text = c.contains(SemanticsActions::SetText);
    let toggle = c.get(SemanticsProperties::ToggleableState);
    let selected = c.get(SemanticsProperties::Selected);
    let role = c.get(SemanticsProperties::Role);
    let hints = c.get(SemanticsProperties::ContentType).map(|t| t.hints).unwrap_or_default();
    let mut value = c.get(SemanticsProperties::EditableText).map(|t| AutofillValue::Text(t.text().to_string()));
    if let Some(data) = c.get(SemanticsProperties::FillableData) {
        value = Some(AutofillValue::of(&data));
    }
    let password = c.contains(SemanticsProperties::Password) || hints.contains(&"password");
    // (Sensitive unless the node says it isn't: Compose's default.)
    let sensitive = password || c.get(SemanticsProperties::IsSensitiveData).unwrap_or(true);
    let text: Option<String> =
        merged.get(SemanticsProperties::Text).map(|t| t.iter().map(|t| format!("{}\n", t.text())).collect());
    let mut class_name = text.as_ref().map(|_| "android.widget.TextView");
    if n.children().is_empty()
        && let Some(c) = role.and_then(compose_legacy_class_name)
    {
        class_name = Some(c);
    }
    if has_set_text {
        class_name = Some("android.widget.EditText");
    }
    let (checkable, checked) = match (toggle, selected) {
        (Some(t), _) => (true, t == ToggleableState::On),
        (None, Some(s)) if role != Some(Role::Tab) => (true, s),
        _ => (false, false),
    };
    AutofillNode {
        id: if root { super::accessibility::HOST_VIEW_ID } else { n.id() as i32 },
        parent,
        autofill_type: match data_type {
            Some(t) => type_of(t),
            None if has_set_text => super::autofill::autofill_type::TEXT,
            None if toggle.is_some() => super::autofill::autofill_type::TOGGLE,
            None => super::autofill::autofill_type::NONE,
        },
        hints,
        value,
        description: c.get(SemanticsProperties::ContentDescription).and_then(|d| d.into_iter().next()),
        focused: c.get(SemanticsProperties::Focused),
        clickable: c.contains(SemanticsActions::OnClick),
        long_clickable: c.contains(SemanticsActions::OnLongClick),
        focusable: c.contains(SemanticsActions::RequestFocus),
        enabled: !merged.contains(SemanticsProperties::Disabled),
        checkable,
        checked,
        selected,
        sensitive,
        visible: !n.is_transparent(),
        bounds: n.bounds_in_window(),
        text,
        class_name,
        max_text_length: c.get(SemanticsProperties::MaxTextLength).filter(|_| has_set_text),
        password: password && has_set_text,
    }
}

/// The content data type of node `id` (what its autofill values are), if it says one.
pub fn content_data_type(host: &KompozHost, id: u64) -> Option<ContentDataType> {
    unmerged(host, id)?.unmerged_config().get(SemanticsProperties::ContentDataType)
}

// ---- Scroll capture ----

/// The scrollable that a long screenshot captures (Compose's ScrollCapture `onScrollCaptureSearch`): of the
/// enabled, shown nodes that scroll vertically by offsets, the deepest, of those the tallest; its semantics id and
/// its viewport (px in the window).
pub fn scroll_capture_candidate(host: &KompozHost) -> Option<(u64, IntRect)> {
    let root = host.semantics_owner().unmerged_root_semantics_node()?;
    let mut candidates: Vec<(usize, i32, u64, IntRect)> = Vec::new();
    visit_scroll_candidates(&root, 0, &mut candidates);
    candidates.sort_by_key(|&(depth, height, _, _)| (depth, height));
    candidates.last().map(|&(_, _, id, viewport)| (id, viewport))
}

fn visit_scroll_candidates(from: &SemanticsNode, depth: usize, out: &mut Vec<(usize, i32, u64, IntRect)>) {
    let mut stack = from.children();
    while let Some(node) = stack.pop() {
        if node.is_fake() || node.is_hidden() || node.unmerged_config().contains(SemanticsProperties::Disabled) {
            continue;
        }
        let b = node.coordinates().bounds_in_window();
        let viewport = IntRect {
            left: b.left.round() as i32,
            top: b.top.round() as i32,
            right: b.right.round() as i32,
            bottom: b.bottom.round() as i32,
        };
        if viewport.right <= viewport.left || viewport.bottom <= viewport.top {
            continue;
        }
        let c = node.unmerged_config();
        let scrolls = c.contains(SemanticsActions::ScrollByOffset)
            && c.get(SemanticsProperties::VerticalScrollAxisRange).is_some_and(|r| (r.max_value)() > 0.0);
        if !scrolls {
            stack.extend(node.children());
            continue;
        }
        out.push((depth + 1, viewport.bottom - viewport.top, node.id(), viewport));
        visit_scroll_candidates(&node, depth + 1, out);
    }
}

/// Scrolls node `id` down by `delta` px (up if negative) by its offset action (reversed where it scrolls
/// reversed), in a coroutine: how far it went. None if it can't.
pub fn scroll_by_offset(host: &KompozHost, id: u64, delta: f32) -> Option<Pin<Box<dyn Future<Output = f32>>>> {
    let node = unmerged(host, id)?;
    let c = node.unmerged_config();
    let action = c.get(SemanticsActions::ScrollByOffset)?;
    let reverse = c.get(SemanticsProperties::VerticalScrollAxisRange).is_some_and(|r| r.reverse_scrolling);
    let actual = if reverse { -delta } else { delta };
    let scrolled = (action.0)(Offset::new(0.0, actual));
    Some(Box::pin(async move {
        let consumed = scrolled.await;
        if reverse { -consumed.y } else { consumed.y }
    }))
}

/// Where scroll capture has scrolled its node to (Compose's RelativeScroller): from where it started, in px, and
/// how far it goes to center a range in its viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RelativeScroller {
    pub viewport: i32,
    pub amount: f32,
}

impl RelativeScroller {
    /// How far to scroll to center `min..max` (px from where it started) in the viewport; none where it is in view
    /// already.
    pub fn delta_to_center(&self, min: i32, max: i32) -> Option<f32> {
        if min as f32 >= self.amount && max as f32 <= self.amount + self.viewport as f32 {
            return None;
        }
        let target = min + (max - min) / 2 - self.viewport / 2;
        Some(target as f32 - self.amount)
    }

    /// An offset from where it started, in the viewport (clamped to it).
    pub fn to_viewport(&self, offset: i32) -> i32 {
        (offset - self.amount.round() as i32).clamp(0, self.viewport)
    }
}
