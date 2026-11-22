// A bottom app bar's actions (or its content, in the overload taking one) and a flexible bottom app bar's content are
// Kotlin's RowScope lambdas (`@Composable RowScope.() -> Unit`; their Rust closures take none): `AnimatedVisibility`
// inside is `RowScope.AnimatedVisibility`; the floating action button's lambda is a plain one (in a Column:
// `ColumnScope.AnimatedVisibility`, the outer scope). A Kotlin object's property that is a method of the unit struct of
// its name (`BottomAppBarDefaults.bottomAppBarFabColor`) is that method called on the struct, its own value; one that
// is an associated function (`BottomAppBarDefaults.ContentPadding`) is called on the type.

@Composable
fun Bars(shown: Boolean) {
    Column {
        BottomAppBar(
            actions = { AnimatedVisibility(shown) { Text("eylem") } },
            floatingActionButton = {
                AnimatedVisibility(shown) {
                    FloatingActionButton(onClick = { }, containerColor = BottomAppBarDefaults.bottomAppBarFabColor) { Text("fab") }
                }
            },
            containerColor = BottomAppBarDefaults.containerColor,
            contentPadding = BottomAppBarDefaults.ContentPadding,
        )
        BottomAppBar { AnimatedVisibility(shown) { Text("içerik") } }
        FlexibleBottomAppBar { AnimatedVisibility(shown) { Text("esnek") } }
    }
}
