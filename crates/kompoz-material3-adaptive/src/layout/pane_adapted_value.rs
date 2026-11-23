//! How a pane is adapted, as Compose's adaptive-layout PaneAdaptedValue.kt, AdaptStrategy.kt and
//! ThreePaneScaffoldAdaptStrategies.kt (1.2.0): [`PaneAdaptedValue`] (expanded, hidden, reflowed under another pane,
//! levitated), [`AdaptStrategy`] (what a pane becomes when not all panes fit) and
//! [`ThreePaneScaffoldAdaptStrategies`].

use super::pane_scaffold_directive::PaneScaffoldDirective;
use super::pane_scaffold_role::{DynPaneScaffoldRole, PaneScaffoldRole, ThreePaneScaffoldRole};
use kompoz_runtime::ThreadBound;
use kompoz_runtime::kt::KtToString;
use kompoz_ui::Alignment;
use std::fmt;
use std::rc::Rc;

/// A composable scrim of a levitated pane (Compose's `(@Composable () -> Unit)?`), compared by identity.
pub type PaneScrim = Option<Rc<dyn Fn()>>;

/// A scrim as an adapted value holds it: bound to the UI thread, so that the value can be a transition's state
/// (an animation value is `Send + Sync`; Compose's states hold any object, touched on the main thread only).
#[derive(Clone)]
pub struct BoundScrim(Option<ThreadBound<Rc<dyn Fn()>>>);

impl BoundScrim {
    pub(crate) fn new(scrim: PaneScrim) -> BoundScrim {
        BoundScrim(scrim.map(ThreadBound::new))
    }

    /// The scrim.
    pub fn get(&self) -> PaneScrim {
        self.0.as_ref().map(|s| s.get().clone())
    }

    fn is_some(&self) -> bool {
        self.0.is_some()
    }
}

/// By identity (Kotlin's `scrim !== other.scrim`).
impl PartialEq for BoundScrim {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Rc::ptr_eq(a.get(), b.get()),
            (None, None) => true,
            _ => false,
        }
    }
}

/// Kotlin's `toString` of a nullable lambda: `null`, or a lambda's (its class here: `Function0<kotlin.Unit>`).
fn scrim_text(scrim: &BoundScrim) -> &'static str {
    if scrim.is_some() { "Function0<kotlin.Unit>" } else { "null" }
}

/// The adapted state of a pane (Compose's sealed interface `PaneAdaptedValue`): whether a pane scaffold composes
/// it and how.
#[derive(Clone)]
pub enum PaneAdaptedValue {
    /// Displayed in its full width and height (Compose's `PaneAdaptedValue.Expanded`).
    Expanded,
    /// Hidden (Compose's `PaneAdaptedValue.Hidden`).
    Hidden,
    /// Reflowed under its `reflow_under` pane (Compose's class `PaneAdaptedValue.Reflowed`; make it with
    /// [`PaneAdaptedValue::reflowed`]).
    Reflowed { reflow_under: DynPaneScaffoldRole },
    /// Levitated over the other panes with `alignment` in the scaffold, `scrim` (if any) blocking the panes under
    /// it (Compose's class `PaneAdaptedValue.Levitated`; make it with [`PaneAdaptedValue::levitated`]).
    Levitated { alignment: ThreadBound<Alignment>, scrim: BoundScrim },
}

impl PaneAdaptedValue {
    /// A pane reflowed to `reflow_under`: displayed under that pane (Compose's `PaneAdaptedValue.Reflowed(…)`).
    ///
    /// Kotlin: `PaneAdaptedValue.Reflowed(reflowUnder: PaneScaffoldRole)`
    pub fn reflowed(reflow_under: impl PaneScaffoldRole) -> PaneAdaptedValue {
        PaneAdaptedValue::Reflowed { reflow_under: reflow_under.into() }
    }

    /// A pane levitated with `alignment` in the scaffold, with `scrim` blocking the panes under it (`None`: no
    /// scrim; [`LevitatedPaneScrim`](crate::layout::LevitatedPaneScrim) is the default one) (Compose's
    /// `PaneAdaptedValue.Levitated(…)`).
    ///
    /// Kotlin: `PaneAdaptedValue.Levitated(alignment: Alignment, scrim: (@Composable () -> Unit)? = null)`
    pub fn levitated(alignment: Alignment, scrim: PaneScrim) -> PaneAdaptedValue {
        PaneAdaptedValue::Levitated { alignment: ThreadBound::new(alignment), scrim: BoundScrim::new(scrim) }
    }

    /// The pane it is reflowed under, if reflowed (Compose's internal `Reflowed.reflowUnder`).
    pub(crate) fn reflow_under(&self) -> Option<&DynPaneScaffoldRole> {
        match self {
            PaneAdaptedValue::Reflowed { reflow_under } => Some(reflow_under),
            _ => None,
        }
    }

    /// Whether it is reflowed (Kotlin's `is PaneAdaptedValue.Reflowed`).
    pub fn is_reflowed(&self) -> bool {
        matches!(self, PaneAdaptedValue::Reflowed { .. })
    }

    /// Whether it is levitated (Kotlin's `is PaneAdaptedValue.Levitated`).
    pub fn is_levitated(&self) -> bool {
        matches!(self, PaneAdaptedValue::Levitated { .. })
    }

    /// The scrim of a levitated pane (Compose's internal `Levitated.scrim`).
    pub(crate) fn levitated_scrim(&self) -> PaneScrim {
        match self {
            PaneAdaptedValue::Levitated { scrim, .. } => scrim.get(),
            _ => None,
        }
    }

    /// The alignment of a levitated pane (Compose's internal `Levitated.alignment`).
    pub(crate) fn levitated_alignment(&self) -> Option<Alignment> {
        match self {
            PaneAdaptedValue::Levitated { alignment, .. } => Some(alignment.get().clone()),
            _ => None,
        }
    }
}

/// Kotlin's `equals`: the simple values by identity, reflowed ones by their target, levitated ones by their
/// alignment and scrim (the scrim by identity).
impl PartialEq for PaneAdaptedValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PaneAdaptedValue::Expanded, PaneAdaptedValue::Expanded) => true,
            (PaneAdaptedValue::Hidden, PaneAdaptedValue::Hidden) => true,
            (PaneAdaptedValue::Reflowed { reflow_under: a }, PaneAdaptedValue::Reflowed { reflow_under: b }) => a == b,
            (
                PaneAdaptedValue::Levitated { alignment: a, scrim: s },
                PaneAdaptedValue::Levitated { alignment: b, scrim: t },
            ) => a == b && s == t,
            _ => false,
        }
    }
}

/// Kotlin's `toString`: `PaneAdaptedValue[Expanded]`, `PaneAdaptedValue[Reflowed to Primary]`, ….
impl fmt::Display for PaneAdaptedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaneAdaptedValue::Expanded => f.write_str("PaneAdaptedValue[Expanded]"),
            PaneAdaptedValue::Hidden => f.write_str("PaneAdaptedValue[Hidden]"),
            PaneAdaptedValue::Reflowed { reflow_under } => write!(f, "PaneAdaptedValue[Reflowed to {reflow_under}]"),
            PaneAdaptedValue::Levitated { alignment, scrim } => {
                write!(f, "PaneAdaptedValue[Levitated with {alignment:?} and scrim={}]", scrim_text(scrim))
            }
        }
    }
}

impl fmt::Debug for PaneAdaptedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl KtToString for PaneAdaptedValue {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

/// How a pane is adapted when not all panes can be expanded (Compose's sealed interface `AdaptStrategy`).
#[derive(Clone)]
pub enum AdaptStrategy {
    /// Hidden when it has to be adapted (Compose's `AdaptStrategy.Hide`, the default).
    Hide,
    /// Reflowed under `reflow_under` in a single-pane layout of more than one vertical partition, when it or that
    /// pane is the current destination, hidden otherwise (Compose's class `AdaptStrategy.Reflow`; make it with
    /// [`AdaptStrategy::reflow`]).
    Reflow { reflow_under: DynPaneScaffoldRole },
    /// Levitated with `alignment` (over the other panes, like a pop-up or a sheet) when it is the current
    /// destination, hidden otherwise; `scrim` blocks the panes under it (Compose's class `AdaptStrategy.Levitate`;
    /// make it with [`AdaptStrategy::levitate`]).
    Levitate { alignment: ThreadBound<Alignment>, scrim: BoundScrim },
}

impl AdaptStrategy {
    /// Kotlin: `AdaptStrategy.Reflow(reflowUnder: PaneScaffoldRole)`
    pub fn reflow(reflow_under: impl PaneScaffoldRole) -> AdaptStrategy {
        AdaptStrategy::Reflow { reflow_under: reflow_under.into() }
    }

    /// Kotlin: `AdaptStrategy.Levitate(alignment: Alignment = Alignment.Center, scrim: (@Composable () -> Unit)? = null)`
    pub fn levitate(alignment: Alignment, scrim: PaneScrim) -> AdaptStrategy {
        AdaptStrategy::Levitate { alignment: ThreadBound::new(alignment), scrim: BoundScrim::new(scrim) }
    }

    /// The adapted value (Compose's deprecated `adapt()`: always hidden).
    #[deprecated(
        note = "This function is deprecated in favor of directly using the info carried by the strategy instances to make adaptation decisions."
    )]
    pub fn adapt(&self) -> PaneAdaptedValue {
        PaneAdaptedValue::Hidden
    }

    /// Whether it reflows (Kotlin's `is AdaptStrategy.Reflow`).
    pub fn is_reflow(&self) -> bool {
        matches!(self, AdaptStrategy::Reflow { .. })
    }

    /// Whether it levitates (Kotlin's `is AdaptStrategy.Levitate`).
    pub fn is_levitate(&self) -> bool {
        matches!(self, AdaptStrategy::Levitate { .. })
    }

    /// A levitating strategy levitating only when `condition` holds: else [`Hide`](Self::Hide) — the pane is
    /// expanded if there's room, hidden otherwise (Compose's `Levitate.onlyIf`, composable there though it reads
    /// nothing of the composition: a plain function here). Of another strategy: it as it is.
    pub fn only_if(&self, condition: bool) -> AdaptStrategy {
        if condition { self.clone() } else { AdaptStrategy::Hide }
    }

    /// A levitating strategy levitating only in a single-pane layout (Compose's `Levitate.onlyIfSinglePane`).
    pub fn only_if_single_pane(&self, scaffold_directive: &PaneScaffoldDirective) -> AdaptStrategy {
        self.only_if(scaffold_directive.is_single_pane_layout())
    }
}

/// Kotlin's `equals`: Hide by identity, Reflow by its target, Levitate by its alignment and scrim (by identity).
impl PartialEq for AdaptStrategy {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AdaptStrategy::Hide, AdaptStrategy::Hide) => true,
            (AdaptStrategy::Reflow { reflow_under: a }, AdaptStrategy::Reflow { reflow_under: b }) => a == b,
            (
                AdaptStrategy::Levitate { alignment: a, scrim: s },
                AdaptStrategy::Levitate { alignment: b, scrim: t },
            ) => a == b && s == t,
            _ => false,
        }
    }
}

/// Kotlin's `toString`: `AdaptStrategy[Hide]`, `AdaptStrategy[Reflow to Primary]`, ….
impl fmt::Display for AdaptStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdaptStrategy::Hide => f.write_str("AdaptStrategy[Hide]"),
            AdaptStrategy::Reflow { reflow_under } => write!(f, "AdaptStrategy[Reflow to {reflow_under}]"),
            AdaptStrategy::Levitate { alignment, scrim } => {
                write!(f, "AdaptStrategy[Levitate, alignment={alignment:?}, scrim={}]", scrim_text(scrim))
            }
        }
    }
}

impl fmt::Debug for AdaptStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl KtToString for AdaptStrategy {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

/// How each pane of a three-pane scaffold is adapted (Compose's `ThreePaneScaffoldAdaptStrategies`): what
/// [`calculate_three_pane_scaffold_value`](crate::layout::calculate_three_pane_scaffold_value) adapts the panes by.
/// Equal by its three strategies.
#[derive(Clone, PartialEq, Debug)]
pub struct ThreePaneScaffoldAdaptStrategies {
    primary_pane_adapt_strategy: AdaptStrategy,
    secondary_pane_adapt_strategy: AdaptStrategy,
    tertiary_pane_adapt_strategy: AdaptStrategy,
}

/// Kotlin: `ThreePaneScaffoldAdaptStrategies(primaryPaneAdaptStrategy: AdaptStrategy, secondaryPaneAdaptStrategy: AdaptStrategy, tertiaryPaneAdaptStrategy: AdaptStrategy)`
pub fn ThreePaneScaffoldAdaptStrategies(
    primary_pane_adapt_strategy: AdaptStrategy,
    secondary_pane_adapt_strategy: AdaptStrategy,
    tertiary_pane_adapt_strategy: AdaptStrategy,
) -> ThreePaneScaffoldAdaptStrategies {
    ThreePaneScaffoldAdaptStrategies {
        primary_pane_adapt_strategy,
        secondary_pane_adapt_strategy,
        tertiary_pane_adapt_strategy,
    }
}

impl ThreePaneScaffoldAdaptStrategies {
    /// The strategy of the pane of `role` (Kotlin's operator `get`).
    pub fn get(&self, role: ThreePaneScaffoldRole) -> &AdaptStrategy {
        match role {
            ThreePaneScaffoldRole::Primary => &self.primary_pane_adapt_strategy,
            ThreePaneScaffoldRole::Secondary => &self.secondary_pane_adapt_strategy,
            ThreePaneScaffoldRole::Tertiary => &self.tertiary_pane_adapt_strategy,
        }
    }
}
