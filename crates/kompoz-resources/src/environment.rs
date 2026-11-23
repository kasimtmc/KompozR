//! The resource environment (CMP's ResourceEnvironment.kt): the language, region, theme and density resources are
//! picked for — a composition's (its UI language: KompozR's [`LocalStrings`]' locale, as Material's texts follow
//! it; `is_system_in_dark_theme`; `LocalDensity`), or the system's outside one — and how an item is picked by them
//! (Android's order: locale, theme, density, the best density the nearest at or above the screen's).

use crate::qualifier::{DensityQualifier, LanguageQualifier, Qualifier, RegionQualifier, ThemeQualifier};
use crate::resource::{Resource, ResourceItem};
use kompoz_runtime::{KompozitionLocal, kompozable, remember_keyed};
use kompoz_text::intl::{Locale, LocaleList};
use kompoz_ui::platform::{LocalDensity, SystemTheme, current_system_density, current_system_theme};
use kompoz_ui::strings::LocalStrings;
use std::fmt;
use std::rc::Rc;
use std::sync::RwLock;

/// What resources are picked for (CMP's ResourceEnvironment): a language, a region, a theme, a density.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceEnvironment {
    pub language: LanguageQualifier,
    pub region: RegionQualifier,
    pub theme: ThemeQualifier,
    pub density: DensityQualifier,
}

impl ResourceEnvironment {
    /// Kotlin: `ResourceEnvironment(language: LanguageQualifier, region: RegionQualifier, theme: ThemeQualifier, density: DensityQualifier)`
    pub fn new(
        language: LanguageQualifier,
        region: RegionQualifier,
        theme: ThemeQualifier,
        density: DensityQualifier,
    ) -> ResourceEnvironment {
        ResourceEnvironment { language, region, theme, density }
    }

    /// The environment of a locale, a theme and a density (px a dp).
    pub fn of(locale: &Locale, is_dark: bool, density: f32) -> ResourceEnvironment {
        ResourceEnvironment::new(
            LanguageQualifier::new(locale.language()),
            RegionQualifier::new(locale.region()),
            ThemeQualifier::select_by_value(is_dark),
            DensityQualifier::select_by_density(density),
        )
    }
}

/// How a composition's environment is made (CMP's internal `ComposeEnvironment`, provided by tests): a composable
/// function, called where resources are read.
#[derive(Clone)]
pub struct ComposeEnvironment(Rc<dyn Fn() -> ResourceEnvironment>);

impl ComposeEnvironment {
    /// The environment `remember_environment` gives (a composable function: it may read composition locals and
    /// remember).
    pub fn new(remember_environment: impl Fn() -> ResourceEnvironment + 'static) -> ComposeEnvironment {
        ComposeEnvironment(Rc::new(remember_environment))
    }
    /// One environment always (what tests provide).
    pub fn fixed(environment: ResourceEnvironment) -> ComposeEnvironment {
        ComposeEnvironment::new(move || environment.clone())
    }
    /// The environment here (Kotlin's `rememberEnvironment()`).
    #[track_caller]
    pub fn remember_environment(&self) -> ResourceEnvironment {
        (self.0)()
    }
}

impl PartialEq for ComposeEnvironment {
    fn eq(&self, other: &ComposeEnvironment) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for ComposeEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("ComposeEnvironment")
    }
}

/// The composition's environment (CMP's `DefaultComposeEnvironment`): its UI language ([`LocalStrings`]' locale;
/// CMP reads Compose's `Locale.current`, which KompozR's host gives as that locale), its theme
/// (`is_system_in_dark_theme`), its density (`LocalDensity`), remembered while they stay.
#[kompozable]
pub fn default_remember_environment() -> ResourceEnvironment {
    let locale = Locale::new(&LocalStrings.current().locale);
    let dark = kompoz_ui::platform::is_system_in_dark_theme();
    let density = LocalDensity.current();
    remember_keyed((locale.clone(), dark, density.density.to_bits()), move || {
        ResourceEnvironment::of(&locale, dark, density.density)
    })
}

thread_local! {
    static DEFAULT_COMPOSE_ENVIRONMENT: ComposeEnvironment = ComposeEnvironment::new(default_remember_environment);
}

fn default_compose_environment() -> ComposeEnvironment {
    DEFAULT_COMPOSE_ENVIRONMENT.with(Clone::clone)
}

/// How resources' environment is made where they are read (CMP's `LocalComposeEnvironment`, which its tests
/// provide; KompozR's tests do the same: `LocalComposeEnvironment.provides(ComposeEnvironment::fixed(…))`).
pub static LocalComposeEnvironment: KompozitionLocal<ComposeEnvironment> =
    KompozitionLocal::new_static(default_compose_environment);

/// The environment here (CMP's `rememberResourceEnvironment()`).
#[kompozable]
pub fn remember_resource_environment() -> ResourceEnvironment {
    LocalComposeEnvironment.current().remember_environment()
}

/// What the system's environment is, outside a composition (CMP's `getSystemEnvironment`, which its tests
/// replace): the first of the user's locales, the system's theme as the platform told a host (light until then),
/// the screen's density as it told it (MDPI without one: headless, as CMP's desktop default).
pub fn system_environment() -> ResourceEnvironment {
    let locale = LocaleList::current().get(0).cloned().unwrap_or_else(|| Locale::new("und"));
    let dark = current_system_theme() == SystemTheme::Dark;
    let density = match current_system_density() {
        Some(d) => DensityQualifier::select_by_density(d),
        None => DensityQualifier::Mdpi,
    };
    ResourceEnvironment::new(
        LanguageQualifier::new(locale.language()),
        RegionQualifier::new(locale.region()),
        ThemeQualifier::select_by_value(dark),
        density,
    )
}

type EnvironmentProvider = fn() -> ResourceEnvironment;

static GET_RESOURCE_ENVIRONMENT: RwLock<EnvironmentProvider> = RwLock::new(system_environment);

/// What [`get_system_resource_environment`] asks from now on (CMP's `getResourceEnvironment` reference, which its
/// tests override); [`system_environment`] by default.
pub fn set_resource_environment_provider(provider: EnvironmentProvider) {
    *GET_RESOURCE_ENVIRONMENT.write().unwrap_or_else(|e| e.into_inner()) = provider;
}

/// The resource environment for code outside a composition (CMP's `getSystemResourceEnvironment()`); asks the
/// system each time — not for a composable to call uncached.
pub fn get_system_resource_environment() -> ResourceEnvironment {
    let provider = *GET_RESOURCE_ENVIRONMENT.read().unwrap_or_else(|e| e.into_inner());
    provider()
}

/// Why no item is picked: none fits, or more than one does (CMP's `error(…)`: IllegalStateException).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResourceSelectionError(pub String);

impl fmt::Display for ResourceSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ResourceSelectionError {}

impl Resource {
    /// The item for `environment` (CMP's `getResourceItemByEnvironment`): by locale (the exact language and region,
    /// else the language alone, else none), then by theme, then by density, as Android picks
    /// (developer.android.com/guide/topics/resources/providing-resources#BestMatch).
    pub fn get_resource_item_by_environment(
        &self,
        environment: &ResourceEnvironment,
    ) -> Result<&ResourceItem, ResourceSelectionError> {
        let items: Vec<&ResourceItem> = self.items().iter().collect();
        let items = filter_by_locale(items, &environment.language, &environment.region);
        if items.len() == 1 {
            return Ok(items[0]);
        }
        let items = filter_by(items, &Qualifier::Theme(environment.theme));
        if items.len() == 1 {
            return Ok(items[0]);
        }
        let items = filter_by_density(items, environment.density);
        if items.len() == 1 {
            return Ok(items[0]);
        }
        if items.is_empty() {
            Err(ResourceSelectionError(format!("Resource with ID='{}' not found", self.id())))
        } else {
            let paths: Vec<&str> = items.iter().map(|i| i.path.as_str()).collect();
            Err(ResourceSelectionError(format!(
                "Resource with ID='{}' has more than one file: {}",
                self.id(),
                paths.join(", ")
            )))
        }
    }
}

fn same_kind(a: &Qualifier, b: &Qualifier) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// The items with `qualifier`, else those with none of its kind (the default).
fn filter_by<'a>(items: Vec<&'a ResourceItem>, qualifier: &Qualifier) -> Vec<&'a ResourceItem> {
    let with: Vec<_> = items.iter().copied().filter(|i| i.qualifiers.contains(qualifier)).collect();
    if !with.is_empty() {
        return with;
    }
    items.into_iter().filter(|i| !i.qualifiers.iter().any(|q| same_kind(q, qualifier))).collect()
}

/// Android's preference for scaling a larger image down over scaling a smaller one up: the nearest density at or
/// above, else the nearest below (LDPI aside), else the default (no density: MDPI's), else LDPI.
fn filter_by_density(items: Vec<&ResourceItem>, density: DensityQualifier) -> Vec<&ResourceItem> {
    let with = |q: DensityQualifier| -> Vec<&ResourceItem> {
        items.iter().copied().filter(|i| i.qualifiers.contains(&Qualifier::Density(q))).collect()
    };
    let mut higher: Vec<_> = DensityQualifier::entries.into_iter().filter(|q| q.dpi() >= density.dpi()).collect();
    higher.sort_by_key(|q| q.dpi());
    for q in higher {
        let found = with(q);
        if !found.is_empty() {
            return found;
        }
    }
    let mut lower: Vec<_> = DensityQualifier::entries
        .into_iter()
        .filter(|q| *q != DensityQualifier::Ldpi && q.dpi() < density.dpi())
        .collect();
    lower.sort_by_key(|q| std::cmp::Reverse(q.dpi()));
    for q in lower {
        let found = with(q);
        if !found.is_empty() {
            return found;
        }
    }
    let none: Vec<_> =
        items.iter().copied().filter(|i| !i.qualifiers.iter().any(|q| matches!(q, Qualifier::Density(_)))).collect();
    if !none.is_empty() {
        return none;
    }
    with(DensityQualifier::Ldpi)
}

/// Language and region together (CMP issue 4571): the exact language and region, else the language without a
/// region, else the items without either.
fn filter_by_locale<'a>(
    items: Vec<&'a ResourceItem>,
    language: &LanguageQualifier,
    region: &RegionQualifier,
) -> Vec<&'a ResourceItem> {
    let language = Qualifier::Language(language.clone());
    let region = Qualifier::Region(region.clone());
    let with_language: Vec<_> = items.iter().copied().filter(|i| i.qualifiers.contains(&language)).collect();
    let exact: Vec<_> = with_language.iter().copied().filter(|i| i.qualifiers.contains(&region)).collect();
    if !exact.is_empty() {
        return exact;
    }
    let default_region: Vec<_> =
        with_language.into_iter().filter(|i| !i.qualifiers.iter().any(|q| matches!(q, Qualifier::Region(_)))).collect();
    if !default_region.is_empty() {
        return default_region;
    }
    items
        .into_iter()
        .filter(|i| !i.qualifiers.iter().any(|q| matches!(q, Qualifier::Language(_) | Qualifier::Region(_))))
        .collect()
}
