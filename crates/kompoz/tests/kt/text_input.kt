// A text field's state and transformations in a Kotlin file of its own (kompoz_file!): a state remembered with its
// text, InputTransformation.byValue { current, proposed -> } chained with maxLength, an OutputTransformation
// inserting separators, and the state edited in code (append, replace) — as Compose writes them (kompozable_tests.cpp
// 2862-2893's; its OutlinedTextField is Material's, stage 12's: the state and transformations are handed out to be
// applied as a field does).
package com.example.textinput

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.input.*
import androidx.compose.runtime.*
import androidx.compose.ui.*

@Composable
fun Phone(fields: (TextFieldState, InputTransformation, OutputTransformation) -> Unit) {
    val phone = rememberTextFieldState("5551234567")
    var digits by remember { mutableStateOf("a1b2") }
    val onlyDigits = InputTransformation.byValue { current, proposed -> if (proposed.all { it.isDigit() }) proposed else current }
        .then(InputTransformation.maxLength(10))
    val grouped = OutputTransformation { for (at in listOf(8, 6, 3)) if (length > at) insert(at, " ") }
    fields(phone, onlyDigits, grouped)
    Column {
        Text("değer ${phone.text} · ${digits.filter { it.isDigit() }} · ${"a b".filter { !it.isWhitespace() }}")
    }
}

@Composable
fun Note(edits: Int) {
    val note = rememberTextFieldState()
    LaunchedEffect(edits) {
        note.edit {
            append("satır $edits")
            if (length > 9) replace(0, 2, "S")
        }
    }
    Text("not: ${note.text}", modifier = Modifier.testTag("not"))
}
