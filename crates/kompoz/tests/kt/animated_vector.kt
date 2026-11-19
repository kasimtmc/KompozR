// Animated vectors in a Kotlin file of its own (kompoz_file!): Compose's AnimatedVectorSample — an image toggled
// between its start and its end by a click — its image read from its XML (Compose's animatedVectorResource waits
// for KompozR's resources: fromAnimatedVectorXml gives Kotlin's Result), and a read that fails, as Kotlin reads a
// Result.
package com.example.animatedvector

import androidx.compose.animation.graphics.res.rememberAnimatedVectorPainter
import androidx.compose.animation.graphics.vector.AnimatedImageVector
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.VectorPainter
import androidx.compose.ui.unit.dp

@Composable
fun AnimatedVectorSample(xml: String, seen: (Boolean, VectorPainter) -> Unit) {
    val image = remember(xml) { AnimatedImageVector.fromAnimatedVectorXml(xml).getOrThrow() }
    var atEnd by remember { mutableStateOf(false) }
    val painter = rememberAnimatedVectorPainter(image, atEnd)
    seen(atEnd, painter)
    Image(
        painter = painter,
        contentDescription = "Your content description",
        modifier = Modifier.size(64.dp).clickable { atEnd = !atEnd },
    )
}

@Composable
fun ReadOrNot(xml: String, seen: (Boolean, String?, Int) -> Unit) {
    val read = AnimatedImageVector.fromAnimatedVectorXml(xml)
    val duration = read.getOrNull()?.totalDuration ?: -1
    seen(read.isSuccess, read.exceptionOrNull()?.toString(), duration)
}
