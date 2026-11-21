// Material's text fields as Compose writes them: TextField's overloads by their first argument's type (`state` a
// TextFieldState, `value` a String or a TextFieldValue); slot lambdas (`label`, `prefix`, `trailingIcon` an
// IconButton, `supportingText` an `if` of a lambda and `null`); TextFieldLabelPosition.Above(); TextFieldDefaults'
// `shape` (a property), `colors(…)`, `DecorationBox(…)` and `Container(…)` (composable methods of the object); a lambda
// given where an InputTransformation goes (its SAM constructor, `then { … }`); a safe call on a local Compose holds
// nullable (`keyboardController?.hide()`); a `CharSequence`; a buffer's `usize`s read and given as Kotlin's `Int`;
// a function of a nullable result returning a value; an extension of `List<String>`; `for (c in text)`; a property
// of a measured text (`label.size.width / 2`, the division converted whole).
@Composable
fun Fields(error: Boolean) {
    val state = rememberTextFieldState()
    var text by remember { mutableStateOf("") }
    var value by remember { mutableStateOf(TextFieldValue("ab")) }
    val keyboardController = LocalSoftwareKeyboardController.current
    Column {
        TextField(state = state, label = { Text("Ad") }, labelPosition = TextFieldLabelPosition.Above(), prefix = { Text("₺") },
            supportingText = if (error) { { Text("Hata") } } else null, shape = TextFieldDefaults.shape,
            inputTransformation = InputTransformation.maxLength(4).then { if (!asCharSequence().isDigitsOnly()) revertAllChanges() },
            onKeyboardAction = { keyboardController?.hide() })
        TextField(value = text, onValueChange = { text = it }, trailingIcon = { IconButton(onClick = { text = "" }) { Icon(Icons.Filled.Clear, contentDescription = "Sil") } })
        OutlinedTextField(value = value, onValueChange = { value = it }, colors = OutlinedTextFieldDefaults.colors(unfocusedBorderColor = Color.Red))
        BasicTextField(value = text, onValueChange = { text = it }, decorationBox = { inner ->
            TextFieldDefaults.DecorationBox(value = text, innerTextField = inner, enabled = true, singleLine = true,
                visualTransformation = VisualTransformation.None, interactionSource = remember { MutableInteractionSource() },
                container = { TextFieldDefaults.Container(enabled = true, isError = false, interactionSource = remember { MutableInteractionSource() }, colors = TextFieldDefaults.colors()) })
        })
    }
}

private fun TextFieldState.addComma(at: Int, text: CharSequence) = edit {
    if (at == length || charAt(at) != ',') insert(at, ", ")
    replace(0, 0, text)
}

private fun firstDigit(text: CharSequence): Char? {
    for (c in text) {
        if (c.isDigit()) return c
    }
    return null
}

private fun List<String>.longest(): String? = if (isEmpty()) null else maxBy { it.length }

private fun cursorOf(state: TextFieldState): Int = state.selection.start - 1
