//! Styles (Compose's Style.kt): a [`Style`] describes how to style a node as a block run on a [`StyleScope`], the
//! way a chain of modifiers does; styles combine, the later ones overriding the earlier ones property by property.

use super::scope::StyleScope;
use std::rc::Rc;

/// A style of a scope of its own (Compose's `fun interface CustomStyle<ScopeT : CustomStyleScope>`): what a
/// component taking a style restricted to some properties, or adding its own, defines; a closure taking the scope
/// is one. It becomes a [`Style`] by running it on a scope made of the style scope (Compose's
/// `object : StyleScope by this, MyScope {}`: [`StyleScopeRef`](super::StyleScopeRef)).
pub trait CustomStyle<S: ?Sized> {
    /// Sets the style's properties on `scope` (Compose's `ScopeT.applyStyle()`).
    fn apply_style(&self, scope: &S);
}

impl<S: ?Sized, F: Fn(&S)> CustomStyle<S> for F {
    fn apply_style(&self, scope: &S) {
        self(scope)
    }
}

impl<S: ?Sized> CustomStyle<S> for Rc<dyn CustomStyle<S>> {
    fn apply_style(&self, scope: &S) {
        (**self).apply_style(scope)
    }
}

/// How to style a node (Compose's `fun interface Style`): a block run on a [`StyleScope`], setting properties —
/// sizes, paddings, background, border, shape, shadows, layer properties, text properties —, each set again
/// replacing what was set before, some only in a state (`pressed(|| …)`), some animated (`animate(|| …)`). A
/// style is given to [`Modifier.styleable`](super::StyleModifiers::styleable), or to a component's style
/// parameter.
///
/// A style runs in its own observation scope: a state it reads changes only the phases its properties affect
/// (a background is drawn again, nothing composed); it reads composition locals where it is applied
/// ([`current_value`](super::CustomStyleScopeExt::current_value)).
///
/// A closure `|s: &dyn StyleScope| …` is a style; [`Style`](const@Style) is the empty one, [`then`](Style::then)
/// and [`combined_style`] combine them. Combined or shared, a style is an `Rc<dyn Style>`, itself a style: equal to
/// another when it is the same one, or combines equal ones.
pub trait Style {
    /// Sets the style's properties on `scope` (Compose's `StyleScope.applyStyle()`).
    fn apply_style(&self, scope: &dyn StyleScope);

    /// This style, then `other` over it, property by property (Compose's infix `then`): the empty style gives the
    /// other.
    fn then(self, other: impl Style + 'static) -> Rc<dyn Style>
    where
        Self: Sized + 'static,
    {
        combine2(self.into_shared(), other.into_shared())
    }

    /// It shared (an `Rc` of a shared one is itself).
    #[doc(hidden)]
    fn into_shared(self) -> Rc<dyn Style>
    where
        Self: Sized + 'static,
    {
        Rc::new(self)
    }

    /// The styles it combines, if it is a combined one (Compose's CombinedStyle's `styles`).
    #[doc(hidden)]
    fn combined_styles(&self) -> Option<&[Rc<dyn Style>]> {
        None
    }

    /// Whether it is the empty style (Compose's `=== Style`).
    #[doc(hidden)]
    fn is_empty_style(&self) -> bool {
        false
    }
}

impl<F: Fn(&dyn StyleScope)> Style for F {
    fn apply_style(&self, scope: &dyn StyleScope) {
        self(scope)
    }
}

impl Style for Rc<dyn Style> {
    fn apply_style(&self, scope: &dyn StyleScope) {
        (**self).apply_style(scope)
    }
    fn into_shared(self) -> Rc<dyn Style> {
        self
    }
    fn combined_styles(&self) -> Option<&[Rc<dyn Style>]> {
        (**self).combined_styles()
    }
    fn is_empty_style(&self) -> bool {
        (**self).is_empty_style()
    }
}

/// A style is a custom style of the style scope (Compose's `Style : CustomStyle<StyleScope>`).
impl<'s> CustomStyle<dyn StyleScope + 's> for dyn Style {
    fn apply_style(&self, scope: &(dyn StyleScope + 's)) {
        Style::apply_style(self, scope)
    }
}

/// The type of the empty style, [`Style`](const@Style).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EmptyStyle;

impl Style for EmptyStyle {
    fn apply_style(&self, _: &dyn StyleScope) {}
    fn is_empty_style(&self) -> bool {
        true
    }
}

/// The empty style, setting nothing (Compose's `Style` companion used as a value): a style parameter's default.
/// Combined with another style, it gives that one; `styleable` given it adds nothing to the modifier.
pub const Style: EmptyStyle = EmptyStyle;

/// Styles combined (Compose's internal CombinedStyle): each applied in turn, flat.
struct CombinedStyle {
    styles: Vec<Rc<dyn Style>>,
}

impl Style for CombinedStyle {
    fn apply_style(&self, scope: &dyn StyleScope) {
        for style in &self.styles {
            style.apply_style(scope);
        }
    }
    fn combined_styles(&self) -> Option<&[Rc<dyn Style>]> {
        Some(&self.styles)
    }
}

/// Whether two styles are equal (Compose's `==`): the same one, both the empty one, or combining equal ones.
pub(crate) fn same_style(a: &Rc<dyn Style>, b: &Rc<dyn Style>) -> bool {
    if Rc::ptr_eq(a, b) || (a.is_empty_style() && b.is_empty_style()) {
        return true;
    }
    match (a.combined_styles(), b.combined_styles()) {
        (Some(x), Some(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_style(p, q)),
        _ => false,
    }
}

/// The parts of a style as a combination spreads it: a combined one's, else itself.
fn spread(style: &Rc<dyn Style>, into: &mut Vec<Rc<dyn Style>>) {
    match style.combined_styles() {
        Some(parts) => into.extend(parts.iter().cloned()),
        None => into.push(style.clone()),
    }
}

/// Compose's `Style(style1, style2)`.
fn combine2(style1: Rc<dyn Style>, style2: Rc<dyn Style>) -> Rc<dyn Style> {
    if style1.is_empty_style() {
        return style2;
    }
    if style2.is_empty_style() {
        return style1;
    }
    match (style1.combined_styles().is_some(), style2.combined_styles().is_some()) {
        (false, false) => Rc::new(CombinedStyle { styles: vec![style1, style2] }),
        _ => {
            let mut styles = Vec::new();
            spread(&style1, &mut styles);
            spread(&style2, &mut styles);
            combine_all(styles)
        }
    }
}

/// Compose's `Style(vararg styles)`: the empty ones left out, the one left if only one is.
fn combine_all(styles: Vec<Rc<dyn Style>>) -> Rc<dyn Style> {
    if !styles.iter().any(|s| s.is_empty_style()) {
        return Rc::new(CombinedStyle { styles });
    }
    let mut kept: Vec<Rc<dyn Style>> = styles.into_iter().filter(|s| !s.is_empty_style()).collect();
    match kept.len() {
        0 => Rc::new(EmptyStyle),
        1 => kept.pop().expect("one style"),
        _ => Rc::new(CombinedStyle { styles: kept }),
    }
}

/// Two styles combined, `style2`'s properties overriding `style1`'s (Compose's `Style(style1, style2)`): the
/// empty one left out, combined ones spread.
///
/// Kotlin: `Style(style1: Style, style2: Style)`
pub fn combined_style(style1: impl Style + 'static, style2: impl Style + 'static) -> Rc<dyn Style> {
    combine2(style1.into_shared(), style2.into_shared())
}

/// Three styles combined, each overriding the ones before it (Compose's `Style(style1, style2, style3)`).
///
/// Kotlin: `Style(style1: Style, style2: Style, style3: Style)`
pub fn combined_style3(
    style1: impl Style + 'static,
    style2: impl Style + 'static,
    style3: impl Style + 'static,
) -> Rc<dyn Style> {
    let (style1, style2, style3) = (style1.into_shared(), style2.into_shared(), style3.into_shared());
    if style1.is_empty_style() {
        return combine2(style2, style3);
    }
    if style2.is_empty_style() {
        return combine2(style1, style3);
    }
    if style3.is_empty_style() {
        return combine2(style1, style2);
    }
    let mut styles = Vec::new();
    for style in [&style1, &style2, &style3] {
        spread(style, &mut styles);
    }
    combine_all(styles)
}

/// Styles combined, each overriding the ones before it (Compose's `Style(vararg styles)`): the empty ones left
/// out (none left: the empty style; one: itself).
///
/// Kotlin: `Style(vararg styles: Style)`
pub fn combined_styles(styles: &[Rc<dyn Style>]) -> Rc<dyn Style> {
    combine_all(styles.to_vec())
}
