//! navigation3-runtime's `androidx.navigation3.runtime`: [`NavKey`], [`NavEntry`], [`NavEntryDecorator`]s
//! ([`SaveableStateHolderNavEntryDecorator`]), [`remember_decorated_nav_entries`], the [`entry_provider`] DSL,
//! [`NavBackStack`] and [`remember_nav_back_stack`], [`metadata`]; [`deeplink`] and the results ([`ResultEventBus`]).

mod back_stack;
mod decorated;
pub mod deeplink;
mod entry_dsl;
mod nav_entry;
mod nav_key;
mod nav_metadata;
mod result;
mod saveable_decorator;

pub use back_stack::*;
pub use decorated::*;
pub use entry_dsl::*;
pub use nav_entry::*;
pub use nav_key::*;
pub use nav_metadata::*;
pub use result::*;
pub(crate) use saveable_decorator::saveable_key;
pub use saveable_decorator::*;
