//! `FlexBox`, as Compose's (experimental, foundation-layout's FlexBox.kt): children along a main axis, wrapped onto
//! lines if its config says so, sharing the room by their flex (grow, shrink, basis) as CSS's flexbox does, each
//! aligned across in its line, the lines spread across (Compose's FlexBoxMeasurePolicy, intrinsics included). Its
//! config and its children's are closures run as it measures (Compose's `fun interface`s `FlexBoxConfig` and
//! `FlexConfig`, given their scopes as a parameter here).

use super::coerce_in;
use super::orientation::{LayoutOrientation, OrientationIndependentConstraints};
use super::round;
use kompoz_graphics::{Density, Dp};
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{Memoized, kompozable, named, remember_keyed};
use kompoz_ui::layout::{AlignmentLine, ParentDataModifierNode, layout_into};
use kompoz_ui::{
    Constraints, FirstBaseline, IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasurePolicy, MeasureResult,
    MeasureScope, Modifier, ModifierNode, ModifierNodeElement, Placeable, node_capabilities,
};
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// A layout of its children along a main axis, wrapping them onto lines when its config lets it (Compose's
/// experimental `FlexBox`, a superset of `Row`, `Column`, `FlowRow` and `FlowColumn`). `config` sets the direction,
/// the wrapping, how children are arranged along their line and aligned across it, how lines are spread, and the
/// gaps; it runs as the FlexBox measures (it can depend on the constraints, and a state it reads lays the FlexBox
/// out again without composing it): `config = |c: &mut FlexBoxConfigScope| c.wrap(FlexWrap::Wrap)`, or
/// [`DefaultFlexBoxConfig`] (the default) for a row without wrapping. Children share the room by their
/// [`Modifier.flex`](FlexBoxScopeModifiers::flex) (grow, shrink, basis, own alignment, order); those that can't
/// shrink enough overflow along the main axis.
#[kompozable(inline)]
pub fn FlexBox(
    #[default(Modifier)] modifier: Modifier,
    #[default(DefaultFlexBoxConfig)]
    #[memoized]
    config: impl FlexBoxConfig + 'static,
    content: impl FnOnce(),
) {
    let policy = flex_box_measure_policy(config);
    layout_into(modifier, policy, false, content);
}

/// The policy of a FlexBox (Compose's `flexMultiContentMeasurePolicy`). Compose keeps one policy and hands it the
/// config through `rememberUpdatedState`, so a new config lays the FlexBox out again; here the policy is kept while
/// the config is the same memoized closure (or the same shared config, or the default), and another one makes a
/// new policy, which measures it again: the same layouts.
#[track_caller]
fn flex_box_measure_policy<C: FlexBoxConfig + 'static>(config: Memoized<C>) -> Rc<dyn MeasurePolicy> {
    let id = config.id();
    let config = config.into_value().into_shared();
    remember_keyed(PolicyKey { id, config: config.clone() }, move || {
        Rc::new(FlexBoxMeasurePolicy { config }) as Rc<dyn MeasurePolicy>
    })
}

/// What a FlexBox keeps its policy by: its config's identity as a memoized closure, or the config itself.
struct PolicyKey {
    id: Option<u64>,
    config: Rc<dyn FlexBoxConfig>,
}

impl PartialEq for PolicyKey {
    fn eq(&self, other: &Self) -> bool {
        (self.id.is_some() && self.id == other.id) || same_flex_box_config(&self.config, &other.config)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The container's config

/// What configures a [`FlexBox`] (Compose's `fun interface FlexBoxConfig`): run as the FlexBox measures, it sets
/// what it wants on the [`FlexBoxConfigScope`] it is given, the last setting of a property winning. A closure
/// `|c: &mut FlexBoxConfigScope| …` is one; [`DefaultFlexBoxConfig`] is the empty one, [`then`](FlexBoxConfig::then)
/// and [`FlexBoxConfig()`](fn@FlexBoxConfig) combine them. Combined or shared, a config is an
/// `Rc<dyn FlexBoxConfig>`, itself a config: equal to another when it is the same one, or combines equal ones.
pub trait FlexBoxConfig {
    /// Sets what it configures on `scope` (Compose's `FlexBoxConfigScope.configure()`).
    fn configure(&self, scope: &mut FlexBoxConfigScope);

    /// This config, then `other` over it, property by property (Compose's `then`): the default one gives the
    /// other.
    fn then(self, other: impl FlexBoxConfig + 'static) -> Rc<dyn FlexBoxConfig>
    where
        Self: Sized + 'static,
    {
        then_flex_box_config(self.into_shared(), other.into_shared())
    }

    /// It shared (an `Rc` of a shared one is itself).
    #[doc(hidden)]
    fn into_shared(self) -> Rc<dyn FlexBoxConfig>
    where
        Self: Sized + 'static,
    {
        Rc::new(self)
    }

    /// The configs it combines, if it is a combined one (Compose's CombinedFlexBoxConfig's `configs`).
    #[doc(hidden)]
    fn combined_configs(&self) -> Option<&[Rc<dyn FlexBoxConfig>]> {
        None
    }

    /// Whether it is the default (Compose's `=== FlexBoxConfig`).
    #[doc(hidden)]
    fn is_default_config(&self) -> bool {
        false
    }
}

impl<F: Fn(&mut FlexBoxConfigScope)> FlexBoxConfig for F {
    fn configure(&self, scope: &mut FlexBoxConfigScope) {
        self(scope)
    }
}

impl FlexBoxConfig for Rc<dyn FlexBoxConfig> {
    fn configure(&self, scope: &mut FlexBoxConfigScope) {
        (**self).configure(scope)
    }
    fn into_shared(self) -> Rc<dyn FlexBoxConfig> {
        self
    }
    fn combined_configs(&self) -> Option<&[Rc<dyn FlexBoxConfig>]> {
        (**self).combined_configs()
    }
    fn is_default_config(&self) -> bool {
        (**self).is_default_config()
    }
}

/// The empty FlexBox config, a FlexBox's default (Compose's `FlexBoxConfig` companion used as a value): a row
/// without wrapping, children and lines at the start, no gaps. Combined with another config, it gives that one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DefaultFlexBoxConfig;

impl FlexBoxConfig for DefaultFlexBoxConfig {
    fn configure(&self, _: &mut FlexBoxConfigScope) {}
    fn is_default_config(&self) -> bool {
        true
    }
}

/// Configs combined (Compose's internal CombinedFlexBoxConfig): each run in turn, flat.
struct CombinedFlexBoxConfig {
    configs: Vec<Rc<dyn FlexBoxConfig>>,
}

impl FlexBoxConfig for CombinedFlexBoxConfig {
    fn configure(&self, scope: &mut FlexBoxConfigScope) {
        for config in &self.configs {
            config.configure(scope);
        }
    }
    fn combined_configs(&self) -> Option<&[Rc<dyn FlexBoxConfig>]> {
        Some(&self.configs)
    }
}

/// Whether two FlexBox configs are equal (Compose's `==`): the same one, both the default, or combining equal ones.
fn same_flex_box_config(a: &Rc<dyn FlexBoxConfig>, b: &Rc<dyn FlexBoxConfig>) -> bool {
    if Rc::ptr_eq(a, b) || (a.is_default_config() && b.is_default_config()) {
        return true;
    }
    match (a.combined_configs(), b.combined_configs()) {
        (Some(x), Some(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_flex_box_config(p, q)),
        _ => false,
    }
}

/// `first then second` (Compose's `then`, and the default's override).
fn then_flex_box_config(first: Rc<dyn FlexBoxConfig>, second: Rc<dyn FlexBoxConfig>) -> Rc<dyn FlexBoxConfig> {
    if first.is_default_config() {
        return second;
    }
    if second.is_default_config() {
        return first;
    }
    let mut configs = vec![first];
    match second.combined_configs() {
        Some(parts) => configs.extend(parts.iter().cloned()),
        None => configs.push(second),
    }
    Rc::new(CombinedFlexBoxConfig { configs })
}

/// Compose's `FlexBoxConfig(vararg configs)`: the default ones left out, the one left if only one is.
fn flex_box_config_of(configs: Vec<Rc<dyn FlexBoxConfig>>) -> Rc<dyn FlexBoxConfig> {
    if configs.is_empty() {
        return Rc::new(DefaultFlexBoxConfig);
    }
    if !configs.iter().any(|c| c.is_default_config()) {
        return Rc::new(CombinedFlexBoxConfig { configs });
    }
    let mut filtered: Vec<Rc<dyn FlexBoxConfig>> = configs.into_iter().filter(|c| !c.is_default_config()).collect();
    match filtered.len() {
        0 => Rc::new(DefaultFlexBoxConfig),
        1 => filtered.pop().expect("one config"),
        _ => Rc::new(CombinedFlexBoxConfig { configs: filtered }),
    }
}

/// FlexBox configs combined, each over the ones before it, property by property (Compose's
/// `FlexBoxConfig(first, second)`, `FlexBoxConfig(first, second, third)` and `FlexBoxConfig(vararg configs)`): two or
/// three as a tuple, any number as an array or a `Vec` of shared ones. The default ones are left out.
pub fn FlexBoxConfig(configs: impl FlexBoxConfigArgs) -> Rc<dyn FlexBoxConfig> {
    configs.flex_box_config()
}

/// What [`FlexBoxConfig()`](fn@FlexBoxConfig) is given: Kotlin's arguments, several as a tuple, a vararg as an
/// array or a `Vec`.
pub trait FlexBoxConfigArgs {
    fn flex_box_config(self) -> Rc<dyn FlexBoxConfig>;
}

/// Kotlin: `FlexBoxConfig(first: FlexBoxConfig, second: FlexBoxConfig)`
impl<A: FlexBoxConfig + 'static, B: FlexBoxConfig + 'static> FlexBoxConfigArgs for (A, B) {
    fn flex_box_config(self) -> Rc<dyn FlexBoxConfig> {
        self.0.then(self.1)
    }
}

/// Kotlin: `FlexBoxConfig(first: FlexBoxConfig, second: FlexBoxConfig, third: FlexBoxConfig)`
impl<A: FlexBoxConfig + 'static, B: FlexBoxConfig + 'static, C: FlexBoxConfig + 'static> FlexBoxConfigArgs
    for (A, B, C)
{
    fn flex_box_config(self) -> Rc<dyn FlexBoxConfig> {
        let (first, second, third) = (self.0.into_shared(), self.1.into_shared(), self.2.into_shared());
        if first.is_default_config() {
            return then_flex_box_config(second, third);
        }
        if second.is_default_config() {
            return then_flex_box_config(first, third);
        }
        if third.is_default_config() {
            return then_flex_box_config(first, second);
        }
        // (the combined ones spread, so combinations stay flat)
        let mut configs = Vec::new();
        for config in [first, second, third] {
            match config.combined_configs() {
                Some(parts) => configs.extend(parts.iter().cloned()),
                None => configs.push(config),
            }
        }
        flex_box_config_of(configs)
    }
}

/// Kotlin: `FlexBoxConfig(vararg configs: FlexBoxConfig)`
impl<const N: usize> FlexBoxConfigArgs for [Rc<dyn FlexBoxConfig>; N] {
    fn flex_box_config(self) -> Rc<dyn FlexBoxConfig> {
        flex_box_config_of(self.into())
    }
}

/// A vararg's array made at run time.
impl FlexBoxConfigArgs for Vec<Rc<dyn FlexBoxConfig>> {
    fn flex_box_config(self) -> Rc<dyn FlexBoxConfig> {
        flex_box_config_of(self)
    }
}

/// What a [`FlexBoxConfig`] configures a FlexBox on (Compose's `FlexBoxConfigScope`, and ResolvedFlexBoxConfig
/// behind it): the constraints the FlexBox is measured in, its density (it derefs to it: `c.round_to_px(…)`), and
/// the setters of its properties, each starting from Compose's default as the FlexBox measures. Written as
/// Compose's `toString()` writes it.
pub struct FlexBoxConfigScope {
    density: Density,
    constraints: Constraints,
    direction: FlexDirection,
    wrap: FlexWrap,
    justify_content: FlexJustifyContent,
    align_items: FlexAlignItems,
    align_content: FlexAlignContent,
    row_gap: Dp,
    column_gap: Dp,
    // Baseline alignment: none means alignItems' own.
    baseline_alignment_line: Option<AlignmentLine>,
    baseline_alignment_block: Option<Rc<dyn Fn(&Placeable) -> i32>>,
}

impl std::ops::Deref for FlexBoxConfigScope {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.density
    }
}

impl FlexBoxConfigScope {
    /// Compose's defaults, for a FlexBox measured in `constraints` (Compose's `prepare`).
    fn new(density: Density, constraints: Constraints) -> FlexBoxConfigScope {
        FlexBoxConfigScope {
            density,
            constraints,
            direction: FlexDirection::Row,
            wrap: FlexWrap::NoWrap,
            justify_content: FlexJustifyContent::Start,
            align_items: FlexAlignItems::Start,
            align_content: FlexAlignContent::Start,
            row_gap: Dp(0.0),
            column_gap: Dp(0.0),
            baseline_alignment_line: None,
            baseline_alignment_block: None,
        }
    }

    /// The constraints the FlexBox's parent measures it in (Compose's `constraints`): for a config that adapts to
    /// the room it has.
    pub fn constraints(&self) -> Constraints {
        self.constraints
    }

    /// The main axis, along which the children are laid out (Compose's `direction`); [`FlexDirection::Row`] by
    /// default.
    pub fn direction(&mut self, value: FlexDirection) {
        self.direction = value;
    }

    /// Whether the children stay on one line or wrap onto more (Compose's `wrap`); [`FlexWrap::NoWrap`] by default.
    pub fn wrap(&mut self, value: FlexWrap) {
        self.wrap = value;
    }

    /// How the children are spread along their line (Compose's `justifyContent`); [`FlexJustifyContent::Start`] by
    /// default.
    pub fn justify_content(&mut self, value: FlexJustifyContent) {
        self.justify_content = value;
    }

    /// How the children are aligned across their line, unless one says otherwise (Compose's `alignItems`);
    /// [`FlexAlignItems::Start`] by default.
    pub fn align_items(&mut self, value: FlexAlignItems) {
        self.align_items = value;
    }

    /// The children aligned by `alignment_line` across their line (Compose's `alignItems(alignmentLine)`); one
    /// without it by its far edge (its height for a horizontal line, its width for a vertical one).
    /// Kotlin: `alignItems(alignmentLine: AlignmentLine)`
    pub fn align_items_by_line(&mut self, alignment_line: impl Into<AlignmentLine>) {
        self.align_items = FlexAlignItems::Baseline;
        self.baseline_alignment_line = Some(alignment_line.into());
        self.baseline_alignment_block = None;
    }

    /// The children aligned across their line by the position `alignment_line_block` works out from each measured
    /// one (Compose's `alignItems { measured -> … }`).
    /// Kotlin: `alignItems(alignmentLineBlock: (Measured) -> Int)`
    pub fn align_items_by(&mut self, alignment_line_block: impl Fn(&Placeable) -> i32 + 'static) {
        self.align_items = FlexAlignItems::Baseline;
        self.baseline_alignment_line = None;
        self.baseline_alignment_block = Some(Rc::new(alignment_line_block));
    }

    /// How the lines are spread across, when it wraps (Compose's `alignContent`); [`FlexAlignContent::Start`] by
    /// default.
    pub fn align_content(&mut self, value: FlexAlignContent) {
        self.align_content = value;
    }

    /// The vertical gap, whatever the direction: between lines in a row, between children in a column (Compose's
    /// `rowGap`); none by default.
    pub fn row_gap(&mut self, value: Dp) {
        self.row_gap = value;
    }

    /// The horizontal gap, whatever the direction: between children in a row, between lines in a column
    /// (Compose's `columnGap`); none by default.
    pub fn column_gap(&mut self, value: Dp) {
        self.column_gap = value;
    }

    /// The same gap both ways (Compose's `gap(all)`).
    pub fn gap(&mut self, all: Dp) {
        self.row_gap = all;
        self.column_gap = all;
    }

    /// `row` as the [`row_gap`](FlexBoxConfigScope::row_gap), `column` as the
    /// [`column_gap`](FlexBoxConfigScope::column_gap) (Compose's `gap(row, column)`).
    /// Kotlin: `gap(row: Dp, column: Dp)`
    pub fn gap_row_column(&mut self, row: Dp, column: Dp) {
        self.row_gap = row;
        self.column_gap = column;
    }

    /// Where a measured child's baseline is when the children align by one (Compose's `getBaseline`).
    fn get_baseline(&self, placeable: &Placeable) -> i32 {
        if let Some(block) = &self.baseline_alignment_block {
            block(placeable)
        } else if let Some(line) = self.baseline_alignment_line {
            line_or_far_edge(placeable, line)
        } else {
            let value = placeable.get(FirstBaseline);
            if value != AlignmentLine::Unspecified { value } else { placeable.height() }
        }
    }

    fn is_horizontal(&self) -> bool {
        self.direction == FlexDirection::Row || self.direction == FlexDirection::RowReverse
    }

    fn is_wrap_enabled(&self) -> bool {
        self.wrap == FlexWrap::Wrap || self.wrap == FlexWrap::WrapReverse
    }

    fn is_cross_axis_reverse(&self) -> bool {
        self.wrap == FlexWrap::WrapReverse
    }

    fn has_baseline(&self) -> bool {
        self.align_items == FlexAlignItems::Baseline
    }

    /// The gap along: the column gap in a row, the row gap in a column.
    fn main_axis_gap(&self) -> i32 {
        if self.is_horizontal() { self.round_to_px(self.column_gap) } else { self.round_to_px(self.row_gap) }
    }

    /// The gap across: the row gap in a row, the column gap in a column.
    fn cross_axis_gap(&self) -> i32 {
        if self.is_horizontal() { self.round_to_px(self.row_gap) } else { self.round_to_px(self.column_gap) }
    }

    /// Whether the lines' sizes across are worked out before the children are measured (for stretching and
    /// baselines).
    fn need_upfront_cross_axis_calculation(&self, constraints: &OrientationIndependentConstraints) -> bool {
        self.align_items == FlexAlignItems::Stretch
            || self.align_items == FlexAlignItems::Baseline
            || (self.is_wrap_enabled()
                && self.align_content == FlexAlignContent::Stretch
                && constraints.cross_axis_max() != Constraints::Infinity)
    }
}

/// Where `line` is in `placeable`, or its far edge if it has none.
fn line_or_far_edge(placeable: &Placeable, line: AlignmentLine) -> i32 {
    let value = placeable.get(line);
    if value != AlignmentLine::Unspecified {
        value
    } else if line.is_horizontal() {
        placeable.height()
    } else {
        placeable.width()
    }
}

impl fmt::Display for FlexBoxConfigScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "FlexBoxConfig(\n    direction = {},\n    wrap = {},\n    justifyContent = {},\n    alignItems = {},\n    \
             alignContent = {},\n    rowGap = {},\n    columnGap = {}\n)",
            self.direction,
            self.wrap,
            self.justify_content,
            self.align_items,
            self.align_content,
            self.row_gap.kt_to_string(),
            self.column_gap.kt_to_string(),
        )
    }
}

impl fmt::Debug for FlexBoxConfigScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A child's config

/// What configures a child of a [`FlexBox`] (Compose's `fun interface FlexConfig`), given by
/// [`Modifier.flex`](FlexBoxScopeModifiers::flex): run as the FlexBox measures, it sets what it wants on the
/// [`FlexConfigScope`] it is given, the last setting of a property winning. A closure
/// `|f: &mut FlexConfigScope| …` is one; [`DefaultFlexConfig`] is the empty one, [`then`](FlexConfig::then) and
/// [`FlexConfig()`](fn@FlexConfig) combine them. Combined or shared, a config is an `Rc<dyn FlexConfig>`, itself a
/// config: equal to another when it is the same one, or combines equal ones.
pub trait FlexConfig {
    /// Sets what it configures on `scope` (Compose's `FlexConfigScope.configure()`).
    fn configure(&self, scope: &mut FlexConfigScope);

    /// This config, then `other` over it, property by property (Compose's `then`): the default one gives the
    /// other.
    fn then(self, other: impl FlexConfig + 'static) -> Rc<dyn FlexConfig>
    where
        Self: Sized + 'static,
    {
        then_flex_config(self.into_shared(), other.into_shared())
    }

    /// It shared (an `Rc` of a shared one is itself).
    #[doc(hidden)]
    fn into_shared(self) -> Rc<dyn FlexConfig>
    where
        Self: Sized + 'static,
    {
        Rc::new(self)
    }

    /// The configs it combines, if it is a combined one (Compose's CombinedFlexConfig's `configs`).
    #[doc(hidden)]
    fn combined_configs(&self) -> Option<&[Rc<dyn FlexConfig>]> {
        None
    }

    /// Whether it is the default (Compose's `=== FlexConfig`).
    #[doc(hidden)]
    fn is_default_config(&self) -> bool {
        false
    }
}

impl<F: Fn(&mut FlexConfigScope)> FlexConfig for F {
    fn configure(&self, scope: &mut FlexConfigScope) {
        self(scope)
    }
}

impl FlexConfig for Rc<dyn FlexConfig> {
    fn configure(&self, scope: &mut FlexConfigScope) {
        (**self).configure(scope)
    }
    fn into_shared(self) -> Rc<dyn FlexConfig> {
        self
    }
    fn combined_configs(&self) -> Option<&[Rc<dyn FlexConfig>]> {
        (**self).combined_configs()
    }
    fn is_default_config(&self) -> bool {
        (**self).is_default_config()
    }
}

/// The empty child config (Compose's `FlexConfig` companion used as a value): no growing, shrinking by 1, an auto
/// basis, aligned as the FlexBox says, order 0. Combined with another config, it gives that one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DefaultFlexConfig;

impl FlexConfig for DefaultFlexConfig {
    fn configure(&self, _: &mut FlexConfigScope) {}
    fn is_default_config(&self) -> bool {
        true
    }
}

/// Child configs combined (Compose's internal CombinedFlexConfig): each run in turn, flat.
struct CombinedFlexConfig {
    configs: Vec<Rc<dyn FlexConfig>>,
}

impl FlexConfig for CombinedFlexConfig {
    fn configure(&self, scope: &mut FlexConfigScope) {
        for config in &self.configs {
            config.configure(scope);
        }
    }
    fn combined_configs(&self) -> Option<&[Rc<dyn FlexConfig>]> {
        Some(&self.configs)
    }
}

/// Whether two child configs are equal (Compose's `==`): the same one, both the default, or combining equal ones.
fn same_flex_config(a: &Rc<dyn FlexConfig>, b: &Rc<dyn FlexConfig>) -> bool {
    if Rc::ptr_eq(a, b) || (a.is_default_config() && b.is_default_config()) {
        return true;
    }
    match (a.combined_configs(), b.combined_configs()) {
        (Some(x), Some(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_flex_config(p, q)),
        _ => false,
    }
}

/// `first then second` (Compose's `then`, and the default's override).
fn then_flex_config(first: Rc<dyn FlexConfig>, second: Rc<dyn FlexConfig>) -> Rc<dyn FlexConfig> {
    if first.is_default_config() {
        return second;
    }
    if second.is_default_config() {
        return first;
    }
    let mut configs = vec![first];
    match second.combined_configs() {
        Some(parts) => configs.extend(parts.iter().cloned()),
        None => configs.push(second),
    }
    Rc::new(CombinedFlexConfig { configs })
}

/// Compose's `FlexConfig(vararg configs)`: the default ones left out, the one left if only one is.
fn flex_config_of(configs: Vec<Rc<dyn FlexConfig>>) -> Rc<dyn FlexConfig> {
    if configs.is_empty() {
        return Rc::new(DefaultFlexConfig);
    }
    if !configs.iter().any(|c| c.is_default_config()) {
        return Rc::new(CombinedFlexConfig { configs });
    }
    let mut filtered: Vec<Rc<dyn FlexConfig>> = configs.into_iter().filter(|c| !c.is_default_config()).collect();
    match filtered.len() {
        0 => Rc::new(DefaultFlexConfig),
        1 => filtered.pop().expect("one config"),
        _ => Rc::new(CombinedFlexConfig { configs: filtered }),
    }
}

/// Child configs combined, each over the ones before it, property by property (Compose's
/// `FlexConfig(first, second)`, `FlexConfig(first, second, third)` and `FlexConfig(vararg configs)`): two or three as
/// a tuple, any number as an array or a `Vec` of shared ones. The default ones are left out.
pub fn FlexConfig(configs: impl FlexConfigArgs) -> Rc<dyn FlexConfig> {
    configs.flex_config()
}

/// What [`FlexConfig()`](fn@FlexConfig) is given: Kotlin's arguments, several as a tuple, a vararg as an array or a
/// `Vec`.
pub trait FlexConfigArgs {
    fn flex_config(self) -> Rc<dyn FlexConfig>;
}

/// Kotlin: `FlexConfig(first: FlexConfig, second: FlexConfig)`
impl<A: FlexConfig + 'static, B: FlexConfig + 'static> FlexConfigArgs for (A, B) {
    fn flex_config(self) -> Rc<dyn FlexConfig> {
        self.0.then(self.1)
    }
}

/// Kotlin: `FlexConfig(first: FlexConfig, second: FlexConfig, third: FlexConfig)`
impl<A: FlexConfig + 'static, B: FlexConfig + 'static, C: FlexConfig + 'static> FlexConfigArgs for (A, B, C) {
    fn flex_config(self) -> Rc<dyn FlexConfig> {
        let (first, second, third) = (self.0.into_shared(), self.1.into_shared(), self.2.into_shared());
        if first.is_default_config() {
            return then_flex_config(second, third);
        }
        if second.is_default_config() {
            return then_flex_config(first, third);
        }
        if third.is_default_config() {
            return then_flex_config(first, second);
        }
        // (the combined ones spread, so combinations stay flat)
        let mut configs = Vec::new();
        for config in [first, second, third] {
            match config.combined_configs() {
                Some(parts) => configs.extend(parts.iter().cloned()),
                None => configs.push(config),
            }
        }
        flex_config_of(configs)
    }
}

/// Kotlin: `FlexConfig(vararg configs: FlexConfig)`
impl<const N: usize> FlexConfigArgs for [Rc<dyn FlexConfig>; N] {
    fn flex_config(self) -> Rc<dyn FlexConfig> {
        flex_config_of(self.into())
    }
}

/// A vararg's array made at run time.
impl FlexConfigArgs for Vec<Rc<dyn FlexConfig>> {
    fn flex_config(self) -> Rc<dyn FlexConfig> {
        flex_config_of(self)
    }
}

/// What a [`FlexConfig`] configures a child of a FlexBox on (Compose's `FlexConfigScope`, and the config part of
/// ResolvedFlexItemInfo behind it): the FlexBox's constraints along and across, its density (it derefs to it), and
/// the setters of the child's properties, each starting from Compose's default as the FlexBox measures.
pub struct FlexConfigScope {
    density: Density,
    flex_box_main_axis_max: i32,
    flex_box_main_axis_min: i32,
    flex_box_cross_axis_max: i32,
    flex_box_cross_axis_min: i32,
    align_self: FlexAlignSelf,
    order: i32,
    grow: f32,
    shrink: f32,
    basis: FlexBasis,
    baseline_alignment_line: Option<AlignmentLine>,
    baseline_alignment_block: Option<Rc<dyn Fn(&Placeable) -> i32>>,
}

impl std::ops::Deref for FlexConfigScope {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.density
    }
}

impl FlexConfigScope {
    /// Compose's defaults (a child without a config keeps them, and a density of 1).
    fn new() -> FlexConfigScope {
        FlexConfigScope {
            density: Density::new(1.0, 1.0),
            flex_box_main_axis_max: 0,
            flex_box_main_axis_min: 0,
            flex_box_cross_axis_max: 0,
            flex_box_cross_axis_min: 0,
            align_self: FlexAlignSelf::Auto,
            order: 0,
            grow: 0.0,
            shrink: 1.0,
            basis: FlexBasis::Auto,
            baseline_alignment_line: None,
            baseline_alignment_block: None,
        }
    }

    /// The FlexBox's density and constraints, for its config to read (Compose's `prepare`).
    fn prepare(&mut self, density: Density, constraints: &OrientationIndependentConstraints) {
        self.density = density;
        self.flex_box_main_axis_max = constraints.main_axis_max();
        self.flex_box_main_axis_min = constraints.main_axis_min();
        self.flex_box_cross_axis_max = constraints.cross_axis_max();
        self.flex_box_cross_axis_min = constraints.cross_axis_min();
    }

    /// The most the FlexBox may be along its main axis: its max width in a row, its max height in a column
    /// (Compose's `flexBoxMainAxisMax`).
    pub fn flex_box_main_axis_max(&self) -> i32 {
        self.flex_box_main_axis_max
    }

    /// The least the FlexBox may be along its main axis (Compose's `flexBoxMainAxisMin`).
    pub fn flex_box_main_axis_min(&self) -> i32 {
        self.flex_box_main_axis_min
    }

    /// The most the FlexBox may be across: its max height in a row, its max width in a column (Compose's
    /// `flexBoxCrossAxisMax`).
    pub fn flex_box_cross_axis_max(&self) -> i32 {
        self.flex_box_cross_axis_max
    }

    /// The least the FlexBox may be across (Compose's `flexBoxCrossAxisMin`).
    pub fn flex_box_cross_axis_min(&self) -> i32 {
        self.flex_box_cross_axis_min
    }

    /// How this child is aligned across its line, over the FlexBox's `align_items` (Compose's `alignSelf`);
    /// [`FlexAlignSelf::Auto`] (the FlexBox's) by default.
    pub fn align_self(&mut self, value: FlexAlignSelf) {
        self.align_self = value;
    }

    /// This child aligned by `alignment_line` with the others aligned by a baseline in its line (Compose's
    /// `alignSelf(alignmentLine)`); without it, by its far edge.
    /// Kotlin: `alignSelf(alignmentLine: AlignmentLine)`
    pub fn align_self_by_line(&mut self, alignment_line: impl Into<AlignmentLine>) {
        self.align_self = FlexAlignSelf::Baseline;
        self.baseline_alignment_line = Some(alignment_line.into());
        self.baseline_alignment_block = None;
    }

    /// This child aligned in its line by the position `alignment_line_block` works out from it measured (Compose's
    /// `alignSelf { measured -> … }`).
    /// Kotlin: `alignSelf(alignmentLineBlock: (Measured) -> Int)`
    pub fn align_self_by(&mut self, alignment_line_block: impl Fn(&Placeable) -> i32 + 'static) {
        self.align_self = FlexAlignSelf::Baseline;
        self.baseline_alignment_line = None;
        self.baseline_alignment_block = Some(Rc::new(alignment_line_block));
    }

    /// Where this child goes among its siblings: by ascending order, those of the same in the order they are
    /// composed (Compose's `order`); 0 by default.
    pub fn order(&mut self, value: i32) {
        self.order = value;
    }

    /// How much of the room left along its line this child takes, against its siblings' (Compose's `grow`); 0 (it
    /// doesn't grow) by default. Panics on a negative one.
    pub fn grow(&mut self, value: f32) {
        assert!(value >= 0.0, "Flex grow cannot be negative: {}", value.kt_to_string());
        self.grow = value;
    }

    /// How much this child shrinks when its line doesn't fit, against its siblings' (by its basis too: Compose's
    /// `shrink`); 1 by default, never under its min intrinsic size. Panics on a negative one.
    pub fn shrink(&mut self, value: f32) {
        assert!(value >= 0.0, "Flex shrink cannot be negative: {}", value.kt_to_string());
        self.shrink = value;
    }

    /// This child's size along before it grows or shrinks (Compose's `basis`); [`FlexBasis::Auto`] by default.
    pub fn basis(&mut self, value: FlexBasis) {
        self.basis = value;
    }

    /// A basis of `value` (Compose's `basis(value: Dp)`: `FlexBasis.Dp(value)`).
    /// Kotlin: `basis(value: Dp)`
    pub fn basis_dp(&mut self, value: Dp) {
        self.basis = FlexBasis::Dp(value);
    }

    /// A basis of a fraction of the FlexBox's main axis (Compose's `basis(value: Float)`: `FlexBasis.Percent`).
    /// Kotlin: `basis(value: Float)`
    pub fn basis_percent(&mut self, value: f32) {
        self.basis = FlexBasis::Percent(value);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// FlexBoxScope: Modifier.flex

/// The modifier a child of a [`FlexBox`] tells it its flex with (Compose's `FlexBoxScope`).
#[named(trait FlexBoxScopeModifiers)]
impl Modifier {
    /// This child's flex in its FlexBox: how it grows, shrinks, its basis, its own alignment across and its order,
    /// as `flex_config` sets them as the FlexBox measures (Compose's `FlexBoxScope.flex(flexConfig)` and
    /// `flex { … }`): `Modifier.flex(|f: &mut FlexConfigScope| f.grow(1.0))`. Other layouts ignore it. A closure
    /// isn't equal to another, so its node is updated whenever it composes again, as Compose's new lambda; a shared
    /// config (`Rc<dyn FlexConfig>`) or the default one is equal to itself.
    fn flex(self, flex_config: impl FlexConfig + 'static) -> Modifier {
        self.then_element(FlexBoxChildElement { config: flex_config.into_shared() })
    }
}

/// The element of `flex` (Compose's FlexBoxChildElement).
struct FlexBoxChildElement {
    config: Rc<dyn FlexConfig>,
}

impl PartialEq for FlexBoxChildElement {
    fn eq(&self, other: &Self) -> bool {
        same_flex_config(&self.config, &other.config)
    }
}

impl ModifierNodeElement for FlexBoxChildElement {
    type Node = FlexBoxChildDataNode;
    fn create(&self) -> FlexBoxChildDataNode {
        FlexBoxChildDataNode { config: self.config.clone() }
    }
    fn update(&self, node: &mut FlexBoxChildDataNode) {
        node.config = self.config.clone();
    }
    fn name(&self) -> &'static str {
        "flex"
    }
}

/// The node of `flex` (Compose's FlexBoxChildDataNode): it tells its config, whatever the nodes after it said.
struct FlexBoxChildDataNode {
    config: Rc<dyn FlexConfig>,
}

impl ModifierNode for FlexBoxChildDataNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for FlexBoxChildDataNode {
    fn modify_parent_data(&mut self, _: &Density, _: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        Some(Box::new(FlexBoxChildData { config: self.config.clone() }))
    }
}

/// What a FlexBox's child tells it: its config.
struct FlexBoxChildData {
    config: Rc<dyn FlexConfig>,
}

// ---------------------------------------------------------------------------------------------------------------
// The values

/// The FlexBox's main axis, along which its children are laid out from its main start to its main end (Compose's
/// FlexDirection). Written as Compose's `toString()` writes it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlexDirection {
    bits: u8,
}

impl FlexDirection {
    /// Horizontal, from the start (the left, right to left the right) to the end.
    pub const Row: FlexDirection = FlexDirection { bits: 0 };
    /// Vertical, from the top to the bottom.
    pub const Column: FlexDirection = FlexDirection { bits: 1 };
    /// Horizontal, from the end to the start.
    pub const RowReverse: FlexDirection = FlexDirection { bits: 2 };
    /// Vertical, from the bottom to the top.
    pub const ColumnReverse: FlexDirection = FlexDirection { bits: 3 };
}

/// Whether a FlexBox's children stay on one line or wrap onto more (Compose's FlexWrap).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlexWrap {
    bits: u8,
}

impl FlexWrap {
    /// One line: children shrink to fit if they may, else overflow along the main axis.
    pub const NoWrap: FlexWrap = FlexWrap { bits: 0 };
    /// New lines after the first across (under it in a row).
    pub const Wrap: FlexWrap = FlexWrap { bits: 1 };
    /// New lines before the first across (over it in a row).
    pub const WrapReverse: FlexWrap = FlexWrap { bits: 2 };
}

/// How a FlexBox aligns its children across their lines (Compose's FlexAlignItems); a child can say otherwise
/// ([`FlexAlignSelf`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlexAlignItems {
    bits: u8,
}

impl FlexAlignItems {
    /// At the cross start of their line.
    pub const Start: FlexAlignItems = FlexAlignItems { bits: 0 };
    /// At the cross end of their line.
    pub const End: FlexAlignItems = FlexAlignItems { bits: 1 };
    /// Centered across their line.
    pub const Center: FlexAlignItems = FlexAlignItems { bits: 2 };
    /// Stretched across their line.
    pub const Stretch: FlexAlignItems = FlexAlignItems { bits: 3 };
    /// Their baselines lined up (their first, or their bottoms for those without one).
    pub const Baseline: FlexAlignItems = FlexAlignItems { bits: 4 };
}

/// How a child of a FlexBox is aligned across its line, over the FlexBox's [`FlexAlignItems`] (Compose's
/// FlexAlignSelf).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlexAlignSelf {
    bits: u8,
}

impl FlexAlignSelf {
    /// As the FlexBox's `align_items` says.
    pub const Auto: FlexAlignSelf = FlexAlignSelf { bits: 0 };
    /// At the cross start of its line.
    pub const Start: FlexAlignSelf = FlexAlignSelf { bits: 1 };
    /// At the cross end of its line.
    pub const End: FlexAlignSelf = FlexAlignSelf { bits: 2 };
    /// Centered across its line.
    pub const Center: FlexAlignSelf = FlexAlignSelf { bits: 3 };
    /// Stretched across its line.
    pub const Stretch: FlexAlignSelf = FlexAlignSelf { bits: 4 };
    /// Its baseline lined up with the others' in its line (its bottom if it has none).
    pub const Baseline: FlexAlignSelf = FlexAlignSelf { bits: 5 };
}

/// How a FlexBox spreads its lines across when it wraps and has room left (Compose's FlexAlignContent).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlexAlignContent {
    bits: u8,
}

impl FlexAlignContent {
    /// Packed at the cross start.
    pub const Start: FlexAlignContent = FlexAlignContent { bits: 0 };
    /// Packed at the cross end.
    pub const End: FlexAlignContent = FlexAlignContent { bits: 1 };
    /// Packed in the middle.
    pub const Center: FlexAlignContent = FlexAlignContent { bits: 2 };
    /// The room left shared among the lines, made bigger.
    pub const Stretch: FlexAlignContent = FlexAlignContent { bits: 3 };
    /// The room left between them, none before the first or after the last.
    pub const SpaceBetween: FlexAlignContent = FlexAlignContent { bits: 4 };
    /// The room left around each, half as much at the ends as between two.
    pub const SpaceAround: FlexAlignContent = FlexAlignContent { bits: 5 };
}

/// How a FlexBox spreads the children of a line along it (Compose's FlexJustifyContent).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlexJustifyContent {
    bits: u8,
}

impl FlexJustifyContent {
    /// Packed at the main start.
    pub const Start: FlexJustifyContent = FlexJustifyContent { bits: 0 };
    /// Packed at the main end.
    pub const End: FlexJustifyContent = FlexJustifyContent { bits: 1 };
    /// Packed in the middle.
    pub const Center: FlexJustifyContent = FlexJustifyContent { bits: 2 };
    /// The room left between them, none before the first or after the last.
    pub const SpaceBetween: FlexJustifyContent = FlexJustifyContent { bits: 3 };
    /// The room left around each, half as much at the ends as between two.
    pub const SpaceAround: FlexJustifyContent = FlexJustifyContent { bits: 4 };
    /// The same room before, between and after them.
    pub const SpaceEvenly: FlexJustifyContent = FlexJustifyContent { bits: 5 };
}

/// `Display` and `Debug` as Compose's `toString()`: each value's name.
macro_rules! named_values {
    ($($ty:ident: [$($name:literal),*]),* $(,)?) => {$(
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                const NAMES: &[&str] = &[$($name),*];
                f.write_str(NAMES.get(self.bits as usize).copied().unwrap_or("INVALID"))
            }
        }
        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Display::fmt(self, f)
            }
        }
    )*};
}

named_values!(
    FlexDirection: ["Row", "Column", "RowReverse", "ColumnReverse"],
    FlexWrap: ["NoWrap", "Wrap", "WrapReverse"],
    FlexAlignItems: ["Start", "End", "Center", "Stretch", "Baseline"],
    FlexAlignSelf: ["Auto", "Start", "End", "Center", "Stretch", "Baseline"],
    FlexAlignContent: ["Start", "End", "Center", "Stretch", "SpaceBetween", "SpaceAround"],
    FlexJustifyContent: ["Start", "End", "Center", "SpaceBetween", "SpaceAround", "SpaceEvenly"],
);

/// A child's size along before it grows or shrinks (Compose's FlexBasis, packed as Compose packs it): auto, a
/// size in dp, or a fraction of the FlexBox's main axis. Written as Compose's `toString()` writes it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlexBasis {
    packed_value: u64,
}

const TYPE_SHIFT: u32 = 32;
const TYPE_AUTO: u64 = 0;
const TYPE_DP: u64 = 1;
const TYPE_PERCENT: u64 = 2;

impl FlexBasis {
    /// Its max intrinsic size along (the size its size modifier gives it, if it has one), the default.
    pub const Auto: FlexBasis = FlexBasis { packed_value: TYPE_AUTO << TYPE_SHIFT };

    /// `value` (Compose's `FlexBasis.Dp(value)`).
    pub fn Dp(value: Dp) -> FlexBasis {
        FlexBasis { packed_value: (TYPE_DP << TYPE_SHIFT) | value.0.to_bits() as u64 }
    }

    /// `value` (0 to 1) of the FlexBox's max size along; its max intrinsic size if that isn't bounded (Compose's
    /// `FlexBasis.Percent(value)`).
    pub fn Percent(value: f32) -> FlexBasis {
        FlexBasis { packed_value: (TYPE_PERCENT << TYPE_SHIFT) | value.to_bits() as u64 }
    }

    fn is_auto(&self) -> bool {
        self.packed_value >> TYPE_SHIFT == TYPE_AUTO
    }

    fn is_dp(&self) -> bool {
        self.packed_value >> TYPE_SHIFT == TYPE_DP
    }

    fn is_percent(&self) -> bool {
        self.packed_value >> TYPE_SHIFT == TYPE_PERCENT
    }

    fn value(&self) -> f32 {
        f32::from_bits(self.packed_value as u32)
    }
}

impl fmt::Display for FlexBasis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_auto() {
            f.write_str("FlexBasis.Auto")
        } else if self.is_dp() {
            write!(f, "FlexBasis.Dp({}.dp)", self.value().kt_to_string())
        } else if self.is_percent() {
            write!(f, "FlexBasis.Percent({}%)", round(self.value() * 100.0))
        } else {
            f.write_str("FlexBasis.Unknown")
        }
    }
}

impl fmt::Debug for FlexBasis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Measuring (Compose's FlexBoxMeasurePolicy)

/// Compose's FlexBoxMeasurePolicy: its config resolved for each measurement.
struct FlexBoxMeasurePolicy {
    config: Rc<dyn FlexBoxConfig>,
}

impl FlexBoxMeasurePolicy {
    /// The config, run for `constraints` (Compose's `resolveFlexBoxConfig`).
    fn resolve(&self, density: Density, constraints: Constraints) -> FlexBoxConfigScope {
        let mut scope = FlexBoxConfigScope::new(density, constraints);
        self.config.configure(&mut scope);
        scope
    }
}

/// A child as the FlexBox works on it (Compose's ResolvedFlexItemInfo): its config, and where the algorithm is
/// with it.
struct FlexItem<'a> {
    config: FlexConfigScope,
    measurable: &'a dyn Measurable,
    // (cached; -1 until asked)
    min_main_axis_size: i32,
    max_content_size: i32,
    // The flex algorithm's state.
    flex_base_size: i32,
    hypothetical_main_size: i32,
    target_main_size: i32,
    is_frozen: bool,
    // Where it goes.
    main_position: i32,
    cross_position: i32,
    // Measured.
    placeable: Option<Placeable>,
    baseline: i32,
    cross_axis_size: i32,
}

impl FlexItem<'_> {
    fn has_baseline(&self) -> bool {
        self.config.align_self == FlexAlignSelf::Baseline
    }

    /// Its min intrinsic size along (Compose's `getMinMainAxisSize`).
    fn get_min_main_axis_size(&mut self, is_horizontal: bool) -> i32 {
        if self.min_main_axis_size == -1 {
            self.min_main_axis_size = if is_horizontal {
                self.measurable.min_intrinsic_width(Constraints::Infinity)
            } else {
                self.measurable.min_intrinsic_height(Constraints::Infinity)
            };
        }
        self.min_main_axis_size
    }

    /// Its max intrinsic size along (Compose's `getMaxContentSize`).
    fn get_max_content_size(&mut self, is_horizontal: bool) -> i32 {
        if self.max_content_size == -1 {
            self.max_content_size = if is_horizontal {
                self.measurable.max_intrinsic_width(Constraints::Infinity)
            } else {
                self.measurable.max_intrinsic_height(Constraints::Infinity)
            };
        }
        self.max_content_size
    }

    /// Where its baseline is when it aligns by one (Compose's `getBaseline`), else `Unspecified`.
    fn get_baseline(&self, placeable: &Placeable, fallback: &FlexBoxConfigScope) -> i32 {
        if let Some(block) = &self.config.baseline_alignment_block {
            block(placeable)
        } else if let Some(line) = self.config.baseline_alignment_line {
            line_or_far_edge(placeable, line)
        } else if self.config.align_self == FlexAlignSelf::Baseline
            || (fallback.has_baseline() && self.config.align_self == FlexAlignSelf::Auto)
        {
            fallback.get_baseline(placeable)
        } else {
            AlignmentLine::Unspecified
        }
    }
}

/// A line of children: those from `start_index` to `end_index` (Compose's FlexLine).
#[derive(Default)]
struct FlexLine {
    start_index: usize,
    end_index: usize,
    main_axis_size: i32,
    cross_axis_size: i32,
    cross_start: i32,
    max_above_baseline: i32,
}

impl MeasurePolicy for FlexBoxMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        if measurables.is_empty() {
            return scope.layout(constraints.min_width, constraints.min_height, |_| {});
        }
        let config = self.resolve(scope.density(), constraints);
        let is_horizontal = config.is_horizontal();
        let orientation = if is_horizontal { LayoutOrientation::Horizontal } else { LayoutOrientation::Vertical };
        measure_flex_box(
            scope,
            &config,
            measurables,
            OrientationIndependentConstraints::from(constraints, orientation),
            is_horizontal,
        )
    }

    fn min_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        if measurables.is_empty() {
            return 0;
        }
        let config = self.resolve(scope.density(), Constraints::new(0, Constraints::Infinity, 0, height));
        if config.is_horizontal() {
            // Along: the narrowest without clipping: the widest child if it wraps (each could be a line of its
            // own), else all of them on the line.
            let gap = config.main_axis_gap();
            if config.is_wrap_enabled() {
                measurables.iter().map(|m| m.min_intrinsic_width(height)).max().unwrap_or(0)
            } else {
                sum_by(measurables, |m| m.min_intrinsic_width(height))
                    .wrapping_add((measurables.len() as i32 - 1).max(0).wrapping_mul(gap))
            }
        } else {
            // Across: the lines it would break into along the height, the widest of each.
            intrinsic_cross_axis_size(
                &config,
                measurables,
                height,
                |m| m.min_intrinsic_height(Constraints::Infinity),
                |m, main_size| m.min_intrinsic_width(main_size),
            )
        }
    }

    fn min_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        if measurables.is_empty() {
            return 0;
        }
        let config = self.resolve(scope.density(), Constraints::new(0, width, 0, Constraints::Infinity));
        if config.is_horizontal() {
            // Across: the lines it would break into along the width, the tallest of each, and the gaps.
            intrinsic_cross_axis_size(
                &config,
                measurables,
                width,
                |m| m.min_intrinsic_width(Constraints::Infinity),
                |m, main_size| m.min_intrinsic_height(main_size),
            )
        } else {
            // Along.
            let gap = config.main_axis_gap();
            if config.is_wrap_enabled() {
                measurables.iter().map(|m| m.min_intrinsic_height(width)).max().unwrap_or(0)
            } else {
                sum_by(measurables, |m| m.min_intrinsic_height(width))
                    .wrapping_add((measurables.len() as i32 - 1).max(0).wrapping_mul(gap))
            }
        }
    }

    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        if measurables.is_empty() {
            return 0;
        }
        let config = self.resolve(scope.density(), Constraints::new(0, Constraints::Infinity, 0, height));
        if config.is_horizontal() {
            // Along: all of them on one line, wrapping or not.
            let gap = config.main_axis_gap();
            sum_by(measurables, |m| m.max_intrinsic_width(height))
                .wrapping_add((measurables.len() as i32 - 1).max(0).wrapping_mul(gap))
        } else {
            // Across.
            intrinsic_cross_axis_size(
                &config,
                measurables,
                height,
                |m| m.max_intrinsic_height(Constraints::Infinity),
                |m, main_size| m.max_intrinsic_width(main_size),
            )
        }
    }

    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        if measurables.is_empty() {
            return 0;
        }
        let config = self.resolve(scope.density(), Constraints::new(0, width, 0, Constraints::Infinity));
        if config.is_horizontal() {
            // Across.
            intrinsic_cross_axis_size(
                &config,
                measurables,
                width,
                |m| m.max_intrinsic_width(Constraints::Infinity),
                |m, main_size| m.max_intrinsic_height(main_size),
            )
        } else {
            // Along: all of them on one line.
            let gap = config.main_axis_gap();
            sum_by(measurables, |m| m.max_intrinsic_height(width))
                .wrapping_add((measurables.len() as i32 - 1).max(0).wrapping_mul(gap))
        }
    }
}

/// The sum of what `size` gives for each, as Kotlin's `Int`s add (wrapping).
fn sum_by(measurables: &[&dyn IntrinsicMeasurable], size: impl Fn(&dyn IntrinsicMeasurable) -> i32) -> i32 {
    measurables.iter().fold(0i32, |sum, m| sum.wrapping_add(size(*m)))
}

/// The intrinsic size across: the lines the children would break into along `main_axis_available`, each as big
/// across as its biggest child, and the gaps between them. An approximation, as Compose's: each child is asked its
/// size across at its own size along, not at the size the flex algorithm would give it.
fn intrinsic_cross_axis_size(
    config: &FlexBoxConfigScope,
    measurables: &[&dyn IntrinsicMeasurable],
    main_axis_available: i32,
    main_axis_size: impl Fn(&dyn IntrinsicMeasurable) -> i32,
    cross_axis_size: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
) -> i32 {
    let main_axis_gap = config.main_axis_gap();
    let cross_axis_gap = config.cross_axis_gap();

    let mut current_line_main_axis_size = 0i32;
    let mut current_line_cross_axis_size = 0i32;
    let mut total_cross_axis_size = 0i32;
    let mut is_first_item_in_line = true;

    for measurable in measurables {
        let item_main_axis_size = main_axis_size(*measurable);
        let item_cross_axis_size = cross_axis_size(*measurable, item_main_axis_size);
        // Would it (and the gap before it) overflow the line?
        let projected_line_size = if is_first_item_in_line {
            item_main_axis_size
        } else {
            current_line_main_axis_size.wrapping_add(main_axis_gap).wrapping_add(item_main_axis_size)
        };
        if config.is_wrap_enabled() && !is_first_item_in_line && projected_line_size > main_axis_available {
            // The line done; a new one from it.
            total_cross_axis_size =
                total_cross_axis_size.wrapping_add(current_line_cross_axis_size).wrapping_add(cross_axis_gap);
            current_line_main_axis_size = item_main_axis_size;
            current_line_cross_axis_size = item_cross_axis_size;
        } else {
            current_line_main_axis_size = projected_line_size;
            current_line_cross_axis_size = current_line_cross_axis_size.max(item_cross_axis_size);
            is_first_item_in_line = false;
        }
    }
    // The last line (no gap after it).
    total_cross_axis_size.wrapping_add(current_line_cross_axis_size)
}

/// The FlexBox's layout (Compose's `measureFlexBox`).
fn measure_flex_box(
    scope: &MeasureScope,
    config: &FlexBoxConfigScope,
    measurables: &[&dyn Measurable],
    constraints: OrientationIndependentConstraints,
    is_horizontal: bool,
) -> MeasureResult {
    // The children's states, and whether lines need their sizes across before the children are measured.
    let mut items: Vec<FlexItem> = Vec::with_capacity(measurables.len());
    let mut needs_upfront_cross_axis_calculation = config.need_upfront_cross_axis_calculation(&constraints);
    let mut needs_sorting = false;
    for measurable in measurables {
        let item = create_flex_item(scope, *measurable, is_horizontal, &constraints);
        if item.config.order != 0 {
            needs_sorting = true;
        }
        if item.config.align_self == FlexAlignSelf::Baseline || item.config.align_self == FlexAlignSelf::Stretch {
            needs_upfront_cross_axis_calculation = true;
        }
        items.push(item);
    }
    if needs_sorting {
        // (stable: those of the same order as they were composed)
        items.sort_by_key(|item| item.config.order);
    }

    let main_axis_gap = config.main_axis_gap();
    let cross_axis_gap = config.cross_axis_gap();

    // The lines.
    let (mut lines, mut total_lines_cross_size) = build_flex_lines(
        config,
        &mut items,
        &constraints,
        main_axis_gap,
        cross_axis_gap,
        needs_upfront_cross_axis_calculation,
    );

    // A single line is as big across as the constraints' min, not only as its biggest child.
    if lines.len() == 1 {
        lines[0].cross_axis_size = lines[0].cross_axis_size.max(constraints.cross_axis_min());
    }
    // `align-content: stretch`.
    total_lines_cross_size =
        apply_align_content_stretch(config, &mut lines, &constraints, total_lines_cross_size, cross_axis_gap);

    // The children measured.
    total_lines_cross_size = measure_flex_items(
        &mut lines,
        &mut items,
        config,
        total_lines_cross_size,
        needs_upfront_cross_axis_calculation,
        &constraints,
    );

    // Where the lines go across.
    calculate_line_cross_positions(
        config,
        total_lines_cross_size.max(constraints.cross_axis_min()),
        &mut lines,
        total_lines_cross_size,
        cross_axis_gap,
    );

    total_lines_cross_size = total_cross_axis_size(&lines, config.is_cross_axis_reverse());
    total_lines_cross_size = total_lines_cross_size.max(constraints.cross_axis_min());

    let max_line_main_size = lines.iter().map(|line| line.main_axis_size).max().unwrap_or(constraints.main_axis_min());
    let main_axis_size = coerce_in(
        max_line_main_size.max(constraints.main_axis_min()),
        constraints.main_axis_min(),
        constraints.main_axis_max(),
    );

    let cross_axis_size = coerce_in(total_lines_cross_size, constraints.cross_axis_min(), constraints.cross_axis_max());
    let (layout_width, layout_height) =
        if is_horizontal { (main_axis_size, cross_axis_size) } else { (cross_axis_size, main_axis_size) };

    // Where the children go (Compose works it out as it places them, from these same values).
    let is_main_axis_reverse =
        config.direction == FlexDirection::RowReverse || config.direction == FlexDirection::ColumnReverse;
    let container_main_axis_size = if is_horizontal { layout_width } else { layout_height };
    let mut placements = Vec::with_capacity(items.len());
    for line in &lines {
        position_items_on_main_axis(
            &mut items,
            config,
            container_main_axis_size,
            line,
            main_axis_gap,
            is_main_axis_reverse,
        );
        for item in &mut items[line.start_index..line.end_index] {
            let (x, y) = if is_horizontal {
                (item.main_position, item.cross_position)
            } else {
                (item.cross_position, item.main_position)
            };
            if let Some(placeable) = item.placeable.take() {
                placements.push((placeable, x, y));
            }
        }
    }
    scope.layout(layout_width, layout_height, move |p| {
        for (placeable, x, y) in &placements {
            p.place_relative(placeable, *x, *y);
        }
    })
}

/// A child's state: its config run, its basis, its size along before the room is shared (Compose's
/// `createFlexItem`).
fn create_flex_item<'a>(
    scope: &MeasureScope,
    measurable: &'a dyn Measurable,
    is_horizontal: bool,
    constraints: &OrientationIndependentConstraints,
) -> FlexItem<'a> {
    let mut config = FlexConfigScope::new();
    let parent_data = measurable.parent_data();
    if let Some(data) = parent_data.as_ref().and_then(|d| d.downcast_ref::<FlexBoxChildData>()) {
        config.prepare(scope.density(), constraints);
        data.config.configure(&mut config);
    }
    let mut item = FlexItem {
        config,
        measurable,
        min_main_axis_size: -1,
        max_content_size: -1,
        flex_base_size: 0,
        hypothetical_main_size: 0,
        target_main_size: 0,
        is_frozen: false,
        main_position: 0,
        cross_position: 0,
        placeable: None,
        baseline: AlignmentLine::Unspecified,
        cross_axis_size: 0,
    };
    // Its flex base size.
    let min_main_axis_size = item.get_min_main_axis_size(is_horizontal);
    let basis = item.config.basis;
    let flex_base_size = if basis.is_dp() {
        scope.round_to_px(Dp(basis.value()))
    } else if basis.is_percent() {
        if constraints.main_axis_max() == Constraints::Infinity || basis.value().is_nan() {
            item.get_max_content_size(is_horizontal)
        } else {
            // (Kotlin's Int × Float, to an Int toward zero)
            (constraints.main_axis_max() as f32 * basis.value()) as i32
        }
    } else {
        item.get_max_content_size(is_horizontal)
    };
    item.flex_base_size = flex_base_size;
    item.hypothetical_main_size = flex_base_size.max(min_main_axis_size);
    item.target_main_size = item.hypothetical_main_size;
    item
}

/// The children broken into lines as the wrapping says, each line's room shared out and (if needed now) its size
/// across worked out (Compose's `buildFlexLines`); with the lines' sizes across added up.
fn build_flex_lines(
    config: &FlexBoxConfigScope,
    items: &mut [FlexItem],
    constraints: &OrientationIndependentConstraints,
    main_axis_gap: i32,
    cross_axis_gap: i32,
    needs_upfront_cross_axis_calculation: bool,
) -> (Vec<FlexLine>, i32) {
    let mut lines = Vec::with_capacity(8);
    let mut current_line = FlexLine::default();
    let mut current_line_hypothetical_main_axis_size = 0i32;
    let mut current_cross_position = 0i32;
    let mut total_lines_cross_size = 0i32;
    let mut line_start_index = 0usize;
    let mut remaining_cross_axis_size = constraints.cross_axis_max();

    for index in 0..items.len() {
        let hypothetical_main_size = items[index].hypothetical_main_size;
        if config.is_wrap_enabled()
            && index > line_start_index
            && current_line_hypothetical_main_axis_size.wrapping_add(hypothetical_main_size)
                > constraints.main_axis_max()
        {
            // It doesn't fit: the line is done.
            current_line.start_index = line_start_index;
            current_line.end_index = index;
            process_flex_line(
                &mut current_line,
                items,
                config,
                // (without the gap after its last child)
                current_line_hypothetical_main_axis_size.wrapping_sub(main_axis_gap),
                needs_upfront_cross_axis_calculation,
                constraints,
                remaining_cross_axis_size,
            );
            total_lines_cross_size += current_line.cross_axis_size;
            current_line.cross_start = current_cross_position;
            current_cross_position += current_line.cross_axis_size + cross_axis_gap;
            remaining_cross_axis_size =
                (remaining_cross_axis_size - (current_line.cross_axis_size + cross_axis_gap)).max(0);
            lines.push(std::mem::take(&mut current_line));
            // A new line from this child.
            line_start_index = index;
            current_line_hypothetical_main_axis_size = hypothetical_main_size.wrapping_add(main_axis_gap);
        } else {
            current_line_hypothetical_main_axis_size = current_line_hypothetical_main_axis_size
                .wrapping_add(hypothetical_main_size)
                .wrapping_add(main_axis_gap);
        }
    }

    if line_start_index < items.len() {
        current_line.start_index = line_start_index;
        current_line.end_index = items.len();
        process_flex_line(
            &mut current_line,
            items,
            config,
            current_line_hypothetical_main_axis_size.wrapping_sub(main_axis_gap),
            needs_upfront_cross_axis_calculation,
            constraints,
            remaining_cross_axis_size,
        );
        total_lines_cross_size += current_line.cross_axis_size;
        current_line.cross_start = current_cross_position;
        lines.push(current_line);
    }
    (lines, total_lines_cross_size)
}

/// A line's room shared out among its children, and its size across if needed now (Compose's `processFlexLine`).
fn process_flex_line(
    line: &mut FlexLine,
    items: &mut [FlexItem],
    config: &FlexBoxConfigScope,
    current_line_hypothetical_main_axis_size: i32,
    needs_upfront_cross_axis_calculation: bool,
    constraints: &OrientationIndependentConstraints,
    remaining_cross_axis_size: i32,
) {
    line.main_axis_size = resolve_flexible_lengths(
        config.is_horizontal(),
        items,
        config,
        line.start_index,
        line.end_index,
        current_line_hypothetical_main_axis_size,
        constraints.main_axis_max(),
    );
    if needs_upfront_cross_axis_calculation {
        calculate_line_cross_axis_size(items, config, line, remaining_cross_axis_size);
    }
}

/// The children of a line grown into the room left or shrunk to fit, by their factors (CSS's "resolve flexible
/// lengths": Compose's `resolveFlexibleLengths`); the line's size along.
fn resolve_flexible_lengths(
    is_horizontal: bool,
    items: &mut [FlexItem],
    config: &FlexBoxConfigScope,
    start_index: usize,
    end_index: usize,
    hypothetical_line_size: i32,
    container_main_axis_size: i32,
) -> i32 {
    let item_count = (end_index - start_index) as i32;
    let total_gap = if item_count > 0 { (item_count - 1) * config.main_axis_gap() } else { 0 };
    let line = &mut items[start_index..end_index];

    if container_main_axis_size == Constraints::Infinity {
        let mut line_main_axis_size = total_gap;
        for item in line.iter_mut() {
            item.target_main_size = item.hypothetical_main_size;
            line_main_axis_size = line_main_axis_size.wrapping_add(item.target_main_size);
        }
        return line_main_axis_size;
    }

    let is_growing = hypothetical_line_size < container_main_axis_size;

    let mut unfrozen_count = 0;
    let mut sum_grow = 0f32;
    let mut sum_scaled_shrink = 0f32;
    let mut sum_flex_base_size = 0i32;
    let mut sum_frozen_target_size = 0i32;
    let mut sum_factors = 0f32;

    // The first pass: those that can't flex are frozen at their hypothetical size.
    for item in line.iter_mut() {
        let flex_factor = if is_growing { item.config.grow } else { item.config.shrink };
        if flex_factor == 0.0 || (!is_growing && item.flex_base_size < item.hypothetical_main_size) {
            item.target_main_size = item.hypothetical_main_size;
            item.is_frozen = true;
            sum_frozen_target_size = sum_frozen_target_size.wrapping_add(item.target_main_size);
        } else {
            item.is_frozen = false;
            unfrozen_count += 1;
            sum_flex_base_size = sum_flex_base_size.wrapping_add(item.flex_base_size);
            sum_factors += flex_factor;
            if is_growing {
                sum_grow += item.config.grow;
            } else {
                sum_scaled_shrink += item.config.shrink * item.flex_base_size as f32;
            }
        }
    }

    let initial_free_space = container_main_axis_size
        .wrapping_sub(total_gap)
        .wrapping_sub(sum_frozen_target_size)
        .wrapping_sub(sum_flex_base_size) as f32;

    let mut allocated_unfrozen_size = 0i32;

    if is_growing {
        // Growing has no upper bound: one pass.
        let sum_sizes = total_gap.wrapping_add(sum_frozen_target_size).wrapping_add(sum_flex_base_size);
        let mut free_space = container_main_axis_size.wrapping_sub(sum_sizes) as f32;
        // (factors adding up to less than 1 take only that share of the room)
        if sum_factors < 1.0 {
            let fractional_space = initial_free_space * sum_factors;
            if fractional_space.abs() < free_space.abs() {
                free_space = fractional_space;
            }
        }
        let mut current_free_space = free_space;
        let mut current_sum_grow = sum_grow;
        for item in line.iter_mut() {
            if !item.is_frozen {
                let share = if current_sum_grow > 0.0 { item.config.grow / current_sum_grow } else { 0.0 };
                let space_to_allocate = round(current_free_space * share);
                current_free_space -= space_to_allocate as f32;
                current_sum_grow -= item.config.grow;
                item.target_main_size = item.flex_base_size.wrapping_add(space_to_allocate);
                allocated_unfrozen_size = allocated_unfrozen_size.wrapping_add(item.target_main_size);
            }
        }
    } else {
        // Shrinking: one that would go under its min size is frozen there, and the others share again.
        let mut needs_redistribution = true;
        let mut loop_count = 0;
        while needs_redistribution && loop_count < item_count {
            needs_redistribution = false;
            loop_count += 1;
            allocated_unfrozen_size = 0;
            if unfrozen_count == 0 {
                break;
            }
            let sum_sizes = total_gap.wrapping_add(sum_frozen_target_size).wrapping_add(sum_flex_base_size);
            let mut free_space = container_main_axis_size.wrapping_sub(sum_sizes) as f32;
            if sum_factors < 1.0 {
                let fractional_space = initial_free_space * sum_factors;
                if fractional_space.abs() < free_space.abs() {
                    free_space = fractional_space;
                }
            }
            let mut current_free_space = free_space.abs();
            let mut current_sum_scaled_shrink = sum_scaled_shrink;
            for item in line.iter_mut() {
                if item.is_frozen {
                    continue;
                }
                let scaled_shrink = item.config.shrink * item.flex_base_size as f32;
                let share =
                    if current_sum_scaled_shrink > 0.0 { scaled_shrink / current_sum_scaled_shrink } else { 0.0 };
                let space_to_take = round(current_free_space * share);
                let target_size = item.flex_base_size.wrapping_sub(space_to_take);
                let min_size = item.get_min_main_axis_size(is_horizontal);
                if target_size < min_size {
                    item.is_frozen = true;
                    needs_redistribution = true;
                    let actual_space_taken = item.flex_base_size - min_size;
                    current_free_space -= actual_space_taken as f32;
                    current_sum_scaled_shrink -= scaled_shrink;
                    item.target_main_size = min_size;
                    unfrozen_count -= 1;
                    sum_frozen_target_size = sum_frozen_target_size.wrapping_add(min_size);
                    sum_flex_base_size = sum_flex_base_size.wrapping_sub(item.flex_base_size);
                    sum_scaled_shrink -= scaled_shrink;
                    sum_factors -= item.config.shrink;
                } else {
                    current_free_space -= space_to_take as f32;
                    current_sum_scaled_shrink -= scaled_shrink;
                    item.target_main_size = target_size;
                    allocated_unfrozen_size = allocated_unfrozen_size.wrapping_add(item.target_main_size);
                }
            }
        }
    }

    total_gap.wrapping_add(sum_frozen_target_size).wrapping_add(allocated_unfrozen_size)
}

/// The room left across shared among the lines, for `align-content: stretch` (Compose's
/// `applyAlignContentStretch`); the lines' sizes across added up again.
fn apply_align_content_stretch(
    config: &FlexBoxConfigScope,
    lines: &mut [FlexLine],
    constraints: &OrientationIndependentConstraints,
    total_lines_cross_size: i32,
    cross_axis_gap: i32,
) -> i32 {
    if config.align_content != FlexAlignContent::Stretch
        || constraints.cross_axis_min() == Constraints::Infinity
        || lines.len() <= 1
    {
        return total_lines_cross_size;
    }
    let total_spacing = (lines.len() as i32 - 1) * cross_axis_gap;
    let container_cross_axis_size = constraints.cross_axis_min();
    if total_lines_cross_size + total_spacing >= container_cross_axis_size {
        return total_lines_cross_size;
    }
    let mut updated_total_cross_axis_size = total_lines_cross_size;
    let extra_space = (container_cross_axis_size - total_lines_cross_size - total_spacing).max(0);
    let space_per_line = extra_space / lines.len() as i32;
    let mut current_y = 0;
    for line in lines.iter_mut() {
        line.cross_start = current_y;
        line.cross_axis_size += space_per_line;
        current_y += line.cross_axis_size + cross_axis_gap;
        updated_total_cross_axis_size += space_per_line;
    }
    updated_total_cross_axis_size
}

/// The children not measured yet measured, stretched across their line if they stretch (Compose's
/// `measureFlexItems`); the lines' sizes across added up (worked out from the children unless they were before).
fn measure_flex_items(
    lines: &mut [FlexLine],
    items: &mut [FlexItem],
    config: &FlexBoxConfigScope,
    total_lines_cross_size: i32,
    needs_upfront_cross_axis_calculation: bool,
    constraints: &OrientationIndependentConstraints,
) -> i32 {
    let mut updated_total_cross_size = total_lines_cross_size;
    let mut remaining_cross_axis_size = constraints.cross_axis_max();
    let line_count = lines.len();

    for line in lines.iter_mut() {
        let mut line_cross_axis_size = if needs_upfront_cross_axis_calculation { line.cross_axis_size } else { 0 };
        for item in &mut items[line.start_index..line.end_index] {
            if item.placeable.is_some() {
                if !needs_upfront_cross_axis_calculation {
                    line_cross_axis_size = line_cross_axis_size.max(item.cross_axis_size);
                }
                continue;
            }
            let should_stretch = item.config.align_self == FlexAlignSelf::Stretch
                || (item.config.align_self == FlexAlignSelf::Auto
                    && (config.align_items == FlexAlignItems::Stretch
                        || config.align_content == FlexAlignContent::Stretch));
            let cross_axis_size = measure_item(
                item,
                config,
                if should_stretch { line.cross_axis_size } else { 0 },
                should_stretch,
                remaining_cross_axis_size,
            );
            if !needs_upfront_cross_axis_calculation {
                line_cross_axis_size = line_cross_axis_size.max(cross_axis_size);
            }
            // A single line is as big across as the constraints' min, not only as its biggest child.
            if line_count == 1 {
                line_cross_axis_size = line_cross_axis_size.max(constraints.cross_axis_min());
            }
        }
        if !needs_upfront_cross_axis_calculation {
            line.cross_axis_size = line_cross_axis_size;
            updated_total_cross_size += line_cross_axis_size;
        }
        remaining_cross_axis_size = (remaining_cross_axis_size - line.cross_axis_size - config.cross_axis_gap()).max(0);
    }
    if needs_upfront_cross_axis_calculation { total_lines_cross_size } else { updated_total_cross_size }
}

/// Where each line starts across, as `align-content` spreads them (Compose's `calculateLineCrossPositions`).
fn calculate_line_cross_positions(
    config: &FlexBoxConfigScope,
    total_cross_axis_space: i32,
    lines: &mut [FlexLine],
    total_lines_cross_size: i32,
    cross_axis_gap: i32,
) {
    if lines.len() <= 1 {
        return;
    }
    let line_count = lines.len() as i32;
    let total_gap = (line_count - 1) * cross_axis_gap;
    let free_space = total_cross_axis_space - total_lines_cross_size - total_gap;

    let space_in_between_lines = match config.align_content {
        FlexAlignContent::SpaceAround => free_space / line_count,
        FlexAlignContent::SpaceBetween => free_space / (line_count - 1),
        _ => 0,
    };
    let reverse = config.is_cross_axis_reverse();
    // (reversed, the lines are placed from the cross end: the room left is at the start)
    let (packed_at_start, packed_at_end) = if reverse { (free_space, 0) } else { (0, free_space) };
    let mut cross_position = match config.align_content {
        FlexAlignContent::End => packed_at_end,
        FlexAlignContent::Center => free_space / 2,
        FlexAlignContent::SpaceAround => space_in_between_lines / 2,
        _ => packed_at_start,
    };
    let mut place = |line: &mut FlexLine| {
        line.cross_start = cross_position;
        cross_position += line.cross_axis_size + space_in_between_lines + cross_axis_gap;
    };
    if reverse {
        lines.iter_mut().rev().for_each(&mut place);
    } else {
        lines.iter_mut().for_each(&mut place);
    }
}

/// How far across the lines reach: the end of the last one placed (Compose's `totalCrossAxisSize`).
fn total_cross_axis_size(lines: &[FlexLine], is_reverse: bool) -> i32 {
    let line = if is_reverse { lines.first() } else { lines.last() };
    line.map_or(0, |line| line.cross_start + line.cross_axis_size)
}

/// Where the children of a line go along, as `justify-content` spreads them, and across in the line, as they
/// align (Compose's `positionItemsOnMainAxis`).
fn position_items_on_main_axis(
    items: &mut [FlexItem],
    config: &FlexBoxConfigScope,
    container_main_axis_size: i32,
    line: &FlexLine,
    main_axis_gap: i32,
    is_main_axis_reverse: bool,
) {
    let item_count = (line.end_index - line.start_index) as i32;
    if item_count == 0 {
        return;
    }
    let remaining_space = container_main_axis_size.wrapping_sub(line.main_axis_size);

    let space_between_items = match config.justify_content {
        FlexJustifyContent::SpaceAround => remaining_space / item_count,
        FlexJustifyContent::SpaceBetween if item_count > 1 => remaining_space / (item_count - 1),
        FlexJustifyContent::SpaceEvenly => remaining_space / (item_count + 1),
        _ => 0,
    };
    // (reversed, the children are placed from the main end: the room left is at the start)
    let (packed_at_start, packed_at_end) =
        if is_main_axis_reverse { (remaining_space, 0) } else { (0, remaining_space) };
    let mut main_position = match config.justify_content {
        FlexJustifyContent::End => packed_at_end,
        FlexJustifyContent::Center => remaining_space / 2,
        FlexJustifyContent::SpaceAround => space_between_items / 2,
        FlexJustifyContent::SpaceEvenly => space_between_items,
        // (a single child of a reversed line at its start)
        FlexJustifyContent::SpaceBetween if item_count == 1 => packed_at_start,
        FlexJustifyContent::SpaceBetween => 0,
        _ => packed_at_start,
    };

    let mut position = |item: &mut FlexItem| {
        item.main_position = main_position;
        main_position = main_position
            .wrapping_add(item.target_main_size)
            .wrapping_add(space_between_items)
            .wrapping_add(main_axis_gap);
        // Across: by its own alignment, else the FlexBox's.
        item.cross_position = line.cross_start
            + calculate_item_cross_position(
                item,
                line.max_above_baseline,
                item.cross_axis_size,
                line.cross_axis_size,
                config.align_items,
            );
    };
    let line_items = &mut items[line.start_index..line.end_index];
    if is_main_axis_reverse {
        line_items.iter_mut().rev().for_each(&mut position);
    } else {
        line_items.iter_mut().for_each(&mut position);
    }
}

/// Where a child goes across in its line (Compose's `calculateItemCrossPosition`).
fn calculate_item_cross_position(
    item: &FlexItem,
    line_max_above_baseline: i32,
    item_cross_axis_size: i32,
    line_cross_axis_size: i32,
    container_align_items: FlexAlignItems,
) -> i32 {
    let effective_alignment = if item.config.align_self != FlexAlignSelf::Auto {
        item.config.align_self
    } else {
        match container_align_items {
            FlexAlignItems::Start => FlexAlignSelf::Start,
            FlexAlignItems::End => FlexAlignSelf::End,
            FlexAlignItems::Center => FlexAlignSelf::Center,
            FlexAlignItems::Stretch => FlexAlignSelf::Stretch,
            FlexAlignItems::Baseline => FlexAlignSelf::Baseline,
            _ => FlexAlignSelf::Start,
        }
    };
    match effective_alignment {
        FlexAlignSelf::End => line_cross_axis_size - item_cross_axis_size,
        FlexAlignSelf::Center => (line_cross_axis_size - item_cross_axis_size) / 2,
        FlexAlignSelf::Baseline if item.baseline != AlignmentLine::Unspecified => {
            line_max_above_baseline - item.baseline
        }
        _ => 0,
    }
}

/// A line's size across, from its children: those aligned by a baseline measured and lined up, the others asked
/// their max intrinsic size across at their size along (Compose's `calculateLineCrossAxisSize`).
fn calculate_line_cross_axis_size(
    items: &mut [FlexItem],
    config: &FlexBoxConfigScope,
    line: &mut FlexLine,
    remaining_cross_axis_size: i32,
) {
    let mut line_cross_axis_size = 0;
    let mut max_above_baseline = 0;
    let mut max_below_baseline = 0;
    let is_horizontal = config.is_horizontal();
    for item in &mut items[line.start_index..line.end_index] {
        let cross_axis_size =
            if item.has_baseline() || (config.has_baseline() && item.config.align_self == FlexAlignSelf::Auto) {
                measure_item(item, config, 0, false, remaining_cross_axis_size);
                let placeable = item.placeable.as_ref().expect("measured");
                let baseline = item.get_baseline(placeable, config);
                item.baseline = baseline;
                max_above_baseline = max_above_baseline.max(baseline);
                max_below_baseline = max_below_baseline.max(item.cross_axis_size - baseline);
                max_above_baseline + max_below_baseline
            } else {
                item.cross_axis_size = if is_horizontal {
                    item.measurable.max_intrinsic_height(item.target_main_size)
                } else {
                    item.measurable.max_intrinsic_width(item.target_main_size)
                };
                item.cross_axis_size
            };
        line_cross_axis_size = line_cross_axis_size.max(cross_axis_size);
    }
    line.max_above_baseline = max_above_baseline;
    line.cross_axis_size = line_cross_axis_size;
}

/// A child measured at its size along, stretched to `line_cross_axis_size` if it stretches (Compose's
/// `measureItem`); its size across.
fn measure_item(
    item: &mut FlexItem,
    config: &FlexBoxConfigScope,
    line_cross_axis_size: i32,
    should_stretch: bool,
    remaining_cross_axis_size: i32,
) -> i32 {
    let is_horizontal = config.is_horizontal();
    let item_constraints = if should_stretch && line_cross_axis_size > 0 {
        let cross = line_cross_axis_size.min(remaining_cross_axis_size);
        if is_horizontal {
            Constraints::fixed(item.target_main_size, cross)
        } else {
            Constraints::fixed(cross, item.target_main_size)
        }
    } else if is_horizontal {
        Constraints::fit_prioritizing_width(item.target_main_size, item.target_main_size, 0, remaining_cross_axis_size)
    } else {
        Constraints::fit_prioritizing_height(0, remaining_cross_axis_size, item.target_main_size, item.target_main_size)
    };
    let placeable = item.measurable.measure(item_constraints);
    // (Compose keeps its size along too, read nowhere)
    item.cross_axis_size = if is_horizontal { placeable.height() } else { placeable.width() };
    item.placeable = Some(placeable);
    item.cross_axis_size
}
