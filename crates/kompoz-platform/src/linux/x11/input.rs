//! XInput 2's events as KompozR's (C++'s `input`): the pointer, its buttons, its wheel (smooth from the scroll
//! valuators, else the wheel's buttons 4 to 7), touch, touchpad pinches (XInput 2.4), the devices coming and going.
//! Kept apart from the display so that it can be tested as it is.

use super::ffi::{
    self, DeviceInfo, XI_BUTTON_PRESS, XI_BUTTON_RELEASE, XI_GESTURE_PINCH_BEGIN, XI_GESTURE_PINCH_END,
    XI_GESTURE_PINCH_UPDATE, XI_MOTION, XI_NOTIFY_NORMAL, XI_NOTIFY_UNGRAB, XI_POINTER_EMULATED, XI_SLAVE_POINTER,
    XI_TOUCH_BEGIN, XI_TOUCH_END, XiDevice, XiEvent,
};
use crate::app::{Event, MouseButton};
use kompoz_ui::input::KeyModifiers;
use std::collections::HashMap;

/// The modifiers of an X state mask (Compose's: Shift, Ctrl, Alt (Mod1) and Meta (Mod4, the Super key) apart; C++
/// took Super as Ctrl).
pub fn modifiers_of(state: u32) -> KeyModifiers {
    let mut m = KeyModifiers::None;
    for (mask, flag) in [
        (ffi::SHIFT_MASK, KeyModifiers::Shift),
        (ffi::CONTROL_MASK, KeyModifiers::Ctrl),
        (ffi::MOD1_MASK, KeyModifiers::Alt),
        (ffi::MOD4_MASK, KeyModifiers::Meta),
    ] {
        if state & mask != 0 {
            m = m | flag;
        }
    }
    m
}

/// A scroll valuator of a device (XInput 2.1's smooth scrolling): its axis, how much is a notch, and where it was
/// last (none: not known since the pointer came in).
#[derive(Clone, Copy, PartialEq, Debug)]
struct ScrollAxis {
    vertical: bool,
    increment: f64,
    last: Option<f64>,
}

/// The devices as the input needs them: their scroll valuators, whether there is a mouse and a touch screen.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Devices {
    /// By device, then valuator.
    axes: HashMap<i32, HashMap<usize, ScrollAxis>>,
    pub has_mouse: bool,
    pub has_touch: bool,
}

impl Devices {
    /// What XInput 2 says the devices are (C++'s `queryScrollAxes`).
    pub fn new(devices: &[DeviceInfo]) -> Devices {
        let mut d = Devices::default();
        for info in devices {
            for &(number, vertical, increment) in &info.scroll {
                d.axes.entry(info.id).or_default().insert(number, ScrollAxis { vertical, increment, last: None });
            }
            if info.use_ == XI_SLAVE_POINTER {
                if info.direct_touch {
                    d.has_touch = true;
                } else {
                    d.has_mouse = true;
                }
            }
        }
        d
    }

    /// The pointer of `device` came in or left: where its valuators were isn't known any more (it may have scrolled
    /// elsewhere).
    pub fn forget_positions(&mut self, device: i32) {
        if let Some(axes) = self.axes.get_mut(&device) {
            axes.values_mut().for_each(|a| a.last = None);
        }
    }

    /// The wheel, smooth: each scroll valuator's change since the last, in notches (its increment), up and right
    /// positive; (0, 0) if none moved (or it is the first value since the pointer came in).
    pub fn scrolled(&mut self, device: i32, valuators: &[(usize, f64)]) -> (f64, f64) {
        let (mut dx, mut dy) = (0.0, 0.0);
        let Some(axes) = self.axes.get_mut(&device) else { return (0.0, 0.0) };
        for &(number, value) in valuators {
            let Some(axis) = axes.get_mut(&number) else { continue };
            if let Some(last) = axis.last {
                let notches = (value - last) / axis.increment;
                if axis.vertical {
                    dy -= notches;
                } else {
                    dx += notches;
                }
            }
            axis.last = Some(value);
        }
        (dx, dy)
    }
}

/// XInput 2's events turned into KompozR's: the devices' valuators, the pinch's scale so far.
#[derive(Debug)]
pub struct Input {
    pub devices: Devices,
    pinch_scale: f64,
}

impl Input {
    pub fn new(devices: Devices) -> Input {
        Input { devices, pinch_scale: 1.0 }
    }

    /// The events `e` is (at `now` on the platform's clock); for a hierarchy change, the devices are asked again by
    /// the caller (`devices_changed`) and [`Event::DevicesChanged`] follows.
    pub fn events(&mut self, e: &XiEvent, now: i64) -> Vec<Event> {
        match e {
            XiEvent::Hierarchy | XiEvent::Other => Vec::new(),
            &XiEvent::Pinch { evtype, x, y, scale, delta_angle, .. } => {
                let (x, y) = (x as f32, y as f32);
                match evtype {
                    XI_GESTURE_PINCH_BEGIN => {
                        self.pinch_scale = 1.0;
                        vec![Event::PinchBegin { x, y }]
                    }
                    // (The scale since the pinch began: each step the change since the last.)
                    XI_GESTURE_PINCH_UPDATE => {
                        let step = scale / self.pinch_scale.max(1e-3);
                        self.pinch_scale = scale;
                        vec![Event::Pinch { x, y, scale: step as f32, rotation: delta_angle as f32 }]
                    }
                    XI_GESTURE_PINCH_END => vec![Event::PinchEnd { x, y }],
                    _ => Vec::new(),
                }
            }
            &XiEvent::Crossing { enter, source, x, y, mode, mods } => {
                self.devices.forget_positions(source);
                if mode != XI_NOTIFY_NORMAL && mode != XI_NOTIFY_UNGRAB {
                    return Vec::new();
                }
                if !enter {
                    return vec![Event::PointerLeft];
                }
                vec![Event::MouseMove { x: x as f32, y: y as f32, modifiers: modifiers_of(mods), time: now }]
            }
            XiEvent::Device(d) => self.device_events(d, now),
        }
    }

    fn device_events(&mut self, d: &XiDevice, now: i64) -> Vec<Event> {
        // (A touch's own pointer, or the wheel's buttons for smooth scrolling.)
        let emulated = d.flags & XI_POINTER_EMULATED != 0;
        let modifiers = modifiers_of(d.mods);
        let (x, y) = (d.x as f32, d.y as f32);
        match d.evtype {
            XI_MOTION => {
                if emulated {
                    return Vec::new();
                }
                let mut events = vec![Event::MouseMove { x, y, modifiers, time: now }];
                let (dx, dy) = self.devices.scrolled(d.source, &d.valuators);
                if dx != 0.0 || dy != 0.0 {
                    events.push(Event::Wheel { x, y, dx: dx as f32, dy: dy as f32, modifiers, time: now });
                }
                events
            }
            XI_BUTTON_PRESS | XI_BUTTON_RELEASE => {
                let press = d.evtype == XI_BUTTON_PRESS;
                if (4..=7).contains(&d.detail) {
                    // The wheel's notches, where it isn't smooth.
                    if emulated || !press {
                        return Vec::new();
                    }
                    let dy = match d.detail {
                        4 => 1.0,
                        5 => -1.0,
                        _ => 0.0,
                    };
                    let dx = match d.detail {
                        6 => -1.0,
                        7 => 1.0,
                        _ => 0.0,
                    };
                    return vec![Event::Wheel { x, y, dx, dy, modifiers, time: now }];
                }
                if emulated {
                    return Vec::new();
                }
                let button = match d.detail {
                    1 => MouseButton::Left,
                    2 => MouseButton::Middle,
                    3 => MouseButton::Right,
                    8 => MouseButton::Back,
                    9 => MouseButton::Forward,
                    _ => return Vec::new(),
                };
                vec![if press {
                    Event::MouseDown { x, y, button, modifiers, time: now }
                } else {
                    Event::MouseUp { x, y, button, modifiers, time: now }
                }]
            }
            XI_TOUCH_BEGIN => vec![Event::TouchDown { pointer: d.detail as i64, x, y, time: now }],
            XI_TOUCH_END => vec![Event::TouchUp { pointer: d.detail as i64, x, y, time: now }],
            _ => vec![Event::TouchMove { pointer: d.detail as i64, x, y, time: now }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::x11::ffi::{XI_NOTIFY_UNGRAB, XI_TOUCH_UPDATE};

    fn touchpad() -> Devices {
        Devices::new(&[
            DeviceInfo { id: 2, use_: 1, scroll: vec![], direct_touch: false },
            DeviceInfo {
                id: 11,
                use_: XI_SLAVE_POINTER,
                scroll: vec![(2, false, 15.0), (3, true, 15.0)],
                direct_touch: false,
            },
        ])
    }

    fn device(evtype: i32, detail: i32, flags: i32, valuators: Vec<(usize, f64)>) -> XiEvent {
        XiEvent::Device(XiDevice {
            evtype,
            time: 0,
            device: 2,
            source: 11,
            detail,
            x: 10.0,
            y: 20.0,
            root_x: 110.0,
            root_y: 220.0,
            flags,
            mods: 0,
            valuators,
        })
    }

    #[test]
    fn devices_tell_mice_touch_screens_and_scroll_axes() {
        let d = touchpad();
        assert!(d.has_mouse && !d.has_touch);
        let touch = Devices::new(&[DeviceInfo { id: 12, use_: XI_SLAVE_POINTER, scroll: vec![], direct_touch: true }]);
        assert!(touch.has_touch && !touch.has_mouse);
        assert!(!Devices::new(&[]).has_mouse);
    }

    #[test]
    fn smooth_scrolling_is_each_valuators_change_in_notches() {
        let mut input = Input::new(touchpad());
        // The first values only tell where the valuators are.
        let first = input.events(&device(XI_MOTION, 0, 0, vec![(2, 100.0), (3, 300.0)]), 5);
        assert!(matches!(first.as_slice(), [Event::MouseMove { x: 10.0, y: 20.0, time: 5, .. }]));
        // Down 30 (two notches: scrolled down, -2) and right 7.5 (half a notch).
        let e = input.events(&device(XI_MOTION, 0, 0, vec![(2, 107.5), (3, 330.0)]), 6);
        assert!(matches!(e[1], Event::Wheel { dx: 0.5, dy: -2.0, .. }), "{e:?}");
        // A valuator not a scroll axis (0: x) moves nothing.
        assert_eq!(input.events(&device(XI_MOTION, 0, 0, vec![(0, 50.0)]), 7).len(), 1);
        // Left and come back: the next value is a start again.
        let leave = XiEvent::Crossing { enter: false, source: 11, x: 0.0, y: 0.0, mode: XI_NOTIFY_NORMAL, mods: 0 };
        assert_eq!(input.events(&leave, 8), [Event::PointerLeft]);
        assert_eq!(input.events(&device(XI_MOTION, 0, 0, vec![(3, 900.0)]), 9).len(), 1);
        let up = input.events(&device(XI_MOTION, 0, 0, vec![(3, 885.0)]), 10);
        assert!(matches!(up[1], Event::Wheel { dx: 0.0, dy: 1.0, .. }));
        // A touch's emulated pointer is left out.
        assert!(input.events(&device(XI_MOTION, 0, XI_POINTER_EMULATED, vec![]), 11).is_empty());
    }

    #[test]
    fn buttons_are_mouse_buttons_or_the_wheels_notches() {
        let mut input = Input::new(touchpad());
        let press = |i: &mut Input, detail, flags| i.events(&device(XI_BUTTON_PRESS, detail, flags, vec![]), 1);
        assert!(matches!(press(&mut input, 1, 0)[..], [Event::MouseDown { button: MouseButton::Left, .. }]));
        assert!(matches!(press(&mut input, 2, 0)[..], [Event::MouseDown { button: MouseButton::Middle, .. }]));
        assert!(matches!(press(&mut input, 3, 0)[..], [Event::MouseDown { button: MouseButton::Right, .. }]));
        assert!(matches!(press(&mut input, 8, 0)[..], [Event::MouseDown { button: MouseButton::Back, .. }]));
        assert!(matches!(press(&mut input, 9, 0)[..], [Event::MouseDown { button: MouseButton::Forward, .. }]));
        assert!(press(&mut input, 10, 0).is_empty());
        let release = input.events(&device(XI_BUTTON_RELEASE, 1, 0, vec![]), 1);
        assert!(matches!(release[..], [Event::MouseUp { button: MouseButton::Left, x: 10.0, y: 20.0, .. }]));
        // The wheel's buttons: up, down, left, right; their releases and their smooth-scrolling copies left out.
        assert!(matches!(press(&mut input, 4, 0)[..], [Event::Wheel { dx: 0.0, dy: 1.0, .. }]));
        assert!(matches!(press(&mut input, 5, 0)[..], [Event::Wheel { dx: 0.0, dy: -1.0, .. }]));
        assert!(matches!(press(&mut input, 6, 0)[..], [Event::Wheel { dx: -1.0, dy: 0.0, .. }]));
        assert!(matches!(press(&mut input, 7, 0)[..], [Event::Wheel { dx: 1.0, dy: 0.0, .. }]));
        assert!(input.events(&device(XI_BUTTON_RELEASE, 4, 0, vec![]), 1).is_empty());
        assert!(press(&mut input, 4, XI_POINTER_EMULATED).is_empty());
        assert!(press(&mut input, 1, XI_POINTER_EMULATED).is_empty());
    }

    #[test]
    fn touches_crossings_and_pinches() {
        let mut input = Input::new(touchpad());
        let t = |i: &mut Input, evtype| i.events(&device(evtype, 7, 0, vec![]), 3);
        assert_eq!(t(&mut input, XI_TOUCH_BEGIN), [Event::TouchDown { pointer: 7, x: 10.0, y: 20.0, time: 3 }]);
        assert_eq!(t(&mut input, XI_TOUCH_UPDATE), [Event::TouchMove { pointer: 7, x: 10.0, y: 20.0, time: 3 }]);
        assert_eq!(t(&mut input, XI_TOUCH_END), [Event::TouchUp { pointer: 7, x: 10.0, y: 20.0, time: 3 }]);
        let enter = XiEvent::Crossing { enter: true, source: 11, x: 4.0, y: 5.0, mode: XI_NOTIFY_UNGRAB, mods: 1 };
        assert_eq!(
            input.events(&enter, 4),
            [Event::MouseMove { x: 4.0, y: 5.0, modifiers: KeyModifiers::Shift, time: 4 }]
        );
        let grabbed = XiEvent::Crossing { enter: false, source: 11, x: 0.0, y: 0.0, mode: 1, mods: 0 };
        assert!(input.events(&grabbed, 5).is_empty());
        let pinch = |evtype, scale, delta_angle| XiEvent::Pinch { evtype, x: 1.0, y: 2.0, scale, delta_angle, mods: 0 };
        assert_eq!(input.events(&pinch(XI_GESTURE_PINCH_BEGIN, 1.0, 0.0), 6), [Event::PinchBegin { x: 1.0, y: 2.0 }]);
        assert_eq!(
            input.events(&pinch(XI_GESTURE_PINCH_UPDATE, 1.5, 3.0), 7),
            [Event::Pinch { x: 1.0, y: 2.0, scale: 1.5, rotation: 3.0 }]
        );
        assert_eq!(
            input.events(&pinch(XI_GESTURE_PINCH_UPDATE, 3.0, -1.0), 8),
            [Event::Pinch { x: 1.0, y: 2.0, scale: 2.0, rotation: -1.0 }]
        );
        assert_eq!(input.events(&pinch(XI_GESTURE_PINCH_END, 3.0, 0.0), 9), [Event::PinchEnd { x: 1.0, y: 2.0 }]);
    }

    #[test]
    fn state_masks_are_composes_modifiers() {
        assert_eq!(modifiers_of(0), KeyModifiers::None);
        assert_eq!(modifiers_of(1 | 4), KeyModifiers::Shift | KeyModifiers::Ctrl);
        assert_eq!(modifiers_of(8 | 64), KeyModifiers::Alt | KeyModifiers::Meta);
        assert_eq!(modifiers_of(2 | 16 | (1 << 13)), KeyModifiers::None); // (Lock, NumLock, the group: none of them)
    }
}
