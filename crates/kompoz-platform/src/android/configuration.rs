//! What the activity tells of the display and the system (KompozConfiguration.kt, read again at each configuration
//! change — `configChanges` keeps the activity across all of them, the density too —, and as the display's refresh
//! rate changes): the density, the text size, the refresh rate, the night mode, the touchscreen and keyboard, the
//! user's languages and time zone; the window's metrics from them. Also where an option is (Android has no
//! environment for an app: a file of its name). Pure: tested on the host.

use crate::app::Metrics;

/// The configuration as KompozConfiguration's `values()` lays it out, then its languages and time zone.
#[derive(Clone, PartialEq, Debug)]
pub struct Configuration {
    /// px per dp (`DisplayMetrics.density`).
    pub density: f32,
    /// The system's text size (`Configuration.fontScale`).
    pub font_scale: f32,
    /// The display's refresh rate (`Display.getRefreshRate`), a second.
    pub refresh_rate: f32,
    /// The night mode is on (`UI_MODE_NIGHT_YES`).
    pub dark: bool,
    /// A touchscreen (`Configuration.touchscreen` is not `TOUCHSCREEN_NOTOUCH`).
    pub touchscreen: bool,
    /// A physical keyboard is there and open (`KEYBOARD_QWERTY`, not `HARDKEYBOARDHIDDEN_YES`).
    pub physical_keyboard: bool,
    /// The user's languages, the first the most (`LocaleList`'s BCP 47 tags).
    pub languages: Vec<String>,
    /// The system's time zone (`TimeZone.getDefault().getID()`); empty: not told.
    pub time_zone: String,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            density: 1.0,
            font_scale: 1.0,
            refresh_rate: 60.0,
            dark: false,
            touchscreen: true,
            physical_keyboard: false,
            languages: Vec::new(),
            time_zone: String::new(),
        }
    }
}

impl Configuration {
    /// KompozConfiguration's values (`[density, fontScale, refreshRate, dark, touchscreen, keyboard]`; what is
    /// missing or out of range: the defaults'), its languages (one tag a line) and its time zone.
    pub fn from_java(values: &[f32], languages: &str, time_zone: &str) -> Configuration {
        let d = Configuration::default();
        let at = |i: usize| values.get(i).copied().filter(|v| v.is_finite());
        let positive = |i: usize, default: f32| at(i).filter(|v| *v > 0.0).unwrap_or(default);
        Configuration {
            density: positive(0, d.density),
            font_scale: positive(1, d.font_scale),
            refresh_rate: positive(2, d.refresh_rate),
            dark: at(3).is_some_and(|v| v != 0.0),
            touchscreen: at(4).map_or(d.touchscreen, |v| v != 0.0),
            physical_keyboard: at(5).is_some_and(|v| v != 0.0),
            languages: languages.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect(),
            time_zone: time_zone.trim().to_string(),
        }
    }

    /// The display's beat, ns.
    pub fn frame_period_ns(&self) -> i64 {
        (1e9 / self.refresh_rate.max(1.0) as f64) as i64
    }

    /// The window's metrics at `width` by `height` px: its pixels are its points (the events' positions are in
    /// them), a dp the density's pixels (`scale`: `KOMPOZ_SCALE` instead, for a run compared with another's), the
    /// insets told to the host apart (no safe area here).
    pub fn metrics(&self, width: i32, height: i32, scale: Option<f32>, font_scale: Option<f32>) -> Metrics {
        Metrics {
            pixel_width: width.max(1),
            pixel_height: height.max(1),
            point_width: width.max(1),
            point_height: height.max(1),
            scale: scale.filter(|s| *s > 0.0).unwrap_or(self.density),
            point_to_px: 1.0,
            font_scale: font_scale.filter(|s| *s > 0.0).unwrap_or(self.font_scale),
            refresh_rate: self.refresh_rate,
            safe_area: None,
            dark_theme: self.dark,
            language: self.languages.first().map(|tag| language_of(tag)).unwrap_or_default(),
            title_bar: None,
        }
    }
}

/// A BCP 47 tag's language ("tr-TR": "tr"; "und": none).
fn language_of(tag: &str) -> String {
    let language: String = tag.chars().take_while(|c| !matches!(c, '-' | '_')).collect();
    if language == "und" { String::new() } else { language }
}

/// The file an option is read from in the app's files (C++'s): its name in lower case, '-' for '_'
/// (`KOMPOZ_SCRIPT`: `kompoz-script`), written with `adb shell run-as <app> …`, or into the app's external files
/// where run-as is refused.
pub fn option_file(name: &str) -> String {
    name.chars().map(|c| if c == '_' { '-' } else { c.to_ascii_lowercase() }).collect()
}

/// An option file's text as the option's value (an `echo`'s line end dropped).
pub fn option_value(text: &str) -> String {
    text.trim_end_matches(['\n', '\r']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_activitys_values_are_read() {
        let c = Configuration::from_java(&[2.625, 1.15, 120.0, 1.0, 1.0, 0.0], "tr-TR\nen-US\n", " Europe/Istanbul ");
        assert_eq!(c.density, 2.625);
        assert_eq!(c.font_scale, 1.15);
        assert!(c.dark && c.touchscreen && !c.physical_keyboard);
        assert_eq!(c.languages, vec!["tr-TR".to_string(), "en-US".to_string()]);
        assert_eq!(c.time_zone, "Europe/Istanbul");
        assert_eq!(c.frame_period_ns(), 8_333_333);
        let m = c.metrics(1080, 2400, None, None);
        assert_eq!((m.pixel_width, m.point_height, m.scale, m.point_to_px), (1080, 2400, 2.625, 1.0));
        assert_eq!(m.language, "tr");
        assert!(m.safe_area.is_none() && m.title_bar.is_none());
        let m = c.metrics(1080, 2400, Some(1.0), Some(2.0));
        assert_eq!((m.scale, m.font_scale), (1.0, 2.0));
    }

    #[test]
    fn what_is_missing_is_the_defaults() {
        let c = Configuration::from_java(&[0.0, f32::NAN], "", "");
        assert_eq!(c, Configuration::default());
        assert_eq!(Configuration::from_java(&[], "und", "").metrics(0, 0, None, None).language, "");
    }

    #[test]
    fn options_are_files() {
        assert_eq!(option_file("KOMPOZ_SCRIPT"), "kompoz-script");
        assert_eq!(option_file("KOMPOZ_GPU_VALIDATION"), "kompoz-gpu-validation");
        assert_eq!(option_value("1\n"), "1");
        assert_eq!(option_value("wait 10; quit\r\n"), "wait 10; quit");
    }
}
