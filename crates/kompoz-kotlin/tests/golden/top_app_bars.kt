// A top app bar's actions are Kotlin's RowScope lambda (`actions: @Composable RowScope.() -> Unit`; its Rust closure
// takes none): `AnimatedVisibility` inside is `RowScope.AnimatedVisibility`, in every top app bar (the subtitle form
// too); its title and navigation icon are plain lambdas (in a Column: `ColumnScope.AnimatedVisibility`, the outer
// scope).

@Composable
fun Bars(shown: Boolean) {
    Column {
        TopAppBar(
            title = { AnimatedVisibility(shown) { Text("başlık") } },
            actions = { AnimatedVisibility(shown) { Text("eylem") } },
        )
        TopAppBar(title = { Text("başlık") }, subtitle = { Text("alt") }, actions = { AnimatedVisibility(shown) { Text("alt eylem") } })
        CenterAlignedTopAppBar(title = { Text("orta") }, actions = { AnimatedVisibility(shown) { Text("orta eylem") } })
        MediumTopAppBar(title = { Text("orta boy") }, actions = { AnimatedVisibility(shown) { Text("orta boy eylem") } })
        LargeFlexibleTopAppBar(title = { Text("büyük") }, actions = { AnimatedVisibility(shown) { Text("büyük eylem") } })
        TwoRowsTopAppBar(title = { expanded -> Text("iki") }, actions = { AnimatedVisibility(shown) { Text("iki eylem") } })
    }
}
