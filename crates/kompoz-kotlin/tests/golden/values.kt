// Top-level vals of their values' types (one read before it is declared, generics told by a lambda's value or a type
// argument), composition locals as statics (a const fn's), a camelCase val in snake_case, and a name whose
// declaration isn't seen here (another file's val, Rust's thread-local) read as Kotlin reads it.

val Greeting = "selam " + Mark

private val Mark = "!"

val Steps = 3 * 2

val Purple = Color(0xFF6650A4)

val LocalSpacing = staticCompositionLocalOf { Spacing() }

val LocalAccount = compositionLocalOf<Account> { error("not provided") }

val defaultLabel = "etiket"

@Composable
fun Locals() {
    Text("$Greeting ${LocalSpacing.current} $OtherFilesVal")
    CompositionLocalProvider(LocalAccount provides Account("Ada")) {
        Text(LocalAccount.current.name + defaultLabel)
    }
    val saved by rememberSaveable(stateSaver = townSaver) { mutableStateOf(Town("İzmir")) }
    Text(saved.name)
    // (another file's local: read lent, provided in place)
    CompositionLocalProvider(LocalOtherFiles provides "verilen") {
        Text(LocalOtherFiles.current)
    }
}
