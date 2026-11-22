//! The title bar KompozR draws at the top of a window the platform leaves undecorated ([`Metrics::title_bar`]; C++'s
//! title_bar.cpp): the app's title in the middle, the window's buttons on the sides the desktop puts them, dragged to
//! move the window, double-clicked to maximize it, the window manager's menu on the secondary button. In the
//! system's colors (its dark theme, its accent: Material 3's roles as Compose's dynamic color schemes work them out
//! of the system's palette), not the app's: it stands in for the system's. Foundation's pieces only (material is a
//! later stage); its icons are Material icons' paths (Apache-2.0, as Compose's material-icons).
//!
//! [`Metrics::title_bar`]: super::Metrics::title_bar

use super::{TitleBar, WindowAction, WindowButton};
use kompoz_foundation::prelude::*;
use kompoz_graphics::material_color::palettes::{DynamicTonalPalette, palette_tone};
use kompoz_graphics::{
    BlendMode, CircleShape, Color, ColorFilter, Dp, ImageVector, PathFillType, SolidColor, StrokeCap, StrokeJoin,
    UnitExt, VectorPath, add_path_nodes,
};
use kompoz_runtime::{Content, State, key, kompozable, remember_updated_state};
use kompoz_text::style::line_height_style::{Alignment as LineAlignment, Mode, Trim};
use kompoz_text::{FontWeight, LineHeightStyle, TextOverflow, TextStyle};
use kompoz_ui::input::{AwaitPointerEventScope, PointerButton, PointerEvent, PointerEventType, PointerInputScope};
use kompoz_ui::layout::layout_into;
use kompoz_ui::platform::{LocalDynamicTonalPalette, LocalWindowInfo, is_system_in_dark_theme};
use kompoz_ui::semantics::Role;
use kompoz_ui::strings::LocalStrings;
use kompoz_ui::{Alignment, Constraints, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Modifier, Placeable};
use std::rc::Rc;

/// What the title bar asks of the window: an action, where (px in the window; the menu's place, a move's start).
pub type WindowActions = Rc<dyn Fn(WindowAction, kompoz_graphics::Offset)>;

/// The bar's height, its buttons' size and the gaps around them (dp, C++'s).
pub const TITLE_BAR_HEIGHT: Dp = Dp(40.0);
pub const TITLE_BAR_BUTTON: Dp = Dp(28.0);
pub const TITLE_BAR_GAP: Dp = Dp(8.0);

/// The bar's `layout_id` (C++'s): the window's frame puts it above the content.
pub const TITLE_BAR_ID: &str = "titleBar";

/// The few roles of Material 3's color scheme the bar is drawn in.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TitleBarColors {
    /// The bar.
    pub surface_container: Color,
    /// Its buttons.
    pub surface_container_highest: Color,
    /// Their icons, and the title while the window is focused.
    pub on_surface: Color,
    /// The title while it isn't.
    pub on_surface_variant: Color,
}

impl TitleBarColors {
    /// The bar's colors in the system's theme: from the system's palette (the accent's, the wallpaper's) with the
    /// tones Compose's `dynamicLightColorScheme` and `dynamicDarkColorScheme` give these roles (C++'s
    /// dynamic_color.cpp), else Material 3's baseline `lightColorScheme` and `darkColorScheme` ones.
    pub fn of(dark: bool, palette: Option<&DynamicTonalPalette>) -> TitleBarColors {
        match (palette, dark) {
            (Some(p), false) => TitleBarColors {
                surface_container: palette_tone(&p.neutral1, 94.0),
                surface_container_highest: palette_tone(&p.neutral1, 90.0),
                on_surface: palette_tone(&p.neutral1, 10.0),
                on_surface_variant: palette_tone(&p.neutral2, 30.0),
            },
            (Some(p), true) => TitleBarColors {
                surface_container: palette_tone(&p.neutral1, 12.0),
                surface_container_highest: palette_tone(&p.neutral1, 22.0),
                on_surface: palette_tone(&p.neutral1, 90.0),
                on_surface_variant: palette_tone(&p.neutral2, 80.0),
            },
            (None, false) => TitleBarColors {
                surface_container: Color::from_argb(0xFFF3EDF7),
                surface_container_highest: Color::from_argb(0xFFE6E0E9),
                on_surface: Color::from_argb(0xFF1D1B20),
                on_surface_variant: Color::from_argb(0xFF49454F),
            },
            (None, true) => TitleBarColors {
                surface_container: Color::from_argb(0xFF211F26),
                surface_container_highest: Color::from_argb(0xFF36343B),
                on_surface: Color::from_argb(0xFFE6E0E9),
                on_surface_variant: Color::from_argb(0xFFCAC4D0),
            },
        }
    }
}

/// A Material icon as Compose's `materialIcon` and `materialPath` make one: 24 by 24, one path filled black.
fn material_icon(name: &str, path_data: &str) -> ImageVector {
    ImageVector::Builder(name, Dp(24.0), Dp(24.0), 24.0, 24.0, Color::Unspecified, BlendMode::SrcIn, false)
        .add_path_of(VectorPath {
            path_data: add_path_nodes(path_data),
            path_fill_type: PathFillType::NonZero,
            fill: Some(SolidColor(Color::Black)),
            stroke_line_width: 1.0,
            stroke_line_cap: StrokeCap::Butt,
            stroke_line_join: StrokeJoin::Bevel,
            stroke_line_miter: 1.0,
            ..VectorPath::default()
        })
        .build()
}

thread_local! {
    // Material icons' Filled.Close, Filled.Minimize, Filled.CropSquare (maximize) and Filled.FilterNone (restore).
    static CLOSE: ImageVector = material_icon(
        "Filled.Close",
        "M19,6.41L17.59,5 12,10.59 6.41,5 5,6.41 10.59,12 5,17.59 6.41,19 12,13.41 17.59,19 19,17.59 13.41,12z",
    );
    static MINIMIZE: ImageVector = material_icon("Filled.Minimize", "M6,19h12v2H6z");
    static MAXIMIZE: ImageVector = material_icon(
        "Filled.CropSquare",
        "M18,4H6C4.9,4 4,4.9 4,6v12c0,1.1 0.9,2 2,2h12c1.1,0 2,-0.9 2,-2V6C20,4.9 19.1,4 18,4zM18,18H6V6h12V18z",
    );
    static RESTORE: ImageVector = material_icon(
        "Filled.FilterNone",
        "M3,5H1v16c0,1.1 0.9,2 2,2h16v-2H3V5zM21,1H7C5.9,1 5,1.9 5,3v14c0,1.1 0.9,2 2,2h14c1.1,0 2,-0.9 2,-2V3C23,1.9 \
         22.1,1 21,1zM21,17H7V3h14V17z",
    );
}

/// A button's icon: Close's, Minimize's, Maximize's or (maximized) Restore's.
pub fn button_icon(button: WindowButton, maximized: bool) -> ImageVector {
    let icon = match button {
        WindowButton::Close => &CLOSE,
        WindowButton::Minimize => &MINIMIZE,
        WindowButton::Maximize if maximized => &RESTORE,
        WindowButton::Maximize => &MAXIMIZE,
    };
    icon.with(Clone::clone)
}

/// What a button asks of the window.
pub fn button_action(button: WindowButton) -> WindowAction {
    match button {
        WindowButton::Close => WindowAction::Close,
        WindowButton::Minimize => WindowAction::Minimize,
        WindowButton::Maximize => WindowAction::ToggleMaximize,
    }
}

/// Material 3's `titleSmall` (C++'s typography: 14 sp on 20, 0.1 sp apart, medium, lines centered untrimmed).
fn title_small(color: Color) -> TextStyle {
    TextStyle {
        color,
        font_size: 14.sp(),
        line_height: 20.sp(),
        letter_spacing: 0.1.sp(),
        font_weight: Some(FontWeight::Medium),
        line_height_style: Some(LineHeightStyle::new(LineAlignment::Center, Trim::None, Mode::Fixed)),
        ..TextStyle::Default
    }
}

/// The bar's gestures: a drag moves the window (from where it passed the slop), a double click (the primary
/// button's, or a finger's) maximizes or restores it, the secondary button's press opens the window manager's menu
/// there. `act` is read as it is when each comes.
fn bar_gestures(modifier: Modifier, act: State<WindowActions>) -> Modifier {
    let (drag, tap, menu) = (act.clone(), act.clone(), act);
    modifier
        .pointer_input(0, move |s: PointerInputScope| {
            let act = drag.clone();
            async move {
                s.detect_drag_gestures(|_, _| {}).on_drag_start(move |at| (act.get())(WindowAction::Move, at)).await;
            }
        })
        .pointer_input(1, move |s: PointerInputScope| {
            let act = tap.clone();
            async move {
                s.detect_tap_gestures()
                    .on_double_tap(move |_| (act.get())(WindowAction::ToggleMaximize, Default::default()))
                    .await;
            }
        })
        .on_pointer_event(PointerEventType::Press, move |_: &AwaitPointerEventScope, e: &PointerEvent| {
            if e.button == Some(PointerButton::Secondary)
                && let Some(change) = e.changes.first()
            {
                (menu.get())(WindowAction::Menu, change.position);
            }
        })
}

/// A window button: a circle with its icon, labelled for screen readers in the app's language.
#[kompozable]
fn WindowButtonView(button: WindowButton, maximized: bool, colors: TitleBarColors, act: WindowActions) {
    let strings = LocalStrings.current();
    let label = match button {
        WindowButton::Close => strings.close.clone(),
        WindowButton::Minimize => strings.minimize.clone(),
        WindowButton::Maximize if maximized => strings.restore.clone(),
        WindowButton::Maximize => strings.maximize.clone(),
    };
    let action = button_action(button);
    let icon = button_icon(button, maximized);
    Box(
        modifier = Modifier
            .size(TITLE_BAR_BUTTON)
            .clip(CircleShape)
            .background(colors.surface_container_highest)
            .clickable(role = Some(Role::Button), on_click = move || act(action, Default::default())),
        content_alignment = Alignment::Center,
        content = move || {
            VectorImage(
                icon,
                Some(label),
                modifier = Modifier.size(16.dp()),
                color_filter = Some(ColorFilter::tint(colors.on_surface, BlendMode::SrcIn)),
            );
        },
    );
}

/// The buttons of one side, in order, `side` of the bar.
#[kompozable]
fn WindowButtons(
    buttons: Vec<WindowButton>,
    maximized: bool,
    colors: TitleBarColors,
    act: WindowActions,
    side: Alignment,
) {
    Row(
        modifier = Modifier.align(side).padding_hv(horizontal = TITLE_BAR_GAP),
        horizontal_arrangement = Arrangement::spaced_by(TITLE_BAR_GAP),
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            for &b in &buttons {
                let act = act.clone();
                key(&(b as i32), move || WindowButtonView(b, maximized, colors, act));
            }
        },
    );
}

/// The bar: the title in the middle of the window (clear of the buttons on either side; dimmed while the window
/// isn't focused), the buttons on their sides.
#[kompozable]
fn Bar(bar: TitleBar, title: String, act: WindowActions) {
    let colors = TitleBarColors::of(is_system_in_dark_theme(), LocalDynamicTonalPalette.current().as_ref());
    let focused = LocalWindowInfo.current().is_window_focused();
    let most = bar.start.len().max(bar.end.len());
    let side = Dp(TITLE_BAR_GAP.0 + most as f32 * (TITLE_BAR_BUTTON.0 + TITLE_BAR_GAP.0));
    let gestures = bar_gestures(
        Modifier.layout_id(TITLE_BAR_ID).fill_max_width().height(TITLE_BAR_HEIGHT).background(colors.surface_container),
        remember_updated_state!(act.clone()),
    );
    let color = if focused { colors.on_surface } else { colors.on_surface_variant };
    Box(
        modifier = gestures,
        content = move || {
            BasicText(
                title.clone(),
                modifier = Modifier.align(Alignment::Center).padding_hv(horizontal = side),
                style = title_small(color),
                overflow = TextOverflow::Ellipsis,
                max_lines = 1,
            );
            WindowButtons(bar.start.clone(), bar.maximized, colors, act.clone(), Alignment::CenterStart);
            WindowButtons(bar.end.clone(), bar.maximized, colors, act.clone(), Alignment::CenterEnd);
        },
    );
}

/// The window's content (C++'s WindowFrame): `title_bar` above `content` (none: `content` alone, the whole window,
/// as it would be without it), the content's nodes each in what the bar leaves of the window. `act` does what the
/// bar's buttons and gestures ask.
#[kompozable]
pub fn WindowFrame(title_bar: Option<TitleBar>, title: String, act: WindowActions, content: Rc<dyn Content>) {
    let policy = FRAME_POLICY.with(Rc::clone);
    layout_into(Modifier, policy, false, move || {
        // (Each in a group of its own: the content stays what it was as the bar comes and goes.)
        key(&0, || {
            if let Some(bar) = title_bar.clone() {
                Bar(bar, title.clone(), act.clone());
            }
        });
        key(&1, || content.compose());
    });
}

thread_local! {
    /// The frame's policy, one for every frame: kept, so that a frame composed again isn't measured again for it.
    static FRAME_POLICY: Rc<dyn MeasurePolicy> = Rc::new(FrameMeasurePolicy);
}

/// The bar across the top, the content's nodes each in what is left, as the window holds them.
struct FrameMeasurePolicy;

impl MeasurePolicy for FrameMeasurePolicy {
    fn measure(&self, scope: &MeasureScope, children: &[&dyn Measurable], c: Constraints) -> MeasureResult {
        let is_bar = |m: &&&dyn Measurable| m.layout_id().is_some_and(|id| id.is(&TITLE_BAR_ID));
        let bar: Option<Placeable> = children
            .iter()
            .find(is_bar)
            .map(|m| m.measure(Constraints::new(c.max_width, c.max_width, 0, c.max_height)));
        let top = bar.as_ref().map_or(0, Placeable::height);
        let rest = Constraints::new(0, c.max_width, 0, (c.max_height - top).max(0));
        let content: Vec<Placeable> = children.iter().filter(|m| !is_bar(m)).map(|m| m.measure(rest)).collect();
        scope.layout(c.max_width, c.max_height, move |p| {
            if let Some(bar) = &bar {
                p.place_relative(bar, 0, 0);
            }
            for placeable in &content {
                p.place_relative(placeable, 0, top);
            }
        })
    }
}
