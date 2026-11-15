//! A minimum number of lines as a minimum height (Compose's MinLinesConstrainer).

use super::layout_utils::round_to_int;
use kompoz_graphics::{Constraints, Density, LayoutDirection};
use kompoz_text::{FontFamilyResolver, Paragraph, ParagraphIntrinsics, TextOverflow, TextStyle};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The reference text one line is measured with (Compose's EmptyTextReplacement: ten H, a text field's least
/// width).
const EMPTY_TEXT_REPLACEMENT: &str = "HHHHHHHHHH";
/// Two lines of it (Compose's TwoLineTextReplacement).
const TWO_LINE_TEXT_REPLACEMENT: &str = "HHHHHHHHHH\nHHHHHHHHHH";

/// Turns a minimum number of lines into a minimum height (Compose's MinLinesConstrainer): how tall a line of the
/// style is, and each line after it, measured once (two paragraphs) and kept. Get one with
/// [`MinLinesConstrainer::from`], which reuses the last one made for the same style, density, direction and
/// resolver.
#[derive(Clone)]
pub(crate) struct MinLinesConstrainer {
    inner: Rc<Inner>,
}

struct Inner {
    layout_direction: LayoutDirection,
    /// The style with its defaults resolved (what it was asked for, and what it measures in).
    input_text_style: TextStyle,
    density: Density,
    font_family_resolver: Rc<dyn FontFamilyResolver>,
    line_height_cache: Cell<f32>,
    one_line_height_cache: Cell<f32>,
}

thread_local! {
    /// The last one made (Compose's cache of one: the styles coerced at once tend to be alike).
    static LAST: RefCell<Option<MinLinesConstrainer>> = const { RefCell::new(None) };
}

impl MinLinesConstrainer {
    /// One for `style` in `layout_direction`, `density` and `resolver`: `kept` if it is one for them, else the
    /// last one made if it is, else a new one (Compose's MinLinesConstrainer.from). Unlike Compose's (which
    /// compares only the density's pixel ratio) the font scale counts too, as line heights follow it.
    pub(crate) fn from(
        kept: Option<&MinLinesConstrainer>,
        layout_direction: LayoutDirection,
        style: &TextStyle,
        density: Density,
        font_family_resolver: &Rc<dyn FontFamilyResolver>,
    ) -> MinLinesConstrainer {
        let resolved = style.resolve_defaults(layout_direction);
        let fits = |c: &MinLinesConstrainer| {
            let i = &c.inner;
            i.layout_direction == layout_direction
                && i.input_text_style == resolved
                && i.density == density
                && same_resolver(&i.font_family_resolver, font_family_resolver)
        };
        if let Some(k) = kept
            && fits(k)
        {
            return k.clone();
        }
        if let Some(last) = LAST.with(|l| l.borrow().clone())
            && fits(&last)
        {
            return last;
        }
        let made = MinLinesConstrainer {
            inner: Rc::new(Inner {
                layout_direction,
                input_text_style: resolved,
                density,
                font_family_resolver: font_family_resolver.clone(),
                line_height_cache: Cell::new(f32::NAN),
                one_line_height_cache: Cell::new(f32::NAN),
            }),
        };
        LAST.with(|l| *l.borrow_mut() = Some(made.clone()));
        made
    }

    /// `constraints` with a min height of `min_lines` lines (Compose's coerceMinLines): the first line's height
    /// and each further line's, rounded, within the max height; one line leaves the min height as it is. The first
    /// call measures two paragraphs.
    pub(crate) fn coerce_min_lines(&self, constraints: Constraints, min_lines: usize) -> Constraints {
        let i = &self.inner;
        let (mut one_line, mut line) = (i.one_line_height_cache.get(), i.line_height_cache.get());
        if one_line.is_nan() || line.is_nan() {
            one_line = self.height_of(EMPTY_TEXT_REPLACEMENT, false, 1);
            let two_lines = self.height_of(TWO_LINE_TEXT_REPLACEMENT, true, 2);
            line = two_lines - one_line;
            i.one_line_height_cache.set(one_line);
            i.line_height_cache.set(line);
        }
        let min_height = if min_lines != 1 {
            round_to_int(one_line + line * (min_lines as f32 - 1.0)).max(0).min(constraints.max_height)
        } else {
            constraints.min_height
        };
        Constraints::new(constraints.min_width, constraints.max_width, min_height, constraints.max_height)
    }

    /// How tall `text` is in its style, unbounded.
    fn height_of(&self, text: &str, soft_wrap: bool, max_lines: usize) -> f32 {
        let i = &self.inner;
        let intrinsics = ParagraphIntrinsics::new(
            text,
            &i.input_text_style,
            &[],
            &[],
            i.density,
            &i.font_family_resolver,
            soft_wrap,
        );
        Paragraph::new(Rc::new(intrinsics), Constraints::default(), max_lines, TextOverflow::Clip).height()
    }
}

/// The same resolver (the same object).
pub(crate) fn same_resolver(a: &Rc<dyn FontFamilyResolver>, b: &Rc<dyn FontFamilyResolver>) -> bool {
    std::ptr::eq(Rc::as_ptr(a) as *const (), Rc::as_ptr(b) as *const ())
}
