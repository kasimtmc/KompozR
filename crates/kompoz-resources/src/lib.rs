//! KompozR's resources, as Compose Multiplatform's `org.jetbrains.compose.resources` (components-resources
//! 1.11.1): a crate's `composeResources/` directory — `drawable*/` images, vector drawables and SVGs, `font*/`
//! fonts, `values*/*.xml` strings, quantity strings and string arrays, `files/` raw files — read through the `Res`
//! the [`compose_resources!`] macro generates (CMP's build generates its `Res` class the same way, at build time),
//! each resource picked for the environment: the UI language (and region), the theme, the screen density.
//!
//! ```ignore
//! kompoz_resources::compose_resources!(); // `Res`, of this crate's composeResources/
//!
//! #[kompozable]
//! fn Header() {
//!     Image(painter_resource(Res::drawable::logo()), Some(string_resource(Res::string::app_name())));
//!     Text(plural_string_resource_formatted(Res::plurals::new_messages(), 3, &[&3]));
//! }
//! ```
//!
//! The directory's layout is CMP's: a resource type, then qualifiers, `-` between them (`drawable-dark-xxhdpi`,
//! `values-pt-rBR`): a language (`en`, `fil`), a region after it (`rBR`), a theme (`light`, `dark`), a density
//! (`ldpi` … `xxxhdpi`). Values files are converted at build time into CMP's records (`values/strings.commonMain.cvr`:
//! `type|key|base64` lines) that the strings are read from; everything is embedded in the program (CMP puts it on
//! the class path) and read by [`DefaultResourceReader`] — a composition's reader is [`LocalResourceReader`]'s,
//! its environment [`LocalComposeEnvironment`]'s (both for tests to provide).

#![allow(non_upper_case_globals, non_snake_case)]

mod caches;
pub mod environment;
pub mod font;
pub mod image;
pub mod plural;
pub mod qualifier;
pub mod reader;
pub mod resource;
mod state;
pub mod strings;
pub mod svg;
pub mod vector;

pub use caches::{ResourceCaches, run_blocking};
pub use environment::{
    ComposeEnvironment, LocalComposeEnvironment, ResourceEnvironment, ResourceSelectionError,
    default_remember_environment, get_system_resource_environment, remember_resource_environment,
    set_resource_environment_provider, system_environment,
};
pub use font::{FontResourceError, default_font, font_resource, get_font_resource_bytes};
pub use image::{
    ImageResourceError, decode_to_image_bitmap, decode_to_svg_painter, get_drawable_resource_bytes, image_resource,
    painter_resource, vector_resource,
};
pub use plural::{PluralCategory, PluralRule, PluralRuleList, PluralRuleParseException};
pub use qualifier::{DensityQualifier, LanguageQualifier, Qualifier, RegionQualifier, ThemeQualifier};
pub use reader::{
    DefaultResourceReader, LocalResourceReader, MissingResourceException, ResourceFuture, ResourceReader,
    ResourceReaderRef, default_resource_reader, drawable_of_package, get_resource_uri, read_resource_bytes,
};
pub use resource::{
    DrawableResource, FontResource, PluralStringResource, Resource, ResourceItem, StringArrayResource, StringResource,
};
pub use strings::{
    StringResourceError, get_plural_string, get_plural_string_formatted, get_plural_string_formatted_in,
    get_plural_string_in, get_string, get_string_array, get_string_array_in, get_string_formatted,
    get_string_formatted_in, get_string_in, plural_string_resource, plural_string_resource_formatted,
    string_array_resource, string_resource, string_resource_formatted,
};
pub use svg::{SvgPainter, SvgParseError};
pub use vector::xmldom::MalformedXMLException;
pub use vector::{VectorParseError, decode_to_image_vector};

/// `Res`: the generated accessors of a `composeResources/` directory (CMP's generated `Res` class), at build time
/// (see [the macro's own docs](kompoz_macros::compose_resources)).
pub use kompoz_macros::compose_resources;

/// What an app writes with: the resource types, the composables and suspending readers, the macro (its modules
/// left out, which share names with other crates').
pub mod prelude {
    pub use crate::{
        ComposeEnvironment, DensityQualifier, DrawableResource, FontResource, LanguageQualifier,
        LocalComposeEnvironment, LocalResourceReader, MissingResourceException, PluralStringResource, RegionQualifier,
        ResourceCaches, ResourceEnvironment, ResourceReader, ResourceReaderRef, StringArrayResource, StringResource,
        ThemeQualifier, compose_resources, decode_to_image_vector, decode_to_svg_painter, font_resource,
        get_drawable_resource_bytes, get_font_resource_bytes, get_plural_string, get_plural_string_formatted,
        get_plural_string_formatted_in, get_plural_string_in, get_string, get_string_array, get_string_array_in,
        get_string_formatted, get_string_formatted_in, get_string_in, get_system_resource_environment, image_resource,
        painter_resource, plural_string_resource, plural_string_resource_formatted, remember_resource_environment,
        string_array_resource, string_resource, string_resource_formatted, vector_resource,
    };
}

/// What the generated code names (CMP's `@InternalResourceApi`).
#[doc(hidden)]
pub mod __private {
    pub use crate::reader::{EmbeddedFile, register_drawables, register_embedded_files};
    pub use std::sync::OnceLock;
}
