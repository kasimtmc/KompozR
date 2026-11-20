//! Insets set as the app likes, as Compose's internal MutableWindowInsets.kt: material3's copy of foundation's
//! `MutableWindowInsets` (its `insets` a state read through its four sides). kompoz-foundation's
//! [`MutableWindowInsets`] is the same class (a state of the insets, `WindowInsets` of none to start with, its
//! readers told when set; [`as_window_insets`](MutableWindowInsets::as_window_insets) the `WindowInsets` it is):
//! it is used as it is.

#[allow(unused_imports)] // (used as the components come)
pub(crate) use kompoz_foundation::layout::MutableWindowInsets;
