//! The desktop's services on Windows followed by the app in its window (`KOMPOZ_DISPLAY_TESTS=1`; on Windows, and
//! under wine here; skipped otherwise): the app (this test run again, in a process of its own) says what its locals
//! say — the system's dark theme, the font scale, high contrast, the accent's and the wallpaper's palettes' hues —
//! each time they change; the test writes the apps' theme, DWM's accent color and the text size to the registry as
//! Settings does (RegSetValueEx) and sets the wallpaper (SPI_SETDESKWALLPAPER), and waits for the app to say each.
//! What it changed is put back after.

#![allow(non_snake_case)]

#[cfg(windows)]
mod on_windows {
    #![allow(unsafe_code)]

    use kompoz_foundation::prelude::*;
    use kompoz_graphics::material_color::hct::Hct;
    use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
    use kompoz_graphics::*;
    use kompoz_runtime::*;
    use kompoz_ui::Modifier;
    use kompoz_ui::media_query::media_query;
    use kompoz_ui::platform::{
        LocalDensity, LocalDynamicTonalPalette, LocalWallpaperTonalPalette, is_system_in_dark_theme,
    };
    use std::ffi::c_void;
    use std::io::{BufRead, BufReader};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::mpsc::{Receiver, channel};
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_VALUE_TYPE, RRF_NOEXPAND,
        RRF_RT_ANY, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegSetValueExW,
    };
    use windows::Win32::UI::WindowsAndMessaging as wm;
    use windows::core::PCWSTR;

    pub const CHILD: &str = "KOMPOZ_DESKTOP_WINDOWS_CHILD";

    const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    const DWM: &str = r"Software\Microsoft\Windows\DWM";
    const ACCESSIBILITY: &str = r"Software\Microsoft\Accessibility";

    /// A palette's hue (its accent's at tone 40), whole degrees; -1: none.
    fn hue(p: Option<DynamicTonalPalette>) -> i32 {
        p.map_or(-1, |p| Hct::from_color(p.accent1[8]).hue().round() as i32)
    }

    /// What the app's locals say, a line each time it is composed with them.
    #[kompozable]
    pub fn Desktop() {
        let line = format!(
            "desktop: dark={} font={} hc={} accent={} wallpaper={}",
            is_system_in_dark_theme(),
            LocalDensity.current().font_scale,
            media_query(|m| m.high_contrast()),
            hue(LocalDynamicTonalPalette.current()),
            hue(LocalWallpaperTonalPalette.current()),
        );
        SideEffect(move || eprintln!("{line}"));
        Box(modifier = Modifier.fill_max_size().background(Color::White));
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// A value of the user's as it was (its type and bytes; none: not there).
    fn saved(key: &str, value: &str) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
        let (k, v) = (wide(key), wide(value));
        let mut kind = REG_VALUE_TYPE(0);
        let mut data = vec![0u8; 4096];
        let mut size = data.len() as u32;
        // SAFETY: the strings live through the call; `data` is `size` writable bytes; `kind` is written.
        let read = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(k.as_ptr()),
                PCWSTR(v.as_ptr()),
                RRF_RT_ANY | RRF_NOEXPAND,
                Some(&mut kind),
                Some(data.as_mut_ptr() as *mut c_void),
                Some(&mut size),
            )
        };
        (read == ERROR_SUCCESS).then(|| {
            data.truncate(size as usize);
            (kind, data)
        })
    }

    /// A value of the user's set (its key made where it isn't there); none: deleted.
    fn put(key: &str, value: &str, data: Option<(REG_VALUE_TYPE, &[u8])>) {
        let (k, v) = (wide(key), wide(value));
        let mut opened = HKEY::default();
        // SAFETY: the strings live through the calls; the key is written, then closed.
        unsafe {
            let made = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(k.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut opened,
                None,
            );
            assert_eq!(made, ERROR_SUCCESS, "{key} made");
            match data {
                Some((kind, bytes)) => {
                    assert_eq!(RegSetValueExW(opened, PCWSTR(v.as_ptr()), None, kind, Some(bytes)), ERROR_SUCCESS);
                }
                None => {
                    let _ = RegDeleteValueW(opened, PCWSTR(v.as_ptr()));
                }
            }
            let _ = RegCloseKey(opened);
        }
    }

    fn put_dword(key: &str, value: &str, v: u32) {
        put(key, value, Some((REG_DWORD, &v.to_le_bytes())));
    }

    /// The desktop's wallpaper now (SPI_GETDESKWALLPAPER).
    fn wallpaper() -> String {
        let mut path = [0u16; 300];
        // SAFETY: pvParam is `path`, uiParam its length; written 0-terminated.
        let _ = unsafe {
            wm::SystemParametersInfoW(
                wm::SPI_GETDESKWALLPAPER,
                path.len() as u32,
                Some(path.as_mut_ptr() as *mut c_void),
                wm::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        String::from_utf16_lossy(&path[..path.iter().position(|&c| c == 0).unwrap_or(0)])
    }

    /// The desktop's wallpaper set as Settings sets it.
    fn set_wallpaper(path: &str) {
        let mut w = wide(path);
        // SAFETY: pvParam is the path, 0-terminated, alive through the call.
        let set = unsafe {
            wm::SystemParametersInfoW(
                wm::SPI_SETDESKWALLPAPER,
                0,
                Some(w.as_mut_ptr() as *mut c_void),
                wm::SPIF_UPDATEINIFILE | wm::SPIF_SENDCHANGE,
            )
        };
        assert!(set.is_ok(), "the wallpaper set to {path}");
    }

    /// A 24-bit BMP of one color.
    fn write_bmp(path: &Path, rgb: u32) {
        let (w, h) = (64u32, 48u32);
        let row = (w * 3).div_ceil(4) * 4;
        let size = 54 + row * h;
        let mut b = vec![0u8; size as usize];
        let mut put = |at: usize, v: u32, n: usize| {
            for i in 0..n {
                b[at + i] = (v >> (8 * i)) as u8;
            }
        };
        put(0, u32::from(b'B') | u32::from(b'M') << 8, 2);
        put(2, size, 4);
        put(10, 54, 4);
        put(14, 40, 4);
        put(18, w, 4);
        put(22, h, 4);
        put(26, 1, 2);
        put(28, 24, 2);
        for y in 0..h {
            for x in 0..w {
                put((54 + y * row + x * 3) as usize, rgb, 3);
            }
        }
        std::fs::write(path, b).unwrap();
    }

    /// What the test changed, put back however it ends; the app ended.
    struct Restore {
        values: Vec<(&'static str, &'static str, Option<(REG_VALUE_TYPE, Vec<u8>)>)>,
        wallpaper: String,
        app: Option<Child>,
    }

    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(mut app) = self.app.take() {
                let _ = app.kill();
                let _ = app.wait();
            }
            for (key, value, data) in &self.values {
                put(key, value, data.as_ref().map(|(kind, bytes)| (*kind, bytes.as_slice())));
            }
            set_wallpaper(&self.wallpaper);
        }
    }

    /// The app's lines until one `wanted` takes (at most 20 seconds); the last seen if none.
    fn wait_for(lines: &Receiver<String>, what: &str, wanted: impl Fn(&str) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut last = String::new();
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match lines.recv_timeout(left) {
                Ok(line) if line.starts_with("desktop: ") => {
                    if wanted(&line) {
                        return;
                    }
                    last = line;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        panic!("the app didn't follow {what}; it said last: {last}");
    }

    /// A line's value of `name` (`name=value`).
    fn field<'a>(line: &'a str, name: &str) -> &'a str {
        line.split(' ').find_map(|p| p.strip_prefix(name).and_then(|p| p.strip_prefix('='))).unwrap_or("")
    }

    fn seed_hue(rgb: u32) -> i32 {
        hue(Some(DynamicTonalPalette::from_seed(Color::hex(rgb))))
    }

    fn near(line: &str, name: &str, hue: i32) -> bool {
        field(line, name).parse::<i32>().is_ok_and(|h| (h - hue).abs() <= 8)
    }

    pub fn desktop_on_windows() {
        let dir: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join("desktop-windows");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut restore = Restore {
            values: [(PERSONALIZE, "AppsUseLightTheme"), (DWM, "AccentColor"), (ACCESSIBILITY, "TextScaleFactor")]
                .into_iter()
                .map(|(k, v)| (k, v, saved(k, v)))
                .collect(),
            wallpaper: wallpaper(),
            app: None,
        };
        put_dword(PERSONALIZE, "AppsUseLightTheme", 1);
        put_dword(DWM, "AccentColor", 0xff_d7_78_00); // (ABGR: #0078D7)
        put(ACCESSIBILITY, "TextScaleFactor", None);
        let (green, red) = (dir.join("green.bmp"), dir.join("red.bmp"));
        write_bmp(&green, 0x2E7D32);
        write_bmp(&red, 0xD32F2F);
        set_wallpaper(&green.to_string_lossy());

        let mut app = Command::new(std::env::current_exe().unwrap())
            .current_dir(&dir)
            .env(CHILD, "1")
            .env("KOMPOZ_SCALE", "1")
            .env("KOMPOZ_SCRIPT", "wait 30;sleep 600000;quit")
            .env_remove("KOMPOZ_FONT_SCALE")
            .env_remove("KOMPOZ_NO_DESKTOP_ACCENT")
            .env_remove("KOMPOZ_NO_WALLPAPER_COLORS")
            .env_remove("KOMPOZ_HEADLESS")
            .env_remove("KOMPOZ_PLATFORM")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the app starts");
        let (sender, lines) = channel();
        let stderr = app.stderr.take().expect("its standard error");
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                println!("{line}");
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        restore.app = Some(app);

        let blue = seed_hue(0x0078D7);
        wait_for(&lines, "the desktop as it started", |l| {
            field(l, "dark") == "false"
                && field(l, "font") == "1"
                && near(l, "accent", blue)
                && near(l, "wallpaper", seed_hue(0x2E7D32))
        });
        put_dword(PERSONALIZE, "AppsUseLightTheme", 0);
        wait_for(&lines, "the dark theme", |l| field(l, "dark") == "true");
        put_dword(DWM, "AccentColor", 0xff_2f_2f_d3); // (#D32F2F)
        wait_for(&lines, "the accent", |l| near(l, "accent", seed_hue(0xD32F2F)));
        put_dword(ACCESSIBILITY, "TextScaleFactor", 125);
        wait_for(&lines, "the text size", |l| field(l, "font") == "1.25");
        set_wallpaper(&red.to_string_lossy());
        wait_for(&lines, "the wallpaper", |l| near(l, "wallpaper", seed_hue(0xD32F2F)));
        drop(restore);
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    use kompoz_graphics::UnitExt;
    if std::env::var_os(on_windows::CHILD).is_some() {
        return kompoz_platform::App::new("Masaüstü").size(200.dp(), 150.dp()).run(on_windows::Desktop);
    }
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 runs it");
        return std::process::ExitCode::SUCCESS;
    }
    let ok = std::panic::catch_unwind(on_windows::desktop_on_windows).is_ok();
    println!("{} desktop_on_windows", if ok { "ok    " } else { "FAILED" });
    if ok { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE }
}

#[cfg(not(windows))]
fn main() {
    println!("skipped: the Windows platform's desktop services run on Windows");
}
