// A floating toolbar's content, leading and trailing content are Kotlin's layout scope lambdas (their Rust closures
// take none): a horizontal one's RowScope (`AnimatedVisibility` inside is `RowScope.AnimatedVisibility`), a vertical
// one's ColumnScope; the FAB's lambda of the FAB forms, before the content, is a plain one (in a Box: no outer scope).

@Composable
fun Toolbars(shown: Boolean) {
    Box {
        HorizontalFloatingToolbar(
            expanded = true,
            leadingContent = { AnimatedVisibility(shown) { Text("baş") } },
            trailingContent = { AnimatedVisibility(shown) { Text("son") } },
        ) {
            AnimatedVisibility(shown) { Text("orta") }
        }
        VerticalFloatingToolbar(
            expanded = true,
            floatingActionButton = { AnimatedVisibility(shown) { Text("fab") } },
        ) {
            AnimatedVisibility(shown) { Text("sütun") }
        }
    }
}
