//! The source side of XDND 5 (a drag of the app's own out to other apps, WP5b; the target side is [`dnd`]): which
//! messages go to the drop target under the pointer as it moves (`XdndEnter` with the types offered — the first
//! three, or all in the source's `XdndTypeList` —, `XdndPosition` with the root position, the time and the action
//! asked for, never another before the target's `XdndStatus` answers the last, none within the rectangle it said it
//! needs no moves in; `XdndLeave` to the one it leaves), the release (`XdndDrop` where the target accepts, after
//! waiting for the status of the last move if one is due; else `XdndLeave`), and the end: the target's
//! `XdndFinished` (accepted, with the action it took: version 5), or untaken — dropped on nothing or a target that
//! refused, called off (Escape), or a target that never answered (a second for a status at the release, five after
//! the last request for the data for the finish). The server's part (finding the target and its proxy, sending,
//! the selection's requests, the grabs, the icon) is the platform's; this is what can be tested as it is.
//!
//! [`dnd`]: super::dnd

use super::ffi::{Atom, NONE, Time, Window};
use kompoz_ui::draganddrop::DragAndDropTransferAction;
use std::ffi::c_long;

/// The XDND version KompozR speaks.
pub const XDND_VERSION: u32 = 5;
/// The oldest a target may speak (older ones are taken as not aware: GTK's and Qt's floor).
pub const XDND_OLDEST: u32 = 3;
/// How long the release waits for the status of the last move (ns).
pub const STATUS_TIMEOUT_NS: i64 = 1_000_000_000;
/// How long a drop waits for the target's finish after its last request for the data (ns).
pub const FINISH_TIMEOUT_NS: i64 = 5_000_000_000;

/// The action atoms (`XdndActionCopy`, `XdndActionMove`, `XdndActionLink`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ActionAtoms {
    pub copy: Atom,
    pub move_: Atom,
    pub link: Atom,
}

impl ActionAtoms {
    /// The atom of `action`.
    pub fn atom(&self, action: DragAndDropTransferAction) -> Atom {
        if action == DragAndDropTransferAction::Copy {
            self.copy
        } else if action == DragAndDropTransferAction::Move {
            self.move_
        } else {
            self.link
        }
    }

    /// The action an atom is (none: none, or one KompozR doesn't have: `XdndActionAsk`, `XdndActionPrivate`).
    pub fn action(&self, atom: Atom) -> Option<DragAndDropTransferAction> {
        if atom == NONE {
            None
        } else if atom == self.copy {
            Some(DragAndDropTransferAction::Copy)
        } else if atom == self.move_ {
            Some(DragAndDropTransferAction::Move)
        } else if atom == self.link {
            Some(DragAndDropTransferAction::Link)
        } else {
            None
        }
    }
}

/// A drop target as found under the pointer: its window (`XdndAware`: what the messages are about), where they go
/// (its `XdndProxy`, else it), and the version spoken (the lower of its and ours).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DropTarget {
    pub window: Window,
    pub proxy: Window,
    pub version: u32,
}

/// What a message is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MessageKind {
    Enter,
    Position,
    Leave,
    Drop,
}

/// A message to send: to `to` (the target's proxy, or it), its window field `window` (the target), its data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Message {
    pub to: Window,
    pub window: Window,
    pub kind: MessageKind,
    pub data: [c_long; 5],
}

/// What the cursor shows: whether the target under the pointer takes the drop and how.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragCursor {
    NoDrop,
    Copy,
    Move,
    Link,
}

/// A move to tell: where on the screen, when, the action asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Position {
    x: i32,
    y: i32,
    time: Time,
    action: Atom,
}

/// The target the drag is over, and what it said.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Over {
    target: DropTarget,
    /// A position was sent and its status hasn't come.
    waiting: bool,
    /// A move waiting for that status (the latest).
    pending: Option<Position>,
    /// The last position sent.
    sent: Option<Position>,
    accepted: bool,
    action: Atom,
    /// Where it needs no moves told (root coordinates), unless it wants them all.
    quiet: Option<(i32, i32, i32, i32)>,
}

/// Where the drag is in its life.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Dragging,
    /// Let go while a status was due: waiting for it (until `deadline`) to drop or leave.
    Releasing {
        time: Time,
        deadline: i64,
    },
    /// Dropped: waiting for the finish (until `deadline`).
    Dropped {
        deadline: i64,
    },
    Ended,
}

/// The drag's end: taken with that action, or not.
pub type Ended = Option<Option<DragAndDropTransferAction>>;

/// A drag's XDND source (C++ Kompoz had none: X11's drags were received only).
#[derive(Clone, Debug)]
pub struct XdndSource {
    source: Window,
    types: Vec<Atom>,
    actions: ActionAtoms,
    over: Option<Over>,
    phase: Phase,
}

impl XdndSource {
    /// A drag from `source` (the app's window, owning `XdndSelection`) offering `types` (the best first).
    pub fn new(source: Window, types: Vec<Atom>, actions: ActionAtoms) -> XdndSource {
        XdndSource { source, types, actions, over: None, phase: Phase::Dragging }
    }

    /// Whether the source's `XdndTypeList` is to be set (more types than an `XdndEnter` carries).
    pub fn needs_type_list(&self) -> bool {
        self.types.len() > 3
    }

    /// Whether the pointer still drags it (not let go, called off or over).
    pub fn is_dragging(&self) -> bool {
        self.phase == Phase::Dragging
    }

    /// What the cursor shows.
    pub fn cursor(&self) -> DragCursor {
        let Some(o) = self.over.filter(|o| o.accepted) else { return DragCursor::NoDrop };
        match self.actions.action(o.action) {
            Some(DragAndDropTransferAction::Move) => DragCursor::Move,
            Some(DragAndDropTransferAction::Link) => DragCursor::Link,
            _ => DragCursor::Copy,
        }
    }

    fn message(&self, target: DropTarget, kind: MessageKind, data: [c_long; 5]) -> Message {
        Message { to: target.proxy, window: target.window, kind, data }
    }

    fn enter(&self, target: DropTarget) -> Message {
        let mut data =
            [self.source as c_long, ((target.version as c_long) << 24) | c_long::from(self.needs_type_list()), 0, 0, 0];
        for (i, &t) in self.types.iter().take(3).enumerate() {
            data[2 + i] = t as c_long;
        }
        self.message(target, MessageKind::Enter, data)
    }

    fn leave(&self, target: DropTarget) -> Message {
        self.message(target, MessageKind::Leave, [self.source as c_long, 0, 0, 0, 0])
    }

    fn position(&self, target: DropTarget, p: Position) -> Message {
        let at = ((p.x.clamp(0, 0xffff) as c_long) << 16) | p.y.clamp(0, 0xffff) as c_long;
        let action = if target.version >= 2 { p.action as c_long } else { 0 };
        self.message(target, MessageKind::Position, [self.source as c_long, 0, at, p.time as c_long, action])
    }

    /// Sends `p` to the target it is over, or keeps it for when its status comes (one at a time); none within the
    /// rectangle it needs no moves in (the same action).
    fn tell(&mut self, p: Position) -> Vec<Message> {
        let Some(mut o) = self.over else { return Vec::new() };
        if o.waiting {
            o.pending = Some(p);
            self.over = Some(o);
            return Vec::new();
        }
        let quiet = o.quiet.is_some_and(|(x, y, w, h)| p.x >= x && p.x < x + w && p.y >= y && p.y < y + h);
        if quiet && o.sent.is_some_and(|s| s.action == p.action) {
            return Vec::new();
        }
        o.waiting = true;
        o.sent = Some(p);
        self.over = Some(o);
        vec![self.position(o.target, p)]
    }

    /// The pointer moved to (`x`, `y`) on the screen at `time`, over `target` (none: no XDND window there), the
    /// user asking for `action`: the target left told it left, a new one told the drag entered, the move told.
    pub fn moved(&mut self, target: Option<DropTarget>, x: i32, y: i32, time: Time, action: Atom) -> Vec<Message> {
        if self.phase != Phase::Dragging {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.over.map(|o| o.target.window) != target.map(|t| t.window) {
            if let Some(old) = self.over.take() {
                out.push(self.leave(old.target));
            }
            if let Some(t) = target {
                out.push(self.enter(t));
                self.over = Some(Over {
                    target: t,
                    waiting: false,
                    pending: None,
                    sent: None,
                    accepted: false,
                    action: NONE,
                    quiet: None,
                });
            }
        }
        out.extend(self.tell(Position { x, y, time, action }));
        out
    }

    /// The user asked for another action (a modifier pressed or let go) where the pointer is.
    pub fn action_changed(&mut self, action: Atom) -> Vec<Message> {
        let Some(o) = self.over else { return Vec::new() };
        let Some(last) = o.pending.or(o.sent) else { return Vec::new() };
        if self.phase != Phase::Dragging || last.action == action {
            return Vec::new();
        }
        self.tell(Position { action, ..last })
    }

    /// A target's `XdndStatus` (`data[0]`: its window): whether it takes the drop, how, where it needs no moves;
    /// the move kept meanwhile told, or the release waiting for it done.
    pub fn status(&mut self, data: &[c_long; 5], now: i64) -> (Vec<Message>, Ended) {
        let Some(mut o) = self.over.filter(|o| o.target.window == data[0] as Window) else {
            return (Vec::new(), None);
        };
        o.waiting = false;
        o.accepted = data[1] & 1 != 0;
        o.action = if o.target.version >= 2 { data[4] as Atom } else { self.actions.copy };
        let (w, h) = (((data[3] >> 16) & 0xffff) as i32, (data[3] & 0xffff) as i32);
        o.quiet = (data[1] & 2 == 0 && w > 0 && h > 0).then_some((
            ((data[2] >> 16) & 0xffff) as i32,
            (data[2] & 0xffff) as i32,
            w,
            h,
        ));
        if !o.accepted {
            o.action = NONE;
        }
        self.over = Some(o);
        match self.phase {
            Phase::Releasing { time, .. } => self.drop_or_leave(time, now),
            Phase::Dragging => match o.pending.take() {
                Some(p) => {
                    self.over = Some(Over { pending: None, ..o });
                    (self.tell(p), None)
                }
                None => (Vec::new(), None),
            },
            _ => (Vec::new(), None),
        }
    }

    /// Let go at `time`: dropped on the target if it takes it (once the status of the last move is in), else the
    /// drag left it, untaken.
    pub fn released(&mut self, time: Time, now: i64) -> (Vec<Message>, Ended) {
        if self.phase != Phase::Dragging {
            return (Vec::new(), None);
        }
        match self.over {
            None => {
                self.phase = Phase::Ended;
                (Vec::new(), Some(None))
            }
            Some(o) if o.waiting => {
                self.phase = Phase::Releasing { time, deadline: now + STATUS_TIMEOUT_NS };
                (Vec::new(), None)
            }
            Some(_) => self.drop_or_leave(time, now),
        }
    }

    fn drop_or_leave(&mut self, time: Time, now: i64) -> (Vec<Message>, Ended) {
        let Some(o) = self.over else {
            self.phase = Phase::Ended;
            return (Vec::new(), Some(None));
        };
        if o.accepted && o.action != NONE {
            self.phase = Phase::Dropped { deadline: now + FINISH_TIMEOUT_NS };
            let drop = self.message(o.target, MessageKind::Drop, [self.source as c_long, 0, time as c_long, 0, 0]);
            return (vec![drop], None);
        }
        self.phase = Phase::Ended;
        (vec![self.leave(o.target)], Some(None))
    }

    /// The target asked for the data (a request for `XdndSelection`): it is still at it, the finish waited for longer.
    pub fn activity(&mut self, now: i64) {
        if let Phase::Dropped { .. } = self.phase {
            self.phase = Phase::Dropped { deadline: now + FINISH_TIMEOUT_NS };
        }
    }

    /// A target's `XdndFinished` (`data[0]`: its window): the drag's end, taken with the action it says (version 5;
    /// before it, the status's) or not.
    pub fn finished(&mut self, data: &[c_long; 5]) -> Ended {
        let Phase::Dropped { .. } = self.phase else { return None };
        let o = self.over.filter(|o| o.target.window == data[0] as Window)?;
        self.phase = Phase::Ended;
        if o.target.version >= 5 {
            let accepted = data[1] & 1 != 0;
            Some(if accepted { self.actions.action(data[2] as Atom) } else { None })
        } else {
            Some(self.actions.action(o.action))
        }
    }

    /// Called off (Escape): the target told it left, untaken. (Once dropped, it is the target's.)
    pub fn cancel(&mut self) -> (Vec<Message>, Ended) {
        if !matches!(self.phase, Phase::Dragging | Phase::Releasing { .. }) {
            return (Vec::new(), None);
        }
        self.phase = Phase::Ended;
        let leave = self.over.map(|o| self.leave(o.target));
        (leave.into_iter().collect(), Some(None))
    }

    /// When it gives up waiting (a status at the release, the finish), if it waits.
    pub fn deadline(&self) -> Option<i64> {
        match self.phase {
            Phase::Releasing { deadline, .. } | Phase::Dropped { deadline } => Some(deadline),
            _ => None,
        }
    }

    /// The time is `now`: a target that didn't answer in time left (untaken).
    pub fn timed_out(&mut self, now: i64) -> (Vec<Message>, Ended) {
        match self.phase {
            Phase::Releasing { deadline, .. } if now >= deadline => {
                self.phase = Phase::Ended;
                let leave = self.over.map(|o| self.leave(o.target));
                (leave.into_iter().collect(), Some(None))
            }
            Phase::Dropped { deadline } if now >= deadline => {
                self.phase = Phase::Ended;
                (Vec::new(), Some(None))
            }
            _ => (Vec::new(), None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DragAndDropTransferAction as A;
    use MessageKind::*;

    const SOURCE: Window = 0x3a00007;
    const ATOMS: ActionAtoms = ActionAtoms { copy: 300, move_: 301, link: 302 };
    const T1: DropTarget = DropTarget { window: 0x4c00011, proxy: 0x4c00011, version: 5 };
    const T2: DropTarget = DropTarget { window: 0x5000003, proxy: 0x5000099, version: 4 };

    fn source(types: &[Atom]) -> XdndSource {
        XdndSource::new(SOURCE, types.to_vec(), ATOMS)
    }

    fn kinds(messages: &[Message]) -> Vec<(MessageKind, Window, Window)> {
        messages.iter().map(|m| (m.kind, m.to, m.window)).collect()
    }

    /// A target's status: accepting with `action` (none: refusing), wanting every move or not (a rectangle).
    fn status(target: DropTarget, action: Atom, rect: Option<(i32, i32, i32, i32)>) -> [c_long; 5] {
        let accept = c_long::from(action != NONE);
        let (flags, at, size) = match rect {
            Some((x, y, w, h)) => (accept, ((x as c_long) << 16) | y as c_long, ((w as c_long) << 16) | h as c_long),
            None => (accept | 2, 0, 0),
        };
        [target.window as c_long, flags, at, size, action as c_long]
    }

    #[test]
    fn enter_carries_three_types_or_says_there_is_a_list() {
        let mut s = source(&[100, 101, 102, 103]);
        assert!(s.needs_type_list());
        let m = s.moved(Some(T1), 10, 20, 1000, ATOMS.move_);
        assert_eq!(kinds(&m), [(Enter, T1.proxy, T1.window), (Position, T1.proxy, T1.window)]);
        assert_eq!(m[0].data, [SOURCE as c_long, (5 << 24) | 1, 100, 101, 102]);
        assert_eq!(m[1].data, [SOURCE as c_long, 0, (10 << 16) | 20, 1000, ATOMS.move_ as c_long]);
        let mut s = source(&[100]);
        assert!(!s.needs_type_list());
        let m = s.moved(Some(T2), 1, 2, 5, ATOMS.copy);
        assert_eq!(m[0].data, [SOURCE as c_long, 4 << 24, 100, 0, 0]); // (its version: the lower of the two)
        assert_eq!((m[0].to, m[0].window), (T2.proxy, T2.window)); // (to its proxy, about it)
    }

    #[test]
    fn one_position_at_a_time_the_latest_kept() {
        let mut s = source(&[100]);
        assert_eq!(s.moved(Some(T1), 10, 10, 1, ATOMS.copy).len(), 2);
        // No status yet: the moves kept, the latest only.
        assert!(s.moved(Some(T1), 11, 10, 2, ATOMS.copy).is_empty());
        assert!(s.moved(Some(T1), 12, 10, 3, ATOMS.copy).is_empty());
        assert_eq!(s.cursor(), DragCursor::NoDrop);
        let (m, end) = s.status(&status(T1, ATOMS.copy, None), 0);
        assert_eq!(end, None);
        assert_eq!(kinds(&m), [(Position, T1.proxy, T1.window)]);
        assert_eq!(m[0].data[2..4], [(12 << 16) | 10, 3]);
        assert_eq!(s.cursor(), DragCursor::Copy);
        // Another's status, or none due: nothing.
        assert!(s.status(&status(T2, ATOMS.copy, None), 0).0.is_empty());
        assert!(s.status(&status(T1, ATOMS.copy, None), 0).0.is_empty());
        assert_eq!(s.moved(Some(T1), 13, 10, 4, ATOMS.copy).len(), 1);
    }

    #[test]
    fn no_moves_within_the_quiet_rectangle_unless_the_action_changes() {
        let mut s = source(&[100]);
        s.moved(Some(T1), 50, 50, 1, ATOMS.move_);
        s.status(&status(T1, ATOMS.move_, Some((40, 40, 20, 20))), 0);
        assert_eq!(s.cursor(), DragCursor::Move);
        assert!(s.moved(Some(T1), 55, 45, 2, ATOMS.move_).is_empty());
        // Ctrl: Copy asked for where it is.
        let m = s.action_changed(ATOMS.copy);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].data[2..5], [(50 << 16) | 50, 1, ATOMS.copy as c_long]);
        s.status(&status(T1, ATOMS.copy, Some((40, 40, 20, 20))), 0);
        assert!(s.action_changed(ATOMS.copy).is_empty());
        assert!(s.moved(Some(T1), 59, 59, 3, ATOMS.copy).is_empty());
        assert_eq!(s.moved(Some(T1), 60, 59, 4, ATOMS.copy).len(), 1); // (out of it)
    }

    #[test]
    fn going_from_one_target_to_another_and_to_none() {
        let mut s = source(&[100]);
        s.moved(Some(T1), 1, 1, 1, ATOMS.copy);
        s.status(&status(T1, ATOMS.copy, None), 0);
        let m = s.moved(Some(T2), 300, 1, 2, ATOMS.copy);
        assert_eq!(
            kinds(&m),
            [(Leave, T1.proxy, T1.window), (Enter, T2.proxy, T2.window), (Position, T2.proxy, T2.window)]
        );
        assert_eq!(m[0].data, [SOURCE as c_long, 0, 0, 0, 0]);
        assert_eq!(s.cursor(), DragCursor::NoDrop); // (the new one hasn't said)
        // Its status for the last move comes after it left: its window isn't the one over.
        let m = s.moved(None, 700, 1, 3, ATOMS.copy);
        assert_eq!(kinds(&m), [(Leave, T2.proxy, T2.window)]);
        assert!(s.status(&status(T2, ATOMS.copy, None), 0).0.is_empty());
        assert_eq!(s.cursor(), DragCursor::NoDrop);
    }

    #[test]
    fn a_drop_taken_ends_with_the_targets_action() {
        let mut s = source(&[100]);
        s.moved(Some(T1), 1, 1, 1, ATOMS.move_);
        s.status(&status(T1, ATOMS.move_, None), 0);
        let (m, end) = s.released(77, 10);
        assert_eq!(end, None);
        assert_eq!(kinds(&m), [(Drop, T1.proxy, T1.window)]);
        assert_eq!(m[0].data, [SOURCE as c_long, 0, 77, 0, 0]);
        assert_eq!(s.deadline(), Some(10 + FINISH_TIMEOUT_NS));
        s.activity(1_000);
        assert_eq!(s.deadline(), Some(1_000 + FINISH_TIMEOUT_NS));
        assert_eq!(s.finished(&[T2.window as c_long, 1, ATOMS.copy as c_long, 0, 0]), None); // (another's)
        assert_eq!(s.finished(&[T1.window as c_long, 1, ATOMS.copy as c_long, 0, 0]), Some(Some(A::Copy)));
        assert!(!s.is_dragging() && s.deadline().is_none());
        assert_eq!(s.cancel(), (Vec::new(), None));
        // Version 5 refusing at the finish; an older one's finish: the status's action.
        let mut s = source(&[100]);
        s.moved(Some(T1), 1, 1, 1, ATOMS.link);
        s.status(&status(T1, ATOMS.link, None), 0);
        s.released(2, 0);
        assert_eq!(s.finished(&[T1.window as c_long, 0, 0, 0, 0]), Some(None));
        let old = DropTarget { version: 4, ..T1 };
        let mut s = source(&[100]);
        s.moved(Some(old), 1, 1, 1, ATOMS.link);
        s.status(&status(old, ATOMS.link, None), 0);
        s.released(2, 0);
        assert_eq!(s.finished(&[T1.window as c_long, 0, 0, 0, 0]), Some(Some(A::Link)));
    }

    #[test]
    fn released_before_the_status_waits_for_it() {
        let mut s = source(&[100]);
        s.moved(Some(T1), 1, 1, 1, ATOMS.copy);
        let (m, end) = s.released(5, 100);
        assert!(m.is_empty() && end.is_none());
        assert_eq!(s.deadline(), Some(100 + STATUS_TIMEOUT_NS));
        assert!(s.moved(Some(T2), 2, 2, 6, ATOMS.copy).is_empty()); // (no more moves once let go)
        let (m, end) = s.status(&status(T1, ATOMS.copy, None), 200);
        assert_eq!((kinds(&m), end), (vec![(Drop, T1.proxy, T1.window)], None));
        assert_eq!(m[0].data[2], 5);
        // Refused: it left, untaken.
        let mut s = source(&[100]);
        s.moved(Some(T1), 1, 1, 1, ATOMS.copy);
        s.released(5, 100);
        let (m, end) = s.status(&status(T1, NONE, None), 200);
        assert_eq!((kinds(&m), end), (vec![(Leave, T1.proxy, T1.window)], Some(None)));
        // No status at all: it left after a second.
        let mut s = source(&[100]);
        s.moved(Some(T1), 1, 1, 1, ATOMS.copy);
        s.released(5, 100);
        assert_eq!(s.timed_out(100 + STATUS_TIMEOUT_NS - 1), (Vec::new(), None));
        let (m, end) = s.timed_out(100 + STATUS_TIMEOUT_NS);
        assert_eq!((kinds(&m), end), (vec![(Leave, T1.proxy, T1.window)], Some(None)));
    }

    #[test]
    fn dropped_on_nothing_refused_called_off_or_never_finished() {
        // Nothing under the pointer.
        let mut s = source(&[100]);
        s.moved(None, 1, 1, 1, ATOMS.copy);
        assert_eq!(s.released(2, 0), (Vec::new(), Some(None)));
        // A target that refuses: it left.
        let mut s = source(&[100]);
        s.moved(Some(T1), 1, 1, 1, ATOMS.copy);
        s.status(&status(T1, NONE, Some((0, 0, 100, 100))), 0);
        let (m, end) = s.released(2, 0);
        assert_eq!((kinds(&m), end), (vec![(Leave, T1.proxy, T1.window)], Some(None)));
        // Escape over a target.
        let mut s = source(&[100]);
        s.moved(Some(T1), 1, 1, 1, ATOMS.copy);
        let (m, end) = s.cancel();
        assert_eq!((kinds(&m), end), (vec![(Leave, T1.proxy, T1.window)], Some(None)));
        assert_eq!(s.released(2, 0), (Vec::new(), None));
        // Dropped, the finish never coming.
        let mut s = source(&[100]);
        s.moved(Some(T1), 1, 1, 1, ATOMS.copy);
        s.status(&status(T1, ATOMS.copy, None), 0);
        s.released(2, 0);
        assert_eq!(s.cancel(), (Vec::new(), None)); // (dropped: the target's now)
        assert_eq!(s.timed_out(FINISH_TIMEOUT_NS), (Vec::new(), Some(None)));
        assert_eq!(s.deadline(), None);
    }

    #[test]
    fn actions_and_atoms() {
        assert_eq!(ATOMS.atom(A::Move), 301);
        assert_eq!(ATOMS.action(302), Some(A::Link));
        assert_eq!(ATOMS.action(999), None); // (XdndActionAsk, XdndActionPrivate)
        assert_eq!(ATOMS.action(NONE), None);
    }
}
