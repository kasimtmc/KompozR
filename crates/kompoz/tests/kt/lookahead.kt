// Lookahead in a Kotlin file of its own (kompoz_file!): a scroller placing its content on its own frame of reference,
// and an approach layout placed on its way by its placement approach, as Compose writes them.
package com.example.lookahead

import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.*

@Composable
fun Scroller(scrolled: MutableState<Int>, seen: (Offset, Offset, Boolean) -> Unit) {
    Layout(
        content = {
            // (told of its outer coordinates, the ones its parent placed: before its size)
            Box(modifier = Modifier.onPlaced { coordinates ->
                val parent = coordinates.parentLayoutCoordinates!!
                seen(
                    parent.localPositionOf(coordinates, Offset.Zero),
                    parent.localPositionOf(coordinates, Offset.Zero, includeMotionFrameOfReference = false),
                    coordinates.introducesMotionFrameOfReference,
                )
            }.size(20.dp))
        },
        modifier = Modifier.size(100.dp),
    ) { measurables, constraints ->
        val placeable = measurables[0].measure(constraints.copy(minWidth = 0, minHeight = 0))
        layout(100, 100) {
            withMotionFrameOfReferencePlacement {
                placeable.place(0, scrolled.value)
            }
        }
    }
}

@Composable
fun Mover(moving: MutableState<Boolean>) {
    LookaheadScope {
        Box(
            modifier = Modifier.size(100.dp).approachLayout(
                isMeasurementApproachInProgress = { false },
                isPlacementApproachInProgress = { lookaheadCoordinates -> moving.value },
                approachMeasure = { measurable, constraints ->
                    val placeable = measurable.measure(constraints)
                    layout(placeable.width, placeable.height) { placeable.place(30, 0) }
                },
            ),
        ) {
            Box(modifier = Modifier.size(20.dp).testTag("taşınan"))
        }
    }
}
