//! A window's menu bar as KompozR draws it (Compose Desktop's `FrameWindowScope.MenuBar` on Linux, where it is a
//! Swing JMenuBar in the window): its menus across the top of the window below the title bar, each opening below
//! it, a submenu beside its item; Compose Desktop's own look (its context menu's colors: white or near black, a
//! faint tint under the pointer), foundation's pieces only. The pointer: a press on a menu opens it (again: closes
//! it), moving over another while one is open opens that one, over a submenu's item opens the submenu, a click on
//! an item runs it, a press outside the menus closes them. The keyboard (Swing's): Alt and a menu's mnemonic opens
//! it, F10 the first; while one is open the menus have the keyboard — Up and Down move through the items (separators
//! and disabled ones passed over), Right opens a submenu or goes to the next menu, Left closes a submenu or goes to
//! the previous menu, Enter or Space runs the item (opens a submenu), Escape closes a level, an item's mnemonic
//! runs it. The items' shortcuts work in the window whether the menus are open or not (the window's request runs
//! them: [`WindowRequest::menu_bar_key`]).
//!
//! What the bar does with keys is a state machine over the menus ([`MenuNav`]), tested apart from drawing.

use super::MenuBarShown;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{
    BlendMode, Color, Dp, DrawStyle, IntOffset, IntRect, IntSize, LayoutDirection, Offset, Path, RoundedCornerShape,
    Stroke, StrokeCap, StrokeJoin, UnitExt,
};
use kompoz_runtime::{
    DisposableEffect, MutableState, SideEffect, key, kompozable, mutable_state_of, named_args, on_dispose, remember,
};
use kompoz_text::{AnnotatedString, Range, SpanStyle, TextDecoration, TextOverflow, TextStyle};
use kompoz_ui::draw::DrawScope;
use kompoz_ui::input::{
    AwaitPointerEventScope, Key, KeyEvent, KeyEventType, KeyModifiers, PointerEvent, PointerEventType,
};
use kompoz_ui::platform::{LocalDensity, LocalWindowInfo, is_system_in_dark_theme};
use kompoz_ui::semantics::Role;
use kompoz_ui::window::{
    KeyShortcut, MenuItem, MenuItemKind, Popup, PopupPositionProvider, PopupProperties, WindowRequest,
};
use kompoz_ui::{Alignment, Modifier};
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// The bar's `layout_id`: the window's frame puts it below the title bar, above the content.
pub const MENU_BAR_ID: &str = "menuBar";

/// The bar's height, a menu's title's padding across, an item's height and its parts' (dp).
pub const MENU_BAR_HEIGHT: Dp = Dp(28.0);
const TITLE_PADDING: Dp = Dp(10.0);
const ITEM_HEIGHT: Dp = Dp(28.0);
const GUTTER: Dp = Dp(28.0);
const ITEM_END: Dp = Dp(12.0);
const MENU_PADDING: Dp = Dp(4.0);

/// A window's menu bar for its frame: the window's request (its menus, its keys) and who shows the bar (KompozR draws
/// it only where the platform doesn't: [`MenuBarShown::Drawn`]).
#[derive(Clone)]
pub struct MenuBarFrame {
    pub request: Weak<WindowRequest>,
    pub shown: MutableState<MenuBarShown>,
}

/// The bar's colors: Compose Desktop's context menu's (its Light and Dark `DefaultContextMenuRepresentation`), the
/// bar a shade off the menus'.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MenuBarColors {
    pub bar: Color,
    pub menu: Color,
    pub text: Color,
    pub disabled_text: Color,
    /// Under the pointer, or the keyboard's item.
    pub highlight: Color,
    /// The shortcuts' text, the separators.
    pub secondary: Color,
}

impl MenuBarColors {
    /// In the system's theme.
    pub fn of(dark: bool) -> MenuBarColors {
        let base = if dark { DarkDefaultContextMenuRepresentation } else { LightDefaultContextMenuRepresentation };
        let text = base.text_color;
        MenuBarColors {
            bar: if dark { Color::from_argb(0xFF1E1E1E) } else { Color::from_argb(0xFFF2F2F2) },
            menu: base.background_color,
            text,
            disabled_text: base.disabled_text_color,
            highlight: named_args!(text.copy(alpha = 0.10)),
            secondary: named_args!(text.copy(alpha = 0.6)),
        }
    }
}

// ---- The keyboard and the pointer over the menus, apart from drawing ----

/// Where the bar's menus are open: the menu open and the submenus open in it (indexes: the menu's, then its items'),
/// the item highlighted at each open level (none: none yet), and whether the keyboard opened them (then the first
/// item is highlighted). Closed: nothing open.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MenuNav {
    pub open: Vec<usize>,
    pub highlight: Vec<Option<usize>>,
}

/// Whether an item can be highlighted and run (a separator or a disabled one can't).
fn selectable(item: &MenuItem) -> bool {
    item.enabled && item.kind != MenuItemKind::Separator
}

/// The next selectable item from `from` going `step` (1 or -1), round the end; from none: the first (or last).
fn next_selectable(items: &[MenuItem], from: Option<usize>, step: isize) -> Option<usize> {
    let n = items.len() as isize;
    if n == 0 {
        return None;
    }
    let mut i = match from {
        Some(i) => i as isize,
        None if step > 0 => -1,
        None => n,
    };
    for _ in 0..n {
        i = (i + step).rem_euclid(n);
        if selectable(&items[i as usize]) {
            return Some(i as usize);
        }
    }
    None
}

/// Whether `c` is the item's mnemonic (case aside).
fn is_mnemonic(item: &MenuItem, c: char) -> bool {
    item.mnemonic.is_some_and(|m| m.to_lowercase().eq(c.to_lowercase()))
}

/// The letter or digit a key is (its US keyboard's), if one.
fn key_char(key: Key) -> Option<char> {
    char::from_u32(key.typed_code_point(false) as u32).filter(|c| c.is_alphanumeric())
}

/// What a key or the pointer asked of the menus besides moving through them: an item to run (its path).
pub type Run = Option<Vec<usize>>;

impl MenuNav {
    pub fn is_open(&self) -> bool {
        !self.open.is_empty()
    }

    /// Menu `menu` open (closed if it can't be: disabled, none there), its first item highlighted if the keyboard
    /// opened it.
    pub fn opened(menus: &[MenuItem], menu: usize, keyboard: bool) -> MenuNav {
        match menus.get(menu) {
            Some(m) if m.enabled => {
                let first = if keyboard { next_selectable(&m.items, None, 1) } else { None };
                MenuNav { open: vec![menu], highlight: vec![first] }
            }
            _ => MenuNav::default(),
        }
    }

    /// The items of open level `level` (0: the menu's).
    pub fn items_at<'a>(&self, menus: &'a [MenuItem], level: usize) -> &'a [MenuItem] {
        let mut items = match self.open.first().and_then(|&m| menus.get(m)) {
            Some(m) => m.items.as_slice(),
            None => return &[],
        };
        for &i in self.open.iter().skip(1).take(level) {
            items = match items.get(i) {
                Some(sub) => sub.items.as_slice(),
                None => return &[],
            };
        }
        items
    }

    fn deepest(&self) -> usize {
        self.open.len().saturating_sub(1)
    }

    /// The highlighted item's path (the menu's index, the open submenus', its own), if one is.
    fn highlighted_path(&self) -> Option<Vec<usize>> {
        let h = (*self.highlight.last()?)?;
        let mut path = self.open.clone();
        path.push(h);
        Some(path)
    }

    /// The item at `index` of the deepest level run: a submenu opened (its first item highlighted), an item's path
    /// to run (the menus closed).
    fn activate(&self, menus: &[MenuItem], index: usize, keyboard: bool) -> (MenuNav, Run) {
        let items = self.items_at(menus, self.deepest());
        let Some(item) = items.get(index).filter(|i| selectable(i)) else { return (self.clone(), None) };
        let mut next = self.clone();
        *next.highlight.last_mut().expect("open") = Some(index);
        if item.kind == MenuItemKind::Menu {
            next.open.push(index);
            next.highlight.push(if keyboard { next_selectable(&item.items, None, 1) } else { None });
            return (next, None);
        }
        let run = next.highlighted_path();
        (MenuNav::default(), run)
    }

    /// The next (or previous) menu of the bar that can open, round the end, opened by the keyboard.
    fn neighbour(&self, menus: &[MenuItem], step: isize) -> MenuNav {
        let n = menus.len() as isize;
        let mut i = self.open.first().map_or(0, |&m| m as isize);
        for _ in 0..n {
            i = (i + step).rem_euclid(n);
            if menus[i as usize].enabled {
                return MenuNav::opened(menus, i as usize, true);
            }
        }
        self.clone()
    }

    /// A key while the menus are open (they have the keyboard): what they are then, an item to run, and whether it
    /// was taken (all but the modifiers' own keys and the key ups of keys not theirs are).
    pub fn key(&self, menus: &[MenuItem], event: &KeyEvent) -> (MenuNav, Run, bool) {
        let modifier_key = matches!(
            event.key,
            Key::ShiftLeft
                | Key::ShiftRight
                | Key::CtrlLeft
                | Key::CtrlRight
                | Key::AltLeft
                | Key::AltRight
                | Key::MetaLeft
                | Key::MetaRight
        );
        if !self.is_open() || modifier_key {
            return (self.clone(), None, false);
        }
        if event.r#type != KeyEventType::KeyDown {
            return (self.clone(), None, true);
        }
        let level = self.deepest();
        let items = self.items_at(menus, level);
        let h = self.highlight.get(level).copied().flatten();
        let alt = event.modifiers.contains(KeyModifiers::Alt);
        let with_highlight = |h: Option<usize>| {
            let mut next = self.clone();
            next.highlight[level] = h;
            next
        };
        match event.key {
            Key::DirectionDown => (with_highlight(next_selectable(items, h, 1).or(h)), None, true),
            Key::DirectionUp => (with_highlight(next_selectable(items, h, -1).or(h)), None, true),
            Key::DirectionRight => match h.and_then(|i| items.get(i)) {
                Some(item) if item.kind == MenuItemKind::Menu && selectable(item) => {
                    let (next, _) = self.activate(menus, h.expect("highlighted"), true);
                    (next, None, true)
                }
                _ => (self.neighbour(menus, 1), None, true),
            },
            Key::DirectionLeft if self.open.len() > 1 => {
                let mut next = self.clone();
                next.open.pop();
                next.highlight.pop();
                (next, None, true)
            }
            Key::DirectionLeft => (self.neighbour(menus, -1), None, true),
            Key::Enter | Key::NumPadEnter | Key::Spacebar => match h {
                Some(i) => {
                    let (next, run) = self.activate(menus, i, true);
                    (next, run, true)
                }
                None => (self.clone(), None, true),
            },
            Key::Escape if self.open.len() > 1 => {
                let mut next = self.clone();
                next.open.pop();
                next.highlight.pop();
                (next, None, true)
            }
            Key::Escape | Key::F10 => (MenuNav::default(), None, true),
            k => match key_char(k) {
                // Alt and a menu's mnemonic: that menu (Swing's, also while another is open).
                Some(c) if alt => match menus.iter().position(|m| m.enabled && is_mnemonic(m, c)) {
                    Some(m) => (MenuNav::opened(menus, m, true), None, true),
                    None => (self.clone(), None, true),
                },
                Some(c) => match items.iter().position(|i| selectable(i) && is_mnemonic(i, c)) {
                    Some(i) => {
                        let (next, run) = self.activate(menus, i, true);
                        (next, run, true)
                    }
                    None => (self.clone(), None, true),
                },
                None => (self.clone(), None, true),
            },
        }
    }

    /// A key while the menus are closed, the window's focused path having left it: Alt and a menu's mnemonic opens
    /// that menu, F10 the first that can open. None: not the bar's.
    pub fn closed_key(menus: &[MenuItem], event: &KeyEvent) -> Option<MenuNav> {
        if event.r#type != KeyEventType::KeyDown {
            return None;
        }
        let m = event.modifiers;
        let plain =
            !m.contains(KeyModifiers::Ctrl) && !m.contains(KeyModifiers::Meta) && !m.contains(KeyModifiers::Shift);
        if event.key == Key::F10 && plain && !m.contains(KeyModifiers::Alt) {
            let first = menus.iter().position(|m| m.enabled)?;
            return Some(MenuNav::opened(menus, first, true));
        }
        if plain && m.contains(KeyModifiers::Alt) {
            let c = key_char(event.key)?;
            let menu = menus.iter().position(|menu| menu.enabled && is_mnemonic(menu, c))?;
            return Some(MenuNav::opened(menus, menu, true));
        }
        None
    }

    /// The pointer over item `index` of open level `level`: it highlighted, the deeper levels closed, a submenu's
    /// opened.
    pub fn hovered(&self, menus: &[MenuItem], level: usize, index: usize) -> MenuNav {
        if level >= self.open.len() {
            return self.clone();
        }
        let mut next = self.clone();
        next.open.truncate(level + 1);
        next.highlight.truncate(level + 1);
        let items = next.items_at(menus, level);
        match items.get(index) {
            Some(item) if selectable(item) => {
                next.highlight[level] = Some(index);
                if item.kind == MenuItemKind::Menu {
                    next.open.push(index);
                    next.highlight.push(None);
                }
            }
            _ => next.highlight[level] = None,
        }
        next
    }

    /// Item `index` of open level `level` clicked: a submenu opened, an item's path to run (the menus closed).
    pub fn clicked(&self, menus: &[MenuItem], level: usize, index: usize) -> (MenuNav, Run) {
        let mut at = self.hovered(menus, level, index);
        if at.open.len() > level + 1 {
            return (at, None); // (a submenu: open)
        }
        at.open.truncate(level + 1);
        at.highlight.truncate(level + 1);
        at.activate(menus, index, false)
    }
}

/// A shortcut as the menus show it: the modifiers, then the key ("Ctrl+Shift+S", "Alt+F4").
pub fn shortcut_label(s: &KeyShortcut) -> String {
    let mut out = String::new();
    for (on, name) in [(s.ctrl, "Ctrl"), (s.alt, "Alt"), (s.shift, "Shift"), (s.meta, "Meta")] {
        if on {
            out.push_str(name);
            out.push('+');
        }
    }
    let named = [
        (Key::Enter, "Enter"),
        (Key::Escape, "Esc"),
        (Key::Tab, "Tab"),
        (Key::Spacebar, "Space"),
        (Key::Backspace, "Backspace"),
        (Key::Delete, "Delete"),
        (Key::Insert, "Insert"),
        (Key::MoveHome, "Home"),
        (Key::MoveEnd, "End"),
        (Key::PageUp, "PgUp"),
        (Key::PageDown, "PgDown"),
        (Key::DirectionUp, "Up"),
        (Key::DirectionDown, "Down"),
        (Key::DirectionLeft, "Left"),
        (Key::DirectionRight, "Right"),
        (Key::Plus, "+"),
        (Key::Minus, "-"),
        (Key::Comma, ","),
        (Key::Period, "."),
    ];
    if let Some((_, name)) = named.iter().find(|(k, _)| *k == s.key) {
        out.push_str(name);
    } else if let Some(i) = crate::keys::FUNCTION_KEYS.iter().position(|f| *f == s.key) {
        out.push_str(&format!("F{}", i + 1));
    } else if let Some(c) = char::from_u32(s.key.typed_code_point(false) as u32).filter(|c| *c != '\0') {
        out.extend(c.to_uppercase());
    }
    out
}

/// `text` with its mnemonic's first occurrence underlined (Swing's).
fn mnemonic_text(text: &str, mnemonic: Option<char>) -> AnnotatedString {
    let at = mnemonic.and_then(|m| text.char_indices().find(|(_, c)| c.to_lowercase().eq(m.to_lowercase())));
    match at {
        Some((i, c)) => AnnotatedString::with_styles(
            text.to_string(),
            vec![Range::new(
                SpanStyle { text_decoration: Some(TextDecoration::Underline), ..SpanStyle::default() },
                i,
                i + c.len_utf8(),
            )],
            Vec::new(),
        ),
        None => AnnotatedString::new(text.to_string()),
    }
}

fn label_style(color: Color) -> TextStyle {
    TextStyle { color, font_size: 14.sp(), ..TextStyle::Default }
}

// ---- Where the menus go ----

/// A menu below its title on the bar, its left at the title's, kept in the window.
struct BelowTitle;

impl PopupPositionProvider for BelowTitle {
    fn calculate_position(&self, anchor: IntRect, window: IntSize, _: LayoutDirection, size: IntSize) -> IntOffset {
        let x = anchor.left.min(window.width - size.width).max(0);
        let y = anchor.bottom.min(window.height - size.height).max(0);
        IntOffset::new(x, y)
    }
}

/// A submenu beside its item (its first item level with it: `padding` px above), on the left where the right has
/// no room, kept in the window.
struct BesideItem {
    padding: i32,
}

impl PopupPositionProvider for BesideItem {
    fn calculate_position(&self, anchor: IntRect, window: IntSize, _: LayoutDirection, size: IntSize) -> IntOffset {
        let x = if anchor.right + size.width <= window.width { anchor.right } else { anchor.left - size.width };
        let y = (anchor.top - self.padding).min(window.height - size.height).max(0);
        IntOffset::new(x.max(0), y)
    }
}

/// What is below the bar, the whole window's width: a press there closes the menus.
struct BelowBar;

impl PopupPositionProvider for BelowBar {
    fn calculate_position(&self, anchor: IntRect, _: IntSize, _: LayoutDirection, _: IntSize) -> IntOffset {
        IntOffset::new(0, anchor.bottom)
    }
}

// ---- Drawing ----

/// The state the bar's pieces share: its menus as shown, where they are open, the window's request (an item run
/// through it: its callbacks the latest), the colors.
#[derive(Clone)]
struct Bar {
    menus: Rc<Vec<MenuItem>>,
    nav: MutableState<MenuNav>,
    request: Weak<WindowRequest>,
    colors: MenuBarColors,
}

impl Bar {
    /// What the pointer or a key asked: the menus where they are now, an item run.
    fn go(&self, (next, run): (MenuNav, Run)) {
        if self.nav.peek() != next {
            self.nav.set(next);
        }
        if let Some(path) = run
            && let Some(request) = self.request.upgrade()
        {
            request.click_menu_bar_item(&path);
        }
    }
}

/// The window's menu bar, its menus `menus`: drawn while the platform leaves it to KompozR (its keys given to the
/// window's request meanwhile), nothing otherwise.
#[kompozable]
pub fn MenuBarView(frame: MenuBarFrame) {
    let Some(request) = frame.request.upgrade() else { return };
    let shown = frame.shown.get();
    let menus = request.menu_bar();
    let nav = remember(|| mutable_state_of(MenuNav::default()));
    let drawn = shown == MenuBarShown::Drawn && menus.as_ref().is_some_and(|m| !m.is_empty());
    {
        // The window's keys: the open menus' first (they have the keyboard), Alt and a mnemonic, F10 after the
        // focused path; none while the bar isn't drawn.
        let (nav, weak) = (nav.clone(), frame.request.clone());
        SideEffect(move || {
            let Some(request) = weak.upgrade() else { return };
            if !drawn {
                nav.set(MenuNav::default());
                request.set_menu_bar_keys(None);
                return;
            }
            let (preview_nav, preview_request) = (nav.clone(), weak.clone());
            let preview = Rc::new(move |e: &KeyEvent| {
                let now = preview_nav.peek();
                let Some(request) = preview_request.upgrade() else { return false };
                let Some(menus) = request.menu_bar_menus() else { return false };
                let (next, run, taken) = now.key(&menus, e);
                if next != now {
                    preview_nav.set(next);
                }
                if let Some(path) = run {
                    request.click_menu_bar_item(&path);
                }
                taken
            });
            let after_nav = nav.clone();
            let after_request = weak.clone();
            let after = Rc::new(move |e: &KeyEvent| {
                let Some(request) = after_request.upgrade() else { return false };
                let Some(menus) = request.menu_bar_menus() else { return false };
                match MenuNav::closed_key(&menus, e) {
                    Some(next) => {
                        after_nav.set(next);
                        true
                    }
                    None => false,
                }
            });
            request.set_menu_bar_keys(Some((preview, after)));
        });
    }
    {
        let weak = frame.request.clone();
        DisposableEffect((), move || {
            on_dispose(move || {
                if let Some(request) = weak.upgrade() {
                    request.set_menu_bar_keys(None);
                }
            })
        });
    }
    let Some(menus) = menus.filter(|_| drawn) else { return };
    // (Closed as the window loses the focus, Swing's.)
    if !LocalWindowInfo.current().is_window_focused() && nav.peek().is_open() {
        let nav = nav.clone();
        SideEffect(move || nav.set(MenuNav::default()));
    }
    let bar = Bar { menus, nav, request: frame.request.clone(), colors: MenuBarColors::of(is_system_in_dark_theme()) };
    let open = bar.nav.get();
    let row = bar.clone();
    Row(
        modifier = Modifier
            .layout_id(MENU_BAR_ID)
            .fill_max_width()
            .height(MENU_BAR_HEIGHT)
            .background(bar.colors.bar)
            .padding_hv(horizontal = Dp(4.0)),
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            for i in 0..row.menus.len() {
                let bar = row.clone();
                key(&(i as i32), move || MenuTitle(bar, i));
            }
        },
    );
    if open.is_open() {
        // A press below the bar, outside the menus: they close (the press taken, Swing's).
        let closer = bar.nav.clone();
        Popup(
            popup_position_provider = Some(Rc::new(BelowBar) as Rc<dyn PopupPositionProvider>),
            properties = PopupProperties {
                clipping_enabled: false,
                dismiss_on_click_outside: false,
                ..PopupProperties::default()
            },
            content = move || {
                let closer = closer.clone();
                Box(modifier = Modifier.fill_max_size().on_pointer_event(
                    PointerEventType::Press,
                    move |_: &AwaitPointerEventScope, e: &PointerEvent| {
                        closer.set(MenuNav::default());
                        for c in &e.changes {
                            c.consume();
                        }
                    },
                ));
            },
        );
    }
}

/// Menu `index`'s title on the bar: its text (its mnemonic underlined), tinted while it is open or under the
/// pointer; its menu below it while it is open.
#[kompozable]
fn MenuTitle(bar: Bar, index: usize) {
    let menu = bar.menus[index].clone();
    let nav = bar.nav.get();
    let is_open = nav.open.first() == Some(&index);
    let hovered = remember(|| mutable_state_of(false));
    let colors = bar.colors;
    let enabled = menu.enabled;
    let (press, enter, exit) = (bar.clone(), bar.clone(), hovered.clone());
    let entered = hovered.clone();
    let clicked = bar.clone();
    let tint = if is_open || (hovered.get() && enabled) { colors.highlight } else { Color::Transparent };
    let text = menu.text.clone();
    Box(
        modifier = Modifier
            .height(Dp(MENU_BAR_HEIGHT.0 - 4.0))
            .background(tint, Arc::new(RoundedCornerShape(Dp(4.0))))
            .on_pointer_event(PointerEventType::Press, move |_: &AwaitPointerEventScope, e: &PointerEvent| {
                let now = press.nav.peek();
                let next = if now.open.first() == Some(&index) {
                    MenuNav::default()
                } else {
                    MenuNav::opened(&press.menus, index, false)
                };
                press.go((next, None));
                for c in &e.changes {
                    c.consume();
                }
            })
            .on_pointer_event(PointerEventType::Enter, move |_: &AwaitPointerEventScope, _: &PointerEvent| {
                entered.set(true);
                let now = enter.nav.peek();
                if now.is_open() && now.open.first() != Some(&index) {
                    enter.go((MenuNav::opened(&enter.menus, index, false), None));
                }
            })
            .on_pointer_event(PointerEventType::Exit, move |_: &AwaitPointerEventScope, _: &PointerEvent| {
                exit.set(false);
            })
            .semantics_with(true, move |c| {
                c.set_role(Role::DropdownList);
                c.set_content_description(text.clone());
                if !enabled {
                    c.disabled();
                }
                let bar = clicked.clone();
                c.on_click(None, move || {
                    let now = bar.nav.peek();
                    let next = if now.open.first() == Some(&index) {
                        MenuNav::default()
                    } else {
                        MenuNav::opened(&bar.menus, index, true)
                    };
                    bar.go((next, None));
                    true
                });
            })
            .padding_hv(horizontal = TITLE_PADDING),
        content_alignment = Alignment::Center,
        content = move || {
            BasicText(
                mnemonic_text(&menu.text, menu.mnemonic),
                style = label_style(if enabled { colors.text } else { colors.disabled_text }),
                max_lines = 1,
            );
        },
    );
    if is_open {
        let bar = bar.clone();
        Popup(
            popup_position_provider = Some(Rc::new(BelowTitle) as Rc<dyn PopupPositionProvider>),
            properties = PopupProperties { dismiss_on_click_outside: false, ..PopupProperties::default() },
            content = move || MenuPanel(bar.clone(), 0),
        );
    }
}

/// Open level `level`'s items in a panel (a shadow under it, the menus' color).
#[kompozable]
fn MenuPanel(bar: Bar, level: usize) {
    let nav = bar.nav.get();
    let count = nav.items_at(&bar.menus, level).len();
    let colors = bar.colors;
    Column(
        modifier = Modifier
            .shadow(Dp(8.0))
            .background(colors.menu, Arc::new(RoundedCornerShape(Dp(4.0))))
            .padding_hv(vertical = MENU_PADDING)
            .width_intrinsic(IntrinsicSize::Max)
            .width_in(min = Dp(160.0)),
        content = move || {
            for i in 0..count {
                let bar = bar.clone();
                key(&(i as i32), move || MenuRow(bar, level, i));
            }
        },
    );
}

/// Item `index` of open level `level`: a separator's line, or its check, radio dot or icon, its text (its mnemonic
/// underlined), its shortcut, a submenu's arrow; tinted while highlighted; a submenu beside it while open.
#[kompozable]
fn MenuRow(bar: Bar, level: usize, index: usize) {
    let nav = bar.nav.get();
    let Some(item) = nav.items_at(&bar.menus, level).get(index).cloned() else { return };
    let colors = bar.colors;
    if item.kind == MenuItemKind::Separator {
        Box(modifier = Modifier
            .padding_hv(vertical = Dp(4.0))
            .fill_max_width()
            .height(Dp(1.0))
            .background(colors.secondary.copy(alpha = 0.2)));
        return;
    }
    let highlighted = nav.highlight.get(level).copied().flatten() == Some(index);
    let sub_open = highlighted && nav.open.len() > level + 1;
    let enabled = item.enabled;
    let text_color = if enabled { colors.text } else { colors.disabled_text };
    let (enter, click) = (bar.clone(), bar.clone());
    let semantic = bar.clone();
    let label = item.text.clone();
    let kind = item.kind;
    Row(
        modifier = Modifier
            .fill_max_width()
            .height(ITEM_HEIGHT)
            .background(if highlighted && enabled { colors.highlight } else { Color::Transparent })
            .on_pointer_event(PointerEventType::Enter, move |_: &AwaitPointerEventScope, _: &PointerEvent| {
                let now = enter.nav.peek();
                enter.go((now.hovered(&enter.menus, level, index), None));
            })
            .on_pointer_event(PointerEventType::Release, move |_: &AwaitPointerEventScope, e: &PointerEvent| {
                let now = click.nav.peek();
                click.go(now.clicked(&click.menus, level, index));
                for c in &e.changes {
                    c.consume();
                }
            })
            .semantics_with(true, move |c| {
                c.set_content_description(label.clone());
                match kind {
                    MenuItemKind::Checkbox { checked } => {
                        c.set_role(Role::Checkbox);
                        c.set_toggleable_state(checked.into());
                    }
                    MenuItemKind::RadioButton { selected } => {
                        c.set_role(Role::RadioButton);
                        c.set_selected(selected);
                    }
                    MenuItemKind::Menu => c.set_role(Role::DropdownList),
                    _ => c.set_role(Role::Button),
                }
                if !enabled {
                    c.disabled();
                }
                let bar = semantic.clone();
                c.on_click(None, move || {
                    let now = bar.nav.peek();
                    bar.go(now.clicked(&bar.menus, level, index));
                    true
                });
            }),
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            // Its mark (a check, a radio dot) or icon, in the gutter.
            let mark = item.clone();
            Box(
                modifier = Modifier.width(GUTTER).fill_max_height(),
                content_alignment = Alignment::Center,
                content = move || match (mark.kind, &mark.icon) {
                    (MenuItemKind::Checkbox { checked: true }, _) => {
                        Canvas(Modifier.size(Dp(14.0)), on_draw = move |s: &mut DrawScope| draw_check(s, text_color));
                    }
                    (MenuItemKind::RadioButton { selected: true }, _) => {
                        Canvas(Modifier.size(Dp(8.0)), on_draw = move |s: &mut DrawScope| draw_dot(s, text_color));
                    }
                    (_, Some(icon)) => Image(icon.clone(), None, modifier = Modifier.size(Dp(16.0))),
                    _ => {}
                },
            );
            BasicText(
                mnemonic_text(&item.text, item.mnemonic),
                modifier = Modifier.weight(1.0, fill = false),
                style = label_style(text_color),
                overflow = TextOverflow::Ellipsis,
                max_lines = 1,
            );
            Spacer(Modifier.weight(1.0).width_in(min = Dp(24.0)));
            if let Some(s) = &item.shortcut {
                BasicText(
                    shortcut_label(s),
                    style = label_style(if enabled { colors.secondary } else { colors.disabled_text }),
                    max_lines = 1,
                );
            }
            Box(
                modifier = Modifier.width(if item.kind == MenuItemKind::Menu { Dp(24.0) } else { ITEM_END }),
                content_alignment = Alignment::Center,
                content = move || {
                    if kind == MenuItemKind::Menu {
                        Canvas(Modifier.size(Dp(8.0)), on_draw = move |s: &mut DrawScope| draw_arrow(s, text_color));
                    }
                },
            );
        },
    );
    if sub_open {
        let padding = LocalDensity.current().round_to_px(MENU_PADDING);
        let bar = bar.clone();
        Popup(
            popup_position_provider = Some(Rc::new(BesideItem { padding }) as Rc<dyn PopupPositionProvider>),
            properties = PopupProperties { dismiss_on_click_outside: false, ..PopupProperties::default() },
            content = move || MenuPanel(bar.clone(), level + 1),
        );
    }
}

/// A check mark in the canvas's square.
fn draw_check(s: &mut DrawScope, color: Color) {
    let (w, h) = (s.size().width, s.size().height);
    let mut path = Path::new();
    path.move_to(w * 0.12, h * 0.52);
    path.line_to(w * 0.40, h * 0.80);
    path.line_to(w * 0.90, h * 0.20);
    let stroke = Stroke::new(w * 0.14, Stroke::DefaultMiter, StrokeCap::Round, StrokeJoin::Round, None);
    s.draw_path_with(&path, color, 1.0, DrawStyle::Stroke(stroke), None, BlendMode::SrcOver);
}

/// A radio button's dot filling the canvas.
fn draw_dot(s: &mut DrawScope, color: Color) {
    let size = s.size();
    let center = Offset::new(size.width / 2.0, size.height / 2.0);
    named_args!(s.draw_circle(color, radius = size.width.min(size.height) / 2.0, center = center));
}

/// A submenu's arrow, pointing right, filling the canvas.
fn draw_arrow(s: &mut DrawScope, color: Color) {
    let (w, h) = (s.size().width, s.size().height);
    let mut path = Path::new();
    path.move_to(w * 0.25, 0.0);
    path.line_to(w * 0.85, h / 2.0);
    path.line_to(w * 0.25, h);
    path.close();
    s.draw_path_with(&path, color, 1.0, DrawStyle::Fill, None, BlendMode::SrcOver);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_ui::window::{KeyShortcut, MenuBarScope, MenuScope};

    fn menus() -> Vec<MenuItem> {
        let bar = MenuBarScope::new();
        named_args!(bar.Menu(
            text = "File".into(),
            mnemonic = Some('F'),
            content = |m: &MenuScope| {
                named_args!(m.Item(text = "New".into(), mnemonic = Some('N'), on_click = || {}));
                m.Separator();
                named_args!(m.Item(text = "Disabled".into(), enabled = false, on_click = || {}));
                named_args!(m.Menu(
                    text = "Recent".into(),
                    mnemonic = Some('R'),
                    content = |r: &MenuScope| {
                        named_args!(r.Item(text = "a.txt".into(), on_click = || {}));
                        named_args!(r.Item(text = "b.txt".into(), on_click = || {}));
                    }
                ));
                named_args!(m.Item(text = "Quit".into(), mnemonic = Some('Q'), on_click = || {}));
            }
        ));
        named_args!(bar.Menu(
            text = "Edit".into(),
            mnemonic = Some('E'),
            content = |m: &MenuScope| {
                named_args!(m.Item(text = "Undo".into(), mnemonic = Some('U'), on_click = || {}));
            }
        ));
        named_args!(bar.Menu(text = "Off".into(), enabled = false, content = |_: &MenuScope| {}));
        bar.take_menus()
    }

    fn down(key: Key, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(key, KeyEventType::KeyDown, modifiers)
    }

    #[test]
    fn the_keyboard_opens_and_walks_the_menus() {
        let m = menus();
        let none = KeyModifiers::None;
        // Alt+F: File open, its first item highlighted; F10 the same.
        let nav = MenuNav::closed_key(&m, &down(Key::F, KeyModifiers::Alt)).expect("Alt+F");
        assert_eq!(nav, MenuNav { open: vec![0], highlight: vec![Some(0)] });
        assert_eq!(MenuNav::closed_key(&m, &down(Key::F10, none)), Some(nav.clone()));
        assert_eq!(MenuNav::closed_key(&m, &down(Key::F, none)), None, "F alone is the window's");
        assert_eq!(MenuNav::closed_key(&m, &down(Key::F, KeyModifiers::Ctrl | KeyModifiers::Alt)), None);
        // Down passes the separator and the disabled item, to the submenu; Right opens it.
        let (nav, run, taken) = nav.key(&m, &down(Key::DirectionDown, none));
        assert!(taken && run.is_none());
        assert_eq!(nav.highlight, [Some(3)]);
        let (nav, _, _) = nav.key(&m, &down(Key::DirectionRight, none));
        assert_eq!(nav, MenuNav { open: vec![0, 3], highlight: vec![Some(3), Some(0)] });
        // Down, Enter: b.txt runs, the menus close.
        let (nav, _, _) = nav.key(&m, &down(Key::DirectionDown, none));
        let (nav, run, _) = nav.key(&m, &down(Key::Enter, none));
        assert_eq!(run, Some(vec![0, 3, 1]));
        assert!(!nav.is_open());
        // Up from the first item goes round to the last; Left in a menu goes to the previous menu (round the end,
        // past the disabled one); Right on an item to the next.
        let nav = MenuNav::opened(&m, 0, true);
        let (up, _, _) = nav.key(&m, &down(Key::DirectionUp, none));
        assert_eq!(up.highlight, [Some(4)]);
        let (left, _, _) = nav.key(&m, &down(Key::DirectionLeft, none));
        assert_eq!(left, MenuNav { open: vec![1], highlight: vec![Some(0)] });
        let (right, _, _) = left.key(&m, &down(Key::DirectionRight, none));
        assert_eq!(right.open, [0]);
        // A mnemonic runs its item; Escape closes a level, then the menu; Alt+E another menu.
        let (_, run, _) = nav.key(&m, &down(Key::Q, none));
        assert_eq!(run, Some(vec![0, 4]));
        let sub = MenuNav { open: vec![0, 3], highlight: vec![Some(3), Some(0)] };
        let (back, _, _) = sub.key(&m, &down(Key::Escape, none));
        assert_eq!(back, MenuNav { open: vec![0], highlight: vec![Some(3)] });
        let (closed, _, taken) = back.key(&m, &down(Key::Escape, none));
        assert!(taken && !closed.is_open());
        let (edit, _, _) = nav.key(&m, &down(Key::E, KeyModifiers::Alt));
        assert_eq!(edit.open, [1]);
        // Open, the menus take the keys (a letter that is no mnemonic too); a modifier's own key goes on.
        assert!(nav.key(&m, &down(Key::Z, none)).2);
        assert!(!nav.key(&m, &down(Key::ShiftLeft, none)).2);
        assert!(!MenuNav::default().key(&m, &down(Key::DirectionDown, none)).2, "closed: not theirs");
        // A disabled menu doesn't open.
        assert!(!MenuNav::opened(&m, 2, true).is_open());
    }

    #[test]
    fn the_pointer_opens_submenus_and_runs_items() {
        let m = menus();
        let nav = MenuNav::opened(&m, 0, false);
        assert_eq!(nav.highlight, [None]);
        let over = nav.hovered(&m, 0, 3);
        assert_eq!(over, MenuNav { open: vec![0, 3], highlight: vec![Some(3), None] });
        let back = over.hovered(&m, 0, 0);
        assert_eq!(back, MenuNav { open: vec![0], highlight: vec![Some(0)] });
        assert_eq!(back.hovered(&m, 0, 1).highlight, [None], "a separator isn't highlighted");
        let (after, run) = over.clicked(&m, 1, 0);
        assert_eq!(run, Some(vec![0, 3, 0]));
        assert!(!after.is_open());
        let (still, run) = nav.clicked(&m, 0, 2);
        assert!(run.is_none() && still.is_open(), "a disabled item does nothing");
        let (sub, run) = nav.clicked(&m, 0, 3);
        assert!(run.is_none() && sub.open == [0, 3]);
    }

    #[test]
    fn shortcuts_and_mnemonics_are_shown() {
        let s = KeyShortcut { key: Key::S, ctrl: true, meta: false, alt: false, shift: true };
        assert_eq!(shortcut_label(&s), "Ctrl+Shift+S");
        assert_eq!(
            shortcut_label(&KeyShortcut { key: Key::F4, ctrl: false, meta: false, alt: true, shift: false }),
            "Alt+F4"
        );
        assert_eq!(
            shortcut_label(&KeyShortcut { key: Key::Delete, ctrl: false, meta: false, alt: false, shift: false }),
            "Delete"
        );
        let t = mnemonic_text("Save as", Some('a'));
        assert_eq!(t.span_styles().len(), 1);
        assert_eq!((t.span_styles()[0].start, t.span_styles()[0].end), (1, 2));
        assert!(mnemonic_text("Quit", Some('x')).span_styles().is_empty());
    }
}
