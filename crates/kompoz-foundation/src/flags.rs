//! Compose's `ComposeFoundationFlags`: the switches of foundation's behaviour under change. KompozR follows each
//! flag at its Compose 1.12.1 default, fixed where the code says so; the one an app may switch is here.

use std::sync::atomic::{AtomicBool, Ordering};

static INHERITED_TEXT_STYLE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Compose's `ComposeFoundationFlags` (an `object` of mutable flags).
pub struct ComposeFoundationFlags;

impl ComposeFoundationFlags {
    /// Whether `BasicText` inherits the text properties of the styles of the layouts it is in
    /// ([`styleable`](crate::style::StyleModifiers::styleable)'s `font_size`, `content_color`…; Compose's
    /// `isInheritedTextStyleEnabled`, off by default): its own style's specified properties win.
    pub fn is_inherited_text_style_enabled() -> bool {
        INHERITED_TEXT_STYLE_ENABLED.load(Ordering::Relaxed)
    }

    /// Switches it (Compose's `ComposeFoundationFlags.isInheritedTextStyleEnabled = value`).
    pub fn set_is_inherited_text_style_enabled(value: bool) {
        INHERITED_TEXT_STYLE_ENABLED.store(value, Ordering::Relaxed);
    }
}
