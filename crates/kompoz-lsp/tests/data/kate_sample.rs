//! A source with Kotlin in it: which lines Kate highlights as which language (tests/kate.rs).

use kompoz::prelude::*;
use std::collections::HashMap;

/* A Rust comment over lines, whose kompoz! doesn't start the Kotlin:
let ui = kompoz! { fun NotKotlin() {} }
*/
fn label(n: usize) -> String {
    let text = "kompoz! { not Kotlin either";
    format!("{n} items {text}")
}

kompoz! {
    data class Item(val id: Int, val title: String, val done: Boolean = false)

    @Composable
    fun ItemRow(item: Item, onToggle: (Boolean) -> Unit) {
        Row(modifier = Modifier.fillMaxWidth().clickable { onToggle(!item.done) }) {
            Checkbox(checked = item.done, onCheckedChange = onToggle)
            // a comment in the Kotlin, its } not counted
            Text("${item.title} #$item.id", style = MaterialTheme.typography.bodyLarge)
        }
    }
    /* a block comment { with /* a nested one */ still } a comment */
    private val Spacing = 8.dp

    object Routes {
        const val Home = "home"
    }

    enum class Mode(val label: String) { Light("light"), Dark("dark") }

    sealed class Screen(val route: String)

    fun Braces(text: String = "a } in a string", c: Char = '}') {
        val nested = listOf(mapOf(1 to "${ if (true) { "}" } else "" }"))
        val raw = """a raw } string ${nested.size}"""
    }
}

struct Box<T> {
    value: T,
}

impl<T: Clone> Box<T> {
    pub fn get(&self) -> T {
        self.value.clone()
    }
}

macro_rules! twice {
    ($e:expr) => {
        ($e, $e)
    };
}

fn main() {
    let map: HashMap<u32, &str> = HashMap::new();
    println!("{} {:?}", label(map.len()), twice!(1));
}

fn more() -> (Ui, Ui) {
    let one = kompoz!(Text("a ) in a string")); let after = Some(1);
    let rows = kompoz!
        [Text("one"), Text("two")];
    (one, rows)
}
