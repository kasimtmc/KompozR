//! What a text context menu holds (Compose's text.contextmenu.data and the TextContextMenuItems of
//! CommonContextMenuArea.kt): its items and separators, their keys, the session that closes it.

use std::fmt;
use std::rc::Rc;

/// What identifies a component of a text context menu, to filter it by (Compose's component `key` and
/// TextContextMenuKeys).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TextContextMenuKey {
    Cut,
    Copy,
    Paste,
    SelectAll,
    Autofill,
    /// A separator's.
    Separator,
    /// An app's own item's.
    Other(Rc<str>),
}

/// An open text context menu (Compose's TextContextMenuSession): what an item's click is given to close it.
pub trait TextContextMenuSession {
    fn close(&self);
}

type ItemClick = Rc<dyn Fn(&dyn TextContextMenuSession)>;

/// An item of a text context menu (Compose's TextContextMenuItem): its key, its label, whether it can be chosen
/// now, and what choosing it does (given the session, to close the menu).
#[derive(Clone)]
pub struct TextContextMenuItem {
    pub key: TextContextMenuKey,
    pub label: String,
    pub enabled: bool,
    pub on_click: ItemClick,
}

impl fmt::Debug for TextContextMenuItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TextContextMenuItem(key={:?}, label={:?}, enabled={})", self.key, self.label, self.enabled)
    }
}

/// A component of a text context menu (Compose's TextContextMenuComponent): an item or a separator.
#[derive(Clone, Debug)]
pub enum TextContextMenuComponent {
    Item(TextContextMenuItem),
    /// A line between items (Compose's TextContextMenuSeparator).
    Separator,
}

impl TextContextMenuComponent {
    /// Its key.
    pub fn key(&self) -> TextContextMenuKey {
        match self {
            TextContextMenuComponent::Item(item) => item.key.clone(),
            TextContextMenuComponent::Separator => TextContextMenuKey::Separator,
        }
    }

    /// One of the text items (copy, select all, …) with its key and label, that runs `on_click` and closes the
    /// menu when chosen.
    pub fn item(kind: TextContextMenuItems, enabled: bool, on_click: impl Fn() + 'static) -> TextContextMenuComponent {
        TextContextMenuComponent::Item(TextContextMenuItem {
            key: kind.key(),
            label: kind.label().to_string(),
            enabled,
            on_click: Rc::new(move |session| {
                on_click();
                session.close();
            }),
        })
    }
}

/// What a text context menu shows (Compose's TextContextMenuData): its components, in order.
#[derive(Clone, Debug, Default)]
pub struct TextContextMenuData {
    pub components: Vec<TextContextMenuComponent>,
}

impl TextContextMenuData {
    /// A menu of nothing (Compose's TextContextMenuData.Empty).
    pub const Empty: TextContextMenuData = TextContextMenuData { components: Vec::new() };

    pub fn new(components: Vec<TextContextMenuComponent>) -> TextContextMenuData {
        TextContextMenuData { components }
    }
}

/// The text items of a context menu or toolbar (Compose's TextContextMenuItems): each one's key and label.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextContextMenuItems {
    Cut,
    Copy,
    Paste,
    SelectAll,
    Autofill,
}

impl TextContextMenuItems {
    pub fn key(self) -> TextContextMenuKey {
        match self {
            TextContextMenuItems::Cut => TextContextMenuKey::Cut,
            TextContextMenuItems::Copy => TextContextMenuKey::Copy,
            TextContextMenuItems::Paste => TextContextMenuKey::Paste,
            TextContextMenuItems::SelectAll => TextContextMenuKey::SelectAll,
            TextContextMenuItems::Autofill => TextContextMenuKey::Autofill,
        }
    }

    /// Its label (Android's English strings).
    pub fn label(self) -> &'static str {
        match self {
            TextContextMenuItems::Cut => "Cut",
            TextContextMenuItems::Copy => "Copy",
            TextContextMenuItems::Paste => "Paste",
            TextContextMenuItems::SelectAll => "Select all",
            TextContextMenuItems::Autofill => "Autofill",
        }
    }
}
