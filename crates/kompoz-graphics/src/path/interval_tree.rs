//! Intervals of floats with data, found by what they overlap (Compose's IntervalTree, ui-graphics's: a red-black
//! tree keyed by the intervals' starts, each node knowing the least start and greatest end below it). What
//! PathEasing finds the segment of a path covering an x with.

/// An interval, `start..=end`, with its data (Compose's Interval).
#[derive(Clone, Debug, PartialEq)]
pub struct Interval<T> {
    pub start: f32,
    pub end: f32,
    pub data: Option<T>,
}

impl<T> Interval<T> {
    pub fn new(start: f32, end: f32, data: Option<T>) -> Interval<T> {
        Interval { start, end, data }
    }
    pub fn overlaps(&self, other: &Interval<T>) -> bool {
        self.start <= other.end && self.end >= other.start
    }
    pub fn overlaps_range(&self, start: f32, end: f32) -> bool {
        self.start <= end && self.end >= start
    }
    pub fn contains(&self, value: f32) -> bool {
        value >= self.start && value <= self.end
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TreeColor {
    Red,
    Black,
}

/// Kotlin's `Float.MIN_VALUE`: the least positive float.
const KotlinFloatMinValue: f32 = f32::from_bits(1);

/// The tree's terminator (Compose's `terminator`, a node of its own): index 0.
const T: usize = 0;

struct Node<D> {
    interval: Interval<D>,
    color: TreeColor,
    min: f32,
    max: f32,
    left: usize,
    right: usize,
    parent: usize,
}

/// Intervals found by what they overlap (Compose's IntervalTree). Nodes live in an arena; index 0 is the
/// terminator, an empty interval (`Float.MAX_VALUE..Float.MIN_VALUE`) as Compose's.
pub struct IntervalTree<D> {
    nodes: Vec<Node<D>>,
    root: usize,
}

impl<D> Default for IntervalTree<D> {
    fn default() -> Self {
        IntervalTree::new()
    }
}

impl<D> IntervalTree<D> {
    pub fn new() -> IntervalTree<D> {
        let terminator = Node {
            interval: Interval::new(f32::MAX, KotlinFloatMinValue, None),
            color: TreeColor::Black,
            min: f32::MAX,
            max: KotlinFloatMinValue,
            left: T,
            right: T,
            parent: T,
        };
        IntervalTree { nodes: vec![terminator], root: T }
    }

    /// Removes every interval.
    pub fn clear(&mut self) {
        self.nodes.truncate(1);
        let t = &mut self.nodes[T];
        (t.left, t.right, t.parent) = (T, T, T);
        self.root = T;
    }

    /// The first interval (in the tree's search order) overlapping `start..=end`, if any (Compose's
    /// findFirstOverlap; its EmptyInterval is `None` here).
    pub fn find_first_overlap(&self, start: f32, end: f32) -> Option<&Interval<D>> {
        let mut found = None;
        self.for_each(start, end, |i| {
            found = Some(i);
            false
        });
        found
    }

    /// Every interval overlapping `start..=end`, in the tree's search order (Compose's findOverlaps).
    pub fn find_overlaps(&self, start: f32, end: f32) -> Vec<&Interval<D>> {
        let mut out = Vec::new();
        self.for_each(start, end, |i| {
            out.push(i);
            true
        });
        out
    }

    /// Visits the overlapping intervals as Compose's search does (a stack: a node, then its right side before its
    /// left); `visit` says whether to go on.
    fn for_each<'a>(&'a self, start: f32, end: f32, mut visit: impl FnMut(&'a Interval<D>) -> bool) {
        if self.root == T {
            return;
        }
        let mut stack = vec![self.root];
        while let Some(n) = stack.pop() {
            let node = &self.nodes[n];
            if node.interval.overlaps_range(start, end) && !visit(&node.interval) {
                return;
            }
            if node.left != T && self.nodes[node.left].max >= start {
                stack.push(node.left);
            }
            if node.right != T && self.nodes[node.right].min <= end {
                stack.push(node.right);
            }
        }
    }

    /// Whether an interval holds `value` (Compose's `value in tree`).
    pub fn contains(&self, value: f32) -> bool {
        self.find_first_overlap(value, value).is_some()
    }

    /// Whether an interval overlaps `start..=end`.
    pub fn contains_range(&self, start: f32, end: f32) -> bool {
        self.find_first_overlap(start, end).is_some()
    }

    /// The intervals by their starts (Compose's iterator: in order, lowest first).
    pub fn iter(&self) -> impl Iterator<Item = &Interval<D>> + '_ {
        let mut next = self.lowest_node(self.root);
        std::iter::from_fn(move || {
            if next == T {
                return None;
            }
            let node = next;
            next = self.next_node(node);
            Some(&self.nodes[node].interval)
        })
    }

    fn lowest_node(&self, mut node: usize) -> usize {
        if node == T {
            return T;
        }
        while self.nodes[node].left != T {
            node = self.nodes[node].left;
        }
        node
    }

    fn next_node(&self, node: usize) -> usize {
        if self.nodes[node].right != T {
            return self.lowest_node(self.nodes[node].right);
        }
        let (mut a, mut b) = (node, self.nodes[node].parent);
        while b != T && a == self.nodes[b].right {
            a = b;
            b = self.nodes[b].parent;
        }
        b
    }

    /// Adds `start..=end` with its data (Compose's addInterval).
    pub fn add_interval(&mut self, start: f32, end: f32, data: Option<D>) {
        let node = self.nodes.len();
        self.nodes.push(Node {
            interval: Interval::new(start, end, data),
            color: TreeColor::Red,
            min: start,
            max: end,
            left: T,
            right: T,
            parent: T,
        });
        let mut current = self.root;
        let mut parent = T;
        while current != T {
            parent = current;
            current = if start <= self.nodes[current].interval.start {
                self.nodes[current].left
            } else {
                self.nodes[current].right
            };
        }
        self.nodes[node].parent = parent;
        if parent == T {
            self.root = node;
        } else if start <= self.nodes[parent].interval.start {
            self.nodes[parent].left = node;
        } else {
            self.nodes[parent].right = node;
        }
        self.update_node_data(node);
        self.rebalance(node);
    }

    fn parent(&self, n: usize) -> usize {
        self.nodes[n].parent
    }

    fn rebalance(&mut self, target: usize) {
        let mut node = target;
        while node != self.root && self.nodes[self.parent(node)].color == TreeColor::Red {
            let ancestor = self.parent(self.parent(node));
            if self.parent(node) == self.nodes[ancestor].left {
                let right = self.nodes[ancestor].right;
                if self.nodes[right].color == TreeColor::Red {
                    self.nodes[right].color = TreeColor::Black;
                    let p = self.parent(node);
                    self.nodes[p].color = TreeColor::Black;
                    self.nodes[ancestor].color = TreeColor::Red;
                    node = ancestor;
                } else {
                    if node == self.nodes[self.parent(node)].right {
                        node = self.parent(node);
                        self.rotate_left(node);
                    }
                    let p = self.parent(node);
                    self.nodes[p].color = TreeColor::Black;
                    self.nodes[ancestor].color = TreeColor::Red;
                    self.rotate_right(ancestor);
                }
            } else {
                let left = self.nodes[ancestor].left;
                if self.nodes[left].color == TreeColor::Red {
                    self.nodes[left].color = TreeColor::Black;
                    let p = self.parent(node);
                    self.nodes[p].color = TreeColor::Black;
                    self.nodes[ancestor].color = TreeColor::Red;
                    node = ancestor;
                } else {
                    if node == self.nodes[self.parent(node)].left {
                        node = self.parent(node);
                        self.rotate_right(node);
                    }
                    let p = self.parent(node);
                    self.nodes[p].color = TreeColor::Black;
                    self.nodes[ancestor].color = TreeColor::Red;
                    self.rotate_left(ancestor);
                }
            }
        }
        let root = self.root;
        self.nodes[root].color = TreeColor::Black;
    }

    fn rotate_left(&mut self, node: usize) {
        let right = self.nodes[node].right;
        let right_left = self.nodes[right].left;
        self.nodes[node].right = right_left;
        if right_left != T {
            self.nodes[right_left].parent = node;
        }
        let parent = self.nodes[node].parent;
        self.nodes[right].parent = parent;
        if parent == T {
            self.root = right;
        } else if self.nodes[parent].left == node {
            self.nodes[parent].left = right;
        } else {
            self.nodes[parent].right = right;
        }
        self.nodes[right].left = node;
        self.nodes[node].parent = right;
        self.update_node_data(node);
    }

    fn rotate_right(&mut self, node: usize) {
        let left = self.nodes[node].left;
        let left_right = self.nodes[left].right;
        self.nodes[node].left = left_right;
        if left_right != T {
            self.nodes[left_right].parent = node;
        }
        let parent = self.nodes[node].parent;
        self.nodes[left].parent = parent;
        if parent == T {
            self.root = left;
        } else if self.nodes[parent].right == node {
            self.nodes[parent].right = left;
        } else {
            self.nodes[parent].left = left;
        }
        self.nodes[left].right = node;
        self.nodes[node].parent = left;
        self.update_node_data(node);
    }

    /// The least start and greatest end below each node from `node` up to the root (Compose's updateNodeData).
    fn update_node_data(&mut self, node: usize) {
        let mut current = node;
        while current != T {
            let (l, r) = (self.nodes[current].left, self.nodes[current].right);
            let min = self.nodes[current].interval.start.min(self.nodes[l].min.min(self.nodes[r].min));
            let max = self.nodes[current].interval.end.max(self.nodes[l].max.max(self.nodes[r].max));
            self.nodes[current].min = min;
            self.nodes[current].max = max;
            current = self.nodes[current].parent;
        }
    }
}
