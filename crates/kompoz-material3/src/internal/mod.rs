//! What material3's components share (Compose's `androidx.compose.material3.internal`).

// (what the components share, used as they come)
#![allow(dead_code)]

pub(crate) mod accessibility_service_state_provider;
pub(crate) mod accessibility_util;
pub(crate) mod animated_shape;
pub(crate) mod back_handler;
// (WP6: pickers)
pub(crate) mod basic_edge_to_edge_dialog;
pub(crate) mod basic_tooltip;
pub(crate) mod calendar_model;
pub(crate) mod child_parent_semantics;
pub(crate) mod circular_wavy_progress_modifiers;
pub(crate) mod drag_gesture_detector_copy;
pub(crate) mod draggable_anchors;
pub(crate) mod elevation;
pub(crate) mod float_producer;
pub(crate) mod icons;
pub(crate) mod interactions; // (WP1b: toggle_split)
pub(crate) mod internal_mutator_mutex;
pub(crate) mod kotlinx_datetime_calendar_model;
pub(crate) mod l10n;
pub(crate) mod layout_util;
pub(crate) mod linear_wavy_progress_modifiers;
pub(crate) mod mapped_interaction_source;
pub(crate) mod menu_position;
pub(crate) mod mutable_window_insets;
pub(crate) mod platform_date_format;
pub(crate) mod platform_optimized_cancellation_exception;
pub(crate) mod provide_content_color_text_style;
pub(crate) mod shape_util;
pub(crate) mod strings;
pub(crate) mod system;
pub(crate) mod system_bars_default_insets;
pub(crate) mod text_field_impl;
