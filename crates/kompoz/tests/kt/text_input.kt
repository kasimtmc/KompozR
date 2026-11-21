// A text field's state and transformations in a Kotlin file of its own (kompoz_file!): a state remembered with its
// text, InputTransformation.byValue { current, proposed -> } chained with maxLength, an OutputTransformation
// inserting separators given to Material's OutlinedTextField (kompozable_tests.cpp 2862-2893's), and the state edited
// in code (append, replace) — as Compose writes them.
package com.example.textinput

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.input.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.*

@Composable
fun Phone() {
    val phone = rememberTextFieldState("5551234567")
    var digits by remember { mutableStateOf("a1b2") }
    val onlyDigits = InputTransformation.byValue { current, proposed -> if (proposed.all { it.isDigit() }) proposed else current }
        .then(InputTransformation.maxLength(10))
    val grouped = OutputTransformation { for (at in listOf(8, 6, 3)) if (length > at) insert(at, " ") }
    Column {
        OutlinedTextField(state = phone, modifier = Modifier.testTag("telefon"), inputTransformation = onlyDigits, outputTransformation = grouped)
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
