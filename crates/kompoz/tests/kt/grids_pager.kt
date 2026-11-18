// Lazy grids and pagers in a Kotlin file of their own (kompoz_file!): a gallery whose grid state is read by its
// layout info and scrolled to an item from a coroutine, a horizontal grid with a full-line column, a vertical pager
// telling its current, settled and target pages, and pages of a fixed size snapping to the middle, turned by
// animateScrollToPage, as Compose writes them.
package com.example.grids

import androidx.compose.foundation.*
import androidx.compose.foundation.gestures.snapping.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.foundation.pager.*
import androidx.compose.runtime.*
import androidx.compose.ui.*
import kotlinx.coroutines.launch

@Composable
fun Gallery() {
    val state = rememberLazyGridState()
    val scope = rememberCoroutineScope()
    val columns by remember { derivedStateOf { state.layoutInfo.maxSpan } }
    Column {
        Text("ilk ${state.firstVisibleItemIndex} · $columns sütun", modifier = Modifier.clickable {
            scope.launch { state.scrollToItem(31) }
        })
        LazyVerticalGrid(
            columns = GridCells.Adaptive(minSize = 100.dp),
            state = state,
            modifier = Modifier.size(320.dp, 400.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            items(90, key = { it }, contentType = { "resim" }) { i ->
                Box(Modifier.height(30.dp).testTag("resim $i"))
            }
        }
    }
}

@Composable
fun Shelves() {
    LazyHorizontalGrid(rows = GridCells.Fixed(2), modifier = Modifier.fillMaxWidth().height(200.dp)) {
        item(span = { GridItemSpan(maxLineSpan) }) { Box(Modifier.width(40.dp).fillMaxHeight().testTag("yan")) }
        itemsIndexed(listOf("a", "b", "c", "d"), key = { index, name -> "$name$index" }) { index, _ ->
            Box(Modifier.width(60.dp).fillMaxHeight().testTag("kutu $index"))
        }
    }
}

@Composable
fun Story() {
    val state = rememberPagerState(pageCount = { 4 })
    Column {
        Text("${state.currentPage}/${state.settledPage}/${state.targetPage}")
        VerticalPager(state = state, modifier = Modifier.fillMaxWidth().height(300.dp)) { page ->
            Text("Sayfa $page", modifier = Modifier.testTag("sayfa $page"))
        }
    }
}

@Composable
fun Carousel() {
    val state = rememberPagerState(pageCount = { 6 })
    val scope = rememberCoroutineScope()
    Column {
        Text("ileri", modifier = Modifier.clickable { scope.launch { state.animateScrollToPage(2) } })
        HorizontalPager(
            state = state,
            modifier = Modifier.fillMaxWidth().height(100.dp),
            contentPadding = PaddingValues(horizontal = 50.dp),
            pageSize = PageSize.Fixed(200.dp),
            pageSpacing = 10.dp,
            snapPosition = SnapPosition.Center,
            flingBehavior = PagerDefaults.flingBehavior(state = state, pagerSnapDistance = PagerSnapDistance.atMost(2)),
        ) { page ->
            Box(Modifier.fillMaxSize().testTag("orta $page"))
        }
    }
}
