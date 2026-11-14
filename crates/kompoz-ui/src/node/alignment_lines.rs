//! A layout node's alignment lines: those of its placed children moved into it and merged, and its own (Compose's
//! LayoutNodeAlignmentLines), with who used them so a change invalidates what it must.

use super::{LayoutNode, NodeCoordinator};
use crate::layout::AlignmentLine;
use kompoz_graphics::Offset;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What a layout node knows of its alignment lines (Compose's AlignmentLines).
#[derive(Default)]
pub(crate) struct AlignmentLines {
    pub(crate) dirty: Cell<bool>,
    pub(crate) used_during_parent_measurement: Cell<bool>,
    pub(crate) used_during_parent_layout: Cell<bool>,
    pub(crate) previous_used_during_parent_layout: Cell<bool>,
    pub(crate) used_by_modifier_measurement: Cell<bool>,
    pub(crate) used_by_modifier_layout: Cell<bool>,
    /// The last calculation, in the node's inner coordinates.
    map: RefCell<Vec<(AlignmentLine, i32)>>,
}

impl AlignmentLines {
    pub(crate) fn new() -> AlignmentLines {
        let lines = AlignmentLines::default();
        lines.dirty.set(true);
        lines
    }

    /// Whether anyone asked for them.
    pub(crate) fn queried(&self) -> bool {
        self.used_during_parent_measurement.get()
            || self.previous_used_during_parent_layout.get()
            || self.used_by_modifier_measurement.get()
            || self.used_by_modifier_layout.get()
    }

    pub(crate) fn last_calculation(&self) -> Vec<(AlignmentLine, i32)> {
        self.map.borrow().clone()
    }

    pub(crate) fn get(&self, line: AlignmentLine) -> Option<i32> {
        self.map.borrow().iter().find(|(l, _)| *l == line).map(|(_, v)| *v)
    }

    pub(crate) fn reset(&self) {
        self.dirty.set(true);
        self.used_during_parent_measurement.set(false);
        self.previous_used_during_parent_layout.set(false);
        self.used_during_parent_layout.set(false);
        self.used_by_modifier_measurement.set(false);
        self.used_by_modifier_layout.set(false);
    }

    fn add(&self, owner: &LayoutNode, line: AlignmentLine, initial: i32, from: &Rc<NodeCoordinator>) {
        let mut position = Offset::new(initial as f32, initial as f32);
        let mut coordinator = from.clone();
        loop {
            position = coordinator.to_parent_position(position);
            let Some(up) = coordinator.wrapped_by() else { break };
            coordinator = up;
            if Rc::ptr_eq(&coordinator, &owner.inner) {
                break;
            }
            if coordinator.provides_line(line) {
                let p = coordinator.alignment_line(line) as f32;
                position = Offset::new(p, p);
            }
        }
        let in_container = (if line.is_horizontal() { position.y } else { position.x }).round() as i32;
        let mut map = self.map.borrow_mut();
        match map.iter_mut().find(|(l, _)| *l == line) {
            Some((_, v)) => *v = line.merge(*v, in_container),
            None => map.push((line, in_container)),
        }
    }
}

impl LayoutNode {
    /// Whether its alignment lines are needed (asked for here or by an ancestor that asked for its own: Compose's
    /// `AlignmentLines.required`).
    pub(crate) fn alignment_lines_required(&self) -> bool {
        if self.alignment_lines.queried() {
            return true;
        }
        self.parent().is_some_and(|p| p.alignment_lines_required())
    }

    /// Works its lines out again (Compose's `AlignmentLines.recalculate`): each placed child's, moved up through
    /// its coordinators and merged, and the lines its own coordinators below the inner one give, then those its
    /// measure policy gives.
    pub(crate) fn recalculate_alignment_lines(&self) {
        self.alignment_lines.map.borrow_mut().clear();
        for child in self.children() {
            if child.place_order.get() == NOT_PLACED {
                continue;
            }
            if child.alignment_lines.dirty.get() {
                child.layout_children();
            }
            for (line, position) in child.alignment_lines.last_calculation() {
                self.alignment_lines.add(self, line, position, &child.inner);
            }
            let mut coordinator = child.inner.wrapped_by();
            while let Some(c) = coordinator {
                if Rc::ptr_eq(&c, &self.inner) {
                    break;
                }
                for line in c.measure_result_lines() {
                    let position = c.alignment_line(line);
                    self.alignment_lines.add(self, line, position, &c);
                }
                coordinator = c.wrapped_by();
            }
        }
        let own: Vec<(AlignmentLine, i32)> = self.inner.measure_result_line_values();
        {
            let mut map = self.alignment_lines.map.borrow_mut();
            for (line, v) in own {
                match map.iter_mut().find(|(l, _)| *l == line) {
                    Some((_, value)) => *value = v,
                    None => map.push((line, v)),
                }
            }
        }
        self.alignment_lines.dirty.set(false);
    }

    /// Its lines may have changed (Compose's `onAlignmentsChanged`): who used them measures or lays out again,
    /// and so on up.
    pub(crate) fn on_alignments_changed(&self) {
        let lines = &self.alignment_lines;
        lines.dirty.set(true);
        let Some(parent) = self.parent() else { return };
        if lines.used_during_parent_measurement.get() {
            parent.request_remeasure(false);
        } else if lines.previous_used_during_parent_layout.get() || lines.used_during_parent_layout.get() {
            parent.request_relayout_forced(false);
        }
        if lines.used_by_modifier_measurement.get() {
            self.request_remeasure(false);
        }
        if lines.used_by_modifier_layout.get() {
            self.request_relayout_forced(false);
        }
        parent.on_alignments_changed();
    }
}

/// A child not placed by its parent in the last placement (Compose's NotPlacedPlaceOrder).
pub(crate) const NOT_PLACED: i32 = i32::MAX;
