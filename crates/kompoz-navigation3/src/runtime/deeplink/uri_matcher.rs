//! UriDeepLinkMatcher.kt: a matcher of a URI pattern ([`UriDeepLinkMatcher`]) and its results ([`UriMatchResult`]),
//! the pattern's parts as regexes (`UriPatternParser`), a request's arguments by them (`UriRequestParser`).

use super::decoder::DeepLinkDecoder;
use super::matcher::{DeepLinkFilter, DeepLinkMatcher, MatchResultOf, equals_ignore_case};
use super::request::DeepLinkRequest;
use super::uri::{DeepLinkUri, deep_link_uri_utils};
use kompoz_runtime::Saver;
use kompoz_runtime::kt::{self, KtMap, Regex, RegexOption, require};
use std::cell::OnceCell;
use std::fmt;
use std::rc::Rc;
use std::sync::LazyLock;

/// A deep link's arguments: each name's values.
type Arguments = KtMap<String, Vec<String>>;

/// A placeholder, `{argName}`.
static FILL_IN_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new("\\{(.+?)\\}"));
/// `"?type=user_{id}"` becomes `"^user_([\\s\\S]+?)?$"`.
const PLACEHOLDER_CONTENT_PATTERN: &str = "([\\s\\S]+?)?";
static PATH_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("([^/]*?|)"));
const DEFAULT_SCHEME_PATTERN: &str = "http[s]?://";
const STRICT_SCHEME_PATTERN: &str = "https://";
/// RFC 3986, section 3.1.
static EXPECTED_SCHEME_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("^[a-zA-Z][a-zA-Z0-9+\\-.]*:"));

/// A [`DeepLinkMatcher`] of a URI pattern (Compose's `UriDeepLinkMatcher<T>`): a request whose URI matches it gives
/// a [`UriMatchResult`], its key `T` made by the serializer of the arguments the URI gives.
///
/// **Path patterns** and what they take:
/// 1. An exact path, `www.example.com/users`: `https://www.example.com/users`, no arguments.
/// 2. A placeholder, `www.example.com/users/{id}`: `https://www.example.com/users/123` gives `id` `["123"]`.
/// 3. Placeholders, `www.example.com/users/{first}-{last}`: `…/users/john-doe` gives `first` `["john"]` and `last`
///    `["doe"]`.
/// 4. A wildcard, `www.example.com/users/.*`: `…/users/123/456`, no arguments.
/// 5. A literal and a placeholder, `www.example.com/users/user_{id}`: `…/users/user_123` gives `id` `["123"]`.
/// 6. An empty text, `www.example.com/users/{id}/profile`: `…/users//profile` gives `id` `[""]`.
///
/// **Query patterns**:
/// 1. A named parameter, `?name={name}`: `?name=john` gives `name` `["john"]`.
/// 2. An unnamed one, `?{rawQuery}`: `?anything&else` gives `rawQuery` `["anything", "else"]`.
/// 3. A partial literal, `?type=user_{id}`: `?type=user_123` gives `id` `["123"]`.
/// 4. Placeholders in one parameter, `?name={first}_{last}`: `?name=john_doe` gives `first` `["john"]`, `last`
///    `["doe"]`.
/// 5. A collection, `?list={list}`: `?list=10&list=20` gives `list` `["10", "20"]`.
/// 6. A wildcard, `?type=user_.*`: `?type=user_admin` gives `type` `["admin"]`.
/// 7. Named and unnamed, `?user={name}&{other}`: `?user=john&something` gives `name` `["john"]`, `other`
///    `["something"]`.
///
/// Not supported: a parameter name twice in a pattern (`?name={first}&name={last}`), two unnamed parameters
/// (`?{arg1}&{arg2}`) — both IllegalArgumentExceptions (panics) when the pattern is first matched with; a request's
/// parameters the pattern doesn't name are left aside (`?name=john&city=someCity` gives `name` alone).
///
/// **Fragment patterns**: a static one, `#section1`; a placeholder, `#section_{id}` (`#section_123` gives `id`
/// `["123"]`); a wildcard, `#section_.*`.
///
/// **An empty value** (`?name=&age=`) is an empty text for a `String` argument, and no value for any other (its
/// default if it has one; else a required value missing: no match).
///
/// **Arguments of a class** (not a primitive): `Profile(user: User, id: Int)`, `User(name: String, age: Int)`, read
/// from flattened arguments (`www.nav3example.com/profile?name=john&age=25&id=1234`), each class's elements of the
/// same arguments: a name two elements have takes one value for both (of two types, unpredictable mismatches).
///
/// Kotlin's open class's protected `matchUri` and `matchArguments` are public here ([`match_uri`](Self::match_uri),
/// [`match_arguments`](Self::match_arguments)): a matcher of its own built on one calls them.
pub struct UriDeepLinkMatcher<T> {
    normalized_uri_pattern: DeepLinkUri,
    serializer: Saver<T>,
    filters: Vec<Rc<dyn DeepLinkFilter>>,
    parsed_pattern: OnceCell<ParsedPattern>,
}

impl<T: 'static> UriDeepLinkMatcher<T> {
    /// A matcher of `uri_pattern`, its keys made by `serializer` (Kotlin's `KSerializer<T>`: a [`Saver`],
    /// `kompoz_runtime::serializer::<T>()` for a `Saveable` `T`), requests filtered by `filters`.
    /// Kotlin: `UriDeepLinkMatcher(uriPattern: DeepLinkUri, serializer: KSerializer<T>, filters: List<Filter> = emptyList())`
    pub fn new(
        uri_pattern: DeepLinkUri,
        serializer: Saver<T>,
        filters: Vec<Rc<dyn DeepLinkFilter>>,
    ) -> UriDeepLinkMatcher<T> {
        // (RFC 2396/3986: a URI without a scheme or "//" has no authority, which puts the path segments off by one
        // against requests with them: it is given its "//")
        let text = uri_pattern.to_string();
        let normalized_uri_pattern = if EXPECTED_SCHEME_REGEX.contains_match_in(&text) || text.starts_with("//") {
            uri_pattern
        } else {
            DeepLinkUri::new(&format!("//{text}"))
        };
        UriDeepLinkMatcher { normalized_uri_pattern, serializer, filters, parsed_pattern: OnceCell::new() }
    }

    /// The pattern parsed when first matched with (Kotlin's `lazy`): its errors then.
    #[track_caller]
    fn parsed_pattern(&self) -> &ParsedPattern {
        self.parsed_pattern.get_or_init(|| {
            // (the names taken so far: none twice)
            let mut taken = Vec::new();
            let path = uri_pattern_parser::parse_path(&self.normalized_uri_pattern, &mut taken);
            let query = uri_pattern_parser::parse_query(&self.normalized_uri_pattern, &mut taken);
            let fragment = uri_pattern_parser::parse_fragment(&self.normalized_uri_pattern, &mut taken);
            ParsedPattern { path, query, fragment }
        })
    }

    /// The match of `uri`: its scheme, authority and path segments (count, literals, placeholders) the pattern's,
    /// then the arguments it gives made a key.
    /// Kotlin: `matchUri(uri: DeepLinkUri): UriMatchResult<T>?` (protected)
    #[track_caller]
    pub fn match_uri(&self, uri: &DeepLinkUri) -> Option<UriMatchResult<T>> {
        let parsed = self.parsed_pattern();
        let regex_pattern = parsed.path.0.pattern();
        let scheme_end = kt::KtStr::index_of(&regex_pattern, "://", 0, false);
        let scheme_regex =
            Regex::with_option(&kt::KtStr::substring(&regex_pattern, 1, Some(scheme_end)), RegexOption::IgnoreCase);
        if !scheme_regex.matches(uri.get_scheme().unwrap_or("")) {
            return None;
        }
        let same_authority = match (uri.get_authority(), self.normalized_uri_pattern.get_authority()) {
            (None, None) => true,
            (Some(a), Some(b)) => equals_ignore_case(a, b),
            _ => false,
        };
        if !same_authority {
            return None;
        }
        // The path must match (segments, literals, placeholders, in the pattern's order); whether the request
        // matches is known when a key is made of the arguments.
        let path_args = uri_request_parser::extract_path_args(&parsed.path, uri)?;
        let query_args = uri_request_parser::extract_query_args(&parsed.query, uri);
        let fragment_args = uri_request_parser::extract_fragment_args(parsed.fragment.as_ref(), uri);
        self.match_arguments(path_args, query_args, fragment_args)
    }

    /// The match of a URI's arguments: a key made of them by the serializer (path arguments taking precedence, then
    /// the query's, then the fragment's), or none if it can't be.
    /// Kotlin: `matchArguments(pathArgs: Map<String, List<String>> = emptyMap(), queryArgs: Map<String, List<String>> = emptyMap(), fragmentArgs: Map<String, List<String>> = emptyMap()): UriMatchResult<T>?` (protected)
    ///
    /// # Panics
    /// As Kotlin's decoder throws for a key type it can't read (see `UriDeepLinkMatcher`).
    #[track_caller]
    pub fn match_arguments(
        &self,
        path_args: Arguments,
        query_args: Arguments,
        fragment_args: Arguments,
    ) -> Option<UriMatchResult<T>> {
        let mut arguments = Arguments::new();
        // (path arguments win: they are required)
        arguments.put_all(fragment_args.iter().map(|(k, v)| (k.clone(), v.clone())));
        arguments.put_all(query_args.iter().map(|(k, v)| (k.clone(), v.clone())));
        arguments.put_all(path_args.iter().map(|(k, v)| (k.clone(), v.clone())));
        // (a key the serializer doesn't restore of what was decoded: no match, where Kotlin's serializer throws)
        let key = DeepLinkDecoder::new(&arguments).decode_serializable_value(&self.serializer).ok()??;
        let is_exact_path =
            path_args.is_empty() && !self.normalized_uri_pattern.get_path_segments().iter().any(|s| s == ".*");
        Some(UriMatchResult::with_path(key, arguments, is_exact_path, path_args.size()))
    }
}

/// [`DeepLinkMatcher`]'s methods of its type (what the Kotlin syntax reads its results' types by).
impl<T: 'static> UriDeepLinkMatcher<T> {
    /// Kotlin: `match(request: DeepLinkRequest): UriMatchResult<T>?`
    pub fn r#match(&self, request: &DeepLinkRequest) -> Option<UriMatchResult<T>> {
        DeepLinkMatcher::r#match(self, request)
    }

    /// Kotlin: `withBackStack(backStackBuilder: (matchResult: MatchResult<T>) -> List<K>): BackStackMatcher<T, K>`
    pub fn with_back_stack<K: 'static>(
        self,
        back_stack_builder: impl Fn(&dyn MatchResultOf<T>) -> Vec<K> + 'static,
    ) -> super::BackStackMatcher<UriDeepLinkMatcher<T>, K> {
        DeepLinkMatcher::with_back_stack(self, back_stack_builder)
    }
}

impl<T: 'static> DeepLinkMatcher for UriDeepLinkMatcher<T> {
    type Key = T;
    type Result = UriMatchResult<T>;

    fn filters(&self) -> &[Rc<dyn DeepLinkFilter>] {
        &self.filters
    }

    /// The match of the request's URI (none without one): see [`match_uri`](UriDeepLinkMatcher::match_uri).
    fn match_request(&self, request: &DeepLinkRequest) -> Option<UriMatchResult<T>> {
        request.uri().and_then(|uri| self.match_uri(uri))
    }
}

/// A [`UriDeepLinkMatcher`]'s match (Compose's open class `UriMatchResult<T>`): the key and the arguments the request
/// gave.
///
/// **Order**: against another match, this one wins (1)
/// 1. if the other isn't a `UriMatchResult` (once unwrapped if it is a
///    [`WrappedMatchResult`](super::WrappedMatchResult));
/// 2. with an exact path when the other's has a wildcard or arguments;
/// 3. with more path arguments;
/// 4. with arguments when the other has none;
/// 5. with more arguments in all (path, query, fragment).
#[derive(Clone, PartialEq)]
pub struct UriMatchResult<T> {
    key: T,
    arguments: Arguments,
    is_exact_path: bool,
    matching_path_argument_count: i32,
}

impl<T> UriMatchResult<T> {
    /// Kotlin: `UriMatchResult(key: T, arguments: Map<String, List<String>> = emptyMap())`
    pub fn new(key: T, arguments: Arguments) -> UriMatchResult<T> {
        UriMatchResult { key, arguments, is_exact_path: false, matching_path_argument_count: -1 }
    }

    /// Kotlin's internal constructor: whether the path is exact (no arguments, no wildcards) and how many path
    /// arguments it took.
    pub(crate) fn with_path(
        key: T,
        arguments: Arguments,
        is_exact_path: bool,
        matching_path_argument_count: i32,
    ) -> UriMatchResult<T> {
        UriMatchResult { key, arguments, is_exact_path, matching_path_argument_count }
    }

    /// The arguments the request gave.
    /// Kotlin: `arguments: Map<String, List<String>>`
    pub fn arguments(&self) -> &Arguments {
        &self.arguments
    }

    /// Whether it matched on an exact path: no arguments, no wildcards in it.
    /// Kotlin: `isExactPath: Boolean` (protected)
    pub fn is_exact_path(&self) -> bool {
        self.is_exact_path
    }

    /// The same match, its key `f`'s of it (Kotlin's covariance, `out T`: matches of keys of different types
    /// compared).
    pub fn map_key<U>(self, f: impl FnOnce(T) -> U) -> UriMatchResult<U> {
        UriMatchResult {
            key: f(self.key),
            arguments: self.arguments,
            is_exact_path: self.is_exact_path,
            matching_path_argument_count: self.matching_path_argument_count,
        }
    }
}

impl<T: 'static> MatchResultOf<T> for UriMatchResult<T> {
    fn key(&self) -> &T {
        &self.key
    }

    fn compare_to(&self, other: &dyn MatchResultOf<T>) -> i32 {
        let other = other.as_wrapped().unwrap_or(other);
        let Some(other) = other.as_uri_match_result() else { return 1 };
        if self.is_exact_path != other.is_exact_path {
            return if self.is_exact_path { 1 } else { -1 };
        }
        let path_difference = self.matching_path_argument_count - other.matching_path_argument_count;
        if path_difference != 0 {
            return path_difference.signum();
        }
        if self.arguments.is_empty() != other.arguments.is_empty() {
            return if self.arguments.is_empty() { -1 } else { 1 };
        }
        (self.arguments.size() - other.arguments.size()).signum()
    }

    fn as_uri_match_result(&self) -> Option<&UriMatchResult<T>> {
        Some(self)
    }
}

impl<T: fmt::Debug> fmt::Debug for UriMatchResult<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UriMatchResult")
            .field("key", &self.key)
            .field("arguments", &self.arguments.as_slice())
            .field("is_exact_path", &self.is_exact_path)
            .field("matching_path_argument_count", &self.matching_path_argument_count)
            .finish()
    }
}

/// A pattern's parts (Compose's `ParsedPattern`): the path's regex and argument names; each query parameter's
/// pattern by its name (none without a query); the fragment's regex and argument names (none without a fragment).
struct ParsedPattern {
    path: (Regex, Vec<String>),
    query: KtMap<String, QueryParamPattern>,
    fragment: Option<(Regex, Vec<String>)>,
}

/// A query parameter of a pattern (Compose's internal `QueryParamPattern`): its regex (placeholders, wildcards,
/// literals), the argument names its placeholders take, whether it is unnamed (`?{user}`).
#[derive(Clone)]
struct QueryParamPattern {
    param_regex: Option<String>,
    /// The regex compiled (Kotlin compiles it for each value matched: the same regex).
    compiled: Option<Regex>,
    arguments: Vec<String>,
    is_unnamed_params: bool,
}

/// Kotlin's internal `UriPatternParser`.
mod uri_pattern_parser {
    use super::*;

    /// Takes `name`: false if it was taken.
    fn take(taken: &mut Vec<String>, name: &str) -> bool {
        if taken.iter().any(|t| t == name) {
            return false;
        }
        taken.push(name.to_string());
        true
    }

    /// The pattern's scheme, authority and path segments as a regex, and the path's argument names (its
    /// placeholders, `{id}`, in order).
    #[track_caller]
    pub(super) fn parse_path(uri_pattern: &DeepLinkUri, taken: &mut Vec<String>) -> (Regex, Vec<String>) {
        let mut uri_regex = String::from("^");
        let segments: Vec<&String> = uri_pattern.get_path_segments().iter().filter(|s| !s.is_empty()).collect();
        match uri_pattern.get_scheme() {
            None => uri_regex.push_str(DEFAULT_SCHEME_PATTERN),
            Some(scheme) if equals_ignore_case(scheme, "https") => uri_regex.push_str(STRICT_SCHEME_PATTERN),
            Some(scheme) if equals_ignore_case(scheme, "http") => uri_regex.push_str(DEFAULT_SCHEME_PATTERN),
            Some(scheme) => {
                uri_regex.push_str(&Regex::escape(scheme));
                uri_regex.push_str("://");
            }
        }
        if let Some(authority) = uri_pattern.get_authority() {
            uri_regex.push_str(&Regex::escape(authority));
            if !segments.is_empty() {
                uri_regex.push('/');
            }
        }
        let mut args = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            build_regex(uri_pattern, segment, &mut args, &mut uri_regex, taken);
            if index != segments.len() - 1 {
                uri_regex.push('/');
            }
        }
        let text = uri_pattern.to_string();
        let before = text.split('?').next().unwrap_or("").split('#').next().unwrap_or("");
        if before.ends_with('/') {
            uri_regex.push('/');
        }
        // The end, all parameters optional, or the `?` (or `#`) and anything after it.
        uri_regex.push_str("($|(\\?(.)*)|(#(.)*))");
        (Regex::with_option(&save_wildcard_in_regex(&uri_regex), RegexOption::IgnoreCase), args)
    }

    /// The pattern's query parameters by name, each its regex and argument names (the parameters a request has
    /// beyond them are left aside).
    #[track_caller]
    pub(super) fn parse_query(uri_pattern: &DeepLinkUri, taken: &mut Vec<String>) -> KtMap<String, QueryParamPattern> {
        let mut out = KtMap::new();
        let mut unnamed_param: Option<String> = None;
        // ("…?user={id}&place={location}": "user" and "place"; an unnamed parameter's name is all of "{argName}")
        for param_name in uri_pattern.get_query_parameter_names().iter() {
            let query_params = uri_pattern.get_query_parameters(param_name);
            require(
                query_params.len() <= 1,
                Some(&|| {
                    format!("Query parameter [{param_name}] must only be present once in the pattern [{uri_pattern}].")
                }),
            );
            let first_param = query_params.first().cloned().unwrap_or_default();
            let mut param = QueryParamPattern {
                param_regex: None,
                compiled: None,
                arguments: Vec::new(),
                is_unnamed_params: false,
            };
            // "www.example.com?{arg}"
            let is_arg = FILL_IN_PATTERN.matches(param_name);
            // "www.example.com?debug"
            let is_flag = first_param.is_empty();
            if is_arg {
                require(
                    unnamed_param.is_none(),
                    Some(&|| {
                        format!(
                            "DeepLink pattern [{uri_pattern}] already has unnamed query parameter {}. Cannot add \
                             unnamed query parameter {param_name}.",
                            unnamed_param.as_deref().unwrap_or("null")
                        )
                    }),
                );
                unnamed_param = Some(param_name.clone());
            }
            let query_param = if is_arg || is_flag {
                param.is_unnamed_params = true;
                param_name.clone()
            } else {
                first_param
            };
            // (literals and placeholders interleaved: "…?user=name_{first}_{last}")
            let literals: Vec<String> =
                FILL_IN_PATTERN.split(&query_param, 0).iter().map(|l| Regex::escape(l)).collect();
            let matches = FILL_IN_PATTERN.find_all(&query_param, 0);
            let mut arg_regex = String::new();
            for (index, literal) in literals.iter().enumerate() {
                arg_regex.push_str(literal);
                // (the placeholders' names: not pure wildcards and flags)
                if index < matches.len() {
                    arg_regex.push_str(PLACEHOLDER_CONTENT_PATTERN);
                    param.arguments.push(matches[index].group_values()[1].clone());
                }
            }
            arg_regex.push('$');
            param.param_regex = Some(save_wildcard_in_regex(&arg_regex));
            // The names of wildcards and flags.
            if param.arguments.is_empty() {
                let regex = param.param_regex.clone().unwrap_or_default();
                if regex.contains(".*") {
                    param.param_regex = Some(regex.replace(".*", "(.*)"));
                    param.arguments.push(param_name.clone());
                } else if is_flag {
                    param.param_regex = Some(format!("({})$", regex.strip_suffix('$').unwrap_or(&regex)));
                    param.arguments.push(param_name.clone());
                }
            }
            for arg_name in &param.arguments {
                require(
                    take(taken, arg_name),
                    Some(&|| format!("Duplicate placeholder name [{arg_name}] found in query of [{uri_pattern}].")),
                );
            }
            param.compiled = param.param_regex.as_deref().map(Regex::new);
            out.put(param_name.clone(), param);
        }
        out
    }

    /// The pattern's fragment as a regex and its argument names; none without a fragment.
    #[track_caller]
    pub(super) fn parse_fragment(uri_pattern: &DeepLinkUri, taken: &mut Vec<String>) -> Option<(Regex, Vec<String>)> {
        let fragment = uri_pattern.get_fragment()?;
        let mut frag_regex = String::new();
        let mut args = Vec::new();
        build_regex(uri_pattern, fragment, &mut args, &mut frag_regex, taken);
        Some((Regex::with_option(&save_wildcard_in_regex(&frag_regex), RegexOption::IgnoreCase), args))
    }

    /// A segment's regex: its placeholders replaced by a regex taking an argument (`user_{id}_profile`:
    /// `user_([^/]*?|)_profile` and `id`), its literals escaped.
    #[track_caller]
    fn build_regex(
        uri_pattern: &DeepLinkUri,
        segment: &str,
        args: &mut Vec<String>,
        uri_regex: &mut String,
        taken: &mut Vec<String>,
    ) {
        // No placeholders: the literal.
        if !segment.contains('{') {
            uri_regex.push_str(&Regex::escape(segment));
            return;
        }
        // One placeholder or several (`{arg}`, `{arg1}-{arg2}`), literals and placeholders (`user_{id}`), invalid
        // braces (`{{}}`).
        let mut result = FILL_IN_PATTERN.find(segment, 0);
        let mut append_pos = 0;
        while let Some(m) = result {
            let arg_name = m.group_values()[1].clone();
            require(
                take(taken, &arg_name),
                Some(&|| format!("Duplicate placeholder name [{arg_name}] found in uri pattern [{uri_pattern}].")),
            );
            args.push(arg_name);
            let range = m.range();
            // The literal before the placeholder.
            if range.first() > append_pos {
                uri_regex.push_str(&Regex::escape(&kt::KtStr::substring(&segment, append_pos, Some(range.first()))));
            }
            uri_regex.push_str(PATH_REGEX.pattern());
            append_pos = range.last() + 1;
            result = m.next();
        }
        // The literal after the placeholders.
        if append_pos < kt::KtStr::length(&segment) {
            uri_regex.push_str(&Regex::escape(&kt::KtStr::substring(&segment, append_pos, None)));
        }
    }

    /// An escaped wildcard made a regex's again (escaping all, then this, is simpler than escaping around them).
    fn save_wildcard_in_regex(regex: &str) -> String {
        if regex.contains("\\Q") && regex.contains("\\E") {
            regex.replace(".*", "\\E.*\\Q")
        } else if regex.contains("\\.\\*") {
            regex.replace("\\.\\*", ".*")
        } else {
            regex.to_string()
        }
    }
}

/// Kotlin's internal `UriRequestParser`.
mod uri_request_parser {
    use super::*;

    /// The path's arguments of `requested_uri` (empty without any), or none if its path doesn't match.
    pub(super) fn extract_path_args(
        parsed_path: &(Regex, Vec<String>),
        requested_uri: &DeepLinkUri,
    ) -> Option<Arguments> {
        let m = parsed_path.0.match_entire(&requested_uri.to_string())?;
        let groups = m.groups();
        Some(
            parsed_path
                .1
                .iter()
                .enumerate()
                .map(|(index, arg_name)| {
                    let value =
                        groups[index + 1].as_ref().map(|g| deep_link_uri_utils::decode(&g.value)).unwrap_or_default();
                    (arg_name.clone(), vec![value])
                })
                .collect(),
        )
    }

    /// The query's arguments of `requested_uri`: each pattern parameter's values by name (an unnamed one's the
    /// parts without a `=` that aren't a named parameter's flag), those matching its regex giving its placeholders'
    /// values; empty without a query.
    #[track_caller]
    pub(super) fn extract_query_args(
        parsed_query: &KtMap<String, QueryParamPattern>,
        requested_uri: &DeepLinkUri,
    ) -> Arguments {
        let split_input_params: Vec<&str> =
            requested_uri.get_query().map(|q| q.split('&').collect()).unwrap_or_default();
        let named_param_patterns: Vec<&String> =
            parsed_query.iter().filter(|(_, p)| !p.is_unnamed_params).map(|(k, _)| k).collect();
        let mut out = Arguments::new();
        for (param_name, param_pattern) in parsed_query.iter() {
            let mut input_param_values = requested_uri.get_query_parameters(param_name);
            // Unnamed inputs and flags (no other named parameters).
            if param_pattern.is_unnamed_params {
                input_param_values = split_input_params
                    .iter()
                    .filter(|part| !part.contains('=') && !named_param_patterns.iter().any(|n| n == *part))
                    .map(|part| part.to_string())
                    .collect();
            }
            // Each value (a repeated parameter, `?arg=1&arg=2`, has several).
            for input_param_value in &input_param_values {
                let Some(arg_match) = param_pattern.compiled.as_ref().and_then(|r| r.match_entire(input_param_value))
                else {
                    continue;
                };
                let groups = arg_match.groups();
                for (index, arg_name) in param_pattern.arguments.iter().enumerate() {
                    // (groups from 1: 0 is the whole match)
                    let value = groups[index + 1].as_ref().map(|g| g.value.clone()).unwrap_or_default();
                    let mut values = out.get_ref(arg_name).cloned().unwrap_or_default();
                    values.push(value);
                    out.put(arg_name.clone(), values);
                }
            }
        }
        out
    }

    /// The fragment's arguments of `requested_uri`: empty if it has no fragment, doesn't match, or has none.
    pub(super) fn extract_fragment_args(
        parsed_fragment: Option<&(Regex, Vec<String>)>,
        requested_uri: &DeepLinkUri,
    ) -> Arguments {
        let Some(fragment) = requested_uri.get_fragment() else { return Arguments::new() };
        let Some((regex, names)) = parsed_fragment else { return Arguments::new() };
        let Some(result) = regex.match_entire(fragment) else { return Arguments::new() };
        let groups = result.groups();
        names
            .iter()
            .enumerate()
            .map(|(index, arg_name)| {
                let value =
                    groups[index + 1].as_ref().map(|g| deep_link_uri_utils::decode(&g.value)).unwrap_or_default();
                (arg_name.clone(), vec![value])
            })
            .collect()
    }
}
