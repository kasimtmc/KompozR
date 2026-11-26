// SYNTAX TEST "source.rust" "kompoz! in a Rust source: the Kotlin in its three delimiters, the Rust around it"
use kompoz::prelude::*;
// <--- keyword.other.rust

kompoz! {
// <------- meta.macro.kompoz.rust entity.name.function.macro.rust
//      ^ meta.macro.kompoz.rust punctuation.brackets.curly.rust
    @Composable
//  ^ meta.embedded.block.kotlin storage.type.annotation.kotlin punctuation.definition.annotation.kotlin
    fun Greeting(name: String) {
//  ^^^ meta.embedded.block.kotlin storage.type.function.kotlin
//      ^^^^^^^^ meta.embedded.block.kotlin entity.name.function.kotlin
        Column(modifier = Modifier.padding(16.dp)) {
//      ^^^^^^ entity.name.function.call.kotlin
//                                         ^^ constant.numeric.decimal.kotlin
            Text("Hello } $name, ${name.length}!")
//               ^^^^^^^^^ string.quoted.double.kotlin
//                        ^ punctuation.definition.template-expression.kotlin
//                                 ^^^^^^ meta.template.expression.kotlin
            if (name.isEmpty()) { Text("{") } else { Text('}'.toString()) }
//          ^^ keyword.control.kotlin
//                                     ^^^ string.quoted.double.kotlin
//                                            ^^^^ keyword.control.kotlin
//                                                        ^^^ string.quoted.single.kotlin
        }
//      ^ punctuation.section.block.end.kotlin
    }
//  ^ meta.embedded.block.kotlin punctuation.section.block.end.kotlin
    // a closing brace in a comment: }
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ meta.embedded.block.kotlin comment.line.double-slash.kotlin
    val Accent = Color(0xff3f51b5)
//  ^^^ meta.embedded.block.kotlin storage.type.variable.kotlin
}
// <- meta.macro.kompoz.rust punctuation.brackets.curly.rust
// <- - meta.embedded.block.kotlin

fn after_braces() -> u32 {
// <-- keyword.other.fn.rust
// <-- - meta.macro.kompoz.rust
// ^^^^^^^^^^^^ entity.name.function.rust
    let x = 1;
//  ^^^ keyword.other.rust
//  ^^^ - storage.type.variable.kotlin
    x
}

kompoz!(
// <------- meta.macro.kompoz.rust entity.name.function.macro.rust
//     ^ punctuation.brackets.round.rust
    fun Row(items: List<String>) = Column(Modifier.fillMaxWidth()) {
//  ^^^ meta.embedded.block.kotlin storage.type.function.kotlin
//                                        ^^^^^^^^ entity.name.type.kotlin
        items.forEach { Text(if (it == ")") "(" else it) }
//                                     ^^^ string.quoted.double.kotlin
//                                          ^^^ string.quoted.double.kotlin
//                                              ^^^^ keyword.control.kotlin
    }
    val pair = listOf(Pair(1, 2), Pair(3, (4)))
//  ^^^ meta.embedded.block.kotlin storage.type.variable.kotlin
//                                         ^ constant.numeric.decimal.kotlin
//                                           ^^ meta.embedded.block.kotlin
);
// <- meta.macro.kompoz.rust punctuation.brackets.round.rust
// <- - meta.embedded.block.kotlin
//^ - meta.macro.kompoz.rust

kompoz![
// <------- meta.macro.kompoz.rust entity.name.function.macro.rust
//     ^ punctuation.brackets.square.rust
    val grid = arrayOf(intArrayOf(1, 2), intArrayOf(3, 4))[0]
//  ^^^ meta.embedded.block.kotlin storage.type.variable.kotlin
//                                                         ^ constant.numeric.decimal.kotlin
    val s = "]"
//          ^^^ string.quoted.double.kotlin
];
// <- meta.macro.kompoz.rust punctuation.brackets.square.rust
// <- - meta.embedded.block.kotlin

fn after_all() {
// <-- keyword.other.fn.rust
    let s = "kompoz! { not Kotlin }";
//          ^^^^^^^^^^^^^^^^^^^^^^^^ string.quoted.double.rust
//           ^^^^^^^ - entity.name.function.macro.rust
    // kompoz! { not Kotlin either }
//     ^^^^^^^ comment.line.double-slash.rust
//     ^^^^^^^ - meta.macro.kompoz.rust
    let page = kompoz! { Text("inline") };
//             ^^^^^^^ meta.macro.kompoz.rust entity.name.function.macro.rust
//                       ^^^^ meta.embedded.block.kotlin entity.name.function.call.kotlin
//                                      ^ punctuation.brackets.curly.rust
//                                       ^ - meta.macro.kompoz.rust
    kompoz::kompoz_file!("src/main.kt");
//          ^^^^^^^^^^^^ entity.name.function.macro.rust
//          ^^^^^^^^^^^^ - meta.macro.kompoz.rust
//                       ^^^^^^^^^^^^^ string.quoted.double.rust
}

kompoz!
// <------- meta.macro.kompoz.rust entity.name.function.macro.rust
{
// <- meta.macro.kompoz.rust punctuation.brackets.curly.rust
    fun Later() = Text("}")
//  ^^^ meta.embedded.block.kotlin storage.type.function.kotlin
//                     ^^^ string.quoted.double.kotlin
}
// <- meta.macro.kompoz.rust punctuation.brackets.curly.rust
// <- - meta.embedded.block.kotlin

const LATER: u32 = 1;
// <----- - meta.macro.kompoz.rust
// <----- storage.type.rust

kompoz!
    (Text(listOf(1, 2)[0].toString()))
//  ^ meta.macro.kompoz.rust punctuation.brackets.round.rust
//   ^^^^ meta.embedded.block.kotlin entity.name.function.call.kotlin
//                                   ^ meta.macro.kompoz.rust punctuation.brackets.round.rust
//                                    ^ - meta.macro.kompoz.rust
fn last() {}
// <-- keyword.other.fn.rust
// <-- - meta.macro.kompoz.rust
