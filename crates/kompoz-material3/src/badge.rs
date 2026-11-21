//! Material's badges, as Compose's Badge.kt: a [`Badge`] — a small dot, or a pill holding a short text (a count) —
//! put on the top end corner of what it tells about by a [`BadgedBox`], which keeps the size and the baselines of
//! that anchor (the badge sticks out of it), the badge kept inside the bounds a navigation item gives it
//! ([`badge_bounds`], Compose's internal `Modifier.badgeBounds` and its rulers).

use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::tokens::BadgeTokens;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp};
use kompoz_runtime::{kompozable, named_args};
use kompoz_ui::layout::{
    AlignmentLine, FirstBaseline, HorizontalRuler, LastBaseline, Layout, VerticalRuler, measure_policy,
};
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;

/// A badge on the top end corner of `content` (Compose's `BadgedBox`): `content` in a Box centering it (the
/// anchor), `badge` (typically a [`Badge`]) in a Box of its own; the box as big as the anchor, its baselines the
/// anchor's only. A badge without content (no wider than [`Badge`]'s dot) is put 6 dp in from the anchor's end,
/// sticking 6 dp down into it; one with content 12 dp in, 14 dp down into it — kept at least the badge's width in
/// from the end and below the top of the bounds a layout around provides by [`badge_bounds`] (a navigation item's).
#[kompozable]
pub fn BadgedBox(badge: impl Fn() + 'static, #[default(Modifier)] modifier: Modifier, content: impl Fn() + 'static) {
    Layout(
        modifier = modifier,
        content = move || {
            Box(modifier = Modifier.layout_id("anchor"), content_alignment = Alignment::Center, content = || content());
            Box(modifier = Modifier.layout_id("badge"), content = || badge());
        },
        measure_policy = measure_policy(|scope, measurables, constraints| {
            let badge_placeable = measurables
                .iter()
                .find(|m| m.layout_id().is_some_and(|id| id.is(&"badge")))
                .expect("BadgedBox: its badge")
                .measure(
                    // Measure with loose constraints for height as we don't want the text to take up more space
                    // than it needs.
                    named_args!(constraints.copy(min_height = 0)),
                );

            let anchor_placeable = measurables
                .iter()
                .find(|m| m.layout_id().is_some_and(|id| id.is(&"anchor")))
                .expect("BadgedBox: its anchor")
                .measure(constraints);

            let first_baseline = anchor_placeable.get(FirstBaseline);
            let last_baseline = anchor_placeable.get(LastBaseline);
            let total_width = anchor_placeable.width();
            let total_height = anchor_placeable.height();

            // Use the width of the badge to infer whether it has any content (based on radius used in [Badge]) and
            // determine its horizontal offset.
            let has_content = badge_placeable.width() > scope.round_to_px(BadgeTokens::Size);
            let badge_horizontal_offset = if has_content { BadgeWithContentHorizontalOffset } else { BadgeOffset };
            let badge_vertical_offset = if has_content { BadgeWithContentVerticalOffset } else { BadgeOffset };
            let badge_horizontal_offset_px = scope.round_to_px(badge_horizontal_offset);
            let badge_vertical_offset_px = scope.round_to_px(badge_vertical_offset);
            let (top_ruler, end_ruler) = (BadgeTopRuler(), BadgeEndRuler());

            named_args!(scope.layout(
                total_width,
                total_height,
                // Provide custom baselines based only on the anchor content to avoid default baseline calculations
                // from including by any badge content.
                alignment_lines = vec![
                    (AlignmentLine::from(FirstBaseline), first_baseline),
                    (AlignmentLine::from(LastBaseline), last_baseline),
                ],
                placement_block = move |p| {
                    named_args!(p.place_relative(&anchor_placeable, 0, 0));

                    // Desired Badge placement
                    let badge_x = (anchor_placeable.width() - badge_horizontal_offset_px)
                        .min((end_ruler.current(p, f32::INFINITY) as i32) - badge_placeable.width());

                    let badge_y = (-badge_placeable.height() + badge_vertical_offset_px)
                        .max(top_ruler.current(p, f32::NEG_INFINITY) as i32);

                    named_args!(p.place_relative(&badge_placeable, badge_x, badge_y));
                }
            ))
        }),
    );
}

/// A badge (Compose's `Badge`): dynamic information, as a count of what is new, on a navigation item — a dot of 6
/// dp without `content`; with it, a pill at least 16 dp each way, 4 dp of padding at its sides, its content (in a
/// Row, centered) in `content_color` and the type scale's label small. `container_color` is the scheme's error
/// color by default; `content_color` the one on `container_color` (Kotlin's default `contentColorFor(containerColor)`,
/// here `None`). It isn't a Surface: what is behind it still gets the pointer.
#[kompozable]
pub fn Badge(
    #[default(Modifier)] modifier: Modifier,
    #[default(BadgeDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(None)] content: Option<Rc<dyn Fn()>>,
) {
    let content_color = content_color.unwrap_or_else(|| crate::content_color_for(container_color));
    let size = if content.is_some() { BadgeTokens::LargeSize } else { BadgeTokens::Size };
    let shape = if content.is_some() { BadgeTokens::LargeShape.value() } else { BadgeTokens::Shape.value() };

    // Draw badge container.
    let padding = if content.is_some() {
        named_args!(Modifier.padding_hv(horizontal = BadgeWithContentHorizontalPadding))
    } else {
        Modifier
    };
    Row(
        modifier = modifier.default_min_size_with(size, size).background(container_color, shape).then(padding),
        vertical_alignment = Alignment::CenterVertically,
        horizontal_arrangement = Arrangement::Center,
        content = move || {
            if let Some(content) = content.clone() {
                // Not using Surface composable because it blocks touch propagation behind it.
                let style = BadgeTokens::LargeLabelTextFont.value();
                ProvideContentColorTextStyle(content_color, style, move || content());
            }
        },
    );
}

/// The badges' defaults (Compose's `BadgeDefaults`).
pub struct BadgeDefaults;

impl BadgeDefaults {
    /// The scheme's error color (Compose's `BadgeDefaults.containerColor`).
    pub fn container_color() -> Color {
        BadgeTokens::Color.value()
    }
}

// Leading and trailing text padding when a badge is displaying text that is too long to fit in a circular badge, e.g.
// if badge number is greater than 9.
pub(crate) const BadgeWithContentHorizontalPadding: Dp = Dp(4.0);

// Offsets for badge when there is short or long content
// Horizontally align start/end of text badge 12.dp from the top end corner of its anchor
// Vertical overlap with anchor is 14.dp
pub(crate) const BadgeWithContentHorizontalOffset: Dp = Dp(12.0);
pub(crate) const BadgeWithContentVerticalOffset: Dp = Dp(14.0);

// Offsets for badge when there is no content
// Horizontally align start/end of icon only badge 6.dp from the end/start edge of anchor
// Vertical overlap with anchor is 6.dp
pub(crate) const BadgeOffset: Dp = Dp(6.0);

thread_local! {
    static BADGE_TOP_RULER: HorizontalRuler = HorizontalRuler::named("BadgeTopRuler");
    static BADGE_END_RULER: VerticalRuler = VerticalRuler::named("BadgeEndRuler");
}

/// The top of the bounds a badge stays in (Compose's internal `BadgeTopRuler`), provided by [`badge_bounds`].
pub(crate) fn BadgeTopRuler() -> HorizontalRuler {
    BADGE_TOP_RULER.with(HorizontalRuler::clone)
}

/// The end of the bounds a badge stays in (Compose's internal `BadgeEndRuler`), provided by [`badge_bounds`].
pub(crate) fn BadgeEndRuler() -> VerticalRuler {
    BADGE_END_RULER.with(VerticalRuler::clone)
}

/// `modifier` giving the badges of what it wraps its bounds (Compose's internal `Modifier.badgeBounds`): its right
/// edge ([`BadgeEndRuler`]; provided as is, not mirrored, since [`BadgedBox`] places the badge relative) and its top
/// ([`BadgeTopRuler`]).
#[allow(dead_code)] // (navigation items': WP4b)
pub(crate) fn badge_bounds(modifier: Modifier) -> Modifier {
    modifier.layout(|scope, measurable, constraints| {
        let placeable = measurable.measure(constraints);
        let (end_ruler, top_ruler) = (BadgeEndRuler(), BadgeTopRuler());
        scope.layout_with_rulers(
            placeable.width(),
            placeable.height(),
            move |rulers| {
                // use provides instead of provideRelative cause we will place relative in the badge code
                rulers.provides(&end_ruler, rulers.coordinates().size().width as f32);
                rulers.provides(&top_ruler, 0.0);
            },
            move |p| named_args!(p.place(&placeable, 0, 0)),
        )
    })
}
