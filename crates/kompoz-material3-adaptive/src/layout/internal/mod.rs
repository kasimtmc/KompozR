//! Compose's `androidx.compose.material3.adaptive.layout.internal` (adaptive-layout 1.2.0, Compose for desktop's
//! actuals): the layout's translated texts ([`strings`], generated [`l10n`]), the semantics modifier whose properties
//! read composition locals and launch coroutines ([`delegable_modifier`]), the remember helpers
//! ([`remember_utils`]) and the identity hash ([`system`]).

pub(crate) mod delegable_modifier;
#[rustfmt::skip]
pub(crate) mod l10n;
pub(crate) mod remember_utils;
pub(crate) mod strings;
pub(crate) mod system;
