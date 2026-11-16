//! The wallpaper's colors (C++'s platform tests): KDE Plasma's config read, and the wallpaper followed as it
//! changes, from a KDE config of the tests' own in a scratch config directory, changed as KConfig does; the desktop's
//! services stopped at once while a big wallpaper's palette is still being worked out. One thread, in sequence.

#[cfg(target_os = "linux")]
mod linux {
    use kompoz_graphics::Color;
    use kompoz_graphics::material_color::hct::Hct;
    use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
    use kompoz_platform::linux::atspi::AtspiOptions;
    use kompoz_platform::linux::desktop::{DesktopOptions, DesktopServices};
    use kompoz_platform::linux::portal::PortalOptions;
    use kompoz_platform::linux::wallpaper::{WallpaperColors, WallpaperOptions, kde_wallpaper_from_config, palette_of};
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    /// A 24-bit BMP of one color.
    fn write_bmp(path: &Path, rgb: u32) {
        write_bmp_sized(path, rgb, 64, 48);
    }

    /// A 24-bit BMP of one color, `w` × `h`.
    fn write_bmp_sized(path: &Path, rgb: u32, w: u32, h: u32) {
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
                put((54 + y * row + x * 3) as usize, rgb, 3); // little endian: B, G, R
            }
        }
        std::fs::write(path, b).unwrap();
    }

    fn dirs(d: &str) -> Vec<String> {
        let names: &[&str] = match d {
            "/usr/share/wallpapers/Next/contents/images" => {
                &["1920x1080.png", "5120x2880.png", "800x600.jpg", "screenshot.png"]
            }
            "/usr/share/wallpapers/Next/contents/images_dark" => &["3840x2160.png"],
            _ => &[],
        };
        names.iter().map(|s| s.to_string()).collect()
    }

    pub fn kde_config_is_read() {
        let config = "
[Containments][1]
activityId=
formfactor=2
lastScreen=0
location=4
plugin=org.kde.panel
wallpaperplugin=org.kde.image

[Containments][1][Wallpaper][org.kde.image][General]
Image=/panel.png

[Containments][2]
activityId=other
lastScreen=0
location=0
plugin=org.kde.plasma.folder
wallpaperplugin=org.kde.image

[Containments][2][Wallpaper][org.kde.image][General]
Image=file:///home/me/Other%20Pics/a.jpg

[Containments][3]
activityId=current
lastScreen=1
location=0
wallpaperplugin=org.kde.image

[Containments][3][Wallpaper][org.kde.image][General]
Image=/second-screen.jpg

[Containments][4]
activityId=current
lastScreen=0
location=0
wallpaperplugin=org.kde.image

[Containments][4][Wallpaper][org.kde.image][General]
Image=file:///usr/share/wallpapers/Next/
PreviewImage=/ignored.png
";
        let read = |config: &str, activity: &str, dark: bool| kde_wallpaper_from_config(config, activity, dark, &dirs);
        // The first screen's desktop in the current activity; a package: its biggest picture, images_dark when dark.
        assert_eq!(
            read(config, "current", false).as_deref(),
            Some("/usr/share/wallpapers/Next/contents/images/5120x2880.png")
        );
        assert_eq!(
            read(config, "current", true).as_deref(),
            Some("/usr/share/wallpapers/Next/contents/images_dark/3840x2160.png")
        );
        // Another activity's own (a file:// URI, %20 decoded); panels never.
        assert_eq!(read(config, "other", false).as_deref(), Some("/home/me/Other Pics/a.jpg"));
        let no_folder = &config[..config.find("[Containments][4]").unwrap()];
        assert_eq!(read(no_folder, "current", false).as_deref(), Some("/home/me/Other Pics/a.jpg")); // screen 0 beats activity
        let either_order = "[Containments][6]\nactivityId=current\nlastScreen=1\nwallpaperplugin=org.kde.image\n\
                            [Containments][6][Wallpaper][org.kde.image][General]\nImage=/one.jpg\n\
                            [Containments][7]\nactivityId=other\nlastScreen=0\nwallpaperplugin=org.kde.image\n\
                            [Containments][7][Wallpaper][org.kde.image][General]\nImage=/zero.jpg\n";
        assert_eq!(read(either_order, "current", false).as_deref(), Some("/zero.jpg")); // whatever the order they come in
        let panel = "[Containments][1]\nlocation=4\nwallpaperplugin=org.kde.image\n\
                     [Containments][1][Wallpaper][org.kde.image][General]\nImage=/p.png\n";
        assert_eq!(read(panel, "", false), None);
        assert_eq!(read("", "", false), None);
        // A slideshow (another plugin) isn't read.
        let slideshow = "[Containments][5]\nlastScreen=0\nwallpaperplugin=org.kde.slideshow\n\
                         [Containments][5][Wallpaper][org.kde.image][General]\nImage=/x.png\n";
        assert_eq!(read(slideshow, "", false), None);
    }

    /// A KDE config showing `image` on the first screen's desktop, written into `home` as KConfig writes one
    /// (another file renamed over it).
    fn write_kde_config(home: &Path, image: &str) {
        let tmp = home.join("tmp-appletsrc");
        std::fs::write(
            &tmp,
            format!(
                "[Containments][9]\nlastScreen=0\nlocation=0\nwallpaperplugin=org.kde.image\n\n\
                 [Containments][9][Wallpaper][org.kde.image][General]\nImage={image}\n"
            ),
        )
        .unwrap();
        std::fs::rename(&tmp, home.join("plasma-org.kde.plasma.desktop-appletsrc")).unwrap();
    }

    /// The wallpaper changed while the app runs: told by inotify, once the change settled.
    pub fn wallpaper_changes_are_followed() {
        let home = std::env::temp_dir().join(format!("kompoz-platform-tests-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let (red, blue, blue2) = (home.join("red.bmp"), home.join("blue.bmp"), home.join("blue2.bmp"));
        write_bmp(&red, 0xD32F2F);
        write_bmp(&blue, 0x1976D2);
        write_bmp(&blue2, 0x1976D2);
        let write_config = |image: &str| write_kde_config(&home, image);
        write_config(red.to_str().unwrap());
        let notified = Arc::new(AtomicUsize::new(0));
        let n = notified.clone();
        let options = WallpaperOptions { config_home: home.clone(), desktop: "KDE".into(), enabled: true };
        let colors = WallpaperColors::new(
            false,
            Arc::new(move || {
                n.fetch_add(1, Ordering::SeqCst);
            }),
            options,
        );
        assert!(colors.listening());
        let wait_for = |done: &dyn Fn() -> bool| {
            for _ in 0..100 {
                if done() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            false
        };
        let hue_near = |rgb: u32| {
            let hue = Hct::from_color(DynamicTonalPalette::from_seed(Color::hex(rgb)).accent1[8]).hue();
            let colors = &colors;
            move || colors.palette().is_some_and(|p| (Hct::from_color(p.accent1[8]).hue() - hue).abs() < 8.0)
        };
        assert!(wait_for(&hue_near(0xD32F2F)));
        let t0 = Instant::now();
        write_config(&format!("file://{}", blue.display()));
        assert!(wait_for(&hue_near(0x1976D2)));
        let seen = t0.elapsed();
        // By inotify, after the change settled (not by looking again a while later).
        assert!(seen >= Duration::from_millis(250) && seen < Duration::from_secs(3), "{seen:?}");
        // Another file with the same colors: not notified, the palette being the same.
        notified.store(0, Ordering::SeqCst);
        write_config(blue2.to_str().unwrap());
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(notified.load(Ordering::SeqCst), 0);
        // A picture that isn't there: none.
        write_config(home.join("no-such-picture.png").to_str().unwrap());
        assert!(wait_for(&|| colors.palette().is_none()));
        // Off: nothing.
        let off = WallpaperOptions { config_home: home.clone(), desktop: "KDE".into(), enabled: false };
        let none = WallpaperColors::new(false, Arc::new(|| {}), off);
        assert!(!none.listening() && none.palette().is_none());
        drop(colors);
        let _ = std::fs::remove_dir_all(&home);
    }

    /// The app quits while a big wallpaper's palette is being worked out (decoding it takes seconds in a debug build,
    /// most of it in one call): the desktop's services stop at once all the same, the palette never told. And the
    /// wallpaper changed meanwhile: the new one's palette comes without waiting for the big one's.
    pub fn stopping_never_waits_for_a_palette() {
        let home = std::env::temp_dir().join(format!("kompoz-platform-stop-tests-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let big = home.join("big.bmp");
        write_bmp_sized(&big, 0x1976D2, 4000, 3000);
        write_kde_config(&home, big.to_str().unwrap());
        // How long its palette takes here (else the stop below would prove nothing).
        let t0 = Instant::now();
        assert!(palette_of(big.to_str().unwrap()).is_some());
        let alone = t0.elapsed();
        println!("  the big wallpaper's palette alone: {alone:?}");
        if alone < Duration::from_millis(300) {
            println!("  (too quick to stop while it is being worked out: an optimized build)");
            let _ = std::fs::remove_dir_all(&home);
            return;
        }

        let notified = Arc::new(AtomicUsize::new(0));
        let n = notified.clone();
        let mut desktop = DesktopServices::new();
        let options = DesktopOptions {
            portal: PortalOptions { address: None, accent: false },
            atspi: AtspiOptions { forced: Some(false), ..AtspiOptions::default() },
            wallpaper: WallpaperOptions { config_home: home.clone(), desktop: "KDE".into(), enabled: true },
        };
        let wake = Arc::new(move || {
            n.fetch_add(1, Ordering::SeqCst);
        });
        desktop.start_with("Test app", wake, options);
        std::thread::sleep(Duration::from_millis(100)); // (its palette being worked out)
        let t0 = Instant::now();
        desktop.stop();
        let stopping = t0.elapsed();
        println!("  stopped in {stopping:?}");
        assert!(stopping < Duration::from_millis(100), "stopping took {stopping:?}");
        assert_eq!(notified.load(Ordering::SeqCst), 0, "the palette came before the stop: nothing was proved");
        // Never told afterwards either (its result goes nowhere).
        std::thread::sleep(alone + Duration::from_millis(300));
        assert_eq!(notified.load(Ordering::SeqCst), 0);

        // Changed while the big one's is being worked out: that one stops, the new one's is told.
        let red = home.join("red.bmp");
        write_bmp(&red, 0xD32F2F);
        let options = WallpaperOptions { config_home: home.clone(), desktop: "KDE".into(), enabled: true };
        let colors = WallpaperColors::new(false, Arc::new(|| {}), options);
        std::thread::sleep(Duration::from_millis(100));
        let start = Instant::now();
        write_kde_config(&home, red.to_str().unwrap());
        let mut hue = None;
        while start.elapsed() < Duration::from_secs(5) && hue.is_none() {
            hue = colors.palette().map(|p| Hct::from_color(p.accent1[8]).hue());
            std::thread::sleep(Duration::from_millis(20));
        }
        let took = start.elapsed();
        let red_hue = Hct::from_color(DynamicTonalPalette::from_seed(Color::hex(0xD32F2F)).accent1[8]).hue();
        let hue = hue.expect("the new wallpaper's palette");
        assert!((hue - red_hue).abs() < 8.0, "the red one's palette, not the big blue one's: hue {hue}");
        assert!(took < alone, "the red one waited for the big one: {took:?} (the big one's alone: {alone:?})");
        let t0 = Instant::now();
        drop(colors);
        assert!(t0.elapsed() < Duration::from_millis(100), "stopping took {:?}", t0.elapsed());
        let _ = std::fs::remove_dir_all(&home);
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        let tests: &[(&str, fn())] = &[
            ("kde_config_is_read", linux::kde_config_is_read),
            ("wallpaper_changes_are_followed", linux::wallpaper_changes_are_followed),
            ("stopping_never_waits_for_a_palette", linux::stopping_never_waits_for_a_palette),
        ];
        let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
        let mut failed = 0;
        for (name, test) in tests {
            if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                continue;
            }
            let ok = std::panic::catch_unwind(test).is_ok();
            failed += usize::from(!ok);
            println!("{} {name}", if ok { "ok    " } else { "FAILED" });
        }
        if failed > 0 {
            std::process::exit(1);
        }
    }
}
