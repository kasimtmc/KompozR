// Styles in a Kotlin file of their own (kompoz_file!): a card's style made of a base style and a hover style, its
// shape, padding, shadow and border, a hover block animated; the styleable box composed with them.
package com.example.style

import androidx.compose.foundation.*
import androidx.compose.foundation.interaction.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.*
import androidx.compose.foundation.style.*
import androidx.compose.runtime.*
import androidx.compose.ui.*
import androidx.compose.ui.graphics.*
import androidx.compose.ui.platform.*
import androidx.compose.ui.unit.*

val CardBase: Style = Style {
    size(64.dp, 32.dp)
    shape(RoundedCornerShape(8.dp))
    background(Color.Magenta)
    contentPadding(horizontal = 8.dp, vertical = 4.dp)
    dropShadow(Shadow.None)
    // (a Density's members: the scope is one)
    translationX(4.dp.toPx())
}

val CardHover: Style = Style {
    hovered {
        animate {
            background(Color.Cyan)
            borderWidth(1.dp)
            borderColor(Color.Blue)
        }
    }
}

@OptIn(ExperimentalFoundationStyleApi::class)
@Composable
fun StyledCard(modifier: Modifier = Modifier, style: Style = Style) {
    val interactionSource = remember { MutableInteractionSource() }
    val styleState = rememberUpdatedStyleState(interactionSource) { it.isEnabled = true }
    Box(modifier.hoverable(interactionSource).styleable(styleState, CardBase then CardHover then style)) {
        Box(Modifier.fillMaxSize().testTag("kart iç"))
    }
}

@Composable
fun StyleFile() {
    StyledCard(modifier = Modifier.testTag("kart"))
}
