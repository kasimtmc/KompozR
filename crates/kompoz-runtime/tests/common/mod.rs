//! What the runtime's tests share: a node that records its children, a few composables emitting it, a rule that
//! composes content and runs the frames until nothing is left to do, and a runner that runs the tests one after
//! the other on one thread (the runtime's UI thread), as the UI does.

#![allow(dead_code, non_snake_case)]

use kompoz_runtime::*;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A node: its name, and the nodes the composition put below it.
pub struct TestNode {
    pub name: RefCell<String>,
    pub children: RefCell<Vec<Rc<TestNode>>>,
    /// How many times a composable configured it.
    pub compositions: Cell<u32>,
}

impl TestNode {
    pub fn new(name: &str) -> Rc<TestNode> {
        Rc::new(TestNode {
            name: RefCell::new(name.to_string()),
            children: RefCell::new(Vec::new()),
            compositions: Cell::new(0),
        })
    }

    /// Its own name and its descendants' in tree order.
    pub fn names(&self) -> Vec<String> {
        let mut out = Vec::new();
        for c in self.children.borrow().iter() {
            out.push(c.name.borrow().clone());
            out.extend(c.names());
        }
        out
    }

    pub fn child_names(&self) -> Vec<String> {
        self.children.borrow().iter().map(|c| c.name.borrow().clone()).collect()
    }

    pub fn find(self: &Rc<Self>, name: &str) -> Option<Rc<TestNode>> {
        for c in self.children.borrow().iter() {
            if *c.name.borrow() == name {
                return Some(c.clone());
            }
            if let Some(n) = c.find(name) {
                return Some(n);
            }
        }
        None
    }
}

impl Node for TestNode {
    fn set_children(&self, children: &[Rc<dyn Node>]) {
        *self.children.borrow_mut() =
            children.iter().map(|c| (c.clone() as Rc<dyn Any>).downcast::<TestNode>().expect("a test node")).collect();
    }
}

/// A leaf node named `text` (skipped when its text is the same and it read nothing that changed).
#[kompozable]
pub fn Text(text: impl Into<String>) {
    emit(
        || TestNode::new(""),
        |n| {
            *n.name.borrow_mut() = text;
            n.compositions.set(n.compositions.get() + 1);
        },
        || {},
    );
}

/// A node named `name` whose content runs in a scope of its own (as a Compose block's does).
#[kompozable]
pub fn Column(name: &'static str, content: impl Fn()) {
    emit(|| TestNode::new(name), |n| n.compositions.set(n.compositions.get() + 1), content);
}

/// Composes content into a root node and runs its frames.
pub struct Rule {
    pub root: Rc<TestNode>,
    composition: Option<Composition>,
    time: Cell<i64>,
}

impl Rule {
    pub fn new(content: impl Content + 'static) -> Rule {
        let root = TestNode::new("root");
        let composition = Composition::new(Some(root.clone()));
        composition.set_content(content);
        let rule = Rule { root, composition: Some(composition), time: Cell::new(0) };
        rule.wait_for_idle();
        rule
    }

    pub fn composition(&self) -> &Composition {
        self.composition.as_ref().unwrap()
    }

    /// Frames until nothing is left to recompose: what was written applied, then recomposed.
    pub fn wait_for_idle(&self) {
        for _ in 0..1000 {
            Snapshot::send_apply_notifications();
            let t = self.time.get();
            self.composition().invalidate_frame_scopes(t);
            let busy = self.composition().recompose(t);
            self.time.set(t + 16_000_000);
            if !busy && !self.composition().has_invalidations() && !global_snapshot_changed() {
                return;
            }
        }
        panic!("never idle");
    }

    pub fn names(&self) -> Vec<String> {
        self.root.names()
    }

    pub fn has(&self, name: &str) -> bool {
        self.names().iter().any(|n| n == name)
    }

    /// The composition disposed of.
    pub fn dispose(&mut self) {
        self.composition = None;
    }
}

/// Runs the tests given one after the other on this thread, made the UI thread; the arguments name the ones to
/// run (all if none).
pub fn run(tests: &[(&str, fn())]) {
    set_ui_thread();
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = Vec::new();
    let mut ran = 0;
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        ran += 1;
        if std::panic::catch_unwind(test).is_err() {
            failed.push(*name);
            println!("FAILED {name}");
        } else {
            println!("ok     {name}");
        }
    }
    println!("\n{ran} run, {} failed", failed.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
}

#[macro_export]
macro_rules! tests {
    ($($name:ident),* $(,)?) => {
        fn main() {
            common::run(&[$((stringify!($name), $name as fn())),*]);
        }
    };
}
