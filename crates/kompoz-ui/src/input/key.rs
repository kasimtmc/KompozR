//! Keys and key events (Compose's `androidx.compose.ui.input.key` Key, KeyEvent and KeyEventType): a key is
//! Compose's key code (Android's key code in the high 32 bits), a key event its key, whether it went down or up,
//! the character it types and the modifier keys held.

use std::fmt;

/// A key (Compose's Key): Compose's key code, Android's key code packed into its high 32 bits (Compose's
/// `Key(nativeKeyCode)`). The keys are its constants.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key(pub u64);

impl Key {
    /// The key of a platform key code (Compose's `Key(nativeKeyCode)`).
    pub const fn native(code: i32) -> Key {
        Key(((code as i64) << 32) as u64)
    }

    /// Its platform key code (Compose's `nativeKeyCode`).
    pub const fn native_key_code(self) -> i32 {
        ((self.0 as i64) >> 32) as i32
    }

    /// The character it types on a US keyboard (shifted or not), 0 for none: what a key event without the
    /// platform's character carries.
    pub fn typed_code_point(self, shift: bool) -> i32 {
        let c: Option<char> = match self {
            k if (Key::A.0..=Key::Z.0).contains(&k.0) => {
                let i = (k.native_key_code() - Key::A.native_key_code()) as u8;
                Some(if shift { (b'A' + i) as char } else { (b'a' + i) as char })
            }
            k if (Key::Zero.0..=Key::Nine.0).contains(&k.0) => {
                let i = (k.native_key_code() - Key::Zero.native_key_code()) as usize;
                Some(if shift {
                    [')', '!', '@', '#', '$', '%', '^', '&', '*', '('][i]
                } else {
                    (b'0' + i as u8) as char
                })
            }
            k if (Key::NumPad0.0..=Key::NumPad9.0).contains(&k.0) => {
                Some((b'0' + (k.native_key_code() - Key::NumPad0.native_key_code()) as u8) as char)
            }
            Key::Spacebar => Some(' '),
            Key::Tab => Some('\t'),
            Key::Enter | Key::NumPadEnter => Some('\n'),
            Key::Comma => Some(if shift { '<' } else { ',' }),
            Key::Period => Some(if shift { '>' } else { '.' }),
            Key::Minus => Some(if shift { '_' } else { '-' }),
            Key::Equals => Some(if shift { '+' } else { '=' }),
            Key::LeftBracket => Some(if shift { '{' } else { '[' }),
            Key::RightBracket => Some(if shift { '}' } else { ']' }),
            Key::Backslash => Some(if shift { '|' } else { '\\' }),
            Key::Semicolon => Some(if shift { ':' } else { ';' }),
            Key::Apostrophe => Some(if shift { '"' } else { '\'' }),
            Key::Slash => Some(if shift { '?' } else { '/' }),
            Key::Grave => Some(if shift { '~' } else { '`' }),
            Key::At => Some('@'),
            Key::Plus | Key::NumPadAdd => Some('+'),
            Key::Multiply | Key::NumPadMultiply => Some('*'),
            Key::Pound => Some('#'),
            Key::NumPadDivide => Some('/'),
            Key::NumPadSubtract => Some('-'),
            Key::NumPadDot => Some('.'),
            Key::NumPadComma => Some(','),
            Key::NumPadEquals => Some('='),
            Key::NumPadLeftParenthesis => Some('('),
            Key::NumPadRightParenthesis => Some(')'),
            _ => None,
        };
        c.map_or(0, |c| c as i32)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Key code: {}", self.0 as i64)
    }
}

#[allow(non_upper_case_globals)]
impl Key {
    /// Android's `KEYCODE_UNKNOWN`.
    pub const Unknown: Key = Key::native(0);
    /// Android's `KEYCODE_SOFT_LEFT`.
    pub const SoftLeft: Key = Key::native(1);
    /// Android's `KEYCODE_SOFT_RIGHT`.
    pub const SoftRight: Key = Key::native(2);
    /// Android's `KEYCODE_HOME`.
    pub const Home: Key = Key::native(3);
    /// Android's `KEYCODE_HOME`.
    pub const SystemHome: Key = Key::native(3);
    /// Android's `KEYCODE_BACK`.
    pub const Back: Key = Key::native(4);
    /// Android's `KEYCODE_HELP`.
    pub const Help: Key = Key::native(259);
    /// Android's `KEYCODE_NAVIGATE_PREVIOUS`.
    pub const NavigatePrevious: Key = Key::native(260);
    /// Android's `KEYCODE_NAVIGATE_NEXT`.
    pub const NavigateNext: Key = Key::native(261);
    /// Android's `KEYCODE_NAVIGATE_IN`.
    pub const NavigateIn: Key = Key::native(262);
    /// Android's `KEYCODE_NAVIGATE_OUT`.
    pub const NavigateOut: Key = Key::native(263);
    /// Android's `KEYCODE_SYSTEM_NAVIGATION_UP`.
    pub const SystemNavigationUp: Key = Key::native(280);
    /// Android's `KEYCODE_SYSTEM_NAVIGATION_DOWN`.
    pub const SystemNavigationDown: Key = Key::native(281);
    /// Android's `KEYCODE_SYSTEM_NAVIGATION_LEFT`.
    pub const SystemNavigationLeft: Key = Key::native(282);
    /// Android's `KEYCODE_SYSTEM_NAVIGATION_RIGHT`.
    pub const SystemNavigationRight: Key = Key::native(283);
    /// Android's `KEYCODE_CALL`.
    pub const Call: Key = Key::native(5);
    /// Android's `KEYCODE_ENDCALL`.
    pub const EndCall: Key = Key::native(6);
    /// Android's `KEYCODE_DPAD_UP`.
    pub const DirectionUp: Key = Key::native(19);
    /// Android's `KEYCODE_DPAD_DOWN`.
    pub const DirectionDown: Key = Key::native(20);
    /// Android's `KEYCODE_DPAD_LEFT`.
    pub const DirectionLeft: Key = Key::native(21);
    /// Android's `KEYCODE_DPAD_RIGHT`.
    pub const DirectionRight: Key = Key::native(22);
    /// Android's `KEYCODE_DPAD_CENTER`.
    pub const DirectionCenter: Key = Key::native(23);
    /// Android's `KEYCODE_DPAD_UP_LEFT`.
    pub const DirectionUpLeft: Key = Key::native(268);
    /// Android's `KEYCODE_DPAD_DOWN_LEFT`.
    pub const DirectionDownLeft: Key = Key::native(269);
    /// Android's `KEYCODE_DPAD_UP_RIGHT`.
    pub const DirectionUpRight: Key = Key::native(270);
    /// Android's `KEYCODE_DPAD_DOWN_RIGHT`.
    pub const DirectionDownRight: Key = Key::native(271);
    /// Android's `KEYCODE_VOLUME_UP`.
    pub const VolumeUp: Key = Key::native(24);
    /// Android's `KEYCODE_VOLUME_DOWN`.
    pub const VolumeDown: Key = Key::native(25);
    /// Android's `KEYCODE_POWER`.
    pub const Power: Key = Key::native(26);
    /// Android's `KEYCODE_CAMERA`.
    pub const Camera: Key = Key::native(27);
    /// Android's `KEYCODE_CLEAR`.
    pub const Clear: Key = Key::native(28);
    /// Android's `KEYCODE_0`.
    pub const Zero: Key = Key::native(7);
    /// Android's `KEYCODE_1`.
    pub const One: Key = Key::native(8);
    /// Android's `KEYCODE_2`.
    pub const Two: Key = Key::native(9);
    /// Android's `KEYCODE_3`.
    pub const Three: Key = Key::native(10);
    /// Android's `KEYCODE_4`.
    pub const Four: Key = Key::native(11);
    /// Android's `KEYCODE_5`.
    pub const Five: Key = Key::native(12);
    /// Android's `KEYCODE_6`.
    pub const Six: Key = Key::native(13);
    /// Android's `KEYCODE_7`.
    pub const Seven: Key = Key::native(14);
    /// Android's `KEYCODE_8`.
    pub const Eight: Key = Key::native(15);
    /// Android's `KEYCODE_9`.
    pub const Nine: Key = Key::native(16);
    /// Android's `KEYCODE_PLUS`.
    pub const Plus: Key = Key::native(81);
    /// Android's `KEYCODE_MINUS`.
    pub const Minus: Key = Key::native(69);
    /// Android's `KEYCODE_STAR`.
    pub const Multiply: Key = Key::native(17);
    /// Android's `KEYCODE_EQUALS`.
    pub const Equals: Key = Key::native(70);
    /// Android's `KEYCODE_POUND`.
    pub const Pound: Key = Key::native(18);
    /// Android's `KEYCODE_A`.
    pub const A: Key = Key::native(29);
    /// Android's `KEYCODE_B`.
    pub const B: Key = Key::native(30);
    /// Android's `KEYCODE_C`.
    pub const C: Key = Key::native(31);
    /// Android's `KEYCODE_D`.
    pub const D: Key = Key::native(32);
    /// Android's `KEYCODE_E`.
    pub const E: Key = Key::native(33);
    /// Android's `KEYCODE_F`.
    pub const F: Key = Key::native(34);
    /// Android's `KEYCODE_G`.
    pub const G: Key = Key::native(35);
    /// Android's `KEYCODE_H`.
    pub const H: Key = Key::native(36);
    /// Android's `KEYCODE_I`.
    pub const I: Key = Key::native(37);
    /// Android's `KEYCODE_J`.
    pub const J: Key = Key::native(38);
    /// Android's `KEYCODE_K`.
    pub const K: Key = Key::native(39);
    /// Android's `KEYCODE_L`.
    pub const L: Key = Key::native(40);
    /// Android's `KEYCODE_M`.
    pub const M: Key = Key::native(41);
    /// Android's `KEYCODE_N`.
    pub const N: Key = Key::native(42);
    /// Android's `KEYCODE_O`.
    pub const O: Key = Key::native(43);
    /// Android's `KEYCODE_P`.
    pub const P: Key = Key::native(44);
    /// Android's `KEYCODE_Q`.
    pub const Q: Key = Key::native(45);
    /// Android's `KEYCODE_R`.
    pub const R: Key = Key::native(46);
    /// Android's `KEYCODE_S`.
    pub const S: Key = Key::native(47);
    /// Android's `KEYCODE_T`.
    pub const T: Key = Key::native(48);
    /// Android's `KEYCODE_U`.
    pub const U: Key = Key::native(49);
    /// Android's `KEYCODE_V`.
    pub const V: Key = Key::native(50);
    /// Android's `KEYCODE_W`.
    pub const W: Key = Key::native(51);
    /// Android's `KEYCODE_X`.
    pub const X: Key = Key::native(52);
    /// Android's `KEYCODE_Y`.
    pub const Y: Key = Key::native(53);
    /// Android's `KEYCODE_Z`.
    pub const Z: Key = Key::native(54);
    /// Android's `KEYCODE_COMMA`.
    pub const Comma: Key = Key::native(55);
    /// Android's `KEYCODE_PERIOD`.
    pub const Period: Key = Key::native(56);
    /// Android's `KEYCODE_ALT_LEFT`.
    pub const AltLeft: Key = Key::native(57);
    /// Android's `KEYCODE_ALT_RIGHT`.
    pub const AltRight: Key = Key::native(58);
    /// Android's `KEYCODE_SHIFT_LEFT`.
    pub const ShiftLeft: Key = Key::native(59);
    /// Android's `KEYCODE_SHIFT_RIGHT`.
    pub const ShiftRight: Key = Key::native(60);
    /// Android's `KEYCODE_TAB`.
    pub const Tab: Key = Key::native(61);
    /// Android's `KEYCODE_SPACE`.
    pub const Spacebar: Key = Key::native(62);
    /// Android's `KEYCODE_SYM`.
    pub const Symbol: Key = Key::native(63);
    /// Android's `KEYCODE_EXPLORER`.
    pub const Browser: Key = Key::native(64);
    /// Android's `KEYCODE_ENVELOPE`.
    pub const Envelope: Key = Key::native(65);
    /// Android's `KEYCODE_ENTER`.
    pub const Enter: Key = Key::native(66);
    /// Android's `KEYCODE_DEL`.
    pub const Backspace: Key = Key::native(67);
    /// Android's `KEYCODE_FORWARD_DEL`.
    pub const Delete: Key = Key::native(112);
    /// Android's `KEYCODE_ESCAPE`.
    pub const Escape: Key = Key::native(111);
    /// Android's `KEYCODE_CTRL_LEFT`.
    pub const CtrlLeft: Key = Key::native(113);
    /// Android's `KEYCODE_CTRL_RIGHT`.
    pub const CtrlRight: Key = Key::native(114);
    /// Android's `KEYCODE_CAPS_LOCK`.
    pub const CapsLock: Key = Key::native(115);
    /// Android's `KEYCODE_SCROLL_LOCK`.
    pub const ScrollLock: Key = Key::native(116);
    /// Android's `KEYCODE_META_LEFT`.
    pub const MetaLeft: Key = Key::native(117);
    /// Android's `KEYCODE_META_RIGHT`.
    pub const MetaRight: Key = Key::native(118);
    /// Android's `KEYCODE_FUNCTION`.
    pub const Function: Key = Key::native(119);
    /// Android's `KEYCODE_SYSRQ`.
    pub const PrintScreen: Key = Key::native(120);
    /// Android's `KEYCODE_BREAK`.
    pub const Break: Key = Key::native(121);
    /// Android's `KEYCODE_MOVE_HOME`.
    pub const MoveHome: Key = Key::native(122);
    /// Android's `KEYCODE_MOVE_END`.
    pub const MoveEnd: Key = Key::native(123);
    /// Android's `KEYCODE_INSERT`.
    pub const Insert: Key = Key::native(124);
    /// Android's `KEYCODE_CUT`.
    pub const Cut: Key = Key::native(277);
    /// Android's `KEYCODE_COPY`.
    pub const Copy: Key = Key::native(278);
    /// Android's `KEYCODE_PASTE`.
    pub const Paste: Key = Key::native(279);
    /// Android's `KEYCODE_GRAVE`.
    pub const Grave: Key = Key::native(68);
    /// Android's `KEYCODE_LEFT_BRACKET`.
    pub const LeftBracket: Key = Key::native(71);
    /// Android's `KEYCODE_RIGHT_BRACKET`.
    pub const RightBracket: Key = Key::native(72);
    /// Android's `KEYCODE_SLASH`.
    pub const Slash: Key = Key::native(76);
    /// Android's `KEYCODE_BACKSLASH`.
    pub const Backslash: Key = Key::native(73);
    /// Android's `KEYCODE_SEMICOLON`.
    pub const Semicolon: Key = Key::native(74);
    /// Android's `KEYCODE_APOSTROPHE`.
    pub const Apostrophe: Key = Key::native(75);
    /// Android's `KEYCODE_AT`.
    pub const At: Key = Key::native(77);
    /// Android's `KEYCODE_NUM`.
    pub const Number: Key = Key::native(78);
    /// Android's `KEYCODE_HEADSETHOOK`.
    pub const HeadsetHook: Key = Key::native(79);
    /// Android's `KEYCODE_FOCUS`.
    pub const Focus: Key = Key::native(80);
    /// Android's `KEYCODE_MENU`.
    pub const Menu: Key = Key::native(82);
    /// Android's `KEYCODE_NOTIFICATION`.
    pub const Notification: Key = Key::native(83);
    /// Android's `KEYCODE_SEARCH`.
    pub const Search: Key = Key::native(84);
    /// Android's `KEYCODE_PAGE_UP`.
    pub const PageUp: Key = Key::native(92);
    /// Android's `KEYCODE_PAGE_DOWN`.
    pub const PageDown: Key = Key::native(93);
    /// Android's `KEYCODE_PICTSYMBOLS`.
    pub const PictureSymbols: Key = Key::native(94);
    /// Android's `KEYCODE_SWITCH_CHARSET`.
    pub const SwitchCharset: Key = Key::native(95);
    /// Android's `KEYCODE_BUTTON_A`.
    pub const ButtonA: Key = Key::native(96);
    /// Android's `KEYCODE_BUTTON_B`.
    pub const ButtonB: Key = Key::native(97);
    /// Android's `KEYCODE_BUTTON_C`.
    pub const ButtonC: Key = Key::native(98);
    /// Android's `KEYCODE_BUTTON_X`.
    pub const ButtonX: Key = Key::native(99);
    /// Android's `KEYCODE_BUTTON_Y`.
    pub const ButtonY: Key = Key::native(100);
    /// Android's `KEYCODE_BUTTON_Z`.
    pub const ButtonZ: Key = Key::native(101);
    /// Android's `KEYCODE_BUTTON_L1`.
    pub const ButtonL1: Key = Key::native(102);
    /// Android's `KEYCODE_BUTTON_R1`.
    pub const ButtonR1: Key = Key::native(103);
    /// Android's `KEYCODE_BUTTON_L2`.
    pub const ButtonL2: Key = Key::native(104);
    /// Android's `KEYCODE_BUTTON_R2`.
    pub const ButtonR2: Key = Key::native(105);
    /// Android's `KEYCODE_BUTTON_THUMBL`.
    pub const ButtonThumbLeft: Key = Key::native(106);
    /// Android's `KEYCODE_BUTTON_THUMBR`.
    pub const ButtonThumbRight: Key = Key::native(107);
    /// Android's `KEYCODE_BUTTON_START`.
    pub const ButtonStart: Key = Key::native(108);
    /// Android's `KEYCODE_BUTTON_SELECT`.
    pub const ButtonSelect: Key = Key::native(109);
    /// Android's `KEYCODE_BUTTON_MODE`.
    pub const ButtonMode: Key = Key::native(110);
    /// Android's `KEYCODE_BUTTON_1`.
    pub const Button1: Key = Key::native(188);
    /// Android's `KEYCODE_BUTTON_2`.
    pub const Button2: Key = Key::native(189);
    /// Android's `KEYCODE_BUTTON_3`.
    pub const Button3: Key = Key::native(190);
    /// Android's `KEYCODE_BUTTON_4`.
    pub const Button4: Key = Key::native(191);
    /// Android's `KEYCODE_BUTTON_5`.
    pub const Button5: Key = Key::native(192);
    /// Android's `KEYCODE_BUTTON_6`.
    pub const Button6: Key = Key::native(193);
    /// Android's `KEYCODE_BUTTON_7`.
    pub const Button7: Key = Key::native(194);
    /// Android's `KEYCODE_BUTTON_8`.
    pub const Button8: Key = Key::native(195);
    /// Android's `KEYCODE_BUTTON_9`.
    pub const Button9: Key = Key::native(196);
    /// Android's `KEYCODE_BUTTON_10`.
    pub const Button10: Key = Key::native(197);
    /// Android's `KEYCODE_BUTTON_11`.
    pub const Button11: Key = Key::native(198);
    /// Android's `KEYCODE_BUTTON_12`.
    pub const Button12: Key = Key::native(199);
    /// Android's `KEYCODE_BUTTON_13`.
    pub const Button13: Key = Key::native(200);
    /// Android's `KEYCODE_BUTTON_14`.
    pub const Button14: Key = Key::native(201);
    /// Android's `KEYCODE_BUTTON_15`.
    pub const Button15: Key = Key::native(202);
    /// Android's `KEYCODE_BUTTON_16`.
    pub const Button16: Key = Key::native(203);
    /// Android's `KEYCODE_FORWARD`.
    pub const Forward: Key = Key::native(125);
    /// Android's `KEYCODE_F1`.
    pub const F1: Key = Key::native(131);
    /// Android's `KEYCODE_F2`.
    pub const F2: Key = Key::native(132);
    /// Android's `KEYCODE_F3`.
    pub const F3: Key = Key::native(133);
    /// Android's `KEYCODE_F4`.
    pub const F4: Key = Key::native(134);
    /// Android's `KEYCODE_F5`.
    pub const F5: Key = Key::native(135);
    /// Android's `KEYCODE_F6`.
    pub const F6: Key = Key::native(136);
    /// Android's `KEYCODE_F7`.
    pub const F7: Key = Key::native(137);
    /// Android's `KEYCODE_F8`.
    pub const F8: Key = Key::native(138);
    /// Android's `KEYCODE_F9`.
    pub const F9: Key = Key::native(139);
    /// Android's `KEYCODE_F10`.
    pub const F10: Key = Key::native(140);
    /// Android's `KEYCODE_F11`.
    pub const F11: Key = Key::native(141);
    /// Android's `KEYCODE_F12`.
    pub const F12: Key = Key::native(142);
    // F13 to F24: Android has no key codes for them (Compose desktop takes AWT's); KompozR's own, past Android's.
    /// F13 (KompozR's: Android has none).
    pub const F13: Key = Key::native(1013);
    /// F14 (KompozR's: Android has none).
    pub const F14: Key = Key::native(1014);
    /// F15 (KompozR's: Android has none).
    pub const F15: Key = Key::native(1015);
    /// F16 (KompozR's: Android has none).
    pub const F16: Key = Key::native(1016);
    /// F17 (KompozR's: Android has none).
    pub const F17: Key = Key::native(1017);
    /// F18 (KompozR's: Android has none).
    pub const F18: Key = Key::native(1018);
    /// F19 (KompozR's: Android has none).
    pub const F19: Key = Key::native(1019);
    /// F20 (KompozR's: Android has none).
    pub const F20: Key = Key::native(1020);
    /// F21 (KompozR's: Android has none).
    pub const F21: Key = Key::native(1021);
    /// F22 (KompozR's: Android has none).
    pub const F22: Key = Key::native(1022);
    /// F23 (KompozR's: Android has none).
    pub const F23: Key = Key::native(1023);
    /// F24 (KompozR's: Android has none).
    pub const F24: Key = Key::native(1024);
    /// Android's `KEYCODE_NUM_LOCK`.
    pub const NumLock: Key = Key::native(143);
    /// Android's `KEYCODE_NUMPAD_0`.
    pub const NumPad0: Key = Key::native(144);
    /// Android's `KEYCODE_NUMPAD_1`.
    pub const NumPad1: Key = Key::native(145);
    /// Android's `KEYCODE_NUMPAD_2`.
    pub const NumPad2: Key = Key::native(146);
    /// Android's `KEYCODE_NUMPAD_3`.
    pub const NumPad3: Key = Key::native(147);
    /// Android's `KEYCODE_NUMPAD_4`.
    pub const NumPad4: Key = Key::native(148);
    /// Android's `KEYCODE_NUMPAD_5`.
    pub const NumPad5: Key = Key::native(149);
    /// Android's `KEYCODE_NUMPAD_6`.
    pub const NumPad6: Key = Key::native(150);
    /// Android's `KEYCODE_NUMPAD_7`.
    pub const NumPad7: Key = Key::native(151);
    /// Android's `KEYCODE_NUMPAD_8`.
    pub const NumPad8: Key = Key::native(152);
    /// Android's `KEYCODE_NUMPAD_9`.
    pub const NumPad9: Key = Key::native(153);
    /// Android's `KEYCODE_NUMPAD_DIVIDE`.
    pub const NumPadDivide: Key = Key::native(154);
    /// Android's `KEYCODE_NUMPAD_MULTIPLY`.
    pub const NumPadMultiply: Key = Key::native(155);
    /// Android's `KEYCODE_NUMPAD_SUBTRACT`.
    pub const NumPadSubtract: Key = Key::native(156);
    /// Android's `KEYCODE_NUMPAD_ADD`.
    pub const NumPadAdd: Key = Key::native(157);
    /// Android's `KEYCODE_NUMPAD_DOT`.
    pub const NumPadDot: Key = Key::native(158);
    /// Android's `KEYCODE_NUMPAD_COMMA`.
    pub const NumPadComma: Key = Key::native(159);
    /// Android's `KEYCODE_NUMPAD_ENTER`.
    pub const NumPadEnter: Key = Key::native(160);
    /// Android's `KEYCODE_NUMPAD_EQUALS`.
    pub const NumPadEquals: Key = Key::native(161);
    /// Android's `KEYCODE_NUMPAD_LEFT_PAREN`.
    pub const NumPadLeftParenthesis: Key = Key::native(162);
    /// Android's `KEYCODE_NUMPAD_RIGHT_PAREN`.
    pub const NumPadRightParenthesis: Key = Key::native(163);
    /// Android's `KEYCODE_MEDIA_PLAY`.
    pub const MediaPlay: Key = Key::native(126);
    /// Android's `KEYCODE_MEDIA_PAUSE`.
    pub const MediaPause: Key = Key::native(127);
    /// Android's `KEYCODE_MEDIA_PLAY_PAUSE`.
    pub const MediaPlayPause: Key = Key::native(85);
    /// Android's `KEYCODE_MEDIA_STOP`.
    pub const MediaStop: Key = Key::native(86);
    /// Android's `KEYCODE_MEDIA_RECORD`.
    pub const MediaRecord: Key = Key::native(130);
    /// Android's `KEYCODE_MEDIA_NEXT`.
    pub const MediaNext: Key = Key::native(87);
    /// Android's `KEYCODE_MEDIA_PREVIOUS`.
    pub const MediaPrevious: Key = Key::native(88);
    /// Android's `KEYCODE_MEDIA_REWIND`.
    pub const MediaRewind: Key = Key::native(89);
    /// Android's `KEYCODE_MEDIA_FAST_FORWARD`.
    pub const MediaFastForward: Key = Key::native(90);
    /// Android's `KEYCODE_MEDIA_CLOSE`.
    pub const MediaClose: Key = Key::native(128);
    /// Android's `KEYCODE_MEDIA_AUDIO_TRACK`.
    pub const MediaAudioTrack: Key = Key::native(222);
    /// Android's `KEYCODE_MEDIA_EJECT`.
    pub const MediaEject: Key = Key::native(129);
    /// Android's `KEYCODE_MEDIA_TOP_MENU`.
    pub const MediaTopMenu: Key = Key::native(226);
    /// Android's `KEYCODE_MEDIA_SKIP_FORWARD`.
    pub const MediaSkipForward: Key = Key::native(272);
    /// Android's `KEYCODE_MEDIA_SKIP_BACKWARD`.
    pub const MediaSkipBackward: Key = Key::native(273);
    /// Android's `KEYCODE_MEDIA_STEP_FORWARD`.
    pub const MediaStepForward: Key = Key::native(274);
    /// Android's `KEYCODE_MEDIA_STEP_BACKWARD`.
    pub const MediaStepBackward: Key = Key::native(275);
    /// Android's `KEYCODE_MUTE`.
    pub const MicrophoneMute: Key = Key::native(91);
    /// Android's `KEYCODE_VOLUME_MUTE`.
    pub const VolumeMute: Key = Key::native(164);
    /// Android's `KEYCODE_INFO`.
    pub const Info: Key = Key::native(165);
    /// Android's `KEYCODE_CHANNEL_UP`.
    pub const ChannelUp: Key = Key::native(166);
    /// Android's `KEYCODE_CHANNEL_DOWN`.
    pub const ChannelDown: Key = Key::native(167);
    /// Android's `KEYCODE_ZOOM_IN`.
    pub const ZoomIn: Key = Key::native(168);
    /// Android's `KEYCODE_ZOOM_OUT`.
    pub const ZoomOut: Key = Key::native(169);
    /// Android's `KEYCODE_TV`.
    pub const Tv: Key = Key::native(170);
    /// Android's `KEYCODE_WINDOW`.
    pub const Window: Key = Key::native(171);
    /// Android's `KEYCODE_GUIDE`.
    pub const Guide: Key = Key::native(172);
    /// Android's `KEYCODE_DVR`.
    pub const Dvr: Key = Key::native(173);
    /// Android's `KEYCODE_BOOKMARK`.
    pub const Bookmark: Key = Key::native(174);
    /// Android's `KEYCODE_CAPTIONS`.
    pub const Captions: Key = Key::native(175);
    /// Android's `KEYCODE_SETTINGS`.
    pub const Settings: Key = Key::native(176);
    /// Android's `KEYCODE_TV_POWER`.
    pub const TvPower: Key = Key::native(177);
    /// Android's `KEYCODE_TV_INPUT`.
    pub const TvInput: Key = Key::native(178);
    /// Android's `KEYCODE_STB_POWER`.
    pub const SetTopBoxPower: Key = Key::native(179);
    /// Android's `KEYCODE_STB_INPUT`.
    pub const SetTopBoxInput: Key = Key::native(180);
    /// Android's `KEYCODE_AVR_POWER`.
    pub const AvReceiverPower: Key = Key::native(181);
    /// Android's `KEYCODE_AVR_INPUT`.
    pub const AvReceiverInput: Key = Key::native(182);
    /// Android's `KEYCODE_PROG_RED`.
    pub const ProgramRed: Key = Key::native(183);
    /// Android's `KEYCODE_PROG_GREEN`.
    pub const ProgramGreen: Key = Key::native(184);
    /// Android's `KEYCODE_PROG_YELLOW`.
    pub const ProgramYellow: Key = Key::native(185);
    /// Android's `KEYCODE_PROG_BLUE`.
    pub const ProgramBlue: Key = Key::native(186);
    /// Android's `KEYCODE_APP_SWITCH`.
    pub const AppSwitch: Key = Key::native(187);
    /// Android's `KEYCODE_LANGUAGE_SWITCH`.
    pub const LanguageSwitch: Key = Key::native(204);
    /// Android's `KEYCODE_MANNER_MODE`.
    pub const MannerMode: Key = Key::native(205);
    /// Android's `KEYCODE_3D_MODE`.
    pub const Toggle2D3D: Key = Key::native(206);
    /// Android's `KEYCODE_CONTACTS`.
    pub const Contacts: Key = Key::native(207);
    /// Android's `KEYCODE_CALENDAR`.
    pub const Calendar: Key = Key::native(208);
    /// Android's `KEYCODE_MUSIC`.
    pub const Music: Key = Key::native(209);
    /// Android's `KEYCODE_CALCULATOR`.
    pub const Calculator: Key = Key::native(210);
    /// Android's `KEYCODE_ZENKAKU_HANKAKU`.
    pub const ZenkakuHankaru: Key = Key::native(211);
    /// Android's `KEYCODE_EISU`.
    pub const Eisu: Key = Key::native(212);
    /// Android's `KEYCODE_MUHENKAN`.
    pub const Muhenkan: Key = Key::native(213);
    /// Android's `KEYCODE_HENKAN`.
    pub const Henkan: Key = Key::native(214);
    /// Android's `KEYCODE_KATAKANA_HIRAGANA`.
    pub const KatakanaHiragana: Key = Key::native(215);
    /// Android's `KEYCODE_YEN`.
    pub const Yen: Key = Key::native(216);
    /// Android's `KEYCODE_RO`.
    pub const Ro: Key = Key::native(217);
    /// Android's `KEYCODE_KANA`.
    pub const Kana: Key = Key::native(218);
    /// Android's `KEYCODE_ASSIST`.
    pub const Assist: Key = Key::native(219);
    /// Android's `KEYCODE_BRIGHTNESS_DOWN`.
    pub const BrightnessDown: Key = Key::native(220);
    /// Android's `KEYCODE_BRIGHTNESS_UP`.
    pub const BrightnessUp: Key = Key::native(221);
    /// Android's `KEYCODE_SLEEP`.
    pub const Sleep: Key = Key::native(223);
    /// Android's `KEYCODE_WAKEUP`.
    pub const WakeUp: Key = Key::native(224);
    /// Android's `KEYCODE_SOFT_SLEEP`.
    pub const SoftSleep: Key = Key::native(276);
    /// Android's `KEYCODE_PAIRING`.
    pub const Pairing: Key = Key::native(225);
    /// Android's `KEYCODE_LAST_CHANNEL`.
    pub const LastChannel: Key = Key::native(229);
    /// Android's `KEYCODE_TV_DATA_SERVICE`.
    pub const TvDataService: Key = Key::native(230);
    /// Android's `KEYCODE_VOICE_ASSIST`.
    pub const VoiceAssist: Key = Key::native(231);
    /// Android's `KEYCODE_TV_RADIO_SERVICE`.
    pub const TvRadioService: Key = Key::native(232);
    /// Android's `KEYCODE_TV_TELETEXT`.
    pub const TvTeletext: Key = Key::native(233);
    /// Android's `KEYCODE_TV_NUMBER_ENTRY`.
    pub const TvNumberEntry: Key = Key::native(234);
    /// Android's `KEYCODE_TV_TERRESTRIAL_ANALOG`.
    pub const TvTerrestrialAnalog: Key = Key::native(235);
    /// Android's `KEYCODE_TV_TERRESTRIAL_DIGITAL`.
    pub const TvTerrestrialDigital: Key = Key::native(236);
    /// Android's `KEYCODE_TV_SATELLITE`.
    pub const TvSatellite: Key = Key::native(237);
    /// Android's `KEYCODE_TV_SATELLITE_BS`.
    pub const TvSatelliteBs: Key = Key::native(238);
    /// Android's `KEYCODE_TV_SATELLITE_CS`.
    pub const TvSatelliteCs: Key = Key::native(239);
    /// Android's `KEYCODE_TV_SATELLITE_SERVICE`.
    pub const TvSatelliteService: Key = Key::native(240);
    /// Android's `KEYCODE_TV_NETWORK`.
    pub const TvNetwork: Key = Key::native(241);
    /// Android's `KEYCODE_TV_ANTENNA_CABLE`.
    pub const TvAntennaCable: Key = Key::native(242);
    /// Android's `KEYCODE_TV_INPUT_HDMI_1`.
    pub const TvInputHdmi1: Key = Key::native(243);
    /// Android's `KEYCODE_TV_INPUT_HDMI_2`.
    pub const TvInputHdmi2: Key = Key::native(244);
    /// Android's `KEYCODE_TV_INPUT_HDMI_3`.
    pub const TvInputHdmi3: Key = Key::native(245);
    /// Android's `KEYCODE_TV_INPUT_HDMI_4`.
    pub const TvInputHdmi4: Key = Key::native(246);
    /// Android's `KEYCODE_TV_INPUT_COMPOSITE_1`.
    pub const TvInputComposite1: Key = Key::native(247);
    /// Android's `KEYCODE_TV_INPUT_COMPOSITE_2`.
    pub const TvInputComposite2: Key = Key::native(248);
    /// Android's `KEYCODE_TV_INPUT_COMPONENT_1`.
    pub const TvInputComponent1: Key = Key::native(249);
    /// Android's `KEYCODE_TV_INPUT_COMPONENT_2`.
    pub const TvInputComponent2: Key = Key::native(250);
    /// Android's `KEYCODE_TV_INPUT_VGA_1`.
    pub const TvInputVga1: Key = Key::native(251);
    /// Android's `KEYCODE_TV_AUDIO_DESCRIPTION`.
    pub const TvAudioDescription: Key = Key::native(252);
    /// Android's `KEYCODE_TV_AUDIO_DESCRIPTION_MIX_UP`.
    pub const TvAudioDescriptionMixingVolumeUp: Key = Key::native(253);
    /// Android's `KEYCODE_TV_AUDIO_DESCRIPTION_MIX_DOWN`.
    pub const TvAudioDescriptionMixingVolumeDown: Key = Key::native(254);
    /// Android's `KEYCODE_TV_ZOOM_MODE`.
    pub const TvZoomMode: Key = Key::native(255);
    /// Android's `KEYCODE_TV_CONTENTS_MENU`.
    pub const TvContentsMenu: Key = Key::native(256);
    /// Android's `KEYCODE_TV_MEDIA_CONTEXT_MENU`.
    pub const TvMediaContextMenu: Key = Key::native(257);
    /// Android's `KEYCODE_TV_TIMER_PROGRAMMING`.
    pub const TvTimerProgramming: Key = Key::native(258);
    /// Android's `KEYCODE_STEM_PRIMARY`.
    pub const StemPrimary: Key = Key::native(264);
    /// Android's `KEYCODE_STEM_1`.
    pub const Stem1: Key = Key::native(265);
    /// Android's `KEYCODE_STEM_2`.
    pub const Stem2: Key = Key::native(266);
    /// Android's `KEYCODE_STEM_3`.
    pub const Stem3: Key = Key::native(267);
    /// Android's `KEYCODE_ALL_APPS`.
    pub const AllApps: Key = Key::native(284);
    /// Android's `KEYCODE_REFRESH`.
    pub const Refresh: Key = Key::native(285);
    /// Android's `KEYCODE_THUMBS_UP`.
    pub const ThumbsUp: Key = Key::native(286);
    /// Android's `KEYCODE_THUMBS_DOWN`.
    pub const ThumbsDown: Key = Key::native(287);
    /// Android's `KEYCODE_PROFILE_SWITCH`.
    pub const ProfileSwitch: Key = Key::native(288);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadDirectionUp: Key = Key::native(-1000000001);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadDirectionDown: Key = Key::native(-1000000002);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadDirectionLeft: Key = Key::native(-1000000003);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadDirectionRight: Key = Key::native(-1000000004);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadMoveHome: Key = Key::native(-1000000005);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadMoveEnd: Key = Key::native(-1000000006);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadPageUp: Key = Key::native(-1000000007);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadPageDown: Key = Key::native(-1000000008);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadInsert: Key = Key::native(-1000000009);
    /// Unsupported on Android (Compose's placeholder code).
    pub const NumPadDelete: Key = Key::native(-1000000010);
}

/// The modifier keys held (Compose's keyboard modifiers), as bits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct KeyModifiers(pub u32);

impl KeyModifiers {
    pub const None: KeyModifiers = KeyModifiers(0);
    pub const Shift: KeyModifiers = KeyModifiers(1);
    pub const Ctrl: KeyModifiers = KeyModifiers(1 << 1);
    pub const Alt: KeyModifiers = KeyModifiers(1 << 2);
    pub const Meta: KeyModifiers = KeyModifiers(1 << 3);
    pub fn contains(self, other: KeyModifiers) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for KeyModifiers {
    type Output = KeyModifiers;
    fn bitor(self, other: KeyModifiers) -> KeyModifiers {
        KeyModifiers(self.0 | other.0)
    }
}

/// Whether a key went down or up (Compose's KeyEventType).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum KeyEventType {
    Unknown,
    KeyUp,
    KeyDown,
}

/// A key event (Compose's KeyEvent): its key, whether it went down or up, the character it types (UTF-16 code
/// point; 0 for none) and the modifier keys held.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyEvent {
    pub key: Key,
    pub r#type: KeyEventType,
    pub utf16_code_point: i32,
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    /// An event of `key`, its character the one it types on a US keyboard (what a platform without its own
    /// characters, and tests, send).
    pub fn new(key: Key, r#type: KeyEventType, modifiers: KeyModifiers) -> KeyEvent {
        let utf16_code_point = key.typed_code_point(modifiers.contains(KeyModifiers::Shift));
        KeyEvent { key, r#type, utf16_code_point, modifiers }
    }

    /// An event with the character the platform says it types.
    pub fn with_code_point(key: Key, r#type: KeyEventType, utf16_code_point: i32, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { key, r#type, utf16_code_point, modifiers }
    }

    pub fn is_alt_pressed(&self) -> bool {
        self.modifiers.contains(KeyModifiers::Alt)
    }
    pub fn is_ctrl_pressed(&self) -> bool {
        self.modifiers.contains(KeyModifiers::Ctrl)
    }
    pub fn is_meta_pressed(&self) -> bool {
        self.modifiers.contains(KeyModifiers::Meta)
    }
    pub fn is_shift_pressed(&self) -> bool {
        self.modifiers.contains(KeyModifiers::Shift)
    }
}
