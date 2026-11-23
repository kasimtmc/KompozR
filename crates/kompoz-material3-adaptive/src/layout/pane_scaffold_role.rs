//! The roles of panes, as Compose's adaptive-layout PaneScaffoldRole.kt (1.2.0): [`PaneScaffoldRole`] (what a pane
//! scaffold indexes its panes by) and [`ThreePaneScaffoldRole`] (the three-pane scaffolds' primary, secondary and
//! tertiary panes).

use kompoz_runtime::kt::KtToString;
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// The roles of panes in pane scaffold implementations (Compose's interface `PaneScaffoldRole`): what a scaffold
/// indexes its panes and their adapted values by. [`ThreePaneScaffoldRole`] is the three-pane scaffolds'.
pub trait PaneScaffoldRole: Any + fmt::Debug + fmt::Display {
    /// It as `Any` (Kotlin's `as? ThreePaneScaffoldRole`).
    fn as_any(&self) -> &dyn Any;

    /// Whether it is the same role as `other` (Kotlin's `==` of two roles).
    fn role_eq(&self, other: &dyn PaneScaffoldRole) -> bool;
}

impl PartialEq for dyn PaneScaffoldRole {
    fn eq(&self, other: &Self) -> bool {
        self.role_eq(other)
    }
}

/// A role a value holds (Compose's `PaneScaffoldRole` references): shared, compared by the role.
#[derive(Clone, Debug)]
pub struct DynPaneScaffoldRole(pub Rc<dyn PaneScaffoldRole>);

impl DynPaneScaffoldRole {
    /// The role as a three-pane scaffold's, if it is one (Kotlin's `as? ThreePaneScaffoldRole`).
    pub fn as_three_pane(&self) -> Option<ThreePaneScaffoldRole> {
        self.0.as_any().downcast_ref::<ThreePaneScaffoldRole>().copied()
    }
}

impl PartialEq for DynPaneScaffoldRole {
    fn eq(&self, other: &Self) -> bool {
        self.0.role_eq(&*other.0)
    }
}

impl fmt::Display for DynPaneScaffoldRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.0, f)
    }
}

impl<R: PaneScaffoldRole> From<R> for DynPaneScaffoldRole {
    fn from(role: R) -> DynPaneScaffoldRole {
        DynPaneScaffoldRole(Rc::new(role))
    }
}

/// The panes of a three-pane scaffold ([`ThreePaneScaffold`](crate::layout) and the list-detail and supporting
/// pane scaffolds made of it), by priority (Compose's enum `ThreePaneScaffoldRole`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ThreePaneScaffoldRole {
    /// The primary pane: the highest priority in layout adaptation, the screen's most important content (the
    /// details of a list-detail layout).
    Primary,
    /// The secondary pane: the second highest priority, the screen's supplementary content (the list of a
    /// list-detail layout).
    Secondary,
    /// The tertiary pane: the lowest priority, additional content usually shown only on the user's interaction.
    Tertiary,
}

impl ThreePaneScaffoldRole {
    /// Its values in order (Kotlin's `entries`).
    pub const ENTRIES: [ThreePaneScaffoldRole; 3] =
        [ThreePaneScaffoldRole::Primary, ThreePaneScaffoldRole::Secondary, ThreePaneScaffoldRole::Tertiary];

    /// Its name (Kotlin's `name`).
    pub fn name(self) -> &'static str {
        match self {
            ThreePaneScaffoldRole::Primary => "Primary",
            ThreePaneScaffoldRole::Secondary => "Secondary",
            ThreePaneScaffoldRole::Tertiary => "Tertiary",
        }
    }

    /// Its ordinal (Kotlin's `ordinal`).
    pub fn ordinal(self) -> i32 {
        self as i32
    }
}

/// Kotlin's enum `toString`: its name.
impl fmt::Display for ThreePaneScaffoldRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl KtToString for ThreePaneScaffoldRole {
    fn kt_write(&self, out: &mut String) {
        out.push_str(self.name());
    }
}

impl PaneScaffoldRole for ThreePaneScaffoldRole {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn role_eq(&self, other: &dyn PaneScaffoldRole) -> bool {
        other.as_any().downcast_ref::<ThreePaneScaffoldRole>() == Some(self)
    }
}

/// Saved as its ordinal (Kotlin's enums are Serializable).
impl kompoz_runtime::Saveable for ThreePaneScaffoldRole {
    fn save(&self) -> kompoz_runtime::SavedValue {
        kompoz_runtime::SavedValue::Int(*self as i64)
    }

    fn restore(saved: &kompoz_runtime::SavedValue) -> Option<Self> {
        let ordinal = usize::try_from(i64::restore(saved)?).ok()?;
        ThreePaneScaffoldRole::ENTRIES.get(ordinal).copied()
    }
}
