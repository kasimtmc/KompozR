//! The Counter itself: a count and a button that adds to it (counter-kotlin's test draws it beside the Kotlin
//! one, which must draw the same).

use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_text::TextStyle;
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::sync::Arc;

/// The button's color, and the text's.
const ACCENT: Color = Color::from_argb(0xff3f51b5);
const INK: Color = Color::from_argb(0xff1c1b1f);

#[kompozable]
pub fn Counter() {
    let count = remember_saveable(|| mutable_state_of(0));
    Column(
        modifier = Modifier.fill_max_size().background(Color::White).padding(24.dp()),
        vertical_arrangement = Arrangement::spaced_by(16.dp()),
        content = move || {
            // (The button first, at the top left: the scripts' `click 70 70` presses it.)
            Box(
                modifier = Modifier
                    .size_wh(120.dp(), 80.dp())
                    .background(ACCENT, shape = Arc::new(RoundedCornerShape(16.dp())))
                    .clickable(role = Some(Role::Button), on_click = move || count.set(count.get() + 1)),
                content_alignment = Alignment::Center,
                content = || {
                    BasicText(
                        "+1",
                        style = TextStyle { font_size: 28.sp(), color: Color::White, ..TextStyle::Default },
                    );
                },
            );
            BasicText(
                format!("Sayaç: {}", count.get()),
                style = TextStyle { font_size: 32.sp(), color: INK, ..TextStyle::Default },
            );
        },
    );
}
