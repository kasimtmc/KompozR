//! Form A: named and defaulted arguments, as in Kotlin.

use kompoz_syntax_proto::a::*;
use kompoz_syntax_proto::common::*;
use kompoz_syntax_proto_macros::kompozable;

#[kompozable]
fn Counter() {
    let count = remember(|| mutable_state_of(0));

    Column(modifier = Modifier.padding(16.dp()), vertical_arrangement = Arrangement::spaced_by(8.dp()), || {
        Text(format!("Count: {count}"), style = MaterialTheme.typography().title_large);
        Button(on_click = || count.set(count.get() + 1), || {
            Text("Increment");
        });
    });
}

// Compose's Scaffold sample, as the user wrote it in Kotlin.
#[allow(non_upper_case_globals)]
struct AppIcons;
#[allow(non_upper_case_globals)]
impl AppIcons {
    const Add: ImageVector = Icons::Default::Add;
}

#[kompozable]
fn ScaffoldExample() {
    let presses = remember(|| mutable_int_state_of(0));

    Scaffold(
        top_bar = || {
            TopAppBar(
                colors = top_app_bar_colors(
                    container_color = MaterialTheme.color_scheme().primary_container,
                    title_content_color = MaterialTheme.color_scheme().primary,
                ),
                title = || Text("Top app bar"),
            );
        },
        bottom_bar = || {
            BottomAppBar(
                container_color = MaterialTheme.color_scheme().primary_container,
                content_color = MaterialTheme.color_scheme().primary,
                || {
                    Text(modifier = Modifier.fill_max_width(), text_align = TextAlign::Center, text = "Bottom app bar");
                },
            );
        },
        floating_action_button = || {
            FloatingActionButton(on_click = || presses.update(|p| p + 1), || {
                Icon(painter = remember_vector_painter(AppIcons::Add), content_description = "Add");
            });
        },
        |inner_padding| {
            Column(
                modifier = Modifier.padding(inner_padding),
                vertical_arrangement = Arrangement::spaced_by(16.dp()),
                || {
                    Text(
                        modifier = Modifier.padding(8.dp()),
                        text = format!(
                            "This is an example of a scaffold. It uses the Scaffold composable's parameters to create a \
                         screen with a simple top app bar, bottom app bar, and floating action button.\n\n\
                         It also contains some basic inner content, such as this text.\n\n\
                         You have pressed the floating action button {presses} times."
                        ),
                    );
                },
            );
        },
    );
}

// The card of the user's AddTextDialog: named arguments of methods, and a factory function.
#[kompozable]
fn GradientCard(colors: ColorScheme) {
    Box(
        modifier = Modifier
            .background(
                brush = Brush::linear_gradient(vec![
                    colors.primary_container.copy(alpha = 0.6),
                    colors.tertiary_container.copy(alpha = 0.6),
                ]),
                shape = RoundedCornerShape(28.dp()),
            )
            .fill_max_width()
            .fill_max_height(fraction = 0.3)
            .padding(4.dp()),
        || Text("…"),
    );
}

#[kompozable]
fn App() {
    MaterialTheme(|| {
        Counter();
        ScaffoldExample();
        GradientCard(MaterialTheme.color_scheme());
    });
}

fn main() {
    kompoz_syntax_proto::compose!(App);
    click("Button");
    click("FloatingActionButton");
    kompoz_syntax_proto::compose!(App);
}
