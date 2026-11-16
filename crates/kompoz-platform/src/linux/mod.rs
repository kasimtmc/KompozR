//! What KompozR's Linux platforms (Wayland, X11) share: the kernel's facilities as safe functions ([`sys`]), the
//! event loop ([`event_loop`]), keyboards through xkbcommon ([`xkb`]), and the desktop's links and dropped files
//! ([`desktop`]).

pub mod desktop;
pub mod event_loop;
pub mod sys;
pub mod xkb;
