//! Reading resource files (CMP's ResourceReader.kt and its desktop `JvmResourceReader`): a reader gives a file's
//! bytes, a part of them (a record of a converted values file) or its URI, by its path in the resources. The
//! default one reads what the generated `Res` embedded in the program (CMP's: the class path, where the build put
//! the files), and otherwise a file of that path next to the program or in the working directory; a composition's
//! is [`LocalResourceReader`]'s (tests provide their own).

use kompoz_runtime::KompozitionLocal;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{OnceLock, RwLock};

/// A resource that isn't there (CMP's MissingResourceException): "Missing resource with path: …".
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MissingResourceException {
    pub path: String,
    message: String,
}

impl MissingResourceException {
    pub fn new(path: impl Into<String>) -> MissingResourceException {
        let path = path.into();
        MissingResourceException { message: format!("Missing resource with path: {path}"), path }
    }
    /// With what more is known (Kotlin's internal `(path, message)` constructor).
    pub fn with_message(path: impl Into<String>, message: &str) -> MissingResourceException {
        let path = path.into();
        MissingResourceException { message: format!("Missing resource with path: {path}. {message}"), path }
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for MissingResourceException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MissingResourceException {}

/// What a reader's suspending functions give (Kotlin's `suspend fun`: a future, polled on the caller's thread).
pub type ResourceFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, MissingResourceException>> + 'a>>;

/// What reads resource files (CMP's `ResourceReader`, `@ExperimentalResourceApi`): by their path in the resources
/// (`composeResources/<package>/drawable/logo.png`).
pub trait ResourceReader {
    /// The whole file.
    fn read<'a>(&'a self, path: &'a str) -> ResourceFuture<'a, Vec<u8>>;
    /// `size` bytes of it from `offset`.
    fn read_part<'a>(&'a self, path: &'a str, offset: i64, size: i64) -> ResourceFuture<'a, Vec<u8>>;
    /// Its URI.
    fn get_uri(&self, path: &str) -> Result<String, MissingResourceException>;
}

/// A reader as a composition local holds it: compared by instance.
#[derive(Clone)]
pub struct ResourceReaderRef(pub Rc<dyn ResourceReader>);

impl ResourceReaderRef {
    pub fn new(reader: impl ResourceReader + 'static) -> ResourceReaderRef {
        ResourceReaderRef(Rc::new(reader))
    }
}

impl std::ops::Deref for ResourceReaderRef {
    type Target = dyn ResourceReader;
    fn deref(&self) -> &(dyn ResourceReader + 'static) {
        &*self.0
    }
}

impl PartialEq for ResourceReaderRef {
    fn eq(&self, other: &ResourceReaderRef) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for ResourceReaderRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("ResourceReader")
    }
}

// ---- What the generated Res embeds ----------------------------------------------------------------------------

/// A file a generated `Res` embedded: its path in the resources, its bytes, and the file it was read from at build
/// time (its URI while it is there).
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct EmbeddedFile {
    pub path: &'static str,
    pub bytes: &'static [u8],
    pub source: &'static str,
}

fn embedded() -> &'static RwLock<HashMap<&'static str, EmbeddedFile>> {
    static FILES: OnceLock<RwLock<HashMap<&'static str, EmbeddedFile>>> = OnceLock::new();
    FILES.get_or_init(Default::default)
}

/// A generated `Res`'s files, made readable by the default reader (it calls this before its resources are read,
/// as the build puts them on CMP's class path).
#[doc(hidden)]
pub fn register_embedded_files(files: &'static [EmbeddedFile]) {
    let mut map = embedded().write().unwrap_or_else(|e| e.into_inner());
    for f in files {
        map.insert(f.path, *f);
    }
}

/// What gives a generated `Res`'s drawables by name.
type DrawablesOf = fn() -> std::collections::BTreeMap<String, crate::resource::DrawableResource>;

fn packages() -> &'static RwLock<Vec<(&'static str, DrawablesOf)>> {
    static PACKAGES: OnceLock<RwLock<Vec<(&'static str, DrawablesOf)>>> = OnceLock::new();
    PACKAGES.get_or_init(Default::default)
}

/// A generated `Res`'s drawables, known by name for the resources of its directory (`module_dir`:
/// `composeResources/<package>/`) to refer to (`@drawable/name`).
#[doc(hidden)]
pub fn register_drawables(module_dir: &'static str, drawables: DrawablesOf) {
    let mut p = packages().write().unwrap_or_else(|e| e.into_inner());
    if !p.iter().any(|(d, _)| *d == module_dir) {
        p.push((module_dir, drawables));
    }
}

/// The drawable `name` of the resources `path` is one of (what `@drawable/name` in it refers to), if its `Res` is
/// known.
pub fn drawable_of_package(path: &str, name: &str) -> Option<crate::resource::DrawableResource> {
    let found = packages()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(d, _)| path.starts_with(d))
        .map(|(_, f)| *f)?;
    found().remove(name)
}

fn embedded_file(path: &str) -> Option<EmbeddedFile> {
    embedded().read().unwrap_or_else(|e| e.into_inner()).get(path).copied()
}

/// Where a path not embedded is looked for: as it is (relative to the working directory), then next to the program.
fn file_on_disk(path: &str) -> Option<PathBuf> {
    let p = PathBuf::from(path);
    if p.is_file() {
        return Some(p);
    }
    let beside = std::env::current_exe().ok()?.parent()?.join(path);
    beside.is_file().then_some(beside)
}

/// The default reader (CMP's `JvmResourceReader.Default`, on the class path): what a generated `Res` embedded,
/// else a file of the path in the working directory or next to the program.
#[derive(Clone, Copy, Default, Debug)]
pub struct DefaultResourceReader;

impl DefaultResourceReader {
    /// The whole file now (what the futures give).
    pub fn read_now(path: &str) -> Result<Vec<u8>, MissingResourceException> {
        if let Some(f) = embedded_file(path) {
            return Ok(f.bytes.to_vec());
        }
        let file = file_on_disk(path).ok_or_else(|| MissingResourceException::new(path))?;
        std::fs::read(file).map_err(|_| MissingResourceException::new(path))
    }

    /// Part of it now: `size` bytes from `offset` (fewer where the file ends first, as `readNBytes` reads them).
    pub fn read_part_now(path: &str, offset: i64, size: i64) -> Result<Vec<u8>, MissingResourceException> {
        let part = |bytes: &[u8]| {
            let start = (offset.max(0) as usize).min(bytes.len());
            let end = (start + size.max(0) as usize).min(bytes.len());
            bytes[start..end].to_vec()
        };
        if let Some(f) = embedded_file(path) {
            return Ok(part(f.bytes));
        }
        Ok(part(&Self::read_now(path)?))
    }

    /// Its URI: the file it was embedded from (while it is there) or found on disk, as a `file:` URI; else, an
    /// embedded file no longer on disk, written to the temporary directory once and that file's.
    pub fn uri_now(path: &str) -> Result<String, MissingResourceException> {
        let uri = |p: &std::path::Path| {
            let absolute = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            format!("file://{}", percent_encode_path(&absolute.to_string_lossy()))
        };
        if let Some(f) = embedded_file(path) {
            let source = std::path::Path::new(f.source);
            if source.is_file() {
                return Ok(uri(source));
            }
            // (the web has no files to write it out to)
            if cfg!(all(target_family = "wasm", target_os = "unknown")) {
                return Err(MissingResourceException::with_message(path, "No file for a URI on the web"));
            }
            let out = std::env::temp_dir().join(format!("kompoz-resources-{}", std::process::id())).join(path);
            if !out.is_file() {
                let written = out.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok())
                    && std::fs::write(&out, f.bytes).is_ok();
                if !written {
                    return Err(MissingResourceException::with_message(path, "Can't write it out for a URI"));
                }
            }
            return Ok(uri(&out));
        }
        file_on_disk(path).map(|p| uri(&p)).ok_or_else(|| MissingResourceException::new(path))
    }
}

/// A path as a URI's path: what isn't unreserved (or `/`) percent-encoded, as `File.toURI()` does.
fn percent_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~' | b':' | b'@' | b'!' | b'$' | b'&')
            || matches!(b, b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'=')
        {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl ResourceReader for DefaultResourceReader {
    fn read<'a>(&'a self, path: &'a str) -> ResourceFuture<'a, Vec<u8>> {
        Box::pin(async move { DefaultResourceReader::read_now(path) })
    }
    fn read_part<'a>(&'a self, path: &'a str, offset: i64, size: i64) -> ResourceFuture<'a, Vec<u8>> {
        Box::pin(async move { DefaultResourceReader::read_part_now(path, offset, size) })
    }
    fn get_uri(&self, path: &str) -> Result<String, MissingResourceException> {
        DefaultResourceReader::uri_now(path)
    }
}

thread_local! {
    static DEFAULT_READER: ResourceReaderRef = ResourceReaderRef::new(DefaultResourceReader);
}

/// The default reader as a composition local holds it (one instance a thread).
pub fn default_resource_reader() -> ResourceReaderRef {
    DEFAULT_READER.with(Clone::clone)
}

/// The reader composables read resources with (CMP's `LocalResourceReader`, `@ExperimentalResourceApi`): the
/// default one unless provided (tests count reads or give files of their own).
pub static LocalResourceReader: KompozitionLocal<ResourceReaderRef> =
    KompozitionLocal::new_static(default_resource_reader);

/// A resource file's bytes by its path in the resources (CMP's `readResourceBytes`, what `Res.readBytes` calls).
pub async fn read_resource_bytes(path: &str) -> Result<Vec<u8>, MissingResourceException> {
    DefaultResourceReader.read(path).await
}

/// A resource file's URI (CMP's `getResourceUri`, what `Res.getUri` calls).
pub fn get_resource_uri(path: &str) -> Result<String, MissingResourceException> {
    DefaultResourceReader.get_uri(path)
}
