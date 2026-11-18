//! Modifier locals (Compose's ModifierLocal, `modifierLocalOf`, ModifierLocalMap and ModifierLocalModifierNode): a
//! node provides values for locals to the nodes after it in its chain and in the layouts inside it; a node reads a
//! local's value as the nearest provider above it gives it (`NodeCx::modifier_local_current`), else the local's
//! default. (Compose's older `Modifier.modifierLocalProvider/Consumer` aren't ported: deprecated there in favour of
//! the node API.)

use super::{ModifierNode, NodeCx, NodeKind};
use std::any::Any;
use std::rc::Rc;

/// A local a modifier node provides to those after and inside it (Compose's ProvidableModifierLocal, made by
/// `modifierLocalOf`): its default when nothing provides it. Declare one as a `static`: it is known by its address.
pub struct ModifierLocal<T: 'static> {
    default_factory: fn() -> T,
}

impl<T: 'static> ModifierLocal<T> {
    /// A local whose value is `default_factory`'s where nothing provides it (Compose's `modifierLocalOf`).
    pub const fn new(default_factory: fn() -> T) -> ModifierLocal<T> {
        ModifierLocal { default_factory }
    }

    /// Its default.
    pub fn default_value(&self) -> T {
        (self.default_factory)()
    }

    fn key(&'static self) -> usize {
        self as *const ModifierLocal<T> as usize
    }
}

/// Compose's `modifierLocalOf(defaultFactory)`.
pub const fn modifier_local_of<T: 'static>(default_factory: fn() -> T) -> ModifierLocal<T> {
    ModifierLocal::new(default_factory)
}

/// The values a node provides (Compose's ModifierLocalMap): one for each local it provides.
#[derive(Clone, Default)]
pub struct ModifierLocalMap {
    values: Vec<(usize, Rc<dyn Any>)>,
}

impl ModifierLocalMap {
    /// A map providing `value` for `local` (Compose's `modifierLocalMapOf(key to value)`).
    pub fn of<T: Clone + 'static>(local: &'static ModifierLocal<T>, value: T) -> ModifierLocalMap {
        let mut map = ModifierLocalMap::default();
        map.set(local, value);
        map
    }

    /// Whether it provides `local`.
    pub fn contains<T: 'static>(&self, local: &'static ModifierLocal<T>) -> bool {
        self.values.iter().any(|(k, _)| *k == local.key())
    }

    /// The value it provides for `local`, if it does.
    pub fn get<T: Clone + 'static>(&self, local: &'static ModifierLocal<T>) -> Option<T> {
        let (_, v) = self.values.iter().find(|(k, _)| *k == local.key())?;
        v.downcast_ref::<T>().cloned()
    }

    /// Provides `value` for `local` (Compose's `ModifierLocalModifierNode.provide`, on its own map).
    pub fn set<T: Clone + 'static>(&mut self, local: &'static ModifierLocal<T>, value: T) {
        let value: Rc<dyn Any> = Rc::new(value);
        match self.values.iter_mut().find(|(k, _)| *k == local.key()) {
            Some(slot) => slot.1 = value,
            None => self.values.push((local.key(), value)),
        }
    }
}

/// A node providing modifier locals (Compose's ModifierLocalModifierNode): what it provides, read by the nodes
/// after it in its chain and inside its layout (`node_capabilities!(modifier_local)`).
pub trait ModifierLocalModifierNode: ModifierNode {
    fn provided_values(&self) -> &ModifierLocalMap;
}

impl NodeCx {
    /// `local`'s value as the nearest node above this one provides it — earlier in its chain, then in its
    /// ancestors' — else its default (Compose's `ModifierLocalModifierNode.current`). Not observed: read it again
    /// when it is needed.
    pub fn modifier_local_current<T: Clone + 'static>(&self, local: &'static ModifierLocal<T>) -> T {
        let mut found = None;
        self.visit_ancestors(NodeKind::Locals, false, &mut |cx| {
            let value = cx.with_modifier_local(|n| n.provided_values().get(local)).flatten();
            match value {
                Some(v) => {
                    found = Some(v);
                    false
                }
                None => true,
            }
        });
        found.unwrap_or_else(|| local.default_value())
    }

    fn with_modifier_local<R>(&self, f: impl FnOnce(&mut dyn ModifierLocalModifierNode) -> R) -> Option<R> {
        let mut node = self.cell.node.borrow_mut();
        node.as_modifier_local().map(f)
    }
}
