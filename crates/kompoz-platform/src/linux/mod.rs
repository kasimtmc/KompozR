//! KompozR's Linux platforms — Wayland ([`wayland`]) and X11 ([`x11`]) — and what they share: the kernel's facilities
//! as safe functions ([`sys`]), the event loop ([`event_loop`]), keyboards through xkbcommon ([`xkb`]), the desktop's
//! links, dropped files and services ([`desktop`]: [`DesktopServices`](desktop::DesktopServices), what a platform
//! runs them with), and what those are made of: our own D-Bus client ([`dbus`]), the settings portal ([`portal`]),
//! the screen readers' AT-SPI bridge ([`atspi`]), the wallpaper's colors ([`wallpaper`]), a microphone and a camera
//! ([`media`]); and the images' HTTP client, libcurl ([`curl`]).

pub mod atspi;
pub mod curl;
pub mod dbus;
pub mod desktop;
pub mod event_loop;
pub mod media;
pub mod portal;
pub mod sys;
pub mod transfer;
pub mod wallpaper;
pub mod wayland;
pub mod x11;
pub mod xkb;
