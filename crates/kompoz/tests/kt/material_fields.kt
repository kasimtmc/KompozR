// Material's text fields in a Kotlin file of their own (kompoz_file!): material3 1.5's TextFieldSamples.kt as Compose
// writes them — test tags added to find them — and the C++ Kompoz's own Compose code (kompozable_tests.cpp's TextForms,
// Phone; its AddTextDialog is in kotlin_lists_sheets.rs, beside its Dialog), then the API the samples leave out (the
// label above, the indicator line, the field's shape and colors). What waits for other packages: the samples'
// TooltipBox around an IconButton (WP7: the IconButton as it is), the Checkbox showing the placeholder (WP2: a
// clickable Text in its place). TextFieldWithErrorState's local function `validate` is a lambda here (Kotlin's local
// functions come with stage 14).
package com.example.materialfields

import androidx.compose.foundation.*
import androidx.compose.foundation.interaction.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.*
import androidx.compose.foundation.text.input.*
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.*
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.*
import androidx.compose.ui.platform.*
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.*
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.*
import androidx.compose.ui.unit.*
import androidx.core.text.isDigitsOnly

// ---- material3's TextFieldSamples.kt ------------------------------------------------------------------------------

@Composable
fun SimpleTextFieldSample() {
    TextField(
        state = rememberTextFieldState(),
        lineLimits = TextFieldLineLimits.SingleLine,
        label = { Text("Label") },
        modifier = Modifier.testTag("simple"),
    )
}

@Composable
fun SimpleOutlinedTextFieldSample() {
    OutlinedTextField(
        state = rememberTextFieldState(),
        lineLimits = TextFieldLineLimits.SingleLine,
        label = { Text("Label") },
        modifier = Modifier.testTag("outlined"),
    )
}

@Composable
fun TextFieldWithTransformations() {
    TextField(
        state = rememberTextFieldState(),
        lineLimits = TextFieldLineLimits.SingleLine,
        label = { Text("Phone number") },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        // Input transformation to limit user input to 10 digits
        inputTransformation =
            InputTransformation.maxLength(10).then {
                if (!this.asCharSequence().isDigitsOnly()) {
                    revertAllChanges()
                }
            },
        outputTransformation = {
            // Output transformation to format as a phone number: (XXX) XXX-XXXX
            if (length > 0) insert(0, "(")
            if (length > 4) insert(4, ") ")
            if (length > 9) insert(9, "-")
        },
        modifier = Modifier.testTag("phone"),
    )
}

@Composable
fun TextFieldWithIcons() {
    val state = rememberTextFieldState()

    TextField(
        state = state,
        lineLimits = TextFieldLineLimits.SingleLine,
        label = { Text("Label") },
        leadingIcon = { Icon(Icons.Filled.Favorite, contentDescription = null) },
        trailingIcon = {
            IconButton(onClick = { state.clearText() }) {
                Icon(Icons.Filled.Clear, contentDescription = "Clear text")
            }
        },
        modifier = Modifier.testTag("icons"),
    )
}

@Composable
fun TextFieldWithPlaceholder() {
    var alwaysMinimizeLabel by remember { mutableStateOf(false) }
    Column {
        Text("Show placeholder even when unfocused", modifier = Modifier.clickable { alwaysMinimizeLabel = !alwaysMinimizeLabel })
        Spacer(Modifier.height(16.dp))
        TextField(
            state = rememberTextFieldState(),
            lineLimits = TextFieldLineLimits.SingleLine,
            label = { Text("Email") },
            labelPosition = TextFieldLabelPosition.Attached(alwaysMinimize = alwaysMinimizeLabel),
            placeholder = { Text("example@gmail.com") },
        )
    }
}

@Composable
fun TextFieldWithPrefixAndSuffix() {
    var alwaysMinimizeLabel by remember { mutableStateOf(false) }
    Column {
        Text("Show placeholder even when unfocused", modifier = Modifier.clickable { alwaysMinimizeLabel = !alwaysMinimizeLabel })
        Spacer(Modifier.height(16.dp))
        TextField(
            state = rememberTextFieldState(),
            lineLimits = TextFieldLineLimits.SingleLine,
            label = { Text("Label") },
            labelPosition = TextFieldLabelPosition.Attached(alwaysMinimize = alwaysMinimizeLabel),
            prefix = { Text("www.") },
            suffix = { Text(".com") },
            placeholder = { Text("google") },
        )
    }
}

@Composable
fun TextFieldWithErrorState() {
    val errorMessage = "Text input too long"
    val state = rememberTextFieldState()
    var isError by rememberSaveable { mutableStateOf(false) }
    val charLimit = 10

    val validate = { text: CharSequence -> isError = text.length > charLimit }

    LaunchedEffect(Unit) {
        // Run validation whenever text value changes
        snapshotFlow { state.text }.collect { validate(it) }
    }
    TextField(
        state = state,
        lineLimits = TextFieldLineLimits.SingleLine,
        label = { Text(if (isError) "Username*" else "Username") },
        supportingText = {
            Row {
                Text(if (isError) errorMessage else "", Modifier.clearAndSetSemantics {})
                Spacer(Modifier.weight(1f))
                Text("Limit: ${state.text.length}/$charLimit")
            }
        },
        isError = isError,
        onKeyboardAction = { validate(state.text) },
        modifier =
            Modifier.testTag("error").semantics {
                maxTextLength = charLimit
                // Provide localized description of the error
                if (isError) error(errorMessage)
            },
    )
}

@Composable
fun TextFieldWithSupportingText() {
    TextField(
        state = rememberTextFieldState(),
        lineLimits = TextFieldLineLimits.SingleLine,
        label = { Text("Label") },
        supportingText = {
            Text("Supporting text that is long and perhaps goes onto another line.")
        },
        modifier = Modifier.testTag("supporting"),
    )
}

@Composable
fun PasswordTextField() {
    var passwordHidden by rememberSaveable { mutableStateOf(true) }
    SecureTextField(
        state = rememberTextFieldState(),
        label = { Text("Enter password") },
        textObfuscationMode =
            if (passwordHidden) TextObfuscationMode.RevealLastTyped
            else TextObfuscationMode.Visible,
        trailingIcon = {
            // Provide localized description for accessibility services
            val description = if (passwordHidden) "Show password" else "Hide password"
            IconButton(onClick = { passwordHidden = !passwordHidden }) {
                val visibilityIcon =
                    if (passwordHidden) Icons.Filled.Visibility else Icons.Filled.VisibilityOff
                Icon(imageVector = visibilityIcon, contentDescription = description)
            }
        },
        modifier = Modifier.testTag("password"),
    )
}

@Composable
fun TextFieldWithInitialValueAndSelection() {
    val state = rememberTextFieldState("Initial text", TextRange(0, 12))
    TextField(state = state, lineLimits = TextFieldLineLimits.SingleLine, label = { Text("Label") }, modifier = Modifier.testTag("initial"))
}

@Composable
fun OutlinedTextFieldWithInitialValueAndSelection() {
    val state = rememberTextFieldState("Initial text", TextRange(0, 12))
    OutlinedTextField(
        state = state,
        lineLimits = TextFieldLineLimits.SingleLine,
        label = { Text("Label") },
        modifier = Modifier.testTag("initial-outlined"),
    )
}

@Composable
fun DenseTextFieldContentPadding() {
    TextField(
        state = rememberTextFieldState(),
        lineLimits = TextFieldLineLimits.SingleLine,
        label = { Text("Label") },
        // Need to set a min height using `heightIn` to override the default
        modifier = Modifier.heightIn(min = 48.dp).testTag("dense"),
        contentPadding = PaddingValues(top = 4.dp, bottom = 4.dp, start = 12.dp, end = 12.dp),
    )
}

@Composable
fun TextFieldWithHideKeyboardOnImeAction() {
    val keyboardController = LocalSoftwareKeyboardController.current
    TextField(
        state = rememberTextFieldState(),
        label = { Text("Label") },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        onKeyboardAction = { keyboardController?.hide() },
        modifier = Modifier.testTag("hide"),
    )
}

@Composable
fun TextArea() {
    val state =
        rememberTextFieldState(
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor " +
                "incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quisque " +
                "nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. " +
                "Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu " +
                "fugiat nulla pariatur. Excepteur sint occaecat cupidatat non  proident, sunt in " +
                "culpa qui officia deserunt mollit anim id est laborum."
        )
    TextField(state = state, modifier = Modifier.height(120.dp).testTag("area"), label = { Text("Label") })
}

@Composable
fun CustomTextFieldUsingDecorator() {
    val state = rememberTextFieldState()
    val interactionSource = remember { MutableInteractionSource() }
    val customColors =
        TextFieldDefaults.colors(
            focusedTextColor = Color.DarkGray,
            unfocusedTextColor = Color.Gray,
            focusedIndicatorColor = Color.Blue,
            cursorColor = Color.Green,
        )
    val enabled = true
    val isError = false
    val lineLimits = TextFieldLineLimits.SingleLine

    val textColor =
        LocalTextStyle.current.color.takeOrElse {
            customColors.textColor(
                enabled = enabled,
                isError = isError,
                focused = interactionSource.collectIsFocusedAsState().value,
            )
        }

    BasicTextField(
        state = state,
        modifier = Modifier.testTag("decorated"),
        interactionSource = interactionSource,
        enabled = enabled,
        lineLimits = lineLimits,
        // Colors of non-decorator elements (such as text color or cursor color)
        // must be passed to BasicTextField
        textStyle = LocalTextStyle.current.merge(TextStyle(color = textColor)),
        cursorBrush = SolidColor(customColors.cursorColor),
        decorator =
            TextFieldDefaults.decorator(
                state = state,
                outputTransformation = null,
                lineLimits = lineLimits,
                enabled = enabled,
                isError = isError,
                interactionSource = interactionSource,
                container = {
                    TextFieldDefaults.Container(
                        enabled = enabled,
                        isError = isError,
                        interactionSource = interactionSource,
                        colors = customColors,
                        // Update indicator line thickness
                        unfocusedIndicatorLineThickness = 2.dp,
                        focusedIndicatorLineThickness = 4.dp,
                    )
                },
            ),
    )
}

@Composable
fun CustomOutlinedTextFieldUsingDecorator() {
    val state = rememberTextFieldState()
    val interactionSource = remember { MutableInteractionSource() }
    val customColors =
        OutlinedTextFieldDefaults.colors(
            focusedTextColor = Color.DarkGray,
            unfocusedTextColor = Color.Gray,
            focusedBorderColor = Color.Blue,
            cursorColor = Color.Green,
        )
    val enabled = true
    val isError = false
    val lineLimits = TextFieldLineLimits.SingleLine

    val textColor =
        LocalTextStyle.current.color.takeOrElse {
            customColors.textColor(
                enabled = enabled,
                isError = isError,
                focused = interactionSource.collectIsFocusedAsState().value,
            )
        }

    BasicTextField(
        state = state,
        modifier = Modifier.testTag("outlined-decorated"),
        interactionSource = interactionSource,
        enabled = enabled,
        lineLimits = lineLimits,
        // Colors of non-decorator elements (such as text color or cursor color)
        // must be passed to BasicTextField
        textStyle = LocalTextStyle.current.merge(TextStyle(color = textColor)),
        cursorBrush = SolidColor(customColors.cursorColor),
        decorator =
            OutlinedTextFieldDefaults.decorator(
                state = state,
                outputTransformation = null,
                lineLimits = lineLimits,
                enabled = enabled,
                isError = isError,
                interactionSource = interactionSource,
                container = {
                    OutlinedTextFieldDefaults.Container(
                        enabled = enabled,
                        isError = isError,
                        interactionSource = interactionSource,
                        colors = customColors,
                        // Update border thickness and shape
                        shape = RectangleShape,
                        unfocusedBorderThickness = 2.dp,
                        focusedBorderThickness = 4.dp,
                    )
                },
            ),
    )
}

@Composable
fun CustomTextFieldBasedOnDecorationBox() {
    var text by remember { mutableStateOf("") }
    val interactionSource = remember { MutableInteractionSource() }
    val customColors =
        TextFieldDefaults.colors(
            focusedTextColor = Color.DarkGray,
            unfocusedTextColor = Color.Gray,
            focusedIndicatorColor = Color.Blue,
            cursorColor = Color.Green,
        )
    val enabled = true
    val isError = false
    val singleLine = true

    val textColor =
        LocalTextStyle.current.color.takeOrElse {
            customColors.textColor(
                enabled = enabled,
                isError = isError,
                focused = interactionSource.collectIsFocusedAsState().value,
            )
        }

    BasicTextField(
        value = text,
        onValueChange = { text = it },
        modifier = Modifier.testTag("box-decorated"),
        interactionSource = interactionSource,
        enabled = enabled,
        singleLine = singleLine,
        // Colors of non-decoration-box elements (such as text color or cursor color)
        // must be passed to BasicTextField
        textStyle = LocalTextStyle.current.merge(TextStyle(color = textColor)),
        cursorBrush = SolidColor(customColors.cursorColor),
        decorationBox = { innerTextField ->
            TextFieldDefaults.DecorationBox(
                value = text,
                innerTextField = innerTextField,
                visualTransformation = VisualTransformation.None,
                singleLine = singleLine,
                enabled = enabled,
                isError = isError,
                interactionSource = interactionSource,
                container = {
                    TextFieldDefaults.Container(
                        enabled = enabled,
                        isError = isError,
                        interactionSource = interactionSource,
                        colors = customColors,
                        // Update indicator line thickness
                        unfocusedIndicatorLineThickness = 2.dp,
                        focusedIndicatorLineThickness = 4.dp,
                    )
                },
            )
        },
    )
}

@Composable
fun CustomOutlinedTextFieldBasedOnDecorationBox() {
    var text by remember { mutableStateOf("") }
    val interactionSource = remember { MutableInteractionSource() }
    val customColors =
        OutlinedTextFieldDefaults.colors(
            focusedTextColor = Color.DarkGray,
            unfocusedTextColor = Color.Gray,
            focusedBorderColor = Color.Blue,
            cursorColor = Color.Green,
        )
    val enabled = true
    val isError = false
    val singleLine = true

    val textColor =
        LocalTextStyle.current.color.takeOrElse {
            customColors.textColor(
                enabled = enabled,
                isError = isError,
                focused = interactionSource.collectIsFocusedAsState().value,
            )
        }

    BasicTextField(
        value = text,
        onValueChange = { text = it },
        modifier = Modifier.testTag("boxed"),
        interactionSource = interactionSource,
        enabled = enabled,
        singleLine = singleLine,
        // Colors of non-decoration-box elements (such as text color or cursor color)
        // must be passed to BasicTextField
        textStyle = LocalTextStyle.current.merge(TextStyle(color = textColor)),
        cursorBrush = SolidColor(customColors.cursorColor),
        decorationBox = { innerTextField ->
            OutlinedTextFieldDefaults.DecorationBox(
                value = text,
                innerTextField = innerTextField,
                visualTransformation = VisualTransformation.None,
                singleLine = singleLine,
                enabled = enabled,
                isError = isError,
                interactionSource = interactionSource,
                container = {
                    OutlinedTextFieldDefaults.Container(
                        enabled = enabled,
                        isError = isError,
                        interactionSource = interactionSource,
                        colors = customColors,
                        // Update border thickness and shape
                        shape = RectangleShape,
                        unfocusedBorderThickness = 2.dp,
                        focusedBorderThickness = 4.dp,
                    )
                },
            )
        },
    )
}

// ---- kompozable_tests.cpp's Compose code ----------------------------------------------------------------------------

// Text as Compose writes it: an annotated string built on its builder, a field with composable
// label and supporting text, a state field with lineLimits and a placeholder, a measurer in a Canvas.
fun openTerms() {}

@Composable
fun TextForms() {
    var email by remember { mutableStateOf("") }
    val message = rememberTextFieldState()
    val measurer = rememberTextMeasurer()
    Column {
        Text(buildAnnotatedString {
            append("accept the ")
            withLink(LinkAnnotation.Clickable("terms") { openTerms() }) { append("terms") }
            withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(" now") }
        })
        OutlinedTextField(value = email, onValueChange = { email = it }, singleLine = true, label = { Text("Email") }, supportingText = { Text("Never shared") })
        OutlinedTextField(state = message, lineLimits = TextFieldLineLimits.MultiLine(maxHeightInLines = 4), placeholder = { Text("Message") })
        SelectionContainer { Text("Order KMP-1") }
        Canvas(modifier = Modifier.size(40.dp)) {
            val label = measurer.measure("7%", style = TextStyle(fontSize = 24.sp))
            drawText(label, topLeft = Offset(center.x - label.size.width / 2, 0f))
        }
    }
}

// A text field's transformations as Compose writes them: InputTransformation.byValue { current, proposed -> },
// OutputTransformation { insert(…) } on the buffer, a text filtered by its characters, LocalFocusManager.
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

// ---- What the samples leave out -------------------------------------------------------------------------------------

// Value-based fields (a String's and a TextFieldValue's: Compose's other overloads), the error state told to
// accessibility.
@Composable
fun ValueTextFields() {
    var text by remember { mutableStateOf("") }
    var value by remember { mutableStateOf(TextFieldValue("ab", TextRange(2))) }
    Column {
        TextField(value = text, onValueChange = { text = it }, label = { Text("Name") }, singleLine = true, modifier = Modifier.testTag("string"))
        OutlinedTextField(value = value, onValueChange = { value = it }, label = { Text("Note") }, isError = value.text.length > 3, modifier = Modifier.testTag("value"))
        Text("text=$text", modifier = Modifier.testTag("echo"))
    }
}

// The label above the field (TextFieldLabelPosition.Above), the theme's field shape, a supporting text only in error
// (an `if` of a lambda and `null`), a prefix and a suffix, outlined fields' colors and shape.
@Composable
fun LabelsAndShapes() {
    val amount = rememberTextFieldState()
    val isError = amount.text.length > 4
    Column {
        TextField(
            state = amount,
            label = { Text("Tutar") },
            labelPosition = TextFieldLabelPosition.Above(),
            prefix = { Text("₺") },
            suffix = { Text(",00") },
            supportingText = if (isError) { { Text("En çok 4 hane") } } else null,
            isError = isError,
            shape = TextFieldDefaults.shape,
            modifier = Modifier.testTag("above"),
        )
        OutlinedTextField(
            state = rememberTextFieldState("kenar"),
            shape = RoundedCornerShape(12.dp),
            colors = OutlinedTextFieldDefaults.colors(unfocusedBorderColor = Color.Red),
            modifier = Modifier.testTag("red-outline"),
        )
        SecureTextField(state = rememberTextFieldState("parola"), modifier = Modifier.testTag("secure"))
        OutlinedSecureTextField(state = rememberTextFieldState("parola"), label = { Text("Parola") }, modifier = Modifier.testTag("outlined-secure"))
    }
}

// A field of one's own: a BasicTextField with Material's indicator line under it (Modifier.indicatorLine).
@Composable
fun IndicatorLineField() {
    val state = rememberTextFieldState("çizgi")
    val interactionSource = remember { MutableInteractionSource() }
    BasicTextField(
        state = state,
        interactionSource = interactionSource,
        modifier =
            Modifier.testTag("line")
                .indicatorLine(
                    enabled = true,
                    isError = false,
                    interactionSource = interactionSource,
                    colors = TextFieldDefaults.colors(unfocusedIndicatorColor = Color.Magenta),
                    unfocusedIndicatorLineThickness = 3.dp,
                )
                .padding(8.dp),
    )
}
