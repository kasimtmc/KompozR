//! DeepLinkRequest.kt: [`DeepLinkRequest`], its mime type extra ([`MimeTypeExtrasKey`]).

use super::extras::{RequestExtras, RequestExtrasKey, empty_request_extras, request_extras};
use super::uri::DeepLinkUri;
use kompoz_runtime::kt::require;
use std::fmt;

/// A requested deep link (Compose's `DeepLinkRequest`): its URI and its [`RequestExtras`], at least one of them.
#[derive(Clone, Debug)]
pub struct DeepLinkRequest {
    uri: Option<DeepLinkUri>,
    extras: RequestExtras,
}

/// The [`RequestExtrasKey`] of the mime type [`DeepLinkRequest::mime_type_extra`] stores.
///
/// Kotlin: `DeepLinkRequest.MimeTypeExtrasKey`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MimeTypeExtrasKey;

impl RequestExtrasKey for MimeTypeExtrasKey {
    type Value = String;
}

impl DeepLinkRequest {
    /// A request of `uri` and `extras`.
    /// Kotlin: `DeepLinkRequest(uri: DeepLinkUri? = null, extras: RequestExtras = emptyRequestExtras())`
    ///
    /// # Panics
    /// With neither a URI nor extras (Kotlin's `require`: an IllegalArgumentException).
    #[track_caller]
    pub fn new(uri: Option<DeepLinkUri>, extras: RequestExtras) -> DeepLinkRequest {
        require(
            uri.is_some() || !extras.map_is_empty(),
            Some(&|| "DeepLinkRequest must have either a uri or extras.".to_string()),
        );
        DeepLinkRequest { uri, extras }
    }

    /// A request of the URI `uri` (its text) and `extras`.
    /// Kotlin: `DeepLinkRequest(uri: String, extras: RequestExtras = emptyRequestExtras())`
    #[track_caller]
    pub fn from_uri_string(uri: &str, extras: RequestExtras) -> DeepLinkRequest {
        DeepLinkRequest::new(Some(DeepLinkUri::new(uri)), extras)
    }

    /// A request of `uri` alone.
    /// Kotlin: `DeepLinkRequest(uri: DeepLinkUri)`
    pub fn of_uri(uri: DeepLinkUri) -> DeepLinkRequest {
        DeepLinkRequest { uri: Some(uri), extras: empty_request_extras() }
    }

    /// Kotlin: `uri: DeepLinkUri?`
    pub fn uri(&self) -> Option<&DeepLinkUri> {
        self.uri.as_ref()
    }

    /// Kotlin: `extras: RequestExtras`
    pub fn extras(&self) -> &RequestExtras {
        &self.extras
    }

    /// [`RequestExtras`] storing `mime_type` with [`MimeTypeExtrasKey`] (`extras.get(&MimeTypeExtrasKey)` reads it).
    /// Kotlin: `DeepLinkRequest.mimeTypeExtra(mimeType: String): RequestExtras`
    pub fn mime_type_extra(mime_type: &str) -> RequestExtras {
        request_extras(|scope| scope.put(MimeTypeExtrasKey, mime_type.to_string()))
    }
}

/// `DeepLinkRequest{ uri=… extras=… }` (Kotlin's `toString()`).
impl fmt::Display for DeepLinkRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeepLinkRequest{")?;
        if let Some(uri) = &self.uri {
            write!(f, " uri={uri}")?;
        }
        if !self.extras.map_is_empty() {
            write!(f, " extras={}", self.extras)?;
        }
        f.write_str(" }")
    }
}

impl kompoz_runtime::kt::KtToString for DeepLinkRequest {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}
