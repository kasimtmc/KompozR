//! Material's feature flags, as Compose's ComposeMaterial3Flags.kt: behaviour fixes and features an app turns on or
//! off for the whole process (at its start, before composing), each at Compose's default.

use std::sync::atomic::{AtomicBool, Ordering};

static CHECKBOX_STYLING_FIX: AtomicBool = AtomicBool::new(false);
static SNACKBAR_STYLING_FIX: AtomicBool = AtomicBool::new(false);
static PRECISION_POINTER_COMPONENT_SIZING: AtomicBool = AtomicBool::new(false);
static ANCHORED_DRAGGABLE_STRICT_OFFSET_CHECK: AtomicBool = AtomicBool::new(true);
static ANCHORED_DRAGGABLE_INVALIDATION_FIX: AtomicBool = AtomicBool::new(true);
static ANCHORED_DRAGGABLE_ANCHOR_RECOVERY: AtomicBool = AtomicBool::new(true);

/// Material's process-wide flags (Compose's `object ComposeMaterial3Flags`): read
/// `ComposeMaterial3Flags::is_checkbox_styling_fix_enabled()`, set with `set_is_checkbox_styling_fix_enabled(true)`
/// (Kotlin: `ComposeMaterial3Flags.isCheckboxStylingFixEnabled = true`).
pub struct ComposeMaterial3Flags;

impl ComposeMaterial3Flags {
    /// The checkbox's corrected stroke and checkmark styling (off by default).
    pub fn is_checkbox_styling_fix_enabled() -> bool {
        CHECKBOX_STYLING_FIX.load(Ordering::Relaxed)
    }
    pub fn set_is_checkbox_styling_fix_enabled(enabled: bool) {
        CHECKBOX_STYLING_FIX.store(enabled, Ordering::Relaxed);
    }

    /// The snackbar's corrected text styling (off by default).
    pub fn is_snackbar_styling_fix_enabled() -> bool {
        SNACKBAR_STYLING_FIX.load(Ordering::Relaxed)
    }
    pub fn set_is_snackbar_styling_fix_enabled(enabled: bool) {
        SNACKBAR_STYLING_FIX.store(enabled, Ordering::Relaxed);
    }

    /// Components taking smaller sizes for a mouse and keyboard (off by default; see
    /// [`should_use_precision_pointer_component_sizing`](crate::should_use_precision_pointer_component_sizing)).
    pub fn is_precision_pointer_component_sizing_enabled() -> bool {
        PRECISION_POINTER_COMPONENT_SIZING.load(Ordering::Relaxed)
    }
    pub fn set_is_precision_pointer_component_sizing_enabled(enabled: bool) {
        PRECISION_POINTER_COMPONENT_SIZING.store(enabled, Ordering::Relaxed);
    }

    /// The anchored-draggable components' strict check of their offset before it is set (on by default).
    pub fn is_anchored_draggable_components_strict_offset_check_enabled() -> bool {
        ANCHORED_DRAGGABLE_STRICT_OFFSET_CHECK.load(Ordering::Relaxed)
    }
    pub fn set_is_anchored_draggable_components_strict_offset_check_enabled(enabled: bool) {
        ANCHORED_DRAGGABLE_STRICT_OFFSET_CHECK.store(enabled, Ordering::Relaxed);
    }

    /// The anchored-draggable components' invalidation fix (on by default).
    pub fn is_anchored_draggable_components_invalidation_fix_enabled() -> bool {
        ANCHORED_DRAGGABLE_INVALIDATION_FIX.load(Ordering::Relaxed)
    }
    pub fn set_is_anchored_draggable_components_invalidation_fix_enabled(enabled: bool) {
        ANCHORED_DRAGGABLE_INVALIDATION_FIX.store(enabled, Ordering::Relaxed);
    }

    /// The anchored-draggable components finding their anchor again after their anchors change (on by default).
    pub fn is_anchored_draggable_components_anchor_recovery_enabled() -> bool {
        ANCHORED_DRAGGABLE_ANCHOR_RECOVERY.load(Ordering::Relaxed)
    }
    pub fn set_is_anchored_draggable_components_anchor_recovery_enabled(enabled: bool) {
        ANCHORED_DRAGGABLE_ANCHOR_RECOVERY.store(enabled, Ordering::Relaxed);
    }
}
