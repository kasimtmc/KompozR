// The state-based text fields in a Kotlin file of their own (kompoz_file!): a field whose keyboard action is Next
// handled by a `KeyboardActionHandler { … }` running the default action (the focus moves on), a secure field with an
// obfuscation character of its own and a lambda as its decorator, and kompozable_tests.cpp 2862-2893's phone field
// with its input and output transformations (Material's OutlinedTextField, as the C++ wrote it) — as Compose writes
// them.
package com.example.textfield

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.*
import androidx.compose.foundation.text.input.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.*
import androidx.compose.ui.platform.*
import androidx.compose.ui.text.input.*

@Composable
fun Login(submit: (String, String) -> Unit) {
    val user = rememberTextFieldState()
    val password = rememberTextFieldState()
    Column {
        BasicTextField(
            state = user,
            modifier = Modifier.width(200.dp).testTag("kullanıcı"),
            lineLimits = TextFieldLineLimits.SingleLine,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
            onKeyboardAction = KeyboardActionHandler { performDefaultAction -> performDefaultAction() },
        )
        BasicSecureTextField(
            state = password,
            modifier = Modifier.width(200.dp).testTag("parola"),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
            onKeyboardAction = { submit(user.text.toString(), password.text.toString()) },
            decorator = { innerTextField ->
                Box(Modifier.padding(4.dp).testTag("parola-iç")) { innerTextField() }
            },
            textObfuscationMode = TextObfuscationMode.Hidden,
            textObfuscationCharacter = '*',
        )
    }
}

@Composable
fun Phone() {
    val phone = rememberTextFieldState("5551234567")
    var digits by remember { mutableStateOf("a1b2") }
    val focus = LocalFocusManager.current
    Column {
        OutlinedTextField(state = phone, modifier = Modifier.testTag("telefon"),
                          inputTransformation = InputTransformation.byValue { current, proposed -> if (proposed.all { it.isDigit() }) proposed else current }
                              .then(InputTransformation.maxLength(10)),
                          outputTransformation = OutputTransformation { for (at in listOf(8, 6, 3)) if (length > at) insert(at, " ") })
        Text("değer ${phone.text} · ${digits.filter { it.isDigit() }} · ${"a b".filter { !it.isWhitespace() }}")
        Button(onClick = { focus.clearFocus() }) { Text("Odağı bırak") }
    }
}
