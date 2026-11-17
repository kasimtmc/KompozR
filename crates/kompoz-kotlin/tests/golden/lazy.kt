// Lazy lists: `items(count)` and `items(list)` told apart by their argument, keys and content types of Kotlin's
// `Any` made LazyKey / SlotKey (a value, a nullable one, a lambda's result, a key lambda's `return@items`, an `if`'s
// branches), a list's element type given to the closures made trait objects (`key = { it.id }`), the element lent to
// a lambda taken as Kotlin's value (a clone the click handler keeps), a state list's element set by index,
// `overscrollEffect = null` as the `Some(None)` of a nullable parameter whose default isn't null, a
// TargetedFlingBehavior given as a FlingBehavior, an animation spec value given as its shared trait object.

// (`Todo(id, title, done)`: a Rust type the Kotlin calls; data classes come with stage 14)

@Composable
fun Todos(group: String?) {
    val todos = remember { mutableStateListOf(Todo(1, "Yaz", false), Todo(2, "Oku", false)) }
    val state = rememberLazyListState()
    LazyColumn(
        state = state,
        flingBehavior = rememberSnapFlingBehavior(lazyListState = state),
        overscrollEffect = null,
    ) {
        stickyHeader(key = group) { Text("başlık") }
        item(key = 7, contentType = "tek") { Text("bir") }
        items(3, key = { "satır $it" }, contentType = { if (it == 0) null else "satır" }) { i -> Text("$i") }
        items(todos, key = { if (it.done) return@items it.title; it.id }) { t ->
            Text(
                t.title,
                modifier = Modifier
                    .animateItem(placementSpec = tween(200))
                    .clickable {
                        val at = todos.indexOfFirst { it.id == t.id }
                        todos[at] = Todo(t.id, t.title, !t.done)
                    },
            )
        }
        itemsIndexed(todos) { index, t -> Text("$index ${t.title}") }
    }
}
