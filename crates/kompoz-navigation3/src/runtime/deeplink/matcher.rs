//! DeepLinkMatcher.kt, WrappedMatchResult.kt, StaticKeyDeepLinkMatcher.kt and BackStackMatcher.kt: matchers of deep
//! link requests ([`DeepLinkMatcher`]), their filters ([`DeepLinkFilter`]) and results ([`MatchResultOf`],
//! [`DeepLinkMatchResult`]), a matcher of filters alone ([`StaticKeyDeepLinkMatcher`]) and one building a back stack
//! ([`BackStackMatcher`]).

use super::request::{DeepLinkRequest, MimeTypeExtrasKey};
use super::uri_matcher::UriMatchResult;
use std::cell::OnceCell;
use std::fmt;
use std::rc::Rc;

/// A filter of deep link requests, such as a mime type's (Compose's fun interface `DeepLinkMatcher.Filter`): what a
/// matcher declares a request must have (a request may have more). A closure `Fn(&DeepLinkRequest) -> bool` is one.
///
/// Kotlin: `DeepLinkMatcher.Filter`
pub trait DeepLinkFilter {
    /// Whether `request` passes.
    /// Kotlin: `filterRequest(request: DeepLinkRequest): Boolean`
    fn filter_request(&self, request: &DeepLinkRequest) -> bool;
}

impl<F: Fn(&DeepLinkRequest) -> bool> DeepLinkFilter for F {
    fn filter_request(&self, request: &DeepLinkRequest) -> bool {
        self(request)
    }
}

/// A filter passing the requests whose mime type extra ([`DeepLinkRequest::mime_type_extra`]) is `mime_type`, case
/// aside.
/// Kotlin: `DeepLinkMatcher.mimeTypeFilter(mimeType: String): Filter`
pub fn mime_type_filter(mime_type: &str) -> Rc<dyn DeepLinkFilter> {
    let mime_type = mime_type.to_string();
    Rc::new(move |request: &DeepLinkRequest| {
        request.extras().get(&MimeTypeExtrasKey).is_some_and(|requested| equals_ignore_case(&mime_type, requested))
    })
}

/// Java's `equalsIgnoreCase` (Kotlin's `equals(other, ignoreCase = true)`): the same length, each pair of characters
/// the same, or of the same upper case, or of the same lower case of that.
pub(crate) fn equals_ignore_case(a: &str, b: &str) -> bool {
    // (a simple case mapping: the full one when it is one character)
    fn simple(mut mapped: impl Iterator<Item = char>, c: char) -> char {
        match (mapped.next(), mapped.next()) {
            (Some(x), None) => x,
            _ => c,
        }
    }
    let upper = |c: char| simple(c.to_uppercase(), c);
    let lower = |c: char| simple(c.to_lowercase(), c);
    a.chars().count() == b.chars().count()
        && a.chars().zip(b.chars()).all(|(x, y)| x == y || upper(x) == upper(y) || lower(upper(x)) == lower(upper(y)))
}

/// What a match of a [`DeepLinkMatcher`] is (the members of Compose's open class `DeepLinkMatcher.MatchResult<T>`,
/// whose instances are [`DeepLinkMatchResult`]s): the key of the deep link's target, and an order among matches
/// (`compare_to`: the greater wins).
///
/// Kotlin's results are covariant (`out T`: a `UriMatchResult<Article>` is a `MatchResult<NavKey>`); here a result's
/// key type is one type: results of different key types compare once mapped to one (`map_key`).
pub trait MatchResultOf<T>: 'static {
    /// The navigation key of the deep link's target.
    /// Kotlin: `key: T`
    fn key(&self) -> &T;

    /// This match against `other`: positive if this one is greater, 0 if equal, negative if less.
    /// Kotlin: `compareTo(other: MatchResult<T>): Int`
    fn compare_to(&self, other: &dyn MatchResultOf<T>) -> i32;

    /// The result it wraps, if it is a [`WrappedMatchResult`] (Kotlin's `is WrappedMatchResult` and its
    /// `matchResult`).
    fn as_wrapped(&self) -> Option<&dyn MatchResultOf<T>> {
        None
    }

    /// It as a [`UriMatchResult`], if it is one (Kotlin's `is UriMatchResult`: a type built on one answers with it).
    fn as_uri_match_result(&self) -> Option<&UriMatchResult<T>> {
        None
    }
}

/// A match of a [`DeepLinkMatcher`]: the key of the deep link's target (Compose's `DeepLinkMatcher.MatchResult`, as
/// it is: equal to any other in order).
///
/// Kotlin: `DeepLinkMatcher.MatchResult`
#[derive(Clone, Debug, PartialEq)]
pub struct DeepLinkMatchResult<T> {
    key: T,
}

impl<T> DeepLinkMatchResult<T> {
    /// Kotlin: `DeepLinkMatcher.MatchResult(key: T)`
    pub fn new(key: T) -> DeepLinkMatchResult<T> {
        DeepLinkMatchResult { key }
    }

    /// The same match, its key `f`'s of it (Kotlin's covariance, `out T`).
    pub fn map_key<U>(self, f: impl FnOnce(T) -> U) -> DeepLinkMatchResult<U> {
        DeepLinkMatchResult { key: f(self.key) }
    }
}

impl<T: 'static> MatchResultOf<T> for DeepLinkMatchResult<T> {
    fn key(&self) -> &T {
        &self.key
    }
    fn compare_to(&self, _other: &dyn MatchResultOf<T>) -> i32 {
        0
    }
}

/// A match wrapping another one (Compose's abstract class `WrappedMatchResult`): its key the wrapped one's, its order
/// the wrapped one's. An implementation's [`MatchResultOf`] gives the wrapped result as
/// [`as_wrapped`](MatchResultOf::as_wrapped) and compares by [`compare_wrapped`] (as [`BackStackMatchResult`]).
pub trait WrappedMatchResult<T>: MatchResultOf<T> {
    /// The wrapped match.
    /// Kotlin: `matchResult: DeepLinkMatcher.MatchResult<T>`
    fn match_result(&self) -> &dyn MatchResultOf<T>;
}

/// [`WrappedMatchResult`]'s order: its `match_result` against `other` (`other`'s wrapped result if it wraps one).
/// Kotlin: `WrappedMatchResult.compareTo(other: DeepLinkMatcher.MatchResult<T>): Int`
pub fn compare_wrapped<T: 'static>(match_result: &dyn MatchResultOf<T>, other: &dyn MatchResultOf<T>) -> i32 {
    let other = other.as_wrapped().unwrap_or(other);
    match_result.compare_to(other)
}

/// A deep link that can be deep linked into when matched with a [`DeepLinkRequest`] (Compose's abstract class
/// `DeepLinkMatcher<T, R>`): associated with a navigation key type ([`Key`](Self::Key), Kotlin's `T`; a key type may
/// have several matchers), giving its results ([`Result`](Self::Result), Kotlin's `R`) when a request passes its
/// [`filters`](Self::filters) and [`match_request`](Self::match_request) says so.
pub trait DeepLinkMatcher {
    /// Kotlin's `T`.
    type Key: 'static;
    /// Kotlin's `R`.
    type Result: MatchResultOf<Self::Key>;

    /// The filters a request passes before it is matched.
    /// Kotlin: `filters: List<Filter>`
    fn filters(&self) -> &[Rc<dyn DeepLinkFilter>];

    /// The matching proper, after the filters: the result, if `request` matches.
    /// Kotlin: `matchRequest(request: DeepLinkRequest): R?`
    fn match_request(&self, request: &DeepLinkRequest) -> Option<Self::Result>;

    /// The result of `request`, if each filter passes it and it matches (Compose's final `match`: not to be given
    /// another body).
    /// Kotlin: `match(request: DeepLinkRequest): R?`
    fn r#match(&self, request: &DeepLinkRequest) -> Option<Self::Result> {
        // (filters first: no key made for a request they don't pass)
        if !self.filters().iter().all(|filter| filter.filter_request(request)) {
            return None;
        }
        self.match_request(request)
    }

    /// This matcher building a back stack when it matches: `back_stack_builder` of its result (Compose's
    /// `withBackStack`).
    /// Kotlin: `DeepLinkMatcher<T, MatchResult<T>>.withBackStack(backStackBuilder: (matchResult: MatchResult<T>) -> List<K>): BackStackMatcher<T, K>`
    fn with_back_stack<K: 'static>(
        self,
        back_stack_builder: impl Fn(&dyn MatchResultOf<Self::Key>) -> Vec<K> + 'static,
    ) -> BackStackMatcher<Self, K>
    where
        Self: Sized,
    {
        BackStackMatcher { matcher: self, back_stack_builder: Rc::new(back_stack_builder) }
    }
}

/// A matcher of filters alone: when they all pass a request, a [`DeepLinkMatchResult`] of its [`key`](Self::key)
/// (Compose's `StaticKeyDeepLinkMatcher`).
pub struct StaticKeyDeepLinkMatcher<T> {
    key: T,
    filters: Vec<Rc<dyn DeepLinkFilter>>,
}

impl<T: Clone + 'static> StaticKeyDeepLinkMatcher<T> {
    /// Kotlin: `StaticKeyDeepLinkMatcher(key: T, filters: List<Filter>)`
    pub fn new(key: T, filters: Vec<Rc<dyn DeepLinkFilter>>) -> StaticKeyDeepLinkMatcher<T> {
        StaticKeyDeepLinkMatcher { key, filters }
    }

    /// Kotlin: `key: T`
    pub fn key(&self) -> &T {
        &self.key
    }

    /// [`DeepLinkMatcher::r#match`] of its type (what the Kotlin syntax reads its result's type by).
    /// Kotlin: `match(request: DeepLinkRequest): DeepLinkMatcher.MatchResult<T>?`
    pub fn r#match(&self, request: &DeepLinkRequest) -> Option<DeepLinkMatchResult<T>> {
        DeepLinkMatcher::r#match(self, request)
    }
}

impl<T: Clone + 'static> DeepLinkMatcher for StaticKeyDeepLinkMatcher<T> {
    type Key = T;
    type Result = DeepLinkMatchResult<T>;

    fn filters(&self) -> &[Rc<dyn DeepLinkFilter>] {
        &self.filters
    }

    fn match_request(&self, _request: &DeepLinkRequest) -> Option<DeepLinkMatchResult<T>> {
        Some(DeepLinkMatchResult::new(self.key.clone()))
    }
}

type BackStackBuilder<T, K> = Rc<dyn Fn(&dyn MatchResultOf<T>) -> Vec<K>>;

/// A matcher building a back stack when it matches (Compose's `BackStackMatcher<T, K>`, made by
/// [`with_back_stack`](DeepLinkMatcher::with_back_stack)): its [`matcher`](Self::matcher)'s match, wrapped with the
/// back stack its builder gives of it. Kotlin's `T : K` (the key one of the stack's): here the builder makes `K`s of
/// whatever it is given.
pub struct BackStackMatcher<M: DeepLinkMatcher, K> {
    matcher: M,
    back_stack_builder: BackStackBuilder<M::Key, K>,
}

impl<M: DeepLinkMatcher, K> BackStackMatcher<M, K> {
    /// The wrapped matcher.
    pub fn matcher(&self) -> &M {
        &self.matcher
    }
}

impl<M: DeepLinkMatcher, K: 'static> BackStackMatcher<M, K> {
    /// [`DeepLinkMatcher::r#match`] of its type (what the Kotlin syntax reads its result's type by).
    /// Kotlin: `match(request: DeepLinkRequest): BackStackMatchResult<T, K>?`
    pub fn r#match(&self, request: &DeepLinkRequest) -> Option<BackStackMatchResult<M::Key, K>> {
        DeepLinkMatcher::r#match(self, request)
    }
}

impl<M: DeepLinkMatcher, K: 'static> DeepLinkMatcher for BackStackMatcher<M, K> {
    type Key = M::Key;
    type Result = BackStackMatchResult<M::Key, K>;

    /// None of its own (the wrapped matcher's apply when it matches).
    fn filters(&self) -> &[Rc<dyn DeepLinkFilter>] {
        &[]
    }

    fn match_request(&self, request: &DeepLinkRequest) -> Option<BackStackMatchResult<M::Key, K>> {
        let result = self.matcher.r#match(request)?;
        Some(BackStackMatchResult {
            match_result: Rc::new(result),
            back_stack_builder: self.back_stack_builder.clone(),
            back_stack: OnceCell::new(),
        })
    }
}

/// A [`BackStackMatcher`]'s match (Compose's `BackStackMatchResult<T, K>`, a [`WrappedMatchResult`]): the wrapped
/// matcher's match and the back stack built of it, when first asked for.
pub struct BackStackMatchResult<T, K> {
    match_result: Rc<dyn MatchResultOf<T>>,
    back_stack_builder: BackStackBuilder<T, K>,
    back_stack: OnceCell<Vec<K>>,
}

/// The same match (Kotlin's reference to it): its back stack as built so far.
impl<T, K: Clone> Clone for BackStackMatchResult<T, K> {
    fn clone(&self) -> Self {
        BackStackMatchResult {
            match_result: self.match_result.clone(),
            back_stack_builder: self.back_stack_builder.clone(),
            back_stack: self.back_stack.clone(),
        }
    }
}

impl<T, K> BackStackMatchResult<T, K> {
    /// The back stack of navigation keys built for this match: built when first asked for, then kept.
    /// Kotlin: `backStack: List<K>`
    pub fn back_stack(&self) -> &[K] {
        self.back_stack.get_or_init(|| (self.back_stack_builder)(&*self.match_result))
    }
}

impl<T: 'static, K: 'static> MatchResultOf<T> for BackStackMatchResult<T, K> {
    fn key(&self) -> &T {
        self.match_result.key()
    }
    fn compare_to(&self, other: &dyn MatchResultOf<T>) -> i32 {
        compare_wrapped(&*self.match_result, other)
    }
    fn as_wrapped(&self) -> Option<&dyn MatchResultOf<T>> {
        Some(&*self.match_result)
    }
}

impl<T: 'static, K: 'static> WrappedMatchResult<T> for BackStackMatchResult<T, K> {
    fn match_result(&self) -> &dyn MatchResultOf<T> {
        &*self.match_result
    }
}

impl<T: fmt::Debug + 'static, K: fmt::Debug> fmt::Debug for BackStackMatchResult<T, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackStackMatchResult")
            .field("key", self.match_result.key())
            .field("back_stack", &self.back_stack.get())
            .finish()
    }
}
