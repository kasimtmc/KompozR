// The state-based text fields in a Kotlin file of their own (kompoz_file!): a field whose keyboard action is Next
// handled by a `KeyboardActionHandler { … }` running the default action (the focus moves on), a secure field with an
// obfuscation character of its own and a lambda as its decorator, and kompozable_tests.cpp 2862-2893's phone field
// with its input and output transformations — as Compose writes them. The C++'s field was Material's
// OutlinedTextField, stage 12's: BasicTextField stands for it.
package com.example.textfield

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.*
import androidx.compose.foundation.text.input.*
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
        BasicTextField(
            state = phone,
            modifier = Modifier.width(200.dp).testTag("telefon"),
            inputTransformation = InputTransformation.byValue { current, proposed -> if (proposed.all { it.isDigit() }) proposed else current }
                .then(InputTransformation.maxLength(10)),
            outputTransformation = OutputTransformation { for (at in listOf(8, 6, 3)) if (length > at) insert(at, " ") },
        )
        Text("değer ${phone.text} · ${digits.filter { it.isDigit() }} · ${"a b".filter { !it.isWhitespace() }}")
        // (the C++'s Button is Material's: a clickable Box stands for it)
        Box(Modifier.clickable { focus.clearFocus(force = false) }) { Text("Odağı bırak") }
    }
}
