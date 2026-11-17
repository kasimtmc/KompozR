//! The `Layout` composables (Compose's `Layout`, `MultiMeasureLayout` and `Layout(contents, …)`): a layout node
//! emitted with a measure policy, a modifier and the content that emits its children (several contents, each in a
//! virtual node, for a multi-content policy).

use super::{
    DefaultIntrinsicMeasurable, IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasurePolicy, MeasureResult,
    MeasureScope, MinMax, Side,
};
use crate::modifier::Modifier;
use crate::node::LayoutNode;
use crate::platform::{LocalDensity, LocalLayoutDirection};
use crate::unit::Constraints;
use kompoz_runtime::{Memoized, Node, current_composite_key_hash, current_kompozition_local_map, emit, kompozable};
use std::any::Any;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// A measure policy from a closure (Compose's `MeasurePolicy { measurables, constraints -> … }`), its parameter
/// types inferred: `Layout(content = …, measure_policy(|scope, measurables, constraints| scope.layout(…)))`.
pub fn measure_policy<F>(measure: F) -> F
where
    F: Fn(&MeasureScope, &[&dyn Measurable], Constraints) -> MeasureResult + 'static,
{
    measure
}

/// A layout: the children `content` emits, measured and placed by `measure_policy`, decorated by `modifier`
/// (Compose's `Layout`). Without content, a leaf (Compose's `Layout(modifier, measurePolicy)`). A closure policy
/// written in a kompozable body is memoized by what it captures, as Compose's strong skipping remembers a lambda:
/// while those values stay the same, the policy kept is the one the layout has, which isn't measured again for it;
/// a new one (other values captured, or one the body can't compare) measures it again.
#[kompozable(inline)]
pub fn Layout(
    #[default(|| {})] content: impl FnOnce(),
    #[default(Modifier)] modifier: Modifier,
    #[memoized] measure_policy: impl MeasurePolicy,
) {
    layout_into(modifier, kept_policy(measure_policy), false, content);
}

/// A layout whose policy may measure a child more than once (Compose's deprecated `MultiMeasureLayout`).
#[kompozable(inline)]
pub fn MultiMeasureLayout(
    #[default(Modifier)] modifier: Modifier,
    content: impl FnOnce(),
    #[memoized] measure_policy: impl MeasurePolicy,
) {
    layout_into(modifier, kept_policy(measure_policy), true, content);
}

/// The policy a layout keeps for `policy`: the one it has while `policy` is the same memoized closure (so the
/// node, comparing policies by identity, isn't measured again), else a new one.
#[track_caller]
fn kept_policy<P: MeasurePolicy>(policy: Memoized<P>) -> Rc<dyn MeasurePolicy> {
    policy.remember(|p| Rc::new(p) as Rc<dyn MeasurePolicy>)
}

/// Emits a layout node (what `Layout` does, for composables that keep their measure policy: `Box`, `Row`, …):
/// its policy, density, layout direction and composition locals set, then its modifier, as Compose's
/// ComposeUiNode setters, then its children.
#[track_caller]
pub fn layout_into(
    modifier: Modifier,
    measure_policy: Rc<dyn MeasurePolicy>,
    multi_measure: bool,
    content: impl FnOnce(),
) {
    let density = LocalDensity.current();
    let layout_direction = LocalLayoutDirection.current();
    let locals = current_kompozition_local_map();
    let key = current_composite_key_hash();
    emit(
        LayoutNode::new,
        move |node: &LayoutNode| {
            node.set_measure_policy(measure_policy);
            node.can_multi_measure.set(multi_measure);
            node.set_density(density);
            node.set_layout_direction(layout_direction);
            node.set_locals(locals);
            node.modifier_set(modifier);
            node.set_composite_key(key);
            node.note_composed();
        },
        content,
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Layouts of several contents (Compose's MultiContentMeasurePolicy.kt and Layout(contents, …))

/// A measure policy of a layout of several contents (Compose's `MultiContentMeasurePolicy`): it measures the
/// layouts each content emitted, a list for each, in the contents' order. Its intrinsics measure it with each child
/// answering with its intrinsic size, unless it says otherwise.
pub trait MultiContentMeasurePolicy: 'static {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult;

    fn min_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        height: i32,
    ) -> i32 {
        default_multi_content_intrinsic(self, scope, measurables, MinMax::Min, Side::Width, height)
    }
    fn min_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        width: i32,
    ) -> i32 {
        default_multi_content_intrinsic(self, scope, measurables, MinMax::Min, Side::Height, width)
    }
    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        height: i32,
    ) -> i32 {
        default_multi_content_intrinsic(self, scope, measurables, MinMax::Max, Side::Width, height)
    }
    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        width: i32,
    ) -> i32 {
        default_multi_content_intrinsic(self, scope, measurables, MinMax::Max, Side::Height, width)
    }
}

impl<F> MultiContentMeasurePolicy for F
where
    F: Fn(&MeasureScope, &[Vec<&dyn Measurable>], Constraints) -> MeasureResult + 'static,
{
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult {
        self(scope, measurables, constraints)
    }
}

/// Compose's default intrinsics of a multi-content policy: measured with each child answering with its intrinsic
/// size, within `Constraints(maxHeight = height)` (or `maxWidth = width`).
fn default_multi_content_intrinsic<P: MultiContentMeasurePolicy + ?Sized>(
    policy: &P,
    scope: &IntrinsicMeasureScope,
    measurables: &[Vec<&dyn IntrinsicMeasurable>],
    min_max: MinMax,
    side: Side,
    size: i32,
) -> i32 {
    let mapped: Vec<Vec<DefaultIntrinsicMeasurable>> = measurables
        .iter()
        .map(|list| list.iter().map(|m| DefaultIntrinsicMeasurable { measurable: *m, min_max, side }).collect())
        .collect();
    let refs: Vec<Vec<&dyn Measurable>> =
        mapped.iter().map(|list| list.iter().map(|m| m as &dyn Measurable).collect()).collect();
    let constraints = match side {
        Side::Width => Constraints::new(0, Constraints::Infinity, 0, size),
        Side::Height => Constraints::new(0, size, 0, Constraints::Infinity),
    };
    let result = policy.measure(&scope.for_intrinsics(), &refs, constraints);
    match side {
        Side::Width => result.width,
        Side::Height => result.height,
    }
}

/// A layout of several contents (Compose's `Layout(contents, modifier, measurePolicy)`): the layouts each of
/// `contents` emits are its children (one after the other, drawn and hit as one list), and `measure_policy`
/// measures them a list per content. Each content is composed in a virtual node of its own (Compose's
/// `combineAsVirtualLayouts`).
///
/// Kotlin: `Layout(contents: List<@Composable () -> Unit>, modifier: Modifier = Modifier, measurePolicy: MultiContentMeasurePolicy)`
#[kompozable(inline)]
pub fn MultiContentLayout(
    contents: Vec<Rc<dyn Fn()>>,
    #[default(Modifier)] modifier: Modifier,
    measure_policy: Rc<dyn MultiContentMeasurePolicy>,
) {
    // (Compose's `remember(measurePolicy) { createMeasurePolicy(measurePolicy) }`: kept while it is the same one)
    let policy = kompoz_runtime::remember_keyed(PolicyKey(measure_policy.clone()), move || {
        Rc::new(MultiContentMeasurePolicyImpl { policy: measure_policy }) as Rc<dyn MeasurePolicy>
    });
    layout_into(modifier, policy, false, move || {
        for content in &contents {
            emit_virtual(content.clone());
        }
    });
}

/// A multi-content policy compared by its instance (Compose's `remember(measurePolicy)`).
struct PolicyKey(Rc<dyn MultiContentMeasurePolicy>);

impl PartialEq for PolicyKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Emits a virtual node holding what `content` emits (one of `combineAsVirtualLayouts`' nodes).
#[track_caller]
fn emit_virtual(content: Rc<dyn Fn()>) {
    emit(VirtualLayoutNode::new, |_: &VirtualLayoutNode| {}, move || content());
}

/// A virtual layout node (Compose's `LayoutNode(isVirtual = true)`): no layout of its own, its layouts are its
/// parent's children in its place; its parent's multi-content policy measures them as one list.
pub(crate) struct VirtualLayoutNode {
    nodes: RefCell<Vec<Rc<LayoutNode>>>,
    parent: RefCell<Weak<LayoutNode>>,
}

impl VirtualLayoutNode {
    fn new() -> Rc<VirtualLayoutNode> {
        Rc::new(VirtualLayoutNode { nodes: RefCell::new(Vec::new()), parent: RefCell::new(Weak::new()) })
    }

    /// The layouts it holds, in order.
    pub(crate) fn nodes(&self) -> Vec<Rc<LayoutNode>> {
        self.nodes.borrow().clone()
    }

    pub(crate) fn set_parent(&self, parent: Weak<LayoutNode>) {
        *self.parent.borrow_mut() = parent;
    }
}

impl Node for VirtualLayoutNode {
    fn set_children(&self, children: &[Rc<dyn Node>]) {
        let nodes = children
            .iter()
            .map(|c| (c.clone() as Rc<dyn Any>).downcast::<LayoutNode>().expect("kompoz: a node of another kind"))
            .collect();
        *self.nodes.borrow_mut() = nodes;
        let parent = self.parent.borrow().upgrade();
        if let Some(parent) = parent {
            parent.sync_virtual_children();
        }
    }
}

/// What a multi-content layout's node measures with (Compose's `MultiContentMeasurePolicyImpl`): its children's
/// lists by the virtual nodes they are in (Compose's `getChildrenOfVirtualChildren`).
struct MultiContentMeasurePolicyImpl {
    policy: Rc<dyn MultiContentMeasurePolicy>,
}

impl MultiContentMeasurePolicyImpl {
    fn groups(scope: &MeasureScope) -> Vec<Vec<Rc<LayoutNode>>> {
        scope.node.as_ref().and_then(Weak::upgrade).map(|n| n.virtual_children_groups()).unwrap_or_default()
    }
}

impl MeasurePolicy for MultiContentMeasurePolicyImpl {
    fn measure(&self, scope: &MeasureScope, _: &[&dyn Measurable], constraints: Constraints) -> MeasureResult {
        let groups = Self::groups(scope);
        let refs: Vec<Vec<&dyn Measurable>> =
            groups.iter().map(|g| g.iter().map(|n| &**n as &dyn Measurable).collect()).collect();
        self.policy.measure(scope, &refs, constraints)
    }

    fn min_intrinsic_width(&self, scope: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], height: i32) -> i32 {
        let groups = Self::groups(scope);
        self.policy.min_intrinsic_width(scope, &intrinsic_refs(&groups), height)
    }
    fn min_intrinsic_height(&self, scope: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        let groups = Self::groups(scope);
        self.policy.min_intrinsic_height(scope, &intrinsic_refs(&groups), width)
    }
    fn max_intrinsic_width(&self, scope: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], height: i32) -> i32 {
        let groups = Self::groups(scope);
        self.policy.max_intrinsic_width(scope, &intrinsic_refs(&groups), height)
    }
    fn max_intrinsic_height(&self, scope: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        let groups = Self::groups(scope);
        self.policy.max_intrinsic_height(scope, &intrinsic_refs(&groups), width)
    }
}

fn intrinsic_refs(groups: &[Vec<Rc<LayoutNode>>]) -> Vec<Vec<&dyn IntrinsicMeasurable>> {
    groups.iter().map(|g| g.iter().map(|n| &**n as &dyn IntrinsicMeasurable).collect()).collect()
}
