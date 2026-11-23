//! Deep links: the URIs (and actions, and MIME types) a destination answers, and the requests that ask for one
//! (Compose's `NavDeepLink`, its builders, `navDeepLink { … }` and `navDeepLink<T>(basePath)`,
//! `NavDeepLinkRequest`).
//!
//! A URI pattern's path becomes a regular expression as Compose's does (each `{name}` an argument matching what a
//! path segment can hold, the scheme regardless of case, `.*` a wildcard); its query parameters each match their
//! own pattern (`?q={q}`, `?name={first}_{last}`, a repeated one a list's values); its fragment another. The
//! values found are parsed by the destination's arguments' types (a value a type can't parse: no match).

use crate::argument::{NavArguments, missing_required_arguments};
use crate::nav_type::NavType;
use crate::saved_state::{SavedState, contains, put, put_all};
use crate::serialization::{Route, TypeMap, generate_route_pattern};
use crate::uri::{self, NavUri};
use kompoz_runtime::SavedValue;
use kompoz_runtime::kt::{MatchResult, Regex, RegexOption};
use std::cell::OnceCell;
use std::fmt;
use std::rc::Rc;

const PATH_REGEX: &str = "([^/]*?|)";
const ANY_SYMBOLS_IN_THE_TAIL: &str = "([\\s\\S]+?)?";

thread_local! {
    static FILL_IN_PATTERN: Regex = Regex::new("\\{(.+?)\\}");
    static SCHEME_PATTERN: Regex = Regex::new("^[a-zA-Z]+[+\\w\\-.]*:");
    static QUERY_PATTERN: Regex = Regex::new("^[^?#]+\\?([^#]*).*");
    static PATH_END: Regex = Regex::new("(\\?|#|$)");
    static WILDCARD: Regex = Regex::new(".*");
    static PATH: Regex = Regex::new(PATH_REGEX);
}

/// A query parameter's pattern: its regular expression and the arguments its `{…}` parts fill.
#[derive(Clone, Debug)]
struct ParamQuery {
    param_regex: Option<String>,
    arguments: Vec<String>,
}

#[derive(Debug)]
struct Query {
    params: Vec<(String, ParamQuery)>,
    single_query_param_value_only: bool,
}

struct DeepLinkData {
    uri_pattern: Option<String>,
    action: Option<String>,
    mime_type: Option<String>,
    path_args: Vec<String>,
    path_regex: Option<String>,
    path_pattern: OnceCell<Option<Regex>>,
    is_exact_deep_link: bool,
    query: OnceCell<Query>,
    fragment: OnceCell<Option<(Vec<String>, String)>>,
    fragment_pattern: OnceCell<Option<Regex>>,
    mime_type_regex: Option<String>,
    mime_type_pattern: OnceCell<Option<Regex>>,
}

/// A deep link a destination answers (Compose's `NavDeepLink`): a URI pattern, an action, a MIME type — any of
/// them. Cheap to clone; equal when the three are.
#[derive(Clone)]
pub struct NavDeepLink(Rc<DeepLinkData>);

impl PartialEq for NavDeepLink {
    fn eq(&self, other: &Self) -> bool {
        self.0.uri_pattern == other.0.uri_pattern
            && self.0.action == other.0.action
            && self.0.mime_type == other.0.mime_type
    }
}

impl fmt::Debug for NavDeepLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NavDeepLink")
            .field("uri_pattern", &self.0.uri_pattern)
            .field("action", &self.0.action)
            .field("mime_type", &self.0.mime_type)
            .finish()
    }
}

/// `pattern` compiled regardless of case (Kotlin's `Regex(pattern, RegexOption.IGNORE_CASE)`).
fn ignore_case(pattern: &str) -> Regex {
    Regex::with_option(pattern, RegexOption::IgnoreCase)
}

/// `literal` escaped for a regular expression (Kotlin's `Regex.escape`).
fn escape(literal: &str) -> String {
    Regex::escape(literal)
}

/// The `.*`s of an escaped literal back to wildcards (Kotlin's `saveWildcardInRegex`).
fn save_wildcard_in_regex(s: &str) -> String {
    if s.contains("\\Q") && s.contains("\\E") {
        s.replace(".*", "\\E.*\\Q")
    } else if s.contains("\\.\\*") {
        s.replace("\\.\\*", ".*")
    } else {
        s.to_string()
    }
}

/// Kotlin's `s.substring(start, end)` (UTF-16 units).
fn kt_substring(s: &str, start: i32, end: Option<i32>) -> String {
    kompoz_runtime::kt::KtStr::substring(s, start, end)
}

/// Kotlin's `s.length` (UTF-16 units).
fn kt_length(s: &str) -> i32 {
    kompoz_runtime::kt::KtStr::length(s)
}

/// Group `index`'s text in `result`, if it took part (Kotlin's `result.groups[index]?.value`).
fn group(result: &MatchResult, index: usize) -> Option<String> {
    result.groups()[index].as_ref().map(|g| g.value.clone())
}

/// `uri`'s `{name}` parts as arguments, the rest literal, appended to `regex` (Kotlin's `buildRegex`; indices
/// Kotlin's, UTF-16 units).
fn build_regex(uri: &str, args: &mut Vec<String>, regex: &mut String) {
    let mut result = FILL_IN_PATTERN.with(|f| f.find(uri, 0));
    let mut append_pos = 0;
    while let Some(m) = result {
        args.push(group(&m, 1).expect("a name"));
        let range = m.range();
        if range.first() > append_pos {
            regex.push_str(&escape(&kt_substring(uri, append_pos, Some(range.first()))));
        }
        regex.push_str(PATH_REGEX);
        append_pos = range.last() + 1;
        result = m.next();
    }
    if append_pos < kt_length(uri) {
        regex.push_str(&escape(&kt_substring(uri, append_pos, None)));
    }
}

/// A MIME type's type and subtype.
struct MimeType {
    type_: String,
    sub_type: String,
}

impl MimeType {
    fn new(mime_type: &str) -> MimeType {
        let mut parts: Vec<&str> = mime_type.split('/').collect();
        while parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
        MimeType {
            type_: parts.first().copied().unwrap_or_default().to_string(),
            sub_type: parts.get(1).copied().unwrap_or_default().to_string(),
        }
    }

    /// 3 when both match, 2 the type only, 1 the subtype only.
    fn compare_to(&self, other: &MimeType) -> i32 {
        i32::from(self.type_ == other.type_) * 2 + i32::from(self.sub_type == other.sub_type)
    }
}

impl NavDeepLink {
    /// A deep link of `uri_pattern`, `action` and `mime_type` (each may be none).
    ///
    /// # Panics
    /// A MIME type not of the form `type/subtype`.
    pub fn new(uri_pattern: Option<String>, action: Option<String>, mime_type: Option<String>) -> NavDeepLink {
        let mut path_args = Vec::new();
        let mut path_regex = None;
        let mut is_exact_deep_link = false;
        if let Some(pattern) = &uri_pattern {
            let mut regex = String::from("^");
            if !SCHEME_PATTERN.with(|s| s.contains_match_in(pattern)) {
                regex.push_str("http[s]?://");
            }
            if let Some(end) = PATH_END.with(|p| p.find(pattern, 0)) {
                build_regex(&kt_substring(pattern.as_str(), 0, Some(end.range().first())), &mut path_args, &mut regex);
                // (Kotlin's `contains(Regex)` finds a match of `.*` in any text, an empty one: no deep link is
                // exact — 2.10's)
                is_exact_deep_link =
                    !WILDCARD.with(|w| w.contains_match_in(&regex)) && !PATH.with(|p| p.contains_match_in(&regex));
                regex.push_str("($|(\\?(.)*)|(#(.)*))");
            }
            path_regex = Some(save_wildcard_in_regex(&regex));
        }
        let mime_type_regex = mime_type.as_ref().map(|m| {
            assert!(
                Regex::new("^[\\s\\S]+/[\\s\\S]+$").matches(m),
                "The given mimeType {m} does not match to required \"type/subtype\" format"
            );
            let split = MimeType::new(m);
            let regex = format!("^({}|[*]+)/({}|[*]+)$", split.type_, split.sub_type);
            regex.replace("*|[*]", "[\\s\\S]")
        });
        NavDeepLink(Rc::new(DeepLinkData {
            uri_pattern,
            action,
            mime_type,
            path_args,
            path_regex,
            path_pattern: OnceCell::new(),
            is_exact_deep_link,
            query: OnceCell::new(),
            fragment: OnceCell::new(),
            fragment_pattern: OnceCell::new(),
            mime_type_regex,
            mime_type_pattern: OnceCell::new(),
        }))
    }

    /// A deep link of a URI pattern (Kotlin's `NavDeepLink(uri)`).
    pub fn from_uri_pattern(uri: impl Into<String>) -> NavDeepLink {
        NavDeepLink::new(Some(uri.into()), None, None)
    }

    /// Its URI pattern (Kotlin's `uriPattern`).
    pub fn uri_pattern(&self) -> Option<String> {
        self.0.uri_pattern.clone()
    }

    /// Its action (Kotlin's `action`).
    pub fn action(&self) -> Option<String> {
        self.0.action.clone()
    }

    /// Its MIME type (Kotlin's `mimeType`).
    pub fn mime_type(&self) -> Option<String> {
        self.0.mime_type.clone()
    }

    /// Whether it has no arguments nor wildcards in its path (Kotlin's `isExactDeepLink`; 2.10's is never).
    pub fn is_exact_deep_link(&self) -> bool {
        self.0.is_exact_deep_link
    }

    fn path_pattern(&self) -> Option<&Regex> {
        self.0.path_pattern.get_or_init(|| self.0.path_regex.as_deref().map(ignore_case)).as_ref()
    }

    fn is_parameterized_query(&self) -> bool {
        self.0.uri_pattern.as_deref().is_some_and(|p| QUERY_PATTERN.with(|q| q.matches(p)))
    }

    fn query(&self) -> &Query {
        self.0.query.get_or_init(|| self.parse_query())
    }

    fn parse_query(&self) -> Query {
        let mut q = Query { params: Vec::new(), single_query_param_value_only: false };
        if !self.is_parameterized_query() {
            return q;
        }
        let pattern = self.0.uri_pattern.as_deref().expect("a pattern");
        let uri = NavUri::new(pattern);
        for param_name in uri.get_query_parameter_names() {
            let query_params = uri.get_query_parameters(&param_name);
            assert!(
                query_params.len() <= 1,
                "Query parameter {param_name} must only be present once in {pattern}. To support repeated query \
                 parameters, use an array type for your argument and the pattern provided in your URI will be used \
                 to parse each query parameter instance."
            );
            // (`www.example.com?{arg}`: one parameter without a value, everything after the '?' its value)
            let query_param = match query_params.into_iter().next() {
                Some(v) => v,
                None => {
                    q.single_query_param_value_only = true;
                    param_name.clone()
                }
            };
            let mut arg_regex = String::new();
            let mut append_pos = 0;
            let mut param = ParamQuery { param_regex: None, arguments: Vec::new() };
            for m in FILL_IN_PATTERN.with(|f| f.find_all(&query_param, 0)) {
                param.arguments.push(group(&m, 1).expect("a name"));
                let range = m.range();
                if range.first() > append_pos {
                    arg_regex.push_str(&escape(&kt_substring(query_param.as_str(), append_pos, Some(range.first()))));
                }
                arg_regex.push_str(ANY_SYMBOLS_IN_THE_TAIL);
                append_pos = range.last() + 1;
            }
            if append_pos < kt_length(query_param.as_str()) {
                arg_regex.push_str(&escape(&kt_substring(query_param.as_str(), append_pos, None)));
            }
            arg_regex.push('$');
            param.param_regex = Some(save_wildcard_in_regex(&arg_regex));
            q.params.push((param_name, param));
        }
        q
    }

    fn fragment(&self) -> Option<&(Vec<String>, String)> {
        self.0
            .fragment
            .get_or_init(|| {
                let pattern = self.0.uri_pattern.as_deref()?;
                let fragment = NavUri::new(pattern).get_fragment()?;
                let mut args = Vec::new();
                let mut regex = String::new();
                build_regex(&fragment, &mut args, &mut regex);
                Some((args, regex))
            })
            .as_ref()
    }

    fn fragment_args(&self) -> Vec<String> {
        self.fragment().map(|f| f.0.clone()).unwrap_or_default()
    }

    fn fragment_pattern(&self) -> Option<&Regex> {
        self.0.fragment_pattern.get_or_init(|| self.fragment().map(|f| ignore_case(&f.1))).as_ref()
    }

    fn mime_type_pattern(&self) -> Option<&Regex> {
        self.0.mime_type_pattern.get_or_init(|| self.0.mime_type_regex.as_deref().map(Regex::new)).as_ref()
    }

    /// The names of the arguments it fills: its path's, its query's, its fragment's.
    pub(crate) fn argument_names(&self) -> Vec<String> {
        let mut names = self.0.path_args.clone();
        for (_, p) in &self.query().params {
            names.extend(p.arguments.iter().cloned());
        }
        names.extend(self.fragment_args());
        names
    }

    /// Whether `request` is one for it: its URI matches the pattern, its action the action, its MIME type the MIME
    /// type (what it has none of matches anything).
    pub(crate) fn matches(&self, request: &NavDeepLinkRequest) -> bool {
        let uri = match (self.path_pattern(), &request.uri) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(p), Some(u)) => p.matches(u.as_str()),
        };
        let action = match (&self.0.action, &request.action) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(a), Some(b)) => a == b,
        };
        let mime = match (&self.0.mime_type, &request.mime_type) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(_), Some(m)) => self.mime_type_pattern().expect("a MIME pattern").matches(m),
        };
        uri && action && mime
    }

    /// How well `mime_type` matches its MIME type: -1 not at all, else 1 the subtype, 2 the type, 3 both (Kotlin's
    /// `getMimeTypeMatchRating`).
    pub fn get_mime_type_match_rating(&self, mime_type: &str) -> i32 {
        match (&self.0.mime_type, self.mime_type_pattern()) {
            (Some(own), Some(p)) if p.matches(mime_type) => MimeType::new(own).compare_to(&MimeType::new(mime_type)),
            _ => -1,
        }
    }

    /// The arguments `deep_link` gives, if it matches: its path's, its query's and its fragment's values parsed
    /// by `arguments`' types (strings for an argument not there), every required argument there (Kotlin's
    /// `getMatchingArguments`).
    pub fn get_matching_arguments(&self, deep_link: &NavUri, arguments: &NavArguments) -> Option<SavedState> {
        let text = deep_link.as_str();
        let result = self.path_pattern()?.match_entire(text)?;
        let state = SavedState::new();
        if !self.get_matching_path_arguments(&result, &state, arguments) {
            return None;
        }
        if self.is_parameterized_query() && !self.get_matching_query_arguments(deep_link, &state, arguments) {
            return None;
        }
        self.get_matching_uri_fragment(deep_link.get_fragment(), &state, arguments);
        let missing = missing_required_arguments(arguments, |name| !contains(&state, name));
        if !missing.is_empty() {
            return None;
        }
        Some(state)
    }

    /// The path's and query's arguments of `deep_link`, as far as they parse (no check of the required ones).
    pub(crate) fn get_matching_path_and_query_args(
        &self,
        deep_link: Option<&NavUri>,
        arguments: &NavArguments,
    ) -> SavedState {
        let state = SavedState::new();
        let Some(deep_link) = deep_link else { return state };
        let text = deep_link.as_str();
        let Some(result) = self.path_pattern().and_then(|p| p.match_entire(text)) else { return state };
        self.get_matching_path_arguments(&result, &state, arguments);
        if self.is_parameterized_query() {
            self.get_matching_query_arguments(deep_link, &state, arguments);
        }
        state
    }

    fn get_matching_uri_fragment(&self, fragment: Option<String>, state: &SavedState, arguments: &NavArguments) {
        // (Kotlin matches the text "null" for no fragment)
        let fragment = fragment.unwrap_or_else(|| "null".into());
        let Some(result) = self.fragment_pattern().and_then(|p| p.match_entire(&fragment)) else { return };
        for (index, name) in self.fragment_args().iter().enumerate() {
            let value = group(&result, index + 1).as_deref().map(uri::decode).unwrap_or_default();
            if parse_argument(state, name, &value, arguments.get(name)).is_err() {
                return;
            }
        }
    }

    fn get_matching_path_arguments(&self, result: &MatchResult, state: &SavedState, arguments: &NavArguments) -> bool {
        for (index, name) in self.0.path_args.iter().enumerate() {
            let value = group(result, index + 1).as_deref().map(uri::decode).unwrap_or_default();
            if parse_argument(state, name, &value, arguments.get(name)).is_err() {
                // (a value its type can't parse: not a link for this destination)
                return false;
            }
        }
        true
    }

    fn get_matching_query_arguments(&self, deep_link: &NavUri, state: &SavedState, arguments: &NavArguments) -> bool {
        let query = self.query();
        for (param_name, stored) in &query.params {
            let mut input = deep_link.get_query_parameters(param_name);
            if query.single_query_param_value_only
                && let Some(value) = deep_link.get_query()
                && value != deep_link.as_str()
            {
                input = vec![value];
            }
            if !parse_input_params(&input, stored, state, arguments) {
                return false;
            }
        }
        true
    }

    /// How many path segments `requested_link` shares with the pattern (a better match has more).
    pub(crate) fn calculate_matching_path_segments(&self, requested_link: Option<&NavUri>) -> usize {
        let (Some(link), Some(pattern)) = (requested_link, &self.0.uri_pattern) else { return 0 };
        let requested = link.get_path_segments();
        let own = NavUri::new(pattern.as_str()).get_path_segments();
        let mut seen: Vec<&String> = Vec::new();
        for s in &requested {
            if own.contains(s) && !seen.contains(&s) {
                seen.push(s);
            }
        }
        seen.len()
    }
}

/// Parses `value` by `argument`'s type into `state` (a string for no argument) (Kotlin's `parseArgument`).
fn parse_argument(
    state: &SavedState,
    name: &str,
    value: &str,
    argument: Option<&crate::argument::NavArgument>,
) -> Result<(), crate::nav_type::NavTypeError> {
    match argument {
        Some(a) => a.r#type().parse_and_put(state, name, value),
        None => {
            put(state, name, SavedValue::String(value.to_string()));
            Ok(())
        }
    }
}

/// A query parameter's values into `state` (Kotlin's `parseInputParams`): each must match the parameter's
/// pattern; a collection's values add up; a value an argument's type can't parse is left out.
fn parse_input_params(inputs: &[String], stored: &ParamQuery, state: &SavedState, arguments: &NavArguments) -> bool {
    let temp = SavedState::new();
    for name in &stored.arguments {
        if let Some(argument) = arguments.get(name) {
            let t: NavType = argument.r#type();
            if t.is_collection() && !argument.is_default_value_present() {
                t.erased().put_empty_collection(&temp, name);
            }
        }
    }
    for input in inputs {
        let regex = stored.param_regex.as_deref().map(Regex::new);
        let Some(m) = regex.as_ref().and_then(|r| r.match_entire(input)) else { return false };
        for (index, name) in stored.arguments.iter().enumerate() {
            let value = group(&m, index + 1).unwrap_or_default();
            let argument = arguments.get(name);
            let _ = if !contains(&temp, name) {
                parse_argument(&temp, name, &value, argument)
            } else {
                parse_argument_for_repeated_param(&temp, name, &value, argument)
            };
        }
    }
    put_all(state, &temp);
    true
}

fn parse_argument_for_repeated_param(
    state: &SavedState,
    name: &str,
    value: &str,
    argument: Option<&crate::argument::NavArgument>,
) -> Result<(), crate::nav_type::NavTypeError> {
    if !contains(state, name) {
        return Ok(());
    }
    if let Some(a) = argument {
        a.r#type().erased().parse_and_put_repeated(state, name, value)?;
    }
    Ok(())
}

/// The builder of a [`NavDeepLink`] (Kotlin's `NavDeepLink.Builder`).
///
/// Kotlin: `NavDeepLink.Builder`
#[derive(Clone, Debug, Default)]
pub struct NavDeepLinkBuilder {
    uri_pattern: Option<String>,
    action: Option<String>,
    mime_type: Option<String>,
}

impl NavDeepLinkBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Kotlin's `NavDeepLink.Builder.fromUriPattern(uriPattern)`.
    pub fn from_uri_pattern(uri_pattern: impl Into<String>) -> Self {
        let mut b = Self::new();
        b.set_uri_pattern(uri_pattern);
        b
    }

    /// Kotlin's `NavDeepLink.Builder.fromUriPattern<T>(basePath, typeMap)`.
    pub fn from_uri_pattern_of<T: Route>(base_path: &str, type_map: &TypeMap) -> Self {
        let mut b = Self::new();
        b.set_uri_pattern_of::<T>(base_path, type_map);
        b
    }

    /// Kotlin's `NavDeepLink.Builder.fromAction(action)`.
    ///
    /// # Panics
    /// An empty action.
    pub fn from_action(action: impl Into<String>) -> Self {
        let mut b = Self::new();
        b.set_action(action);
        b
    }

    /// Kotlin's `NavDeepLink.Builder.fromMimeType(mimeType)`.
    pub fn from_mime_type(mime_type: impl Into<String>) -> Self {
        let mut b = Self::new();
        b.set_mime_type(mime_type);
        b
    }

    pub fn set_uri_pattern(&mut self, uri_pattern: impl Into<String>) -> &mut Self {
        self.uri_pattern = Some(uri_pattern.into());
        self
    }

    /// The pattern of `T`'s route under `base_path` (Kotlin's `setUriPattern<T>(basePath, typeMap)`):
    /// `base_path/{id}?tab={tab}`.
    pub fn set_uri_pattern_of<T: Route>(&mut self, base_path: &str, type_map: &TypeMap) -> &mut Self {
        self.uri_pattern = Some(generate_route_pattern::<T>(type_map, Some(base_path)));
        self
    }

    /// # Panics
    /// An empty action.
    pub fn set_action(&mut self, action: impl Into<String>) -> &mut Self {
        let action = action.into();
        assert!(!action.is_empty(), "The NavDeepLink cannot have an empty action.");
        self.action = Some(action);
        self
    }

    pub fn set_mime_type(&mut self, mime_type: impl Into<String>) -> &mut Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub fn build(&self) -> NavDeepLink {
        NavDeepLink::new(self.uri_pattern.clone(), self.action.clone(), self.mime_type.clone())
    }
}

/// What `nav_deep_link`'s block sets (Compose's `NavDeepLinkDslBuilder`): `uriPattern`, `action`, `mimeType`.
#[derive(Clone, Debug, Default)]
pub struct NavDeepLinkDslBuilder {
    uri_pattern: Option<String>,
    action: Option<String>,
    mime_type: Option<String>,
}

impl NavDeepLinkDslBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn for_route<T: Route>(base_path: &str, type_map: &TypeMap) -> Self {
        assert!(!base_path.is_empty(), "The basePath for NavDeepLink from KClass cannot be empty");
        NavDeepLinkDslBuilder {
            uri_pattern: Some(generate_route_pattern::<T>(type_map, Some(base_path))),
            ..Self::default()
        }
    }

    /// Kotlin's `uriPattern`.
    pub fn uri_pattern(&self) -> Option<String> {
        self.uri_pattern.clone()
    }
    /// Kotlin's `uriPattern = …`.
    pub fn set_uri_pattern(&mut self, value: Option<String>) {
        self.uri_pattern = value;
    }
    /// Kotlin's `action`.
    pub fn action(&self) -> Option<String> {
        self.action.clone()
    }
    /// Kotlin's `action = …`.
    ///
    /// # Panics
    /// An empty action.
    pub fn set_action(&mut self, value: Option<String>) {
        assert!(value.as_ref().is_none_or(|v| !v.is_empty()), "The NavDeepLink cannot have an empty action.");
        self.action = value;
    }
    /// Kotlin's `mimeType`.
    pub fn mime_type(&self) -> Option<String> {
        self.mime_type.clone()
    }
    /// Kotlin's `mimeType = …`.
    pub fn set_mime_type(&mut self, value: Option<String>) {
        self.mime_type = value;
    }

    /// # Panics
    /// With none of the three.
    pub(crate) fn build(&self) -> NavDeepLink {
        assert!(
            self.uri_pattern.is_some() || self.action.is_some() || self.mime_type.is_some(),
            "The NavDeepLink must have an uri, action, and/or mimeType."
        );
        let mut b = NavDeepLinkBuilder::new();
        if let Some(u) = &self.uri_pattern {
            b.set_uri_pattern(u.clone());
        }
        if let Some(a) = &self.action {
            b.set_action(a.clone());
        }
        if let Some(m) = &self.mime_type {
            b.set_mime_type(m.clone());
        }
        b.build()
    }
}

/// A deep link as `deep_link_builder` sets it (Compose's `navDeepLink { uriPattern = …; action = … }`).
pub fn nav_deep_link(deep_link_builder: impl FnOnce(&mut NavDeepLinkDslBuilder)) -> NavDeepLink {
    let mut b = NavDeepLinkDslBuilder::new();
    deep_link_builder(&mut b);
    b.build()
}

/// A deep link to `T`'s destination under `base_path` (Compose's `navDeepLink<T>(basePath, typeMap) { … }`): its
/// pattern is `T`'s route's with `base_path` for its name (`kompoz://profile/{id}?tab={tab}`).
///
/// Kotlin: `navDeepLink<T>(basePath: String, typeMap: Map<KType, NavType<*>> = emptyMap(), deepLinkBuilder: NavDeepLinkDslBuilder.() -> Unit = {})`
pub fn nav_deep_link_of_with<T: Route>(
    base_path: &str,
    type_map: TypeMap,
    deep_link_builder: impl FnOnce(&mut NavDeepLinkDslBuilder),
) -> NavDeepLink {
    let mut b = NavDeepLinkDslBuilder::for_route::<T>(base_path, &type_map);
    deep_link_builder(&mut b);
    b.build()
}

/// The same, with no typeMap nor other settings (Compose's `navDeepLink<T>(basePath)`).
///
/// Kotlin: `navDeepLink<T>(basePath: String)`
pub fn nav_deep_link_of<T: Route>(base_path: &str) -> NavDeepLink {
    nav_deep_link_of_with::<T>(base_path, TypeMap::default(), |_| {})
}

/// A request for a deep link (Compose's `NavDeepLinkRequest`): a URI, an action, a MIME type.
#[derive(Clone, PartialEq)]
pub struct NavDeepLinkRequest {
    uri: Option<NavUri>,
    action: Option<String>,
    mime_type: Option<String>,
}

impl fmt::Debug for NavDeepLinkRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for NavDeepLinkRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NavDeepLinkRequest{")?;
        if let Some(u) = &self.uri {
            write!(f, " uri={u}")?;
        }
        if let Some(a) = &self.action {
            write!(f, " action={a}")?;
        }
        if let Some(m) = &self.mime_type {
            write!(f, " mimetype={m}")?;
        }
        f.write_str(" }")
    }
}

impl NavDeepLinkRequest {
    pub fn new(uri: Option<NavUri>, action: Option<String>, mime_type: Option<String>) -> NavDeepLinkRequest {
        NavDeepLinkRequest { uri, action, mime_type }
    }
    /// Its URI (Kotlin's `uri`).
    pub fn uri(&self) -> Option<NavUri> {
        self.uri.clone()
    }
    /// Its action (Kotlin's `action`).
    pub fn action(&self) -> Option<String> {
        self.action.clone()
    }
    /// Its MIME type (Kotlin's `mimeType`).
    pub fn mime_type(&self) -> Option<String> {
        self.mime_type.clone()
    }
}

/// The builder of a [`NavDeepLinkRequest`] (Kotlin's `NavDeepLinkRequest.Builder`).
///
/// Kotlin: `NavDeepLinkRequest.Builder`
#[derive(Clone, Debug, Default)]
pub struct NavDeepLinkRequestBuilder {
    uri: Option<NavUri>,
    action: Option<String>,
    mime_type: Option<String>,
}

impl NavDeepLinkRequestBuilder {
    /// Kotlin's `NavDeepLinkRequest.Builder.fromUri(uri)`.
    pub fn from_uri(uri: NavUri) -> Self {
        let mut b = Self::default();
        b.set_uri(uri);
        b
    }
    /// Kotlin's `NavDeepLinkRequest.Builder.fromAction(action)`.
    pub fn from_action(action: impl Into<String>) -> Self {
        let mut b = Self::default();
        b.set_action(action);
        b
    }
    /// Kotlin's `NavDeepLinkRequest.Builder.fromMimeType(mimeType)`.
    pub fn from_mime_type(mime_type: impl Into<String>) -> Self {
        let mut b = Self::default();
        b.set_mime_type(mime_type);
        b
    }
    pub fn set_uri(&mut self, uri: NavUri) -> &mut Self {
        self.uri = Some(uri);
        self
    }
    /// # Panics
    /// An empty action.
    pub fn set_action(&mut self, action: impl Into<String>) -> &mut Self {
        let action = action.into();
        assert!(!action.is_empty(), "The NavDeepLinkRequest cannot have an empty action.");
        self.action = Some(action);
        self
    }
    /// # Panics
    /// A MIME type not of the form `type/subtype`.
    pub fn set_mime_type(&mut self, mime_type: impl Into<String>) -> &mut Self {
        let mime_type = mime_type.into();
        assert!(
            Regex::new("^[-\\w*.]+/[-\\w+*.]+$").matches(&mime_type),
            "The given mimeType {mime_type} does not match to required \"type/subtype\" format"
        );
        self.mime_type = Some(mime_type);
        self
    }
    pub fn build(&self) -> NavDeepLinkRequest {
        NavDeepLinkRequest::new(self.uri.clone(), self.action.clone(), self.mime_type.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The patterns deep links make, as kotlin.text.Regex (kompoz-runtime's kt::Regex, Java's syntax) reads them.

    #[test]
    fn literals_classes_and_quantifiers() {
        assert!(Regex::new("abc").matches("abc"));
        assert!(!Regex::new("abc").matches("abcd"));
        assert!(Regex::new("a.c").matches("abc") && !Regex::new("a.c").matches("a\nc"));
        assert!(Regex::new("[^/]*").matches("a b") && !Regex::new("[^/]*").matches("a/b"));
        assert!(Regex::new("^[a-zA-Z]+[+\\w\\-.]*:").contains_match_in("kompoz://x"));
        assert!(!Regex::new("^[a-zA-Z]+[+\\w\\-.]*:").contains_match_in("item/42"));
        assert!(Regex::new("^[-\\w*.]+/[-\\w+*.]+$").matches("image/*"));
        assert!(Regex::new("a{2,3}").matches("aaa") && !Regex::new("a{2,3}").matches("aaaa"));
        assert!(Regex::new("[\\s\\S]+").matches("a\nb"));
        assert!(Regex::new("[*]+").matches("**"));
    }

    #[test]
    fn groups_take_what_lazy_and_greedy_quantifiers_leave() {
        let r = Regex::new("^\\Qkompoz://item/\\E([^/]*?|)($|(\\?(.)*)|(#(.)*))");
        let m = r.match_entire("kompoz://item/42?x=1").expect("a match");
        assert_eq!(group(&m, 1).as_deref(), Some("42"));
        assert_eq!(group(&m, 2).as_deref(), Some("?x=1"));
        let lazy = Regex::new("([\\s\\S]+?)?_([\\s\\S]+?)?$");
        let m = lazy.match_entire("John_Doe").expect("a match");
        assert_eq!((group(&m, 1).as_deref(), group(&m, 2).as_deref()), (Some("John"), Some("Doe")));
        let m = lazy.match_entire("_").expect("a match");
        assert_eq!((group(&m, 1), group(&m, 2)), (None, None));
    }

    #[test]
    fn case_is_ignored_when_asked_and_quoting_is_javas() {
        assert!(ignore_case("\\Qhttps://example.com/\\E([^/]*?|)").matches("HTTPS://Example.com/7"));
        assert!(!Regex::new("\\Qhttps\\E").matches("HTTPS"));
        assert_eq!(escape("a.b"), "\\Qa.b\\E");
        assert!(Regex::new(&escape("a\\Eb")).matches("a\\Eb"));
        let all: Vec<_> = FILL_IN_PATTERN.with(|f| f.find_all("a/{x}/{y}", 0)).iter().map(|m| group(m, 1)).collect();
        assert_eq!(all, [Some("x".to_string()), Some("y".to_string())]);
        let m = PATH_END.with(|p| p.find("a/b", 0)).expect("the end");
        assert_eq!((m.range().first(), m.range().last()), (3, 2));
    }

    #[test]
    fn arguments_are_found_by_kotlins_indices_past_wide_characters() {
        // (UTF-16 units, as Kotlin's: a character beyond the BMP counts two)
        let (mut args, mut regex) = (Vec::new(), String::new());
        build_regex("😀/{a}/ü{b}", &mut args, &mut regex);
        assert_eq!(args, ["a", "b"]);
        assert_eq!(regex, format!("\\Q😀/\\E{PATH_REGEX}\\Q/ü\\E{PATH_REGEX}"));
    }
}
