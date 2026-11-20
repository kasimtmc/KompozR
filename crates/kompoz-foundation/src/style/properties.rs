//! What a style resolves to (Compose's StyleProperties.kt): the properties a [`Style`](super::Style) set, each
//! known by an id, the phases each one affects (inner and outer layout, drawing, the layer, text layout and text
//! drawing), what changed between two resolutions, and the interpolation of animated properties between two of
//! them.
//!
//! Compose packs the text "enums" (font style, alignment, direction, hyphens, synthesis, decoration, weight) into
//! one `Int`; they are kept as fields here, their ids saying which are set as Compose's bits do.

use super::animations::StyleAnimations;
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, RectangleShape, Shadow, Shape, SolidColor, TextUnit, lerp, lerp_color,
    lerp_shape,
};
use kompoz_text::{
    BaselineShift, FontFamily, FontStyle, FontSynthesis, FontWeight, Hyphens, LineBreak, TextAlign, TextDecoration,
    TextDirection, TextIndent, TextMotion,
};
use kompoz_ui::layout::TransformOrigin;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- Phases ---------------------------------------------------------------------------------------------------

pub(crate) const InnerLayoutFlag: u32 = 1 << 0;
pub(crate) const DrawFlag: u32 = 1 << 1;
pub(crate) const LayerFlag: u32 = 1 << 2;
pub(crate) const OuterLayoutFlag: u32 = 1 << 3;
pub(crate) const TextLayoutFlag: u32 = 1 << 4;
pub(crate) const TextDrawFlag: u32 = 1 << 5;

/// The phases text inherits (Compose's InheritedFlags).
pub(crate) const InheritedFlags: u32 = TextLayoutFlag | TextDrawFlag;

pub(crate) const PhaseFlagMask: u32 =
    InnerLayoutFlag | DrawFlag | LayerFlag | OuterLayoutFlag | TextDrawFlag | TextLayoutFlag;

// ---- Property ids ---------------------------------------------------------------------------------------------

/// A primitive property's id: its bit in a `u64` set (Compose's `Byte` ids, kept apart from the objects' `Int`
/// ones by their type).
pub(crate) type PrimitiveId = u8;
/// An object property's id: [`FirstObjectProperty`] on, its bit in a `u32` set from there.
pub(crate) type ObjectId = u32;

pub(crate) const ContentPaddingStartId: PrimitiveId = 0;
pub(crate) const ContentPaddingEndId: PrimitiveId = 1;
pub(crate) const ContentPaddingTopId: PrimitiveId = 2;
pub(crate) const ContentPaddingBottomId: PrimitiveId = 3;
pub(crate) const ExternalPaddingStartId: PrimitiveId = 4;
pub(crate) const ExternalPaddingEndId: PrimitiveId = 5;
pub(crate) const ExternalPaddingTopId: PrimitiveId = 6;
pub(crate) const ExternalPaddingBottomId: PrimitiveId = 7;
pub(crate) const BorderWidthId: PrimitiveId = 8;
pub(crate) const WidthId: PrimitiveId = 9;
pub(crate) const HeightId: PrimitiveId = 10;
pub(crate) const WidthFractionId: PrimitiveId = 11;
pub(crate) const HeightFractionId: PrimitiveId = 12;
pub(crate) const LeftId: PrimitiveId = 13;
pub(crate) const TopId: PrimitiveId = 14;
pub(crate) const RightId: PrimitiveId = 15;
pub(crate) const BottomId: PrimitiveId = 16;
pub(crate) const MinWidthId: PrimitiveId = 17;
pub(crate) const MinHeightId: PrimitiveId = 18;
pub(crate) const MaxWidthId: PrimitiveId = 19;
pub(crate) const MaxHeightId: PrimitiveId = 20;
pub(crate) const AlphaId: PrimitiveId = 21;
pub(crate) const ScaleXId: PrimitiveId = 22;
pub(crate) const ScaleYId: PrimitiveId = 23;
pub(crate) const TranslationXId: PrimitiveId = 24;
pub(crate) const TranslationYId: PrimitiveId = 25;
pub(crate) const RotationXId: PrimitiveId = 26;
pub(crate) const RotationYId: PrimitiveId = 27;
pub(crate) const RotationZId: PrimitiveId = 28;
pub(crate) const TransformOriginXId: PrimitiveId = 29;
pub(crate) const TransformOriginYId: PrimitiveId = 30;
pub(crate) const ClipId: PrimitiveId = 31;
pub(crate) const ZIndexId: PrimitiveId = 32;
pub(crate) const CameraDistanceId: PrimitiveId = 33;
pub(crate) const BackgroundColorId: PrimitiveId = 34;
pub(crate) const BorderColorId: PrimitiveId = 35;
pub(crate) const ForegroundColorId: PrimitiveId = 36;
pub(crate) const ContentColorId: PrimitiveId = 37;
pub(crate) const TextDecorationId: PrimitiveId = 38;
pub(crate) const FontWeightId: PrimitiveId = 39;
pub(crate) const FontStyleId: PrimitiveId = 40;
pub(crate) const TextAlignId: PrimitiveId = 41;
pub(crate) const TextDirectionId: PrimitiveId = 42;
pub(crate) const BaselineShiftId: PrimitiveId = 43;
pub(crate) const HyphensId: PrimitiveId = 44;
pub(crate) const FontSynthesisId: PrimitiveId = 45;
pub(crate) const FontSizeId: PrimitiveId = 46;
pub(crate) const LineHeightId: PrimitiveId = 47;
pub(crate) const LetterSpacingId: PrimitiveId = 48;
pub(crate) const LineBreakId: PrimitiveId = 49;
pub(crate) const PrimitivePropertyCount: u32 = 50;

/// The text "enums" (Compose's TextEnumMask, its font size among them).
const TextEnumMask: u64 = (1 << FontSizeId)
    | (1 << TextDecorationId)
    | (1 << TextAlignId)
    | (1 << TextDirectionId)
    | (1 << HyphensId)
    | (1 << FontStyleId)
    | (1 << FontSynthesisId)
    | (1 << FontWeightId);

pub(crate) const FirstObjectProperty: ObjectId = PrimitivePropertyCount;
pub(crate) const BorderBrushId: ObjectId = FirstObjectProperty;
pub(crate) const BackgroundBrushId: ObjectId = FirstObjectProperty + 1;
pub(crate) const ForegroundBrushId: ObjectId = FirstObjectProperty + 2;
pub(crate) const ShapeId: ObjectId = FirstObjectProperty + 3;
pub(crate) const ColorFilterId: ObjectId = FirstObjectProperty + 4;
pub(crate) const DropShadowId: ObjectId = FirstObjectProperty + 5;
pub(crate) const InnerShadowId: ObjectId = FirstObjectProperty + 6;
pub(crate) const ContentBrushId: ObjectId = FirstObjectProperty + 7;
pub(crate) const FontFamilyId: ObjectId = FirstObjectProperty + 8;
pub(crate) const TextMotionId: ObjectId = FirstObjectProperty + 9;
pub(crate) const TextIndentId: ObjectId = FirstObjectProperty + 10;
pub(crate) const LastObjectProperty: ObjectId = TextIndentId;
pub(crate) const ObjectPropertyCount: u32 = LastObjectProperty - FirstObjectProperty + 1;
pub(crate) const PropertyCount: u32 = PrimitivePropertyCount + ObjectPropertyCount;

/// The phases each property affects (Compose's propertyFlags, built by its buildFlags).
const PROPERTY_FLAGS: [u32; PropertyCount as usize] = build_flags();

const fn build_flags() -> [u32; PropertyCount as usize] {
    let mut f = [0u32; PropertyCount as usize];
    f[ContentPaddingTopId as usize] |= InnerLayoutFlag;
    f[ContentPaddingBottomId as usize] |= InnerLayoutFlag;
    f[ContentPaddingStartId as usize] |= InnerLayoutFlag;
    f[ContentPaddingEndId as usize] |= InnerLayoutFlag;
    f[ExternalPaddingTopId as usize] |= OuterLayoutFlag;
    f[ExternalPaddingBottomId as usize] |= OuterLayoutFlag;
    f[ExternalPaddingStartId as usize] |= OuterLayoutFlag;
    f[ExternalPaddingEndId as usize] |= OuterLayoutFlag;
    f[BorderWidthId as usize] |= DrawFlag | InnerLayoutFlag;
    f[BorderColorId as usize] |= DrawFlag;
    f[BorderBrushId as usize] |= DrawFlag;
    f[WidthId as usize] |= OuterLayoutFlag;
    f[HeightId as usize] |= OuterLayoutFlag;
    f[WidthFractionId as usize] |= OuterLayoutFlag;
    f[HeightFractionId as usize] |= OuterLayoutFlag;
    f[LeftId as usize] |= OuterLayoutFlag;
    f[TopId as usize] |= OuterLayoutFlag;
    f[RightId as usize] |= OuterLayoutFlag;
    f[BottomId as usize] |= OuterLayoutFlag;
    f[MinWidthId as usize] |= OuterLayoutFlag;
    f[MinHeightId as usize] |= OuterLayoutFlag;
    f[MaxWidthId as usize] |= OuterLayoutFlag;
    f[MaxHeightId as usize] |= OuterLayoutFlag;
    f[AlphaId as usize] |= LayerFlag;
    f[ScaleXId as usize] |= LayerFlag;
    f[ScaleYId as usize] |= LayerFlag;
    f[TranslationXId as usize] |= LayerFlag;
    f[TranslationYId as usize] |= LayerFlag;
    f[TransformOriginXId as usize] |= LayerFlag;
    f[TransformOriginYId as usize] |= LayerFlag;
    f[RotationXId as usize] |= LayerFlag;
    f[RotationYId as usize] |= LayerFlag;
    f[RotationZId as usize] |= LayerFlag;
    f[ZIndexId as usize] |= LayerFlag;
    f[BackgroundColorId as usize] |= DrawFlag;
    f[BackgroundBrushId as usize] |= DrawFlag;
    f[ForegroundColorId as usize] |= DrawFlag;
    f[ForegroundBrushId as usize] |= DrawFlag;
    f[ClipId as usize] |= LayerFlag;
    f[ShapeId as usize] |= DrawFlag;
    f[ColorFilterId as usize] |= LayerFlag;
    f[DropShadowId as usize] |= DrawFlag;
    f[InnerShadowId as usize] |= DrawFlag;
    f[ContentColorId as usize] |= TextDrawFlag;
    f[ContentBrushId as usize] |= TextDrawFlag;
    let text = TextLayoutFlag | TextDrawFlag;
    f[FontFamilyId as usize] |= text;
    f[TextMotionId as usize] |= text;
    f[TextIndentId as usize] |= text;
    f[FontSizeId as usize] |= text;
    f[LineHeightId as usize] |= text;
    f[LetterSpacingId as usize] |= text;
    f[BaselineShiftId as usize] |= text;
    f[LineBreakId as usize] |= text;
    f[FontWeightId as usize] |= text;
    f[FontStyleId as usize] |= text;
    f[TextAlignId as usize] |= text;
    f[TextDirectionId as usize] |= text;
    f[HyphensId as usize] |= text;
    f[FontSynthesisId as usize] |= text;
    f[TextDecorationId as usize] |= text;
    f
}

const fn primitive_flags_of(flag: u32) -> u64 {
    let mut flags = 0u64;
    let mut id = 0;
    while id < PrimitivePropertyCount {
        if flag & PROPERTY_FLAGS[id as usize] != 0 {
            flags |= 1 << id;
        }
        id += 1;
    }
    flags
}

const fn object_flags_of(flag: u32) -> u32 {
    let mut flags = 0u32;
    let mut id = FirstObjectProperty;
    while id <= LastObjectProperty {
        if flag & PROPERTY_FLAGS[id as usize] != 0 {
            flags |= 1 << (id - FirstObjectProperty);
        }
        id += 1;
    }
    flags
}

pub(crate) const InnerLayoutPrimitiveFlags: u64 = primitive_flags_of(InnerLayoutFlag);
pub(crate) const OuterLayoutPrimitiveFlags: u64 = primitive_flags_of(OuterLayoutFlag);
pub(crate) const DrawPrimitiveFlags: u64 = primitive_flags_of(DrawFlag);
pub(crate) const LayerPrimitiveFlags: u64 = primitive_flags_of(LayerFlag);
pub(crate) const TextDrawPrimitiveFlags: u64 = primitive_flags_of(TextDrawFlag);
pub(crate) const TextLayoutPrimitiveFlags: u64 = primitive_flags_of(TextLayoutFlag);

pub(crate) const InnerLayoutObjectFlags: u32 = object_flags_of(InnerLayoutFlag);
pub(crate) const OuterLayoutObjectFlags: u32 = object_flags_of(OuterLayoutFlag);
pub(crate) const DrawObjectFlags: u32 = object_flags_of(DrawFlag);
pub(crate) const LayerObjectFlags: u32 = object_flags_of(LayerFlag);
pub(crate) const TextDrawObjectFlags: u32 = object_flags_of(TextDrawFlag);
pub(crate) const TextLayoutObjectFlags: u32 = object_flags_of(TextLayoutFlag);

/// The phases a set of primitive properties affects.
pub(crate) fn primitive_phase_flags_of(primitives_set: u64) -> u32 {
    let mut flags = 0;
    if primitives_set & InnerLayoutPrimitiveFlags != 0 {
        flags |= InnerLayoutFlag;
    }
    if primitives_set & OuterLayoutPrimitiveFlags != 0 {
        flags |= OuterLayoutFlag;
    }
    if primitives_set & DrawPrimitiveFlags != 0 {
        flags |= DrawFlag;
    }
    if primitives_set & LayerPrimitiveFlags != 0 {
        flags |= LayerFlag;
    }
    if primitives_set & TextDrawPrimitiveFlags != 0 {
        flags |= TextDrawFlag;
    }
    if primitives_set & TextLayoutPrimitiveFlags != 0 {
        flags |= TextLayoutFlag;
    }
    flags
}

/// The phases a set of object properties affects.
pub(crate) fn object_phase_flags_of(objects_set: u32) -> u32 {
    // (no object property affects the layouts: Compose's checks of InnerLayoutObjectFlags and OuterLayoutObjectFlags
    // are always false)
    const _: () = assert!(InnerLayoutObjectFlags == 0 && OuterLayoutObjectFlags == 0);
    let mut flags = 0;
    if objects_set & DrawObjectFlags != 0 {
        flags |= DrawFlag;
    }
    if objects_set & LayerObjectFlags != 0 {
        flags |= LayerFlag;
    }
    if objects_set & TextDrawObjectFlags != 0 {
        flags |= TextDrawFlag;
    }
    if objects_set & TextLayoutObjectFlags != 0 {
        flags |= TextLayoutFlag;
    }
    flags
}

/// The primitive properties affecting the phases `flags`.
pub(crate) fn primitives_set_for_flags(flags: u32) -> u64 {
    let mut set = 0;
    if flags & InnerLayoutFlag != 0 {
        set |= InnerLayoutPrimitiveFlags;
    }
    if flags & OuterLayoutFlag != 0 {
        set |= OuterLayoutPrimitiveFlags;
    }
    if flags & DrawFlag != 0 {
        set |= DrawPrimitiveFlags;
    }
    if flags & LayerFlag != 0 {
        set |= LayerPrimitiveFlags;
    }
    if flags & TextDrawFlag != 0 {
        set |= TextDrawPrimitiveFlags;
    }
    if flags & TextLayoutFlag != 0 {
        set |= TextLayoutPrimitiveFlags;
    }
    set
}

/// The object properties affecting the phases `flags`.
pub(crate) fn objects_set_for_flags(flags: u32) -> u32 {
    let mut set = 0;
    if flags & InnerLayoutFlag != 0 {
        set |= InnerLayoutObjectFlags;
    }
    if flags & OuterLayoutFlag != 0 {
        set |= OuterLayoutObjectFlags;
    }
    if flags & DrawFlag != 0 {
        set |= DrawObjectFlags;
    }
    if flags & LayerFlag != 0 {
        set |= LayerObjectFlags;
    }
    if flags & TextDrawFlag != 0 {
        set |= TextDrawObjectFlags;
    }
    if flags & TextLayoutFlag != 0 {
        set |= TextLayoutObjectFlags;
    }
    set
}

// ---- Id sets --------------------------------------------------------------------------------------------------

/// Whether a primitive id is in a set.
pub(crate) fn has_primitive(set: u64, id: PrimitiveId) -> bool {
    (id as u32) < PrimitivePropertyCount && set & (1u64 << id) != 0
}

/// Whether an object id is in a set.
pub(crate) fn has_object(set: u32, id: ObjectId) -> bool {
    id >= FirstObjectProperty && set & (1u32 << (id - FirstObjectProperty)) != 0
}

pub(crate) fn with_primitive(set: u64, id: PrimitiveId) -> u64 {
    set | (1u64 << id)
}

pub(crate) fn without_primitive(set: u64, id: PrimitiveId) -> u64 {
    set & !(1u64 << id)
}

pub(crate) fn with_object(set: u32, id: ObjectId) -> u32 {
    set | (1u32 << (id - FirstObjectProperty))
}

pub(crate) fn without_object(set: u32, id: ObjectId) -> u32 {
    set & !(1u32 << (id - FirstObjectProperty))
}

// ---- Shadows --------------------------------------------------------------------------------------------------

/// A drop or inner shadow property: one shadow, or several (Compose's `Shadow` or `Array<Shadow>` held as `Any?`;
/// several interpolated may leave a place empty).
#[derive(Clone, Debug)]
pub(crate) enum ShadowValue {
    One(Shadow),
    Many(Rc<[Option<Shadow>]>),
}

/// Compose compares the property with `==`: a shadow by its value, an array by its identity.
impl PartialEq for ShadowValue {
    fn eq(&self, other: &ShadowValue) -> bool {
        match (self, other) {
            (ShadowValue::One(a), ShadowValue::One(b)) => a == b,
            (ShadowValue::Many(a), ShadowValue::Many(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

// ---- The properties -------------------------------------------------------------------------------------------

/// The properties a style resolved to (Compose's StyleProperties): which are set (`primitives_set`,
/// `objects_set`), and their values; an unset one keeps its default.
#[derive(Clone)]
pub(crate) struct StyleProperties {
    pub(crate) primitives_set: u64,
    pub(crate) objects_set: u32,

    pub(crate) content_padding_start: f32,
    pub(crate) content_padding_end: f32,
    pub(crate) content_padding_top: f32,
    pub(crate) content_padding_bottom: f32,
    pub(crate) external_padding_start: f32,
    pub(crate) external_padding_end: f32,
    pub(crate) external_padding_top: f32,
    pub(crate) external_padding_bottom: f32,
    pub(crate) border_width: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) width_fraction: f32,
    pub(crate) height_fraction: f32,
    pub(crate) left: f32,
    pub(crate) top: f32,
    pub(crate) right: f32,
    pub(crate) bottom: f32,
    pub(crate) min_height: f32,
    pub(crate) max_height: f32,
    pub(crate) min_width: f32,
    pub(crate) max_width: f32,

    // (drawing)
    pub(crate) border_color: Color,
    pub(crate) border_brush: Option<Brush>,
    pub(crate) background_color: Color,
    pub(crate) background_brush: Option<Brush>,
    pub(crate) foreground_color: Color,
    pub(crate) foreground_brush: Option<Brush>,
    pub(crate) clip: bool,
    pub(crate) shape: Arc<dyn Shape>,
    pub(crate) drop_shadow: Option<ShadowValue>,
    pub(crate) inner_shadow: Option<ShadowValue>,

    // (the layer)
    pub(crate) alpha: f32,
    pub(crate) scale_x: f32,
    pub(crate) scale_y: f32,
    pub(crate) translation_x: f32,
    pub(crate) translation_y: f32,
    pub(crate) rotation_x: f32,
    pub(crate) rotation_y: f32,
    pub(crate) rotation_z: f32,
    pub(crate) transform_origin_x: f32,
    pub(crate) transform_origin_y: f32,
    pub(crate) camera_distance: f32,
    pub(crate) z_index: f32,
    pub(crate) color_filter: Option<ColorFilter>,

    // (text, drawing only)
    pub(crate) content_color: Color,
    pub(crate) content_brush: Option<Brush>,

    // (text, its layout)
    pub(crate) font_family: Option<FontFamily>,
    pub(crate) text_motion: TextMotion,
    pub(crate) text_indent: Option<TextIndent>,
    pub(crate) font_size: TextUnit,
    pub(crate) line_height: TextUnit,
    pub(crate) letter_spacing: TextUnit,
    pub(crate) baseline_shift: BaselineShift,
    pub(crate) line_break: LineBreak,
    // (Compose's packed `textEnums`, read through their getters)
    font_style_value: FontStyle,
    text_align_value: TextAlign,
    text_direction_value: TextDirection,
    hyphens_value: Hyphens,
    font_synthesis_value: FontSynthesis,
    text_decoration_value: TextDecoration,
    font_weight_value: FontWeight,
}

impl Default for StyleProperties {
    fn default() -> StyleProperties {
        StyleProperties {
            primitives_set: 0,
            objects_set: 0,
            content_padding_start: 0.0,
            content_padding_end: 0.0,
            content_padding_top: 0.0,
            content_padding_bottom: 0.0,
            external_padding_start: 0.0,
            external_padding_end: 0.0,
            external_padding_top: 0.0,
            external_padding_bottom: 0.0,
            border_width: 0.0,
            width: f32::NAN,
            height: f32::NAN,
            width_fraction: f32::NAN,
            height_fraction: f32::NAN,
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            min_height: f32::NAN,
            max_height: f32::NAN,
            min_width: f32::NAN,
            max_width: f32::NAN,
            border_color: Color::Black,
            border_brush: None,
            background_color: Color::Transparent,
            background_brush: None,
            foreground_color: Color::Unspecified,
            foreground_brush: None,
            clip: false,
            shape: Arc::new(RectangleShape),
            drop_shadow: None,
            inner_shadow: None,
            alpha: 1.0,
            scale_x: 1.0,
            scale_y: 1.0,
            translation_x: 0.0,
            translation_y: 0.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            transform_origin_x: TransformOrigin::Center.pivot_fraction_x,
            transform_origin_y: TransformOrigin::Center.pivot_fraction_y,
            camera_distance: 1.0,
            z_index: 0.0,
            color_filter: None,
            content_color: Color::Unspecified,
            content_brush: None,
            font_family: None,
            text_motion: TextMotion::Static,
            text_indent: None,
            font_size: TextUnit::Unspecified,
            line_height: TextUnit::Unspecified,
            letter_spacing: TextUnit::Unspecified,
            baseline_shift: BaselineShift::Unspecified,
            line_break: LineBreak::Unspecified,
            font_style_value: FontStyle::Normal,
            text_align_value: TextAlign::Unspecified,
            text_direction_value: TextDirection::Unspecified,
            hyphens_value: Hyphens::Unspecified,
            font_synthesis_value: FontSynthesis::None,
            text_decoration_value: TextDecoration::None,
            font_weight_value: FontWeight { weight: 0 },
        }
    }
}

/// Compose's `valueElements` (the inspector's view): the properties set, by name.
impl fmt::Debug for StyleProperties {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut d = f.debug_map();
        for (name, value) in self.value_elements(u64::MAX, u32::MAX) {
            d.entry(&name, &value);
        }
        d.finish()
    }
}

impl StyleProperties {
    /// Back to nothing set (Compose's `clear`).
    pub(crate) fn clear(&mut self) {
        *self = StyleProperties::default();
    }

    /// The properties of `flags`' phases that this one set, set on `target` (Compose's `copyInto(target, flags)`).
    pub(crate) fn copy_into_flags(&self, target: &mut StyleProperties, flags: u32) {
        self.copy_into_filtered(target, primitive_flags_of(flags), objects_set_for_flags(flags));
    }

    /// The properties in the filters that this one set, set on `target` through its setters (Compose's
    /// `copyInto(target, primitivesFilter, objectsFilter)`).
    pub(crate) fn copy_into_filtered(&self, target: &mut StyleProperties, primitives_filter: u64, objects_filter: u32) {
        let p = self.primitives_set & primitives_filter;
        if p != 0 {
            if has_primitive(p, LeftId) {
                target.set_left(self.left);
            }
            if has_primitive(p, TopId) {
                target.set_top(self.top);
            }
            if has_primitive(p, RightId) {
                target.set_right(self.right);
            }
            if has_primitive(p, BottomId) {
                target.set_bottom(self.bottom);
            }
            if has_primitive(p, MinHeightId) {
                target.set_min_height(self.min_height);
            }
            if has_primitive(p, MaxHeightId) {
                target.set_max_height(self.max_height);
            }
            if has_primitive(p, MinWidthId) {
                target.set_min_width(self.min_width);
            }
            if has_primitive(p, MaxWidthId) {
                target.set_max_width(self.max_width);
            }
            if has_primitive(p, ContentPaddingStartId) {
                target.set_content_padding_start(self.content_padding_start);
            }
            if has_primitive(p, ContentPaddingEndId) {
                target.set_content_padding_end(self.content_padding_end);
            }
            if has_primitive(p, ContentPaddingTopId) {
                target.set_content_padding_top(self.content_padding_top);
            }
            if has_primitive(p, ContentPaddingBottomId) {
                target.set_content_padding_bottom(self.content_padding_bottom);
            }
            if has_primitive(p, ExternalPaddingStartId) {
                target.set_external_padding_start(self.external_padding_start);
            }
            if has_primitive(p, ExternalPaddingEndId) {
                target.set_external_padding_end(self.external_padding_end);
            }
            if has_primitive(p, ExternalPaddingTopId) {
                target.set_external_padding_top(self.external_padding_top);
            }
            if has_primitive(p, ExternalPaddingBottomId) {
                target.set_external_padding_bottom(self.external_padding_bottom);
            }
            if has_primitive(p, BorderWidthId) {
                target.set_border_width(self.border_width);
            }
            if has_primitive(p, AlphaId) {
                target.set_alpha(self.alpha);
            }
            if has_primitive(p, ScaleXId) {
                target.set_scale_x(self.scale_x);
            }
            if has_primitive(p, ScaleYId) {
                target.set_scale_y(self.scale_y);
            }
            if has_primitive(p, TranslationXId) {
                target.set_translation_x(self.translation_x);
            }
            if has_primitive(p, TranslationYId) {
                target.set_translation_y(self.translation_y);
            }
            if has_primitive(p, RotationXId) {
                target.set_rotation_x(self.rotation_x);
            }
            if has_primitive(p, RotationYId) {
                target.set_rotation_y(self.rotation_y);
            }
            if has_primitive(p, RotationZId) {
                target.set_rotation_z(self.rotation_z);
            }
            if has_primitive(p, TransformOriginXId) {
                target.set_transform_origin_x(self.transform_origin_x);
            }
            if has_primitive(p, TransformOriginYId) {
                target.set_transform_origin_y(self.transform_origin_y);
            }
            if has_primitive(p, ZIndexId) {
                target.set_z_index(self.z_index);
            }
            if has_primitive(p, CameraDistanceId) {
                target.set_camera_distance(self.camera_distance);
            }
            if has_primitive(p, BorderColorId) {
                target.set_border_color(self.border_color);
            }
            if has_primitive(p, BackgroundColorId) {
                target.set_background_color(self.background_color);
            }
            if has_primitive(p, ForegroundColorId) {
                target.set_foreground_color(self.foreground_color);
            }
            if has_primitive(p, ClipId) {
                target.set_clip(self.clip);
            }
            if has_primitive(p, WidthId) {
                target.set_width(self.width);
            }
            if has_primitive(p, HeightId) {
                target.set_height(self.height);
            }
            if has_primitive(p, WidthFractionId) {
                target.set_width_fraction(self.width_fraction);
            }
            if has_primitive(p, HeightFractionId) {
                target.set_height_fraction(self.height_fraction);
            }
            if has_primitive(p, ContentColorId) {
                target.set_content_color(self.content_color);
            }
            if has_primitive(p, LineHeightId) {
                target.set_line_height(self.line_height);
            }
            if has_primitive(p, LetterSpacingId) {
                target.set_letter_spacing(self.letter_spacing);
            }
            if has_primitive(p, BaselineShiftId) {
                target.set_baseline_shift(self.baseline_shift);
            }
            if has_primitive(p, LineBreakId) {
                target.set_line_break(self.line_break);
            }
            if p & TextEnumMask != 0 {
                if has_primitive(p, TextDecorationId) {
                    target.set_text_decoration(self.text_decoration());
                }
                if has_primitive(p, FontSizeId) {
                    target.set_font_size(self.font_size);
                }
                if has_primitive(p, TextAlignId) {
                    target.set_text_align(self.text_align());
                }
                if has_primitive(p, TextDirectionId) {
                    target.set_text_direction(self.text_direction());
                }
                if has_primitive(p, HyphensId) {
                    target.set_hyphens(self.hyphens());
                }
                if has_primitive(p, FontSynthesisId) {
                    target.set_font_synthesis(self.font_synthesis());
                }
                if has_primitive(p, FontWeightId) {
                    target.set_font_weight(self.font_weight());
                }
                if has_primitive(p, FontStyleId) {
                    target.set_font_style(self.font_style());
                }
            }
        }
        let o = objects_filter & self.objects_set;
        if o != 0 {
            if has_object(o, ShapeId) {
                target.set_shape(self.shape.clone());
            }
            if has_object(o, ColorFilterId) {
                target.set_color_filter(self.color_filter);
            }
            if has_object(o, BorderBrushId) {
                target.set_border_brush(self.border_brush.clone());
            }
            if has_object(o, BackgroundBrushId) {
                target.set_background_brush(self.background_brush.clone());
            }
            if has_object(o, ForegroundBrushId) {
                target.set_foreground_brush(self.foreground_brush.clone());
            }
            if has_object(o, DropShadowId) {
                target.set_drop_shadow(self.drop_shadow.clone());
            }
            if has_object(o, InnerShadowId) {
                target.set_inner_shadow(self.inner_shadow.clone());
            }
            if has_object(o, ContentBrushId) {
                target.set_content_brush(self.content_brush.clone());
            }
            if has_object(o, FontFamilyId) {
                target.set_font_family(self.font_family.clone());
            }
            if has_object(o, TextMotionId) {
                target.set_text_motion(self.text_motion);
            }
            if has_object(o, TextIndentId) {
                target.set_text_indent(self.text_indent.expect("kompoz: a text indent set"));
            }
        }
    }

    /// The primitive properties (in `filter`) that differ from `other`'s: set in one only, or to other values
    /// (Compose's `diffPrimitives`; floats compared by their bits).
    pub(crate) fn diff_primitives(&self, other: &StyleProperties, filter: u64) -> u64 {
        let both = (self.primitives_set & other.primitives_set) & filter;
        let exclusive = (self.primitives_set ^ other.primitives_set) & filter;
        if both == 0 {
            return exclusive;
        }
        let mut c = both;
        let mut float = |id: PrimitiveId, a: f32, b: f32| {
            if has_primitive(c, id) && a.to_bits() == b.to_bits() {
                c = without_primitive(c, id);
            }
        };
        float(ContentPaddingStartId, self.content_padding_start, other.content_padding_start);
        float(ContentPaddingEndId, self.content_padding_end, other.content_padding_end);
        float(ContentPaddingTopId, self.content_padding_top, other.content_padding_top);
        float(ContentPaddingBottomId, self.content_padding_bottom, other.content_padding_bottom);
        float(ExternalPaddingStartId, self.external_padding_start, other.external_padding_start);
        float(ExternalPaddingEndId, self.external_padding_end, other.external_padding_end);
        float(ExternalPaddingTopId, self.external_padding_top, other.external_padding_top);
        float(ExternalPaddingBottomId, self.external_padding_bottom, other.external_padding_bottom);
        float(BorderWidthId, self.border_width, other.border_width);
        float(WidthId, self.width, other.width);
        float(HeightId, self.height, other.height);
        float(WidthFractionId, self.width_fraction, other.width_fraction);
        float(HeightFractionId, self.height_fraction, other.height_fraction);
        float(LeftId, self.left, other.left);
        float(TopId, self.top, other.top);
        float(RightId, self.right, other.right);
        float(BottomId, self.bottom, other.bottom);
        float(MinWidthId, self.min_width, other.min_width);
        float(MinHeightId, self.min_height, other.min_height);
        float(MaxWidthId, self.max_width, other.max_width);
        float(MaxHeightId, self.max_height, other.max_height);
        float(AlphaId, self.alpha, other.alpha);
        float(ScaleXId, self.scale_x, other.scale_x);
        float(ScaleYId, self.scale_y, other.scale_y);
        float(TranslationXId, self.translation_x, other.translation_x);
        float(TranslationYId, self.translation_y, other.translation_y);
        float(RotationXId, self.rotation_x, other.rotation_x);
        float(RotationYId, self.rotation_y, other.rotation_y);
        float(RotationZId, self.rotation_z, other.rotation_z);
        float(TransformOriginXId, self.transform_origin_x, other.transform_origin_x);
        float(TransformOriginYId, self.transform_origin_y, other.transform_origin_y);
        float(ZIndexId, self.z_index, other.z_index);
        float(CameraDistanceId, self.camera_distance, other.camera_distance);
        let mut same = |id: PrimitiveId, equal: bool| {
            if has_primitive(c, id) && equal {
                c = without_primitive(c, id);
            }
        };
        same(ClipId, self.clip == other.clip);
        same(BackgroundColorId, self.background_color == other.background_color);
        same(BorderColorId, self.border_color == other.border_color);
        same(ForegroundColorId, self.foreground_color == other.foreground_color);
        same(ContentColorId, self.content_color == other.content_color);
        same(TextDecorationId, self.text_decoration() == other.text_decoration());
        same(FontWeightId, self.font_weight() == other.font_weight());
        same(FontStyleId, self.font_style() == other.font_style());
        same(TextAlignId, self.text_align() == other.text_align());
        same(TextDirectionId, self.text_direction() == other.text_direction());
        same(BaselineShiftId, self.baseline_shift == other.baseline_shift);
        same(HyphensId, self.hyphens() == other.hyphens());
        same(FontSynthesisId, self.font_synthesis() == other.font_synthesis());
        same(FontSizeId, self.font_size == other.font_size);
        same(LineHeightId, self.line_height == other.line_height);
        same(LetterSpacingId, self.letter_spacing == other.letter_spacing);
        same(LineBreakId, self.line_break == other.line_break);
        c | exclusive
    }

    /// The object properties (in `filter`) that differ from `other`'s (Compose's `diffObjects`).
    pub(crate) fn diff_objects(&self, other: &StyleProperties, filter: u32) -> u32 {
        let both = (self.objects_set & other.objects_set) & filter;
        let exclusive = (self.objects_set ^ other.objects_set) & filter;
        if both == 0 {
            return exclusive;
        }
        let mut c = both;
        let mut same = |id: ObjectId, equal: bool| {
            if has_object(c, id) && equal {
                c = without_object(c, id);
            }
        };
        same(BorderBrushId, self.border_brush == other.border_brush);
        same(BackgroundBrushId, self.background_brush == other.background_brush);
        same(ForegroundBrushId, self.foreground_brush == other.foreground_brush);
        same(ShapeId, *self.shape == *other.shape);
        same(ColorFilterId, self.color_filter == other.color_filter);
        same(DropShadowId, self.drop_shadow == other.drop_shadow);
        same(InnerShadowId, self.inner_shadow == other.inner_shadow);
        same(ContentBrushId, self.content_brush == other.content_brush);
        same(FontFamilyId, self.font_family == other.font_family);
        same(TextMotionId, self.text_motion == other.text_motion);
        same(TextIndentId, self.text_indent == other.text_indent);
        c | exclusive
    }

    /// The phases (of `filter_flags`) whose properties differ from `other`'s (Compose's `diff`): a shape change
    /// is a layer change too where either has layer properties (the layer is made again for its outline).
    pub(crate) fn diff(&self, other: &StyleProperties, filter_flags: u32) -> u32 {
        let primitive_changes = self.diff_primitives(other, primitives_set_for_flags(filter_flags));
        let object_changes = self.diff_objects(other, objects_set_for_flags(filter_flags));
        let mut phases = primitive_phase_flags_of(primitive_changes) | object_phase_flags_of(object_changes);
        if filter_flags & (LayerFlag | DrawFlag) != 0
            && has_object(object_changes, ShapeId)
            && (self.has_layer_properties() || other.has_layer_properties())
        {
            phases |= LayerFlag;
        }
        phases
    }

    pub(crate) fn has_layer_properties(&self) -> bool {
        self.primitives_set & LayerPrimitiveFlags != 0 || self.objects_set & LayerObjectFlags != 0
    }

    pub(crate) fn has(&self, id: PrimitiveId) -> bool {
        has_primitive(self.primitives_set, id)
    }

    pub(crate) fn has_obj(&self, id: ObjectId) -> bool {
        has_object(self.objects_set, id)
    }

    /// The phases its properties affect (Compose's `phaseFlags`).
    pub(crate) fn phase_flags(&self) -> u32 {
        primitive_phase_flags_of(self.primitives_set) | object_phase_flags_of(self.objects_set)
    }

    // ---- Setters (Compose's property functions: each marks its property set; a color and its brush exclude
    // each other) ----

    pub(crate) fn set_content_padding_start(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ContentPaddingStartId);
        self.content_padding_start = v;
    }
    pub(crate) fn set_content_padding_end(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ContentPaddingEndId);
        self.content_padding_end = v;
    }
    pub(crate) fn set_content_padding_top(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ContentPaddingTopId);
        self.content_padding_top = v;
    }
    pub(crate) fn set_content_padding_bottom(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ContentPaddingBottomId);
        self.content_padding_bottom = v;
    }
    pub(crate) fn set_external_padding_start(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ExternalPaddingStartId);
        self.external_padding_start = v;
    }
    pub(crate) fn set_external_padding_end(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ExternalPaddingEndId);
        self.external_padding_end = v;
    }
    pub(crate) fn set_external_padding_top(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ExternalPaddingTopId);
        self.external_padding_top = v;
    }
    pub(crate) fn set_external_padding_bottom(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ExternalPaddingBottomId);
        self.external_padding_bottom = v;
    }
    pub(crate) fn set_border_width(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, BorderWidthId);
        self.border_width = v;
    }
    pub(crate) fn set_border_color(&mut self, v: Color) {
        self.primitives_set = with_primitive(self.primitives_set, BorderColorId);
        self.objects_set = without_object(self.objects_set, BorderBrushId);
        self.border_color = v;
        self.border_brush = None;
    }
    pub(crate) fn set_border_brush(&mut self, v: Option<Brush>) {
        self.primitives_set = without_primitive(self.primitives_set, BorderColorId);
        self.objects_set = if v.is_some() {
            with_object(self.objects_set, BorderBrushId)
        } else {
            without_object(self.objects_set, BorderBrushId)
        };
        self.border_brush = v;
        self.border_color = Color::Unspecified;
    }
    pub(crate) fn set_width(&mut self, v: f32) {
        self.primitives_set = without_primitive(with_primitive(self.primitives_set, WidthId), WidthFractionId);
        self.width = v;
        self.width_fraction = f32::NAN;
    }
    pub(crate) fn set_height(&mut self, v: f32) {
        self.primitives_set = without_primitive(with_primitive(self.primitives_set, HeightId), HeightFractionId);
        self.height = v;
        self.height_fraction = f32::NAN;
    }
    pub(crate) fn set_width_fraction(&mut self, fraction: f32) {
        self.primitives_set = with_primitive(without_primitive(self.primitives_set, WidthId), WidthFractionId);
        self.width_fraction = fraction;
        self.width = f32::NAN;
    }
    pub(crate) fn set_height_fraction(&mut self, fraction: f32) {
        self.primitives_set = with_primitive(without_primitive(self.primitives_set, HeightId), HeightFractionId);
        self.height_fraction = fraction;
        self.height = f32::NAN;
    }
    pub(crate) fn set_left(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, LeftId);
        self.left = v;
    }
    pub(crate) fn set_top(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, TopId);
        self.top = v;
    }
    pub(crate) fn set_right(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, RightId);
        self.right = v;
    }
    pub(crate) fn set_bottom(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, BottomId);
        self.bottom = v;
    }
    pub(crate) fn set_min_width(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, MinWidthId);
        self.min_width = v;
    }
    pub(crate) fn set_min_height(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, MinHeightId);
        self.min_height = v;
    }
    pub(crate) fn set_max_width(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, MaxWidthId);
        self.max_width = v;
    }
    pub(crate) fn set_max_height(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, MaxHeightId);
        self.max_height = v;
    }
    pub(crate) fn set_alpha(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, AlphaId);
        self.alpha = v;
    }
    pub(crate) fn set_scale_x(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ScaleXId);
        self.scale_x = v;
    }
    pub(crate) fn set_scale_y(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ScaleYId);
        self.scale_y = v;
    }
    pub(crate) fn set_translation_x(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, TranslationXId);
        self.translation_x = v;
    }
    pub(crate) fn set_translation_y(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, TranslationYId);
        self.translation_y = v;
    }
    pub(crate) fn set_rotation_x(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, RotationXId);
        self.rotation_x = v;
    }
    pub(crate) fn set_rotation_y(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, RotationYId);
        self.rotation_y = v;
    }
    pub(crate) fn set_rotation_z(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, RotationZId);
        self.rotation_z = v;
    }
    pub(crate) fn set_transform_origin_x(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, TransformOriginXId);
        self.transform_origin_x = v;
    }
    pub(crate) fn set_transform_origin_y(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, TransformOriginYId);
        self.transform_origin_y = v;
    }
    pub(crate) fn set_clip(&mut self, v: bool) {
        self.primitives_set = with_primitive(self.primitives_set, ClipId);
        self.clip = v;
    }
    pub(crate) fn set_z_index(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, ZIndexId);
        self.z_index = v;
    }
    pub(crate) fn set_color_filter(&mut self, v: Option<ColorFilter>) {
        self.objects_set = with_object(self.objects_set, ColorFilterId);
        self.color_filter = v;
    }
    pub(crate) fn set_camera_distance(&mut self, v: f32) {
        self.primitives_set = with_primitive(self.primitives_set, CameraDistanceId);
        self.camera_distance = v;
    }
    pub(crate) fn set_background_color(&mut self, v: Color) {
        self.primitives_set = with_primitive(self.primitives_set, BackgroundColorId);
        self.objects_set = without_object(self.objects_set, BackgroundBrushId);
        self.background_color = v;
        self.background_brush = None;
    }
    pub(crate) fn set_background_brush(&mut self, v: Option<Brush>) {
        self.primitives_set = without_primitive(self.primitives_set, BackgroundColorId);
        self.objects_set = if v.is_some() {
            with_object(self.objects_set, BackgroundBrushId)
        } else {
            without_object(self.objects_set, BackgroundBrushId)
        };
        self.background_brush = v;
        self.background_color = Color::Unspecified;
    }
    pub(crate) fn set_foreground_color(&mut self, v: Color) {
        self.primitives_set = with_primitive(self.primitives_set, ForegroundColorId);
        self.objects_set = without_object(self.objects_set, ForegroundBrushId);
        self.foreground_color = v;
        self.foreground_brush = None;
    }
    pub(crate) fn set_foreground_brush(&mut self, v: Option<Brush>) {
        self.primitives_set = without_primitive(self.primitives_set, ForegroundColorId);
        self.objects_set = if v.is_some() {
            with_object(self.objects_set, ForegroundBrushId)
        } else {
            without_object(self.objects_set, ForegroundBrushId)
        };
        self.foreground_brush = v;
        self.foreground_color = Color::Unspecified;
    }
    pub(crate) fn set_shape(&mut self, v: Arc<dyn Shape>) {
        self.objects_set = with_object(self.objects_set, ShapeId);
        self.shape = v;
    }
    pub(crate) fn set_drop_shadow(&mut self, v: Option<ShadowValue>) {
        self.objects_set = if v.is_some() {
            with_object(self.objects_set, DropShadowId)
        } else {
            without_object(self.objects_set, DropShadowId)
        };
        self.drop_shadow = v;
    }
    pub(crate) fn set_inner_shadow(&mut self, v: Option<ShadowValue>) {
        self.objects_set = if v.is_some() {
            with_object(self.objects_set, InnerShadowId)
        } else {
            without_object(self.objects_set, InnerShadowId)
        };
        self.inner_shadow = v;
    }
    pub(crate) fn set_content_color(&mut self, v: Color) {
        self.primitives_set = with_primitive(self.primitives_set, ContentColorId);
        self.objects_set = without_object(self.objects_set, ContentBrushId);
        self.content_color = v;
        self.content_brush = None;
    }
    pub(crate) fn set_content_brush(&mut self, v: Option<Brush>) {
        self.primitives_set = without_primitive(self.primitives_set, ContentColorId);
        self.objects_set = if v.is_some() {
            with_object(self.objects_set, ContentBrushId)
        } else {
            without_object(self.objects_set, ContentBrushId)
        };
        self.content_brush = v;
        self.content_color = Color::Unspecified;
    }
    pub(crate) fn set_text_decoration(&mut self, v: TextDecoration) {
        self.primitives_set = with_primitive(self.primitives_set, TextDecorationId);
        self.text_decoration_value = v;
    }
    pub(crate) fn set_font_family(&mut self, v: Option<FontFamily>) {
        self.objects_set = with_object(self.objects_set, FontFamilyId);
        self.font_family = v;
    }
    pub(crate) fn set_text_motion(&mut self, v: TextMotion) {
        self.objects_set = with_object(self.objects_set, TextMotionId);
        self.text_motion = v;
    }
    pub(crate) fn set_text_indent(&mut self, v: TextIndent) {
        self.objects_set = with_object(self.objects_set, TextIndentId);
        self.text_indent = Some(v);
    }
    pub(crate) fn set_font_size(&mut self, v: TextUnit) {
        self.primitives_set = with_primitive(self.primitives_set, FontSizeId);
        self.font_size = v;
    }
    pub(crate) fn set_line_height(&mut self, v: TextUnit) {
        self.primitives_set = with_primitive(self.primitives_set, LineHeightId);
        self.line_height = v;
    }
    pub(crate) fn set_letter_spacing(&mut self, v: TextUnit) {
        self.primitives_set = with_primitive(self.primitives_set, LetterSpacingId);
        self.letter_spacing = v;
    }
    pub(crate) fn set_baseline_shift(&mut self, v: BaselineShift) {
        self.primitives_set = with_primitive(self.primitives_set, BaselineShiftId);
        self.baseline_shift = v;
    }
    pub(crate) fn set_line_break(&mut self, v: LineBreak) {
        self.primitives_set = with_primitive(self.primitives_set, LineBreakId);
        self.line_break = v;
    }
    pub(crate) fn set_font_style(&mut self, v: FontStyle) {
        self.primitives_set = with_primitive(self.primitives_set, FontStyleId);
        self.font_style_value = v;
    }
    pub(crate) fn set_text_align(&mut self, v: TextAlign) {
        self.primitives_set = with_primitive(self.primitives_set, TextAlignId);
        self.text_align_value = v;
    }
    pub(crate) fn set_text_direction(&mut self, v: TextDirection) {
        self.primitives_set = with_primitive(self.primitives_set, TextDirectionId);
        self.text_direction_value = v;
    }
    pub(crate) fn set_hyphens(&mut self, v: Hyphens) {
        self.primitives_set = with_primitive(self.primitives_set, HyphensId);
        self.hyphens_value = v;
    }
    pub(crate) fn set_font_weight(&mut self, v: FontWeight) {
        self.primitives_set = with_primitive(self.primitives_set, FontWeightId);
        self.font_weight_value = v;
    }
    pub(crate) fn set_font_synthesis(&mut self, v: FontSynthesis) {
        self.primitives_set = with_primitive(self.primitives_set, FontSynthesisId);
        self.font_synthesis_value = v;
    }

    // ---- The text "enums": their defaults where unset (Compose's getters of the packed bits) ----

    pub(crate) fn font_style(&self) -> FontStyle {
        if self.has(FontStyleId) { self.font_style_value } else { FontStyle::Normal }
    }
    pub(crate) fn text_align(&self) -> TextAlign {
        if self.has(TextAlignId) { self.text_align_value } else { TextAlign::Unspecified }
    }
    pub(crate) fn text_direction(&self) -> TextDirection {
        if self.has(TextDirectionId) { self.text_direction_value } else { TextDirection::Unspecified }
    }
    pub(crate) fn hyphens(&self) -> Hyphens {
        if self.has(HyphensId) { self.hyphens_value } else { Hyphens::Unspecified }
    }
    pub(crate) fn font_weight(&self) -> FontWeight {
        if self.has(FontWeightId) { self.font_weight_value } else { FontWeight::Normal }
    }
    pub(crate) fn font_synthesis(&self) -> FontSynthesis {
        if self.has(FontSynthesisId) { self.font_synthesis_value } else { FontSynthesis::None }
    }
    pub(crate) fn text_decoration(&self) -> TextDecoration {
        if self.has(TextDecorationId) { self.text_decoration_value } else { TextDecoration::None }
    }

    /// The properties set (in the filters), by name (Compose's `valueElements`).
    pub(crate) fn value_elements(&self, primitives_filter: u64, objects_filter: u32) -> Vec<(&'static str, String)> {
        let p = self.primitives_set & primitives_filter;
        let o = self.objects_set & objects_filter;
        let mut out: Vec<(&'static str, String)> = Vec::new();
        let mut add = |set: bool, name: &'static str, value: String| {
            if set {
                out.push((name, value));
            }
        };
        add(has_primitive(p, ContentPaddingStartId), "contentPaddingStart", format!("{}", self.content_padding_start));
        add(has_primitive(p, ContentPaddingEndId), "contentPaddingEnd", format!("{}", self.content_padding_end));
        add(has_primitive(p, ContentPaddingTopId), "contentPaddingTop", format!("{}", self.content_padding_top));
        add(
            has_primitive(p, ContentPaddingBottomId),
            "contentPaddingBottom",
            format!("{}", self.content_padding_bottom),
        );
        add(
            has_primitive(p, ExternalPaddingStartId),
            "externalPaddingStart",
            format!("{}", self.external_padding_start),
        );
        add(has_primitive(p, ExternalPaddingEndId), "externalPaddingEnd", format!("{}", self.external_padding_end));
        add(has_primitive(p, ExternalPaddingTopId), "externalPaddingTop", format!("{}", self.external_padding_top));
        add(
            has_primitive(p, ExternalPaddingBottomId),
            "externalPaddingBottom",
            format!("{}", self.external_padding_bottom),
        );
        add(has_primitive(p, BorderWidthId), "borderWidth", format!("{}", self.border_width));
        add(has_primitive(p, WidthId), "width", format!("{}", self.width));
        add(has_primitive(p, HeightId), "height", format!("{}", self.height));
        add(has_primitive(p, WidthFractionId), "widthFraction", format!("{}", self.width_fraction));
        add(has_primitive(p, HeightFractionId), "heightFraction", format!("{}", self.height_fraction));
        add(has_primitive(p, AlphaId), "alpha", format!("{}", self.alpha));
        add(has_primitive(p, ScaleXId), "scaleX", format!("{}", self.scale_x));
        add(has_primitive(p, ScaleYId), "scaleY", format!("{}", self.scale_y));
        add(has_primitive(p, TranslationXId), "translationX", format!("{}", self.translation_x));
        add(has_primitive(p, TranslationYId), "translationY", format!("{}", self.translation_y));
        add(has_primitive(p, RotationXId), "rotationX", format!("{}", self.rotation_x));
        add(has_primitive(p, RotationYId), "rotationY", format!("{}", self.rotation_y));
        add(has_primitive(p, RotationZId), "rotationZ", format!("{}", self.rotation_z));
        add(has_primitive(p, TransformOriginXId), "transformOriginX", format!("{}", self.transform_origin_x));
        add(has_primitive(p, TransformOriginYId), "transformOriginY", format!("{}", self.transform_origin_y));
        add(has_primitive(p, ZIndexId), "zIndex", format!("{}", self.z_index));
        add(has_object(o, ColorFilterId), "colorFilter", format!("{:?}", self.color_filter));
        add(has_primitive(p, CameraDistanceId), "cameraDistance", format!("{}", self.camera_distance));
        add(has_primitive(p, BorderColorId), "borderColor", format!("{:?}", self.border_color));
        add(has_object(o, BorderBrushId), "borderBrush", format!("{:?}", self.border_brush));
        add(has_primitive(p, BackgroundColorId), "backgroundColor", format!("{:?}", self.background_color));
        add(has_object(o, BackgroundBrushId), "backgroundBrush", format!("{:?}", self.background_brush));
        add(has_primitive(p, ForegroundColorId), "foregroundColor", format!("{:?}", self.foreground_color));
        add(has_object(o, ForegroundBrushId), "foregroundBrush", format!("{:?}", self.foreground_brush));
        add(has_primitive(p, ClipId), "clip", format!("{}", self.clip));
        add(has_object(o, ShapeId), "shape", format!("{:?}", self.shape));
        add(has_primitive(p, ContentColorId), "contentColor", format!("{:?}", self.content_color));
        add(has_object(o, ContentBrushId), "contentBrush", format!("{:?}", self.content_brush));
        add(has_object(o, FontFamilyId), "fontFamily", format!("{:?}", self.font_family));
        add(has_object(o, TextMotionId), "textMotion", format!("{:?}", self.text_motion));
        add(has_object(o, TextIndentId), "textIndent", format!("{:?}", self.text_indent));
        add(has_primitive(p, FontSizeId), "fontSize", format!("{:?}", self.font_size));
        add(has_primitive(p, LineHeightId), "lineHeight", format!("{:?}", self.line_height));
        add(has_primitive(p, LetterSpacingId), "letterSpacing", format!("{:?}", self.letter_spacing));
        add(has_primitive(p, BaselineShiftId), "baselineShift", format!("{:?}", self.baseline_shift));
        add(has_primitive(p, LineBreakId), "lineBreak", format!("{:?}", self.line_break));
        add(has_primitive(p, TextAlignId), "textAlign", format!("{:?}", self.text_align()));
        add(has_primitive(p, TextDirectionId), "textDirection", format!("{:?}", self.text_direction()));
        add(has_primitive(p, HyphensId), "hyphens", format!("{:?}", self.hyphens()));
        add(has_primitive(p, FontStyleId), "fontStyle", format!("{:?}", self.font_style()));
        add(has_primitive(p, FontWeightId), "fontWeight", format!("{:?}", self.font_weight()));
        add(has_primitive(p, FontSynthesisId), "fontSynthesis", format!("{:?}", self.font_synthesis()));
        add(has_primitive(p, TextDecorationId), "textDecoration", format!("{:?}", self.text_decoration()));
        out
    }
}

// ---- Interpolation --------------------------------------------------------------------------------------------

/// Compose's `lerpOuterLayout`: an unset (NaN) end takes the other's value.
fn lerp_outer_layout(
    p: u64,
    a: &StyleProperties,
    b: &StyleProperties,
    anim: &StyleAnimations,
    r: &mut StyleProperties,
) {
    let t = |id: PrimitiveId| anim.time_of(id as u32);
    if has_primitive(p, ExternalPaddingStartId) {
        r.set_external_padding_start(lerp_maybe_nan(
            a.external_padding_start,
            b.external_padding_start,
            t(ExternalPaddingStartId),
        ));
    }
    if has_primitive(p, ExternalPaddingEndId) {
        r.set_external_padding_end(lerp_maybe_nan(
            a.external_padding_end,
            b.external_padding_end,
            t(ExternalPaddingEndId),
        ));
    }
    if has_primitive(p, ExternalPaddingTopId) {
        r.set_external_padding_top(lerp_maybe_nan(
            a.external_padding_top,
            b.external_padding_top,
            t(ExternalPaddingTopId),
        ));
    }
    if has_primitive(p, ExternalPaddingBottomId) {
        r.set_external_padding_bottom(lerp_maybe_nan(
            a.external_padding_bottom,
            b.external_padding_bottom,
            t(ExternalPaddingBottomId),
        ));
    }
    if has_primitive(p, LeftId) {
        r.set_left(lerp_maybe_nan(a.left, b.left, t(LeftId)));
    }
    if has_primitive(p, TopId) {
        r.set_top(lerp_maybe_nan(a.top, b.top, t(TopId)));
    }
    if has_primitive(p, RightId) {
        r.set_right(lerp_maybe_nan(a.right, b.right, t(RightId)));
    }
    if has_primitive(p, BottomId) {
        r.set_bottom(lerp_maybe_nan(a.bottom, b.bottom, t(BottomId)));
    }
    if has_primitive(p, WidthId) {
        r.set_width(lerp_maybe_nan(a.width, b.width, t(WidthId)));
    }
    if has_primitive(p, HeightId) {
        r.set_height(lerp_maybe_nan(a.height, b.height, t(HeightId)));
    }
    if has_primitive(p, WidthFractionId) {
        r.set_width_fraction(lerp_maybe_nan(a.width_fraction, b.width_fraction, t(WidthFractionId)));
    }
    if has_primitive(p, HeightFractionId) {
        r.set_height_fraction(lerp_maybe_nan(a.height_fraction, b.height_fraction, t(HeightFractionId)));
    }
    if has_primitive(p, MinWidthId) {
        r.set_min_width(lerp_maybe_nan(a.min_width, b.min_width, t(MinWidthId)));
    }
    if has_primitive(p, MaxWidthId) {
        r.set_max_width(lerp_maybe_nan(a.max_width, b.max_width, t(MaxWidthId)));
    }
    if has_primitive(p, MinHeightId) {
        r.set_min_height(lerp_maybe_nan(a.min_height, b.min_height, t(MinHeightId)));
    }
    if has_primitive(p, MaxHeightId) {
        r.set_max_height(lerp_maybe_nan(a.max_height, b.max_height, t(MaxHeightId)));
    }
}

/// Compose's `lerpInnerLayout`.
fn lerp_inner_layout(
    p: u64,
    a: &StyleProperties,
    b: &StyleProperties,
    anim: &StyleAnimations,
    r: &mut StyleProperties,
) {
    let t = |id: PrimitiveId| anim.time_of(id as u32);
    if has_primitive(p, ContentPaddingStartId) {
        r.set_content_padding_start(lerp(a.content_padding_start, b.content_padding_start, t(ContentPaddingStartId)));
    }
    if has_primitive(p, ContentPaddingEndId) {
        r.set_content_padding_end(lerp(a.content_padding_end, b.content_padding_end, t(ContentPaddingEndId)));
    }
    if has_primitive(p, ContentPaddingTopId) {
        r.set_content_padding_top(lerp(a.content_padding_top, b.content_padding_top, t(ContentPaddingTopId)));
    }
    if has_primitive(p, ContentPaddingBottomId) {
        r.set_content_padding_bottom(lerp(
            a.content_padding_bottom,
            b.content_padding_bottom,
            t(ContentPaddingBottomId),
        ));
    }
}

/// Compose's `lerpDraw` of the primitives: the colors at their brushes' times (a color and its brush animate as
/// one).
fn lerp_draw_primitives(
    p: u64,
    a: &StyleProperties,
    b: &StyleProperties,
    anim: &StyleAnimations,
    r: &mut StyleProperties,
) {
    if has_primitive(p, BorderWidthId) {
        r.set_border_width(lerp(a.border_width, b.border_width, anim.time_of(BorderWidthId as u32)));
    }
    if has_primitive(p, BorderColorId) {
        r.set_border_color(lerp_color(a.border_color, b.border_color, anim.time_of(BorderBrushId)));
    }
    if has_primitive(p, BackgroundColorId) {
        r.set_background_color(lerp_color(a.background_color, b.background_color, anim.time_of(BackgroundBrushId)));
    }
    if has_primitive(p, ForegroundColorId) {
        r.set_foreground_color(lerp_color(a.foreground_color, b.foreground_color, anim.time_of(ForegroundBrushId)));
    }
}

/// Compose's `lerpDraw` of the objects.
fn lerp_draw_objects(
    o: u32,
    a: &StyleProperties,
    b: &StyleProperties,
    anim: &StyleAnimations,
    r: &mut StyleProperties,
) {
    if has_object(o, BorderBrushId) {
        let t = anim.time_of(BorderBrushId);
        r.set_border_brush(lerp_brush_or_color(&a.border_brush, a.border_color, &b.border_brush, b.border_color, t));
    }
    if has_object(o, BackgroundBrushId) {
        let t = anim.time_of(BackgroundBrushId);
        r.set_background_brush(lerp_brush_or_color(
            &a.background_brush,
            a.background_color,
            &b.background_brush,
            b.background_color,
            t,
        ));
    }
    if has_object(o, ForegroundBrushId) {
        let t = anim.time_of(ForegroundBrushId);
        r.set_foreground_brush(lerp_brush_or_color(
            &a.foreground_brush,
            a.foreground_color,
            &b.foreground_brush,
            b.foreground_color,
            t,
        ));
    }
    if has_object(o, InnerShadowId) {
        let t = anim.time_of(InnerShadowId);
        r.set_inner_shadow(lerp_shadows(a.inner_shadow.as_ref(), b.inner_shadow.as_ref(), t));
    }
    if has_object(o, DropShadowId) {
        let t = anim.time_of(DropShadowId);
        r.set_drop_shadow(lerp_shadows(a.drop_shadow.as_ref(), b.drop_shadow.as_ref(), t));
    }
    if has_object(o, ShapeId) {
        let t = anim.time_of(ShapeId);
        r.set_shape(lerp_style_shape(&a.shape, &b.shape, t));
    }
}

/// Compose's `lerpLayer` of the primitives. (Compose's own interpolation of the transform origin sets the
/// translation, the origin left as resolved: kept, as Compose does it.)
fn lerp_layer_primitives(
    p: u64,
    a: &StyleProperties,
    b: &StyleProperties,
    anim: &StyleAnimations,
    r: &mut StyleProperties,
) {
    let t = |id: PrimitiveId| anim.time_of(id as u32);
    if has_primitive(p, AlphaId) {
        r.set_alpha(lerp(a.alpha, b.alpha, t(AlphaId)));
    }
    if has_primitive(p, ScaleXId) {
        r.set_scale_x(lerp(a.scale_x, b.scale_x, t(ScaleXId)));
    }
    if has_primitive(p, ScaleYId) {
        r.set_scale_y(lerp(a.scale_y, b.scale_y, t(ScaleYId)));
    }
    if has_primitive(p, TranslationXId) {
        r.set_translation_x(lerp(a.translation_x, b.translation_x, t(TranslationXId)));
    }
    if has_primitive(p, TranslationYId) {
        r.set_translation_y(lerp(a.translation_y, b.translation_y, t(TranslationYId)));
    }
    if has_primitive(p, RotationXId) {
        r.set_rotation_x(lerp(a.rotation_x, b.rotation_x, t(RotationXId)));
    }
    if has_primitive(p, RotationYId) {
        r.set_rotation_y(lerp(a.rotation_y, b.rotation_y, t(RotationYId)));
    }
    if has_primitive(p, RotationZId) {
        r.set_rotation_z(lerp(a.rotation_z, b.rotation_z, t(RotationZId)));
    }
    if has_primitive(p, TransformOriginXId) {
        r.set_translation_x(lerp(a.transform_origin_x, b.transform_origin_x, t(TransformOriginXId)));
    }
    if has_primitive(p, TransformOriginYId) {
        r.set_translation_y(lerp(a.transform_origin_y, b.transform_origin_y, t(TransformOriginYId)));
    }
    if has_primitive(p, ZIndexId) {
        r.set_z_index(lerp(a.z_index, b.z_index, t(ZIndexId)));
    }
    if has_primitive(p, ClipId) {
        r.set_clip(if t(ClipId) < 0.5 { a.clip } else { b.clip });
    }
}

/// Compose's `lerpLayer` of the objects.
fn lerp_layer_objects(
    o: u32,
    a: &StyleProperties,
    b: &StyleProperties,
    anim: &StyleAnimations,
    r: &mut StyleProperties,
) {
    if has_object(o, ColorFilterId) {
        let t = anim.time_of(ColorFilterId);
        r.set_color_filter(lerp_color_filter(a.color_filter.as_ref(), b.color_filter.as_ref(), t));
    }
}

/// A property that switches half way, one end's value where only it set it (Compose's `animateAB`).
fn animate_ab<T>(
    set_in_a: bool,
    set_in_b: bool,
    t: impl FnOnce() -> f32,
    a: impl FnOnce() -> T,
    b: impl FnOnce() -> T,
    set: impl FnOnce(T),
) {
    if set_in_a {
        if set_in_b {
            set(if t() < 0.5 { a() } else { b() });
        } else {
            set(a());
        }
    } else {
        set(b());
    }
}

/// Compose's `lerpTextLayout` of the objects.
fn lerp_text_layout_objects(
    o: u32,
    a: &StyleProperties,
    b: &StyleProperties,
    anim: &StyleAnimations,
    r: &mut StyleProperties,
) {
    for id in [FontFamilyId, TextMotionId, TextIndentId] {
        if !has_object(o, id) {
            continue;
        }
        let (ia, ib) = (a.has_obj(id), b.has_obj(id));
        let t = || anim.time_of(id);
        match id {
            FontFamilyId => {
                animate_ab(ia, ib, t, || a.font_family.clone(), || b.font_family.clone(), |v| r.set_font_family(v))
            }
            TextMotionId => animate_ab(ia, ib, t, || a.text_motion, || b.text_motion, |v| r.set_text_motion(v)),
            _ => animate_ab(
                ia,
                ib,
                t,
                || a.text_indent.expect("kompoz: a text indent set"),
                || b.text_indent.expect("kompoz: a text indent set"),
                |v| r.set_text_indent(v),
            ),
        }
    }
}

/// Compose's `lerpTextLayout` of the primitives: each switching half way.
fn lerp_text_layout_primitives(
    p: u64,
    a: &StyleProperties,
    b: &StyleProperties,
    anim: &StyleAnimations,
    r: &mut StyleProperties,
) {
    let ids = [
        TextDecorationId,
        FontSizeId,
        LineHeightId,
        LetterSpacingId,
        BaselineShiftId,
        LineBreakId,
        TextAlignId,
        TextDirectionId,
        HyphensId,
        FontSynthesisId,
        FontWeightId,
        FontStyleId,
    ];
    for id in ids {
        if !has_primitive(p, id) {
            continue;
        }
        let (ia, ib) = (a.has(id), b.has(id));
        let t = || anim.time_of(id as u32);
        match id {
            TextDecorationId => {
                animate_ab(ia, ib, t, || a.text_decoration(), || b.text_decoration(), |v| r.set_text_decoration(v))
            }
            FontSizeId => animate_ab(ia, ib, t, || a.font_size, || b.font_size, |v| r.set_font_size(v)),
            LineHeightId => animate_ab(ia, ib, t, || a.line_height, || b.line_height, |v| r.set_line_height(v)),
            LetterSpacingId => {
                animate_ab(ia, ib, t, || a.letter_spacing, || b.letter_spacing, |v| r.set_letter_spacing(v))
            }
            BaselineShiftId => {
                animate_ab(ia, ib, t, || a.baseline_shift, || b.baseline_shift, |v| r.set_baseline_shift(v))
            }
            LineBreakId => animate_ab(ia, ib, t, || a.line_break, || b.line_break, |v| r.set_line_break(v)),
            TextAlignId => animate_ab(ia, ib, t, || a.text_align(), || b.text_align(), |v| r.set_text_align(v)),
            TextDirectionId => {
                animate_ab(ia, ib, t, || a.text_direction(), || b.text_direction(), |v| r.set_text_direction(v))
            }
            HyphensId => animate_ab(ia, ib, t, || a.hyphens(), || b.hyphens(), |v| r.set_hyphens(v)),
            FontSynthesisId => {
                animate_ab(ia, ib, t, || a.font_synthesis(), || b.font_synthesis(), |v| r.set_font_synthesis(v))
            }
            FontWeightId => animate_ab(ia, ib, t, || a.font_weight(), || b.font_weight(), |v| r.set_font_weight(v)),
            _ => animate_ab(ia, ib, t, || a.font_style(), || b.font_style(), |v| r.set_font_style(v)),
        }
    }
}

const BrushesMask: u32 = (1 << (BorderBrushId - FirstObjectProperty))
    | (1 << (ContentBrushId - FirstObjectProperty))
    | (1 << (BackgroundBrushId - FirstObjectProperty))
    | (1 << (ForegroundBrushId - FirstObjectProperty));

const ColorsMask: u64 =
    (1 << BorderColorId) | (1 << ContentColorId) | (1 << BackgroundColorId) | (1 << ForegroundColorId);

/// A brush set brings its color along (Compose's `widenPrimitivesSet`).
pub(crate) fn widen_primitives_set(primitives_set: u64, objects_set: u32) -> u64 {
    let mut result = primitives_set;
    if objects_set & BrushesMask != 0 {
        if has_object(objects_set, BorderBrushId) {
            result = with_primitive(result, BorderColorId);
        }
        if has_object(objects_set, ContentBrushId) {
            result = with_primitive(result, ContentColorId);
        }
        if has_object(objects_set, BackgroundBrushId) {
            result = with_primitive(result, BackgroundColorId);
        }
        if has_object(objects_set, ForegroundBrushId) {
            result = with_primitive(result, ForegroundColorId);
        }
    }
    result
}

/// The set without its colors (Compose's `filterPrimitiveColors`).
pub(crate) fn filter_primitive_colors(primitives_set: u64) -> u64 {
    primitives_set & !ColorsMask
}

/// A color set brings its brush along (Compose's `widenObjectsSet`).
pub(crate) fn widen_objects_set(primitives_set: u64, objects_set: u32) -> u32 {
    let mut result = objects_set;
    if primitives_set & ColorsMask != 0 {
        if has_primitive(primitives_set, BorderColorId) {
            result = with_object(result, BorderBrushId);
        }
        if has_primitive(primitives_set, ContentColorId) {
            result = with_object(result, ContentBrushId);
        }
        if has_primitive(primitives_set, BackgroundColorId) {
            result = with_object(result, BackgroundBrushId);
        }
        if has_primitive(primitives_set, ForegroundColorId) {
            result = with_object(result, ForegroundBrushId);
        }
    }
    result
}

/// A brush set leaves its color out (Compose's `favorBrushesOverColors`).
fn favor_brushes_over_colors(primitives_set: u64, objects_set: u32) -> u64 {
    let mut result = primitives_set;
    if objects_set & BrushesMask != 0 {
        if has_object(objects_set, BorderBrushId) {
            result = without_primitive(result, BorderColorId);
        }
        if has_object(objects_set, ContentBrushId) {
            result = without_primitive(result, ContentColorId);
        }
        if has_object(objects_set, BackgroundBrushId) {
            result = without_primitive(result, BackgroundColorId);
        }
        if has_object(objects_set, ForegroundBrushId) {
            result = without_primitive(result, ForegroundColorId);
        }
    }
    result
}

/// The properties of the sets between `a` and `b` at their animations' times, set on `result` (Compose's
/// `lerp(a, b, animations, primitivesSet, objectsSet, result)`): a color and its brush as one, a brush where
/// either end has one.
pub(crate) fn lerp_properties(
    a: &StyleProperties,
    b: &StyleProperties,
    animations: &StyleAnimations,
    primitives_set: u64,
    objects_set: u32,
    result: &mut StyleProperties,
) {
    let primitives_filter = widen_primitives_set(primitives_set, objects_set);
    let objects_filter = widen_objects_set(primitives_set, objects_set);
    let o = (a.objects_set | b.objects_set) & objects_filter;
    let p = favor_brushes_over_colors((a.primitives_set | b.primitives_set) & primitives_filter, o);
    if p == 0 && o == 0 {
        return;
    }
    if p & OuterLayoutPrimitiveFlags != 0 {
        lerp_outer_layout(p, a, b, animations, result);
    }
    if p & InnerLayoutPrimitiveFlags != 0 {
        lerp_inner_layout(p, a, b, animations, result);
    }
    if p & DrawPrimitiveFlags != 0 {
        lerp_draw_primitives(p, a, b, animations, result);
    }
    if o & DrawObjectFlags != 0 {
        lerp_draw_objects(o, a, b, animations, result);
    }
    if p & LayerPrimitiveFlags != 0 {
        lerp_layer_primitives(p, a, b, animations, result);
    }
    if o & LayerObjectFlags != 0 {
        lerp_layer_objects(o, a, b, animations, result);
    }
    if has_primitive(p, ContentColorId) {
        let t = animations.time_of(ContentBrushId);
        result.set_content_color(lerp_color(a.content_color, b.content_color, t));
    }
    if has_object(o, ContentBrushId) {
        let t = animations.time_of(ContentBrushId);
        result.set_content_brush(lerp_brush_or_color(
            &a.content_brush,
            a.content_color,
            &b.content_brush,
            b.content_color,
            t,
        ));
    }
    if p & TextLayoutPrimitiveFlags != 0 {
        lerp_text_layout_primitives(p, a, b, animations, result);
    }
    if o & TextLayoutObjectFlags != 0 {
        lerp_text_layout_objects(o, a, b, animations, result);
    }
}

/// An unset (NaN) end takes the other's value (Compose's `lerpMaybeNan`).
fn lerp_maybe_nan(a: f32, b: f32, t: f32) -> f32 {
    let next = (1.0 - t) * a + t * b;
    if a.is_nan() {
        b
    } else if b.is_nan() {
        a
    } else {
        next
    }
}

/// Compose's `Interpolatable.lerp(a, b, t)` of two brushes, either missing: equal ones switch half way; else
/// `a`'s interpolation to `b` (a missing one as transparent), or `b`'s back to `a`, or a switch half way.
fn interpolatable_brush(a: Option<&Brush>, b: Option<&Brush>, t: f32) -> Option<Brush> {
    if a == b {
        return if t < 0.5 { a.cloned() } else { b.cloned() };
    }
    let mut result = a.and_then(|a| a.lerp(b, t));
    if result.is_none() {
        result = b.and_then(|b| b.lerp(a, 1.0 - t));
    }
    result.or_else(|| if t < 0.5 { a.cloned() } else { b.cloned() })
}

/// Between two brushes, or a brush and a color (made a solid brush) (Compose's style `lerp` of brushes).
fn lerp_brush_or_color(
    left_brush: &Option<Brush>,
    left_color: Color,
    right_brush: &Option<Brush>,
    right_color: Color,
    t: f32,
) -> Option<Brush> {
    let (a, b) = match (left_brush, right_brush) {
        (None, None) => return None,
        (None, Some(r)) => (SolidColor(left_color), r.clone()),
        (Some(l), None) => (l.clone(), SolidColor(right_color)),
        (Some(l), Some(r)) => (l.clone(), r.clone()),
    };
    interpolatable_brush(Some(&a), Some(&b), t).or_else(|| Some(if t < 0.5 { a } else { b }))
}

/// Compose's style `lerp` of shapes: the ends themselves at 0 and 1.
fn lerp_style_shape(a: &Arc<dyn Shape>, b: &Arc<dyn Shape>, t: f32) -> Arc<dyn Shape> {
    if t == 0.0 {
        a.clone()
    } else if t == 1.0 {
        b.clone()
    } else {
        lerp_shape(a, b, t)
    }
}

/// Compose's style `lerp` of color filters: tints and lightings between their kinds, else a switch half way.
fn lerp_color_filter(a: Option<&ColorFilter>, b: Option<&ColorFilter>, fraction: f32) -> Option<ColorFilter> {
    match (a, b) {
        (
            Some(ColorFilter::BlendMode { color: c1, blend_mode: m1 }),
            Some(ColorFilter::BlendMode { color: c2, blend_mode: m2 }),
        ) => {
            let mode: BlendMode = if fraction <= 0.5 { *m1 } else { *m2 };
            Some(ColorFilter::tint(lerp_color(*c1, *c2, fraction), mode))
        }
        (
            Some(ColorFilter::Lighting { multiply: m1, add: a1 }),
            Some(ColorFilter::Lighting { multiply: m2, add: a2 }),
        ) => Some(ColorFilter::lighting(lerp_color(*m1, *m2, fraction), lerp_color(*a1, *a2, fraction))),
        _ => {
            if fraction <= 0.5 {
                a.cloned()
            } else {
                b.cloned()
            }
        }
    }
}

/// Compose's `lerp(Shadow?, Shadow?, t)` (graphics' shadows): a missing end as the other made transparent.
fn lerp_shadow_nullable(a: Option<&Shadow>, b: Option<&Shadow>, t: f32) -> Option<Shadow> {
    match (a, b) {
        (None, None) => None,
        (None, Some(b)) => Some(lerp_shadow_non_null(&transparent_copy(b), b, t)),
        (Some(a), None) => Some(lerp_shadow_non_null(a, &transparent_copy(a), t)),
        (Some(a), Some(b)) => Some(lerp_shadow_non_null(a, b, t)),
    }
}

/// Compose's `Shadow.transparentCopy`: transparent, its brush dropped.
fn transparent_copy(s: &Shadow) -> Shadow {
    Shadow { color: Color::Transparent, brush: None, ..s.clone() }
}

/// Compose's `lerpNonNull` of shadows.
fn lerp_shadow_non_null(a: &Shadow, b: &Shadow, t: f32) -> Shadow {
    Shadow {
        color: lerp_color(a.color, b.color, t),
        offset: kompoz_graphics::lerp_offset(a.offset, b.offset, t),
        blur_radius: lerp(a.blur_radius, b.blur_radius, t),
        spread: lerp(a.spread, b.spread, t),
        brush: interpolatable_brush(a.brush.as_ref(), b.brush.as_ref(), t),
        alpha: lerp(a.alpha, b.alpha, t),
        blend_mode: if t < 0.5 { a.blend_mode } else { b.blend_mode },
    }
}

/// Compose's `lerpShadows`: one and one, or several place by place.
fn lerp_shadows(a: Option<&ShadowValue>, b: Option<&ShadowValue>, t: f32) -> Option<ShadowValue> {
    if a.is_none() && b.is_none() {
        return None;
    }
    let many = |v: Option<&ShadowValue>| -> Option<Rc<[Option<Shadow>]>> {
        match v {
            Some(ShadowValue::Many(m)) => Some(m.clone()),
            _ => None,
        }
    };
    let one = |v: Option<&ShadowValue>| -> Option<Shadow> {
        match v {
            Some(ShadowValue::One(s)) => Some(s.clone()),
            _ => None,
        }
    };
    let (am, bm) = (many(a), many(b));
    if am.is_none() && bm.is_none() {
        return lerp_shadow_nullable(one(a).as_ref(), one(b).as_ref(), t).map(ShadowValue::One);
    }
    let arr = |m: Option<Rc<[Option<Shadow>]>>, v: Option<&ShadowValue>| -> Vec<Option<Shadow>> {
        match m {
            Some(m) => m.to_vec(),
            None => vec![one(v)],
        }
    };
    let (av, bv) = (arr(am, a), arr(bm, b));
    let size = av.len().max(bv.len());
    let result: Vec<Option<Shadow>> = (0..size)
        .map(|i| {
            let left = av.get(i).cloned().flatten();
            let right = bv.get(i).cloned().flatten();
            lerp_shadow_nullable(left.as_ref(), right.as_ref(), t)
        })
        .collect();
    Some(ShadowValue::Many(result.into()))
}
