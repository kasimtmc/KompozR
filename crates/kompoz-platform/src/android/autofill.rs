//! What Android's autofill is told (Compose's AndroidAutofillManager and `ViewStructure.populate`): the view's
//! virtual structure — the unmerged semantics tree's nodes related to autofill (a content type, a content data type,
//! fillable data), each a child of the nearest such node above it, with its autofill id, type, hints, value, texts,
//! states and bounds — and as the app goes on, the AutofillManager's notifications from the host's `autofill_*`
//! callbacks (a field entered or left, its value changed, shown or gone, the form committed or cancelled, a field
//! asking to be filled), each value of the type its node says. What a service fills in comes back as
//! [`FillableData`] through the host. Kotlin's KompozAutofill reads both from little-endian buffers
//! ([`encode_structure`], [`encode_notices`]).
//!
//! Plain Rust, tested on every host; `semantics` reads the nodes from the host.

use super::accessibility::Writer;
use kompoz_graphics::Rect;
use kompoz_ui::semantics::{ContentDataType, FillableData};

/// An autofill value (Android's AutofillValue), of its node's type.
#[derive(Clone, PartialEq, Debug)]
pub enum AutofillValue {
    Text(String),
    Toggle(bool),
    List(i32),
    /// Milliseconds since the epoch.
    Date(i64),
}

impl AutofillValue {
    /// The value of fillable data (Compose's AndroidFillableData's `autofillValue`).
    pub fn of(data: &FillableData) -> AutofillValue {
        match data {
            FillableData::Text(t) => AutofillValue::Text(t.clone()),
            FillableData::Boolean(b) => AutofillValue::Toggle(*b),
            FillableData::ListIndex(i) => AutofillValue::List(*i),
            FillableData::Date(d) => AutofillValue::Date(*d),
        }
    }

    /// A value the host told as text (its `autofill_value`: a toggle as "true"/"false", a list index and a date as
    /// numbers) as the type of `data_type` (a text where it says none); none if it doesn't read as one.
    pub fn parse(data_type: Option<ContentDataType>, text: &str) -> Option<AutofillValue> {
        Some(match data_type {
            Some(ContentDataType::Toggle) => AutofillValue::Toggle(text.parse().ok()?),
            Some(ContentDataType::List) => AutofillValue::List(text.parse().ok()?),
            Some(ContentDataType::Date) => AutofillValue::Date(text.parse().ok()?),
            Some(ContentDataType::None) => return None,
            Some(ContentDataType::Text) | None => AutofillValue::Text(text.to_string()),
        })
    }

    /// What a service filled in (Android's AutofillValue as KompozAutofill passes it: its type, a number, a text) as
    /// the host's fillable data.
    pub fn fillable(kind: i32, number: i64, text: Option<String>) -> Option<FillableData> {
        Some(match kind {
            autofill_type::TEXT => FillableData::Text(text?),
            autofill_type::TOGGLE => FillableData::Boolean(number != 0),
            autofill_type::LIST => FillableData::ListIndex(number as i32),
            autofill_type::DATE => FillableData::Date(number),
            _ => return None,
        })
    }

    fn write(&self, w: &mut Writer) {
        match self {
            AutofillValue::Text(t) => {
                w.int(autofill_type::TEXT);
                w.long(0);
                w.str(Some(t));
            }
            AutofillValue::Toggle(b) => {
                w.int(autofill_type::TOGGLE);
                w.long(*b as i64);
                w.str(None);
            }
            AutofillValue::List(i) => {
                w.int(autofill_type::LIST);
                w.long(*i as i64);
                w.str(None);
            }
            AutofillValue::Date(d) => {
                w.int(autofill_type::DATE);
                w.long(*d);
                w.str(None);
            }
        }
    }
}

/// View's `AUTOFILL_TYPE_*` (the same values).
pub mod autofill_type {
    pub const NONE: i32 = 0;
    pub const TEXT: i32 = 1;
    pub const TOGGLE: i32 = 2;
    pub const LIST: i32 = 3;
    pub const DATE: i32 = 4;
}

/// The autofill type of a content data type (Compose's `ContentDataType.dataType`).
pub fn type_of(data_type: ContentDataType) -> i32 {
    match data_type {
        ContentDataType::None => autofill_type::NONE,
        ContentDataType::Text => autofill_type::TEXT,
        ContentDataType::List => autofill_type::LIST,
        ContentDataType::Date => autofill_type::DATE,
        ContentDataType::Toggle => autofill_type::TOGGLE,
    }
}

/// A node of the view's autofill structure (one `ViewStructure`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct AutofillNode {
    /// Its virtual id (the root's: the view's own structure, -1).
    pub id: i32,
    /// The node it is a child of, by its index in the structure (-1: the root has none).
    pub parent: i32,
    /// Its autofill type (none: [`autofill_type::NONE`], not set).
    pub autofill_type: i32,
    pub hints: Vec<&'static str>,
    pub value: Option<AutofillValue>,
    pub description: Option<String>,
    pub focused: Option<bool>,
    pub clickable: bool,
    pub long_clickable: bool,
    pub focusable: bool,
    pub enabled: bool,
    pub checkable: bool,
    pub checked: bool,
    pub selected: Option<bool>,
    /// Its data is sensitive (a password, or not said otherwise: Compose's default).
    pub sensitive: bool,
    pub visible: bool,
    /// Its bounds in the view, px.
    pub bounds: Rect,
    /// Its texts, merged (a line each).
    pub text: Option<String>,
    pub class_name: Option<&'static str>,
    pub max_text_length: Option<i32>,
    /// A password field (its input type a password's).
    pub password: bool,
}

/// The structure as KompozAutofill reads it: the count, then each node (the root first, each parent before its
/// children).
pub fn encode_structure(nodes: &[AutofillNode]) -> Vec<u8> {
    let mut w = Writer::default();
    w.int(nodes.len() as i32);
    for n in nodes {
        w.int(n.id);
        w.int(n.parent);
        w.int(n.autofill_type);
        w.int(n.hints.len() as i32);
        for h in &n.hints {
            w.str(Some(h));
        }
        match &n.value {
            Some(v) => v.write(&mut w),
            None => {
                w.int(autofill_type::NONE);
                w.long(0);
                w.str(None);
            }
        }
        w.str(n.description.as_deref());
        w.int(n.focused.map_or(-1, |f| f as i32));
        w.int(n.selected.map_or(-1, |s| s as i32));
        for b in [n.clickable, n.long_clickable, n.focusable, n.enabled, n.checkable, n.checked, n.sensitive, n.visible]
        {
            w.bool(b);
        }
        w.rect(n.bounds);
        w.str(n.text.as_deref());
        w.str(n.class_name);
        w.int(n.max_text_length.unwrap_or(-1));
        w.bool(n.password);
    }
    w.0
}

/// What the AutofillManager is told (KompozAutofill's `NOTICE_*`).
#[derive(Clone, PartialEq, Debug)]
pub enum Notice {
    /// `notifyViewEntered` with the field's bounds in the view (px).
    Entered { id: i32, bounds: Rect },
    /// `notifyViewExited`.
    Exited { id: i32 },
    /// `notifyValueChanged`.
    Value { id: i32, value: AutofillValue },
    /// `notifyViewVisibilityChanged`.
    Visibility { id: i32, visible: bool },
    /// `commit` or `cancel`.
    Done { commit: bool },
    /// `requestAutofill` for the field at its bounds.
    Request { id: i32, bounds: Rect },
}

/// KompozAutofill's notice kinds.
pub mod notice {
    pub const ENTERED: i32 = 0;
    pub const EXITED: i32 = 1;
    pub const VALUE: i32 = 2;
    pub const VISIBILITY: i32 = 3;
    pub const COMMIT: i32 = 4;
    pub const CANCEL: i32 = 5;
    pub const REQUEST: i32 = 6;
}

/// The notices as KompozAutofill reads them: the count, then each one's kind and node, and its bounds, value or
/// visibility.
pub fn encode_notices(notices: &[Notice]) -> Vec<u8> {
    let mut w = Writer::default();
    w.int(notices.len() as i32);
    for n in notices {
        match n {
            Notice::Entered { id, bounds } | Notice::Request { id, bounds } => {
                w.int(if matches!(n, Notice::Entered { .. }) { notice::ENTERED } else { notice::REQUEST });
                w.int(*id);
                w.rect(*bounds);
            }
            Notice::Exited { id } => {
                w.int(notice::EXITED);
                w.int(*id);
            }
            Notice::Value { id, value } => {
                w.int(notice::VALUE);
                w.int(*id);
                value.write(&mut w);
            }
            Notice::Visibility { id, visible } => {
                w.int(notice::VISIBILITY);
                w.int(*id);
                w.bool(*visible);
            }
            Notice::Done { commit } => {
                w.int(if *commit { notice::COMMIT } else { notice::CANCEL });
                w.int(-1);
            }
        }
    }
    w.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_follow_their_nodes_types() {
        assert_eq!(AutofillValue::parse(Some(ContentDataType::Toggle), "true"), Some(AutofillValue::Toggle(true)));
        assert_eq!(AutofillValue::parse(Some(ContentDataType::List), "3"), Some(AutofillValue::List(3)));
        assert_eq!(AutofillValue::parse(Some(ContentDataType::Date), "86400000"), Some(AutofillValue::Date(86400000)));
        assert_eq!(AutofillValue::parse(None, "ada"), Some(AutofillValue::Text("ada".into())));
        assert_eq!(AutofillValue::parse(Some(ContentDataType::Toggle), "evet"), None);
        assert_eq!(AutofillValue::parse(Some(ContentDataType::None), "x"), None);
        assert_eq!(
            AutofillValue::fillable(autofill_type::TEXT, 0, Some("a@b.c".into())),
            Some(FillableData::Text("a@b.c".into()))
        );
        assert_eq!(AutofillValue::fillable(autofill_type::TOGGLE, 1, None), Some(FillableData::Boolean(true)));
        assert_eq!(AutofillValue::fillable(autofill_type::TEXT, 0, None), None);
        assert_eq!(AutofillValue::of(&FillableData::ListIndex(2)), AutofillValue::List(2));
        assert_eq!(type_of(ContentDataType::Date), autofill_type::DATE);
        let structure = encode_structure(&[AutofillNode { id: -1, parent: -1, ..Default::default() }]);
        assert_eq!(i32::from_le_bytes(structure[..4].try_into().unwrap()), 1);
        let notices = encode_notices(&[Notice::Done { commit: true }, Notice::Exited { id: 4 }]);
        assert_eq!(&notices[..12], [2, 0, 0, 0, 4, 0, 0, 0, 255, 255, 255, 255]);
    }
}
