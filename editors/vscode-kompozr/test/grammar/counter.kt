// SYNTAX TEST "source.kotlin.kompozr" "A .kt file read by kompoz_file!: KompozR's Kotlin"
package com.example.counter
// <------- storage.type.kotlin

import androidx.compose.runtime.*
// <------ storage.type.kotlin

/** The button's color. */
// <--- comment.block.kotlin
val Accent = Color(0xff3f51b5)
// <--- storage.type.variable.kotlin
//  ^^^^^^ variable.other.declaration.kotlin
//           ^^^^^ entity.name.function.call.kotlin
//                 ^^^^^^^^^^ constant.numeric.hex.kotlin

@Composable
// <- storage.type.annotation.kotlin punctuation.definition.annotation.kotlin
fun Counter(label: String = "Count: }", step: Int = 1) {
// <--- storage.type.function.kotlin
//  ^^^^^^^ entity.name.function.kotlin
//                 ^^^^^^ entity.name.type.kotlin
//                          ^^^^^^^^^^ string.quoted.double.kotlin
//                                                  ^ constant.numeric.decimal.kotlin
//                                                     ^ punctuation.section.block.begin.kotlin
    var count by rememberSaveable { mutableStateOf(0) }
//  ^^^ storage.type.variable.kotlin
//      ^^^^^ variable.other.declaration.kotlin
//            ^^ storage.type.kotlin
//               ^^^^^^^^^^^^^^^^ entity.name.function.call.kotlin
    Text("$label ${count + step}!", modifier = Modifier.padding(16.dp))
//  ^^^^ entity.name.function.call.kotlin
//        ^ punctuation.definition.template-expression.kotlin
//         ^^^^^ variable.other.template.kotlin
//               ^^ meta.template.expression.kotlin punctuation.definition.template-expression.begin.kotlin
//                 ^^^^^ meta.template.expression.kotlin meta.embedded.line.kotlin
//                                                              ^^ constant.numeric.decimal.kotlin
    when (val shape = current) {
//  ^^^^ keyword.control.kotlin
        is Shape.Circle -> return@Counter
//      ^^ keyword.operator.type.kotlin
//                      ^^ keyword.operator.arrow.kotlin
//                         ^^^^^^ keyword.control.kotlin
//                               ^ punctuation.definition.label.kotlin
//                                ^^^^^^^ entity.name.label.kotlin
        else -> count?.let { it + 1 } ?: 0
//                   ^^ keyword.operator.safe-call.kotlin
//                           ^^ variable.language.kotlin
//                                    ^^ keyword.operator.elvis.kotlin
    }
}
// <- punctuation.section.block.end.kotlin

sealed interface Shape {
// <------ storage.modifier.kotlin
//     ^^^^^^^^^ storage.type.class.kotlin
//               ^^^^^ entity.name.type.class.kotlin
    data class Circle(val radius: Float) : Shape
//  ^^^^ storage.modifier.kotlin
//       ^^^^^ storage.type.class.kotlin
//             ^^^^^^ entity.name.type.class.kotlin
    object Empty : Shape
//  ^^^^^^ storage.type.class.kotlin
//         ^^^^^ entity.name.type.class.kotlin
}
val c = 'x'; val big = 0xFF_00L; val half = 0.5f; val no = null
//      ^^^ string.quoted.single.kotlin
//                     ^^^^^^^^ constant.numeric.hex.kotlin
//                                          ^^^^ constant.numeric.decimal.kotlin
//                                                          ^^^^ constant.language.kotlin
