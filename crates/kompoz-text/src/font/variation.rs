//! A variable font's instance (Compose's FontVariation): settings of its axes.

use super::synthesis::FontStyle;
use super::weight::FontWeight;
use kompoz_graphics::{Density, TextUnit};
use std::fmt;
use std::hash::{Hash, Hasher};

/// An axis set to a value (Compose's FontVariation.Setting): `wght` 650, `opsz` 14 sp. Made by [`FontVariation`]'s
/// functions. Equal by axis and value, a whole number not equal to the same as a float (as Compose's).
#[derive(Clone, Copy, PartialEq)]
pub struct FontVariationSetting {
    tag: [u8; 4],
    value: SettingValue,
}

#[derive(Clone, Copy, PartialEq)]
enum SettingValue {
    Float(f32),
    Int(i32),
    /// In sp: px through a density.
    TextUnit(TextUnit),
}

impl FontVariationSetting {
    /// The axis' four-letter tag.
    pub fn axis_name(&self) -> &str {
        std::str::from_utf8(&self.tag).unwrap_or("????")
    }
    /// The axis' tag as bytes.
    pub fn tag(&self) -> [u8; 4] {
        self.tag
    }
    /// Whether its value needs a density (an optical size in sp).
    pub fn needs_density(&self) -> bool {
        matches!(self.value, SettingValue::TextUnit(_))
    }
    /// The value the axis is set to (Compose's toVariationValue): an sp one through `density`'s font scale
    /// (panics without one).
    pub fn to_variation_value(&self, density: Option<Density>) -> f32 {
        match self.value {
            SettingValue::Float(v) => v,
            SettingValue::Int(v) => v as f32,
            SettingValue::TextUnit(t) => {
                let d = density.expect("kompoz: an optical size in sp needs a density");
                t.value() * d.font_scale
            }
        }
    }
}

impl Eq for FontVariationSetting {}

impl Hash for FontVariationSetting {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tag.hash(state);
        match self.value {
            SettingValue::Float(v) => (0u8, (v + 0.0).to_bits()).hash(state),
            SettingValue::Int(v) => (1u8, v).hash(state),
            SettingValue::TextUnit(t) => (2u8, t).hash(state),
        }
    }
}

/// `FontVariation.Setting(axisName='wght', value=650)`, as Compose writes it.
impl fmt::Debug for FontVariationSetting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = self.axis_name();
        match self.value {
            SettingValue::Float(v) => write!(f, "FontVariation.Setting(axisName='{name}', value={v:?})"),
            SettingValue::Int(v) => write!(f, "FontVariation.Setting(axisName='{name}', value={v})"),
            SettingValue::TextUnit(t) => write!(f, "FontVariation.Setting(axisName='{name}', value={t:?})"),
        }
    }
}

/// A variable font's axes as set (Compose's FontVariation.Settings): each axis once.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct FontVariationSettings {
    settings: Vec<FontVariationSetting>,
}

impl FontVariationSettings {
    /// None set: the font's defaults.
    pub const Empty: FontVariationSettings = FontVariationSettings { settings: Vec::new() };

    /// These settings (an axis set twice panics, as Compose's).
    pub fn new(settings: impl IntoIterator<Item = FontVariationSetting>) -> FontVariationSettings {
        let settings: Vec<FontVariationSetting> = settings.into_iter().collect();
        for (i, s) in settings.iter().enumerate() {
            assert!(
                !settings[..i].iter().any(|o| o.tag == s.tag),
                "kompoz: '{}' must be unique. Actual [{}]",
                s.axis_name(),
                settings.iter().map(|s| s.axis_name()).collect::<Vec<_>>().join(", ")
            );
        }
        FontVariationSettings { settings }
    }
    pub fn settings(&self) -> &[FontVariationSetting] {
        &self.settings
    }
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }
    /// Whether any value needs a density.
    pub fn needs_density(&self) -> bool {
        self.settings.iter().any(|s| s.needs_density())
    }
    /// Each axis' tag and value (sp through `density`).
    pub fn values(&self, density: Option<Density>) -> Vec<([u8; 4], f32)> {
        self.settings.iter().map(|s| (s.tag, s.to_variation_value(density))).collect()
    }
}

/// The settings of a variable font's axes (Compose's FontVariation object): its registered axes' settings, checked
/// as Compose checks them, and one of any axis.
pub struct FontVariation;

fn tag_of(name: &str) -> [u8; 4] {
    let bytes = name.as_bytes();
    assert!(bytes.len() == 4, "kompoz: a variation axis' name is 4 characters, not '{name}'");
    [bytes[0], bytes[1], bytes[2], bytes[3]]
}

impl FontVariation {
    /// Any axis, by its four-letter name (Compose's FontVariation.Setting(name, value)).
    pub fn setting(name: &str, value: f32) -> FontVariationSetting {
        FontVariationSetting { tag: tag_of(name), value: SettingValue::Float(value) }
    }
    /// `ital`, 0 to 1.
    pub fn italic(value: f32) -> FontVariationSetting {
        assert!((0.0..=1.0).contains(&value), "kompoz: 'ital' must be in 0.0f..1.0f. Actual: {value}");
        FontVariationSetting { tag: *b"ital", value: SettingValue::Float(value) }
    }
    /// `opsz`, in sp (its value then follows the font scale).
    pub fn optical_sizing(text_size: TextUnit) -> FontVariationSetting {
        assert!(text_size.is_sp(), "kompoz: 'opsz' must be provided in sp units");
        FontVariationSetting { tag: *b"opsz", value: SettingValue::TextUnit(text_size) }
    }
    /// `slnt`, -90 to 90 degrees.
    pub fn slant(value: f32) -> FontVariationSetting {
        assert!((-90.0..=90.0).contains(&value), "kompoz: 'slnt' must be in -90f..90f. Actual: {value}");
        FontVariationSetting { tag: *b"slnt", value: SettingValue::Float(value) }
    }
    /// `wdth`, above 0.
    pub fn width(value: f32) -> FontVariationSetting {
        assert!(value > 0.0, "kompoz: 'wdth' must be strictly > 0.0f. Actual: {value}");
        FontVariationSetting { tag: *b"wdth", value: SettingValue::Float(value) }
    }
    /// `wght`, 1 to 1000.
    pub fn weight(value: i32) -> FontVariationSetting {
        assert!((1..=1000).contains(&value), "kompoz: 'wght' value must be in [1, 1000]. Actual: {value}");
        FontVariationSetting { tag: *b"wght", value: SettingValue::Int(value) }
    }
    /// `GRAD`, -1000 to 1000.
    pub fn grade(value: i32) -> FontVariationSetting {
        assert!((-1000..=1000).contains(&value), "kompoz: 'GRAD' must be in -1000..1000");
        FontVariationSetting { tag: *b"GRAD", value: SettingValue::Int(value) }
    }
    /// A weight's and a style's settings (`wght`, `ital`) and then `settings` (Compose's
    /// FontVariation.Settings(weight, style, vararg settings)).
    pub fn settings(
        weight: FontWeight,
        style: FontStyle,
        settings: impl IntoIterator<Item = FontVariationSetting>,
    ) -> FontVariationSettings {
        let mut all = vec![FontVariation::weight(weight.weight), FontVariation::italic(style.value() as f32)];
        all.extend(settings);
        FontVariationSettings::new(all)
    }
}
