//! String resources (CMP's StringResources.kt, PluralStringResources.kt, StringArrayResources.kt,
//! StringResourcesUtils.kt): a values file's strings, quantity strings and string arrays, read from the record the
//! build converted it into (`type|key|base64`: one line of `values*/<file>.commonMain.cvr`, at its offset), cached
//! by record; `%1$s`/`%1$d` placeholders replaced by the arguments' texts.

use crate::caches::AsyncCache;
use crate::environment::{ResourceEnvironment, ResourceSelectionError, get_system_resource_environment};
use crate::plural::{PluralCategory, PluralRuleList};
use crate::reader::{DefaultResourceReader, LocalResourceReader, MissingResourceException, ResourceReader};
use crate::resource::{PluralStringResource, ResourceItem, StringArrayResource, StringResource};
use crate::state::remember_resource_state;
use kompoz_runtime::kompozable;
use kompoz_runtime::kt::KtToString;
use std::collections::HashMap;
use std::fmt;

/// Why a string resource can't be read: its file (CMP's MissingResourceException), its item (IllegalStateException),
/// its record of another kind than asked (ClassCastException), a quantity without a string
/// (IllegalStateException).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StringResourceError {
    Missing(MissingResourceException),
    Selection(ResourceSelectionError),
    WrongType(String),
    NoQuantity(String),
}

impl fmt::Display for StringResourceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StringResourceError::Missing(e) => e.fmt(f),
            StringResourceError::Selection(e) => e.fmt(f),
            StringResourceError::WrongType(m) | StringResourceError::NoQuantity(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for StringResourceError {}

impl From<MissingResourceException> for StringResourceError {
    fn from(e: MissingResourceException) -> Self {
        StringResourceError::Missing(e)
    }
}

impl From<ResourceSelectionError> for StringResourceError {
    fn from(e: ResourceSelectionError) -> Self {
        StringResourceError::Selection(e)
    }
}

/// `%N$s` and `%N$d` (CMP's `SimpleStringFormatRegex`) replaced by `args[N - 1]` (CMP's `replaceWithArgs`); an
/// index past the arguments panics, as Kotlin's IndexOutOfBoundsException.
pub(crate) fn replace_with_args(s: &str, args: &[String]) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut copied = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let digits = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            let after = i + 1 + digits;
            if digits > 0 && b.get(after) == Some(&b'$') && matches!(b.get(after + 1), Some(b'd' | b's')) {
                let n: usize = s[i + 1..after].parse().unwrap_or(usize::MAX);
                out.push_str(&s[copied..i]);
                let arg = n
                    .checked_sub(1)
                    .and_then(|k| args.get(k))
                    .unwrap_or_else(|| panic!("Index {} out of bounds for length {}", n as i64 - 1, args.len()));
                out.push_str(arg);
                i = after + 2;
                copied = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&s[copied..]);
    out
}

/// A record's value (CMP's `StringItem`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum StringItem {
    Value(String),
    Plurals(HashMap<PluralCategory, String>),
    Array(Vec<String>),
}

static STRING_ITEMS: AsyncCache<String, StringItem> = AsyncCache::new();

pub(crate) fn clear_cache() {
    STRING_ITEMS.clear();
}

/// Standard Base64 (with padding, as `java.util.Base64`'s encoder writes it) decoded; none where it isn't.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn value(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    }
    let s = s.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.chunks(4) {
        let mut n = 0u32;
        for (k, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * k);
        }
        match chunk.len() {
            4 => out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8]),
            3 => out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8]),
            2 => out.push((n >> 16) as u8),
            _ => return None,
        }
    }
    Some(out)
}

fn decode_text(b64: &str) -> String {
    String::from_utf8_lossy(&base64_decode(b64).unwrap_or_default()).into_owned()
}

/// A record (`type|key|data`) read (CMP's `getStringItem` body): its type's value of its data.
pub(crate) fn parse_record(record: &str) -> StringItem {
    let parts: Vec<&str> = record.split('|').collect();
    let (kind, data) = (parts[0], parts[parts.len() - 1]);
    match kind {
        "plurals" => StringItem::Plurals(
            data.split(',')
                .map(|item| {
                    let (category, value) = item.split_once(':').unwrap_or((item, item));
                    let category = PluralCategory::from_string(category)
                        .unwrap_or_else(|| panic!("NullPointerException: no plural category {category:?}"));
                    (category, decode_text(value))
                })
                .collect(),
        ),
        "string-array" => StringItem::Array(data.split(',').map(decode_text).collect()),
        _ => StringItem::Value(decode_text(data)),
    }
}

/// The record of `item` read through `reader` (CMP's `getStringItem`), cached by its file, offset and size.
pub(crate) async fn get_string_item(
    item: &ResourceItem,
    reader: &dyn ResourceReader,
) -> Result<StringItem, MissingResourceException> {
    let key = format!("{}/{}-{}", item.path, item.offset, item.size);
    STRING_ITEMS
        .get_or_load(key, || async {
            let bytes = reader.read_part(&item.path, item.offset, item.size).await?;
            Ok(parse_record(&String::from_utf8_lossy(&bytes)))
        })
        .await
}

fn args_of(format_args: &[&dyn KtToString]) -> Vec<String> {
    format_args.iter().map(|a| a.kt_to_string()).collect()
}

/// Panics with the error, as Kotlin throws it where a composable reads it.
fn or_throw<T>(result: Result<T, StringResourceError>) -> T {
    result.unwrap_or_else(|e| panic!("{e}"))
}

// ---- Strings -------------------------------------------------------------------------------------------------------

async fn load_string(
    resource: &StringResource,
    reader: &dyn ResourceReader,
    environment: &ResourceEnvironment,
) -> Result<String, StringResourceError> {
    let item = resource.get_resource_item_by_environment(environment)?;
    match get_string_item(item, reader).await? {
        StringItem::Value(text) => Ok(text),
        other => Err(StringResourceError::WrongType(format!(
            "class StringItem${} cannot be cast to class StringItem$Value ({})",
            kind_name(&other),
            resource.id()
        ))),
    }
}

fn kind_name(item: &StringItem) -> &'static str {
    match item {
        StringItem::Value(_) => "Value",
        StringItem::Plurals(_) => "Plurals",
        StringItem::Array(_) => "Array",
    }
}

/// The string of `resource` here (CMP's `stringResource(resource)`), in the environment's language. Panics if it
/// can't be read (Kotlin's exception).
#[kompozable]
pub fn string_resource(resource: StringResource) -> String {
    let reader = LocalResourceReader.current();
    let state = remember_resource_state(resource.clone(), String::new, move |env| async move {
        or_throw(load_string(&resource, &*reader, &env).await)
    });
    state.get()
}

/// The string of `resource` with its `%N$s`/`%N$d` placeholders replaced by `format_args`' texts (CMP's
/// `stringResource(resource, vararg formatArgs)`).
///
/// Kotlin: `stringResource(resource: StringResource, vararg formatArgs: Any): String`
#[kompozable]
pub fn string_resource_formatted<'a>(resource: StringResource, format_args: &'a [&'a dyn KtToString]) -> String {
    let reader = LocalResourceReader.current();
    let args = args_of(format_args);
    let state = remember_resource_state((resource.clone(), args.clone()), String::new, move |env| async move {
        or_throw(load_string(&resource, &*reader, &env).await.map(|s| replace_with_args(&s, &args)))
    });
    state.get()
}

/// The string of `resource` in the system's environment (CMP's suspending `getString(resource)`), through the
/// default reader.
///
/// Kotlin: `getString(resource: StringResource): String`
pub async fn get_string(resource: &StringResource) -> Result<String, StringResourceError> {
    load_string(resource, &DefaultResourceReader, &get_system_resource_environment()).await
}

/// The same in `environment` (CMP's `getString(environment, resource)`).
///
/// Kotlin: `getString(environment: ResourceEnvironment, resource: StringResource): String`
pub async fn get_string_in(
    environment: &ResourceEnvironment,
    resource: &StringResource,
) -> Result<String, StringResourceError> {
    load_string(resource, &DefaultResourceReader, environment).await
}

/// The string, formatted (CMP's `getString(resource, vararg formatArgs)`).
///
/// Kotlin: `getString(resource: StringResource, vararg formatArgs: Any): String`
pub async fn get_string_formatted(
    resource: &StringResource,
    format_args: &[&dyn KtToString],
) -> Result<String, StringResourceError> {
    let args = args_of(format_args);
    let s = load_string(resource, &DefaultResourceReader, &get_system_resource_environment()).await?;
    Ok(replace_with_args(&s, &args))
}

/// The string in `environment`, formatted (CMP's `getString(environment, resource, vararg formatArgs)`).
///
/// Kotlin: `getString(environment: ResourceEnvironment, resource: StringResource, vararg formatArgs: Any): String`
pub async fn get_string_formatted_in(
    environment: &ResourceEnvironment,
    resource: &StringResource,
    format_args: &[&dyn KtToString],
) -> Result<String, StringResourceError> {
    let args = args_of(format_args);
    let s = load_string(resource, &DefaultResourceReader, environment).await?;
    Ok(replace_with_args(&s, &args))
}

// ---- Quantity strings ----------------------------------------------------------------------------------------------

async fn load_plural_string(
    resource: &PluralStringResource,
    quantity: i32,
    reader: &dyn ResourceReader,
    environment: &ResourceEnvironment,
) -> Result<String, StringResourceError> {
    let item = resource.get_resource_item_by_environment(environment)?;
    let StringItem::Plurals(items) = get_string_item(item, reader).await? else {
        return Err(StringResourceError::WrongType(format!(
            "class StringItem cannot be cast to class StringItem$Plurals ({})",
            resource.id()
        )));
    };
    let rules = PluralRuleList::get_instance(&environment.language, &environment.region).await;
    let category = rules.get_category(quantity);
    items.get(&category).or_else(|| items.get(&PluralCategory::Other)).cloned().ok_or_else(|| {
        StringResourceError::NoQuantity(format!(
            "Quantity string ID=`{}` does not have the pluralization {} for quantity {quantity}!",
            resource.key(),
            category.name()
        ))
    })
}

/// The string of `resource` for `quantity` here (CMP's `pluralStringResource(resource, quantity)`): the
/// language's plural category of it (CLDR's rules), else its `other` string.
#[kompozable]
pub fn plural_string_resource(resource: PluralStringResource, quantity: i32) -> String {
    let reader = LocalResourceReader.current();
    let state = remember_resource_state((resource.clone(), quantity), String::new, move |env| async move {
        or_throw(load_plural_string(&resource, quantity, &*reader, &env).await)
    });
    state.get()
}

/// The same, formatted (CMP's `pluralStringResource(resource, quantity, vararg formatArgs)`).
///
/// Kotlin: `pluralStringResource(resource: PluralStringResource, quantity: Int, vararg formatArgs: Any): String`
#[kompozable]
pub fn plural_string_resource_formatted<'a>(
    resource: PluralStringResource,
    quantity: i32,
    format_args: &'a [&'a dyn KtToString],
) -> String {
    let reader = LocalResourceReader.current();
    let args = args_of(format_args);
    let keys = (resource.clone(), quantity, args.clone());
    let state = remember_resource_state(keys, String::new, move |env| async move {
        or_throw(load_plural_string(&resource, quantity, &*reader, &env).await.map(|s| replace_with_args(&s, &args)))
    });
    state.get()
}

/// Kotlin: `getPluralString(resource: PluralStringResource, quantity: Int): String`
pub async fn get_plural_string(resource: &PluralStringResource, quantity: i32) -> Result<String, StringResourceError> {
    load_plural_string(resource, quantity, &DefaultResourceReader, &get_system_resource_environment()).await
}

/// Kotlin: `getPluralString(environment: ResourceEnvironment, resource: PluralStringResource, quantity: Int): String`
pub async fn get_plural_string_in(
    environment: &ResourceEnvironment,
    resource: &PluralStringResource,
    quantity: i32,
) -> Result<String, StringResourceError> {
    load_plural_string(resource, quantity, &DefaultResourceReader, environment).await
}

/// Kotlin: `getPluralString(resource: PluralStringResource, quantity: Int, vararg formatArgs: Any): String`
pub async fn get_plural_string_formatted(
    resource: &PluralStringResource,
    quantity: i32,
    format_args: &[&dyn KtToString],
) -> Result<String, StringResourceError> {
    let args = args_of(format_args);
    let s = load_plural_string(resource, quantity, &DefaultResourceReader, &get_system_resource_environment()).await?;
    Ok(replace_with_args(&s, &args))
}

/// Kotlin: `getPluralString(environment: ResourceEnvironment, resource: PluralStringResource, quantity: Int, vararg formatArgs: Any): String`
pub async fn get_plural_string_formatted_in(
    environment: &ResourceEnvironment,
    resource: &PluralStringResource,
    quantity: i32,
    format_args: &[&dyn KtToString],
) -> Result<String, StringResourceError> {
    let args = args_of(format_args);
    let s = load_plural_string(resource, quantity, &DefaultResourceReader, environment).await?;
    Ok(replace_with_args(&s, &args))
}

// ---- String arrays -------------------------------------------------------------------------------------------------

async fn load_string_array(
    resource: &StringArrayResource,
    reader: &dyn ResourceReader,
    environment: &ResourceEnvironment,
) -> Result<Vec<String>, StringResourceError> {
    let item = resource.get_resource_item_by_environment(environment)?;
    match get_string_item(item, reader).await? {
        StringItem::Array(items) => Ok(items),
        _ => Err(StringResourceError::WrongType(format!(
            "class StringItem cannot be cast to class StringItem$Array ({})",
            resource.id()
        ))),
    }
}

/// The strings of `resource` here (CMP's `stringArrayResource(resource)`).
#[kompozable]
pub fn string_array_resource(resource: StringArrayResource) -> Vec<String> {
    let reader = LocalResourceReader.current();
    let state = remember_resource_state(resource.clone(), Vec::new, move |env| async move {
        or_throw(load_string_array(&resource, &*reader, &env).await)
    });
    state.get()
}

/// Kotlin: `getStringArray(resource: StringArrayResource): List<String>`
pub async fn get_string_array(resource: &StringArrayResource) -> Result<Vec<String>, StringResourceError> {
    load_string_array(resource, &DefaultResourceReader, &get_system_resource_environment()).await
}

/// Kotlin: `getStringArray(environment: ResourceEnvironment, resource: StringArrayResource): List<String>`
pub async fn get_string_array_in(
    environment: &ResourceEnvironment,
    resource: &StringArrayResource,
) -> Result<Vec<String>, StringResourceError> {
    load_string_array(resource, &DefaultResourceReader, environment).await
}
