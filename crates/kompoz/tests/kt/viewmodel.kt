// ViewModels and saved state as a Compose app writes them: the C++ Kompoz's TodoScreen (kompozable_tests.cpp
// `classes_as_kotlin_writes_them`: its classes, a ViewModel's state flow and function, a lifecycle effect) and
// QueryViewModel's screen, lifecycle-viewmodel-compose 2.11's samples (LifecycleViewModelSamples.kt) and savedstate
// 1.4's (SavedStateCodecSamples.kt, SavedStateRegistryOwnerDelegatesSamples.kt).
//
// The codec samples' `@Serializable data class User` is local to each sample there: one top-level class here, as
// the functions give it back. Their `class UUIDSerializer : KSerializer<UUID>` needs kotlinx.serialization's
// KSerializer/Encoder/Decoder and java.util.UUID (KompozR's explicit serializer is a Saver: UUIDSerializer() is one,
// in kotlin_viewmodel.rs); SavedStateRegistryOwnerDelegatesSamples' `class MyActivity : ComponentActivity()` is
// Android's (stage 15: an Activity is the Android platform's).

package androidx.lifecycle.viewmodel.compose.samples

import androidx.compose.foundation.clickable
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.*
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
import kotlinx.serialization.Serializable

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
    return encodeToSavedState(UUIDSerializer(), "0f0e0d0c-0b0a-0908-0706-050403020100")
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

@Sampled
fun decodeWithExplicitSerializer(): String {
    val uuidSavedState = savedState { putString("", "0f0e0d0c-0b0a-0908-0706-050403020100") }
    val uuid = decodeFromSavedState(UUIDSerializer(), uuidSavedState)
    return uuid
}

// The rest of the API as an app writes it: a saved state read back (`read { }`), a handle's values set by index and
// read as a flow.
fun userName(state: SavedState): String = state.read { if (contains("name")) getString("name") else "?" }

fun setAndGet(handle: SavedStateHandle, query: String): String {
    handle["query"] = query
    val read: String? = handle.get("query")
    return read ?: ""
}
