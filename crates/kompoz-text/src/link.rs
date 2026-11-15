//! Links in text (Compose's LinkAnnotation, TextLinkStyles and LinkInteractionListener) and the other annotations
//! a text can carry for the platform (Compose's UrlAnnotation and TtsAnnotation).

use crate::span_style::SpanStyle;
use std::fmt;
use std::sync::Arc;

/// What is called when a link is clicked (Compose's LinkInteractionListener): equal only to itself (a copy of it).
#[derive(Clone)]
pub struct LinkInteractionListener(Arc<dyn Fn(&LinkAnnotation) + Send + Sync>);

impl LinkInteractionListener {
    pub fn new(on_click: impl Fn(&LinkAnnotation) + Send + Sync + 'static) -> LinkInteractionListener {
        LinkInteractionListener(Arc::new(on_click))
    }
    /// Tells it `link` was clicked.
    pub fn on_click(&self, link: &LinkAnnotation) {
        (self.0)(link)
    }
}

impl PartialEq for LinkInteractionListener {
    fn eq(&self, o: &LinkInteractionListener) -> bool {
        Arc::ptr_eq(&self.0, &o.0)
    }
}

impl fmt::Debug for LinkInteractionListener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LinkInteractionListener({:p})", Arc::as_ptr(&self.0) as *const ())
    }
}

/// How a link looks (Compose's TextLinkStyles): always, and while it is focused, hovered or pressed (each merged
/// over the ones before it).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TextLinkStyles {
    pub style: Option<SpanStyle>,
    pub focused_style: Option<SpanStyle>,
    pub hovered_style: Option<SpanStyle>,
    pub pressed_style: Option<SpanStyle>,
}

impl TextLinkStyles {
    pub fn new(
        style: Option<SpanStyle>,
        focused_style: Option<SpanStyle>,
        hovered_style: Option<SpanStyle>,
        pressed_style: Option<SpanStyle>,
    ) -> TextLinkStyles {
        TextLinkStyles { style, focused_style, hovered_style, pressed_style }
    }
}

/// Compose's `TextLinkStyles(style = null, focusedStyle = null, hoveredStyle = null, pressedStyle = null)`.
#[kompoz_runtime::named]
pub fn TextLinkStyles(
    #[default(None)] style: Option<SpanStyle>,
    #[default(None)] focused_style: Option<SpanStyle>,
    #[default(None)] hovered_style: Option<SpanStyle>,
    #[default(None)] pressed_style: Option<SpanStyle>,
) -> TextLinkStyles {
    TextLinkStyles::new(style, focused_style, hovered_style, pressed_style)
}

/// A link on a stretch of text (Compose's LinkAnnotation): a URL, opened by the platform's uri handler unless a
/// listener takes the click, or a tagged stretch whose listener is told. Neither has a look of its own unless its
/// styles give one.
#[derive(Clone, PartialEq)]
pub enum LinkAnnotation {
    Url {
        url: String,
        styles: Option<TextLinkStyles>,
        link_interaction_listener: Option<LinkInteractionListener>,
    },
    Clickable {
        tag: String,
        styles: Option<TextLinkStyles>,
        link_interaction_listener: Option<LinkInteractionListener>,
    },
}

impl LinkAnnotation {
    /// A URL with no styles or listener of its own (Compose's LinkAnnotation.Url(url)).
    pub fn url(url: impl Into<String>) -> LinkAnnotation {
        LinkAnnotation::url_with(url, None, None)
    }
    /// A URL, its styles and listener given.
    pub fn url_with(
        url: impl Into<String>,
        styles: Option<TextLinkStyles>,
        link_interaction_listener: Option<LinkInteractionListener>,
    ) -> LinkAnnotation {
        LinkAnnotation::Url { url: url.into(), styles, link_interaction_listener }
    }
    /// A tagged stretch whose listener is told of clicks (Compose's LinkAnnotation.Clickable(tag) { … }).
    pub fn clickable(
        tag: impl Into<String>,
        on_click: impl Fn(&LinkAnnotation) + Send + Sync + 'static,
    ) -> LinkAnnotation {
        LinkAnnotation::clickable_with(tag, None, Some(LinkInteractionListener::new(on_click)))
    }
    /// A tagged stretch, its styles and listener given.
    pub fn clickable_with(
        tag: impl Into<String>,
        styles: Option<TextLinkStyles>,
        link_interaction_listener: Option<LinkInteractionListener>,
    ) -> LinkAnnotation {
        LinkAnnotation::Clickable { tag: tag.into(), styles, link_interaction_listener }
    }
    /// How it looks.
    pub fn styles(&self) -> Option<&TextLinkStyles> {
        match self {
            LinkAnnotation::Url { styles, .. } | LinkAnnotation::Clickable { styles, .. } => styles.as_ref(),
        }
    }
    /// Who is told of clicks.
    pub fn link_interaction_listener(&self) -> Option<&LinkInteractionListener> {
        match self {
            LinkAnnotation::Url { link_interaction_listener, .. }
            | LinkAnnotation::Clickable { link_interaction_listener, .. } => link_interaction_listener.as_ref(),
        }
    }
    /// The URL of a URL link.
    pub fn as_url(&self) -> Option<&str> {
        match self {
            LinkAnnotation::Url { url, .. } => Some(url),
            _ => None,
        }
    }
    /// The tag of a clickable one.
    pub fn as_tag(&self) -> Option<&str> {
        match self {
            LinkAnnotation::Clickable { tag, .. } => Some(tag),
            _ => None,
        }
    }
    /// The same with other styles (Compose's copy(styles = …)).
    pub fn with_styles(&self, styles: Option<TextLinkStyles>) -> LinkAnnotation {
        match self.clone() {
            LinkAnnotation::Url { url, link_interaction_listener, .. } => {
                LinkAnnotation::Url { url, styles, link_interaction_listener }
            }
            LinkAnnotation::Clickable { tag, link_interaction_listener, .. } => {
                LinkAnnotation::Clickable { tag, styles, link_interaction_listener }
            }
        }
    }
}

/// `LinkAnnotation.Url(url=…)`, `LinkAnnotation.Clickable(tag=…)`, as Compose writes them.
impl fmt::Debug for LinkAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LinkAnnotation::Url { url, .. } => write!(f, "LinkAnnotation.Url(url={url})"),
            LinkAnnotation::Clickable { tag, .. } => write!(f, "LinkAnnotation.Clickable(tag={tag})"),
        }
    }
}

/// A URL on a stretch of text for accessibility services (Compose's UrlAnnotation; links are LinkAnnotations).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct UrlAnnotation {
    pub url: String,
}

impl UrlAnnotation {
    pub fn new(url: impl Into<String>) -> UrlAnnotation {
        UrlAnnotation { url: url.into() }
    }
}

/// How a text-to-speech engine reads a stretch of text (Compose's TtsAnnotation): as `verbatim` says, character
/// by character (Compose's VerbatimTtsAnnotation).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TtsAnnotation {
    Verbatim(String),
}

impl TtsAnnotation {
    /// Compose's VerbatimTtsAnnotation(verbatim).
    pub fn verbatim(verbatim: impl Into<String>) -> TtsAnnotation {
        TtsAnnotation::Verbatim(verbatim.into())
    }
}
