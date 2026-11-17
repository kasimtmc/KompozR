// Lazy lists in a Kotlin file of their own (kompoz_file!): a carousel whose fling snaps an item to the center
// (a LazyRow with rememberSnapFlingBehavior over its state), items filling a fraction of the list and gliding when
// the list is reordered (fillParentMaxWidth, animateItem), the last item in view read through layoutInfo in a
// derived state, and a scroll asked for from a LaunchedEffect, as Compose writes them.
package com.example.lazy

import androidx.compose.animation.core.*
import androidx.compose.foundation.*
import androidx.compose.foundation.gestures.snapping.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.*
import androidx.compose.runtime.*
import androidx.compose.ui.*

@Composable
fun Carousel() {
    val state = rememberLazyListState()
    Column {
        Text("konum ${state.firstVisibleItemIndex}:${state.firstVisibleItemScrollOffset}")
        LazyRow(
            modifier = Modifier.size(300.dp, 100.dp).testTag("carousel"),
            state = state,
            flingBehavior = rememberSnapFlingBehavior(lazyListState = state),
            overscrollEffect = null,
        ) {
            items(20, key = { "kart $it" }, contentType = { "kart" }) { i ->
                Box(Modifier.size(100.dp).testTag("kart $i"))
            }
        }
    }
}

@Composable
fun Shelf(order: List<Int>) {
    LazyColumn(modifier = Modifier.size(200.dp, 400.dp)) {
        items(order, key = { it }) { n ->
            Box(
                Modifier.animateItem(placementSpec = tween(200, easing = LinearEasing))
                    .fillParentMaxWidth(0.5f)
                    .height(50.dp)
                    .testTag("raf $n")
            )
        }
    }
}

@Composable
fun Jump(target: Int) {
    val state = rememberLazyListState()
    val lastVisible by remember { derivedStateOf { state.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: -1 } }
    LaunchedEffect(target) {
        state.scrollToItem(target)
    }
    Column {
        Text("son $lastVisible")
        LazyColumn(state = state, modifier = Modifier.height(200.dp)) {
            items(100) { Text("satır $it", modifier = Modifier.height(50.dp)) }
        }
    }
}
