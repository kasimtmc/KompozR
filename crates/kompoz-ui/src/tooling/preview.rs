//! Compose's ui-tooling-preview (1.12.1) data sources: [`PreviewParameterProvider`] (what gives a preview its
//! parameter's values) and the providers of `androidx.compose.ui.tooling.preview.datasource` — [`LoremIpsum`] (a
//! text of placeholder words, which Compose's samples show in scrolling content) and
//! [`CollectionPreviewParameterProvider`]. KompozR has no previews in an IDE (its `@Preview` annotations are taken and
//! ignored in Kotlin's syntax); these are for the code that uses them, as Compose's samples do.
//!
//! Kotlin's `values` is a lazy `Sequence`; KompozR has no sequences, so `values()` is the list of the values (the
//! same values in order; `LoremIpsum().values.first()` is the same text).

/// Compose's package `androidx.compose.ui.tooling.preview.datasource`.
pub mod datasource {
    pub use super::{CollectionPreviewParameterProvider, LoremIpsum};
}

/// What gives a preview's parameter its values (Compose's interface `PreviewParameterProvider<T>`).
pub trait PreviewParameterProvider<T> {
    /// The values, in order (Compose's `values`, a `Sequence` there).
    fn values(&self) -> Vec<T>;

    /// How many values there are (Compose's `count`).
    fn count(&self) -> i32 {
        self.values().len() as i32
    }

    /// The name the preview shows for the value at `index`, none for the default (Compose's `getDisplayName`).
    fn get_display_name(&self, index: i32) -> Option<String> {
        let _ = index;
        None
    }
}

/// The words a [`LoremIpsum`] repeats (Compose's private `LOREM_IPSUM_SOURCE`, split at its spaces).
const LOREM_IPSUM_SOURCE: &str = concat!(
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ",
    "Integer sodales laoreet commodo. ",
    "Phasellus a purus eu risus elementum consequat. ",
    "Aenean eu elit ut nunc convallis laoreet non ut libero. ",
    "Suspendisse interdum placerat risus vel ornare. ",
    "Donec vehicula, turpis sed consectetur ullamcorper, ",
    "ante nunc egestas quam, ultricies adipiscing velit enim at nunc. ",
    "Aenean id diam neque. ",
    "Praesent ut lacus sed justo viverra fermentum et ut sem. ",
    "Fusce convallis gravida lacinia. ",
    "Integer semper dolor ut elit sagittis lacinia. ",
    "Praesent sodales scelerisque eros at rhoncus. ",
    "Duis posuere sapien vel ipsum ornare interdum at eu quam. ",
    "Vestibulum vel massa erat. Aenean quis sagittis purus. ",
    "Phasellus arcu purus, rutrum id consectetur non, bibendum at nibh. ",
    "\n\n",
    "Duis nec erat dolor. Nulla vitae consectetur ligula. ",
    "Quisque nec mi est. ",
    "Ut quam ante, rutrum at pellentesque gravida, pretium in dui. ",
    "Cras eget sapien velit. ",
    "Suspendisse ut sem nec tellus vehicula eleifend sit amet quis velit. ",
    "Phasellus quis suscipit nisi. Nam elementum malesuada tincidunt. ",
    "Curabitur iaculis pretium eros, malesuada faucibus leo eleifend a. ",
    "Curabitur congue orci in neque euismod a blandit libero vehicula."
);

/// A provider of one value, a text of `words` placeholder words (Compose's `LoremIpsum`, of 500 words without an
/// argument): "Lorem ipsum dolor sit amet, …", the source's words over again as many times as it takes, joined by
/// spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoremIpsum {
    words: i32,
}

impl LoremIpsum {
    /// Kotlin: `LoremIpsum(words: Int)`
    pub fn new(words: i32) -> LoremIpsum {
        LoremIpsum { words }
    }

    /// Its one value: the text (Compose's `override val values`, a member of the class itself, which Kotlin code
    /// calls on a `LoremIpsum`: `LoremIpsum().values.first()`).
    pub fn values(&self) -> Vec<String> {
        vec![LoremIpsum::generate_lorem_ipsum(self.words)]
    }

    /// Generate a Lorem Ipsum `words` long (Compose's private `generateLoremIpsum`).
    fn generate_lorem_ipsum(words: i32) -> String {
        let source: Vec<&str> = LOREM_IPSUM_SOURCE.split(' ').collect();
        source.iter().cycle().take(words.max(0) as usize).copied().collect::<Vec<_>>().join(" ")
    }
}

/// Compose's `LoremIpsum()`: 500 words.
impl Default for LoremIpsum {
    fn default() -> LoremIpsum {
        LoremIpsum::new(500)
    }
}

/// A Lorem Ipsum of 500 words (Compose's constructor `LoremIpsum()`).
///
/// Kotlin: `LoremIpsum()`
#[allow(non_snake_case)]
pub fn LoremIpsum() -> LoremIpsum {
    LoremIpsum::default()
}

impl PreviewParameterProvider<String> for LoremIpsum {
    fn values(&self) -> Vec<String> {
        LoremIpsum::values(self)
    }
}

/// A provider of a collection's values (Compose's `CollectionPreviewParameterProvider<T>`).
#[derive(Clone, Debug, PartialEq)]
pub struct CollectionPreviewParameterProvider<T> {
    collection: Vec<T>,
}

impl<T: Clone> CollectionPreviewParameterProvider<T> {
    /// Kotlin: `CollectionPreviewParameterProvider(collection: Collection<T>)`
    pub fn new(collection: Vec<T>) -> CollectionPreviewParameterProvider<T> {
        CollectionPreviewParameterProvider { collection }
    }

    /// The collection's values, in order (Compose's `override val values`, a member of the class itself).
    pub fn values(&self) -> Vec<T> {
        self.collection.clone()
    }
}

impl<T: Clone> PreviewParameterProvider<T> for CollectionPreviewParameterProvider<T> {
    fn values(&self) -> Vec<T> {
        CollectionPreviewParameterProvider::values(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lorem_ipsum_repeats_its_words_as_compose() {
        let text = LoremIpsum().values();
        assert_eq!(text.len(), 1);
        let words: Vec<&str> = text[0].split(' ').collect();
        assert_eq!(words.len(), 500);
        assert!(text[0].starts_with("Lorem ipsum dolor sit amet, consectetur adipiscing elit. Integer sodales"));
        // The paragraph break starts the word after it (Kotlin's `trimIndent()` leaves the source as it is).
        assert!(text[0].contains("nibh. \n\nDuis nec erat dolor."));
        // Past the source's end it starts again.
        let source_words = LOREM_IPSUM_SOURCE.split(' ').count();
        assert_eq!(words[source_words], "Lorem");
        assert_eq!(LoremIpsum::new(3).values(), ["Lorem ipsum dolor"]);
        assert_eq!(LoremIpsum::new(0).values(), [""]);
        assert_eq!(LoremIpsum().count(), 1);
        assert_eq!(LoremIpsum().get_display_name(0), None);
        let collection = CollectionPreviewParameterProvider::new(vec![1, 2, 3]);
        assert_eq!(collection.values(), [1, 2, 3]);
        assert_eq!(collection.count(), 3);
        // (the providers' own `values` and the interface's are the same)
        assert_eq!(PreviewParameterProvider::values(&LoremIpsum::new(2)), LoremIpsum::new(2).values());
        assert_eq!(PreviewParameterProvider::values(&collection), collection.values());
    }
}
