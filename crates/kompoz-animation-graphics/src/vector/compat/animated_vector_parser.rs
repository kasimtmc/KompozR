//! `<animated-vector>` read (Compose's XmlAnimatedVectorParser.android.kt): its drawable and its `<target>`s, each
//! the name of a group or path and its animation.

use super::resources::{AttrDef, Resources, XmlResourceError, attrs};
use super::xml::{XmlEvent, XmlPullParser};
use crate::res::animator_resources::load_animator_resource;
use crate::vector::animated_image_vector::{AnimatedImageVector, AnimatedVectorTarget};
use kompoz_graphics::ImageVector;

const TagAnimatedVector: &str = "animated-vector";
const TagAnimatedVectorTarget: &str = "target";

const STYLEABLE_ANIMATED_VECTOR_DRAWABLE: &[&AttrDef] = &[&attrs::DRAWABLE];
const STYLEABLE_ANIMATED_VECTOR_DRAWABLE_DRAWABLE: usize = 0;
const STYLEABLE_ANIMATED_VECTOR_DRAWABLE_TARGET: &[&AttrDef] = &[&attrs::NAME, &attrs::ANIMATION];
const STYLEABLE_ANIMATED_VECTOR_DRAWABLE_TARGET_ANIMATION: usize = 1;
const STYLEABLE_ANIMATED_VECTOR_DRAWABLE_TARGET_NAME: usize = 0;

/// Android's "Resource ID #0x0" (a reference that isn't there).
fn no_resource() -> XmlResourceError {
    XmlResourceError::new("NotFoundException", "Resource ID #0x0")
}

/// Compose's `parseAnimatedVectorTarget`: a `<target>`'s name (empty if it has none) and its animation's animator.
fn parse_animated_vector_target(
    parser: &XmlPullParser,
    res: Resources,
) -> Result<AnimatedVectorTarget, XmlResourceError> {
    let a = res.obtain_attributes(parser, STYLEABLE_ANIMATED_VECTOR_DRAWABLE_TARGET)?;
    let name = a.get_string(STYLEABLE_ANIMATED_VECTOR_DRAWABLE_TARGET_NAME).unwrap_or_default();
    let animation = a.get_resource_id(STYLEABLE_ANIMATED_VECTOR_DRAWABLE_TARGET_ANIMATION).ok_or_else(no_resource)?;
    Ok(AnimatedVectorTarget { name, animator: load_animator_resource(res, &animation)? })
}

/// Compose's `parseAnimatedImageVector`: the element the parser is at (its tag isn't checked, as Compose's) read as
/// an animated vector: its targets, then its drawable (Compose's `ImageVector.vectorResource`; KompozR's reader of
/// vector drawables, [`ImageVector::from_vector_xml_with`], its color references looked up the same way).
pub(crate) fn parse_animated_image_vector(
    parser: &mut XmlPullParser,
    res: Resources,
) -> Result<AnimatedImageVector, XmlResourceError> {
    let a = res.obtain_attributes(parser, STYLEABLE_ANIMATED_VECTOR_DRAWABLE)?;
    let drawable = a.get_resource_id(STYLEABLE_ANIMATED_VECTOR_DRAWABLE_DRAWABLE);
    let mut targets = Vec::new();
    parser.for_each_child_of(TagAnimatedVector, |p| {
        if p.event_type() == XmlEvent::StartTag && p.name() == TagAnimatedVectorTarget {
            targets.push(parse_animated_vector_target(p, res)?);
        }
        Ok(())
    })?;
    let drawable = drawable.ok_or_else(no_resource)?;
    let text = res.xml_text(&drawable)?;
    let image_vector = ImageVector::from_vector_xml_with(&text, &|reference| res.color_of_reference(reference))
        .ok_or_else(|| XmlResourceError::new("XmlPullParserException", "the drawable isn't a <vector>"))?;
    Ok(AnimatedImageVector::new(image_vector, targets))
}
