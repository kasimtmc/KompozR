//! What Windows' input methods see of the focused field and what they do to it, in plain Rust (tested on every
//! host): the field as the core will have it once it handled what was sent ([`Field`]), Windows' offsets (UTF-16
//! code units of the whole text, where KompozR's are UTF-8 bytes), and the translations both of the platform's
//! input method paths share:
//!
//! - IMM32's composition (its string, its attributes — the clause being converted —, its caret) as the core's
//!   composing text ([`imm_preedit`]); the buffers its requests are answered with: the text around the selection
//!   to reconvert, or around the composition for an input method's prediction ([`reconversion`], [`Reconversion`]),
//!   where a character is ([`char_position`]), where the input method's windows go ([`placement`]);
//! - what a TSF text service did to the document (the text store's copy: [`Document`], its offsets ACPs, UTF-16)
//!   as the core's events ([`transition`]: the composing text where only it changed, else the part that changed
//!   replaced around the caret, or selected and replaced elsewhere, the selection set where the edit left it), the
//!   display attributes of a composition's runs as its highlighted part ([`target_clause`]), the app's own change
//!   as what the text store tells TSF ([`text_change`]);
//! - a field's options as Windows' input scopes ([`input_scopes`]: the touch keyboard's layout, what a text service
//!   offers).
//!
//! The core gets an input method's work as Linux's input methods give it (the platform-neutral events): committed
//! text ([`Event::Text`]), the composing text with its caret or highlighted part ([`Event::Preedit`]), text deleted
//! around the selection ([`Event::DeleteSurroundingText`]), a selection set ([`Event::SetSelection`]). [`apply`] is
//! what the core does with each, so that what an input method asks next is answered from the field as it will be.
//!
//! What the host tells of a field's place is its caret's box (Compose's focusedRectInRoot, the one rectangle Linux's
//! text-input and macOS's input client are given too): each character's rectangle a request asks for is that box.

use crate::app::Event;
use kompoz_graphics::Rect;
use kompoz_ui::platform::{ImeAction, ImeOptions, KeyboardType, TextEditState};
use kompoz_ui::text_input::EditCommand;

// ---- UTF-16 offsets ---------------------------------------------------------------------------------------------

/// The text's length in UTF-16 code units.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Byte `byte` of `text` (the character it is in: its start) in UTF-16 code units.
pub fn utf16_of_byte(text: &str, byte: usize) -> usize {
    utf16_len(&text[..floor_char(text, byte)])
}

/// UTF-16 code unit `unit` of `text` as a byte offset (between a surrogate pair's halves: its character's start;
/// past the end: the end).
pub fn byte_of_utf16(text: &str, unit: usize) -> usize {
    let mut units = 0;
    for (i, c) in text.char_indices() {
        let next = units + c.len_utf16();
        if next > unit {
            return i;
        }
        units = next;
    }
    text.len()
}

/// The nearest character boundary of `text` at or before `at` (clamped to it).
fn floor_char(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// The nearest character boundary of `text` at or after `at` (clamped to it).
fn ceil_char(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

/// The selection in order (bytes, on whole characters of the text).
pub fn selection(state: &TextEditState) -> (usize, usize) {
    let t = &state.text;
    let (a, b) = (floor_char(t, state.selection_start), floor_char(t, state.selection_end));
    (a.min(b), a.max(b))
}

/// The composition in order, none if empty (bytes, whole characters).
pub fn composition(state: &TextEditState) -> Option<(usize, usize)> {
    let t = &state.text;
    let (a, b) = state.composition?;
    let (a, b) = (floor_char(t, a.min(b)), ceil_char(t, a.max(b)));
    (a < b).then_some((a, b))
}

/// The state with its selection and composition in order, on whole characters (what the core's buffer makes of it).
pub fn normalized(state: &TextEditState) -> TextEditState {
    let (start, end) = selection(state);
    TextEditState {
        text: state.text.clone(),
        selection_start: start,
        selection_end: end,
        composition: composition(state),
    }
}

// ---- The field and the core's events ----------------------------------------------------------------------------

/// What the core does with an input method's event to the focused field (the app core's: Compose's commands),
/// applied to `state`.
pub fn apply(state: &mut TextEditState, e: &Event) {
    match e {
        Event::Text(text) => state.apply(&EditCommand::CommitText { text: text.clone(), new_cursor_position: 1 }),
        Event::Preedit { text, cursor } => match *cursor {
            Some((start, end)) if !text.is_empty() && (start, end) != (text.len(), text.len()) => {
                // (As the host's set_composing_text_with_selection: the composition begins where what it replaces
                // begins, its selection inside it after it.)
                let at = composition(state).map_or(selection(state).0, |c| c.0).min(state.text.len());
                let (start, end) = (floor_char(text, start.min(end)), floor_char(text, start.max(end)));
                state.apply(&EditCommand::SetComposingText { text: text.clone(), new_cursor_position: 1 });
                state.apply(&EditCommand::SetSelection { start: at + start, end: at + end });
            }
            _ => state.apply(&EditCommand::SetComposingText { text: text.clone(), new_cursor_position: 1 }),
        },
        &Event::DeleteSurroundingText { before, after } => {
            state.apply(&EditCommand::DeleteSurroundingText { before, after })
        }
        &Event::SetSelection { start, end } => state.apply(&EditCommand::SetSelection { start, end }),
        _ => {}
    }
}

/// The focused field as the platform's input method paths share it: whether one takes typing, where its caret is
/// (px in the window's client area), what it wants, and its text, selection and composition as the core will have
/// them once it has handled the events sent (`unanswered` while it hasn't: the field the core then has is the
/// answer, [`answer`](Field::answer)).
#[derive(Clone, Debug, Default)]
pub struct Field {
    pub active: bool,
    pub area: Rect,
    pub options: ImeOptions,
    pub state: TextEditState,
    /// Events were sent that the core hasn't handled yet.
    pub unanswered: bool,
}

/// What the field the core has after it handled what was sent (or after the app changed it) is to the input
/// method.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Answer {
    /// What the input method sees: nothing to tell.
    Same,
    /// The app changed the text (or took less of what was typed): the input method is told, its composition ended
    /// where the field's isn't the one it composes.
    Changed,
}

impl Field {
    /// `events` sent to the core: applied here (the input method's next question is answered from the field as it
    /// will be); them again, for the window's queue.
    pub fn send(&mut self, events: Vec<Event>) -> Vec<Event> {
        for e in &events {
            apply(&mut self.state, e);
        }
        if !events.is_empty() {
            self.unanswered = true;
        }
        events
    }

    /// The field as the core has it (after handling what was sent, or changed by the app: `text_input_changed`):
    /// whether it is what the input method sees.
    pub fn answer(&mut self, state: &TextEditState) -> Answer {
        self.unanswered = false;
        let state = normalized(state);
        if state == normalized(&self.state) {
            return Answer::Same;
        }
        self.state = state;
        Answer::Changed
    }
}

// ---- IMM32 ------------------------------------------------------------------------------------------------------

/// IMM32's attributes of a composition's characters (imm.h's ATTR_*), and TSF's display attributes' (TF_ATTR_*,
/// the same values): the clause being converted is the target.
pub const ATTR_INPUT: u8 = 0;
pub const ATTR_TARGET_CONVERTED: u8 = 1;
#[cfg(test)]
pub const ATTR_CONVERTED: u8 = 2;
pub const ATTR_TARGET_NOTCONVERTED: u8 = 3;

/// The clause being converted (UTF-16 units of the composition): the first run of target attributes; none without
/// one (nothing converted yet: the input method's caret shows where typing goes).
pub fn target_clause(attributes: &[u8]) -> Option<(usize, usize)> {
    let target = |a: &u8| *a == ATTR_TARGET_CONVERTED || *a == ATTR_TARGET_NOTCONVERTED;
    let start = attributes.iter().position(target)?;
    let len = attributes[start..].iter().take_while(|a| target(a)).count();
    Some((start, start + len))
}

/// IMM32's composition (GCS_COMPSTR's UTF-16 string, GCS_COMPATTR's attribute per unit, GCS_CURSORPOS's caret) as
/// the core's composing text: the clause being converted highlighted, else the caret where the input method has it
/// (none: after the text). Empty: the composition gone.
pub fn imm_preedit(text: &[u16], attributes: &[u8], cursor: Option<usize>) -> Event {
    let text = String::from_utf16_lossy(text);
    if text.is_empty() {
        return Event::Preedit { text, cursor: None };
    }
    let cursor = match target_clause(attributes) {
        Some((a, b)) => Some((byte_of_utf16(&text, a), byte_of_utf16(&text, b))),
        None => cursor.map(|c| {
            let at = byte_of_utf16(&text, c);
            (at, at)
        }),
    };
    Event::Preedit { text, cursor }
}

/// The most of a field an input method is given around the selection to reconvert or to read (UTF-16 units: a
/// RECONVERTSTRING's string).
pub const RECONVERSION_MAX: usize = 1024;

/// What a reconversion (IMR_RECONVERTSTRING) or an input method's reading of the document (IMR_DOCUMENTFEED) is
/// given: the paragraph around the part it is about (at most [`RECONVERSION_MAX`] units, cut at whole characters),
/// that part in it (`composition`: to reconvert, or being composed) and the part of it to convert first
/// (`target`), UTF-16 units of `text`; where `text` begins in the field (units).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reconversion {
    pub text: Vec<u16>,
    pub start: usize,
    pub composition: (usize, usize),
    pub target: (usize, usize),
}

/// The size of RECONVERTSTRING's header (eight DWORDs).
pub const RECONVERTSTRING_HEADER: usize = 32;

impl Reconversion {
    /// It as Windows' RECONVERTSTRING (imm.h, the wide version): the header — its size, version 0, the string's
    /// length (units) and offset (bytes from the header's start), the composition's and the target's lengths
    /// (units) and offsets (bytes from the string's start) — then the string, 0-terminated.
    pub fn encode(&self) -> Vec<u8> {
        let size = RECONVERTSTRING_HEADER + (self.text.len() + 1) * 2;
        let mut out = Vec::with_capacity(size);
        let (comp, target) = (self.composition, self.target);
        for value in [
            size,
            0,
            self.text.len(),
            RECONVERTSTRING_HEADER,
            comp.1 - comp.0,
            comp.0 * 2,
            target.1 - target.0,
            target.0 * 2,
        ] {
            out.extend_from_slice(&(value as u32).to_le_bytes());
        }
        for unit in self.text.iter().chain(&[0]) {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        out
    }

    /// What an input method confirmed of a RECONVERTSTRING we gave it (IMR_CONFIRMRECONVERTSTRING: the part it
    /// reconverts, which it may have moved to whole words), as units of the field — `start` being where the
    /// string we gave began in it; none if the buffer doesn't hold what it says.
    pub fn confirmed(buffer: &[u8], start: usize) -> Option<(usize, usize)> {
        let dword = |i: usize| -> Option<usize> {
            let b = buffer.get(i * 4..i * 4 + 4)?;
            Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        };
        let (str_len, comp_len, comp_offset) = (dword(2)?, dword(4)?, dword(5)?);
        if comp_offset % 2 != 0 || comp_offset / 2 + comp_len > str_len {
            return None;
        }
        Some((start + comp_offset / 2, start + comp_offset / 2 + comp_len))
    }
}

/// The part of `text` (bytes) an input method is given around bytes `lo` to `hi`: the paragraph they are in (the
/// line breaks around them), at most `max` UTF-16 units of it — `lo` to `hi` whole where they fit, the rest shared
/// before and after them (a side's unused share the other's) —, cut at whole characters.
fn paragraph_around(text: &str, lo: usize, hi: usize, max: usize) -> (usize, usize) {
    let start = text[..lo].rfind('\n').map_or(0, |i| i + 1);
    let end = text[hi..].find('\n').map_or(text.len(), |i| hi + i);
    let units = |a: usize, b: usize| utf16_len(&text[a..b]);
    let inner = units(lo, hi);
    if inner >= max {
        // (Longer than all there is room for: its beginning.)
        return (lo, byte_of_utf16_from(text, lo, max));
    }
    let spare = max - inner;
    let (have_before, have_after) = (units(start, lo), units(hi, end));
    let after = (spare - (spare / 2).min(have_before)).min(have_after);
    let before = (spare - after).min(have_before);
    let from = byte_of_utf16_back(text, lo, before);
    (ceil_char(text, from.max(start)), byte_of_utf16_from(text, hi, after).min(end))
}

/// The byte `units` UTF-16 units after byte `from` of `text` (whole characters: no further).
fn byte_of_utf16_from(text: &str, from: usize, units: usize) -> usize {
    from + byte_of_utf16(&text[from..], units)
}

/// The byte at most `units` UTF-16 units before byte `to` of `text` (whole characters).
fn byte_of_utf16_back(text: &str, to: usize, units: usize) -> usize {
    let mut left = units;
    let mut at = to;
    for (i, c) in text[..to].char_indices().rev() {
        if c.len_utf16() > left {
            break;
        }
        left -= c.len_utf16();
        at = i;
    }
    at
}

/// What IMR_RECONVERTSTRING (`feed` false: the selection to reconvert; none while composing) or IMR_DOCUMENTFEED
/// (`feed`: the text around the composition, or the caret, for the input method's prediction) is answered with.
pub fn reconversion(state: &TextEditState, feed: bool, max: usize) -> Option<Reconversion> {
    let state = normalized(state);
    let (lo, hi) = match (composition(&state), feed) {
        (Some(_), false) => return None,
        (Some(c), true) => c,
        (None, _) => selection(&state),
    };
    let text = &state.text;
    let (start, end) = paragraph_around(text, lo, hi, max.max(1));
    let (start, end) = (start.min(lo), end.max(start));
    let units = |a: usize, b: usize| utf16_len(&text[a..b]);
    let hi = hi.min(end);
    let part = (units(start, lo), units(start, lo) + units(lo, hi));
    // (What to convert first: the selection, where it is inside the composition being read; else all of it.)
    let target = match feed {
        true => {
            let (a, b) = selection(&state);
            if a >= lo && b <= hi && a < b { (units(start, a), units(start, b)) } else { part }
        }
        false => part,
    };
    Some(Reconversion {
        text: text[start..end].encode_utf16().collect(),
        start: utf16_len(&text[..start]),
        composition: part,
        target,
    })
}

/// Where a character is for an input method (IMR_QUERYCHARPOSITION's IMECHARPOSITION, TSF's GetTextExt): its top
/// left, its line's height and the document's rectangle, on the screen (`origin`: the client area's top left on
/// it) — every character's the caret's box (what the host tells), at least a pixel wide and high.
pub fn char_position(area: Rect, origin: (i32, i32)) -> CharPosition {
    let (left, top) = (area.left.floor() as i32 + origin.0, area.top.floor() as i32 + origin.1);
    let (right, bottom) =
        ((area.right.ceil() as i32 + origin.0).max(left + 1), (area.bottom.ceil() as i32 + origin.1).max(top + 1));
    CharPosition { left, top, right, bottom }
}

/// A rectangle on the screen (px), as Win32's RECT.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CharPosition {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Where the input method's windows go (client coordinates): its composition window's point (the caret's top left,
/// which it doesn't show: KompozR draws the composing text) and the rectangle its candidate window keeps out of
/// (the caret's box, CFS_EXCLUDE: below it, or above where there is no room).
pub fn placement(area: Rect) -> (CharPosition, CharPosition) {
    let r = char_position(area, (0, 0));
    (CharPosition { left: r.left, top: r.top, right: r.left, bottom: r.top }, r)
}

// ---- TSF --------------------------------------------------------------------------------------------------------

/// The text store's document (what TSF's text services read and edit, under its locks): the field's text, its
/// selection (and which end of it is active: TS_AE_START or TS_AE_END), the composition a text service has going
/// (as its composition view's range says), the display attributes of that composition's runs (UTF-16 units of the
/// text, TF_ATTR_*). Offsets are ACPs: UTF-16 units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub state: TextEditState,
    pub active_start: bool,
    pub attributes: Vec<(usize, usize, u8)>,
}

impl Document {
    pub fn of(state: &TextEditState) -> Document {
        Document { state: normalized(state), active_start: false, attributes: Vec::new() }
    }

    fn text(&self) -> &str {
        &self.state.text
    }

    /// The text's end (GetEndACP).
    pub fn end(&self) -> usize {
        utf16_len(self.text())
    }

    /// The selection, units, in order (GetSelection).
    pub fn selection(&self) -> (usize, usize) {
        let (a, b) = selection(&self.state);
        (utf16_of_byte(self.text(), a), utf16_of_byte(self.text(), b))
    }

    /// The selection set (SetSelection; units, clamped to the text and to whole characters), `active_start` its
    /// active end.
    pub fn set_selection(&mut self, start: usize, end: usize, active_start: bool) {
        let (a, b) = (byte_of_utf16(self.text(), start.min(end)), byte_of_utf16(self.text(), start.max(end)));
        self.state.selection_start = a;
        self.state.selection_end = b;
        self.active_start = active_start && a != b;
    }

    /// The composition, units (as its view's range last said).
    pub fn composition(&self) -> Option<(usize, usize)> {
        let (a, b) = composition(&self.state)?;
        Some((utf16_of_byte(self.text(), a), utf16_of_byte(self.text(), b)))
    }

    /// The composition as a text service's composition view's range has it now (units; none: none going on).
    pub fn set_composition(&mut self, range: Option<(usize, usize)>) {
        self.state.composition = range
            .map(|(a, b)| (byte_of_utf16(self.text(), a.min(b)), byte_of_utf16(self.text(), a.max(b))))
            .filter(|(a, b)| a < b);
        if self.state.composition.is_none() {
            self.attributes.clear();
        }
    }

    /// The text from `start` to `end` (units; `end` none: the text's end), clamped (GetText).
    pub fn text_of(&self, start: usize, end: Option<usize>) -> Vec<u16> {
        let len = self.end();
        let end = end.unwrap_or(len).min(len);
        let start = start.min(end);
        let (a, b) = (byte_of_utf16(self.text(), start), byte_of_utf16(self.text(), end));
        self.text()[a..b].encode_utf16().collect()
    }

    /// Units `start` to `end` replaced by `text` (SetText, InsertTextAtSelection): the change as TS_TEXTCHANGE says
    /// it (start, old end, new end); the selection after the inserted text (none moved where it didn't cover it:
    /// kept, shifted past the change), a composition around it grown with it.
    pub fn replace(&mut self, start: usize, end: usize, text: &[u16]) -> (usize, usize, usize) {
        let inserted = String::from_utf16_lossy(text);
        let (a, b) = (byte_of_utf16(self.text(), start.min(end)), byte_of_utf16(self.text(), start.max(end)));
        let (start, old_end) = (utf16_of_byte(self.text(), a), utf16_of_byte(self.text(), b));
        let shift = |at: usize| -> usize {
            if at >= b {
                at - (b - a) + inserted.len()
            } else if at > a {
                a + inserted.len()
            } else {
                at
            }
        };
        let (s0, s1) = selection(&self.state);
        let (s0, s1) = if (s0 <= a && s1 >= b) || (s0 >= a && s1 <= b) {
            // (Covering the replaced part, or inside it: after the inserted text.)
            let at = a + inserted.len();
            (at, at)
        } else {
            (shift(s0), shift(s1))
        };
        let composition = composition(&self.state).map(|(c0, c1)| {
            // (A composition the change is in grows with it.)
            if c0 <= a && c1 >= b { (c0, c1 - (b - a) + inserted.len()) } else { (shift(c0), shift(c1)) }
        });
        self.state.text.replace_range(a..b, &inserted);
        self.state.selection_start = s0;
        self.state.selection_end = s1;
        self.state.composition = composition.filter(|(c0, c1)| c0 < c1);
        if s0 == s1 {
            self.active_start = false;
        }
        (start, old_end, start + text.len())
    }

    /// Where `cch` units would go in place of units `start` to `end` (QueryInsert): there (the field takes text
    /// anywhere), clamped to the text.
    pub fn query_insert(&self, start: usize, end: usize) -> (usize, usize) {
        let len = self.end();
        (start.min(len), end.min(len).max(start.min(len)))
    }

    /// The document as the core is to have it: its composition's clause being converted (its display attributes'
    /// target, as IMM32's) as the selection inside it, as the core shows a composing text's highlighted part; else
    /// as it is.
    pub fn presented(&self) -> TextEditState {
        let mut state = normalized(&self.state);
        if let Some((c0, c1)) = self.composition() {
            let mut attributes = vec![ATTR_INPUT; c1 - c0];
            for &(a, b, attr) in &self.attributes {
                for i in a.max(c0)..b.min(c1) {
                    attributes[i - c0] = attr;
                }
            }
            if let Some((t0, t1)) = target_clause(&attributes) {
                state.selection_start = byte_of_utf16(&state.text, c0 + t0);
                state.selection_end = byte_of_utf16(&state.text, c0 + t1);
            }
        }
        state
    }
}

/// The core's events that take the field from `told` (what the core has, or will have once it handled what was
/// sent) to `now` (the text store's document as presented): a composition going on in both at the same place, the
/// rest of the text as it was, its new composing text; else the old composition committed as it stands, then the
/// part of the text that differs replaced — around the selection (what was deleted before and after it, then what
/// replaces it), or selected and replaced where it isn't around it —, the new composition composed at its place;
/// the selection set last where that left it elsewhere. Applied to `told` ([`apply`]) they give `now`.
pub fn transition(told: &TextEditState, now: &TextEditState) -> Vec<Event> {
    let mut cur = normalized(told);
    let now = normalized(now);
    let mut events = Vec::new();
    let mut push = |cur: &mut TextEditState, e: Event| {
        apply(cur, &e);
        events.push(e);
    };
    let preedit_of = |now: &TextEditState, (c0, c1): (usize, usize)| {
        let (s0, s1) = selection(now);
        let cursor = (s0 >= c0 && s1 <= c1).then(|| (s0 - c0, s1 - c0));
        Event::Preedit { text: now.text[c0..c1].to_string(), cursor }
    };
    let mut composed = false;
    if let Some((a, b)) = composition(&cur) {
        match composition(&now) {
            Some((c, d)) if a == c && cur.text[..a] == now.text[..c] && cur.text[b..] == now.text[d..] => {
                if cur != now {
                    push(&mut cur, preedit_of(&now, (c, d)));
                }
                composed = true;
            }
            None if now.text.len() + b >= cur.text.len() + a
                && now.text.starts_with(&cur.text[..a])
                && now.text.ends_with(&cur.text[b..]) =>
            {
                // (The composition committed as the text in its place: what the input method converted it to.)
                let committed = now.text[a..now.text.len() - (cur.text.len() - b)].to_string();
                push(&mut cur, Event::Text(committed));
            }
            _ => {
                let committed = cur.text[a..b].to_string();
                push(&mut cur, Event::Text(committed));
            }
        }
    }
    if !composed {
        match composition(&now) {
            None => edit_to(&mut cur, &now.text, &mut push),
            Some((c, d)) => {
                // (What the composition replaces is the selection, the rest as it was: composed in its place.)
                let (lo, hi) = selection(&cur);
                if !(lo == c && cur.text[..lo] == now.text[..c] && cur.text[hi..] == now.text[d..]) {
                    let outside = format!("{}{}", &now.text[..c], &now.text[d..]);
                    edit_to(&mut cur, &outside, &mut push);
                    if selection(&cur) != (c, c) {
                        push(&mut cur, Event::SetSelection { start: c, end: c });
                    }
                }
                push(&mut cur, preedit_of(&now, (c, d)));
            }
        }
    }
    let (s0, s1) = selection(&now);
    if selection(&cur) != (s0, s1) {
        push(&mut cur, Event::SetSelection { start: s0, end: s1 });
    }
    events
}

/// `cur` (no composition) given `target`'s text: the part that differs replaced — around the selection where it is
/// around it (what is deleted before and after it, then what replaces it), else selected and replaced (the text
/// between it and the caret left as it is: the caller puts the selection back).
fn edit_to(cur: &mut TextEditState, target: &str, push: &mut impl FnMut(&mut TextEditState, Event)) {
    let text = cur.text.clone();
    if text == target {
        return;
    }
    let prefix = common_prefix(&text, target);
    let suffix = common_suffix(&text, target);
    // The smallest part that differs, `deleted` bytes replaced by `inserted`; where the text repeats around it
    // (typing "a" between two others) it may be anywhere along the repeat: around the selection where it can be.
    let p = prefix;
    let s = suffix.min(text.len().min(target.len()) - p);
    let (deleted, inserted) = (text.len() - s - p, target.len() - s - p);
    let (lo, hi) = selection(cur);
    let first = (text.len() - deleted - suffix.min(text.len() - deleted)).max(hi.saturating_sub(deleted));
    let at =
        (first..=prefix.min(lo)).rev().find(|&x| text.is_char_boundary(x) && target.is_char_boundary(x + inserted));
    if let Some(x) = at {
        let replacement = target[x..x + inserted].to_string();
        around_selection(cur, (x, x + deleted), &replacement, push);
        return;
    }
    let (r0, r1) = (p, p + deleted);
    let replacement = &target[p..p + inserted];
    push(cur, Event::SetSelection { start: r0, end: r1 });
    push(cur, Event::Text(replacement.to_string()));
}

/// The part `r0` to `r1` around the selection replaced by `replacement`: what is around the selection deleted, then
/// the selection replaced.
fn around_selection(
    cur: &mut TextEditState,
    (r0, r1): (usize, usize),
    replacement: &str,
    push: &mut impl FnMut(&mut TextEditState, Event),
) {
    let (lo, hi) = selection(cur);
    let (before, after) = (lo - r0, r1 - hi);
    if before > 0 || after > 0 {
        push(cur, Event::DeleteSurroundingText { before, after });
    }
    if !replacement.is_empty() || lo != hi {
        push(cur, Event::Text(replacement.to_string()));
    }
}

/// The bytes `a` and `b` begin with alike (whole characters).
fn common_prefix(a: &str, b: &str) -> usize {
    let n = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    floor_char(a, n)
}

/// The bytes `a` and `b` end with alike (whole characters).
fn common_suffix(a: &str, b: &str) -> usize {
    let n = a.bytes().rev().zip(b.bytes().rev()).take_while(|(x, y)| x == y).count();
    a.len() - ceil_char(a, a.len() - n)
}

/// What the text store tells TSF of the app's change from `old` to `new` (TS_TEXTCHANGE: the start, the old end,
/// the new end, units): the part that differs; none if the text is the same.
pub fn text_change(old: &str, new: &str) -> Option<(usize, usize, usize)> {
    if old == new {
        return None;
    }
    let p = common_prefix(old, new);
    let s = common_suffix(old, new).min(old.len().min(new.len()) - p);
    let start = utf16_len(&old[..p]);
    Some((start, utf16_len(&old[..old.len() - s]), utf16_len(&new[..new.len() - s])))
}

// ---- Input scopes -----------------------------------------------------------------------------------------------

// Windows' input scopes (inputscope.h's InputScope), the ones a field's options become.
pub const IS_DEFAULT: i32 = 0;
pub const IS_URL: i32 = 1;
pub const IS_EMAIL_SMTPEMAILADDRESS: i32 = 5;
pub const IS_PERSONALNAME_FULLNAME: i32 = 7;
pub const IS_ADDRESS_FULLPOSTALADDRESS: i32 = 13;
pub const IS_DATE_FULLDATE: i32 = 22;
pub const IS_DIGITS: i32 = 28;
pub const IS_NUMBER: i32 = 29;
pub const IS_PASSWORD: i32 = 31;
pub const IS_TELEPHONE_FULLTELEPHONENUMBER: i32 = 32;
pub const IS_TIME_FULLTIME: i32 = 36;
pub const IS_ALPHANUMERIC_HALFWIDTH: i32 = 40;
pub const IS_SEARCH: i32 = 50;
pub const IS_SEARCH_INCREMENTAL: i32 = 52;
pub const IS_YOMI: i32 = 56;
pub const IS_TEXT: i32 = 57;
pub const IS_CHAT: i32 = 58;
pub const IS_NUMERIC_PASSWORD: i32 = 63;

/// A field's options as Windows' input scopes (what the touch keyboard lays out, what a text service offers;
/// Linux's content purposes, C++ had none): its keyboard type's, a search field's (its action key Search) as
/// search; a password's hidden ones as passwords (a numeric one's digits).
pub fn input_scopes(options: &ImeOptions) -> Vec<i32> {
    let text = match options.ime_action {
        ImeAction::Search => IS_SEARCH,
        _ => IS_TEXT,
    };
    match options.keyboard_type {
        KeyboardType::Unspecified if options.ime_action == ImeAction::Search => vec![IS_SEARCH],
        KeyboardType::Unspecified => vec![IS_DEFAULT],
        KeyboardType::Text | KeyboardType::EmailSubject | KeyboardType::LongMessage => vec![text],
        KeyboardType::Ascii => vec![IS_ALPHANUMERIC_HALFWIDTH],
        KeyboardType::Number => vec![IS_DIGITS],
        KeyboardType::Decimal | KeyboardType::NumberSigned | KeyboardType::DecimalSigned => vec![IS_NUMBER],
        KeyboardType::Phone => vec![IS_TELEPHONE_FULLTELEPHONENUMBER],
        KeyboardType::Uri => vec![IS_URL],
        KeyboardType::Email => vec![IS_EMAIL_SMTPEMAILADDRESS],
        KeyboardType::Password | KeyboardType::PasswordVisible => vec![IS_PASSWORD],
        KeyboardType::NumberPassword => vec![IS_NUMERIC_PASSWORD],
        KeyboardType::DecimalPassword | KeyboardType::NumberPasswordSigned | KeyboardType::DecimalPasswordSigned => {
            vec![IS_PASSWORD, IS_NUMBER]
        }
        KeyboardType::PostalAddress => vec![IS_ADDRESS_FULLPOSTALADDRESS],
        KeyboardType::PersonName => vec![IS_PERSONALNAME_FULLNAME],
        KeyboardType::ShortMessage => vec![IS_CHAT],
        KeyboardType::Filter => vec![IS_SEARCH_INCREMENTAL],
        KeyboardType::Phonetic => vec![IS_YOMI],
        KeyboardType::DateTime => vec![IS_DATE_FULLDATE, IS_TIME_FULLTIME],
        KeyboardType::Date => vec![IS_DATE_FULLDATE],
        KeyboardType::Time => vec![IS_TIME_FULLTIME],
    }
}

/// Whether the field hides what is typed (a password's, not a shown one's): no input method composes in it (its
/// composing text would show what is typed; Windows' own password boxes have none).
pub fn hides_text(options: &ImeOptions) -> bool {
    matches!(
        options.keyboard_type,
        KeyboardType::Password
            | KeyboardType::NumberPassword
            | KeyboardType::DecimalPassword
            | KeyboardType::NumberPasswordSigned
            | KeyboardType::DecimalPasswordSigned
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str, selection: (usize, usize), composition: Option<(usize, usize)>) -> TextEditState {
        TextEditState { text: text.into(), selection_start: selection.0, selection_end: selection.1, composition }
    }

    fn units(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn offsets_are_utf16() {
        // "ğ😀a": ğ is 2 bytes and 1 unit, 😀 4 bytes and 2 units.
        let text = "ğ😀a";
        assert_eq!(utf16_len(text), 4);
        assert_eq!((utf16_of_byte(text, 2), utf16_of_byte(text, 6), utf16_of_byte(text, 4)), (1, 3, 1));
        assert_eq!((byte_of_utf16(text, 1), byte_of_utf16(text, 2), byte_of_utf16(text, 3)), (2, 2, 6));
        assert_eq!(byte_of_utf16(text, 9), 7);
        assert_eq!(normalized(&state(text, (6, 3), Some((4, 2)))), state(text, (2, 6), Some((2, 6))));
        assert_eq!(normalized(&state(text, (1, 1), Some((2, 2)))).composition, None);
    }

    #[test]
    fn imm32_compositions_become_the_cores_composing_text() {
        // Japanese: "かんじ" typed, the caret after it; converted, its first clause "漢字" the target.
        let typed = imm_preedit(&units("かんじ"), &[ATTR_INPUT; 3], Some(3));
        assert_eq!(typed, Event::Preedit { text: "かんじ".into(), cursor: Some((9, 9)) });
        let converted =
            imm_preedit(&units("漢字を"), &[ATTR_TARGET_CONVERTED, ATTR_TARGET_CONVERTED, ATTR_CONVERTED], Some(0));
        assert_eq!(converted, Event::Preedit { text: "漢字を".into(), cursor: Some((0, 6)) });
        // The second clause the target (not converted yet), a character beyond the BMP before it.
        let attributes = [ATTR_CONVERTED, ATTR_CONVERTED, ATTR_TARGET_NOTCONVERTED];
        assert_eq!(
            imm_preedit(&units("😀を"), &attributes, None),
            Event::Preedit { text: "😀を".into(), cursor: Some((4, 7)) }
        );
        // No caret: after it, hidden. Empty: gone.
        assert_eq!(imm_preedit(&units("a"), &[], None), Event::Preedit { text: "a".into(), cursor: None });
        assert_eq!(imm_preedit(&[], &[], Some(0)), Event::Preedit { text: String::new(), cursor: None });
        assert_eq!(target_clause(&[0, 1, 1, 2, 1]), Some((1, 3)));
        assert_eq!(target_clause(&[0, 2]), None);
    }

    #[test]
    fn the_field_follows_what_is_sent_and_what_the_core_answers() {
        let mut field = Field { active: true, state: state("ab", (2, 2), None), ..Field::default() };
        let sent = field.send(vec![Event::Preedit { text: "か".into(), cursor: None }]);
        assert_eq!(sent.len(), 1);
        assert!(field.unanswered);
        assert_eq!(field.state, state("abか", (5, 5), Some((2, 5))));
        field.send(vec![Event::Text("漢".into())]);
        assert_eq!(field.state, state("ab漢", (5, 5), None));
        // The core took it as sent: nothing to tell. The app changed it: told.
        assert_eq!(field.answer(&state("ab漢", (5, 5), None)), Answer::Same);
        assert!(!field.unanswered);
        assert_eq!(field.answer(&state("", (0, 0), None)), Answer::Changed);
        assert_eq!(field.state, state("", (0, 0), None));
    }

    #[test]
    fn reconversion_gives_the_paragraph_around_the_selection() {
        // "first\nsecond word\nthird": "word" selected → the second line, "word" the part.
        let text = "first\nsecond word\nthird";
        let r = reconversion(&state(text, (13, 17), None), false, RECONVERSION_MAX).unwrap();
        assert_eq!(String::from_utf16_lossy(&r.text), "second word");
        assert_eq!((r.start, r.composition, r.target), (6, (7, 11), (7, 11)));
        // Encoded: the header, then the string; the offsets bytes, the lengths units.
        let bytes = r.encode();
        let dword = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(bytes.len(), 32 + 12 * 2);
        assert_eq!([dword(0), dword(1), dword(2), dword(3)], [56, 0, 11, 32]);
        assert_eq!([dword(4), dword(5), dword(6), dword(7)], [4, 14, 4, 14]);
        assert_eq!(u16::from_le_bytes([bytes[32], bytes[33]]), 's' as u16);
        // Confirmed as the input method moved it ("second word" whole): that part of the field.
        let mut confirmed = bytes.clone();
        confirmed[16..20].copy_from_slice(&11u32.to_le_bytes());
        confirmed[20..24].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(Reconversion::confirmed(&confirmed, r.start), Some((6, 17)));
        confirmed[16..20].copy_from_slice(&40u32.to_le_bytes());
        assert_eq!(Reconversion::confirmed(&confirmed, r.start), None);
        // While composing: no reconversion, but the document read around the composition (UTF-16 units: 😀 is two).
        let composing = state("😀 かな", (10, 10), Some((5, 11)));
        assert_eq!(reconversion(&composing, false, RECONVERSION_MAX), None);
        let feed = reconversion(&composing, true, RECONVERSION_MAX).unwrap();
        assert_eq!((feed.start, feed.composition, feed.target), (0, (3, 5), (3, 5)));
        // The caret alone: an empty part at it.
        let caret = reconversion(&state("abc", (1, 1), None), false, 8).unwrap();
        assert_eq!((caret.composition, String::from_utf16_lossy(&caret.text)), ((1, 1), "abc".into()));
    }

    #[test]
    fn a_long_paragraph_is_cut_around_the_part() {
        let digits = "0123456789";
        let at = |sel: (usize, usize), max: usize| {
            let r = reconversion(&state(digits, sel, None), false, max).unwrap();
            (String::from_utf16_lossy(&r.text), r.start, r.composition)
        };
        assert_eq!(at((5, 5), 4), ("3456".into(), 3, (2, 2)));
        assert_eq!(at((1, 1), 4), ("0123".into(), 0, (1, 1)));
        assert_eq!(at((9, 9), 4), ("6789".into(), 6, (3, 3)));
        assert_eq!(at((2, 6), 6), ("123456".into(), 1, (1, 5)));
        // Longer than there is room for: its beginning.
        assert_eq!(at((1, 9), 4), ("1234".into(), 1, (0, 4)));
        // Whole characters: 😀 not cut in half.
        let r = reconversion(&state("a😀b", (5, 5), None), false, 2).unwrap();
        assert_eq!(String::from_utf16_lossy(&r.text), "b");
    }

    #[test]
    fn characters_are_where_the_caret_is() {
        let p = char_position(Rect::new(10.2, 20.0, 11.0, 38.5), (100, 200));
        assert_eq!(p, CharPosition { left: 110, top: 220, right: 111, bottom: 239 });
        let (point, exclude) = placement(Rect::new(10.0, 20.0, 10.0, 40.0));
        assert_eq!((point.left, point.top), (10, 20));
        assert_eq!(exclude, CharPosition { left: 10, top: 20, right: 11, bottom: 40 });
    }

    /// `transition` from `told` to `now`: its events, which applied to `told` give `now`.
    fn checked(told: TextEditState, now: TextEditState) -> Vec<Event> {
        let events = transition(&told, &now);
        let mut applied = normalized(&told);
        for e in &events {
            apply(&mut applied, e);
        }
        assert_eq!(applied, normalized(&now), "{events:?}");
        events
    }

    #[test]
    fn a_text_services_edits_become_the_cores_events() {
        let preedit = |t: &str, c: Option<(usize, usize)>| Event::Preedit { text: t.into(), cursor: c };
        // Composing at the caret, the composition growing, committed.
        let events = checked(state("ab", (2, 2), None), state("abk", (3, 3), Some((2, 3))));
        assert_eq!(events, [preedit("k", Some((1, 1)))]);
        let events = checked(state("abk", (3, 3), Some((2, 3))), state("abか", (5, 5), Some((2, 5))));
        assert_eq!(events, [preedit("か", Some((3, 3)))]);
        let events = checked(state("abか", (5, 5), Some((2, 5))), state("ab家", (5, 5), None));
        assert_eq!(events, [Event::Text("家".into())]);
        // The composition replacing the selection.
        let events = checked(state("a word", (2, 6), None), state("a ワ", (5, 5), Some((2, 5))));
        assert_eq!(events, [preedit("ワ", Some((3, 3)))]);
        // Typed outside a composition (the touch keyboard, a voice): at the caret.
        let events = checked(state("ab", (1, 1), None), state("aXb", (2, 2), None));
        assert_eq!(events, [Event::Text("X".into())]);
        // A correction of the word before the caret (its space between): selected and replaced, the caret put back.
        let events = checked(state("teh ", (4, 4), None), state("the ", (4, 4), None));
        assert_eq!(
            events,
            [
                Event::SetSelection { start: 1, end: 3 },
                Event::Text("he".into()),
                Event::SetSelection { start: 4, end: 4 }
            ]
        );
        // Typed between two of the same: at the caret.
        assert_eq!(checked(state("aa", (1, 1), None), state("aaa", (2, 2), None)), [Event::Text("a".into())]);
        // An edit far from the caret: selected and replaced, the caret put back.
        let long = "first word and a long sentence after it";
        let fixed = long.replacen("first", "First", 1);
        let events = checked(state(long, (39, 39), None), state(&fixed, (39, 39), None));
        assert_eq!(
            events,
            [
                Event::SetSelection { start: 0, end: 1 },
                Event::Text("F".into()),
                Event::SetSelection { start: 39, end: 39 }
            ]
        );
        // The selection moved alone; deleted text.
        assert_eq!(
            checked(state("abc", (0, 0), None), state("abc", (1, 3), None)),
            [Event::SetSelection { start: 1, end: 3 }]
        );
        assert_eq!(
            checked(state("abc", (3, 3), None), state("ab", (2, 2), None)),
            [Event::DeleteSurroundingText { before: 1, after: 0 }]
        );
        assert_eq!(checked(state("abc", (1, 2), None), state("ac", (1, 1), None)), [Event::Text(String::new())]);
        // A reconversion: the word selected, a composition started over it.
        let events = checked(state("kanji desu", (10, 10), None), state("漢字 desu", (6, 6), Some((0, 6))));
        assert_eq!(events[0], Event::SetSelection { start: 0, end: 5 });
        // A composition moved (committed, another begun elsewhere), and a composition with a selection outside it.
        checked(state("ab cd", (2, 2), Some((0, 2))), state("ab cd", (5, 5), Some((3, 5))));
        checked(state("ab", (2, 2), None), state("abxy", (0, 1), Some((2, 4))));
        // Nothing changed: nothing.
        assert!(checked(state("same", (2, 2), Some((1, 3))), state("same", (2, 2), Some((1, 3)))).is_empty());
    }

    #[test]
    fn many_edits_round_trip() {
        // Every pair of a few states: the events always take one to the other.
        let states = [
            state("", (0, 0), None),
            state("abc", (1, 1), None),
            state("abc", (0, 3), None),
            state("abc", (1, 2), Some((0, 3))),
            state("aXbc", (2, 2), Some((1, 2))),
            state("ğ😀a", (2, 6), None),
            state("ğ😀a", (7, 7), Some((2, 6))),
            state("x\ny", (2, 2), Some((2, 3))),
            state("abcabc", (3, 3), None),
            state("abcXabc", (4, 4), Some((3, 4))),
        ];
        for a in &states {
            for b in &states {
                checked(a.clone(), b.clone());
            }
        }
    }

    #[test]
    fn the_documents_edits_in_acps() {
        // TSF's offsets: units. "a😀c" is 4 of them.
        let mut doc = Document::of(&state("a😀c", (6, 6), None));
        assert_eq!((doc.end(), doc.selection()), (4, (4, 4)));
        assert_eq!(doc.text_of(1, Some(3)), units("😀"));
        assert_eq!(doc.text_of(3, None), units("c"));
        assert_eq!(doc.text_of(9, None), Vec::<u16>::new());
        // Inserted at the selection, which goes after it.
        assert_eq!(doc.replace(4, 4, &units("dé")), (4, 4, 6));
        assert_eq!((doc.state.text.as_str(), doc.selection()), ("a😀cdé", (6, 6)));
        // A composition started over "dé" and replaced inside: it grows with it.
        doc.set_composition(Some((4, 6)));
        assert_eq!(doc.replace(4, 6, &units("ありがとう")), (4, 6, 9));
        assert_eq!(doc.composition(), Some((4, 9)));
        assert_eq!(doc.selection(), (9, 9));
        // A replacement before the selection shifts it; the active end kept while it is a range.
        doc.set_selection(5, 7, true);
        assert!(doc.active_start);
        doc.replace(0, 1, &units("AB"));
        assert_eq!(doc.selection(), (6, 8));
        assert_eq!(doc.composition(), Some((5, 10)));
        assert_eq!(doc.query_insert(3, 99), (3, 10));
        // Display attributes: the target clause presented as the core's selection.
        doc.attributes = vec![(5, 7, ATTR_TARGET_CONVERTED), (7, 10, ATTR_CONVERTED)];
        let presented = doc.presented();
        assert_eq!(
            (
                utf16_of_byte(&presented.text, presented.selection_start),
                utf16_of_byte(&presented.text, presented.selection_end)
            ),
            (5, 7)
        );
        doc.set_composition(None);
        assert!(doc.attributes.is_empty());
        // The app's change as TSF is told: the part that differs.
        assert_eq!(text_change("hello", "help"), Some((3, 5, 4)));
        assert_eq!(text_change("😀", "😀!"), Some((2, 2, 3)));
        assert_eq!(text_change("same", "same"), None);
    }

    #[test]
    fn options_become_input_scopes() {
        let scopes = |keyboard_type, ime_action| {
            input_scopes(&ImeOptions { keyboard_type, ime_action, ..ImeOptions::default() })
        };
        assert_eq!(scopes(KeyboardType::Unspecified, ImeAction::Default), [IS_DEFAULT]);
        assert_eq!(scopes(KeyboardType::Unspecified, ImeAction::Search), [IS_SEARCH]);
        assert_eq!(scopes(KeyboardType::Text, ImeAction::Search), [IS_SEARCH]);
        assert_eq!(scopes(KeyboardType::Email, ImeAction::Done), [IS_EMAIL_SMTPEMAILADDRESS]);
        assert_eq!(scopes(KeyboardType::Uri, ImeAction::Go), [IS_URL]);
        assert_eq!(scopes(KeyboardType::Number, ImeAction::Done), [IS_DIGITS]);
        assert_eq!(scopes(KeyboardType::Phone, ImeAction::Done), [IS_TELEPHONE_FULLTELEPHONENUMBER]);
        assert_eq!(scopes(KeyboardType::Password, ImeAction::Done), [IS_PASSWORD]);
        assert_eq!(scopes(KeyboardType::NumberPassword, ImeAction::Done), [IS_NUMERIC_PASSWORD]);
        assert_eq!(scopes(KeyboardType::DateTime, ImeAction::Done), [IS_DATE_FULLDATE, IS_TIME_FULLTIME]);
        let options = |keyboard_type| ImeOptions { keyboard_type, ..ImeOptions::default() };
        assert!(hides_text(&options(KeyboardType::Password)));
        assert!(!hides_text(&options(KeyboardType::PasswordVisible)));
        assert!(!hides_text(&options(KeyboardType::Text)));
    }
}
