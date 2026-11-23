// Navigation's core as Compose writes it (stage 13 WP2): typed routes' toRoute<T>() and hasRoute<T>(),
// NavDestination.hierarchy, navOptions { popUpTo<T> { saveState = true } }, navDeepLink { } and navDeepLink<T>(basePath),
// an entry's arguments?.getInt("n"), navigate with options and popBackStack<T>.
@Kompozable
fun NavForms(nav: NavController) {
    val destination = nav.currentDestination
    val inAccount = destination?.hierarchy?.any { it.hasRoute<AccountGraph>() } == true
    val tabs = navOptions {
        popUpTo<Shell> { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
    val link = navDeepLink { uriPattern = "kompoz://a/{n}" }
    val member = navDeepLink<Member>("kompoz://member")
    val n = nav.currentBackStackEntry?.arguments?.getInt("n") ?: 0
    val entry = nav.currentBackStackEntry
        val detail = if (entry?.destination?.hasRoute<Detail>() == true) entry?.toRoute<Detail>() else null
    Column {
        Text("$inAccount $n ${detail?.id ?: -1} ${link.uriPattern} ${member.uriPattern}")
        Button(onClick = { nav.navigate(Shell, tabs) }) { Text("kabuk") }
        Button(onClick = {
            nav.navigate(Detail(id = 7, tab = "b")) {
                popUpTo<Start>()
                launchSingleTop = true
            }
        }) { Text("ayrıntı") }
        Button(onClick = { nav.popBackStack<AccountGraph>(inclusive = true) }) { Text("geri") }
        Button(onClick = { nav.navigate("a/3") { popUpTo("grup") { inclusive = true } } }) { Text("a") }
    }
}
