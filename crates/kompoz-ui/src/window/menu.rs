//! Menus of the desktop's own (Compose Desktop's Menu.desktop.kt: a tray icon's menu, a window's menu bar): items
//! built as the composable that takes them composes ([`MenuScope`]; a menu bar's menus [`MenuBarScope`]), shown by
//! the platform (Linux's dbusmenu, a global menu) or drawn by KompozR (a menu bar in its window), their clicks called
//! back on the app's thread; the items' shortcuts found for a key ([`shortcut_item`]: a menu bar's accelerators).

use crate::draw::Painter;
use crate::input::Key;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// A key combination shown beside a menu item (Compose Desktop's KeyShortcut): its key and the modifiers held.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyShortcut {
    pub key: Key,
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Compose Desktop's `KeyShortcut(key, ctrl = false, meta = false, alt = false, shift = false)`.
#[kompoz_runtime::named(type)]
pub fn KeyShortcut(
    key: Key,
    #[default(false)] ctrl: bool,
    #[default(false)] meta: bool,
    #[default(false)] alt: bool,
    #[default(false)] shift: bool,
) -> KeyShortcut {
    KeyShortcut { key, ctrl, meta, alt, shift }
}

/// What kind of item a menu item is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuItemKind {
    /// Clicked.
    Item,
    /// Checked or not, toggled as it is clicked.
    Checkbox { checked: bool },
    /// Selected or not, one of a group.
    RadioButton { selected: bool },
    /// A line between items.
    Separator,
    /// A submenu: its items.
    Menu,
}

/// An item of a menu: what it shows, and what its click does.
#[derive(Clone)]
pub struct MenuItem {
    pub kind: MenuItemKind,
    pub text: String,
    pub icon: Option<Rc<dyn Painter>>,
    pub enabled: bool,
    /// The letter that picks it from the keyboard (shown underlined).
    pub mnemonic: Option<char>,
    pub shortcut: Option<KeyShortcut>,
    /// A submenu's items.
    pub items: Vec<MenuItem>,
    on_click: Option<Rc<dyn Fn()>>,
    on_checked_change: Option<Rc<dyn Fn(bool)>>,
}

impl MenuItem {
    fn new(kind: MenuItemKind, text: String) -> MenuItem {
        MenuItem {
            kind,
            text,
            icon: None,
            enabled: true,
            mnemonic: None,
            shortcut: None,
            items: Vec::new(),
            on_click: None,
            on_checked_change: None,
        }
    }

    /// It was clicked: its `on_click`, or a checkbox's `on_checked_change` with what it toggles to.
    pub fn click(&self) {
        if !self.enabled {
            return;
        }
        match self.kind {
            MenuItemKind::Checkbox { checked } => {
                if let Some(f) = &self.on_checked_change {
                    f(!checked);
                }
            }
            _ => {
                if let Some(f) = &self.on_click {
                    f();
                }
            }
        }
    }

    /// Whether it shows what `other` shows (its callbacks aside): what a platform needn't show again.
    pub fn looks_like(&self, other: &MenuItem) -> bool {
        let icons = match (&self.icon, &other.icon) {
            (None, None) => true,
            (Some(a), Some(b)) => a.painter_eq(&**b),
            _ => false,
        };
        icons
            && self.kind == other.kind
            && self.text == other.text
            && self.enabled == other.enabled
            && self.mnemonic == other.mnemonic
            && self.shortcut == other.shortcut
            && same_items(&self.items, &other.items)
    }
}

impl fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MenuItem")
            .field("kind", &self.kind)
            .field("text", &self.text)
            .field("enabled", &self.enabled)
            .field("mnemonic", &self.mnemonic)
            .field("shortcut", &self.shortcut)
            .field("items", &self.items)
            .finish()
    }
}

/// Whether two menus show the same (their callbacks aside).
pub fn same_items(a: &[MenuItem], b: &[MenuItem]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.looks_like(y))
}

/// The items a menu's content makes (Compose Desktop's MenuScope), in order; a submenu's in its own scope.
#[derive(Default)]
pub struct MenuScope {
    items: RefCell<Vec<MenuItem>>,
}

impl MenuScope {
    pub fn new() -> MenuScope {
        MenuScope::default()
    }

    /// The items made, taken.
    pub fn take_items(&self) -> Vec<MenuItem> {
        std::mem::take(&mut *self.items.borrow_mut())
    }

    fn push(&self, item: MenuItem) {
        self.items.borrow_mut().push(item);
    }
}

/// The menus a window's menu bar's content makes (Compose Desktop's MenuBarScope), in order, each a submenu item.
#[derive(Default)]
pub struct MenuBarScope {
    menus: RefCell<Vec<MenuItem>>,
}

impl MenuBarScope {
    pub fn new() -> MenuBarScope {
        MenuBarScope::default()
    }

    /// The menus made, taken.
    pub fn take_menus(&self) -> Vec<MenuItem> {
        std::mem::take(&mut *self.menus.borrow_mut())
    }
}

#[kompoz_runtime::named]
impl MenuBarScope {
    /// A menu of the bar (Compose Desktop's `MenuBarScope.Menu`): `text` on the bar, `mnemonic` (Alt and it opens
    /// the menu; its first occurrence in `text` underlined), `content` making its items.
    pub fn Menu(
        &self,
        text: String,
        #[default(None)] mnemonic: Option<char>,
        #[default(true)] enabled: bool,
        content: impl Fn(&MenuScope) + 'static,
    ) {
        let sub = MenuScope::new();
        content(&sub);
        let mut item = MenuItem::new(MenuItemKind::Menu, text);
        item.enabled = enabled;
        item.mnemonic = mnemonic;
        item.items = sub.take_items();
        self.menus.borrow_mut().push(item);
    }
}

/// The item at `path` in `items` (an index at each level: a submenu's items below it).
pub fn item_at(items: &[MenuItem], path: &[usize]) -> Option<MenuItem> {
    let (&first, rest) = path.split_first()?;
    let item = items.get(first)?;
    if rest.is_empty() { Some(item.clone()) } else { item_at(&item.items, rest) }
}

/// Whether `shortcut` is what `event` pressed: its key with exactly its modifiers (Swing's KeyStroke, as Compose
/// Desktop's menus' accelerators match).
pub fn shortcut_matches(shortcut: &KeyShortcut, event: &crate::input::KeyEvent) -> bool {
    use crate::input::KeyModifiers;
    let m = event.modifiers;
    event.r#type == crate::input::KeyEventType::KeyDown
        && event.key == shortcut.key
        && m.contains(KeyModifiers::Ctrl) == shortcut.ctrl
        && m.contains(KeyModifiers::Meta) == shortcut.meta
        && m.contains(KeyModifiers::Alt) == shortcut.alt
        && m.contains(KeyModifiers::Shift) == shortcut.shift
}

/// The enabled item (not a submenu) whose shortcut `event` presses, in `items` and their enabled submenus, first
/// first; its path.
pub fn shortcut_item(items: &[MenuItem], event: &crate::input::KeyEvent) -> Option<Vec<usize>> {
    for (i, item) in items.iter().enumerate() {
        if !item.enabled {
            continue;
        }
        if item.kind == MenuItemKind::Menu {
            if let Some(mut path) = shortcut_item(&item.items, event) {
                path.insert(0, i);
                return Some(path);
            }
        } else if item.shortcut.as_ref().is_some_and(|s| shortcut_matches(s, event)) {
            return Some(vec![i]);
        }
    }
    None
}

#[kompoz_runtime::named]
impl MenuScope {
    /// A submenu (Compose Desktop's `MenuScope.Menu`): `content` makes its items.
    pub fn Menu(
        &self,
        text: String,
        #[default(true)] enabled: bool,
        #[default(None)] mnemonic: Option<char>,
        content: impl Fn(&MenuScope) + 'static,
    ) {
        let sub = MenuScope::new();
        content(&sub);
        let mut item = MenuItem::new(MenuItemKind::Menu, text);
        item.enabled = enabled;
        item.mnemonic = mnemonic;
        item.items = sub.take_items();
        self.push(item);
    }

    /// A line between items (Compose Desktop's `MenuScope.Separator`).
    pub fn Separator(&self) {
        self.push(MenuItem::new(MenuItemKind::Separator, String::new()));
    }

    /// An item (Compose Desktop's `MenuScope.Item`): `on_click` as it is clicked.
    pub fn Item(
        &self,
        text: String,
        #[default(None)] icon: Option<Rc<dyn Painter>>,
        #[default(true)] enabled: bool,
        #[default(None)] mnemonic: Option<char>,
        #[default(None)] shortcut: Option<KeyShortcut>,
        on_click: impl Fn() + 'static,
    ) {
        let mut item = MenuItem::new(MenuItemKind::Item, text);
        (item.icon, item.enabled, item.mnemonic, item.shortcut) = (icon, enabled, mnemonic, shortcut);
        item.on_click = Some(Rc::new(on_click));
        self.push(item);
    }

    /// An item checked or not (Compose Desktop's `MenuScope.CheckboxItem`): `on_checked_change` with what a click
    /// toggles it to.
    #[allow(clippy::too_many_arguments)]
    pub fn CheckboxItem(
        &self,
        text: String,
        checked: bool,
        #[default(None)] icon: Option<Rc<dyn Painter>>,
        #[default(true)] enabled: bool,
        #[default(None)] mnemonic: Option<char>,
        #[default(None)] shortcut: Option<KeyShortcut>,
        on_checked_change: impl Fn(bool) + 'static,
    ) {
        let mut item = MenuItem::new(MenuItemKind::Checkbox { checked }, text);
        (item.icon, item.enabled, item.mnemonic, item.shortcut) = (icon, enabled, mnemonic, shortcut);
        item.on_checked_change = Some(Rc::new(on_checked_change));
        self.push(item);
    }

    /// An item of a group, selected or not (Compose Desktop's `MenuScope.RadioButtonItem`).
    #[allow(clippy::too_many_arguments)]
    pub fn RadioButtonItem(
        &self,
        text: String,
        selected: bool,
        #[default(None)] icon: Option<Rc<dyn Painter>>,
        #[default(true)] enabled: bool,
        #[default(None)] mnemonic: Option<char>,
        #[default(None)] shortcut: Option<KeyShortcut>,
        on_click: impl Fn() + 'static,
    ) {
        let mut item = MenuItem::new(MenuItemKind::RadioButton { selected }, text);
        (item.icon, item.enabled, item.mnemonic, item.shortcut) = (icon, enabled, mnemonic, shortcut);
        item.on_click = Some(Rc::new(on_click));
        self.push(item);
    }
}
