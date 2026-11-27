//! Menus of the desktop's own (Compose Desktop's Menu.desktop.kt MenuScope: a tray icon's menu): items built as the
//! composable that takes them composes ([`MenuScope`]), shown by the platform (Linux's dbusmenu), their clicks
//! called back on the app's thread.

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
