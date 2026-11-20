//! A style resolved (Compose's ResolvedStyle.kt): the properties a style set in the last resolution and the ones
//! before, which properties are animated and by what specs, the animations started when animated properties
//! changed, and the properties as they are now (an animated one between where it was and its target).
//!
//! Compose's ResolvedStyle is the StyleScope a style runs on as well; here a resolution runs the style on a
//! [`BuildScope`], each of whose setters records into the resolved style.

use super::animations::{
    FloatSpec, StyleAnimations, default_spring_spec, is_null_or_empty, property_sets_to_animation_set, same_spec,
    to_objects_set, to_primitives_set,
};
use super::properties::*;
use super::scope::*;
use super::state::StyleState;
use super::styles::Style;
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{Brush, Color, ColorFilter, Density, Dp, Shadow, Shape, TextUnit};
use kompoz_text::{
    BaselineShift, FontFamily, FontStyle, FontSynthesis, FontWeight, Hyphens, LineBreak, TextAlign, TextDecoration,
    TextDirection, TextIndent, TextMotion, TextStyle,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;

/// What a resolution keeps (Compose's ResolvedStyle's fields).
#[derive(Default)]
struct Inner {
    properties: Option<StyleProperties>,
    previous: Option<StyleProperties>,
    in_flight_animation_properties: Option<StyleProperties>,
    from_properties: Option<StyleProperties>,
    to_properties: Option<StyleProperties>,
    to_specs: Option<BTreeMap<u32, FloatSpec>>,
    from_specs: Option<BTreeMap<u32, FloatSpec>>,
    previous_from_specs: Option<BTreeMap<u32, FloatSpec>>,
    animated_primitives: u64,
    animated_objects: u32,
    /// The specs of `animate`'s block being run (none: Compose's UnspecifiedSpec, outside of one).
    default_to_spec: Option<FloatSpec>,
    default_from_spec: Option<FloatSpec>,
    animations: Option<StyleAnimations>,
}

/// A style resolved, and resolved again as it or what it reads changes (Compose's ResolvedStyle).
#[derive(Default)]
pub(crate) struct ResolvedStyle {
    inner: RefCell<Inner>,
}

/// Where a resolution runs: what the style reads (its node's state and locals, the density) and where its
/// animations run.
pub(crate) struct BuildContext {
    pub(crate) state: StyleState,
    pub(crate) locals: StyleLocals,
    pub(crate) density: Density,
    pub(crate) animation_scope: Option<CoroutineScope>,
}

impl ResolvedStyle {
    /// Runs `style` on a scope recording into it, then works out what to animate (Compose's `build`).
    pub(crate) fn build(&self, style: &dyn Style, cx: &BuildContext) {
        self.start_build();
        let scope =
            BuildScope { resolved: self, density: cx.density, state: cx.state.clone(), locals: cx.locals.clone() };
        style.apply_style(&scope);
        self.done_build(cx.animation_scope.as_ref());
    }

    /// Cancels its animations (Compose's `closeForTesting`).
    #[cfg(test)]
    pub(crate) fn close(&self) {
        let animations = self.inner.borrow_mut().animations.take();
        if let Some(a) = animations {
            a.close();
        }
    }

    /// The phases of its animated properties (Compose's `animatingFlags`).
    pub(crate) fn animating_flags(&self) -> u32 {
        self.inner.borrow().animations.as_ref().map_or(0, StyleAnimations::phase_flags)
    }

    /// The properties of the sets as their animations have them now, set on `target` (Compose's `lerpInto`).
    fn lerp_into(inner: &Inner, primitives_set: u64, objects_set: u32, target: &mut StyleProperties) {
        let Some(animations) = &inner.animations else { return };
        let Some(from) = inner.from_properties.as_ref().or(inner.previous.as_ref()) else { return };
        let Some(to) = inner.to_properties.as_ref().or(inner.properties.as_ref()) else { return };
        lerp_properties(from, to, animations, primitives_set, objects_set, target);
    }

    /// The properties resolved, those of `flags`' phases being animated as they are now, into `target` (Compose's
    /// `resolveInto`); the animations dropped once none is left.
    pub(crate) fn resolve_into(&self, flags: u32, target: &mut StyleProperties) {
        let mut inner = self.inner.borrow_mut();
        match &inner.properties {
            Some(p) => target.clone_from(p),
            None => *target = StyleProperties::default(),
        }
        let Some(animations) = &inner.animations else { return };
        if inner.from_properties.is_none() && inner.previous.is_none() {
            return;
        }
        if animations.is_empty() {
            inner.from_properties = None;
            inner.to_properties = None;
            inner.animations = None;
            return;
        }
        let primitives_filter = primitives_set_for_flags(flags);
        let objects_filter = objects_set_for_flags(flags);
        let in_flight = animations.in_flight();
        let in_flight_primitives = to_primitives_set(in_flight);
        let in_flight_objects = to_objects_set(in_flight);
        let primitives_to_animate = widen_primitives_set(in_flight_primitives, in_flight_objects) & primitives_filter;
        let objects_to_animate = widen_objects_set(primitives_to_animate, in_flight_objects) & objects_filter;
        if primitives_to_animate == 0 && objects_to_animate == 0 {
            return;
        }
        let from = inner.from_properties.as_ref().or(inner.previous.as_ref()).expect("from");
        let to = inner.to_properties.as_ref().or(inner.properties.as_ref());
        let empty = StyleProperties::default();
        let to = to.unwrap_or(&empty);
        lerp_properties(from, to, animations, primitives_to_animate, objects_to_animate, target);
    }

    /// The values of the animations in flight, kept as where the next resolution's animations start from
    /// (Compose's `prepareBuild`): read outside of the resolution's observation, as the animations change often.
    pub(crate) fn prepare_build(&self) {
        let mut inner = self.inner.borrow_mut();
        let in_flight = match (&inner.animations, &inner.properties) {
            (Some(animations), Some(_)) => animations.in_flight(),
            _ => 0,
        };
        if in_flight != 0 {
            let mut props = StyleProperties::default();
            Self::lerp_into(&inner, to_primitives_set(in_flight), to_objects_set(in_flight), &mut props);
            inner.in_flight_animation_properties = Some(props);
        } else {
            inner.in_flight_animation_properties = None;
        }
    }

    /// The last properties become the previous ones, new ones begin (Compose's `startBuild`).
    fn start_build(&self) {
        let mut inner = self.inner.borrow_mut();
        let properties = inner.properties.take();
        let new_properties = match inner.previous.take() {
            Some(mut p) => {
                p.clear();
                p
            }
            None => StyleProperties::default(),
        };
        inner.properties = Some(new_properties);
        inner.previous = properties;
        inner.previous_from_specs = None;
    }

    /// Starts the animations of the animated properties that changed, recording where they animate from
    /// (Compose's `doneBuild`).
    fn done_build(&self, scope: Option<&CoroutineScope>) {
        let mut guard = self.inner.borrow_mut();
        let inner = &mut *guard;
        if is_null_or_empty(inner.animations.as_ref()) && inner.animated_primitives == 0 && inner.animated_objects == 0
        {
            return;
        }
        // (the first resolution has nothing to animate from)
        let (Some(properties), Some(previous)) = (&inner.properties, &inner.previous) else { return };
        let effective_animated_objects = widen_objects_set(inner.animated_primitives, inner.animated_objects);
        let effective_animated_primitives = widen_primitives_set(inner.animated_primitives, inner.animated_objects);
        let objects_changed = previous.diff_objects(properties, effective_animated_objects);
        let primitives_changed = previous.diff_primitives(properties, effective_animated_primitives);
        if primitives_changed == 0 && objects_changed == 0 {
            return;
        }
        let animations = inner.animations.get_or_insert_with(StyleAnimations::default);
        // (the animations animate brushes: colors are interpolated by their brushes' times)
        let effective_primitives_changed = filter_primitive_colors(primitives_changed);
        let effective_objects_changed = widen_objects_set(primitives_changed, objects_changed);
        let changed_properties =
            property_sets_to_animation_set(effective_primitives_changed, effective_objects_changed);
        let animated_properties =
            property_sets_to_animation_set(effective_animated_primitives, effective_animated_objects);
        let started = animations.record_animations(
            animated_properties,
            changed_properties,
            inner.to_specs.as_ref(),
            inner.from_specs.as_ref(),
            inner.previous_from_specs.as_ref(),
            scope,
        );
        if started != 0 {
            let from = inner.from_properties.get_or_insert_with(StyleProperties::default);
            let started_primitives = to_primitives_set(started);
            let started_objects = to_objects_set(started);
            let widened_primitives = widen_primitives_set(started_primitives, started_objects);
            let widened_objects = widen_objects_set(started_primitives, started_objects);
            let previous = inner.previous.as_ref().expect("previous");
            previous.copy_into_filtered(from, widened_primitives, widened_objects);
            if let Some(in_flight) = inner.in_flight_animation_properties.take() {
                in_flight.copy_into_filtered(from, widened_primitives, widened_objects);
            }
        }
        if inner.animations.as_ref().is_some_and(StyleAnimations::is_empty) {
            inner.animations = None;
        }
    }

    fn with_properties(&self, f: impl FnOnce(&mut StyleProperties)) {
        if let Some(p) = self.inner.borrow_mut().properties.as_mut() {
            f(p);
        }
    }

    /// A primitive property written: animated if written in an `animate` block, or animated before and written
    /// outside of one (Compose's `recordWrite` of a primitive).
    fn record_write_primitive(&self, id: PrimitiveId) {
        let mut inner = self.inner.borrow_mut();
        let was = has_primitive(inner.animated_primitives, id);
        let (to, from) = effective_specs(&inner, id as u32, was);
        let animated = to.is_some() && from.is_some();
        inner.animated_primitives = if animated {
            with_primitive(inner.animated_primitives, id)
        } else {
            without_primitive(inner.animated_primitives, id)
        };
        record_write_common(&mut inner, id as u32, to, from);
    }

    /// The same of an object property.
    fn record_write_object(&self, id: ObjectId) {
        let mut inner = self.inner.borrow_mut();
        let was = has_object(inner.animated_objects, id);
        let (to, from) = effective_specs(&inner, id, was);
        let animated = to.is_some() && from.is_some();
        inner.animated_objects =
            if animated { with_object(inner.animated_objects, id) } else { without_object(inner.animated_objects, id) };
        record_write_common(&mut inner, id, to, from);
    }
}

/// The specs a write gets: `animate`'s, or (outside of one) the ones it had if it was animated (Compose's
/// `recordWrite`'s effective specs: none for a property not animated).
fn effective_specs(inner: &Inner, id: u32, was_animated: bool) -> (Option<FloatSpec>, Option<FloatSpec>) {
    let to = match &inner.default_to_spec {
        Some(spec) => Some(spec.clone()),
        None if was_animated => {
            Some(inner.to_specs.as_ref().and_then(|s| s.get(&id)).cloned().unwrap_or_else(default_spring_spec))
        }
        None => None,
    };
    let from = match &inner.default_from_spec {
        Some(spec) => Some(spec.clone()),
        None if was_animated => {
            Some(inner.from_specs.as_ref().and_then(|s| s.get(&id)).cloned().unwrap_or_else(default_spring_spec))
        }
        None => None,
    };
    (to, from)
}

/// Keeps a write's specs, the default spring left out (Compose's `recordWriteCommon`): a `from` spec dropped is
/// kept as the previous one (what a property animates back with).
fn record_write_common(inner: &mut Inner, id: u32, to: Option<FloatSpec>, from: Option<FloatSpec>) {
    let default = default_spring_spec();
    match to {
        Some(to) if !same_spec(&to, &default) => {
            inner.to_specs.get_or_insert_with(BTreeMap::new).insert(id, to);
        }
        _ => {
            if let Some(specs) = inner.to_specs.as_mut() {
                specs.remove(&id);
            }
        }
    }
    match from {
        Some(from) if !same_spec(&from, &default) => {
            inner.from_specs.get_or_insert_with(BTreeMap::new).insert(id, from);
        }
        _ => {
            if let Some(specs) = inner.from_specs.as_mut()
                && let Some(previous) = specs.remove(&id)
            {
                inner.previous_from_specs.get_or_insert_with(BTreeMap::new).insert(id, previous);
            }
        }
    }
}

/// The scope a resolution runs the style on (Compose's ResolvedStyle as a StyleScope): each setter records the
/// write (animated or not) and sets the property, lengths in px of the node's density.
pub(crate) struct BuildScope<'a> {
    resolved: &'a ResolvedStyle,
    density: Density,
    state: StyleState,
    locals: StyleLocals,
}

impl Deref for BuildScope<'_> {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.density
    }
}

impl BuildScope<'_> {
    fn px(&self, value: Dp) -> f32 {
        value.0 * self.density.density
    }
    fn rounded(&self, value: Dp) -> f32 {
        self.density.round_to_px(value) as f32
    }
    fn write(&self, id: PrimitiveId, set: impl FnOnce(&mut StyleProperties)) {
        self.resolved.record_write_primitive(id);
        self.resolved.with_properties(set);
    }
    fn write_object(&self, id: ObjectId, set: impl FnOnce(&mut StyleProperties)) {
        self.resolved.record_write_object(id);
        self.resolved.with_properties(set);
    }
    /// A color or brush property: both its ids written.
    fn write_both(&self, color: PrimitiveId, brush: ObjectId, set: impl FnOnce(&mut StyleProperties)) {
        self.resolved.record_write_primitive(color);
        self.resolved.record_write_object(brush);
        self.resolved.with_properties(set);
    }
}

impl CustomStyleScope for BuildScope<'_> {
    fn style_locals(&self) -> StyleLocals {
        self.locals.clone()
    }
}

impl StyleStateScope for BuildScope<'_> {
    fn state(&self) -> StyleState {
        self.state.clone()
    }
    fn state_dyn(&self, block: &mut dyn FnMut(), active: &dyn Fn(&StyleState) -> bool) {
        if active(&self.state) {
            block();
        }
    }
}

impl AnimateStyleScope for BuildScope<'_> {
    fn animate_dyn(&self, to_spec: FloatSpec, from_spec: FloatSpec, block: &mut dyn FnMut()) {
        let (previous_to, previous_from) = {
            let mut inner = self.resolved.inner.borrow_mut();
            (inner.default_to_spec.replace(to_spec), inner.default_from_spec.replace(from_spec))
        };
        // (Compose's `finally`: the specs back however the block ends)
        struct Restore<'r>(&'r ResolvedStyle, Option<Option<FloatSpec>>, Option<Option<FloatSpec>>);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                if let Ok(mut inner) = self.0.inner.try_borrow_mut() {
                    inner.default_to_spec = self.1.take().flatten();
                    inner.default_from_spec = self.2.take().flatten();
                }
            }
        }
        let _restore = Restore(self.resolved, Some(previous_to), Some(previous_from));
        block();
    }
}

impl ContentPaddingScope for BuildScope<'_> {
    fn content_padding_start(&self, value: Dp) {
        let v = self.rounded(value);
        self.write(ContentPaddingStartId, |p| p.set_content_padding_start(v));
    }
    fn content_padding_end(&self, value: Dp) {
        let v = self.rounded(value);
        self.write(ContentPaddingEndId, |p| p.set_content_padding_end(v));
    }
    fn content_padding_top(&self, value: Dp) {
        let v = self.rounded(value);
        self.write(ContentPaddingTopId, |p| p.set_content_padding_top(v));
    }
    fn content_padding_bottom(&self, value: Dp) {
        let v = self.rounded(value);
        self.write(ContentPaddingBottomId, |p| p.set_content_padding_bottom(v));
    }
}

impl ExternalPaddingScope for BuildScope<'_> {
    fn external_padding_start(&self, value: Dp) {
        let v = self.rounded(value);
        self.write(ExternalPaddingStartId, |p| p.set_external_padding_start(v));
    }
    fn external_padding_end(&self, value: Dp) {
        let v = self.rounded(value);
        self.write(ExternalPaddingEndId, |p| p.set_external_padding_end(v));
    }
    fn external_padding_top(&self, value: Dp) {
        let v = self.rounded(value);
        self.write(ExternalPaddingTopId, |p| p.set_external_padding_top(v));
    }
    fn external_padding_bottom(&self, value: Dp) {
        let v = self.rounded(value);
        self.write(ExternalPaddingBottomId, |p| p.set_external_padding_bottom(v));
    }
}

impl BorderScope for BuildScope<'_> {
    fn border_width(&self, value: Dp) {
        let width = if value.0.is_nan() {
            0.0
        } else if value == Dp::Hairline {
            1.0
        } else {
            (value.0 * self.density.density).ceil()
        };
        self.write(BorderWidthId, |p| p.set_border_width(width));
    }
    fn border_color(&self, value: Color) {
        self.write_both(BorderColorId, BorderBrushId, |p| p.set_border_color(value));
    }
    fn border_brush(&self, value: Brush) {
        self.write_both(BorderColorId, BorderBrushId, |p| p.set_border_brush(Some(value)));
    }
}

impl SizeScope for BuildScope<'_> {
    fn width(&self, value: Dp) {
        let v = self.px(value);
        self.write(WidthId, |p| p.set_width(v));
    }
    fn height(&self, value: Dp) {
        let v = self.px(value);
        self.write(HeightId, |p| p.set_height(v));
    }
    fn width_fraction(&self, fraction: f32) {
        self.write(WidthFractionId, |p| p.set_width_fraction(fraction));
    }
    fn height_fraction(&self, fraction: f32) {
        self.write(HeightFractionId, |p| p.set_height_fraction(fraction));
    }
}

impl PositionScope for BuildScope<'_> {
    fn left(&self, value: Dp) {
        let v = self.px(value);
        self.write(LeftId, |p| p.set_left(v));
    }
    fn top(&self, value: Dp) {
        let v = self.px(value);
        self.write(TopId, |p| p.set_top(v));
    }
    fn right(&self, value: Dp) {
        let v = self.px(value);
        self.write(RightId, |p| p.set_right(v));
    }
    fn bottom(&self, value: Dp) {
        let v = self.px(value);
        self.write(BottomId, |p| p.set_bottom(v));
    }
}

impl MinSizeScope for BuildScope<'_> {
    fn min_width(&self, value: Dp) {
        let v = self.px(value);
        self.write(MinWidthId, |p| p.set_min_width(v));
    }
    fn min_height(&self, value: Dp) {
        let v = self.px(value);
        self.write(MinHeightId, |p| p.set_min_height(v));
    }
}

impl MaxSizeScope for BuildScope<'_> {
    fn max_width(&self, value: Dp) {
        let v = self.px(value);
        self.write(MaxWidthId, |p| p.set_max_width(v));
    }
    fn max_height(&self, value: Dp) {
        let v = self.px(value);
        self.write(MaxHeightId, |p| p.set_max_height(v));
    }
}

impl AlphaScope for BuildScope<'_> {
    fn alpha(&self, value: f32) {
        self.write(AlphaId, |p| p.set_alpha(value));
    }
}

impl ScaleScope for BuildScope<'_> {
    fn scale_x(&self, value: f32) {
        self.write(ScaleXId, |p| p.set_scale_x(value));
    }
    fn scale_y(&self, value: f32) {
        self.write(ScaleYId, |p| p.set_scale_y(value));
    }
}

impl TranslationScope for BuildScope<'_> {
    fn translation_x(&self, value: f32) {
        self.write(TranslationXId, |p| p.set_translation_x(value));
    }
    fn translation_y(&self, value: f32) {
        self.write(TranslationYId, |p| p.set_translation_y(value));
    }
}

impl RotationScope for BuildScope<'_> {
    fn rotation_x(&self, value: f32) {
        self.write(RotationXId, |p| p.set_rotation_x(value));
    }
    fn rotation_y(&self, value: f32) {
        self.write(RotationYId, |p| p.set_rotation_y(value));
    }
    fn rotation_z(&self, value: f32) {
        self.write(RotationZId, |p| p.set_rotation_z(value));
    }
}

impl TransformOriginScope for BuildScope<'_> {
    fn transform_origin_x(&self, value: f32) {
        self.write(TransformOriginXId, |p| p.set_transform_origin_x(value));
    }
    fn transform_origin_y(&self, value: f32) {
        self.write(TransformOriginYId, |p| p.set_transform_origin_y(value));
    }
}

impl ColorFilterScope for BuildScope<'_> {
    fn color_filter(&self, value: Option<ColorFilter>) {
        self.write_object(ColorFilterId, |p| p.set_color_filter(value));
    }
}

impl ClipScope for BuildScope<'_> {
    fn clip(&self, value: bool) {
        self.write(ClipId, |p| p.set_clip(value));
    }
}

impl ZIndexScope for BuildScope<'_> {
    fn z_index(&self, value: f32) {
        self.write(ZIndexId, |p| p.set_z_index(value));
    }
}

impl BackgroundScope for BuildScope<'_> {
    fn background(&self, color: Color) {
        self.write_both(BackgroundColorId, BackgroundBrushId, |p| p.set_background_color(color));
    }
    fn background_brush(&self, value: Brush) {
        self.write_both(BackgroundColorId, BackgroundBrushId, |p| p.set_background_brush(Some(value)));
    }
}

impl ForegroundScope for BuildScope<'_> {
    fn foreground(&self, value: Color) {
        self.write_both(ForegroundColorId, ForegroundBrushId, |p| p.set_foreground_color(value));
    }
    fn foreground_brush(&self, value: Brush) {
        self.write_both(ForegroundColorId, ForegroundBrushId, |p| p.set_foreground_brush(Some(value)));
    }
}

impl ShapeScope for BuildScope<'_> {
    fn shape(&self, value: Arc<dyn Shape>) {
        self.write_object(ShapeId, |p| p.set_shape(value));
    }
}

impl ShadowScope for BuildScope<'_> {
    fn drop_shadow(&self, value: Shadow) {
        self.write_object(DropShadowId, |p| p.set_drop_shadow(Some(ShadowValue::One(value))));
    }
    fn drop_shadows(&self, value: &[Shadow]) {
        let many: Vec<Option<Shadow>> = value.iter().cloned().map(Some).collect();
        self.write_object(DropShadowId, |p| p.set_drop_shadow(Some(ShadowValue::Many(many.into()))));
    }
    fn inner_shadow(&self, value: Shadow) {
        self.write_object(InnerShadowId, |p| p.set_inner_shadow(Some(ShadowValue::One(value))));
    }
    fn inner_shadows(&self, value: &[Shadow]) {
        let many: Vec<Option<Shadow>> = value.iter().cloned().map(Some).collect();
        self.write_object(InnerShadowId, |p| p.set_inner_shadow(Some(ShadowValue::Many(many.into()))));
    }
}

impl TextStyleScope for BuildScope<'_> {
    /// Each property the style specifies, set as its own setter sets it (Compose's `textStyle`).
    fn text_style(&self, value: TextStyle) {
        if value.color.is_specified() {
            self.content_color(value.color);
        }
        if value.font_size.is_specified() {
            self.font_size(value.font_size);
        }
        if value.letter_spacing.is_specified() {
            self.letter_spacing(value.letter_spacing);
        }
        if let Some(brush) = value.brush.clone() {
            self.content_brush(brush);
        }
        if let Some(style) = value.font_style {
            self.font_style(style);
        }
        if let Some(shift) = value.baseline_shift
            && !shift.0.is_nan()
        {
            self.baseline_shift(shift);
        }
        if let Some(weight) = value.font_weight {
            self.font_weight(weight);
        }
        if let Some(decoration) = value.text_decoration {
            self.text_decoration(decoration);
        }
        if let Some(synthesis) = value.font_synthesis {
            self.font_synthesis(synthesis);
        }
        if let Some(family) = value.font_family.clone() {
            self.font_family(family);
        }
        if let Some(indent) = value.text_indent {
            self.text_indent(indent);
        }
        if value.line_height.is_specified() {
            self.line_height(value.line_height);
        }
        if value.line_break.is_specified() {
            self.line_break(value.line_break);
        }
        if value.hyphens.is_specified() {
            self.hyphens(value.hyphens);
        }
        if value.text_direction.is_specified() {
            self.text_direction(value.text_direction);
        }
        if value.text_align.is_specified() {
            self.text_align(value.text_align);
        }
        if let Some(motion) = value.text_motion {
            self.text_motion(motion);
        }
    }
}

impl ContentColorScope for BuildScope<'_> {
    fn content_color(&self, value: Color) {
        self.write_both(ContentColorId, ContentBrushId, |p| p.set_content_color(value));
    }
    fn content_brush(&self, value: Brush) {
        self.write_both(ContentColorId, ContentBrushId, |p| p.set_content_brush(Some(value)));
    }
}

impl TextDecorationScope for BuildScope<'_> {
    fn text_decoration(&self, value: TextDecoration) {
        self.write(TextDecorationId, |p| p.set_text_decoration(value));
    }
}

impl FontFamilyScope for BuildScope<'_> {
    fn font_family(&self, value: FontFamily) {
        self.write_object(FontFamilyId, |p| p.set_font_family(Some(value)));
    }
}

impl TextIndentScope for BuildScope<'_> {
    fn text_indent(&self, value: TextIndent) {
        self.write_object(TextIndentId, |p| p.set_text_indent(value));
    }
}

impl FontSizeScope for BuildScope<'_> {
    fn font_size(&self, value: TextUnit) {
        self.write(FontSizeId, |p| p.set_font_size(value));
    }
}

impl LineHeightScope for BuildScope<'_> {
    fn line_height(&self, value: TextUnit) {
        self.write(LineHeightId, |p| p.set_line_height(value));
    }
}

impl LetterSpacingScope for BuildScope<'_> {
    fn letter_spacing(&self, value: TextUnit) {
        self.write(LetterSpacingId, |p| p.set_letter_spacing(value));
    }
}

impl BaselineShiftScope for BuildScope<'_> {
    fn baseline_shift(&self, value: BaselineShift) {
        self.write(BaselineShiftId, |p| p.set_baseline_shift(value));
    }
}

impl FontWeightScope for BuildScope<'_> {
    fn font_weight(&self, value: FontWeight) {
        self.write(FontWeightId, |p| p.set_font_weight(value));
    }
}

impl FontStyleScope for BuildScope<'_> {
    fn font_style(&self, value: FontStyle) {
        self.write(FontStyleId, |p| p.set_font_style(value));
    }
}

impl TextDirectionScope for BuildScope<'_> {
    fn text_direction(&self, value: TextDirection) {
        self.write(TextDirectionId, |p| p.set_text_direction(value));
    }
}

impl TextAlignScope for BuildScope<'_> {
    fn text_align(&self, value: TextAlign) {
        self.write(TextAlignId, |p| p.set_text_align(value));
    }
}

impl LineBreakScope for BuildScope<'_> {
    fn line_break(&self, value: LineBreak) {
        self.write(LineBreakId, |p| p.set_line_break(value));
    }
}

impl HyphensScope for BuildScope<'_> {
    fn hyphens(&self, value: Hyphens) {
        self.write(HyphensId, |p| p.set_hyphens(value));
    }
}

impl FontSynthesisScope for BuildScope<'_> {
    fn font_synthesis(&self, value: FontSynthesis) {
        self.write(FontSynthesisId, |p| p.set_font_synthesis(value));
    }
}

impl TextMotionScope for BuildScope<'_> {
    fn text_motion(&self, value: TextMotion) {
        self.write_object(TextMotionId, |p| p.set_text_motion(value));
    }
}
