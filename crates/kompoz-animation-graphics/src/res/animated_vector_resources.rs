//! Compose's `AnimatedImageVector.animatedVectorResource(id)` (AnimatedVectorResources.android.kt) on KompozR's
//! resources (kompoz-resources, CMP's model): an `<animated-vector>` drawable resource read for the composition's
//! environment, the drawables it refers to (`@drawable/name`) those of its own `Res` (picked for the same
//! environment), the framework's interpolators built in; read once while the resource, the reader and the
//! environment stay.

use crate::vector::{AnimatedImageVector, ResourceReference, XmlResourceError};
use kompoz_resources::{
    DrawableResource, LocalResourceReader, ResourceEnvironment, ResourceReader, drawable_of_package,
    remember_resource_environment, run_blocking,
};
use kompoz_runtime::{kompozable, remember_keyed};

/// The text of a drawable of the resources `path` is one of, for `environment`.
fn drawable_text(
    path: &str,
    name: &str,
    reader: &dyn ResourceReader,
    environment: &ResourceEnvironment,
) -> Option<String> {
    let drawable = drawable_of_package(path, name)?;
    let item = drawable.get_resource_item_by_environment(environment).ok()?;
    let bytes = run_blocking(reader.read(&item.path)).ok()?;
    String::from_utf8(bytes).ok()
}

/// `resource`'s animated vector read for `environment` through `reader`: its file, then what it refers to.
pub fn load_animated_vector_resource(
    resource: &DrawableResource,
    reader: &dyn ResourceReader,
    environment: &ResourceEnvironment,
) -> Result<AnimatedImageVector, XmlResourceError> {
    let not_found = |m: String| XmlResourceError { exception: "NotFoundException", message: m };
    let item = resource.get_resource_item_by_environment(environment).map_err(|e| not_found(e.to_string()))?;
    let path = item.path.clone();
    let bytes = run_blocking(reader.read(&path)).map_err(|e| not_found(e.to_string()))?;
    let xml = String::from_utf8(bytes).map_err(|_| XmlResourceError {
        exception: "XmlPullParserException",
        message: format!("{path} isn't UTF-8"),
    })?;
    let resources = |r: &ResourceReference| -> Option<String> {
        if r.package.is_some() || r.is_theme_attribute || r.r#type != "drawable" {
            return None;
        }
        drawable_text(&path, &r.name, reader, environment)
    };
    AnimatedImageVector::from_animated_vector_xml_with(&xml, &resources)
}

/// An animated vector drawable resource, read for the composition's environment (Compose's
/// `AnimatedImageVector.animatedVectorResource(@DrawableRes id)`, here of a `DrawableResource` of a generated
/// `Res`), kept while it, the reader and the environment stay. Panics if it can't be read (Compose's exception).
///
/// Kotlin: `AnimatedImageVector.animatedVectorResource(resource: DrawableResource): AnimatedImageVector`
#[kompozable]
pub fn animated_vector_resource(resource: DrawableResource) -> AnimatedImageVector {
    let reader = LocalResourceReader.current();
    let environment = remember_resource_environment();
    remember_keyed((resource.clone(), reader.clone(), environment.clone()), move || {
        load_animated_vector_resource(&resource, &*reader, &environment).unwrap_or_else(|e| panic!("{e}"))
    })
}
