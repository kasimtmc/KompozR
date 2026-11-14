//! Compose's MyersDiff (a port of the Myers diff algorithm in androidx.compose.ui.node): the fewest insertions and
//! removals that turn one list into the other, as diagonals of matches; no moves.

/// What the diff asks and tells: whether two items match, and the steps that turn the old list into the new one.
pub(crate) trait DiffCallback {
    fn are_items_the_same(&mut self, old_index: usize, new_index: usize) -> bool;
    fn insert(&mut self, new_index: usize);
    fn remove(&mut self, at_index: usize, old_index: usize);
    fn same(&mut self, old_index: usize, new_index: usize);
}

/// Works out the diff of lists of `old_size` and `new_size` items and tells it to `cb`, in order.
pub(crate) fn execute_diff(old_size: usize, new_size: usize, cb: &mut dyn DiffCallback) {
    let diagonals = calculate_diff(old_size as i32, new_size as i32, cb);
    apply_diff(&diagonals, cb);
}

/// A diagonal: where it ends in the old and the new list, and how long it is (Compose's IntStack triples).
type Diagonal = (i32, i32, i32);

fn calculate_diff(old_size: i32, new_size: i32, cb: &mut dyn DiffCallback) -> Vec<Diagonal> {
    let max = (old_size + new_size + 1) / 2;
    let mut diagonals: Vec<Diagonal> = Vec::new();
    let mut stack: Vec<(i32, i32, i32, i32)> = vec![(0, old_size, 0, new_size)];
    let mut forward = Centered::new(max * 2 + 1);
    let mut backward = Centered::new(max * 2 + 1);
    let mut snake = [0i32; 5];
    while let Some((old_start, old_end, new_start, new_end)) = stack.pop() {
        let found = mid_point(old_start, old_end, new_start, new_end, cb, &mut forward, &mut backward, &mut snake);
        if found {
            let s = Snake(snake);
            if s.diagonal_size() > 0 {
                diagonals.push(s.diagonal());
            }
            // (pushed so that the first half is popped last, as Compose's stack does)
            stack.push((old_start, s.start_x(), new_start, s.start_y()));
            stack.push((s.end_x(), old_end, s.end_y(), new_end));
        }
    }
    diagonals.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    diagonals.push((old_size, new_size, 0));
    diagonals
}

fn apply_diff(diagonals: &[Diagonal], cb: &mut dyn DiffCallback) {
    let mut pos_x = 0;
    let mut pos_y = 0;
    for &(end_x, end_y, len) in diagonals {
        let start_x = end_x - len;
        let start_y = end_y - len;
        while pos_x < start_x {
            cb.remove(pos_y as usize, pos_x as usize);
            pos_x += 1;
        }
        while pos_y < start_y {
            cb.insert(pos_y as usize);
            pos_y += 1;
        }
        for _ in 0..len {
            cb.same(pos_x as usize, pos_y as usize);
            pos_x += 1;
            pos_y += 1;
        }
    }
}

/// An array indexed from `-mid` (Compose's CenteredArray).
struct Centered {
    data: Vec<i32>,
    mid: i32,
}

impl Centered {
    fn new(size: i32) -> Centered {
        Centered { data: vec![0; size.max(1) as usize], mid: size / 2 }
    }
    fn get(&self, i: i32) -> i32 {
        self.data[(i + self.mid) as usize]
    }
    fn set(&mut self, i: i32, v: i32) {
        self.data[(i + self.mid) as usize] = v;
    }
}

#[allow(clippy::too_many_arguments)]
fn mid_point(
    old_start: i32,
    old_end: i32,
    new_start: i32,
    new_end: i32,
    cb: &mut dyn DiffCallback,
    forward: &mut Centered,
    backward: &mut Centered,
    snake: &mut [i32; 5],
) -> bool {
    let old_size = old_end - old_start;
    let new_size = new_end - new_start;
    if old_size < 1 || new_size < 1 {
        return false;
    }
    let max = (old_size + new_size + 1) / 2;
    forward.set(1, old_start);
    backward.set(1, old_end);
    for d in 0..max {
        if forward_step(old_start, old_end, new_start, new_end, cb, forward, backward, d, snake) {
            return true;
        }
        if backward_step(old_start, old_end, new_start, new_end, cb, forward, backward, d, snake) {
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn forward_step(
    old_start: i32,
    old_end: i32,
    new_start: i32,
    new_end: i32,
    cb: &mut dyn DiffCallback,
    forward: &mut Centered,
    backward: &Centered,
    d: i32,
    snake: &mut [i32; 5],
) -> bool {
    let old_size = old_end - old_start;
    let new_size = new_end - new_start;
    let check_for_snake = (old_size - new_size).abs() & 1 == 1;
    let delta = old_size - new_size;
    let mut k = -d;
    while k <= d {
        let start_x;
        let mut x;
        if k == -d || (k != d && forward.get(k + 1) > forward.get(k - 1)) {
            start_x = forward.get(k + 1);
            x = start_x;
        } else {
            start_x = forward.get(k - 1);
            x = start_x + 1;
        }
        let mut y = new_start + (x - old_start) - k;
        let start_y = y - i32::from(d != 0 && x == start_x);
        while x < old_end && y < new_end && cb.are_items_the_same(x as usize, y as usize) {
            x += 1;
            y += 1;
        }
        forward.set(k, x);
        if check_for_snake {
            let backwards_k = delta - k;
            if backwards_k > -d && backwards_k < d && backward.get(backwards_k) <= x {
                *snake = [start_x, start_y, x, y, 0];
                return true;
            }
        }
        k += 2;
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn backward_step(
    old_start: i32,
    old_end: i32,
    new_start: i32,
    new_end: i32,
    cb: &mut dyn DiffCallback,
    forward: &Centered,
    backward: &mut Centered,
    d: i32,
    snake: &mut [i32; 5],
) -> bool {
    let old_size = old_end - old_start;
    let new_size = new_end - new_start;
    let check_for_snake = (old_size - new_size) & 1 == 0;
    let delta = old_size - new_size;
    let mut k = -d;
    while k <= d {
        let start_x;
        let mut x;
        if k == -d || (k != d && backward.get(k + 1) < backward.get(k - 1)) {
            start_x = backward.get(k + 1);
            x = start_x;
        } else {
            start_x = backward.get(k - 1);
            x = start_x - 1;
        }
        let mut y = new_end - (old_end - x - k);
        let start_y = y + i32::from(d != 0 && x == start_x);
        while x > old_start && y > new_start && cb.are_items_the_same((x - 1) as usize, (y - 1) as usize) {
            x -= 1;
            y -= 1;
        }
        backward.set(k, x);
        if check_for_snake {
            let forwards_k = delta - k;
            if forwards_k >= -d && forwards_k <= d && forward.get(forwards_k) >= x {
                *snake = [x, y, start_x, start_y, 1];
                return true;
            }
        }
        k += 2;
    }
    false
}

/// A match between the lists, maybe after an insertion or a removal (Compose's Snake).
struct Snake([i32; 5]);

impl Snake {
    fn start_x(&self) -> i32 {
        self.0[0]
    }
    fn start_y(&self) -> i32 {
        self.0[1]
    }
    fn end_x(&self) -> i32 {
        self.0[2]
    }
    fn end_y(&self) -> i32 {
        self.0[3]
    }
    fn reverse(&self) -> bool {
        self.0[4] != 0
    }
    fn diagonal_size(&self) -> i32 {
        (self.end_x() - self.start_x()).min(self.end_y() - self.start_y())
    }
    fn has_addition_or_removal(&self) -> bool {
        self.end_y() - self.start_y() != self.end_x() - self.start_x()
    }
    fn is_addition(&self) -> bool {
        self.end_y() - self.start_y() > self.end_x() - self.start_x()
    }
    /// Its diagonal, as (end x, end y, size).
    fn diagonal(&self) -> Diagonal {
        let mut x = self.start_x();
        let mut y = self.start_y();
        let size;
        if self.has_addition_or_removal() {
            size = self.diagonal_size();
            x += i32::from(!(self.reverse() || self.is_addition()));
            y += i32::from(!(self.reverse() || !self.is_addition()));
        } else {
            size = self.end_x() - self.start_x();
        }
        (x + size, y + size, size)
    }
}
