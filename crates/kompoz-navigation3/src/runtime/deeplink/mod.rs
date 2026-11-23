//! navigation3-runtime's `androidx.navigation3.runtime.deeplink` (its common source set and the non-Android one's
//! URI): deep link requests ([`DeepLinkRequest`]: a [`DeepLinkUri`] and [`RequestExtras`]), matchers of them
//! ([`DeepLinkMatcher`]: [`UriDeepLinkMatcher`] of a URI pattern, [`StaticKeyDeepLinkMatcher`] of filters alone,
//! [`BackStackMatcher`] building a back stack), their results and their order ([`MatchResultOf`],
//! [`UriMatchResult`], [`BackStackMatchResult`]), and [`DeepLinkSerializer`]s of arguments of one's own types.
//!
//! Kotlin's nested names are flat here: `DeepLinkMatcher.MatchResult` is [`DeepLinkMatchResult`] (and the members
//! of its open class [`MatchResultOf`]), `DeepLinkMatcher.Filter` [`DeepLinkFilter`], `DeepLinkMatcher.mimeTypeFilter`
//! [`mime_type_filter`], `DeepLinkRequest.MimeTypeExtrasKey` [`MimeTypeExtrasKey`]. `match` is a Rust keyword: the
//! method is [`r#match`](DeepLinkMatcher::r#match).
//!
//! kotlinx.serialization's `KSerializer<T>` is a [`Saver<T>`](kompoz_runtime::Saver) (`kompoz_runtime::serializer::<T>()`
//! of a `Saveable` key type, its `descriptor()` what the arguments are read by), `@Serializable(with = …)`
//! [`serializable_with!`].

mod decoder;
mod extras;
mod matcher;
mod request;
mod serializer;
mod uri;
mod uri_matcher;

#[doc(hidden)]
pub use extras::request_extras_async;
pub use extras::{RequestExtras, RequestExtrasKey, RequestExtrasScope, empty_request_extras, request_extras};
pub use matcher::{
    BackStackMatchResult, BackStackMatcher, DeepLinkFilter, DeepLinkMatchResult, DeepLinkMatcher, MatchResultOf,
    StaticKeyDeepLinkMatcher, WrappedMatchResult, compare_wrapped, mime_type_filter,
};
pub use request::{DeepLinkRequest, MimeTypeExtrasKey};
pub use serializer::DeepLinkSerializer;
pub use uri::DeepLinkUri;
pub use uri_matcher::{UriDeepLinkMatcher, UriMatchResult};

pub use crate::__serializable_with as serializable_with;

/// What [`serializable_with!`] expands to names.
#[doc(hidden)]
pub mod __private {
    pub use kompoz_runtime::{Saveable, SavedValue, SerialDescriptor};
}
