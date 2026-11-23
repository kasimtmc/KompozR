//! Resources (CMP's Resource.kt and the resource classes): a resource is an id and the items it has, one for each
//! set of qualifiers it was given for (`drawable/logo.png`, `drawable-dark/logo.png`); the generated `Res` makes
//! them (`Res::drawable::logo()`), the composables pick the item of the environment.

use crate::qualifier::Qualifier;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// One file (or one record of a values file) of a resource (CMP's ResourceItem): its qualifiers, its path in the
/// resources (`composeResources/<package>/drawable-dark/logo.png`), and, for a record of a converted values file,
/// where it is in it (`offset` and `size` in bytes; -1 for the whole file).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceItem {
    pub qualifiers: Vec<Qualifier>,
    pub path: String,
    pub offset: i64,
    pub size: i64,
}

impl ResourceItem {
    /// Kotlin: `ResourceItem(qualifiers: Set<Qualifier>, path: String, offset: Long, size: Long)`
    pub fn new(qualifiers: Vec<Qualifier>, path: impl Into<String>, offset: i64, size: i64) -> ResourceItem {
        ResourceItem { qualifiers, path: path.into(), offset, size }
    }
}

/// What every resource is (CMP's sealed `Resource`): an id (`drawable:logo`) and its items. Resources of a kind are
/// equal by id.
#[derive(Clone)]
pub struct Resource {
    id: Arc<str>,
    items: Arc<[ResourceItem]>,
}

impl Resource {
    #[doc(hidden)]
    pub fn new(id: impl Into<Arc<str>>, items: Vec<ResourceItem>) -> Resource {
        Resource { id: id.into(), items: items.into() }
    }
    /// Its id (`drawable:logo`, `string:app_name`).
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Its items, one for each set of qualifiers.
    pub fn items(&self) -> &[ResourceItem] {
        &self.items
    }
}

impl PartialEq for Resource {
    fn eq(&self, other: &Resource) -> bool {
        self.id == other.id
    }
}

impl Eq for Resource {}

impl Hash for Resource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

macro_rules! resources {
    ($($(#[$doc:meta])* $name:ident { $($key:ident)? })*) => {$(
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            resource: Resource,
            $($key: Arc<str>,)?
        }

        impl std::ops::Deref for $name {
            type Target = Resource;
            fn deref(&self) -> &Resource {
                &self.resource
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.resource.id)
            }
        }

        impl AsRef<Resource> for $name {
            fn as_ref(&self) -> &Resource {
                &self.resource
            }
        }
    )*};
}

resources! {
    /// A drawable (CMP's DrawableResource): an image (PNG, JPEG, WebP, BMP), an Android vector drawable (`.xml`)
    /// or an SVG, of `drawable*/`. [`painter_resource`](crate::painter_resource) paints it.
    DrawableResource {}
    /// A string of `values*/strings.xml` (CMP's StringResource), by its key.
    StringResource { key }
    /// A quantity string (`<plurals>`) of `values*/*.xml` (CMP's PluralStringResource), by its key.
    PluralStringResource { key }
    /// A string array (`<string-array>`) of `values*/*.xml` (CMP's StringArrayResource), by its key.
    StringArrayResource { key }
    /// A font file of `font*/` (CMP's FontResource).
    FontResource {}
}

impl DrawableResource {
    /// Kotlin: `DrawableResource(id: String, items: Set<ResourceItem>)`
    #[doc(hidden)]
    pub fn new(id: impl Into<Arc<str>>, items: Vec<ResourceItem>) -> DrawableResource {
        DrawableResource { resource: Resource::new(id, items) }
    }
}

impl FontResource {
    /// Kotlin: `FontResource(id: String, items: Set<ResourceItem>)`
    #[doc(hidden)]
    pub fn new(id: impl Into<Arc<str>>, items: Vec<ResourceItem>) -> FontResource {
        FontResource { resource: Resource::new(id, items) }
    }
}

macro_rules! keyed {
    ($($name:ident)*) => {$(
        impl $name {
            #[doc(hidden)]
            pub fn new(id: impl Into<Arc<str>>, key: impl Into<Arc<str>>, items: Vec<ResourceItem>) -> $name {
                $name { resource: Resource::new(id, items), key: key.into() }
            }
            /// Its key, the `name` it has in its values file.
            pub fn key(&self) -> &str {
                &self.key
            }
        }
    )*};
}

keyed!(StringResource PluralStringResource StringArrayResource);
