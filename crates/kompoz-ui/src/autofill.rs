//! Autofill (Compose's AutofillManager and Android's AndroidAutofillManager over the platform's): what fields are
//! there to fill is read from semantics — a node whose content data type isn't None and that takes `OnFillData` —
//! and told to the platform ([`PlatformCallbacks`](crate::platform::PlatformCallbacks)'s `autofill_*`): a field
//! coming into the layout or going (visibility), its value changing, the focus entering or leaving it (with its
//! bounds), the form sent or dropped ([`AutofillManager::commit`]/[`cancel`](AutofillManager::cancel), or
//! committed once no field of a content type is left), a field asking to be filled. What a service fills in comes
//! back through [`KompozHost::perform_autofill`](crate::host::KompozHost::perform_autofill) (`OnFillData`).

use crate::node::LayoutNode;
use crate::platform::PlatformHandle;
use crate::semantics::{
    ContentDataType, FillableData, SemanticsActions, SemanticsConfiguration, SemanticsListener, SemanticsProperties,
    ToggleableState,
};
use kompoz_graphics::Rect;
use kompoz_runtime::KompozitionLocal;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};

/// The platform's autofill as the app reaches it (Compose's AutofillManager, `LocalAutofillManager.current`):
/// commit a form (its credentials may be saved) or cancel it. Clones are the same manager.
#[derive(Clone)]
pub struct AutofillManager(pub(crate) Weak<HostAutofillManager>);

impl AutofillManager {
    /// The form was sent: the platform may save what was entered (Compose's `commit`).
    pub fn commit(&self) {
        if let Some(m) = self.0.upgrade() {
            m.commit();
        }
    }

    /// The form was dropped: the platform forgets this session (Compose's `cancel`).
    pub fn cancel(&self) {
        if let Some(m) = self.0.upgrade() {
            m.cancel();
        }
    }
}

impl PartialEq for AutofillManager {
    fn eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }
}

/// The autofill manager of the host (Compose's LocalAutofillManager): none outside a host.
pub static LocalAutofillManager: KompozitionLocal<Option<AutofillManager>> = KompozitionLocal::new_static(|| None);

/// Whether a node can be filled (Compose's `isAutofillable`): a data type other than None, an `OnFillData`.
fn is_autofillable(config: &SemanticsConfiguration) -> bool {
    config.get(SemanticsProperties::ContentDataType) != Some(ContentDataType::None)
        && config.contains(SemanticsActions::OnFillData)
}

/// Whether a node counts for the automatic commit (Compose's `isRelatedToAutoCommit`): it says what it holds.
fn is_related_to_auto_commit(config: &SemanticsConfiguration) -> bool {
    config.contains(SemanticsProperties::ContentType)
}

/// Fillable data as the platform's autofill value (Compose's AndroidFillableData's AutofillValue): a text as it
/// is, a toggle as "true"/"false", a list index and a date (milliseconds) as numbers.
fn autofill_value_of(data: &FillableData) -> String {
    match data {
        FillableData::Text(t) => t.clone(),
        FillableData::Boolean(b) => b.to_string(),
        FillableData::ListIndex(i) => i.to_string(),
        FillableData::Date(d) => d.to_string(),
    }
}

/// The host's side of autofill (Compose's AndroidAutofillManager): told of semantics changes, of the focus
/// moving, of nodes attached and detached, of the end of applying a composition's changes.
pub(crate) struct HostAutofillManager {
    platform: PlatformHandle,
    /// The nodes of a content type in the layout (Compose's `currentlyDisplayedIDs`).
    currently_displayed_ids: RefCell<HashSet<u64>>,
    pending_autofill_commit: Cell<bool>,
    /// What waits for the semantics to be read (a focus change, a node attached): told as the frame's semantics
    /// changes are, when no node is in use (Compose reads its kept configurations at once).
    pending: RefCell<Vec<Pending>>,
}

/// A notification waiting for the semantics.
enum Pending {
    /// The node the focus left, if it was fillable (by its kept semantics: it may be gone by then), and the
    /// one it went to.
    Focus(Option<u64>, Option<Weak<LayoutNode>>),
    Attach(Weak<LayoutNode>),
}

impl HostAutofillManager {
    pub(crate) fn new(platform: PlatformHandle) -> Rc<HostAutofillManager> {
        Rc::new(HostAutofillManager {
            platform,
            currently_displayed_ids: RefCell::new(HashSet::new()),
            pending_autofill_commit: Cell::new(false),
            pending: RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn commit(&self) {
        self.platform.get().autofill_done(true);
    }

    pub(crate) fn cancel(&self) {
        self.platform.get().autofill_done(false);
    }

    /// The focus moved from `previous`'s layout to `current`'s: told with the frame's semantics changes.
    pub(crate) fn queue_focus_changed(&self, previous: Option<&Rc<LayoutNode>>, current: Option<&Rc<LayoutNode>>) {
        // The semantics it has kept (not worked out again here: a node may be in use, or leaving).
        let exited = previous.filter(|p| {
            p.semantics_configuration.try_borrow().ok().and_then(|c| c.as_ref().map(is_autofillable)).unwrap_or(false)
        });
        self.pending.borrow_mut().push(Pending::Focus(exited.map(|p| p.id), current.map(Rc::downgrade)));
    }

    /// A node came into the layout: told with the frame's semantics changes.
    pub(crate) fn queue_post_attach(&self, node: &Rc<LayoutNode>) {
        self.pending.borrow_mut().push(Pending::Attach(Rc::downgrade(node)));
    }

    /// Tells what waited (the owner calls it as it dispatches the semantics changes).
    pub(crate) fn flush(&self) {
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        for p in pending {
            match p {
                Pending::Focus(exited, current) => {
                    if let Some(id) = exited {
                        self.platform.get().autofill_focus(id, Rect::Zero, false);
                    }
                    let current = current.and_then(|w| w.upgrade()).filter(|n| n.is_attached());
                    self.on_focus_changed(None, current.as_ref());
                }
                Pending::Attach(node) => {
                    if let Some(node) = node.upgrade().filter(|n| n.is_attached()) {
                        self.on_post_attach(&node);
                    }
                }
            }
        }
    }

    /// The focus moved from `previous`'s layout to `current`'s (Compose's `onFocusChanged`): the platform told it
    /// left a fillable node and entered one (with its bounds in the window).
    pub(crate) fn on_focus_changed(&self, previous: Option<&Rc<LayoutNode>>, current: Option<&Rc<LayoutNode>>) {
        if let Some(p) = previous
            && p.semantics_configuration().is_some_and(|c| is_autofillable(&c))
        {
            self.platform.get().autofill_focus(p.id, Rect::Zero, false);
        }
        if let Some(c) = current
            && c.semantics_configuration().is_some_and(|c| is_autofillable(&c))
        {
            let bounds = c.coordinates().bounds_in_window();
            self.platform.get().autofill_focus(c.id, bounds, true);
        }
    }

    /// A field asks to be filled (Compose's `requestAutofill`): the platform's autofill for it, at its bounds.
    pub(crate) fn request_autofill(&self, node: &Rc<LayoutNode>) {
        let bounds = node.coordinates().bounds_in_window();
        self.platform.get().autofill_request(node.id, bounds);
    }

    /// A node came into the layout (Compose's `onPostAttach`): one of a content type is shown.
    pub(crate) fn on_post_attach(&self, node: &LayoutNode) {
        if node.semantics_configuration().is_some_and(|c| is_related_to_auto_commit(&c)) {
            self.currently_displayed_ids.borrow_mut().insert(node.id);
            self.platform.get().autofill_visibility(node.id, true);
        }
    }

    /// A node left the layout (Compose's `onDetach`): one of a content type is gone.
    pub(crate) fn on_detach(&self, node: &LayoutNode) {
        if self.currently_displayed_ids.borrow_mut().remove(&node.id) {
            self.platform.get().autofill_visibility(node.id, false);
        }
    }

    /// The composition's changes were applied (Compose's `onEndApplyChanges`): once no node of a content type is
    /// left, the form is committed.
    pub(crate) fn on_end_apply_changes(&self) {
        let empty = self.currently_displayed_ids.borrow().is_empty();
        if empty && self.pending_autofill_commit.get() {
            self.platform.get().autofill_done(true);
            self.pending_autofill_commit.set(false);
        }
        if !empty {
            self.pending_autofill_commit.set(true);
        }
    }

    /// What a semantics change tells the platform (Compose's `notifyAutofillManager`).
    fn notify_autofill_manager(
        &self,
        config: Option<&SemanticsConfiguration>,
        previous: Option<&SemanticsConfiguration>,
        id: u64,
    ) {
        let platform = self.platform.get();
        let previous_data_type = previous.and_then(|c| c.get(SemanticsProperties::ContentDataType));
        let data_type = config.and_then(|c| c.get(SemanticsProperties::ContentDataType));
        if data_type == Some(ContentDataType::None) {
            if previous_data_type != Some(ContentDataType::None) {
                platform.autofill_visibility(id, false);
            }
            // Not autofillable any more: nothing else.
            return;
        }
        if previous_data_type == Some(ContentDataType::None) {
            platform.autofill_visibility(id, true);
        }
        // The input text.
        let previous_text = previous.and_then(|c| c.get(SemanticsProperties::InputText)).map(|t| t.text().to_string());
        let new_text = config.and_then(|c| c.get(SemanticsProperties::InputText)).map(|t| t.text().to_string());
        if previous_text != new_text {
            match (&previous_text, &new_text) {
                (None, _) => platform.autofill_visibility(id, true),
                (_, None) => platform.autofill_visibility(id, false),
                (_, Some(text)) => {
                    if data_type == Some(ContentDataType::Text) {
                        platform.autofill_value(id, text);
                    }
                }
            }
        }
        // A toggle's value.
        let previous_toggle = previous.and_then(|c| c.get(SemanticsProperties::ToggleableState));
        let new_toggle = config.and_then(|c| c.get(SemanticsProperties::ToggleableState));
        if previous_toggle != new_toggle {
            match (previous_toggle, new_toggle) {
                (None, _) => platform.autofill_visibility(id, true),
                (_, None) => platform.autofill_visibility(id, false),
                (_, Some(toggle)) => {
                    if data_type == Some(ContentDataType::Toggle) && toggle != ToggleableState::Indeterminate {
                        platform.autofill_value(id, if toggle == ToggleableState::On { "true" } else { "false" });
                    }
                }
            }
        }
        // The fillable data.
        let previous_data = previous.and_then(|c| c.get(SemanticsProperties::FillableData));
        let new_data = config.and_then(|c| c.get(SemanticsProperties::FillableData));
        if previous_data != new_data {
            match (&previous_data, &new_data) {
                (None, _) => platform.autofill_visibility(id, true),
                (_, None) => platform.autofill_visibility(id, false),
                (_, Some(data)) => platform.autofill_value(id, &autofill_value_of(data)),
            }
        }
    }
}

impl SemanticsListener for HostAutofillManager {
    /// A node's semantics changed (Compose's `onSemanticsChanged`): the platform told, and the nodes of a content
    /// type kept.
    fn on_semantics_changed(&self, node: &Rc<LayoutNode>, previous: Option<&SemanticsConfiguration>) {
        let config = node.semantics_configuration();
        self.notify_autofill_manager(config.as_ref(), previous, node.id);
        let previous_related = previous.is_some_and(is_related_to_auto_commit);
        let current_related = config.as_ref().is_some_and(is_related_to_auto_commit);
        if previous_related != current_related {
            if current_related {
                self.currently_displayed_ids.borrow_mut().insert(node.id);
            } else {
                self.currently_displayed_ids.borrow_mut().remove(&node.id);
            }
        }
    }
}
