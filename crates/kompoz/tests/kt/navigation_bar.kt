// Material's navigation bars as Compose's samples write them (material3 1.5's NavigationBarSamples and BadgeSamples'
// NavigationBarItemWithBadge, as written; their @Sampled/@Preview annotations left out).

// ---- NavigationBarSamples.kt ----

@Composable
fun ShortNavigationBarSample() {
    var selectedItem by remember { mutableIntStateOf(0) }
    val items = listOf("Songs", "Artists", "Playlists")
    val selectedIcons = listOf(Icons.Filled.Home, Icons.Filled.Favorite, Icons.Filled.Star)
    val unselectedIcons =
        listOf(Icons.Outlined.Home, Icons.Outlined.FavoriteBorder, Icons.Outlined.StarBorder)

    ShortNavigationBar {
        items.forEachIndexed { index, item ->
            ShortNavigationBarItem(
                icon = {
                    Icon(
                        if (selectedItem == index) selectedIcons[index] else unselectedIcons[index],
                        contentDescription = null,
                    )
                },
                label = { Text(item) },
                selected = selectedItem == index,
                onClick = { selectedItem = index },
            )
        }
    }
}

@Composable
fun ShortNavigationBarWithHorizontalItemsSample() {
    var selectedItem by remember { mutableIntStateOf(0) }
    val items = listOf("Songs", "Artists", "Playlists")
    val selectedIcons = listOf(Icons.Filled.Home, Icons.Filled.Favorite, Icons.Filled.Star)
    val unselectedIcons =
        listOf(Icons.Outlined.Home, Icons.Outlined.FavoriteBorder, Icons.Outlined.StarBorder)

    Column {
        Text(
            "Note: this is configuration is better displayed in medium screen sizes.",
            Modifier.padding(16.dp),
        )

        Spacer(Modifier.height(32.dp))

        ShortNavigationBar(arrangement = ShortNavigationBarArrangement.Centered) {
            items.forEachIndexed { index, item ->
                ShortNavigationBarItem(
                    iconPosition = NavigationItemIconPosition.Start,
                    icon = {
                        Icon(
                            if (selectedItem == index) selectedIcons[index]
                            else unselectedIcons[index],
                            contentDescription = null,
                        )
                    },
                    label = { Text(item) },
                    selected = selectedItem == index,
                    onClick = { selectedItem = index },
                )
            }
        }
    }
}

@Composable
fun NavigationBarSample() {
    var selectedItem by remember { mutableIntStateOf(0) }
    val items = listOf("Songs", "Artists", "Playlists")
    val selectedIcons = listOf(Icons.Filled.Home, Icons.Filled.Favorite, Icons.Filled.Star)
    val unselectedIcons =
        listOf(Icons.Outlined.Home, Icons.Outlined.FavoriteBorder, Icons.Outlined.StarBorder)

    NavigationBar {
        items.forEachIndexed { index, item ->
            NavigationBarItem(
                icon = {
                    Icon(
                        if (selectedItem == index) selectedIcons[index] else unselectedIcons[index],
                        contentDescription = item,
                    )
                },
                label = { Text(item) },
                selected = selectedItem == index,
                onClick = { selectedItem = index },
            )
        }
    }
}

// ---- BadgeSamples.kt ----

@Composable
fun NavigationBarItemWithBadge() {
    NavigationBar {
        NavigationBarItem(
            icon = {
                BadgedBox(
                    badge = {
                        Badge(
                            modifier =
                                Modifier.semantics { contentDescription = "New notification" }
                        )
                    }
                ) {
                    Icon(Icons.Filled.Star, contentDescription = "Favorite")
                }
            },
            selected = false,
            onClick = {},
        )
        NavigationBarItem(
            icon = {
                BadgedBox(
                    badge = {
                        Badge {
                            val badgeNumber = "8"
                            Text(
                                badgeNumber,
                                modifier =
                                    Modifier.semantics {
                                        contentDescription = "$badgeNumber new notifications"
                                    },
                            )
                        }
                    }
                ) {
                    Icon(Icons.Filled.Star, contentDescription = "Favorite")
                }
            },
            selected = false,
            onClick = {},
        )
        NavigationBarItem(
            icon = {
                BadgedBox(
                    badge = {
                        Badge {
                            val badgeNumber = "999+"
                            Text(
                                badgeNumber,
                                modifier =
                                    Modifier.semantics {
                                        contentDescription = "$badgeNumber new notifications"
                                    },
                            )
                        }
                    }
                ) {
                    Icon(Icons.Filled.Star, contentDescription = "Favorite")
                }
            },
            selected = false,
            onClick = {},
        )
    }
}
