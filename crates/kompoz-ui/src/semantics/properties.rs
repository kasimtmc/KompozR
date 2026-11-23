//! The semantics properties and actions (Compose's SemanticsProperties and SemanticsActions), the values they
//! hold, and their setters on [`SemanticsConfiguration`] (Compose's SemanticsPropertyReceiver extensions).
//!
//! Texts are AnnotatedStrings (kompoz-text's, as Compose's). Setters of properties are
//! `set_*` (Compose's `x = …`), those Compose writes as functions keep their names (`heading()`, `on_click(…)`);
//! the actions `SetText` and `SetTextSubstitution`, whose names a property setter has, are set by
//! `set_text_action` and `set_text_substitution_action`.

use super::configuration::{SemanticsConfiguration, SemanticsPropertyKey, keep_parent_only};
use crate::platform::ImeAction;
use crate::state::ToggleableState;
use kompoz_graphics::{Offset, Shape};
use kompoz_runtime::named;
pub use kompoz_text::TextRange;
use kompoz_text::{AnnotatedString, TextLayoutResult};
use std::any::Any;
use std::fmt;
use std::future::Future;
use std::ops::{Add, RangeInclusive};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// Values

/// An action accessibility can perform, and its label (Compose's AccessibilityAction): `F` is its function's type,
/// `dyn Fn() -> bool` for most (true: it was done). Equal when the labels are and the actions are the same one.
pub struct AccessibilityAction<F: ?Sized> {
    pub label: Option<String>,
    pub action: Option<Rc<F>>,
}

impl<F: ?Sized> AccessibilityAction<F> {
    pub fn new(label: Option<String>, action: Option<Rc<F>>) -> Self {
        AccessibilityAction { label, action }
    }
}

impl<F: ?Sized> Clone for AccessibilityAction<F> {
    fn clone(&self) -> Self {
        AccessibilityAction { label: self.label.clone(), action: self.action.clone() }
    }
}

impl<F: ?Sized> PartialEq for AccessibilityAction<F> {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
            && match (&self.action, &other.action) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

impl<F: ?Sized> fmt::Debug for AccessibilityAction<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let action = if self.action.is_some() { "Function" } else { "null" };
        write!(f, "AccessibilityAction(label={:?}, action={action})", self.label)
    }
}

/// An action merged into a parent: the parent's label and action, what it lacks from the child's.
fn merge_action<F: ?Sized>(
    parent: Option<AccessibilityAction<F>>,
    child: AccessibilityAction<F>,
) -> Option<AccessibilityAction<F>> {
    Some(match parent {
        Some(p) => AccessibilityAction { label: p.label.or(child.label), action: p.action.or(child.action) },
        None => child,
    })
}

/// Two actions for one node made one: the first's label and action, what it lacks from the second.
fn combine_action<F: ?Sized>(
    first: &AccessibilityAction<F>,
    second: &AccessibilityAction<F>,
) -> AccessibilityAction<F> {
    AccessibilityAction {
        label: first.label.clone().or_else(|| second.label.clone()),
        action: first.action.clone().or_else(|| second.action.clone()),
    }
}

/// An action key (Compose's ActionPropertyKey): important for accessibility, merged and combined as actions are.
const fn action_key<F: ?Sized + 'static>(name: &'static str) -> SemanticsPropertyKey<AccessibilityAction<F>> {
    SemanticsPropertyKey::accessibility_with_merge_policy(name, merge_action::<F>).combining(combine_action::<F>)
}

/// What an action that takes nothing does.
pub type ActionFn = dyn Fn() -> bool;

/// An action of its own a node offers accessibility (Compose's CustomAccessibilityAction): its label and what it
/// does (true: done). Equal when the labels are and the actions are the same one.
#[derive(Clone)]
pub struct CustomAccessibilityAction {
    pub label: String,
    pub action: Rc<ActionFn>,
}

impl CustomAccessibilityAction {
    /// Kotlin: `CustomAccessibilityAction(label: String, action: () -> Boolean)`
    pub fn new(label: impl Into<String>, action: impl Fn() -> bool + 'static) -> Self {
        CustomAccessibilityAction { label: label.into(), action: Rc::new(action) }
    }
}

impl PartialEq for CustomAccessibilityAction {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && Rc::ptr_eq(&self.action, &other.action)
    }
}

impl fmt::Debug for CustomAccessibilityAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CustomAccessibilityAction(label={}, action=Function)", self.label)
    }
}

/// A progress (Compose's ProgressBarRangeInfo): the current value within `range`, in `steps` steps between its
/// ends (0: continuous). [`Indeterminate`](Self::Indeterminate) for a progress without a value.
#[derive(Clone, PartialEq, Debug)]
pub struct ProgressBarRangeInfo {
    pub current: f32,
    pub range: RangeInclusive<f32>,
    pub steps: i32,
}

impl ProgressBarRangeInfo {
    /// A progress of no known value (an indeterminate spinner).
    pub const Indeterminate: ProgressBarRangeInfo =
        ProgressBarRangeInfo { current: 0.0, range: RangeInclusive::new(0.0, 0.0), steps: 0 };

    /// `current` in `range` with `steps` steps; panics if `current` is NaN, as Compose's.
    #[track_caller]
    pub fn new(current: f32, range: RangeInclusive<f32>, steps: i32) -> Self {
        assert!(!current.is_nan(), "current must not be NaN");
        ProgressBarRangeInfo { current, range, steps }
    }
}

/// A collection's size (Compose's CollectionInfo): its rows and columns (-1 where unknown).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CollectionInfo {
    pub row_count: i32,
    pub column_count: i32,
}

/// An item's place in a collection (Compose's CollectionItemInfo).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CollectionItemInfo {
    pub row_index: i32,
    pub row_span: i32,
    pub column_index: i32,
    pub column_span: i32,
}

/// How far a scrollable has scrolled along an axis (Compose's ScrollAxisRange): its value and its maximum, read
/// when asked, and whether it scrolls the other way. Equal when its functions are the same ones.
#[derive(Clone)]
pub struct ScrollAxisRange {
    pub value: Rc<dyn Fn() -> f32>,
    pub max_value: Rc<dyn Fn() -> f32>,
    pub reverse_scrolling: bool,
}

impl ScrollAxisRange {
    pub fn new(
        value: impl Fn() -> f32 + 'static,
        max_value: impl Fn() -> f32 + 'static,
        reverse_scrolling: bool,
    ) -> ScrollAxisRange {
        ScrollAxisRange { value: Rc::new(value), max_value: Rc::new(max_value), reverse_scrolling }
    }
}

impl PartialEq for ScrollAxisRange {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.value, &other.value)
            && Rc::ptr_eq(&self.max_value, &other.max_value)
            && self.reverse_scrolling == other.reverse_scrolling
    }
}

impl fmt::Debug for ScrollAxisRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ScrollAxisRange(value={}, maxValue={}, reverseScrolling={})",
            (self.value)(),
            (self.max_value)(),
            self.reverse_scrolling
        )
    }
}

/// What a node is to accessibility (Compose's Role).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Button,
    Checkbox,
    Switch,
    RadioButton,
    Tab,
    Image,
    DropdownList,
    ValuePicker,
    Carousel,
}

impl fmt::Debug for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Role::Button => "Button",
            Role::Checkbox => "Checkbox",
            Role::Switch => "Switch",
            Role::RadioButton => "RadioButton",
            Role::Tab => "Tab",
            Role::Image => "Image",
            Role::DropdownList => "DropdownList",
            Role::ValuePicker => "Picker",
            Role::Carousel => "Carousel",
        })
    }
}

/// How changes of a live region are announced (Compose's LiveRegionMode).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LiveRegionMode {
    Polite,
    Assertive,
}

/// What a field holds, for autofill (Compose's ContentType): the platform's autofill hints. Combined with `+`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ContentType {
    pub hints: Vec<&'static str>,
}

// (written out, not made by a macro: the Kotlin index reads them, Kotlin's `ContentType.Username`)
impl ContentType {
    /// The `emailAddress` autofill hint.
    pub fn EmailAddress() -> ContentType {
        ContentType { hints: vec!["emailAddress"] }
    }

    /// The `username` autofill hint.
    pub fn Username() -> ContentType {
        ContentType { hints: vec!["username"] }
    }

    /// The `password` autofill hint.
    pub fn Password() -> ContentType {
        ContentType { hints: vec!["password"] }
    }

    /// The `newUsername` autofill hint.
    pub fn NewUsername() -> ContentType {
        ContentType { hints: vec!["newUsername"] }
    }

    /// The `newPassword` autofill hint.
    pub fn NewPassword() -> ContentType {
        ContentType { hints: vec!["newPassword"] }
    }

    /// The `postalAddress` autofill hint.
    pub fn PostalAddress() -> ContentType {
        ContentType { hints: vec!["postalAddress"] }
    }

    /// The `postalCode` autofill hint.
    pub fn PostalCode() -> ContentType {
        ContentType { hints: vec!["postalCode"] }
    }

    /// The `creditCardNumber` autofill hint.
    pub fn CreditCardNumber() -> ContentType {
        ContentType { hints: vec!["creditCardNumber"] }
    }

    /// The `creditCardSecurityCode` autofill hint.
    pub fn CreditCardSecurityCode() -> ContentType {
        ContentType { hints: vec!["creditCardSecurityCode"] }
    }

    /// The `creditCardExpirationDate` autofill hint.
    pub fn CreditCardExpirationDate() -> ContentType {
        ContentType { hints: vec!["creditCardExpirationDate"] }
    }

    /// The `creditCardExpirationMonth` autofill hint.
    pub fn CreditCardExpirationMonth() -> ContentType {
        ContentType { hints: vec!["creditCardExpirationMonth"] }
    }

    /// The `creditCardExpirationYear` autofill hint.
    pub fn CreditCardExpirationYear() -> ContentType {
        ContentType { hints: vec!["creditCardExpirationYear"] }
    }

    /// The `creditCardExpirationDay` autofill hint.
    pub fn CreditCardExpirationDay() -> ContentType {
        ContentType { hints: vec!["creditCardExpirationDay"] }
    }

    /// The `addressCountry` autofill hint.
    pub fn AddressCountry() -> ContentType {
        ContentType { hints: vec!["addressCountry"] }
    }

    /// The `addressRegion` autofill hint.
    pub fn AddressRegion() -> ContentType {
        ContentType { hints: vec!["addressRegion"] }
    }

    /// The `addressLocality` autofill hint.
    pub fn AddressLocality() -> ContentType {
        ContentType { hints: vec!["addressLocality"] }
    }

    /// The `streetAddress` autofill hint.
    pub fn AddressStreet() -> ContentType {
        ContentType { hints: vec!["streetAddress"] }
    }

    /// The `extendedAddress` autofill hint.
    pub fn AddressAuxiliaryDetails() -> ContentType {
        ContentType { hints: vec!["extendedAddress"] }
    }

    /// The `extendedPostalCode` autofill hint.
    pub fn PostalCodeExtended() -> ContentType {
        ContentType { hints: vec!["extendedPostalCode"] }
    }

    /// The `personName` autofill hint.
    pub fn PersonFullName() -> ContentType {
        ContentType { hints: vec!["personName"] }
    }

    /// The `personGivenName` autofill hint.
    pub fn PersonFirstName() -> ContentType {
        ContentType { hints: vec!["personGivenName"] }
    }

    /// The `personFamilyName` autofill hint.
    pub fn PersonLastName() -> ContentType {
        ContentType { hints: vec!["personFamilyName"] }
    }

    /// The `personMiddleName` autofill hint.
    pub fn PersonMiddleName() -> ContentType {
        ContentType { hints: vec!["personMiddleName"] }
    }

    /// The `personMiddleInitial` autofill hint.
    pub fn PersonMiddleInitial() -> ContentType {
        ContentType { hints: vec!["personMiddleInitial"] }
    }

    /// The `personNamePrefix` autofill hint.
    pub fn PersonNamePrefix() -> ContentType {
        ContentType { hints: vec!["personNamePrefix"] }
    }

    /// The `personNameSuffix` autofill hint.
    pub fn PersonNameSuffix() -> ContentType {
        ContentType { hints: vec!["personNameSuffix"] }
    }

    /// The `phoneNumber` autofill hint.
    pub fn PhoneNumber() -> ContentType {
        ContentType { hints: vec!["phoneNumber"] }
    }

    /// The `phoneNumberDevice` autofill hint.
    pub fn PhoneNumberDevice() -> ContentType {
        ContentType { hints: vec!["phoneNumberDevice"] }
    }

    /// The `phoneCountryCode` autofill hint.
    pub fn PhoneCountryCode() -> ContentType {
        ContentType { hints: vec!["phoneCountryCode"] }
    }

    /// The `phoneNational` autofill hint.
    pub fn PhoneNumberNational() -> ContentType {
        ContentType { hints: vec!["phoneNational"] }
    }

    /// The `gender` autofill hint.
    pub fn Gender() -> ContentType {
        ContentType { hints: vec!["gender"] }
    }

    /// The `birthDateFull` autofill hint.
    pub fn BirthDateFull() -> ContentType {
        ContentType { hints: vec!["birthDateFull"] }
    }

    /// The `birthDateDay` autofill hint.
    pub fn BirthDateDay() -> ContentType {
        ContentType { hints: vec!["birthDateDay"] }
    }

    /// The `birthDateMonth` autofill hint.
    pub fn BirthDateMonth() -> ContentType {
        ContentType { hints: vec!["birthDateMonth"] }
    }

    /// The `birthDateYear` autofill hint.
    pub fn BirthDateYear() -> ContentType {
        ContentType { hints: vec!["birthDateYear"] }
    }

    /// The `smsOTPCode` autofill hint.
    pub fn SmsOtpCode() -> ContentType {
        ContentType { hints: vec!["smsOTPCode"] }
    }
}

impl ContentType {
    /// A content type of the platform's hints given.
    pub fn new(hints: &[&'static str]) -> ContentType {
        ContentType { hints: hints.to_vec() }
    }
}

impl Add for ContentType {
    type Output = ContentType;
    /// Both (Compose's `ContentType.plus`).
    fn add(mut self, other: ContentType) -> ContentType {
        for h in other.hints {
            if !self.hints.contains(&h) {
                self.hints.push(h);
            }
        }
        self
    }
}

/// The kind of data a field holds, for autofill (Compose's ContentDataType).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ContentDataType {
    None,
    Text,
    List,
    Date,
    Toggle,
}

/// Data an autofill service fills in (Compose's FillableData).
#[derive(Clone, PartialEq, Debug)]
pub enum FillableData {
    Text(String),
    Boolean(bool),
    ListIndex(i32),
    /// A date, milliseconds since the epoch.
    Date(i64),
}

impl FillableData {
    pub fn text_value(&self) -> Option<&str> {
        if let FillableData::Text(t) = self { Some(t) } else { None }
    }
    pub fn boolean_value(&self) -> Option<bool> {
        if let FillableData::Boolean(b) = self { Some(*b) } else { None }
    }
    pub fn list_index_value(&self) -> Option<i32> {
        if let FillableData::ListIndex(i) = self { Some(*i) } else { None }
    }
    /// The list index, or `default` (Compose's `getListIndexOrDefault`).
    pub fn list_index_or_default(&self, default: i32) -> i32 {
        self.list_index_value().unwrap_or(default)
    }
    pub fn date_value(&self) -> Option<i64> {
        if let FillableData::Date(d) = self { Some(*d) } else { None }
    }
}

/// The state of an input method's suggestion in a field (Compose's InputTextSuggestionState).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct InputTextSuggestionState {
    pub is_committed_by_input_method_editor: bool,
    pub is_transliteration_suggestion_selected: bool,
}

/// A lazy layout's mapping from an item's key to its index (Compose's IndexForKey value). Equal when it is the
/// same one.
#[derive(Clone)]
pub struct IndexForKey(pub Rc<dyn Fn(&dyn Any) -> i32>);

impl PartialEq for IndexForKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for IndexForKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Function")
    }
}

/// What `ScrollByOffset` does: scrolls by an offset, in a coroutine, and gives back how far it went (Compose's
/// `suspend (Offset) -> Offset`). Equal when it is the same one.
#[derive(Clone)]
pub struct ScrollByOffsetAction(pub Rc<dyn Fn(Offset) -> Pin<Box<dyn Future<Output = Offset>>>>);

impl PartialEq for ScrollByOffsetAction {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for ScrollByOffsetAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Function")
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Merge policies

fn concatenate<T>(parent: Option<Vec<T>>, child: Vec<T>) -> Option<Vec<T>> {
    Some(match parent {
        Some(mut p) => {
            p.extend(child);
            p
        }
        None => child,
    })
}

fn unmergeable_pane_title(_: Option<String>, _: String) -> Option<String> {
    panic!("merge function called on unmergeable property PaneTitle.")
}

fn unmergeable_popup(_: Option<()>, _: ()) -> Option<()> {
    panic!(
        "merge function called on unmergeable property IsPopup. A popup should not be a child of a \
         clickable/focusable node."
    )
}

fn unmergeable_dialog(_: Option<()>, _: ()) -> Option<()> {
    panic!(
        "merge function called on unmergeable property IsDialog. A dialog should not be a child of a \
         clickable/focusable node."
    )
}

// ---------------------------------------------------------------------------------------------------------------
// The keys

/// The semantics properties (Compose's SemanticsProperties).
pub struct SemanticsProperties;

impl SemanticsProperties {
    /// What describes it to accessibility (an icon's label); a merging parent's are its own then its children's.
    pub const ContentDescription: SemanticsPropertyKey<Vec<String>> =
        SemanticsPropertyKey::accessibility_with_merge_policy("ContentDescription", concatenate);
    /// Its state, said (a switch's "on").
    pub const StateDescription: SemanticsPropertyKey<String> = SemanticsPropertyKey::accessibility("StateDescription");
    /// Its progress (a slider's, a progress bar's).
    pub const ProgressBarRangeInfo: SemanticsPropertyKey<ProgressBarRangeInfo> =
        SemanticsPropertyKey::accessibility("ProgressBarRangeInfo");
    /// The title of the pane it is (can't be merged).
    pub const PaneTitle: SemanticsPropertyKey<String> =
        SemanticsPropertyKey::accessibility_with_merge_policy("PaneTitle", unmergeable_pane_title);
    /// The selectables in it are one group.
    pub const SelectableGroup: SemanticsPropertyKey<()> = SemanticsPropertyKey::accessibility("SelectableGroup");
    pub const CollectionInfo: SemanticsPropertyKey<CollectionInfo> =
        SemanticsPropertyKey::accessibility("CollectionInfo");
    pub const CollectionItemInfo: SemanticsPropertyKey<CollectionItemInfo> =
        SemanticsPropertyKey::accessibility("CollectionItemInfo");
    /// It is a heading.
    pub const Heading: SemanticsPropertyKey<()> = SemanticsPropertyKey::accessibility("Heading");
    /// It is a key of a text entry keyboard.
    pub const TextEntryKey: SemanticsPropertyKey<()> = SemanticsPropertyKey::accessibility("TextEntryKey");
    /// It is disabled.
    pub const Disabled: SemanticsPropertyKey<()> = SemanticsPropertyKey::accessibility("Disabled");
    /// It is a live region: its changes are announced.
    pub const LiveRegion: SemanticsPropertyKey<LiveRegionMode> = SemanticsPropertyKey::accessibility("LiveRegion");
    /// Whether it has the focus (a focusable).
    pub const Focused: SemanticsPropertyKey<bool> = SemanticsPropertyKey::accessibility("Focused");
    /// Compose's deprecated name of `IsTraversalGroup`.
    pub const IsContainer: SemanticsPropertyKey<bool> = SemanticsPropertyKey::accessibility("IsContainer");
    /// Its descendants are read before what comes after it.
    pub const IsTraversalGroup: SemanticsPropertyKey<bool> = SemanticsPropertyKey::new("IsTraversalGroup");
    /// It shows sensitive data.
    pub const IsSensitiveData: SemanticsPropertyKey<bool> = SemanticsPropertyKey::new("IsSensitiveData");
    /// Compose's deprecated name of `HideFromAccessibility`.
    pub const InvisibleToUser: SemanticsPropertyKey<()> =
        SemanticsPropertyKey::with_merge_policy("InvisibleToUser", keep_parent_only);
    /// Accessibility leaves it out (a decoration).
    pub const HideFromAccessibility: SemanticsPropertyKey<()> =
        SemanticsPropertyKey::with_merge_policy("HideFromAccessibility", keep_parent_only);
    /// What it holds, for autofill.
    pub const ContentType: SemanticsPropertyKey<ContentType> =
        SemanticsPropertyKey::with_merge_policy("ContentType", keep_parent_only);
    /// The kind of data it holds, for autofill.
    pub const ContentDataType: SemanticsPropertyKey<ContentDataType> =
        SemanticsPropertyKey::with_merge_policy("ContentDataType", keep_parent_only);
    /// The data an autofill service sees in it.
    pub const FillableData: SemanticsPropertyKey<FillableData> =
        SemanticsPropertyKey::with_merge_policy("FillableData", keep_parent_only);
    /// Where it is read among its siblings (lower first; 0 by default).
    pub const TraversalIndex: SemanticsPropertyKey<f32> =
        SemanticsPropertyKey::with_merge_policy("TraversalIndex", keep_parent_only);
    pub const HorizontalScrollAxisRange: SemanticsPropertyKey<ScrollAxisRange> =
        SemanticsPropertyKey::accessibility("HorizontalScrollAxisRange");
    pub const VerticalScrollAxisRange: SemanticsPropertyKey<ScrollAxisRange> =
        SemanticsPropertyKey::accessibility("VerticalScrollAxisRange");
    /// It is a popup (can't be merged).
    pub const IsPopup: SemanticsPropertyKey<()> =
        SemanticsPropertyKey::accessibility_with_merge_policy("IsPopup", unmergeable_popup);
    /// It is a dialog (can't be merged).
    pub const IsDialog: SemanticsPropertyKey<()> =
        SemanticsPropertyKey::accessibility_with_merge_policy("IsDialog", unmergeable_dialog);
    /// What it is; a merging parent keeps its own.
    pub const Role: SemanticsPropertyKey<Role> =
        SemanticsPropertyKey::accessibility_with_merge_policy("Role", keep_parent_only);
    /// What tests find it by; a merging parent keeps its own.
    pub const TestTag: SemanticsPropertyKey<String> =
        SemanticsPropertyKey::with_merge_policy("TestTag", keep_parent_only);
    /// A link's marker, for tests.
    pub const LinkTestMarker: SemanticsPropertyKey<()> =
        SemanticsPropertyKey::with_merge_policy("LinkTestMarker", keep_parent_only);
    /// What it says; a merging parent's are its own then its children's.
    pub const Text: SemanticsPropertyKey<Vec<AnnotatedString>> =
        SemanticsPropertyKey::accessibility_with_merge_policy("Text", concatenate);
    /// A translation of its text.
    pub const TextSubstitution: SemanticsPropertyKey<AnnotatedString> = SemanticsPropertyKey::new("TextSubstitution");
    /// Whether the translation shows.
    pub const IsShowingTextSubstitution: SemanticsPropertyKey<bool> =
        SemanticsPropertyKey::new("IsShowingTextSubstitution");
    /// A field's text as typed (before its visual transformation).
    pub const InputText: SemanticsPropertyKey<AnnotatedString> = SemanticsPropertyKey::accessibility("InputText");
    /// A field's text as shown.
    pub const EditableText: SemanticsPropertyKey<AnnotatedString> = SemanticsPropertyKey::accessibility("EditableText");
    pub const TextSelectionRange: SemanticsPropertyKey<TextRange> =
        SemanticsPropertyKey::accessibility("TextSelectionRange");
    pub const TextCompositionRange: SemanticsPropertyKey<Option<TextRange>> =
        SemanticsPropertyKey::accessibility("TextCompositionRange");
    /// A field's input method action.
    pub const ImeAction: SemanticsPropertyKey<ImeAction> = SemanticsPropertyKey::accessibility("ImeAction");
    /// Whether it is selected (a tab, a list item).
    pub const Selected: SemanticsPropertyKey<bool> = SemanticsPropertyKey::accessibility("Selected");
    /// Whether it is on (a checkbox, a switch).
    pub const ToggleableState: SemanticsPropertyKey<ToggleableState> =
        SemanticsPropertyKey::accessibility("ToggleableState");
    pub const InputTextSuggestionState: SemanticsPropertyKey<InputTextSuggestionState> =
        SemanticsPropertyKey::accessibility("InputTextSuggestionState");
    /// It is a password field.
    pub const Password: SemanticsPropertyKey<()> = SemanticsPropertyKey::accessibility("Password");
    /// What is wrong with it (a field's input).
    pub const Error: SemanticsPropertyKey<String> = SemanticsPropertyKey::accessibility("Error");
    /// A lazy layout's key to index mapping.
    pub const IndexForKey: SemanticsPropertyKey<IndexForKey> = SemanticsPropertyKey::new("IndexForKey");
    /// Whether a field is editable.
    pub const IsEditable: SemanticsPropertyKey<bool> = SemanticsPropertyKey::new("IsEditable");
    /// The most characters a field takes.
    pub const MaxTextLength: SemanticsPropertyKey<i32> = SemanticsPropertyKey::new("MaxTextLength");
    /// Its shape, for tests; a merging parent keeps its own.
    pub const Shape: SemanticsPropertyKey<Arc<dyn Shape>> =
        SemanticsPropertyKey::with_merge_policy("Shape", keep_parent_only);
}

/// The semantics actions (Compose's SemanticsActions): what accessibility (and tests) can do to a node.
pub struct SemanticsActions;

impl SemanticsActions {
    /// Hands out its text layouts: the action adds them to the list it is given (Compose's GetTextLayoutResult).
    pub const GetTextLayoutResult: SemanticsPropertyKey<
        AccessibilityAction<dyn Fn(&mut Vec<TextLayoutResult>) -> bool>,
    > = action_key("GetTextLayoutResult");
    pub const OnClick: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("OnClick");
    pub const OnLongClick: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("OnLongClick");
    /// Scrolls by (x, y) px.
    pub const ScrollBy: SemanticsPropertyKey<AccessibilityAction<dyn Fn(f32, f32) -> bool>> = action_key("ScrollBy");
    /// Scrolls by an offset in a coroutine, giving back how far it went.
    pub const ScrollByOffset: SemanticsPropertyKey<ScrollByOffsetAction> = SemanticsPropertyKey::new("ScrollByOffset");
    pub const ScrollToIndex: SemanticsPropertyKey<AccessibilityAction<dyn Fn(i32) -> bool>> =
        action_key("ScrollToIndex");
    /// Fills in autofill data.
    pub const OnFillData: SemanticsPropertyKey<AccessibilityAction<dyn Fn(FillableData) -> bool>> =
        action_key("OnFillData");
    pub const SetProgress: SemanticsPropertyKey<AccessibilityAction<dyn Fn(f32) -> bool>> = action_key("SetProgress");
    /// Selects from a start to an end (whether relative to the original text).
    pub const SetSelection: SemanticsPropertyKey<AccessibilityAction<dyn Fn(i32, i32, bool) -> bool>> =
        action_key("SetSelection");
    pub const SetText: SemanticsPropertyKey<AccessibilityAction<dyn Fn(AnnotatedString) -> bool>> =
        action_key("SetText");
    pub const SetTextSubstitution: SemanticsPropertyKey<AccessibilityAction<dyn Fn(AnnotatedString) -> bool>> =
        action_key("SetTextSubstitution");
    pub const ShowTextSubstitution: SemanticsPropertyKey<AccessibilityAction<dyn Fn(bool) -> bool>> =
        action_key("ShowTextSubstitution");
    pub const ClearTextSubstitution: SemanticsPropertyKey<AccessibilityAction<ActionFn>> =
        action_key("ClearTextSubstitution");
    pub const InsertTextAtCursor: SemanticsPropertyKey<AccessibilityAction<dyn Fn(AnnotatedString) -> bool>> =
        action_key("InsertTextAtCursor");
    /// Performs a field's input method action (its key named `PerformImeAction`, as Compose's).
    pub const OnImeAction: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("PerformImeAction");
    pub const CopyText: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("CopyText");
    pub const CutText: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("CutText");
    pub const PasteText: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("PasteText");
    pub const Expand: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("Expand");
    pub const Collapse: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("Collapse");
    pub const Dismiss: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("Dismiss");
    pub const RequestFocus: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("RequestFocus");
    /// Its own actions; a merging parent's are its own then its children's.
    pub const CustomActions: SemanticsPropertyKey<Vec<CustomAccessibilityAction>> =
        SemanticsPropertyKey::accessibility_with_merge_policy("CustomActions", concatenate);
    pub const PageUp: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("PageUp");
    pub const PageLeft: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("PageLeft");
    pub const PageDown: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("PageDown");
    pub const PageRight: SemanticsPropertyKey<AccessibilityAction<ActionFn>> = action_key("PageRight");
    /// Hands out the length of a scrollable's viewport.
    pub const GetScrollViewportLength: SemanticsPropertyKey<AccessibilityAction<dyn Fn(&mut Vec<f32>) -> bool>> =
        action_key("GetScrollViewportLength");
}

// ---------------------------------------------------------------------------------------------------------------
// The setters (Compose's SemanticsPropertyReceiver extensions)

fn action0(label: Option<String>, action: impl Fn() -> bool + 'static) -> AccessibilityAction<ActionFn> {
    AccessibilityAction { label, action: Some(Rc::new(action)) }
}

impl SemanticsConfiguration {
    /// `contentDescription = …`.
    pub fn set_content_description(&mut self, description: impl Into<String>) {
        self.set(SemanticsProperties::ContentDescription, vec![description.into()]);
    }
    /// `stateDescription = …`.
    pub fn set_state_description(&mut self, description: impl Into<String>) {
        self.set(SemanticsProperties::StateDescription, description.into());
    }
    /// `progressBarRangeInfo = …`.
    pub fn set_progress_bar_range_info(&mut self, info: ProgressBarRangeInfo) {
        self.set(SemanticsProperties::ProgressBarRangeInfo, info);
    }
    pub fn heading(&mut self) {
        self.set(SemanticsProperties::Heading, ());
    }
    pub fn text_entry_key(&mut self) {
        self.set(SemanticsProperties::TextEntryKey, ());
    }
    /// `paneTitle = …`.
    pub fn set_pane_title(&mut self, title: impl Into<String>) {
        self.set(SemanticsProperties::PaneTitle, title.into());
    }
    pub fn disabled(&mut self) {
        self.set(SemanticsProperties::Disabled, ());
    }
    /// `liveRegion = …`.
    pub fn set_live_region(&mut self, mode: LiveRegionMode) {
        self.set(SemanticsProperties::LiveRegion, mode);
    }
    /// `focused = …`.
    pub fn set_focused(&mut self, focused: bool) {
        self.set(SemanticsProperties::Focused, focused);
    }
    /// `isContainer = …` (Compose's deprecated name of `isTraversalGroup`).
    pub fn set_is_container(&mut self, is_container: bool) {
        self.set(SemanticsProperties::IsContainer, is_container);
    }
    /// `isTraversalGroup = …`.
    pub fn set_is_traversal_group(&mut self, is_traversal_group: bool) {
        self.set(SemanticsProperties::IsTraversalGroup, is_traversal_group);
    }
    /// `isSensitiveData = …`.
    pub fn set_is_sensitive_data(&mut self, is_sensitive: bool) {
        self.set(SemanticsProperties::IsSensitiveData, is_sensitive);
    }
    /// Compose's deprecated `invisibleToUser()`.
    pub fn invisible_to_user(&mut self) {
        self.set(SemanticsProperties::InvisibleToUser, ());
    }
    pub fn hide_from_accessibility(&mut self) {
        self.set(SemanticsProperties::HideFromAccessibility, ());
    }
    /// `contentType = …`.
    pub fn set_content_type(&mut self, content_type: ContentType) {
        self.set(SemanticsProperties::ContentType, content_type);
    }
    /// `contentDataType = …`.
    pub fn set_content_data_type(&mut self, data_type: ContentDataType) {
        self.set(SemanticsProperties::ContentDataType, data_type);
    }
    /// `fillableData = …`.
    pub fn set_fillable_data(&mut self, data: FillableData) {
        self.set(SemanticsProperties::FillableData, data);
    }
    /// `traversalIndex = …`.
    pub fn set_traversal_index(&mut self, index: f32) {
        self.set(SemanticsProperties::TraversalIndex, index);
    }
    /// `horizontalScrollAxisRange = …`.
    pub fn set_horizontal_scroll_axis_range(&mut self, range: ScrollAxisRange) {
        self.set(SemanticsProperties::HorizontalScrollAxisRange, range);
    }
    /// `verticalScrollAxisRange = …`.
    pub fn set_vertical_scroll_axis_range(&mut self, range: ScrollAxisRange) {
        self.set(SemanticsProperties::VerticalScrollAxisRange, range);
    }
    pub fn popup(&mut self) {
        self.set(SemanticsProperties::IsPopup, ());
    }
    pub fn dialog(&mut self) {
        self.set(SemanticsProperties::IsDialog, ());
    }
    /// `role = …`.
    pub fn set_role(&mut self, role: Role) {
        self.set(SemanticsProperties::Role, role);
    }
    /// `testTag = …`.
    pub fn set_test_tag(&mut self, tag: impl Into<String>) {
        self.set(SemanticsProperties::TestTag, tag.into());
    }
    /// `text = …`: what it says.
    pub fn set_text(&mut self, text: impl Into<AnnotatedString>) {
        self.set(SemanticsProperties::Text, vec![text.into()]);
    }
    /// `textSubstitution = …`.
    pub fn set_text_substitution(&mut self, text: impl Into<AnnotatedString>) {
        self.set(SemanticsProperties::TextSubstitution, text.into());
    }
    /// `isShowingTextSubstitution = …`.
    pub fn set_is_showing_text_substitution(&mut self, showing: bool) {
        self.set(SemanticsProperties::IsShowingTextSubstitution, showing);
    }
    /// `inputText = …`.
    pub fn set_input_text(&mut self, text: impl Into<AnnotatedString>) {
        self.set(SemanticsProperties::InputText, text.into());
    }
    /// `editableText = …`.
    pub fn set_editable_text(&mut self, text: impl Into<AnnotatedString>) {
        self.set(SemanticsProperties::EditableText, text.into());
    }
    /// `textSelectionRange = …`.
    pub fn set_text_selection_range(&mut self, range: TextRange) {
        self.set(SemanticsProperties::TextSelectionRange, range);
    }
    /// `textCompositionRange = …`.
    pub fn set_text_composition_range(&mut self, range: Option<TextRange>) {
        self.set(SemanticsProperties::TextCompositionRange, range);
    }
    /// `imeAction = …` (Compose has `onImeAction` set it).
    pub fn set_ime_action(&mut self, action: ImeAction) {
        self.set(SemanticsProperties::ImeAction, action);
    }
    /// `selected = …`.
    pub fn set_selected(&mut self, selected: bool) {
        self.set(SemanticsProperties::Selected, selected);
    }
    /// `collectionInfo = …`.
    pub fn set_collection_info(&mut self, info: CollectionInfo) {
        self.set(SemanticsProperties::CollectionInfo, info);
    }
    /// `collectionItemInfo = …`.
    pub fn set_collection_item_info(&mut self, info: CollectionItemInfo) {
        self.set(SemanticsProperties::CollectionItemInfo, info);
    }
    /// `toggleableState = …`.
    pub fn set_toggleable_state(&mut self, state: ToggleableState) {
        self.set(SemanticsProperties::ToggleableState, state);
    }
    /// `inputTextSuggestionState = …`.
    pub fn set_input_text_suggestion_state(&mut self, state: InputTextSuggestionState) {
        self.set(SemanticsProperties::InputTextSuggestionState, state);
    }
    /// `isEditable = …`.
    pub fn set_is_editable(&mut self, editable: bool) {
        self.set(SemanticsProperties::IsEditable, editable);
    }
    pub fn password(&mut self) {
        self.set(SemanticsProperties::Password, ());
    }
    pub fn error(&mut self, description: impl Into<String>) {
        self.set(SemanticsProperties::Error, description.into());
    }
    pub fn index_for_key(&mut self, mapping: impl Fn(&dyn Any) -> i32 + 'static) {
        self.set(SemanticsProperties::IndexForKey, IndexForKey(Rc::new(mapping)));
    }
    /// `maxTextLength = …`.
    pub fn set_max_text_length(&mut self, length: i32) {
        self.set(SemanticsProperties::MaxTextLength, length);
    }
    /// `shape = …`.
    pub fn set_shape(&mut self, shape: Arc<dyn Shape>) {
        self.set(SemanticsProperties::Shape, shape);
    }
    pub fn selectable_group(&mut self) {
        self.set(SemanticsProperties::SelectableGroup, ());
    }
    /// `customActions = …`.
    pub fn set_custom_actions(&mut self, actions: Vec<CustomAccessibilityAction>) {
        self.set(SemanticsActions::CustomActions, actions);
    }
    /// `scrollByOffset(…)`.
    pub fn scroll_by_offset<Fut: Future<Output = Offset> + 'static>(
        &mut self,
        action: impl Fn(Offset) -> Fut + 'static,
    ) {
        let action: Rc<dyn Fn(Offset) -> Pin<Box<dyn Future<Output = Offset>>>> = Rc::new(move |o| Box::pin(action(o)));
        self.set(SemanticsActions::ScrollByOffset, ScrollByOffsetAction(action));
    }
    /// `getScrollViewportLength(…)`: `action` gives the length, none if it can't.
    pub fn get_scroll_viewport_length(&mut self, label: Option<String>, action: impl Fn() -> Option<f32> + 'static) {
        let f: Rc<dyn Fn(&mut Vec<f32>) -> bool> = Rc::new(move |out| match action() {
            Some(v) => {
                out.push(v);
                true
            }
            None => false,
        });
        self.set(SemanticsActions::GetScrollViewportLength, AccessibilityAction { label, action: Some(f) });
    }
}

#[named]
impl SemanticsConfiguration {
    pub fn get_text_layout_result(
        &mut self,
        #[default(None)] label: Option<String>,
        action: impl Fn(&mut Vec<TextLayoutResult>) -> bool + 'static,
    ) {
        self.set(SemanticsActions::GetTextLayoutResult, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    pub fn on_click(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::OnClick, action0(label, action));
    }
    pub fn on_long_click(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::OnLongClick, action0(label, action));
    }
    pub fn scroll_by(&mut self, #[default(None)] label: Option<String>, action: impl Fn(f32, f32) -> bool + 'static) {
        self.set(SemanticsActions::ScrollBy, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    pub fn scroll_to_index(&mut self, #[default(None)] label: Option<String>, action: impl Fn(i32) -> bool + 'static) {
        self.set(SemanticsActions::ScrollToIndex, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    pub fn on_fill_data(
        &mut self,
        #[default(None)] label: Option<String>,
        action: impl Fn(FillableData) -> bool + 'static,
    ) {
        self.set(SemanticsActions::OnFillData, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    pub fn set_progress(&mut self, #[default(None)] label: Option<String>, action: impl Fn(f32) -> bool + 'static) {
        self.set(SemanticsActions::SetProgress, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    /// Compose's `setText(label, action)`.
    pub fn set_text_action(
        &mut self,
        #[default(None)] label: Option<String>,
        action: impl Fn(AnnotatedString) -> bool + 'static,
    ) {
        self.set(SemanticsActions::SetText, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    /// Compose's `setTextSubstitution(label, action)`.
    pub fn set_text_substitution_action(
        &mut self,
        #[default(None)] label: Option<String>,
        action: impl Fn(AnnotatedString) -> bool + 'static,
    ) {
        self.set(SemanticsActions::SetTextSubstitution, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    pub fn show_text_substitution(
        &mut self,
        #[default(None)] label: Option<String>,
        action: impl Fn(bool) -> bool + 'static,
    ) {
        self.set(SemanticsActions::ShowTextSubstitution, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    pub fn clear_text_substitution(
        &mut self,
        #[default(None)] label: Option<String>,
        action: impl Fn() -> bool + 'static,
    ) {
        self.set(SemanticsActions::ClearTextSubstitution, action0(label, action));
    }
    pub fn insert_text_at_cursor(
        &mut self,
        #[default(None)] label: Option<String>,
        action: impl Fn(AnnotatedString) -> bool + 'static,
    ) {
        self.set(SemanticsActions::InsertTextAtCursor, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    /// `onImeAction(imeActionType, label, action)`: the action key's kind and what it does.
    pub fn on_ime_action(
        &mut self,
        ime_action_type: ImeAction,
        #[default(None)] label: Option<String>,
        action: impl Fn() -> bool + 'static,
    ) {
        self.set(SemanticsProperties::ImeAction, ime_action_type);
        self.set(SemanticsActions::OnImeAction, action0(label, action));
    }
    pub fn set_selection(
        &mut self,
        #[default(None)] label: Option<String>,
        action: impl Fn(i32, i32, bool) -> bool + 'static,
    ) {
        self.set(SemanticsActions::SetSelection, AccessibilityAction { label, action: Some(Rc::new(action)) });
    }
    pub fn copy_text(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::CopyText, action0(label, action));
    }
    pub fn cut_text(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::CutText, action0(label, action));
    }
    pub fn paste_text(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::PasteText, action0(label, action));
    }
    pub fn expand(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::Expand, action0(label, action));
    }
    pub fn collapse(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::Collapse, action0(label, action));
    }
    pub fn dismiss(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::Dismiss, action0(label, action));
    }
    pub fn request_focus(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::RequestFocus, action0(label, action));
    }
    pub fn page_up(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::PageUp, action0(label, action));
    }
    pub fn page_down(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::PageDown, action0(label, action));
    }
    pub fn page_left(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::PageLeft, action0(label, action));
    }
    pub fn page_right(&mut self, #[default(None)] label: Option<String>, action: impl Fn() -> bool + 'static) {
        self.set(SemanticsActions::PageRight, action0(label, action));
    }
}
