// A NavigationBar's content is Kotlin's RowScope (its Rust closure takes none): `NavigationBarItem` inside is
// `RowScope.NavigationBarItem`; the short bar's value classes (`ShortNavigationBarArrangement.Centered`,
// `NavigationItemIconPosition.Start`) and the items' defaults objects used as values.

@Composable
fun Bars(selected: Int, onSelect: (Int) -> Unit) {
    Column {
        NavigationBar {
            NavigationBarItem(
                selected = selected == 0,
                onClick = { onSelect(0) },
                icon = { Icon(Icons.Filled.Home, contentDescription = null) },
                label = { Text("Bir") },
                colors = NavigationBarItemDefaults.colors(indicatorColor = Color.Red),
            )
        }
        ShortNavigationBar(arrangement = ShortNavigationBarArrangement.Centered) {
            ShortNavigationBarItem(
                selected = selected == 1,
                onClick = { onSelect(1) },
                icon = { Icon(Icons.Filled.Star, contentDescription = null) },
                label = { Text("İki") },
                iconPosition = NavigationItemIconPosition.Start,
                colors = ShortNavigationBarItemDefaults.colors(),
            )
        }
    }
}
