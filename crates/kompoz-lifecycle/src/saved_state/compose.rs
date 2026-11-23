//! savedstate-compose's LocalSavedStateRegistryOwner.kt (its non-Android actual). Its serializers
//! (`MutableStateSerializer`, `SnapshotStateListSerializer`, `SnapshotStateMapSerializer`) are KompozR's
//! [`Saveable`](kompoz_runtime::Saveable) impls of those states (kompoz-runtime).

use super::SavedStateRegistryOwner;
use kompoz_runtime::KompozitionLocal;
use std::rc::Rc;

/// The nearest saved state registry owner (Compose's `LocalSavedStateRegistryOwner`): the host's (the app's) unless
/// a navigation entry gives its own below. A static local; not provided, reading it panics ("CompositionLocal
/// LocalSavedStateRegistryOwner not present").
pub static LocalSavedStateRegistryOwner: KompozitionLocal<Rc<dyn SavedStateRegistryOwner>> =
    KompozitionLocal::new_static(|| panic!("CompositionLocal LocalSavedStateRegistryOwner not present"));
