//! What Windows' settings say, from the registry's values and the system's parameters as `desktop` reads them (no
//! Win32 here: tested on every host): the apps' light or dark theme (`Personalize\AppsUseLightTheme`), the accent
//! color (DWM's `AccentColor`, else the accent palette's, else DWM's colorization color), the text size
//! (`Accessibility\TextScaleFactor`, a percentage), high contrast, the client area's animations, and the mouse's and
//! caret's settings as Compose's view configuration takes them; and what changed between two readings.

use kompoz_graphics::{Color, Size};
use kompoz_ui::platform::{ViewConfiguration, WheelScrollAmount};
use std::time::Duration;

/// `SPI_GETWHEELSCROLLLINES`' answer for a page a notch (winuser.h's WHEEL_PAGESCROLL).
pub const WHEEL_PAGESCROLL: u32 = u32::MAX;
/// `GetCaretBlinkTime`'s answer for a caret that doesn't blink (INFINITE).
pub const CARET_NO_BLINK: u32 = u32::MAX;

/// The mouse's and the caret's settings (in px at 96 DPI where they are lengths: `GetSystemMetricsForDpi(…, 96)`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PointerSettings {
    /// `GetDoubleClickTime`, ms.
    pub double_click_ms: u32,
    /// SM_CXDOUBLECLK, SM_CYDOUBLECLK: the rectangle a second click is within.
    pub double_click: (u32, u32),
    /// SM_CXDRAG, SM_CYDRAG: how far from a press the mouse moves before a drag starts (each side).
    pub drag: (u32, u32),
    /// `GetCaretBlinkTime`, ms ([`CARET_NO_BLINK`]: it doesn't).
    pub caret_blink_ms: u32,
    /// `SPI_GETWHEELSCROLLLINES` ([`WHEEL_PAGESCROLL`]: a page).
    pub wheel_lines: u32,
}

impl Default for PointerSettings {
    /// Windows' own defaults.
    fn default() -> Self {
        PointerSettings {
            double_click_ms: 500,
            double_click: (4, 4),
            drag: (4, 4),
            caret_blink_ms: 530,
            wheel_lines: 3,
        }
    }
}

impl PointerSettings {
    /// The view configuration at `scale` px per dp (the window's DPI / 96): the double tap's timeout the double
    /// click time, its area the double click rectangle, the touch slop the drag rectangle's (Compose for desktop
    /// starts a mouse's drag-and-drop past the touch slop; Windows' drag threshold is the same for a pen and touch),
    /// the cursor's blink the caret's, the wheel's notch the lines to scroll; the rest Compose's at that density.
    pub fn view_configuration(&self, scale: f32) -> ViewConfiguration {
        let base = ViewConfiguration::for_density(kompoz_graphics::Density::new(scale.max(0.1), 1.0));
        let px = |v: u32| v as f32 * scale;
        let double_tap = Duration::from_millis(u64::from(self.double_click_ms.clamp(1, 5000)));
        ViewConfiguration {
            double_tap_timeout: double_tap,
            double_tap_min_time: base.double_tap_min_time.min(double_tap),
            touch_slop: px(self.drag.0.max(self.drag.1)).max(1.0),
            double_click_area: Some(Size::new(px(self.double_click.0), px(self.double_click.1))),
            cursor_blink_interval: match self.caret_blink_ms {
                CARET_NO_BLINK | 0 => None,
                ms => Some(Duration::from_millis(u64::from(ms))),
            },
            wheel_scroll_amount: match self.wheel_lines {
                WHEEL_PAGESCROLL => WheelScrollAmount::Page,
                lines => WheelScrollAmount::Lines(lines.min(100) as f32),
            },
            ..base
        }
    }
}

/// The registry's and the system's values as read (none: not there).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RawSettings {
    /// `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`.
    pub apps_use_light_theme: Option<u32>,
    /// `HKCU\Software\Microsoft\Windows\DWM\AccentColor` (0xAABBGGRR).
    pub accent_color: Option<u32>,
    /// `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Accent\AccentPalette` (8 colors, R G B A each,
    /// lightest first: the fourth is the system's accent).
    pub accent_palette: Option<Vec<u8>>,
    /// `HKCU\Software\Microsoft\Windows\DWM\ColorizationColor` (0xAARRGGBB).
    pub colorization_color: Option<u32>,
    /// `HKCU\Software\Microsoft\Accessibility\TextScaleFactor` (100 to 225).
    pub text_scale_factor: Option<u32>,
    /// `SPI_GETHIGHCONTRAST`'s HCF_HIGHCONTRASTON.
    pub high_contrast: bool,
    /// `SPI_GETCLIENTAREAANIMATION` (none: not told, as before Windows 7).
    pub client_area_animation: Option<bool>,
    pub pointer: PointerSettings,
}

/// What the app takes of them.
#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    /// The apps' theme is dark (`AppsUseLightTheme` 0; light where it isn't there, as Windows' default).
    pub dark: bool,
    /// The accent color; none: none told (or `KOMPOZ_NO_DESKTOP_ACCENT`).
    pub accent: Option<Color>,
    /// The system's text size (1: as designed).
    pub text_scale: f32,
    pub high_contrast: bool,
    /// Animations shown (Settings > Accessibility > Visual effects > Animation effects).
    pub animations: bool,
    pub pointer: PointerSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings::from_raw(&RawSettings::default(), true)
    }
}

impl Settings {
    /// What `raw` says, the accent read where `accent_wanted`.
    pub fn from_raw(raw: &RawSettings, accent_wanted: bool) -> Settings {
        Settings {
            dark: raw.apps_use_light_theme == Some(0),
            accent: if accent_wanted { accent(raw) } else { None },
            text_scale: text_scale(raw.text_scale_factor),
            high_contrast: raw.high_contrast,
            animations: raw.client_area_animation.unwrap_or(true),
            pointer: raw.pointer,
        }
    }

    /// How long the app's animations take (Compose's motion duration scale): none at all where animations are off.
    pub fn motion_duration_scale(&self) -> f32 {
        if self.animations { 1.0 } else { 0.0 }
    }
}

/// The accent color: DWM's `AccentColor` (what the system's frames and C++'s Kompoz take), else the accent palette's
/// own (UISettings' Accent), else DWM's colorization color; opaque (the alpha DWM keeps is its glass's).
pub fn accent(raw: &RawSettings) -> Option<Color> {
    if let Some(abgr) = raw.accent_color {
        let (r, g, b) = (abgr & 0xff, (abgr >> 8) & 0xff, (abgr >> 16) & 0xff);
        return Some(Color::from_argb(0xff00_0000 | r << 16 | g << 8 | b));
    }
    if let Some(p) = raw.accent_palette.as_deref().filter(|p| p.len() >= 16) {
        let (r, g, b) = (u32::from(p[12]), u32::from(p[13]), u32::from(p[14]));
        return Some(Color::from_argb(0xff00_0000 | r << 16 | g << 8 | b));
    }
    raw.colorization_color.map(|argb| Color::from_argb(0xff00_0000 | (argb & 0x00ff_ffff)))
}

/// The font scale a `TextScaleFactor` says (a percentage, 100 to 225 in Settings > Accessibility > Text size).
pub fn text_scale(percent: Option<u32>) -> f32 {
    match percent {
        Some(p) if p > 0 => (p.clamp(100, 225) as f32) / 100.0,
        _ => 1.0,
    }
}

/// What changed between two readings, for the windows and their cores.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Changes {
    /// The theme: the windows' captions with it.
    pub theme: bool,
    /// The metrics read again (the theme, the text size, high contrast in the media, the view configuration).
    pub metrics: bool,
    /// The accent color: its palette (and the captions' colors).
    pub accent: bool,
    /// The animations turned on or off (the motion duration scale reads them as each animation asks).
    pub motion: bool,
}

impl Changes {
    pub fn between(before: &Settings, now: &Settings) -> Changes {
        Changes {
            theme: before.dark != now.dark,
            metrics: before.dark != now.dark
                || before.text_scale != now.text_scale
                || before.high_contrast != now.high_contrast
                || before.pointer != now.pointer,
            accent: before.accent != now.accent,
            motion: before.animations != now.animations,
        }
    }

    pub fn any(&self) -> bool {
        self.theme || self.metrics || self.accent || self.motion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_says_the_theme_the_accent_and_the_text_size() {
        let raw = RawSettings {
            apps_use_light_theme: Some(0),
            accent_color: Some(0xff_d7_78_00), // (ABGR: Windows' default blue, #0078D7)
            colorization_color: Some(0xc4_11_22_33),
            text_scale_factor: Some(150),
            ..RawSettings::default()
        };
        let s = Settings::from_raw(&raw, true);
        assert!(s.dark);
        assert_eq!(s.accent, Some(Color::from_argb(0xff0078d7)));
        assert_eq!(s.text_scale, 1.5);
        assert!(s.animations);
        assert_eq!(Settings::from_raw(&raw, false).accent, None);
        // Light where the key isn't there (wine's prefix) or says 1.
        assert!(!Settings::from_raw(&RawSettings::default(), true).dark);
        assert!(!Settings::from_raw(&RawSettings { apps_use_light_theme: Some(1), ..raw.clone() }, true).dark);
        // The palette's fourth color, else the colorization color (its alpha dropped).
        let mut palette = vec![0u8; 32];
        palette[12..16].copy_from_slice(&[0x10, 0x7c, 0x10, 0x00]);
        let raw = RawSettings { accent_color: None, accent_palette: Some(palette), ..raw };
        assert_eq!(accent(&raw), Some(Color::from_argb(0xff107c10)));
        let raw = RawSettings { accent_palette: Some(vec![1, 2, 3]), ..raw };
        assert_eq!(accent(&raw), Some(Color::from_argb(0xff112233)));
        assert_eq!(accent(&RawSettings::default()), None);
        // Text sizes outside Settings' range kept in it.
        assert_eq!(text_scale(None), 1.0);
        assert_eq!(text_scale(Some(0)), 1.0);
        assert_eq!(text_scale(Some(90)), 1.0);
        assert_eq!(text_scale(Some(300)), 2.25);
    }

    #[test]
    fn the_mouse_and_the_caret_are_the_view_configuration() {
        let p = PointerSettings {
            double_click_ms: 400,
            double_click: (6, 8),
            drag: (5, 3),
            caret_blink_ms: 300,
            wheel_lines: 6,
        };
        let c = p.view_configuration(1.5);
        assert_eq!(c.double_tap_timeout, Duration::from_millis(400));
        assert_eq!(c.double_click_area, Some(Size::new(9.0, 12.0)));
        assert_eq!(c.touch_slop, 7.5);
        assert_eq!(c.cursor_blink_interval, Some(Duration::from_millis(300)));
        assert_eq!(c.wheel_scroll_amount, WheelScrollAmount::Lines(6.0));
        // The rest Compose's at that density.
        assert_eq!(c.minimum_touch_target_size, Size::new(72.0, 72.0));
        assert_eq!(c.long_press_timeout, ViewConfiguration::default().long_press_timeout);
        let p = PointerSettings { caret_blink_ms: CARET_NO_BLINK, wheel_lines: WHEEL_PAGESCROLL, ..p };
        let c = p.view_configuration(1.0);
        assert_eq!(c.cursor_blink_interval, None);
        assert_eq!(c.wheel_scroll_amount, WheelScrollAmount::Page);
        // Windows' defaults: Compose's 500 ms blink is close to the caret's 530.
        let c = PointerSettings::default().view_configuration(1.0);
        assert_eq!((c.touch_slop, c.double_tap_timeout_millis()), (4.0, 500));
    }

    #[test]
    fn changes_say_what_to_do() {
        let before = Settings::default();
        assert!(!Changes::between(&before, &before).any());
        let dark = Settings { dark: true, ..before.clone() };
        let c = Changes::between(&before, &dark);
        assert!(c.theme && c.metrics && !c.accent && !c.motion);
        let c = Changes::between(&before, &Settings { accent: Some(Color::from_argb(0xff0078d7)), ..before.clone() });
        assert!(c.accent && !c.metrics);
        let c = Changes::between(&before, &Settings { text_scale: 1.25, ..before.clone() });
        assert!(c.metrics && !c.theme);
        let c = Changes::between(&before, &Settings { high_contrast: true, ..before.clone() });
        assert!(c.metrics);
        let off = Settings { animations: false, ..before.clone() };
        let c = Changes::between(&before, &off);
        assert!(c.motion && !c.metrics);
        assert_eq!((before.motion_duration_scale(), off.motion_duration_scale()), (1.0, 0.0));
        let c = Changes::between(
            &before,
            &Settings { pointer: PointerSettings { wheel_lines: 1, ..before.pointer }, ..before.clone() },
        );
        assert!(c.metrics);
    }
}
