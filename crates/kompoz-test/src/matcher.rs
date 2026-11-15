//! Matchers of semantics nodes (Compose's SemanticsMatcher and Filters.kt): what finders look for and assertions
//! check, each with the description failures print.

use kompoz_ui::platform::ImeAction;
use kompoz_ui::semantics::{
    ProgressBarRangeInfo, Role, SemanticsActions, SemanticsNode, SemanticsProperties, SemanticsPropertyKey,
    ToggleableState,
};
use std::fmt;
use std::rc::Rc;

/// A condition on a semantics node, and its description (Compose's SemanticsMatcher). Combined with `and`, `or`
/// and `not` (`!`).
#[derive(Clone)]
pub struct SemanticsMatcher {
    pub description: String,
    matcher: Rc<dyn Fn(&SemanticsNode) -> bool>,
}

impl fmt::Debug for SemanticsMatcher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SemanticsMatcher({})", self.description)
    }
}

impl SemanticsMatcher {
    pub fn new(description: impl Into<String>, matcher: impl Fn(&SemanticsNode) -> bool + 'static) -> Self {
        SemanticsMatcher { description: description.into(), matcher: Rc::new(matcher) }
    }

    /// `key` set to `expected` (Compose's `expectValue`).
    pub fn expect_value<T: Clone + PartialEq + fmt::Debug + 'static>(
        key: SemanticsPropertyKey<T>,
        expected: T,
    ) -> Self {
        let description = format!("{} = '{}'", key.name(), debug_value(&expected));
        SemanticsMatcher::new(description, move |n| n.config().get(key).as_ref() == Some(&expected))
    }

    /// `key` set (Compose's `keyIsDefined`).
    pub fn key_is_defined<T: Clone + PartialEq + fmt::Debug + 'static>(key: SemanticsPropertyKey<T>) -> Self {
        SemanticsMatcher::new(format!("{} is defined", key.name()), move |n| n.config().contains(key))
    }

    /// `key` not set (Compose's `keyNotDefined`).
    pub fn key_not_defined<T: Clone + PartialEq + fmt::Debug + 'static>(key: SemanticsPropertyKey<T>) -> Self {
        SemanticsMatcher::new(format!("{} is NOT defined", key.name()), move |n| !n.config().contains(key))
    }

    pub fn matches(&self, node: &SemanticsNode) -> bool {
        (self.matcher)(node)
    }

    pub fn matches_any<'a>(&self, nodes: impl IntoIterator<Item = &'a SemanticsNode>) -> bool {
        nodes.into_iter().any(|n| self.matches(n))
    }

    /// Both (Compose's `and`).
    pub fn and(self, other: SemanticsMatcher) -> SemanticsMatcher {
        let description = format!("({}) && ({})", self.description, other.description);
        SemanticsMatcher::new(description, move |n| self.matches(n) && other.matches(n))
    }

    /// Either (Compose's `or`).
    pub fn or(self, other: SemanticsMatcher) -> SemanticsMatcher {
        let description = format!("({}) || ({})", self.description, other.description);
        SemanticsMatcher::new(description, move |n| self.matches(n) || other.matches(n))
    }
}

impl std::ops::Not for SemanticsMatcher {
    type Output = SemanticsMatcher;
    /// Not this (Compose's `not`).
    fn not(self) -> SemanticsMatcher {
        let description = format!("NOT ({})", self.description);
        SemanticsMatcher::new(description, move |n| !self.matches(n))
    }
}

/// A value as Kotlin prints it in a description (strings bare).
fn debug_value<T: fmt::Debug + 'static>(v: &T) -> String {
    let any: &dyn std::any::Any = v;
    if let Some(s) = any.downcast_ref::<String>() {
        return s.clone();
    }
    format!("{v:?}")
}

/// Two characters equal ignoring case as Kotlin's `Char.equals(other, ignoreCase = true)`: the same, or their
/// upper cases or their lower cases the same.
fn char_eq_ignore_case(a: char, b: char) -> bool {
    let upper = |c: char| c.to_uppercase().next().unwrap_or(c);
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    a == b || upper(a) == upper(b) || lower(a) == lower(b)
}

fn eq(a: &str, b: &str, ignore_case: bool) -> bool {
    if !ignore_case {
        return a == b;
    }
    a.chars().count() == b.chars().count() && a.chars().zip(b.chars()).all(|(x, y)| char_eq_ignore_case(x, y))
}

fn contains(a: &str, b: &str, ignore_case: bool) -> bool {
    if !ignore_case {
        return a.contains(b);
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    b.is_empty() || a.windows(b.len()).any(|w| w.iter().zip(&b).all(|(x, y)| char_eq_ignore_case(*x, *y)))
}

// ---------------------------------------------------------------------------------------------------------------
// The matchers (Compose's Filters.kt)

/// Not disabled (Compose's `isEnabled`).
pub fn is_enabled() -> SemanticsMatcher {
    SemanticsMatcher::new("is enabled", |n| !n.config().contains(SemanticsProperties::Disabled))
}

pub fn is_not_enabled() -> SemanticsMatcher {
    SemanticsMatcher::new("is not enabled", |n| n.config().contains(SemanticsProperties::Disabled))
}

/// Has a toggleable state.
pub fn is_toggleable() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsProperties::ToggleableState)
}

pub fn is_on() -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::ToggleableState, ToggleableState::On)
}

pub fn is_off() -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::ToggleableState, ToggleableState::Off)
}

/// Has a selected state.
pub fn is_selectable() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsProperties::Selected)
}

pub fn is_selected() -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::Selected, true)
}

pub fn is_not_selected() -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::Selected, false)
}

/// Has a focused state (a focusable).
pub fn is_focusable() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsProperties::Focused)
}

pub fn is_not_focusable() -> SemanticsMatcher {
    SemanticsMatcher::key_not_defined(SemanticsProperties::Focused)
}

pub fn is_focused() -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::Focused, true)
}

pub fn is_not_focused() -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::Focused, false)
}

pub fn has_click_action() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsActions::OnClick)
}

pub fn has_no_click_action() -> SemanticsMatcher {
    SemanticsMatcher::key_not_defined(SemanticsActions::OnClick)
}

pub fn has_scroll_action() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsActions::ScrollBy)
}

pub fn has_no_scroll_action() -> SemanticsMatcher {
    SemanticsMatcher::key_not_defined(SemanticsActions::ScrollBy)
}

/// A content description equal to `value` (or containing it, with `substring`) (Compose's
/// `hasContentDescription`).
pub fn has_content_description(value: &str, substring: bool, ignore_case: bool) -> SemanticsMatcher {
    let value = value.to_string();
    if substring {
        let d = format!("ContentDescription contains '{value}' (ignoreCase: {ignore_case})");
        SemanticsMatcher::new(d, move |n| {
            n.config()
                .get(SemanticsProperties::ContentDescription)
                .is_some_and(|ds| ds.iter().any(|d| contains(d, &value, ignore_case)))
        })
    } else {
        let d = format!("ContentDescription = '{value}' (ignoreCase: {ignore_case})");
        SemanticsMatcher::new(d, move |n| {
            n.config()
                .get(SemanticsProperties::ContentDescription)
                .is_some_and(|ds| ds.iter().any(|d| eq(d, &value, ignore_case)))
        })
    }
}

/// Exactly these content descriptions, in any order (Compose's `hasContentDescriptionExactly`).
pub fn has_content_description_exactly(values: &[&str]) -> SemanticsMatcher {
    let expected: Vec<String> = values.iter().map(|v| v.to_string()).collect();
    let d = format!("ContentDescription = [{}]", expected.join(","));
    SemanticsMatcher::new(d, move |n| match n.config().get(SemanticsProperties::ContentDescription) {
        Some(given) => {
            given.len() == expected.len()
                && given.iter().all(|g| expected.contains(g))
                && expected.iter().all(|e| given.contains(e))
        }
        None => expected.is_empty(),
    })
}

/// A text (or a field's input or shown text) equal to `text`, or containing it with `substring` (Compose's
/// `hasText`).
pub fn has_text(text: &str, substring: bool, ignore_case: bool) -> SemanticsMatcher {
    let text = text.to_string();
    let property = "Text + InputText + EditableText";
    let d = if substring {
        format!("{property} contains '{text}' (ignoreCase: {ignore_case}) as substring")
    } else {
        format!("{property} contains '{text}' (ignoreCase: {ignore_case})")
    };
    SemanticsMatcher::new(d, move |n| {
        let c = n.config();
        let check = |s: &str| if substring { contains(s, &text, ignore_case) } else { eq(s, &text, ignore_case) };
        c.get(SemanticsProperties::InputText).is_some_and(|t| check(t.text()))
            || c.get(SemanticsProperties::EditableText).is_some_and(|t| check(t.text()))
            || c.get(SemanticsProperties::Text).is_some_and(|ts| ts.iter().any(|t| check(t.text())))
    })
}

/// Exactly these texts (and the shown field text, with `include_editable_text`), in any order (Compose's
/// `hasTextExactly`).
pub fn has_text_exactly(values: &[&str], include_editable_text: bool) -> SemanticsMatcher {
    let expected: Vec<String> = values.iter().map(|v| v.to_string()).collect();
    let property = if include_editable_text { "Text + EditableText" } else { "Text" };
    let d = format!("{property} = [{}]", expected.join(","));
    SemanticsMatcher::new(d, move |n| {
        let c = n.config();
        let mut actual = Vec::new();
        if include_editable_text && let Some(e) = c.get(SemanticsProperties::EditableText) {
            actual.push(e.text().to_string());
        }
        actual.extend(c.get(SemanticsProperties::Text).unwrap_or_default().iter().map(|t| t.text().to_string()));
        actual.iter().all(|a| expected.contains(a)) && expected.iter().all(|e| actual.contains(e))
    })
}

pub fn has_state_description(value: &str) -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::StateDescription, value.to_string())
}

pub fn is_heading() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsProperties::Heading)
}

pub fn has_progress_bar_range_info(info: ProgressBarRangeInfo) -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::ProgressBarRangeInfo, info)
}

pub fn has_test_tag(tag: &str) -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::TestTag, tag.to_string())
}

/// What the node is (KompozR's: Compose's tests use `expectValue(Role, …)`).
pub fn has_role(role: Role) -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::Role, role)
}

pub fn is_dialog() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsProperties::IsDialog)
}

pub fn is_popup() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsProperties::IsPopup)
}

pub fn is_hidden_from_accessibility() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsProperties::HideFromAccessibility)
}

pub fn has_ime_action(action: ImeAction) -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::ImeAction, action)
}

pub fn has_set_text_action() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsActions::SetText)
}

pub fn has_insert_text_at_cursor_action() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsActions::InsertTextAtCursor)
}

pub fn has_perform_ime_action() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsActions::OnImeAction)
}

pub fn has_request_focus_action() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsActions::RequestFocus)
}

pub fn has_scroll_to_index_action() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsActions::ScrollToIndex)
}

pub fn has_scroll_to_key_action() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsActions::ScrollToIndex)
        .and(SemanticsMatcher::key_is_defined(SemanticsProperties::IndexForKey))
}

pub fn has_scroll_to_node_action() -> SemanticsMatcher {
    SemanticsMatcher::key_is_defined(SemanticsActions::ScrollToIndex)
        .and(SemanticsMatcher::key_is_defined(SemanticsActions::ScrollBy))
        .and(
            SemanticsMatcher::key_is_defined(SemanticsProperties::HorizontalScrollAxisRange)
                .or(SemanticsMatcher::key_is_defined(SemanticsProperties::VerticalScrollAxisRange)),
        )
}

pub fn is_editable() -> SemanticsMatcher {
    SemanticsMatcher::expect_value(SemanticsProperties::IsEditable, true)
}

/// A root (of the host's window or of a popup).
pub fn is_root() -> SemanticsMatcher {
    SemanticsMatcher::new("isRoot", |n| n.is_root())
}

pub fn has_parent(matcher: SemanticsMatcher) -> SemanticsMatcher {
    let d = format!("hasParentThat({})", matcher.description);
    SemanticsMatcher::new(d, move |n| n.parent().is_some_and(|p| matcher.matches(&p)))
}

pub fn has_any_child(matcher: SemanticsMatcher) -> SemanticsMatcher {
    let d = format!("hasAnyChildThat({})", matcher.description);
    SemanticsMatcher::new(d, move |n| matcher.matches_any(&n.children()))
}

pub fn has_any_sibling(matcher: SemanticsMatcher) -> SemanticsMatcher {
    let d = format!("hasAnySiblingThat({})", matcher.description);
    SemanticsMatcher::new(d, move |n| {
        let id = n.id();
        n.parent().is_some_and(|p| p.children().iter().any(|c| c.id() != id && matcher.matches(c)))
    })
}

pub fn has_any_ancestor(matcher: SemanticsMatcher) -> SemanticsMatcher {
    let d = format!("hasAnyAncestorThat({})", matcher.description);
    SemanticsMatcher::new(d, move |n| ancestors(n).iter().any(|a| matcher.matches(a)))
}

pub fn has_any_descendant(matcher: SemanticsMatcher) -> SemanticsMatcher {
    fn subtree_matches(matcher: &SemanticsMatcher, node: &SemanticsNode) -> bool {
        let children = node.children();
        matcher.matches_any(&children) || children.iter().any(|c| subtree_matches(matcher, c))
    }
    let d = format!("hasAnyDescendantThat({})", matcher.description);
    SemanticsMatcher::new(d, move |n| subtree_matches(&matcher, n))
}

/// The nodes above `node`, nearest first (Compose's `ancestors`).
pub(crate) fn ancestors(node: &SemanticsNode) -> Vec<SemanticsNode> {
    let mut out = Vec::new();
    let mut p = node.parent();
    while let Some(n) = p {
        p = n.parent();
        out.push(n);
    }
    out
}
