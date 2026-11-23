//! Navigation 3's deep links (navigation3-runtime 1.2's `androidx.navigation3.runtime.deeplink`), as Compose has them:
//! the non-Android `DeepLinkUri` (ActualUri, InternalUri, UriCodec), `UriDeepLinkMatcher` and each pattern of its
//! KDoc, its decoder of keys (defaults, nullables, enums, lists, nested classes, `DeepLinkSerializer`s), the order of
//! matches, `StaticKeyDeepLinkMatcher` and its mime type filter, `withBackStack`, `DeepLinkRequest` and
//! `RequestExtras`; and the deep link samples (navigation3-runtime-samples' DeepLinkMatcherSample.kt and
//! DeepLinkNonPrimitiveArgSample.kt, navigation3-ui-samples' DeepLinkNonPrimitiveArgSamples.kt), their classes'
//! Rust twins asserting the keys they make.

use kompoz_navigation3::runtime::NavKey;
use kompoz_navigation3::runtime::deeplink::{
    DeepLinkFilter, DeepLinkMatchResult, DeepLinkMatcher, DeepLinkRequest, DeepLinkSerializer, DeepLinkUri,
    MatchResultOf, MimeTypeExtrasKey, RequestExtras, RequestExtrasKey, StaticKeyDeepLinkMatcher, UriDeepLinkMatcher,
    UriMatchResult, empty_request_extras, mime_type_filter, request_extras, serializable_with,
};
use kompoz_runtime::kt::{self, KtMap, KtMapExt, panic_kt};
use kompoz_runtime::{Saveable, SavedValue, SerialDescriptor, SerialKind, serializer};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

// ---- Helpers ----

/// Kotlin's `UriDeepLinkMatcher(DeepLinkUri(pattern), serializer<T>())`.
fn matcher<T: Saveable + 'static>(pattern: &str) -> UriDeepLinkMatcher<T> {
    UriDeepLinkMatcher::new(DeepLinkUri(pattern), serializer::<T>(), Vec::new())
}

/// The match of `uri` by a matcher of `pattern`.
fn match_of<T: Saveable + 'static>(pattern: &str, uri: &str) -> Option<UriMatchResult<T>> {
    matcher::<T>(pattern).r#match(&DeepLinkRequest::of_uri(DeepLinkUri(uri)))
}

/// The key of `uri`'s match by a matcher of `pattern`.
fn key_of<T: Saveable + Clone + 'static>(pattern: &str, uri: &str) -> Option<T> {
    match_of::<T>(pattern, uri).map(|r| r.key().clone())
}

/// A match's arguments, in order.
fn arguments_of<T>(result: &UriMatchResult<T>) -> Vec<(String, Vec<String>)> {
    result.arguments().as_slice().to_vec()
}

fn args(entries: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    entries.iter().map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect())).collect()
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// The message `f` panics with (Kotlin's exception: `Kind: message`).
fn panic_message(f: impl FnOnce()) -> String {
    let e = catch_unwind(AssertUnwindSafe(f)).expect_err("a panic");
    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
}

// ---- Key types ----

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Users;

#[derive(Saveable, Clone, PartialEq, Debug)]
struct UserId {
    id: String,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct FirstLast {
    first: String,
    last: String,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct NameKey {
    name: String,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct RawQuery {
    #[saveable(name = "rawQuery")]
    raw_query: Vec<String>,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct IntList {
    list: Vec<i32>,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct TypeKey {
    r#type: String,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct NameOther {
    name: String,
    other: String,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Flag {
    #[saveable(default)]
    debug: String,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct BoolFlag {
    debug: bool,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Ages {
    name: String,
    age: i32,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct AgesWithDefault {
    #[saveable(default)]
    name: String,
    #[saveable(default)]
    age: i32,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Nullables {
    #[saveable(default)]
    nick: Option<String>,
    age: Option<i32>,
}

#[derive(Saveable, Clone, Copy, PartialEq, Debug, Default)]
enum Color {
    #[default]
    Red,
    Green,
    #[saveable(name = "DEEP_BLUE")]
    Blue,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Paint {
    color: Color,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct PaintWithDefault {
    #[saveable(default)]
    color: Color,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Lists {
    ids: Vec<i64>,
    #[saveable(default)]
    names: Vec<String>,
    #[saveable(default)]
    maybe: Vec<Option<i32>>,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Primitives {
    b: bool,
    by: i8,
    s: i16,
    i: i32,
    l: i64,
    f: f32,
    d: f64,
    c: char,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct User {
    name: String,
    age: i32,
}

/// UriDeepLinkMatcher's KDoc: `data class Profile(user: User, id: Int)`.
#[derive(Saveable, Clone, PartialEq, Debug)]
struct Profile {
    user: User,
    id: i32,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct IdUser {
    id: i32,
    name: String,
}

/// A name two classes' elements have, of the same type: one value for both.
#[derive(Saveable, Clone, PartialEq, Debug)]
struct IdProfile {
    user: IdUser,
    id: i32,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct MaybeProfile {
    user: Option<User>,
    id: i32,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct UserList {
    users: Vec<User>,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Pair(i32, i32);

#[derive(Saveable, Clone, PartialEq, Debug)]
struct PairKey {
    pair: Pair,
}

/// A map of texts (kotlinx's `MAP` kind): what the decoder refuses.
#[derive(Clone, PartialEq, Debug)]
struct Texts;

impl Saveable for Texts {
    fn descriptor() -> SerialDescriptor {
        SerialDescriptor::new("kotlin.collections.LinkedHashMap", SerialKind::Map)
    }
    fn save(&self) -> SavedValue {
        SavedValue::map::<&str>([])
    }
    fn restore(_: &SavedValue) -> Option<Self> {
        Some(Texts)
    }
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct MapKey {
    texts: Texts,
}

/// Every argument optional: matches of different patterns, of one key type, compared.
#[derive(Saveable, Clone, PartialEq, Debug)]
struct Loose {
    #[saveable(default)]
    id: Option<String>,
    #[saveable(default)]
    a: Option<String>,
    #[saveable(default)]
    b: Option<String>,
    #[saveable(default)]
    name: Option<String>,
}

// ---- DeepLinkUri (nonAndroidMain's ActualUri, InternalUri, UriCodec) ----

fn uri_parts_are_read_as_the_non_android_uri_reads_them() {
    let uri = DeepLinkUri("https://bob@www.example.com:8080/a/b%20c/?q=1&r=x%26y&flag&q=2#frag%20ment");
    assert_eq!(uri.get_scheme(), Some("https"));
    assert_eq!(uri.get_authority(), Some("bob@www.example.com:8080"));
    // (split on '/' as it is: the leading empty segment kept, and a trailing one)
    assert_eq!(uri.get_path_segments(), strings(&["", "a", "b c", ""]));
    assert_eq!(uri.get_query(), Some("q=1&r=x%26y&flag&q=2"));
    assert_eq!(uri.get_query_parameters("q"), strings(&["1", "2"]));
    assert_eq!(uri.get_query_parameters("r"), strings(&["x&y"]));
    assert_eq!(uri.get_query_parameters("flag"), strings(&[""]));
    assert!(uri.get_query_parameters("none").is_empty());
    assert_eq!(uri.get_query_parameter_names().as_slice(), strings(&["q", "r", "flag"]));
    // (the fragment as it is: not decoded)
    assert_eq!(uri.get_fragment(), Some("frag%20ment"));
    assert!(uri.is_hierarchical());
    assert_eq!(uri.to_string(), "https://bob@www.example.com:8080/a/b%20c/?q=1&r=x%26y&flag&q=2#frag%20ment");
    assert_eq!(
        format!("{uri:?}"),
        "DeepLinkUri(\"https://bob@www.example.com:8080/a/b%20c/?q=1&r=x%26y&flag&q=2#frag%20ment\")"
    );
    assert_eq!(
        DeepLinkUri::new("http://a.com/b").get_path_segments(),
        DeepLinkUri("http://a.com/b").get_path_segments()
    );

    // No path: one empty segment; no query, no fragment.
    let bare = DeepLinkUri("http://a.com");
    assert_eq!(bare.get_path_segments(), strings(&[""]));
    assert_eq!(bare.get_query(), None);
    assert_eq!(bare.get_fragment(), None);
    assert!(bare.get_query_parameter_names().is_empty());
    // A '\' ends the authority (the URL standard's host state).
    assert_eq!(DeepLinkUri("http://a.com\\b").get_authority(), Some("a.com"));
    // An empty fragment is none; an empty query is one.
    assert_eq!(DeepLinkUri("http://a.com/#").get_fragment(), None);
    assert_eq!(DeepLinkUri("http://a.com/?").get_query(), Some(""));
    // A '?' after the '#' is the fragment's.
    let fragment_first = DeepLinkUri("http://a.com/b#x?y");
    assert_eq!(fragment_first.get_query(), None);
    assert_eq!(fragment_first.get_fragment(), Some("x?y"));
    assert_eq!(fragment_first.get_path_segments(), strings(&["", "b"]));
}

fn relative_and_opaque_uris_are_as_androids() {
    // Relative: no scheme, no authority without "//", hierarchical, the path from the start.
    let relative = DeepLinkUri("www.example.com/users?x=1");
    assert_eq!(relative.get_scheme(), None);
    assert_eq!(relative.get_authority(), None);
    assert!(relative.is_hierarchical());
    assert_eq!(relative.get_path_segments(), strings(&["www.example.com", "users"]));
    assert_eq!(relative.get_query_parameters("x"), strings(&["1"]));
    let network = DeepLinkUri("//www.example.com/users");
    assert_eq!(network.get_scheme(), None);
    assert_eq!(network.get_authority(), Some("www.example.com"));
    assert_eq!(network.get_path_segments(), strings(&["", "users"]));

    // Opaque: its scheme-specific part not starting with '/'.
    let opaque = DeepLinkUri("mailto:nobody@example.com?subject=hi");
    assert_eq!(opaque.get_scheme(), Some("mailto"));
    assert!(!opaque.is_hierarchical());
    assert!(opaque.get_path_segments().is_empty());
    assert_eq!(opaque.get_authority(), None);
    assert_eq!(
        panic_message(|| {
            opaque.get_query_parameters("subject");
        }),
        "IllegalArgumentException: Failed requirement."
    );
    assert_eq!(
        panic_message(|| {
            opaque.get_query_parameter_names();
        }),
        "IllegalArgumentException: Failed requirement."
    );
    let no_ssp = DeepLinkUri("mailto:");
    assert!(!no_ssp.is_hierarchical());
    assert!(no_ssp.get_path_segments().is_empty());
}

fn escapes_are_decoded_as_uri_codec_decodes_them() {
    // UTF-8 octets; an invalid octet, a bad digit, a cut escape: U+FFFD (after a bad digit, the octet read so far).
    let uri = DeepLinkUri("http://a.com/%E2%82%AC/%c3%a7ay/%zz/%C3/a%2/a+b/%");
    assert_eq!(
        uri.get_path_segments(),
        strings(&["", "€", "çay", "\u{FFFD}\u{0}z", "\u{FFFD}", "a\u{FFFD}", "a+b", "\u{FFFD}"])
    );
    // Query keys are encoded to be found (letters, digits and "_-!.~'()*" as they are), values and names decoded,
    // `+` kept.
    let query = DeepLinkUri("http://a.com/?a%20b=1&%7Bx%7D=%7By%7D&p=a+b&t=_-!.~'()*&%E2%82%AC=euro&a b=raw");
    assert_eq!(query.get_query_parameters("a b"), strings(&["1"]));
    assert_eq!(query.get_query_parameters("{x}"), strings(&["{y}"]));
    assert_eq!(query.get_query_parameters("p"), strings(&["a+b"]));
    assert_eq!(query.get_query_parameters("t"), strings(&["_-!.~'()*"]));
    assert_eq!(query.get_query_parameters("€"), strings(&["euro"]));
    assert_eq!(query.get_query_parameter_names().as_slice(), strings(&["a b", "{x}", "p", "t", "€"]));
    // (a name without '=' is kept as it is, not decoded)
    let flags = DeepLinkUri("http://a.com/?%7Bx%7D&y");
    assert_eq!(flags.get_query_parameter_names().as_slice(), strings(&["%7Bx%7D", "y"]));
    assert_eq!(flags.get_query_parameters("{x}"), strings(&[""]));
}

// ---- UriDeepLinkMatcher: its KDoc's patterns ----

fn path_patterns_take_their_arguments() {
    // 1. An exact path.
    let exact = match_of::<Users>("www.example.com/users", "https://www.example.com/users").unwrap();
    assert_eq!(exact.key(), &Users);
    assert!(exact.arguments().is_empty());
    assert!(exact.is_exact_path());
    assert!(match_of::<Users>("www.example.com/users", "https://www.example.com/users/1").is_none());
    assert!(match_of::<Users>("www.example.com/users", "https://www.example.com/other").is_none());
    assert!(match_of::<Users>("www.example.com/users", "https://www.example.com").is_none());
    // (any query or fragment after it)
    assert!(match_of::<Users>("www.example.com/users", "https://www.example.com/users?x=1#y").is_some());

    // 2. A placeholder.
    let single = match_of::<UserId>("www.example.com/users/{id}", "https://www.example.com/users/123").unwrap();
    assert_eq!(single.key(), &UserId { id: "123".into() });
    assert_eq!(arguments_of(&single), args(&[("id", &["123"])]));
    assert!(!single.is_exact_path());
    assert_eq!(
        key_of::<UserId>("www.example.com/users/{id}", "https://www.example.com/users/a%20b%2Fc"),
        Some(UserId { id: "a b/c".into() })
    );
    assert_eq!(key_of::<UserId>("www.example.com/users/{id}", "https://www.example.com/users/123/456"), None);

    // 3. Placeholders in one segment.
    let multiple =
        match_of::<FirstLast>("www.example.com/users/{first}-{last}", "https://www.example.com/users/john-doe")
            .unwrap();
    assert_eq!(multiple.key(), &FirstLast { first: "john".into(), last: "doe".into() });
    assert_eq!(arguments_of(&multiple), args(&[("first", &["john"]), ("last", &["doe"])]));
    // (the first one lazily)
    assert_eq!(
        key_of::<FirstLast>("www.example.com/users/{first}-{last}", "https://www.example.com/users/mary-jane-doe"),
        Some(FirstLast { first: "mary".into(), last: "jane-doe".into() })
    );
    assert_eq!(
        key_of::<FirstLast>("www.example.com/users/{first}-{last}", "https://www.example.com/users/johndoe"),
        None
    );

    // 4. A wildcard: no arguments, not an exact path.
    let wildcard = match_of::<Users>("www.example.com/users/.*", "https://www.example.com/users/123/456").unwrap();
    assert!(wildcard.arguments().is_empty());
    assert!(!wildcard.is_exact_path());
    assert!(match_of::<Users>("www.example.com/users/.*", "https://www.example.com/people/123").is_none());

    // 5. A literal and a placeholder.
    let mixed =
        match_of::<UserId>("www.example.com/users/user_{id}", "https://www.example.com/users/user_123").unwrap();
    assert_eq!(mixed.key(), &UserId { id: "123".into() });
    assert_eq!(key_of::<UserId>("www.example.com/users/user_{id}", "https://www.example.com/users/123"), None);
    assert_eq!(
        key_of::<UserId>("www.example.com/users/user_{id}_profile", "https://www.example.com/users/user_7_profile"),
        Some(UserId { id: "7".into() })
    );

    // 6. An empty text.
    let empty =
        match_of::<UserId>("www.example.com/users/{id}/profile", "https://www.example.com/users//profile").unwrap();
    assert_eq!(empty.key(), &UserId { id: String::new() });
    assert_eq!(arguments_of(&empty), args(&[("id", &[""])]));

    // A trailing '/' in the pattern is required of the request, and none without it.
    assert!(match_of::<Users>("www.example.com/users/", "https://www.example.com/users/").is_some());
    assert!(match_of::<Users>("www.example.com/users/", "https://www.example.com/users").is_none());
    assert!(match_of::<Users>("www.example.com/users", "https://www.example.com/users/").is_none());
}

fn query_patterns_take_their_arguments() {
    // 1. A named parameter.
    let named =
        match_of::<NameKey>("www.example.com/users?name={name}", "https://www.example.com/users?name=john").unwrap();
    assert_eq!(named.key(), &NameKey { name: "john".into() });
    assert_eq!(arguments_of(&named), args(&[("name", &["john"])]));
    // (a path without arguments is exact, whatever the query)
    assert!(named.is_exact_path());
    assert_eq!(
        key_of::<NameKey>("www.example.com/users?name={name}", "https://www.example.com/users?name=a%20b%26c"),
        Some(NameKey { name: "a b&c".into() })
    );
    // (a required argument missing: no match)
    assert_eq!(key_of::<NameKey>("www.example.com/users?name={name}", "https://www.example.com/users"), None);
    // (parameter names are case-sensitive)
    assert_eq!(key_of::<NameKey>("www.example.com/users?name={name}", "https://www.example.com/users?Name=j"), None);

    // 2. An unnamed parameter: the parts without a '='.
    let unnamed =
        match_of::<RawQuery>("www.example.com/users?{rawQuery}", "https://www.example.com/users?anything&else")
            .unwrap();
    assert_eq!(unnamed.key(), &RawQuery { raw_query: strings(&["anything", "else"]) });
    assert_eq!(arguments_of(&unnamed), args(&[("rawQuery", &["anything", "else"])]));
    assert_eq!(
        key_of::<RawQuery>("www.example.com/users?{rawQuery}", "https://www.example.com/users?a&b=1&c"),
        Some(RawQuery { raw_query: strings(&["a", "c"]) })
    );

    // 3. A partial literal.
    assert_eq!(
        key_of::<UserId>("www.example.com/users?type=user_{id}", "https://www.example.com/users?type=user_123"),
        Some(UserId { id: "123".into() })
    );
    assert_eq!(
        key_of::<UserId>("www.example.com/users?type=user_{id}", "https://www.example.com/users?type=admin_1"),
        None
    );
    // (values are matched case-sensitively)
    assert_eq!(
        key_of::<UserId>("www.example.com/users?type=user_{id}", "https://www.example.com/users?type=USER_1"),
        None
    );

    // 4. Placeholders in one parameter.
    let multiple = match_of::<FirstLast>(
        "www.example.com/users?name={first}_{last}",
        "https://www.example.com/users?name=john_doe",
    )
    .unwrap();
    assert_eq!(multiple.key(), &FirstLast { first: "john".into(), last: "doe".into() });
    assert_eq!(arguments_of(&multiple), args(&[("first", &["john"]), ("last", &["doe"])]));

    // 5. A collection.
    let collection =
        match_of::<IntList>("www.example.com/users?list={list}", "https://www.example.com/users?list=10&list=20")
            .unwrap();
    assert_eq!(collection.key(), &IntList { list: vec![10, 20] });
    assert_eq!(arguments_of(&collection), args(&[("list", &["10", "20"])]));

    // 6. A wildcard: the parameter's name its argument's.
    let wildcard =
        match_of::<TypeKey>("www.example.com/users?type=user_.*", "https://www.example.com/users?type=user_admin")
            .unwrap();
    assert_eq!(wildcard.key(), &TypeKey { r#type: "admin".into() });
    assert_eq!(arguments_of(&wildcard), args(&[("type", &["admin"])]));

    // 7. Named and unnamed.
    let both = match_of::<NameOther>(
        "www.example.com/users?user={name}&{other}",
        "https://www.example.com/users?user=john&something",
    )
    .unwrap();
    assert_eq!(both.key(), &NameOther { name: "john".into(), other: "something".into() });
    assert_eq!(arguments_of(&both), args(&[("name", &["john"]), ("other", &["something"])]));

    // Parameters the pattern doesn't name are left aside.
    let extra =
        match_of::<NameKey>("www.example.com/users?name={name}", "https://www.example.com/users?name=john&city=x")
            .unwrap();
    assert_eq!(extra.key(), &NameKey { name: "john".into() });
    assert_eq!(arguments_of(&extra), args(&[("name", &["john"])]));

    // A flag: its name its value when present.
    assert_eq!(
        key_of::<Flag>("www.example.com/users?debug", "https://www.example.com/users?debug"),
        Some(Flag { debug: "debug".into() })
    );
    assert_eq!(
        key_of::<Flag>("www.example.com/users?debug", "https://www.example.com/users"),
        Some(Flag { debug: String::new() })
    );
    assert_eq!(
        key_of::<Flag>("www.example.com/users?debug", "https://www.example.com/users?other"),
        Some(Flag { debug: String::new() })
    );
    // (its value isn't a boolean's)
    assert_eq!(key_of::<BoolFlag>("www.example.com/users?debug", "https://www.example.com/users?debug"), None);
}

fn unsupported_patterns_fail_when_first_matched_with() {
    let request = DeepLinkRequest::of_uri(DeepLinkUri("https://www.example.com?name=a"));
    // A parameter name twice.
    let repeated = matcher::<FirstLast>("www.example.com?name={first}&name={last}");
    assert_eq!(
        panic_message(|| {
            repeated.r#match(&request);
        }),
        "IllegalArgumentException: Query parameter [name] must only be present once in the pattern \
         [//www.example.com?name={first}&name={last}]."
    );
    // Two unnamed parameters.
    let unnamed = matcher::<FirstLast>("www.example.com?{arg1}&{arg2}");
    assert_eq!(
        panic_message(|| {
            unnamed.r#match(&request);
        }),
        "IllegalArgumentException: DeepLink pattern [//www.example.com?{arg1}&{arg2}] already has unnamed query \
         parameter {arg1}. Cannot add unnamed query parameter {arg2}."
    );
    // A placeholder name twice: in the path, between the path and the query, in the query.
    let path = matcher::<UserId>("www.example.com/users/{id}/{id}");
    assert_eq!(
        panic_message(|| {
            path.r#match(&request);
        }),
        "IllegalArgumentException: Duplicate placeholder name [id] found in uri pattern \
         [//www.example.com/users/{id}/{id}]."
    );
    let across = matcher::<UserId>("https://www.example.com/users/{id}?other={id}");
    assert_eq!(
        panic_message(|| {
            across.r#match(&request);
        }),
        "IllegalArgumentException: Duplicate placeholder name [id] found in query of \
         [https://www.example.com/users/{id}?other={id}]."
    );
    let query = matcher::<UserId>("www.example.com?a={id}&b=x_{id}");
    assert_eq!(
        panic_message(|| {
            query.r#match(&request);
        }),
        "IllegalArgumentException: Duplicate placeholder name [id] found in query of [//www.example.com?a={id}&b=x_{id}]."
    );
    // A matcher of one is made without failing.
    let _ = matcher::<UserId>("www.example.com/users/{id}/{id}");
}

fn fragment_patterns_take_their_arguments() {
    // 1. A static fragment (the path's regex lets any fragment through: a fragment not matching gives no arguments).
    let fixed = match_of::<Users>("www.example.com/users#section1", "https://www.example.com/users#section1").unwrap();
    assert!(fixed.arguments().is_empty());
    assert!(fixed.is_exact_path());
    assert!(match_of::<Users>("www.example.com/users#section1", "https://www.example.com/users#other").is_some());
    assert!(match_of::<Users>("www.example.com/users#section1", "https://www.example.com/users").is_some());

    // 2. A placeholder.
    let placeholder =
        match_of::<UserId>("www.example.com/users#section_{id}", "https://www.example.com/users#section_123").unwrap();
    assert_eq!(placeholder.key(), &UserId { id: "123".into() });
    assert_eq!(arguments_of(&placeholder), args(&[("id", &["123"])]));
    assert_eq!(
        key_of::<UserId>("www.example.com/users#section_{id}", "https://www.example.com/users#SECTION_a%20b"),
        Some(UserId { id: "a b".into() })
    );
    assert_eq!(key_of::<UserId>("www.example.com/users#section_{id}", "https://www.example.com/users#other"), None);
    assert_eq!(key_of::<UserId>("www.example.com/users#section_{id}", "https://www.example.com/users"), None);

    // 3. A wildcard: no arguments.
    let wildcard =
        match_of::<Users>("www.example.com/users#section_.*", "https://www.example.com/users#section_123").unwrap();
    assert!(wildcard.arguments().is_empty());

    // Path, query and fragment together.
    #[derive(Saveable, Clone, PartialEq, Debug)]
    struct All {
        id: String,
        name: String,
        section: String,
    }
    let all = match_of::<All>(
        "www.example.com/users/{id}?name={name}#s_{section}",
        "https://www.example.com/users/7?name=bob#s_top",
    )
    .unwrap();
    assert_eq!(all.key(), &All { id: "7".into(), name: "bob".into(), section: "top".into() });
    // (fragment's, query's, then path's)
    assert_eq!(arguments_of(&all), args(&[("section", &["top"]), ("name", &["bob"]), ("id", &["7"])]));
}

fn schemes_and_authorities_are_matched_as_compose_matches_them() {
    // No scheme: http and https.
    let any = "www.example.com/users";
    assert!(match_of::<Users>(any, "https://www.example.com/users").is_some());
    assert!(match_of::<Users>(any, "http://www.example.com/users").is_some());
    assert!(match_of::<Users>(any, "ftp://www.example.com/users").is_none());
    // (a request without a scheme isn't matched)
    assert!(match_of::<Users>(any, "www.example.com/users").is_none());
    assert!(match_of::<Users>(any, "//www.example.com/users").is_none());
    // A "//" pattern is as one without a scheme.
    assert!(match_of::<Users>("//www.example.com/users", "http://www.example.com/users").is_some());
    // https: https alone.
    assert!(match_of::<Users>("https://www.example.com/users", "https://www.example.com/users").is_some());
    assert!(match_of::<Users>("https://www.example.com/users", "http://www.example.com/users").is_none());
    assert!(match_of::<Users>("HTTPS://www.example.com/users", "http://www.example.com/users").is_none());
    // http: either.
    assert!(match_of::<Users>("http://www.example.com/users", "https://www.example.com/users").is_some());
    // Others: as they are, case aside.
    assert!(match_of::<Users>("myapp://host/users", "myapp://host/users").is_some());
    assert!(match_of::<Users>("myapp://host/users", "MyApp://host/users").is_some());
    assert!(match_of::<Users>("myapp://host/users", "https://host/users").is_none());
    assert!(match_of::<Users>("my.app+x://host/users", "my.app+x://host/users").is_some());
    assert!(match_of::<Users>("my.app+x://host/users", "myXapp+x://host/users").is_none());
    // Case aside: scheme, authority, path.
    assert!(match_of::<Users>(any, "HTTPS://WWW.Example.COM/USERS").is_some());
    assert_eq!(
        key_of::<UserId>("www.example.com/users/{id}", "https://www.EXAMPLE.com/Users/AbC"),
        Some(UserId { id: "AbC".into() })
    );
    // Another authority, or none.
    assert!(match_of::<Users>(any, "https://other.com/users").is_none());
    assert!(match_of::<Users>(any, "https://www.example.com:8080/users").is_none());
    // (a pattern of a scheme without an authority is given "://": no request matches it)
    assert!(match_of::<Users>("myapp:/users", "myapp:/users").is_none());
    assert!(match_of::<Users>("myapp:/users", "myapp://users").is_none());
}

// ---- UriDeepLinkMatcher: the keys its decoder makes ----

fn empty_query_values_are_texts_or_missing() {
    let pattern = "www.example.com/u?name={name}&age={age}";
    // A text: "".
    assert_eq!(
        key_of::<Ages>(pattern, "https://www.example.com/u?name=&age=5"),
        Some(Ages { name: String::new(), age: 5 })
    );
    // Any other: missing — a required one fails, one with a default takes it.
    assert_eq!(key_of::<Ages>(pattern, "https://www.example.com/u?name=john&age="), None);
    assert_eq!(
        key_of::<AgesWithDefault>(pattern, "https://www.example.com/u?name=&age="),
        Some(AgesWithDefault { name: String::new(), age: 0 })
    );
    assert_eq!(
        match_of::<AgesWithDefault>(pattern, "https://www.example.com/u?name=&age=").map(|r| arguments_of(&r)),
        Some(args(&[("name", &[""]), ("age", &[""])]))
    );
    // (as an enum, a list)
    assert_eq!(key_of::<Paint>("www.example.com/p?color={color}", "https://www.example.com/p?color="), None);
    assert_eq!(
        key_of::<PaintWithDefault>("www.example.com/p?color={color}", "https://www.example.com/p?color="),
        Some(PaintWithDefault { color: Color::Red })
    );
    assert_eq!(key_of::<IntList>("www.example.com/p?list={list}", "https://www.example.com/p?list="), None);
}

fn defaults_and_nullables_are_kotlins() {
    // Defaults: missing or empty.
    assert_eq!(
        key_of::<AgesWithDefault>("www.example.com/u?age={age}", "https://www.example.com/u?age=7"),
        Some(AgesWithDefault { name: String::new(), age: 7 })
    );
    assert_eq!(
        key_of::<AgesWithDefault>("www.example.com/u", "https://www.example.com/u"),
        Some(AgesWithDefault { name: String::new(), age: 0 })
    );
    // A value of the wrong type isn't left to the default.
    assert_eq!(key_of::<AgesWithDefault>("www.example.com/u?age={age}", "https://www.example.com/u?age=x"), None);

    // Nullables: one without a default is required; with one, missing is null.
    let pattern = "www.example.com/u?nick={nick}&age={age}";
    assert_eq!(
        key_of::<Nullables>(pattern, "https://www.example.com/u?age=3"),
        Some(Nullables { nick: None, age: Some(3) })
    );
    assert_eq!(
        key_of::<Nullables>(pattern, "https://www.example.com/u?nick=&age=3"),
        Some(Nullables { nick: Some(String::new()), age: Some(3) })
    );
    assert_eq!(
        key_of::<Nullables>(pattern, "https://www.example.com/u?nick=bo&age=3"),
        Some(Nullables { nick: Some("bo".into()), age: Some(3) })
    );
    assert_eq!(key_of::<Nullables>(pattern, "https://www.example.com/u?nick=bo"), None);
    assert_eq!(key_of::<Nullables>(pattern, "https://www.example.com/u?nick=bo&age="), None);
    // (a text "null" is a text)
    assert_eq!(
        key_of::<Nullables>(pattern, "https://www.example.com/u?nick=null&age=1"),
        Some(Nullables { nick: Some("null".into()), age: Some(1) })
    );
}

fn primitives_enums_and_lists_are_read_by_kind() {
    let pattern = "www.example.com/p?b={b}&by={by}&s={s}&i={i}&l={l}&f={f}&d={d}&c={c}";
    let uri = |b: &str, by: &str, i: &str, f: &str| {
        format!("https://www.example.com/p?b={b}&by={by}&s=300&i={i}&l=9000000000&f={f}&d=2.25&c=xy")
    };
    assert_eq!(
        key_of::<Primitives>(pattern, &uri("true", "-128", "+12", "1.5")),
        Some(Primitives { b: true, by: -128, s: 300, i: 12, l: 9_000_000_000, f: 1.5, d: 2.25, c: 'x' })
    );
    // (Java's parseFloat: a type suffix, surrounding blanks)
    assert_eq!(key_of::<Primitives>(pattern, &uri("false", "0", "0", "2f")).map(|p| p.f), Some(2.0));
    // Not of their types: no match.
    assert_eq!(key_of::<Primitives>(pattern, &uri("True", "0", "0", "1")), None);
    assert_eq!(key_of::<Primitives>(pattern, &uri("true", "128", "0", "1")), None);
    assert_eq!(key_of::<Primitives>(pattern, &uri("true", "0", "2147483648", "1")), None);
    assert_eq!(key_of::<Primitives>(pattern, &uri("true", "0", "1.0", "1")), None);
    assert_eq!(key_of::<Primitives>(pattern, &uri("true", "0", "0", "abc")), None);

    // Enums by name (their serial names).
    let paint = "www.example.com/p?color={color}";
    assert_eq!(key_of::<Paint>(paint, "https://www.example.com/p?color=Green"), Some(Paint { color: Color::Green }));
    assert_eq!(key_of::<Paint>(paint, "https://www.example.com/p?color=DEEP_BLUE"), Some(Paint { color: Color::Blue }));
    assert_eq!(key_of::<Paint>(paint, "https://www.example.com/p?color=green"), None);
    assert_eq!(key_of::<Paint>(paint, "https://www.example.com/p?color=Blue"), None);
    assert_eq!(key_of::<PaintWithDefault>(paint, "https://www.example.com/p?color=Purple"), None);

    // Lists of primitives: each value one item; "null" none of a nullable one.
    let lists = "www.example.com/l?ids={ids}&names={names}&maybe={maybe}";
    assert_eq!(
        key_of::<Lists>(lists, "https://www.example.com/l?ids=1&ids=-2&names=a&maybe=3&maybe=null&names=b"),
        Some(Lists { ids: vec![1, -2], names: strings(&["a", "b"]), maybe: vec![Some(3), None] })
    );
    assert_eq!(
        key_of::<Lists>(lists, "https://www.example.com/l?ids=1"),
        Some(Lists { ids: vec![1], names: Vec::new(), maybe: Vec::new() })
    );
    assert_eq!(key_of::<Lists>(lists, "https://www.example.com/l?ids=1&ids=x"), None);
    assert_eq!(key_of::<Lists>(lists, "https://www.example.com/l?names=a"), None);
    // (an empty first value: missing, the others aside)
    assert_eq!(key_of::<Lists>(lists, "https://www.example.com/l?ids=&ids=2"), None);
    // (a later empty value is an item: of a list of texts, an empty text)
    assert_eq!(
        key_of::<Lists>(lists, "https://www.example.com/l?ids=1&names=a&names="),
        Some(Lists { ids: vec![1], names: strings(&["a", ""]), maybe: Vec::new() })
    );
}

fn classes_are_read_of_flattened_arguments() {
    // UriDeepLinkMatcher's KDoc: `Profile(user: User, id: Int)` of `?name=john&age=25&id=1234`.
    let pattern = "www.nav3example.com/profile?name={name}&age={age}&id={id}";
    assert_eq!(
        key_of::<Profile>(pattern, "https://www.nav3example.com/profile?name=john&age=25&id=1234"),
        Some(Profile { user: User { name: "john".into(), age: 25 }, id: 1234 })
    );
    // (a nested class's required argument missing: no match)
    assert_eq!(key_of::<Profile>(pattern, "https://www.nav3example.com/profile?name=john&id=1234"), None);
    // A name two classes have, of one type: one value for both.
    assert_eq!(
        key_of::<IdProfile>(
            "www.nav3example.com/profile?name={name}&id={id}",
            "https://www.nav3example.com/profile?name=jo&id=9"
        ),
        Some(IdProfile { user: IdUser { id: 9, name: "jo".into() }, id: 9 })
    );
    // A nullable class is null: no argument has its name.
    assert_eq!(
        key_of::<MaybeProfile>(pattern, "https://www.nav3example.com/profile?name=john&age=25&id=1"),
        Some(MaybeProfile { user: None, id: 1 })
    );
    // An object: no arguments needed.
    assert_eq!(key_of::<Users>("www.example.com/{x}", "https://www.example.com/anything"), Some(Users));
}

fn unsupported_types_fail_as_kotlins_decoder_throws() {
    // A collection of other than primitives.
    assert_eq!(
        panic_message(|| {
            match_of::<UserList>("www.example.com/u?users={users}", "https://www.example.com/u?users=a");
        }),
        format!(
            "SerializationException: Cannot decode collection of type with serial name {}. Only collections of \
             primitives are supported.",
            std::any::type_name::<User>()
        )
    );
    // A map.
    assert_eq!(
        panic_message(|| {
            match_of::<MapKey>("www.example.com/u?texts={texts}", "https://www.example.com/u?texts=a");
        }),
        format!(
            "IllegalArgumentException: Cannot decode field [texts] for type with serial name{}. Map decoding is not \
             supported.",
            std::any::type_name::<MapKey>()
        )
    );
    // A value of a kind its own serializer reads (contextual).
    assert_eq!(
        panic_message(|| {
            match_of::<PairKey>("www.example.com/u?pair={pair}", "https://www.example.com/u?pair=a");
        }),
        format!(
            "SerializationException: Cannot decode field [pair] for type with serial name {}. Unsupported structure \
             kind: CONTEXTUAL",
            std::any::type_name::<Pair>()
        )
    );
}

fn arguments_are_matched_path_first() {
    // matchArguments: the path's value of a name over the query's, the query's over the fragment's.
    let m = matcher::<UserId>("www.example.com/users");
    let map =
        |v: &str| -> KtMap<String, Vec<String>> { [("id".to_string(), vec![v.to_string()])].into_iter().collect() };
    let result = m.match_arguments(map("path"), map("query"), map("fragment")).unwrap();
    assert_eq!(result.key(), &UserId { id: "path".into() });
    assert_eq!(arguments_of(&result), args(&[("id", &["path"])]));
    assert!(!result.is_exact_path());
    let result = m.match_arguments(KtMap::new(), map("query"), map("fragment")).unwrap();
    assert_eq!(result.key(), &UserId { id: "query".into() });
    assert!(result.is_exact_path());
    let result = m.match_arguments(KtMap::new(), KtMap::new(), map("fragment")).unwrap();
    assert_eq!(result.key(), &UserId { id: "fragment".into() });
    assert!(m.match_arguments(KtMap::new(), KtMap::new(), KtMap::new()).is_none());
    // No values of a name: a text missing; any other, Kotlin's `first()` of an empty list.
    let no_values = || -> KtMap<String, Vec<String>> { [("id".to_string(), Vec::new())].into_iter().collect() };
    assert!(m.match_arguments(no_values(), KtMap::new(), KtMap::new()).is_none());
    assert_eq!(
        panic_message(|| {
            matcher::<Article>("www.example.com/a").match_arguments(no_values(), KtMap::new(), KtMap::new());
        }),
        "NoSuchElementException: List is empty."
    );
    // A request without a URI isn't matched.
    let extras_only = DeepLinkRequest::new(None, DeepLinkRequest::mime_type_extra("text/plain"));
    assert!(matcher::<Users>("www.example.com/users").r#match(&extras_only).is_none());
}

// ---- The order of matches ----

fn uri_match_results_are_ordered_as_composes() {
    let request = |uri: &str| DeepLinkRequest::of_uri(DeepLinkUri(uri));
    let of = |pattern: &str, uri: &str| matcher::<Loose>(pattern).r#match(&request(uri)).unwrap();
    let new = "https://www.example.com/users/new";

    // An exact path wins over arguments in the path.
    let exact = of("www.example.com/users/new", new);
    let one_arg = of("www.example.com/users/{id}", new);
    let two_args = of("www.example.com/{a}/{b}", new);
    assert!(exact.is_exact_path() && !one_arg.is_exact_path());
    assert_eq!(exact.compare_to(&one_arg), 1);
    assert_eq!(one_arg.compare_to(&exact), -1);
    assert_eq!(exact.compare_to(&two_args), 1);
    // More path arguments win.
    assert_eq!(two_args.compare_to(&one_arg), 1);
    assert_eq!(one_arg.compare_to(&two_args), -1);
    assert_eq!(one_arg.compare_to(&of("www.example.com/users/{name}", new)), 0);
    // Then having arguments.
    let query = "https://www.example.com/users/new?a=1&b=2";
    let none = of("www.example.com/users/.*", query);
    let a = of("www.example.com/users/.*?a={a}", query);
    let a_b = of("www.example.com/users/.*?a={a}&b={b}", query);
    assert!(!none.is_exact_path() && none.arguments().is_empty());
    assert_eq!(a.compare_to(&none), 1);
    assert_eq!(none.compare_to(&a), -1);
    // Then more arguments.
    assert_eq!(a_b.compare_to(&a), 1);
    assert_eq!(a.compare_to(&a_b), -1);
    assert_eq!(a.compare_to(&of("www.example.com/users/.*?b={b}", query)), 0);
    // (an exact path wins over more arguments; the path's count over the query's)
    assert_eq!(of("www.example.com/users/new", query).compare_to(&a_b), 1);
    assert_eq!(
        of("www.example.com/users/{id}", query).compare_to(&of("www.example.com/users/{id}?a={a}&b={b}", query)),
        -1
    );

    // A result made with the public constructor: not exact, no path argument counted (-1).
    let plain = UriMatchResult::new(Loose { id: None, a: None, b: None, name: None }, KtMap::new());
    assert!(!plain.is_exact_path());
    assert_eq!(plain.compare_to(&none), -1);
    assert_eq!(none.compare_to(&plain), 1);

    // A result that isn't a UriMatchResult loses; it is equal to any.
    let other = DeepLinkMatchResult::new(Loose { id: None, a: None, b: None, name: None });
    assert_eq!(none.compare_to(&other), 1);
    assert_eq!(other.compare_to(&exact), 0);

    // The best of several: the greatest.
    let results = [one_arg.clone(), exact.clone(), two_args.clone()];
    let best = results.iter().reduce(|best, r| if r.compare_to(best) > 0 { r } else { best }).unwrap();
    assert_eq!(best, &exact);

    // Wrapped results compare by what they wrap, against what the other wraps.
    let wrapping = |pattern: &str| {
        matcher::<Loose>(pattern).with_back_stack(|r| vec![format!("{:?}", r.key())]).r#match(&request(new)).unwrap()
    };
    let wrapped_exact = wrapping("www.example.com/users/new");
    let wrapped_arg = wrapping("www.example.com/users/{id}");
    assert_eq!(one_arg.compare_to(&wrapped_exact), -1);
    assert_eq!(exact.compare_to(&wrapped_arg), 1);
    assert_eq!(wrapped_exact.compare_to(&one_arg), 1);
    assert_eq!(wrapped_arg.compare_to(&wrapped_exact), -1);
    assert_eq!(wrapped_exact.compare_to(&exact), 0);
    assert_eq!(wrapped_arg.compare_to(&other), 1);
    assert_eq!(wrapped_exact.key(), exact.key());

    // Keys of different types compare once mapped to one (Kotlin's covariance).
    let user = match_of::<UserId>("www.example.com/users/{id}", new).unwrap().map_key(|k| format!("{k:?}"));
    let users = match_of::<Users>("www.example.com/users/new", new).unwrap().map_key(|k| format!("{k:?}"));
    assert_eq!(users.compare_to(&user), 1);
    assert_eq!(user.key(), "UserId { id: \"new\" }");
}

// ---- StaticKeyDeepLinkMatcher, filters, withBackStack ----

#[derive(Saveable, Clone, PartialEq, Debug)]
struct ImageKey;

impl NavKey for ImageKey {}

fn static_key_matchers_match_by_their_filters() {
    let matcher = StaticKeyDeepLinkMatcher::new(ImageKey, vec![mime_type_filter("image/jpg")]);
    assert_eq!(matcher.key(), &ImageKey);
    let with = |mime: &str| DeepLinkRequest::new(None, DeepLinkRequest::mime_type_extra(mime));
    assert_eq!(matcher.r#match(&with("image/jpg")), Some(DeepLinkMatchResult::new(ImageKey)));
    // (case aside)
    assert!(matcher.r#match(&with("IMAGE/JPG")).is_some());
    assert!(matcher.r#match(&with("image/png")).is_none());
    assert!(matcher.r#match(&with("image/jpg ")).is_none());
    // No mime type: no match, whatever the URI.
    assert!(matcher.r#match(&DeepLinkRequest::of_uri(DeepLinkUri("https://example.com/images"))).is_none());
    // No filters: any request.
    let any = StaticKeyDeepLinkMatcher::new(ImageKey, Vec::new());
    assert!(any.r#match(&DeepLinkRequest::of_uri(DeepLinkUri("x"))).is_some());
    // The filter alone.
    let filter = mime_type_filter("text/Plain");
    assert!(filter.filter_request(&with("TEXT/plain")));
    assert!(!filter.filter_request(&DeepLinkRequest::of_uri(DeepLinkUri("x"))));
    // A Uri matcher's filters too.
    let uri_matcher = UriDeepLinkMatcher::new(
        DeepLinkUri("www.example.com/images/{id}"),
        serializer::<UserId>(),
        vec![mime_type_filter("image/jpg")],
    );
    let uri = || Some(DeepLinkUri("https://www.example.com/images/5"));
    assert_eq!(
        uri_matcher
            .r#match(&DeepLinkRequest::new(uri(), DeepLinkRequest::mime_type_extra("image/JPG")))
            .map(|r| r.key().clone()),
        Some(UserId { id: "5".into() })
    );
    assert!(uri_matcher.r#match(&DeepLinkRequest::new(uri(), empty_request_extras())).is_none());
}

fn filters_are_checked_before_matching_in_order() {
    let calls = Rc::new(Cell::new(0));
    let counting = |pass: bool| -> Rc<dyn DeepLinkFilter> {
        let calls = calls.clone();
        Rc::new(move |_: &DeepLinkRequest| {
            calls.set(calls.get() + 1);
            pass
        })
    };
    let request = DeepLinkRequest::of_uri(DeepLinkUri("https://www.example.com?name=a"));
    // A pattern failing when parsed: not parsed while a filter fails, the filters after it not asked.
    let failing = UriDeepLinkMatcher::new(
        DeepLinkUri("www.example.com?name={first}&name={last}"),
        serializer::<FirstLast>(),
        vec![counting(true), counting(false), counting(true)],
    );
    assert!(failing.r#match(&request).is_none());
    assert_eq!(calls.get(), 2);
    let passing = UriDeepLinkMatcher::new(
        DeepLinkUri("www.example.com?name={first}&name={last}"),
        serializer::<FirstLast>(),
        vec![counting(true)],
    );
    assert!(
        panic_message(|| {
            passing.r#match(&request);
        })
        .starts_with("IllegalArgumentException: Query parameter [name]")
    );
    assert_eq!(calls.get(), 3);
    // match_request, the matching proper, doesn't filter.
    let filtered = StaticKeyDeepLinkMatcher::new(ImageKey, vec![counting(false)]);
    assert!(filtered.r#match(&request).is_none());
    assert!(filtered.match_request(&request).is_some());
}

fn back_stacks_are_built_lazily_of_the_match() {
    let built = Rc::new(Cell::new(0));
    let counter = built.clone();
    let matcher = StaticKeyDeepLinkMatcher::new(ImageKey, vec![mime_type_filter("image/jpg")]).with_back_stack(
        move |r: &dyn MatchResultOf<ImageKey>| {
            counter.set(counter.get() + 1);
            vec!["Home".to_string(), format!("{:?}", r.key())]
        },
    );
    // The wrapped matcher's filters apply.
    assert!(matcher.r#match(&DeepLinkRequest::of_uri(DeepLinkUri("x"))).is_none());
    assert!(matcher.filters().is_empty());
    let result = matcher.r#match(&DeepLinkRequest::new(None, DeepLinkRequest::mime_type_extra("image/jpg"))).unwrap();
    assert_eq!(built.get(), 0);
    assert_eq!(result.key(), &ImageKey);
    assert_eq!(result.back_stack(), strings(&["Home", "ImageKey"]));
    assert_eq!(built.get(), 1);
    assert_eq!(result.back_stack(), strings(&["Home", "ImageKey"]));
    assert_eq!(built.get(), 1);
    assert_eq!(
        format!("{result:?}"),
        "BackStackMatchResult { key: ImageKey, back_stack: Some([\"Home\", \"ImageKey\"]) }"
    );
    assert_eq!(matcher.matcher().key(), &ImageKey);
}

// ---- DeepLinkRequest, RequestExtras ----

#[derive(Clone, Copy, PartialEq, Debug)]
struct TextKey;
impl RequestExtrasKey for TextKey {
    type Value = String;
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct NumberKey;
impl RequestExtrasKey for NumberKey {
    type Value = i32;
}

/// Keys of one type told apart by `==`.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Named(&'static str);
impl RequestExtrasKey for Named {
    type Value = bool;
}

fn requests_need_a_uri_or_extras() {
    assert_eq!(
        panic_message(|| {
            DeepLinkRequest::new(None, empty_request_extras());
        }),
        "IllegalArgumentException: DeepLinkRequest must have either a uri or extras."
    );
    let uri_only = DeepLinkRequest::from_uri_string("https://a.com/x?y=1", empty_request_extras());
    assert_eq!(uri_only.uri().map(|u| u.to_string()), Some("https://a.com/x?y=1".to_string()));
    assert!(uri_only.extras().is_empty());
    assert_eq!(uri_only.to_string(), "DeepLinkRequest{ uri=https://a.com/x?y=1 }");
    let both = DeepLinkRequest::from_uri_string("https://a.com", DeepLinkRequest::mime_type_extra("image/jpg"));
    assert_eq!(
        both.to_string(),
        "DeepLinkRequest{ uri=https://a.com extras=RequestExtras({MimeTypeExtrasKey=image/jpg}) }"
    );
    let extras_only = DeepLinkRequest::new(None, request_extras(|s| s.put(NumberKey, 4)));
    assert!(extras_only.uri().is_none());
    assert_eq!(extras_only.to_string(), "DeepLinkRequest{ extras=RequestExtras({NumberKey=4}) }");
    assert_eq!(DeepLinkRequest::mime_type_extra("a/b").get(&MimeTypeExtrasKey), Some(&"a/b".to_string()));
}

fn request_extras_are_a_map_of_typed_keys() {
    let extras = request_extras(|s| {
        s.put(TextKey, "x".into());
        s.put(NumberKey, 1);
        s.put(TextKey, "y".into());
    });
    assert_eq!(extras.size(), 2);
    assert!(!extras.is_empty() && extras.is_not_empty());
    assert_eq!(extras.get(&TextKey), Some(&"y".to_string()));
    assert_eq!(extras.get(&NumberKey), Some(&1));
    assert!(extras.contains(&NumberKey));
    assert!(!extras.contains(&MimeTypeExtrasKey));
    assert_eq!(extras.get(&MimeTypeExtrasKey), None);
    assert_eq!(extras.to_string(), "RequestExtras({TextKey=y, NumberKey=1})");

    // Plus: the other's values for the same keys, the order kept.
    let other = request_extras(|s| {
        s.put(Named("a"), true);
        s.put(TextKey, "z".into());
    });
    let sum = extras.plus(&other);
    assert_eq!(sum.to_string(), "RequestExtras({TextKey=z, NumberKey=1, Named(\"a\")=true})");
    assert_eq!(&extras + &other, sum);
    // Minus: the other's keys out, whatever their values.
    let difference = sum.minus(&request_extras(|s| s.put(TextKey, "other".into())));
    assert_eq!(difference.to_string(), "RequestExtras({NumberKey=1, Named(\"a\")=true})");
    assert_eq!(&sum - &request_extras(|s| s.put(Named("b"), false)), sum);
    assert_eq!((&sum - &request_extras(|s| s.put(Named("a"), false))).size(), 2);

    // Keys of one type, told apart by ==.
    let named = request_extras(|s| {
        s.put(Named("a"), true);
        s.put(Named("b"), false);
    });
    assert_eq!(named.size(), 2);
    assert_eq!(named.get(&Named("b")), Some(&false));
    assert_eq!(named.get(&Named("c")), None);

    // Equality: the same entries, in any order.
    let reversed = request_extras(|s| {
        s.put(NumberKey, 1);
        s.put(TextKey, "y".into());
    });
    assert_eq!(extras, reversed);
    assert_ne!(extras, request_extras(|s| s.put(NumberKey, 1)));
    assert_ne!(
        reversed,
        request_extras(|s| {
            s.put(NumberKey, 2);
            s.put(TextKey, "y".into());
        })
    );
    assert_eq!(empty_request_extras(), request_extras(|_| {}));
    assert_eq!(empty_request_extras(), RequestExtras::default());
    assert!(empty_request_extras().is_empty());
    assert_eq!(empty_request_extras().to_string(), "RequestExtras({})");
}

// ---- The samples ----

/// DeepLinkMatcherSample.kt and DeepLinkNonPrimitiveArgSample.kt's keys.
#[derive(Saveable, Clone, PartialEq, Debug)]
struct UserListKey;
impl NavKey for UserListKey {}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Article {
    id: i32,
}
impl NavKey for Article {}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct ArticleSection {
    category: String,
}
impl NavKey for ArticleSection {}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct HomeKey;
impl NavKey for HomeKey {}

const SAMPLE_BASE_PATH: &str = "www.nav3deeplinksample.com";

/// Kotlin's `matcher.match(request)?.key ?: HomeKey`.
fn start_key<M>(matcher: &M, request: &DeepLinkRequest) -> Rc<dyn NavKey>
where
    M: DeepLinkMatcher,
    M::Key: NavKey + Clone,
{
    match matcher.r#match(request) {
        Some(result) => Rc::new(result.key().clone()),
        None => Rc::new(HomeKey),
    }
}

fn static_key_deep_link_matcher_sample() {
    // declare a matcher based on mime type
    let mime_type = "image/jpg";
    let mime_type_filter = mime_type_filter(mime_type);
    let matcher = StaticKeyDeepLinkMatcher::new(ImageKey, vec![mime_type_filter]);

    // when handling a request
    let request = DeepLinkRequest::new(
        Some(DeepLinkUri(&format!("{SAMPLE_BASE_PATH}/images/"))),
        DeepLinkRequest::mime_type_extra(mime_type),
    );
    // returns a valid result as long as mimeType matches
    let match_result: Option<DeepLinkMatchResult<ImageKey>> = matcher.r#match(&request);
    assert_eq!(match_result.map(|r| r.key().clone()), Some(ImageKey));
}

fn deep_link_matcher_with_back_stack_sample() {
    let matcher = UriDeepLinkMatcher::new(
        DeepLinkUri("https://www.nav3deeplinksample.com/userlist"),
        serializer::<UserListKey>(),
        Vec::new(),
    )
    .with_back_stack(|match_result| -> Vec<Rc<dyn NavKey>> {
        vec![Rc::new(HomeKey) as Rc<dyn NavKey>, Rc::new(match_result.key().clone())]
    });

    // handling a request (the sample's has a trailing '/' its pattern hasn't: as in Kotlin, it doesn't match)
    let request = DeepLinkRequest::of_uri(DeepLinkUri("https://www.nav3deeplinksample.com/userlist/"));
    assert!(matcher.r#match(&request).is_none());
    // returns a BackStackMatchResult containing the matched key and the constructed back stack
    let request = DeepLinkRequest::of_uri(DeepLinkUri("https://www.nav3deeplinksample.com/userlist"));
    let match_result = matcher.r#match(&request).unwrap();
    // listOf(HomeKey, UserListKey)
    assert_eq!(format!("{:?}", match_result.back_stack()), "[HomeKey, UserListKey]");
    assert_eq!(match_result.key(), &UserListKey);
}

fn deep_link_matcher_with_conditional_back_stack_sample() {
    let matcher = UriDeepLinkMatcher::new(
        DeepLinkUri("https://www.nav3deeplinksample.com/articles/{id}"),
        serializer::<Article>(),
        Vec::new(),
    )
    .with_back_stack(|match_result| -> Vec<Rc<dyn NavKey>> {
        let article = match_result.key().clone();
        let parent = if article.id >= 10 {
            ArticleSection { category: "Shipping".into() }
        } else {
            ArticleSection { category: "Returns".into() }
        };
        vec![Rc::new(parent) as Rc<dyn NavKey>, Rc::new(article)]
    });

    // handling a request
    let request = DeepLinkRequest::of_uri(DeepLinkUri("https://www.nav3deeplinksample.com/articles/12"));
    // returns a BackStackMatchResult containing the Article key and parent key based on article id
    let match_result = matcher.r#match(&request).unwrap();
    // listOf(parent, article)
    assert_eq!(
        format!("{:?}", match_result.back_stack()),
        "[ArticleSection { category: \"Shipping\" }, Article { id: 12 }]"
    );
    assert_eq!(match_result.key(), &Article { id: 12 });

    let other = matcher.r#match(&DeepLinkRequest::of_uri(DeepLinkUri("https://www.nav3deeplinksample.com/articles/3")));
    assert_eq!(
        format!("{:?}", other.unwrap().back_stack()),
        "[ArticleSection { category: \"Returns\" }, Article { id: 3 }]"
    );
    assert!(
        matcher
            .r#match(&DeepLinkRequest::of_uri(DeepLinkUri("https://www.nav3deeplinksample.com/articles/x")))
            .is_none()
    );
}

#[derive(Clone, PartialEq, Debug)]
struct Product {
    id: i32,
    name: String,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct ProductInventoryKey {
    product: Product,
    count: i32,
}
impl NavKey for ProductInventoryKey {}

// serializer to deserialize string value into a Product object
struct ProductSerializer;

impl DeepLinkSerializer for ProductSerializer {
    type Value = Product;

    fn serial_name() -> String {
        "androidx.navigation3.ui.samples.Product".into()
    }

    fn deserialize(value: &str) -> Product {
        let split_args: Vec<&str> = value.splitn(2, '-').collect();
        let id = kt::KtStr::to_int_or_null(split_args[0])
            .unwrap_or_else(|| panic_kt("SerializationException", &format!("Invalid product id: {}", split_args[0])));
        let name = split_args[1].to_string();
        Product { id, name }
    }

    fn serialize(value: &Product) -> String {
        format!("{}-{}", value.id, value.name)
    }
}

serializable_with!(Product, ProductSerializer);

fn deep_link_serializer_sample() {
    let matcher = UriDeepLinkMatcher::new(
        DeepLinkUri("http://www.nav3example.com/products?product={product}&count={count}"),
        serializer::<ProductInventoryKey>(),
        Vec::new(),
    );

    let request =
        DeepLinkRequest::of_uri(DeepLinkUri("http://www.nav3example.com/products?product=123-productA&count=12"));
    let key = start_key(&matcher, &request);
    assert_eq!(
        format!("{key:?}"),
        "ProductInventoryKey { product: Product { id: 123, name: \"productA\" }, count: 12 }"
    );

    // A required argument missing: HomeKey.
    let missing = DeepLinkRequest::of_uri(DeepLinkUri("http://www.nav3example.com/products?product=123-productA"));
    assert_eq!(format!("{:?}", start_key(&matcher, &missing)), "HomeKey");
    // The serializer's own exception is thrown (not a decoder's: not a mismatch).
    let invalid = DeepLinkRequest::of_uri(DeepLinkUri("http://www.nav3example.com/products?product=abc-x&count=1"));
    assert_eq!(
        panic_message(|| {
            matcher.r#match(&invalid);
        }),
        "SerializationException: Invalid product id: abc"
    );
    // Its descriptor: a STRING primitive of its serial name; it serializes as it deserializes.
    let descriptor = ProductSerializer::descriptor();
    assert_eq!(
        (descriptor.serial_name.as_str(), descriptor.kind),
        ("androidx.navigation3.ui.samples.Product", SerialKind::String)
    );
    let saver = ProductSerializer::saver();
    let product = Product { id: 7, name: "a-b".into() };
    assert_eq!(saver.save(&product), SavedValue::String("7-a-b".into()));
    assert_eq!(saver.restore(&SavedValue::String("7-a-b".into())), Some(product.clone()));
    assert_eq!(saver.descriptor(), descriptor);
    let key = ProductInventoryKey { product, count: 2 };
    assert_eq!(ProductInventoryKey::restore(&key.save()), Some(key));
}

/// navigation3-ui-samples' DeepLinkNonPrimitiveArgSamples.kt: a class of flattened arguments.
#[derive(Saveable, Clone, PartialEq, Debug)]
struct Name {
    #[saveable(name = "firstName")]
    first_name: String,
    #[saveable(name = "lastName")]
    last_name: String,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct ProfileKey {
    name: Name,
    age: i32,
}
impl NavKey for ProfileKey {}

#[derive(Clone, PartialEq, Debug)]
struct Hat {
    id: i32,
    weight: f64,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct HatListKey {
    hats: Vec<Hat>,
}
impl NavKey for HatListKey {}

// serializer to deserialize string value into a Hat object
struct HatSerializer;

impl DeepLinkSerializer for HatSerializer {
    type Value = Hat;

    fn serial_name() -> String {
        "androidx.navigation3.ui.samples.Hat".into()
    }

    fn deserialize(value: &str) -> Hat {
        let split_args: Vec<&str> = value.splitn(2, ':').collect();
        let id = kt::KtStr::to_int_or_null(split_args[0])
            .unwrap_or_else(|| panic_kt("SerializationException", &format!("Invalid product id: {}", split_args[0])));
        let weight = kt::KtStr::to_double_or_null(split_args[1])
            .unwrap_or_else(|| panic_kt("SerializationException", &format!("Invalid weight: {}", split_args[1])));
        Hat { id, weight }
    }

    fn serialize(value: &Hat) -> String {
        format!("{}:{}", value.id, value.weight)
    }
}

serializable_with!(Hat, HatSerializer);

fn non_primitive_arg_flattened_sample() {
    let matcher = UriDeepLinkMatcher::new(
        DeepLinkUri("http://www.nav3example.com/profile?firstName={firstName}&lastName={lastName}&age={age}"),
        serializer::<ProfileKey>(),
        Vec::new(),
    );

    let request =
        DeepLinkRequest::of_uri(DeepLinkUri("http://www.nav3example.com/profile?firstName=john&lastName=doe&age=25"));
    let key = start_key(&matcher, &request);
    assert_eq!(format!("{key:?}"), "ProfileKey { name: Name { first_name: \"john\", last_name: \"doe\" }, age: 25 }");
    let failed = DeepLinkRequest::of_uri(DeepLinkUri("http://www.nav3example.com/profile?firstName=john&age=25"));
    assert_eq!(format!("{:?}", start_key(&matcher, &failed)), "HomeKey");
}

fn ui_deep_link_serializer_sample() {
    let matcher = UriDeepLinkMatcher::new(
        DeepLinkUri("http://www.nav3example.com/products?product={product}&count={count}"),
        serializer::<ProductInventoryKey>(),
        Vec::new(),
    );

    let request =
        DeepLinkRequest::of_uri(DeepLinkUri("http://www.nav3example.com/products?product=123-productA&count=12"));
    let start_key = matcher.r#match(&request).map(|r| r.key().clone());
    assert_eq!(
        start_key,
        Some(ProductInventoryKey { product: Product { id: 123, name: "productA".into() }, count: 12 })
    );
}

fn list_type_deep_link_serializer_sample() {
    let matcher = UriDeepLinkMatcher::new(
        DeepLinkUri("http://www.nav3example.com/hatList?hats={hats}"),
        serializer::<HatListKey>(),
        Vec::new(),
    );

    let request =
        DeepLinkRequest::of_uri(DeepLinkUri("http://www.nav3example.com/hatList?hats=2342:4.5&hats=2643:5.6"));
    let key = start_key(&matcher, &request);
    assert_eq!(
        format!("{key:?}"),
        "HatListKey { hats: [Hat { id: 2342, weight: 4.5 }, Hat { id: 2643, weight: 5.6 }] }"
    );
    assert_eq!(
        format!(
            "{:?}",
            start_key(&matcher, &DeepLinkRequest::of_uri(DeepLinkUri("http://www.nav3example.com/hatList")))
        ),
        "HomeKey"
    );
    assert_eq!(
        panic_message(|| {
            matcher.r#match(&DeepLinkRequest::of_uri(DeepLinkUri("http://www.nav3example.com/hatList?hats=1:heavy")));
        }),
        "SerializationException: Invalid weight: heavy"
    );
}

fn main() {
    let tests: &[(&str, fn())] = &[
        ("uri_parts_are_read_as_the_non_android_uri_reads_them", uri_parts_are_read_as_the_non_android_uri_reads_them),
        ("relative_and_opaque_uris_are_as_androids", relative_and_opaque_uris_are_as_androids),
        ("escapes_are_decoded_as_uri_codec_decodes_them", escapes_are_decoded_as_uri_codec_decodes_them),
        ("path_patterns_take_their_arguments", path_patterns_take_their_arguments),
        ("query_patterns_take_their_arguments", query_patterns_take_their_arguments),
        ("unsupported_patterns_fail_when_first_matched_with", unsupported_patterns_fail_when_first_matched_with),
        ("fragment_patterns_take_their_arguments", fragment_patterns_take_their_arguments),
        (
            "schemes_and_authorities_are_matched_as_compose_matches_them",
            schemes_and_authorities_are_matched_as_compose_matches_them,
        ),
        ("empty_query_values_are_texts_or_missing", empty_query_values_are_texts_or_missing),
        ("defaults_and_nullables_are_kotlins", defaults_and_nullables_are_kotlins),
        ("primitives_enums_and_lists_are_read_by_kind", primitives_enums_and_lists_are_read_by_kind),
        ("classes_are_read_of_flattened_arguments", classes_are_read_of_flattened_arguments),
        ("unsupported_types_fail_as_kotlins_decoder_throws", unsupported_types_fail_as_kotlins_decoder_throws),
        ("arguments_are_matched_path_first", arguments_are_matched_path_first),
        ("uri_match_results_are_ordered_as_composes", uri_match_results_are_ordered_as_composes),
        ("static_key_matchers_match_by_their_filters", static_key_matchers_match_by_their_filters),
        ("filters_are_checked_before_matching_in_order", filters_are_checked_before_matching_in_order),
        ("back_stacks_are_built_lazily_of_the_match", back_stacks_are_built_lazily_of_the_match),
        ("requests_need_a_uri_or_extras", requests_need_a_uri_or_extras),
        ("request_extras_are_a_map_of_typed_keys", request_extras_are_a_map_of_typed_keys),
        ("static_key_deep_link_matcher_sample", static_key_deep_link_matcher_sample),
        ("deep_link_matcher_with_back_stack_sample", deep_link_matcher_with_back_stack_sample),
        ("deep_link_matcher_with_conditional_back_stack_sample", deep_link_matcher_with_conditional_back_stack_sample),
        ("deep_link_serializer_sample", deep_link_serializer_sample),
        ("non_primitive_arg_flattened_sample", non_primitive_arg_flattened_sample),
        ("ui_deep_link_serializer_sample", ui_deep_link_serializer_sample),
        ("list_type_deep_link_serializer_sample", list_type_deep_link_serializer_sample),
    ];
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = Vec::new();
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        if catch_unwind(test).is_err() {
            failed.push(*name);
            println!("FAILED {name}");
        } else {
            println!("ok     {name}");
        }
    }
    println!("\n{} run, {} failed", tests.len(), failed.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
}
