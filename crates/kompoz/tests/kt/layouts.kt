// Layouts as Compose writes them, from a Kotlin file of its own (kotlin_layouts.rs reads this file through
// kompoz_file!): a FlowColumn whose overflow counts what it left out, a ContextualFlowRow and a ContextualFlowColumn
// whose items know their line, a FlexBox whose config depends on its constraints, and a BasicTooltipBox shown by a
// long press.
package com.example.layouts

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.*

@Composable
fun MoreLayouts() {
    Column {
        FlowColumn(
            modifier = Modifier.height(70.dp).testTag("sütun"),
            maxLines = 1,
            overflow = FlowColumnOverflow.expandIndicator {
                Box(modifier = Modifier.size(30.dp, 10.dp).testTag("daha"))
            },
        ) {
            for (i in 0 until 4) Box(modifier = Modifier.size(30.dp).testTag("c$i"))
        }
        ContextualFlowRow(itemCount = 3, modifier = Modifier.width(100.dp)) { index ->
            Text("$index:$lineIndex", modifier = Modifier.width(45.dp).testTag("ö$index"))
        }
        ContextualFlowColumn(itemCount = 2, modifier = Modifier.height(30.dp)) { index ->
            Box(modifier = Modifier.size(20.dp).testTag("k$index:$indexInLine"))
        }
        FlexBox(
            modifier = Modifier.width(150.dp).testTag("dar"),
            config = { wrap(if (constraints.maxWidth < 200) FlexWrap.Wrap else FlexWrap.NoWrap) },
        ) {
            Box(modifier = Modifier.size(100.dp, 20.dp))
            Box(modifier = Modifier.size(100.dp, 20.dp).testTag("ikinci"))
        }
        BasicTooltipBox(
            positionProvider = below(),
            tooltip = { Box(modifier = Modifier.size(50.dp, 20.dp).testTag("ipucu")) },
            state = rememberBasicTooltipState(),
        ) {
            Box(modifier = Modifier.size(40.dp).testTag("çapa"))
        }
    }
}
