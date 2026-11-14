//! Form B: plain Rust calls, optional parameters in `…Args` structs.

use kompoz_syntax_proto::b::*;
use kompoz_syntax_proto::common::*;
use kompoz_syntax_proto_macros::kompozable;

#[kompozable(plain)]
fn Counter() {
    let count = remember(|| mutable_state_of(0));

    Column(
        ColumnArgs {
            modifier: Modifier.padding(16.dp()),
            vertical_arrangement: Arrangement::spaced_by(8.dp()),
            ..default()
        },
        || {
            Text(format!("Count: {count}"), TextArgs { style: MaterialTheme::typography().title_large, ..default() });
            Button(
                || count.set(count.get() + 1),
                default(),
                || {
                    Text("Increment", default());
                },
            );
        },
    );
}

// Compose's Scaffold sample, as the user wrote it in Kotlin.
struct AppIcons;
#[allow(non_upper_case_globals)]
impl AppIcons {
    const Add: ImageVector = Icons::Default::Add;
}

#[kompozable(plain)]
fn ScaffoldExample() {
    let presses = remember(|| mutable_int_state_of(0));

    Scaffold(
        ScaffoldArgs {
            top_bar: slot(|| {
                TopAppBar(
                    || Text("Top app bar", default()),
                    TopAppBarArgs {
                        colors: TopAppBarColors {
                            container_color: MaterialTheme::color_scheme().primary_container,
                            title_content_color: MaterialTheme::color_scheme().primary,
                        },
                        ..default()
                    },
                );
            }),
            bottom_bar: slot(|| {
                BottomAppBar(
                    BottomAppBarArgs {
                        container_color: MaterialTheme::color_scheme().primary_container,
                        content_color: MaterialTheme::color_scheme().primary,
                        ..default()
                    },
                    || {
                        Text(
                            "Bottom app bar",
                            TextArgs {
                                modifier: Modifier.fill_max_width(),
                                text_align: TextAlign::Center,
                                ..default()
                            },
                        );
                    },
                );
            }),
            floating_action_button: slot(|| {
                FloatingActionButton(
                    || presses.update(|p| p + 1),
                    default(),
                    || {
                        Icon(remember_vector_painter(AppIcons::Add), "Add", default());
                    },
                );
            }),
            ..default()
        },
        |inner_padding| {
            Column(
                ColumnArgs {
                    modifier: Modifier.padding(inner_padding),
                    vertical_arrangement: Arrangement::spaced_by(16.dp()),
                    ..default()
                },
                || {
                    Text(
                        format!(
                            "This is an example of a scaffold. It uses the Scaffold composable's parameters to create \
                             a screen with a simple top app bar, bottom app bar, and floating action button.\n\n\
                             It also contains some basic inner content, such as this text.\n\n\
                             You have pressed the floating action button {presses} times."
                        ),
                        TextArgs { modifier: Modifier.padding(8.dp()), ..default() },
                    );
                },
            );
        },
    );
}

// The card of the user's AddTextDialog.
#[kompozable(plain)]
fn GradientCard(colors: ColorScheme) {
    Box(
        BoxArgs {
            modifier: Modifier
                .background_brush(
                    Brush::linear_gradient(vec![
                        colors.primary_container.with_alpha(0.6),
                        colors.tertiary_container.with_alpha(0.6),
                    ]),
                    RoundedCornerShape(28.dp()),
                )
                .fill_max_width()
                .fill_max_height_fraction(0.3)
                .padding(4.dp()),
        },
        || Text("…", default()),
    );
}

#[kompozable(plain)]
fn App() {
    MaterialTheme(light_color_scheme(), || {
        Counter();
        ScaffoldExample();
        GradientCard(MaterialTheme::color_scheme());
    });
}

fn main() {
    compose(App);
    click("Button");
    click("FloatingActionButton");
    compose(App);
}
