//! What the macOS platform asks of the system besides its windows (C++'s MacPlatform's services): the theme (the
//! app's effective appearance, dark or light), the accent color (System Settings' `controlAccentColor`: the dynamic
//! palette's seed), reduced motion, the languages and the time zone (what Foundation says, for the runtime's
//! environment: an app started from Finder has no `LANG`), the cursors, links opened by the workspace, a microphone
//! and a camera looked for (AVFoundation's default devices), errors shown in an alert, the app bundle's resources as
//! the app's assets, pictures as AppKit's images and the pasteboard's image formats.

#![allow(unsafe_code)]

use kompoz_graphics::{Color, encode_png};
use kompoz_ui::platform::PointerIcon;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{AnyThread, MainThreadMarker, msg_send};
use objc2_app_kit::{
    NSAlert, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSBitmapImageFileType, NSBitmapImageRep,
    NSColor, NSColorSpace, NSCursor, NSImage, NSScreen, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSBundle, NSData, NSDictionary, NSLocale, NSObjectProtocol, NSSize, NSString, NSTimeZone, NSURL,
};
use std::path::PathBuf;

/// Whether the app's appearance is dark (C++'s: the best match of Aqua and Dark Aqua for the app's effective one,
/// what System Settings' appearance and the app's own setting make it).
pub fn dark_mode(app: &NSApplication) -> bool {
    // SAFETY: AppKit's appearance names, static strings valid for the process.
    let (aqua, dark) = unsafe { (NSAppearanceNameAqua, NSAppearanceNameDarkAqua) };
    let names = NSArray::from_slice(&[aqua, dark]);
    app.effectiveAppearance().bestMatchFromAppearancesWithNames(&names).is_some_and(|name| &*name == dark)
}

/// The user's accent color (System Settings > Appearance; "multicolor" is the system's blue), in sRGB; none with
/// `KOMPOZ_NO_DESKTOP_ACCENT` (as on Linux: shots that don't depend on the system's).
pub fn accent_color() -> Option<Color> {
    if kompoz_runtime::environment::var_os("KOMPOZ_NO_DESKTOP_ACCENT").is_some() {
        return None;
    }
    let color = NSColor::controlAccentColor().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
    let c = |v: f64| v.clamp(0.0, 1.0) as f32;
    Some(Color::new(c(color.redComponent()), c(color.greenComponent()), c(color.blueComponent()), 1.0))
}

/// Whether the user asks for less motion (Accessibility > Display > Reduce motion).
pub fn reduce_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

/// The languages the user prefers (System Settings > Language & Region, "tr-TR" …) and the system's time zone, to
/// the runtime's environment (what KompozR's locale and zones read where the environment says nothing).
pub fn publish_locale() {
    let languages: Vec<String> = NSLocale::preferredLanguages().iter().map(|l| l.to_string()).collect();
    kompoz_runtime::environment::set_system_languages(Some(languages));
    NSTimeZone::resetSystemTimeZone();
    let zone = NSTimeZone::systemTimeZone().name().to_string();
    kompoz_runtime::environment::set_system_time_zone(Some(zone));
}

/// The system's cursor for `icon` (C++'s: a closed hand for moving; the wait cursor is the system's own to show). (The
/// resize cursors macOS 15 deprecated have no replacement before it.)
#[allow(deprecated)]
pub fn cursor(icon: PointerIcon) -> Retained<NSCursor> {
    match icon {
        PointerIcon::Default | PointerIcon::Wait => NSCursor::arrowCursor(),
        PointerIcon::Hand => NSCursor::pointingHandCursor(),
        PointerIcon::Text => NSCursor::IBeamCursor(),
        PointerIcon::Crosshair => NSCursor::crosshairCursor(),
        PointerIcon::Move => NSCursor::closedHandCursor(),
        PointerIcon::ResizeHorizontal => NSCursor::resizeLeftRightCursor(),
        PointerIcon::ResizeVertical => NSCursor::resizeUpDownCursor(),
        PointerIcon::NotAllowed => NSCursor::operationNotAllowedCursor(),
    }
}

/// A link opened by the workspace (its app for the scheme; a path without one: the file's app); whether it was.
pub fn open_uri(uri: &str) -> bool {
    let url = if uri.contains("://") || uri.starts_with("mailto:") || uri.starts_with("tel:") {
        NSURL::URLWithString(&NSString::from_str(uri))
    } else {
        Some(NSURL::fileURLWithPath(&NSString::from_str(uri)))
    };
    url.is_some_and(|url| NSWorkspace::sharedWorkspace().openURL(&url))
}

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVMediaTypeAudio: &'static NSString;
    static AVMediaTypeVideo: &'static NSString;
}

/// Whether the system has a microphone and a camera (C++'s: AVFoundation's default devices for audio and video; asking
/// for them needs no permission, using them would).
pub fn probe_media() -> (bool, bool) {
    let Some(class) = AnyClass::get(c"AVCaptureDevice") else { return (false, false) };
    let has = |kind: &NSString| -> bool {
        // SAFETY: `+[AVCaptureDevice defaultDeviceWithMediaType:]` takes a media type and returns a device or nil.
        let device: Option<Retained<AnyObject>> = unsafe { msg_send![class, defaultDeviceWithMediaType: kind] };
        device.is_some()
    };
    // SAFETY: AVFoundation's media type constants, static strings valid for the process (the framework is linked).
    let (audio, video) = unsafe { (AVMediaTypeAudio, AVMediaTypeVideo) };
    (has(audio), has(video))
}

/// An alert with `message` under `title` (C++'s `showError`), until the user closes it.
pub fn show_error(mtm: MainThreadMarker, title: &str, message: &str) {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    alert.runModal();
}

/// The app bundle's resources directory, when the program runs from one (`*.app/Contents/MacOS`): where the app's
/// assets are (C++'s `programDirectory` there).
pub fn bundle_resources() -> Option<PathBuf> {
    let bundle = NSBundle::mainBundle();
    if !bundle.bundlePath().to_string().ends_with(".app") {
        return None;
    }
    bundle.resourcePath().map(|p| PathBuf::from(p.to_string()))
}

/// The refresh rate of `screen` (macOS 12's `maximumFramesPerSecond`; 60 where it says nothing, or before 12).
pub fn refresh_rate(screen: Option<&NSScreen>) -> f32 {
    let screen = screen.filter(|s| s.respondsToSelector(objc2::sel!(maximumFramesPerSecond)));
    match screen.map(|s| s.maximumFramesPerSecond()) {
        Some(fps) if fps > 0 => fps as f32,
        _ => 60.0,
    }
}

/// An image of AppKit's from straight RGBA pixels (through PNG, which AppKit reads as it is), shown `points` wide
/// (its height as its proportions say; 0: its pixels as points).
pub fn image_of_rgba(width: i32, height: i32, rgba: &[u8], points: f64) -> Option<Retained<NSImage>> {
    let png = encode_png(width, height, rgba)?;
    let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(&png))?;
    if points > 0.0 && width > 0 {
        image.setSize(NSSize::new(points, points * height as f64 / width as f64));
    }
    Some(image)
}

/// A TIFF (or any picture AppKit reads) as PNG.
pub fn png_of(bytes: &[u8]) -> Option<Vec<u8>> {
    let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(bytes))?;
    let properties = NSDictionary::new();
    // SAFETY: an empty properties dictionary is a valid one for any file type.
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties) }?;
    Some(png.to_vec())
}

/// A PNG as TIFF (what older apps read from the pasteboard).
pub fn tiff_of(png: &[u8]) -> Option<Vec<u8>> {
    let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(png))?;
    rep.TIFFRepresentation().map(|tiff| tiff.to_vec())
}
