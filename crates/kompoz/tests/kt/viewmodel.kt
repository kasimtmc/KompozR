// ViewModels and saved state as a Compose app writes them: the C++ Kompoz's TodoScreen (kompozable_tests.cpp
// `classes_as_kotlin_writes_them`: its classes, a ViewModel's state flow and function, a lifecycle effect) and
// QueryViewModel's screen, lifecycle-viewmodel-compose 2.11's samples (LifecycleViewModelSamples.kt) and savedstate
// 1.4's (SavedStateCodecSamples.kt, SavedStateRegistryOwnerDelegatesSamples.kt).
//
// The codec samples' `@Serializable data class User` is local to each sample there: one top-level class here, as
// the functions give it back (the samples return nothing: these return what they make, for the test).
// SavedStateRegistryOwnerDelegatesSamples' `class MyActivity : ComponentActivity()` is Android's (stage 15: an
// Activity is the Android platform's).

package androidx.lifecycle.viewmodel.compose.samples

import androidx.compose.foundation.clickable
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.ui.Modifier
import androidx.lifecycle.DEFAULT_ARGS_KEY
import androidx.lifecycle.HasDefaultViewModelProviderFactory
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewmodel.CreationExtras
import androidx.lifecycle.viewmodel.MutableCreationExtras
import androidx.lifecycle.viewmodel.compose.*
import androidx.savedstate.*
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.*
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import java.util.UUID

// ---- the C++ test's classes (kompozable_tests.cpp 2594-2609) ----

data class Todo(val id: Int, val title: String, val done: Boolean = false)

class TodoViewModel : ViewModel() {
    val items = MutableStateFlow(listOf(Todo(1, "Yaz"), Todo(2, "Gönder")))
    var toggles = 0

    fun toggle(id: Int, done: Boolean) {
        toggles++
        items.update { list -> list.map { if (it.id == id) it.copy(done = done) else it } }
    }

    override fun onCleared() {}
}

class Greeter(private val name: String, suffix: String, val punctuation: String = "!") {
    val greeting: String = "Merhaba, $name$suffix"   // (typed: it reads a plain parameter)
    var greeted = 0
    init { greeted = -1 }
    fun greet(): String {
        greeted++
        return greeting + punctuation
    }
    private fun hidden() = 1
}

fun ticks(): Flow<Int> = flow {
    for (i in 0 until 3) {
        emit(i)
        delay(10)
    }
}

class QueryViewModel(private val state: SavedStateHandle) : ViewModel() {
    val query: StateFlow<String> = state.getStateFlow("query", "")
    fun setQuery(q: String) { state["query"] = q }
}

class Player { var playing = false; fun play() { playing = true } fun pause() { playing = false } }

// ---- TodoScreen (kompozable_tests.cpp 2611-2623) ----

@Composable
fun TodoScreen(player: Player) {
    val vm = viewModel<TodoViewModel>()
    val items by vm.items.collectAsState()
    LazyColumn {
        items(items, key = { it.id }) { t ->
            Text(if (t.done) "✓ ${t.title}" else t.title, modifier = Modifier.clickable { vm.toggle(t.id, !t.done) })
        }
    }
    LifecycleResumeEffect(Unit) {
        player.play()
        onPauseOrDispose { player.pause() }
    }
}

// QueryViewModel's screen (kompozable_tests.cpp's class, saved_state_handles_outlive_the_process's app half): its
// constructor takes the handle, the default factory gives it one.
@Composable
fun SearchScreen() {
    val vm = viewModel<QueryViewModel>()
    val query by vm.query.collectAsState()
    Text("uygulama $query", modifier = Modifier.clickable { vm.setQuery(query + "!") })
}

// ---- LifecycleViewModelSamples.kt ----

@Sampled
@Composable
fun CreationExtrasViewModel() {
    val owner = LocalViewModelStoreOwner.current
    val defaultExtras =
        (owner as? HasDefaultViewModelProviderFactory)?.defaultViewModelCreationExtras
            ?: CreationExtras.Empty
    // Custom extras should always be added on top of the default extras
    val extras = MutableCreationExtras(defaultExtras)
    extras[DEFAULT_ARGS_KEY] = savedState { putString("test", "my_value") }
    // This factory is normally created separately and passed in
    val customFactory = remember {
        object : ViewModelProvider.Factory {
            override fun <T : ViewModel> create(modelClass: Class<T>, extras: CreationExtras): T {
                val args = extras[DEFAULT_ARGS_KEY]?.getString("test")
                @Suppress("UNCHECKED_CAST")
                // TestViewModel is a basic ViewModel that sets a String variable
                return TestViewModel(args) as T
            }
        }
    }
    // Create a ViewModel using the custom factory passing in the custom extras
    val viewModel = customFactory.create(TestViewModel::class.java, extras)
    // The value from the extras is now available in the ViewModel
    Text("args ${viewModel.args}")
}

@Sampled
@Composable
fun CreationExtrasViewModelInitializer() {
    // Just like any call to viewModel(), the default owner is the LocalViewModelStoreOwner.current.
    // The lambda is only called the first time the ViewModel needs to be created.
    val viewModel = viewModel {
        // Within the lambda, you have direct access to the CreationExtras which allows you to call
        // extension methods on CreationExtras such as createSavedStateHandle()
        val handle = createSavedStateHandle()
        // You can send any custom parameter, repository, etc. to your ViewModel.
        SavedStateViewModel(handle, "custom_value")
    }
    // The handle and parameter are now available from the ViewModel
    val handle: SavedStateHandle = viewModel.handle
    Text("${viewModel.value} ${handle.keys().size}")
}

class TestViewModel(val args: String?) : ViewModel()

class SavedStateViewModel(val handle: SavedStateHandle, val value: String) : ViewModel()

@Sampled
@Composable
fun RememberViewModelStoreOwnerSample(showDetail: Boolean) {
    if (showDetail) {
        // Creates a ViewModelStoreOwner scoped to this specific call site.
        // When showDetail becomes false, the owner leaves composition and ViewModels are cleared.
        val scopedOwner = rememberViewModelStoreOwner()

        CompositionLocalProvider(LocalViewModelStoreOwner provides scopedOwner) {
            val viewModel = viewModel { TestViewModel("detail_data") }
            // Use viewModel
            Text("${viewModel.args}")
        }
    }
}

@Sampled
@Composable
fun RememberViewModelStoreOwnerWithKeySample() {
    val storeProvider = rememberViewModelStoreProvider()
    val pages = listOf("Page 1", "Page 2", "Page 3")

    HorizontalPager(pageCount = pages.size) { page ->
        // Create a ViewModelStoreOwner for the specific page using the provider and a key.
        // This ensures each page gets its own isolated ViewModel instance.
        val pageOwner = rememberViewModelStoreOwner(provider = storeProvider, key = page)

        CompositionLocalProvider(LocalViewModelStoreOwner provides pageOwner) {
            val pageViewModel = viewModel { TestViewModel(pages[page]) }
            // Use pageViewModel
            Text("${pageViewModel.args}")
        }
    }
}

@Sampled
@Composable
fun RememberViewModelStoreProviderSample() {
    // Creates a provider unique to this feature branch using hashCode.
    val storeProvider = rememberViewModelStoreProvider()

    // Use the provider to get an owner (without key)
    val owner = rememberViewModelStoreOwner(provider = storeProvider)

    CompositionLocalProvider(LocalViewModelStoreOwner provides owner) {
        val viewModel = viewModel { TestViewModel("isolated_data") }
        Text("${viewModel.args}")
    }
}

@Sampled
@Composable
fun RememberViewModelStoreProviderWithKeySample() {
    val featureKey = "shared_feature_flow"

    // Creates a provider scoped to a custom key.
    // This allows sharing the same provider (and its stores) across different parts of the UI
    // that might not share the same parent composition node but share the same feature key.
    val storeProvider = rememberViewModelStoreProvider(key = featureKey)

    // Component A
    FeaturePartA(storeProvider)

    // Component B
    FeaturePartB(storeProvider)
}

// The samples' dummies ("Dummy implementation for sample compilation"), here with content: each page in turn, a
// feature part's ViewModel of the shared provider.
@Composable
private fun HorizontalPager(pageCount: Int, content: @Composable (Int) -> Unit) {
    for (page in 0 until pageCount) {
        content(page)
    }
}

@Composable
private fun FeaturePartA(provider: ViewModelStoreProvider) {
    val owner = rememberViewModelStoreOwner(provider = provider, key = "feature")
    CompositionLocalProvider(LocalViewModelStoreOwner provides owner) {
        val viewModel = viewModel { TestViewModel("shared") }
        Text("A ${viewModel.args}")
    }
}

@Composable
private fun FeaturePartB(provider: ViewModelStoreProvider) {
    val owner = rememberViewModelStoreOwner(provider = provider, key = "feature")
    CompositionLocalProvider(LocalViewModelStoreOwner provides owner) {
        val viewModel = viewModel { TestViewModel("other") }
        Text("B ${viewModel.args}")
    }
}

// ---- SavedStateCodecSamples.kt (savedstate 1.4) ----

@Serializable data class User(val id: Int, val name: String)

@Sampled
fun encode(): SavedState {
    val user = User(123, "foo")
    val savedState = encodeToSavedState(user)
    return savedState
}

@Sampled
fun encodeWithExplicitSerializer(): SavedState {
    class UUIDSerializer : KSerializer<UUID> {
        override val descriptor: SerialDescriptor
            get() = PrimitiveSerialDescriptor("UUIDSerializer", PrimitiveKind.STRING)

        override fun deserialize(decoder: Decoder): UUID {
            return UUID.fromString(decoder.decodeString())
        }

        override fun serialize(encoder: Encoder, value: UUID) {
            encoder.encodeString(value.toString())
        }
    }
    return encodeToSavedState(UUIDSerializer(), UUID.randomUUID())
}

// (a top-level `val` in the sample)
fun userSavedState(): SavedState = savedState {
    putInt("id", 123)
    putString("name", "foo")
}

@Sampled
fun decode(): User {
    val user = decodeFromSavedState<User>(userSavedState())
    return user
}

val uuidSavedState = savedState { putString("", UUID.randomUUID().toString()) }

@Sampled
fun decodeWithExplicitSerializer(): UUID {
    class UUIDSerializer : KSerializer<UUID> {
        override val descriptor: SerialDescriptor
            get() = PrimitiveSerialDescriptor("UUIDSerializer", PrimitiveKind.STRING)

        override fun deserialize(decoder: Decoder): UUID {
            return UUID.fromString(decoder.decodeString())
        }

        override fun serialize(encoder: Encoder, value: UUID) {
            encoder.encodeString(value.toString())
        }
    }
    val uuid = decodeFromSavedState(UUIDSerializer(), uuidSavedState)
    return uuid
}

// (what the sample's saved state holds, for the test)
fun uuidSavedText(): String = uuidSavedState.read { getString("") }

// The rest of the API as an app writes it: a saved state read back (`read { }`), a handle's values set by index and
// read as a flow.
fun userName(state: SavedState): String = state.read { if (contains("name")) getString("name") else "?" }

fun setAndGet(handle: SavedStateHandle, query: String): String {
    handle["query"] = query
    val read: String? = handle.get("query")
    return read ?: ""
}

// ---- LifecycleViewModelSamples.kt's SnapshotStateViewModel samples (lifecycle-viewmodel-compose 2.11) ----

@Sampled
fun SnapshotStateViewModel() {

    /** A simple item that is not inherently [Parcelable] */
    data class Item(val id: UUID, val value: String)

    @OptIn(SavedStateHandleSaveableApi::class)
    class SnapshotStateViewModel(handle: SavedStateHandle) : ViewModel() {

        /**
         * A snapshot-backed [MutableList] of a list of items, persisted by the [SavedStateHandle].
         * The size of this set must remain small in expectation, since the maximum size of saved
         * instance state space is limited.
         */
        private val items: MutableList<Item> =
            handle.saveable(
                key = "items",
                saver =
                    listSaver(
                        save = { it.map { item -> listOf(item.id.toString(), item.value) } },
                        restore = {
                            it.map { saved ->
                                    Item(id = UUID.fromString(saved[0]), value = saved[1])
                                }
                                .toMutableStateList()
                        },
                    ),
            ) {
                mutableStateListOf()
            }

        /**
         * A snapshot-backed [MutableMap] representing a set of selected item ids, persisted by the
         * [SavedStateHandle]. A [MutableSet] is approximated by ignoring the keys. The size of this
         * set must remain small in expectation, since the maximum size of saved instance state
         * space is limited.
         */
        private val selectedItemIds: MutableMap<UUID, Unit> =
            handle.saveable(
                key = "selectedItemIds",
                saver =
                    listSaver(
                        save = { it.keys.map(UUID::toString) },
                        restore = {
                            it.map(UUID::fromString).map { id -> id to Unit }.toMutableStateMap()
                        },
                    ),
            ) {
                mutableStateMapOf()
            }

        /**
         * A snapshot-backed flag representing where selections are enabled, persisted by the
         * [SavedStateHandle].
         */
        var areSelectionsEnabled by handle.saveable("areSelectionsEnabled") { mutableStateOf(true) }

        /** A list of items paired with a selection state. */
        val selectedItems: List<Pair<Item, Boolean>>
            get() = items.map { it to (it.id in selectedItemIds) }

        /** Updates the selection state for the item with [id] to [selected]. */
        fun selectItem(id: UUID, selected: Boolean) {
            if (selected) {
                selectedItemIds[id] = Unit
            } else {
                selectedItemIds.remove(id)
            }
        }

        /** Adds an item with the given [value]. */
        fun addItem(value: String) {
            items.add(Item(UUID.randomUUID(), value))
        }
    }
}

@Sampled
fun SnapshotStateViewModelWithDelegates() {

    /** A simple item that is not inherently [Parcelable] */
    data class Item(val id: UUID, val value: String)

    @OptIn(SavedStateHandleSaveableApi::class)
    class SnapshotStateViewModel(handle: SavedStateHandle) : ViewModel() {

        /**
         * A snapshot-backed [MutableList] of a list of items, persisted by the [SavedStateHandle].
         * The size of this set must remain small in expectation, since the maximum size of saved
         * instance state space is limited.
         */
        private val items: MutableList<Item> by
            handle.saveable(
                saver =
                    listSaver(
                        save = { it.map { item -> listOf(item.id.toString(), item.value) } },
                        restore = {
                            it.map { saved ->
                                    Item(id = UUID.fromString(saved[0]), value = saved[1])
                                }
                                .toMutableStateList()
                        },
                    )
            ) {
                mutableStateListOf()
            }

        /**
         * A snapshot-backed [MutableMap] representing a set of selected item ids, persisted by the
         * [SavedStateHandle]. A [MutableSet] is approximated by ignoring the keys. The size of this
         * set must remain small in expectation, since the maximum size of saved instance state
         * space is limited.
         */
        private val selectedItemIds: MutableMap<UUID, Unit> by
            handle.saveable(
                saver =
                    listSaver(
                        save = { it.keys.map(UUID::toString) },
                        restore = {
                            it.map(UUID::fromString).map { id -> id to Unit }.toMutableStateMap()
                        },
                    )
            ) {
                mutableStateMapOf()
            }

        /**
         * A snapshot-backed flag representing where selections are enabled, persisted by the
         * [SavedStateHandle].
         */
        var areSelectionsEnabled by handle.saveable { mutableStateOf(true) }

        /** A list of items paired with a selection state. */
        val selectedItems: List<Pair<Item, Boolean>>
            get() = items.map { it to (it.id in selectedItemIds) }

        /** Updates the selection state for the item with [id] to [selected]. */
        fun selectItem(id: UUID, selected: Boolean) {
            if (selected) {
                selectedItemIds[id] = Unit
            } else {
                selectedItemIds.remove(id)
            }
        }

        /** Adds an item with the given [value]. */
        fun addItem(value: String) {
            items.add(Item(UUID.randomUUID(), value))
        }
    }
}

// The samples' ViewModel as an app keeps it (theirs are local to the sample functions): a top-level one of the
// delegates sample's body, which the test makes, changes, and makes again from what its handle saved.
data class SelectionItem(val id: UUID, val value: String)

@OptIn(SavedStateHandleSaveableApi::class)
class SelectionViewModel(handle: SavedStateHandle) : ViewModel() {
    private val items: MutableList<SelectionItem> by
        handle.saveable(
            saver =
                listSaver(
                    save = { it.map { item -> listOf(item.id.toString(), item.value) } },
                    restore = {
                        it.map { saved -> SelectionItem(id = UUID.fromString(saved[0]), value = saved[1]) }
                            .toMutableStateList()
                    },
                )
        ) {
            mutableStateListOf()
        }

    private val selectedItemIds: MutableMap<UUID, Unit> by
        handle.saveable(
            saver =
                listSaver(
                    save = { it.keys.map(UUID::toString) },
                    restore = { it.map(UUID::fromString).map { id -> id to Unit }.toMutableStateMap() },
                )
        ) {
            mutableStateMapOf()
        }

    var areSelectionsEnabled by handle.saveable { mutableStateOf(true) }

    val selectedItems: List<Pair<SelectionItem, Boolean>>
        get() = items.map { it to (it.id in selectedItemIds) }

    fun selectItem(id: UUID, selected: Boolean) {
        if (selected) {
            selectedItemIds[id] = Unit
        } else {
            selectedItemIds.remove(id)
        }
    }

    fun addItem(value: String) {
        items.add(SelectionItem(UUID.randomUUID(), value))
    }

    fun report(): String =
        selectedItems.joinToString { (item, selected) -> "${item.value}=$selected" } + " $areSelectionsEnabled"
}

// Two items, the first selected, selections turned off: what the handle saved.
fun selectionsChanged(vm: SelectionViewModel): String {
    vm.addItem("elma")
    vm.addItem("armut")
    vm.selectItem(vm.selectedItems[0].first.id, true)
    vm.areSelectionsEnabled = false
    return vm.report()
}
