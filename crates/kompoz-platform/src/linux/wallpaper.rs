//! The wallpaper's own colors on Linux desktops, the way Android 12 takes them from the wallpaper (C++'s
//! common/wallpaper_colors.cpp): the wallpaper found (KDE Plasma's, GNOME's and the desktops' that keep its
//! settings, Cinnamon's, MATE's, Xfce's), decoded small and its palette picked
//! ([`DynamicTonalPalette::from_image`], the core Windows shares: [`wallpaper_palette`](crate::wallpaper_palette)) on
//! a thread of its own, and again whenever it changes — told by inotify when the desktop's settings files are
//! written, looked at again every 30 seconds (a picture replaced in place); it notifies as each palette comes. A
//! palette is worked out on a thread of its own again, which stopping (the app quitting) never waits for. `KOMPOZ_NO_WALLPAPER_COLORS` leaves it out (screenshots the
//! same on every machine).

use super::desktop::Notify;
use super::sys::{self, PollFd};
use crate::wallpaper_palette::PaletteWork;
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

/// The settings files whose writing may change the wallpaper (KDE's, its activities', dconf's, Xfce's).
const SETTINGS_FILES: [&str; 4] =
    ["plasma-org.kde.plasma.desktop-appletsrc", "kactivitymanagerdrc", "user", "xfce4-desktop.xml"];

/// Where the desktop keeps its settings, and which desktop it is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WallpaperOptions {
    /// `$XDG_CONFIG_HOME` (else `~/.config`).
    pub config_home: PathBuf,
    /// `$XDG_CURRENT_DESKTOP` ("KDE", "GNOME", "ubuntu:GNOME").
    pub desktop: String,
    /// Not `KOMPOZ_NO_WALLPAPER_COLORS`.
    pub enabled: bool,
}

impl WallpaperOptions {
    pub fn from_env() -> WallpaperOptions {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
        let config_home = var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(var("HOME").unwrap_or_default()).join(".config"));
        WallpaperOptions {
            config_home,
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            enabled: std::env::var_os("KOMPOZ_NO_WALLPAPER_COLORS").is_none(),
        }
    }
}

/// A path from a `file://` URI (or a path as it is), `%XX` decoded.
fn path_from_uri(s: &str) -> String {
    let s = s.strip_prefix("file://").unwrap_or(s);
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A command's output, trimmed; empty if it couldn't run.
fn command_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// A gsettings key's value as a path (its quotes and `file://` taken off); empty if it has none.
fn gsetting(schema: &str, key: &str) -> String {
    let v = command_output("gsettings", &["get", schema, key]);
    let v = v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')).unwrap_or(&v);
    if v.is_empty() { String::new() } else { path_from_uri(v) }
}

fn xfce_wallpaper() -> String {
    let properties = command_output("xfconf-query", &["-c", "xfce4-desktop", "-l"]);
    properties
        .lines()
        .find(|line| line.ends_with("/last-image"))
        .map(|line| command_output("xfconf-query", &["-c", "xfce4-desktop", "-p", line]))
        .unwrap_or_default()
}

/// A directory's files' names, sorted.
fn list_dir(dir: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// The area a KDE package's picture's name says ("5120x2880.png"): 0 if it says none.
fn named_area(file: &str) -> i64 {
    let stem = Path::new(file).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
    let n = digits(&stem);
    let Some(rest) = stem[n..].strip_prefix('x') else { return 0 };
    let m = digits(rest);
    match (stem[..n].parse::<i64>(), rest[..m].parse::<i64>()) {
        (Ok(w), Ok(h)) => w * h,
        _ => 0,
    }
}

/// A KDE wallpaper package's picture (/usr/share/wallpapers/Next/): the biggest in contents/images
/// (images_dark for the dark theme, where it has one), by the WxH its name says.
fn package_image(dir: &str, dark: bool, read_dir: &dyn Fn(&str) -> Vec<String>) -> Option<String> {
    let subs: &[&str] = if dark { &["/contents/images_dark", "/contents/images"] } else { &["/contents/images"] };
    for sub in subs {
        let mut best: Option<(i64, String)> = None;
        for f in read_dir(&format!("{dir}{sub}")) {
            let area = named_area(&f);
            if best.as_ref().is_none_or(|(b, _)| area > *b) {
                best = Some((area, f));
            }
        }
        if let Some((_, file)) = best {
            return Some(format!("{dir}{sub}/{file}"));
        }
    }
    None
}

/// Where KDE Plasma's wallpaper is, as `config` (the text of plasma-org.kde.plasma.desktop-appletsrc) says, for
/// the dark theme or the light one: the desktop (not a panel) of the first screen, in `activity` (the current one's
/// id, or empty), showing a picture; a package's best picture; none if it says none. `read_dir` lists a directory's
/// files.
pub fn kde_wallpaper_from_config(
    config: &str,
    activity: &str,
    dark: bool,
    read_dir: &dyn Fn(&str) -> Vec<String>,
) -> Option<String> {
    // [Containments][N] has the desktop's activity, screen, location and wallpaper plugin;
    // [Containments][N][Wallpaper][org.kde.image][General] its Image.
    #[derive(Default)]
    struct Containment {
        keys: std::collections::HashMap<String, String>,
        image: String,
    }
    let mut containments: std::collections::BTreeMap<String, Containment> = Default::default();
    let mut section: Option<String> = None;
    let mut image_section = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = None;
            image_section = false;
            let Some(rest) = line.strip_prefix("[Containments][") else { continue };
            let Some(close) = rest.find(']') else { continue };
            let (id, rest) = (&rest[..close], &rest[close + 1..]);
            if rest.is_empty() {
                section = Some(id.to_string());
            } else if rest == "[Wallpaper][org.kde.image][General]" {
                section = Some(id.to_string());
                image_section = true;
            }
            continue;
        }
        let (Some(id), Some((key, value))) = (&section, line.split_once('=')) else { continue };
        let c = containments.entry(id.clone()).or_default();
        if image_section {
            if key == "Image" {
                c.image = value.to_string();
            }
        } else {
            c.keys.insert(key.to_string(), value.to_string());
        }
    }
    let key = |c: &Containment, k: &str| c.keys.get(k).cloned().unwrap_or_default();
    let mut best: Option<(i32, &str)> = None;
    for c in containments.values() {
        if c.image.is_empty() || key(c, "wallpaperplugin") != "org.kde.image" {
            continue;
        }
        let location = key(c, "location");
        if !location.is_empty() && location != "0" {
            continue; // a panel's
        }
        let score = if key(c, "lastScreen") == "0" { 2 } else { 0 }
            + if !activity.is_empty() && key(c, "activityId") == activity { 1 } else { 0 };
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, &c.image));
        }
    }
    let mut path = path_from_uri(best?.1);
    if path.ends_with('/') {
        path.pop();
    }
    Some(package_image(&path, dark, read_dir).unwrap_or(path))
}

/// Where the wallpaper is, as the desktop says, for the dark theme or the light one; none if it doesn't say.
pub fn find_wallpaper(dark: bool, options: &WallpaperOptions) -> Option<String> {
    let desktop = options.desktop.to_lowercase();
    let found = if desktop.contains("kde") {
        let config = std::fs::read_to_string(options.config_home.join("plasma-org.kde.plasma.desktop-appletsrc"))
            .unwrap_or_default();
        if config.is_empty() {
            return None;
        }
        let activities = std::fs::read_to_string(options.config_home.join("kactivitymanagerdrc")).unwrap_or_default();
        let activity = activities
            .lines()
            .filter_map(|line| line.strip_prefix("currentActivity="))
            .next_back()
            .map(|a| a.trim().to_string())
            .unwrap_or_default();
        kde_wallpaper_from_config(&config, &activity, dark, &list_dir)
            .or_else(|| {
                // Plasma's default, never changed.
                let next = "/usr/share/wallpapers/Next";
                Path::new(next).is_dir().then(|| package_image(next, dark, &list_dir)).flatten()
            })
            .unwrap_or_default()
    } else if desktop.contains("cinnamon") {
        gsetting("org.cinnamon.desktop.background", "picture-uri")
    } else if desktop.contains("mate") {
        gsetting("org.mate.background", "picture-filename")
    } else if desktop.contains("xfce") {
        xfce_wallpaper()
    } else if ["gnome", "unity", "budgie", "pantheon"].iter().any(|d| desktop.contains(d)) {
        let image = if dark { gsetting("org.gnome.desktop.background", "picture-uri-dark") } else { String::new() };
        if image.is_empty() { gsetting("org.gnome.desktop.background", "picture-uri") } else { image }
    } else {
        String::new()
    };
    (!found.is_empty()).then_some(found)
}

/// The wallpaper's palette (the core Windows shares: [`wallpaper_palette`](crate::wallpaper_palette)).
pub use crate::wallpaper_palette::palette_of;

/// A palette being worked out, waking the wallpaper's thread (its eventfd) when it comes.
struct Work(PaletteWork);

impl Work {
    fn start(path: String, wake: &Arc<OwnedFd>) -> Work {
        let wake = wake.clone();
        Work(PaletteWork::start(path, move || {
            let _ = sys::eventfd_write(wake.as_fd(), 1);
        }))
    }

    fn take(&self) -> Option<Option<DynamicTonalPalette>> {
        self.0.take()
    }
}

struct Shared {
    notify: Notify,
    running: AtomicBool,
    dark: AtomicBool,
    palette: Mutex<Option<DynamicTonalPalette>>,
}

impl Shared {
    /// The palette now: told if it changed.
    fn tell(&self, value: Option<DynamicTonalPalette>) {
        {
            let mut palette = self.palette.lock().unwrap_or_else(|e| e.into_inner());
            if *palette == value {
                return;
            }
            *palette = value;
        }
        (self.notify)();
    }
}

/// The wallpaper's colors, found and followed on a thread of its own (C++'s WallpaperColors).
pub struct WallpaperColors {
    shared: Arc<Shared>,
    /// What wakes the thread (an eventfd: the theme changed, stopping), and the thread.
    thread: Option<(Arc<OwnedFd>, JoinHandle<()>)>,
}

impl WallpaperColors {
    /// Starts looking (nothing comes where none is found), for the dark theme or the light one; `notify` is called
    /// on its thread as each palette comes.
    pub fn new(dark_theme: bool, notify: Notify, options: WallpaperOptions) -> WallpaperColors {
        let shared = Arc::new(Shared {
            notify,
            running: AtomicBool::new(true),
            dark: AtomicBool::new(dark_theme),
            palette: Mutex::new(None),
        });
        let thread = options
            .enabled
            .then(|| {
                let wake = Arc::new(sys::eventfd().ok()?);
                // The settings directories watched for files written whole or moved in (as KConfig and dconf write).
                let inotify = sys::inotify().ok().map(|fd| {
                    let config = &options.config_home;
                    let mask = libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_CREATE;
                    for dir in [config.clone(), config.join("dconf"), config.join("xfce4/xfconf/xfce-perchannel-xml")] {
                        let _ = sys::inotify_add_watch(fd.as_fd(), &dir, mask);
                    }
                    File::from(fd)
                });
                let (shared, their_wake) = (shared.clone(), wake.clone());
                let thread = std::thread::Builder::new()
                    .name("kompoz-wallpaper".into())
                    .spawn(move || run(&shared, &their_wake, inotify, &options))
                    .ok()?;
                Some((wake, thread))
            })
            .flatten();
        WallpaperColors { shared, thread }
    }

    /// Whether it was started: it notifies when the palette is learned or changes.
    pub fn listening(&self) -> bool {
        self.thread.is_some()
    }

    /// The latest palette; none: no wallpaper found (or it couldn't be read).
    pub fn palette(&self) -> Option<DynamicTonalPalette> {
        *self.shared.palette.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Some wallpapers have a picture for the dark theme (KDE's packages, GNOME's picture-uri-dark).
    pub fn set_dark_theme(&self, dark: bool) {
        if self.shared.dark.swap(dark, Ordering::SeqCst) != dark {
            self.wake();
        }
    }

    fn wake(&self) {
        if let Some((wake, _)) = &self.thread {
            let _ = sys::eventfd_write(wake.as_fd(), 1);
        }
    }
}

impl Drop for WallpaperColors {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::SeqCst);
        self.wake();
        if let Some((_, thread)) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Which wallpaper: its path and its file's time.
type Key = (Option<String>, Option<SystemTime>);

/// The thread: the wallpaper read whenever it may have changed (its path or its file's time differ), its palette
/// worked out on a thread of its own ([`Work`]) and told when it comes, unless the wallpaper changed again by then.
fn run(shared: &Shared, wake: &Arc<OwnedFd>, mut inotify: Option<File>, options: &WallpaperOptions) {
    // The wallpaper whose palette was told last, and the one whose palette is being worked out.
    let mut told: Option<Key> = None;
    let mut working: Option<(Key, Work)> = None;
    while shared.running.load(Ordering::SeqCst) {
        let path = find_wallpaper(shared.dark.load(Ordering::SeqCst), options);
        let stamp = path.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        let key: Key = (path.clone(), stamp);
        let ready = working.as_ref().filter(|(k, _)| *k == key).and_then(|(_, work)| work.take());
        if let Some(palette) = ready {
            working = None;
            told = Some(key);
            shared.tell(palette);
        } else if told.as_ref() == Some(&key) {
            working = None; // (back to the one told before another's palette came: that one stops)
        } else if working.as_ref().is_none_or(|(k, _)| *k != key) {
            working = None; // (one for a wallpaper no longer shown stops)
            match path {
                Some(path) => working = Some((key, Work::start(path, wake))),
                None => {
                    told = Some(key);
                    shared.tell(None);
                }
            }
        }
        wait_for_change(wake, inotify.as_mut());
    }
    // (A palette still being worked out is told to stop as `working` goes, and not waited for.)
}

/// Waits for a change: a settings file written (then the rest of the change, 300 ms), the theme changed, a palette
/// worked out or stopping, or a while (30 seconds with inotify, else 3).
fn wait_for_change(wake: &OwnedFd, inotify: Option<&mut File>) {
    let Some(inotify) = inotify else {
        let mut fds = [PollFd::readable(wake.as_raw_fd())];
        if sys::poll(&mut fds, Some(Duration::from_secs(3))).is_ok() && fds[0].ready {
            let _ = sys::eventfd_read(wake.as_fd());
        }
        return;
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return;
        }
        let mut fds = [PollFd::readable(wake.as_raw_fd()), PollFd::readable(inotify.as_raw_fd())];
        if sys::poll(&mut fds, Some(left)).is_err() {
            return;
        }
        if fds[0].ready {
            let _ = sys::eventfd_read(wake.as_fd());
            return;
        }
        if fds[1].ready && settings_written(inotify) {
            // The rest of the change (unless woken meanwhile: then the next wait returns at once).
            let mut fds = [PollFd::readable(wake.as_raw_fd())];
            let _ = sys::poll(&mut fds, Some(Duration::from_millis(300)));
            settings_written(inotify);
            return;
        }
    }
}

/// Whether the files the events read name were among the settings files (other files in ~/.config are not).
fn settings_written(inotify: &mut File) -> bool {
    let mut ours = false;
    let mut buf = [0u8; 4096];
    while let Ok(n) = inotify.read(&mut buf) {
        if n == 0 {
            break;
        }
        // struct inotify_event: wd (i32), mask, cookie, len (u32s), then the name, NUL-padded to len.
        let mut at = 0;
        while at + 16 <= n {
            let len = u32::from_ne_bytes(buf[at + 12..at + 16].try_into().expect("4 bytes")) as usize;
            let name = &buf[at + 16..(at + 16 + len).min(n)];
            let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(name.len())];
            if SETTINGS_FILES.iter().any(|f| f.as_bytes() == name) {
                ours = true;
            }
            at += 16 + len;
        }
    }
    ours
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_and_package_names_are_read() {
        assert_eq!(path_from_uri("file:///home/me/Other%20Pics/a.jpg"), "/home/me/Other Pics/a.jpg");
        assert_eq!(path_from_uri("/plain/path%2"), "/plain/path%2");
        assert_eq!(named_area("5120x2880.png"), 5120 * 2880);
        assert_eq!(named_area("1920x1080-dark.jpg"), 1920 * 1080);
        assert_eq!(named_area("screenshot.png"), 0);
        assert_eq!(named_area("x10.png"), 0);
    }
}
