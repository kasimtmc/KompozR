//! A three-pane scaffold's motions, as Compose's adaptive-layout ThreePaneMotion.kt (1.2.0): [`ThreePaneMotion`]
//! (each pane's motion), how they follow the scaffold state's change (`calculate_three_pane_motion`), and the
//! scaffold's motion data provider.

use super::pane_motion::{PaneMotion, PaneMotionData, PaneScaffoldMotionDataProvider, calculate_pane_motion};
use super::pane_scaffold_role::ThreePaneScaffoldRole;
use super::predictive_back_scale_state::PredictiveBackScaleState;
use super::three_pane_scaffold_state::ThreePaneScaffoldState;
use super::three_pane_scaffold_value::{PaneScaffoldHorizontalOrder, ThreePaneScaffoldHorizontalOrder};
use kompoz_graphics::IntSize;
use kompoz_runtime::kompozable;
use kompoz_runtime::kt::KtToString;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

/// Each pane's motion of a three-pane scaffold (Compose's `ThreePaneMotion`). Equal by the three.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ThreePaneMotion {
    primary_pane_motion: PaneMotion,
    secondary_pane_motion: PaneMotion,
    tertiary_pane_motion: PaneMotion,
}

impl ThreePaneMotion {
    /// No pane moves (Compose's `ThreePaneMotion.NoMotion`).
    pub const NoMotion: ThreePaneMotion = ThreePaneMotion {
        primary_pane_motion: PaneMotion::NoMotion,
        secondary_pane_motion: PaneMotion::NoMotion,
        tertiary_pane_motion: PaneMotion::NoMotion,
    };

    /// Compose's internal constructor.
    pub(crate) fn new(primary: PaneMotion, secondary: PaneMotion, tertiary: PaneMotion) -> ThreePaneMotion {
        ThreePaneMotion {
            primary_pane_motion: primary,
            secondary_pane_motion: secondary,
            tertiary_pane_motion: tertiary,
        }
    }

    /// The motion of the pane of `role` (Kotlin's operator `get`).
    pub fn get(&self, role: ThreePaneScaffoldRole) -> PaneMotion {
        match role {
            ThreePaneScaffoldRole::Primary => self.primary_pane_motion,
            ThreePaneScaffoldRole::Secondary => self.secondary_pane_motion,
            ThreePaneScaffoldRole::Tertiary => self.tertiary_pane_motion,
        }
    }
}

/// Kotlin's `toString`.
impl fmt::Display for ThreePaneMotion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ThreePaneMotion(primaryPaneMotion={}, secondaryPaneMotion={}, tertiaryPaneMotion={})",
            self.primary_pane_motion, self.secondary_pane_motion, self.tertiary_pane_motion
        )
    }
}

impl KtToString for ThreePaneMotion {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

/// The panes' motions of the state's change (Compose's internal `ThreePaneScaffoldState.calculateThreePaneMotion`):
/// worked out again only while the state is changing (not at the end of a change), kept otherwise.
#[kompozable]
pub(crate) fn calculate_three_pane_motion(
    state: &dyn ThreePaneScaffoldState,
    ltr_pane_order: ThreePaneScaffoldHorizontalOrder,
) -> ThreePaneMotion {
    let result = kompoz_runtime::remember(|| Rc::new(Cell::new(ThreePaneMotion::NoMotion)));
    let (current, target) = (state.current_state(), state.target_state());
    if current != target {
        // Only update motions when the state changes to prevent unnecessary recomposition at the end of state
        // transitions.
        let motions = calculate_pane_motion(&current, &target, &ltr_pane_order);
        let at = |role| motions[ltr_pane_order.index_of(role) as usize];
        result.set(ThreePaneMotion::new(
            at(ThreePaneScaffoldRole::Primary),
            at(ThreePaneScaffoldRole::Secondary),
            at(ThreePaneScaffoldRole::Tertiary),
        ));
    }
    result.get()
}

/// The three-pane scaffold's motion data (Compose's internal `ThreePaneScaffoldMotionDataProvider`, what its scopes'
/// `motion_data_provider` is: public here as the scopes' associated type): each pane's, the scaffold's size, the
/// order left to right, the predictive back scale.
pub struct ThreePaneScaffoldMotionDataProvider {
    ltr_order: Cell<ThreePaneScaffoldHorizontalOrder>,
    primary: PaneMotionData,
    secondary: PaneMotionData,
    tertiary: PaneMotionData,
    pub(crate) scaffold_state: RefCell<Option<Rc<dyn ThreePaneScaffoldState>>>,
    pub(crate) predictive_back_scale_state: Rc<PredictiveBackScaleState>,
    scaffold_size: Cell<IntSize>,
}

impl ThreePaneScaffoldMotionDataProvider {
    pub(crate) fn new() -> Rc<ThreePaneScaffoldMotionDataProvider> {
        Rc::new_cyclic(|this: &Weak<ThreePaneScaffoldMotionDataProvider>| {
            let (size_of, in_progress_of) = (this.clone(), this.clone());
            ThreePaneScaffoldMotionDataProvider {
                // (Compose's lateinit: set by `update` before it is read)
                ltr_order: Cell::new(ThreePaneScaffoldHorizontalOrder::new(
                    ThreePaneScaffoldRole::Primary,
                    ThreePaneScaffoldRole::Secondary,
                    ThreePaneScaffoldRole::Tertiary,
                )),
                primary: PaneMotionData::new(),
                secondary: PaneMotionData::new(),
                tertiary: PaneMotionData::new(),
                scaffold_state: RefCell::new(None),
                predictive_back_scale_state: Rc::new(PredictiveBackScaleState::new(
                    move || size_of.upgrade().map_or(IntSize::Zero, |p| p.scaffold_size.get()),
                    move || {
                        in_progress_of.upgrade().is_some_and(|p| {
                            p.scaffold_state.borrow().as_ref().is_some_and(|s| s.is_predictive_back_in_progress())
                        })
                    },
                )),
                scaffold_size: Cell::new(IntSize::Zero),
            }
        })
    }

    pub(crate) fn set_scaffold_size(&self, size: IntSize) {
        self.scaffold_size.set(size);
    }

    /// Each pane's motion and the order left to right; the origins worked out again (Compose's `update`).
    pub(crate) fn update(&self, three_pane_motion: ThreePaneMotion, ltr_order: ThreePaneScaffoldHorizontalOrder) {
        self.ltr_order.set(ltr_order);
        for role in ThreePaneScaffoldRole::ENTRIES {
            let data = self.get(role);
            data.set_motion(three_pane_motion.get(role));
            data.is_origin_size_and_position_set.set(false);
        }
    }
}

impl PaneScaffoldMotionDataProvider for ThreePaneScaffoldMotionDataProvider {
    type Role = ThreePaneScaffoldRole;

    fn scaffold_size(&self) -> IntSize {
        self.scaffold_size.get()
    }

    fn count(&self) -> i32 {
        3
    }

    fn get_role_at(&self, index: i32) -> ThreePaneScaffoldRole {
        self.ltr_order.get().get(index)
    }

    fn get(&self, role: ThreePaneScaffoldRole) -> &PaneMotionData {
        match role {
            ThreePaneScaffoldRole::Primary => &self.primary,
            ThreePaneScaffoldRole::Secondary => &self.secondary,
            ThreePaneScaffoldRole::Tertiary => &self.tertiary,
        }
    }
}
