// navigation3-runtime 1.2's samples as Compose writes them: NavBackStackSamples.kt (open and closed polymorphism),
// NavBackStackSerializerSamples.kt, RememberNavBackStackSamples.kt, and the deep link ones (DeepLinkMatcherSample.kt,
// DeepLinkNonPrimitiveArgSample.kt). Checked by kotlin_navigation3.rs (its module `runtime_samples`: the deep link
// samples' package has classes of the ui samples' names).
//
// What a sample computes and drops (`val decoded = …`) is returned too, for the test, and a deep link sample's request
// URI is a parameter (each such line marked). The samples' files are one here: the nested classes of their objects of
// one name (`Home` of NavBackStackSamples, of NavBackStackSerializerSamples, …) written qualified, not imported.
// The withReflection forms run as on Android: KompozR's classes of the keys seen in this process are its reflection.
//
// Not here yet: CommonUiNavDisplaySample.kt and ListDetailNavDisplaySample.kt — generic composables (`fun <T : Any>
// CommonUiNavDisplay(backstack: List<T>, …)`) over the API's generic types (the stage-14 form kt/navigation3.kt's
// header names), their entries' metadata read by its texts (`entry.metadata[TRANSITION_SPEC] as? ContentTransform`);
// ListDetailNavDisplay's `calculateWindowSizeClass(LocalActivity.current!!)` is Android's (stage 15).

package androidx.navigation3.runtime.samples

import androidx.compose.runtime.Composable
import androidx.compose.runtime.saveable.rememberSerializable
import androidx.navigation3.runtime.NavBackStack
import androidx.navigation3.runtime.NavKey
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.runtime.serialization.NavBackStackSerializer
import androidx.navigation3.runtime.deeplink.*
import androidx.savedstate.serialization.SavedStateConfiguration
import androidx.savedstate.serialization.decodeFromSavedState
import androidx.savedstate.serialization.encodeToSavedState
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.modules.SerializersModule
import kotlinx.serialization.modules.polymorphic
import kotlinx.serialization.modules.subclass
import kotlinx.serialization.serializer

// ---- NavBackStackSamples.kt ----

object NavBackStackSamples {

    // --- Open Polymorphism ---
    @Serializable open class Home : NavKey

    @Serializable open class Chat : NavKey

    @Serializable open class Spaces : NavKey

    // --- Closed Polymorphism ---
    @Serializable
    sealed class SealedKey : NavKey {

        @Serializable class Inbox : SealedKey()

        @Serializable class Settings : SealedKey()
    }
}

@Composable
fun NavBackStack_OpenPolymorphism(): NavBackStack<NavKey> { // (returned: for the test)
    // https://github.com/Kotlin/kotlinx.serialization/blob/master/docs/polymorphism.md#open-polymorphism
    return rememberSerializable(
        serializer = serializer(),
        configuration =
            SavedStateConfiguration {
                serializersModule = SerializersModule {
                    polymorphic(baseClass = NavKey::class) {
                        subclass(clazz = NavBackStackSamples.Home::class)
                        subclass(clazz = NavBackStackSamples.Chat::class)
                        subclass(clazz = NavBackStackSamples.Spaces::class)
                    }
                }
            },
    ) {
        NavBackStack<NavKey>()
    }
}

@Composable
fun NavBackStack_ClosedPolymorphism(): NavBackStack<NavBackStackSamples.SealedKey> { // (returned: for the test)
    // https://github.com/Kotlin/kotlinx.serialization/blob/master/docs/polymorphism.md#closed-polymorphism
    return rememberSerializable(serializer = serializer()) { NavBackStack<NavBackStackSamples.SealedKey>() }
}

// ---- NavBackStackSerializerSamples.kt ----

private object NavBackStackSerializerSamples {

    @Serializable open class Home(val id: String) : NavKey

    @Serializable open class Details(val itemId: Long) : NavKey
}

@Composable
fun NavBackStackSerializer_withReflection(): NavBackStack<NavKey> { // (returned: for the test)
    // On Android, the no-argument overload uses reflection and requires no configuration.
    // This will throw a runtime exception on non-Android platforms during serialization.
    val serializer = NavBackStackSerializer<NavKey>()

    val backStack = NavBackStack(NavBackStackSerializerSamples.Home("abc"), NavBackStackSerializerSamples.Details(42))
    val encoded = encodeToSavedState(serializer, backStack)
    val decoded = decodeFromSavedState(serializer, encoded)
    return decoded // (for the test)
}

@Composable
fun NavBackStackSerializer_withSerializersModule(): NavBackStack<NavKey> { // (returned: for the test)
    val module = SerializersModule {
        polymorphic(NavKey::class) {
            subclass(NavBackStackSerializerSamples.Home.serializer())
            subclass(NavBackStackSerializerSamples.Details.serializer())
        }
    }
    val configuration = SavedStateConfiguration { serializersModule = module }

    val serializer = NavBackStackSerializer<NavKey>()

    // Pass the same configuration (or at least its serializersModule) to encode/decode:
    val backStack = NavBackStack(NavBackStackSerializerSamples.Home("abc"), NavBackStackSerializerSamples.Details(42))
    val encoded = encodeToSavedState(serializer, backStack, configuration)
    val decoded = decodeFromSavedState(serializer, encoded, configuration)
    return decoded // (for the test)
}

// ---- RememberNavBackStackSamples.kt ----

private object RememberNavBackStackSamples {

    @Serializable open class Home(val id: String) : NavKey

    @Serializable open class Details(val itemId: Long) : NavKey
}

@Composable
fun rememberNavBackStack_withReflection(): NavBackStack<NavKey> { // (returned: for the test)
    // Works on Android (uses reflection internally).
    return rememberNavBackStack(RememberNavBackStackSamples.Home("start"), RememberNavBackStackSamples.Details(42))
}

@Composable
fun rememberNavBackStack_withSerializersModule(): NavBackStack<NavKey> { // (returned: for the test)
    val config = SavedStateConfiguration {
        // Register subtypes for open polymorphism or multiplatform use.
        serializersModule = SerializersModule {
            polymorphic(baseClass = NavKey::class) {
                subclass(serializer = RememberNavBackStackSamples.Home.serializer())
                subclass(serializer = RememberNavBackStackSamples.Details.serializer())
            }
        }
    }

    // Pass the configuration so encoding/decoding works consistently.
    return rememberNavBackStack(config, RememberNavBackStackSamples.Home("start"))
}

// ---- deeplink/DeepLinkMatcherSample.kt ----

@Serializable object UserListKey : NavKey

@Serializable object ImageKey : NavKey

@Serializable data class Article(val id: Int) : NavKey

@Serializable data class ArticleSection(val category: String) : NavKey

private const val SAMPLE_BASE_PATH = "www.nav3deeplinksample.com"

fun staticKeyDeepLinkMatcherSample(): DeepLinkMatcher.MatchResult<ImageKey>? { // (returned: for the test)
    // declare a matcher based on mime type
    val mimeType = "image/jpg"
    val mimeTypeFilter = DeepLinkMatcher.mimeTypeFilter(mimeType)
    val matcher = StaticKeyDeepLinkMatcher(ImageKey, listOf(mimeTypeFilter))

    // when handling a request
    val request =
        DeepLinkRequest(
            uri = DeepLinkUri("$SAMPLE_BASE_PATH/images/"),
            extras = DeepLinkRequest.mimeTypeExtra(mimeType),
        )
    // returns a valid result as long as mimeType matches
    val matchResult: DeepLinkMatcher.MatchResult<ImageKey>? = matcher.match(request)
    return matchResult // (for the test)
}

// (the request's uri a parameter, for the test: the sample's has a trailing '/' its pattern hasn't — as Kotlin's,
// it doesn't match)
fun deepLinkMatcherWithBackStackSample(uri: String): BackStackMatchResult<UserListKey, NavKey>? {
    val matcher =
        UriDeepLinkMatcher(
                uriPattern = DeepLinkUri("https://www.nav3deeplinksample.com/userlist"),
                serializer = serializer<UserListKey>(),
            )
            .withBackStack { matchResult -> listOf(HomeKey, matchResult.key) }

    // handling a request
    val request = DeepLinkRequest(uri = DeepLinkUri(uri))
    // returns a BackStackMatchResult containing the matched key and the constructed back stack
    val matchResult: BackStackMatchResult<UserListKey, NavKey>? = matcher.match(request)
    // listOf(HomeKey, UserListKey)
    val backStack: List<NavKey>? = matchResult?.backStack
    val key: UserListKey? = matchResult?.key
    return matchResult // (for the test)
}

// (the request's uri a parameter, for the test)
fun deepLinkMatcherWithConditionalBackStackSample(uri: String): BackStackMatchResult<Article, NavKey>? {
    val matcher =
        UriDeepLinkMatcher(
                uriPattern = DeepLinkUri("https://www.nav3deeplinksample.com/articles/{id}"),
                serializer = serializer<Article>(),
            )
            .withBackStack { matchResult ->
                val article = matchResult.key
                val parent =
                    if (article.id >= 10) ArticleSection("Shipping") else ArticleSection("Returns")
                listOf(parent, article)
            }

    // handling a request
    val request = DeepLinkRequest(uri = DeepLinkUri(uri))

    // returns a BackStackMatchResult containing the Article key and parent key based on article id
    val matchResult: BackStackMatchResult<Article, NavKey>? = matcher.match(request)
    // listOf(parent, article)
    val backStack: List<NavKey>? = matchResult?.backStack
    val key: Article? = matchResult?.key
    return matchResult // (for the test)
}

// ---- deeplink/DeepLinkNonPrimitiveArgSample.kt ----

// (the request's uri a parameter, for the test)
fun deepLinkSerializerSample(uri: String): NavKey {
    val matcher =
        UriDeepLinkMatcher(
            uriPattern =
                DeepLinkUri("http://www.nav3example.com/products?product={product}&count={count}"),
            serializer = serializer<ProductInventoryKey>(),
        )

    val request =
        DeepLinkRequest(
            uri = DeepLinkUri(uri)
        )
    val startKey = matcher.match(request)?.key ?: HomeKey
    return startKey // (for the test)
}

@Serializable object HomeKey : NavKey

private data class Product(val id: Int, val name: String)

@Serializable
private data class ProductInventoryKey(
    @Serializable(with = ProductSerializer::class) val product: Product,
    val count: Int,
) : NavKey

// serializer to deserialize string value into a Product object
private object ProductSerializer : DeepLinkSerializer<Product>() {
    override val serialName: String = "androidx.navigation3.ui.samples.Product"

    override fun deserialize(value: String): Product {
        val splitArgs = value.split("-", limit = 2)
        val id =
            splitArgs[0].toIntOrNull()
                ?: throw SerializationException("Invalid product id: ${splitArgs[0]}")
        val name = splitArgs[1]
        return Product(id, name)
    }

    override fun serialize(value: Product): String = "${value.id}-${value.name}"
}
