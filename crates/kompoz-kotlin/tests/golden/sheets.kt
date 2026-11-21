// Sheets as Compose writes them (WP5): `null` to a nullable slot (`sheetDragHandle = null`); a corner shape of four
// corners (`RoundedCornerShape(0.dp, 24.dp, 24.dp, 0.dp)`, its four-corner constructor); a focus properties block
// setting `canFocus` (FocusProperties' Kotlin properties).
@Composable
fun Sheets() {
    val scaffoldState = rememberBottomSheetScaffoldState()
    BottomSheetScaffold(
        sheetContent = {
            Button(
                modifier = Modifier.focusProperties { canFocus = scaffoldState.bottomSheetState.currentValue == SheetValue.Expanded },
                onClick = {},
            ) { Text("Collapse") }
        },
        scaffoldState = scaffoldState,
        sheetDragHandle = null,
        sheetShape = RoundedCornerShape(0.dp, 24.dp, 24.dp, 0.dp),
    ) { padding ->
        Box(Modifier.padding(padding))
    }
}
