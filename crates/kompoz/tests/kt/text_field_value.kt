// The value-based text fields in a Kotlin file of their own (kompoz_file!): a sign-in form of String fields — the
// user's name moving the focus on with Next, its password masked by PasswordVisualTransformation with a mask of its
// own and signing in with Done, a button clearing the name — and a note kept as a TextFieldValue the app selects all
// of, as Compose writes them.
package com.example.textfieldvalue

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.*
import androidx.compose.runtime.*
import androidx.compose.ui.*
import androidx.compose.ui.text.*
import androidx.compose.ui.text.input.*

@Composable
fun SignIn(signIn: (String, String) -> Unit) {
    var user by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    Column {
        BasicTextField(
            value = user,
            onValueChange = { user = it },
            modifier = Modifier.width(200.dp).testTag("kullanıcı"),
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
        )
        BasicTextField(
            value = password,
            onValueChange = { password = it },
            modifier = Modifier.width(200.dp).testTag("parola"),
            singleLine = true,
            visualTransformation = PasswordVisualTransformation(mask = '*'),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { signIn(user, password) }),
        )
        BasicText("Temizle", modifier = Modifier.clickable { user = "" })
    }
}

@Composable
fun Note() {
    var note by remember { mutableStateOf(TextFieldValue("taslak")) }
    Column {
        BasicTextField(value = note, onValueChange = { note = it }, modifier = Modifier.width(200.dp).testTag("not"))
        BasicText("Tümünü seç", modifier = Modifier.clickable { note = note.copy(selection = TextRange(0, note.text.length)) })
        BasicText("${note.selection.start}..${note.selection.end}", modifier = Modifier.testTag("not-seçim"))
    }
}
