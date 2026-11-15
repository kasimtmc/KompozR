//! The fonts text is drawn in, by number: what a glyph run names its font by, and what a renderer (on any
//! thread) reads the font's data back from.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, LazyLock, RwLock};

/// A font file's bytes, shared between threads however they are held: a `Vec`, an `Arc<[u8]>`, a file mapped
/// into memory by a font collection (what fontique's blobs and skrifa's fonts hold, without a copy).
pub type FontBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// A font's data: the file's bytes and which face of a collection it is.
#[derive(Clone)]
pub struct FontData {
    pub blob: FontBytes,
    pub index: u32,
}

impl FontData {
    /// The file's bytes.
    pub fn bytes(&self) -> &[u8] {
        (*self.blob).as_ref()
    }
}

/// Its size and index (not its bytes).
impl fmt::Debug for FontData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FontData({} bytes, index {})", self.bytes().len(), self.index)
    }
}

/// A font registered in the [`FontRegistry`]: a number standing for its data, the same for as long as the
/// process runs (glyph runs, caches and atlases key by it).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FontId(pub u64);

/// The fonts text has been resolved to, by id (the process's one registry): a backend registers each face it
/// shapes with, and whoever draws a glyph run reads the face's bytes back by the run's [`FontId`]. The same bytes
/// (the same memory, however it is held) and index give the same id. A font registered stays for the process's
/// life, as the font collections that hand fonts out keep them.
pub struct FontRegistry;

struct Registry {
    fonts: Vec<FontData>,
    by_data: HashMap<(usize, usize, u32), FontId>,
}

static REGISTRY: LazyLock<RwLock<Registry>> =
    LazyLock::new(|| RwLock::new(Registry { fonts: Vec::new(), by_data: HashMap::new() }));

impl FontRegistry {
    /// The id of face `index` of `blob` (registered now if it wasn't).
    pub fn register(blob: Arc<[u8]>, index: u32) -> FontId {
        FontRegistry::register_bytes(Arc::new(blob), index)
    }

    /// The id of face `index` of bytes held any way (registered now if they weren't): the same memory is the same
    /// font, whatever holds it.
    pub fn register_bytes(blob: FontBytes, index: u32) -> FontId {
        let bytes: &[u8] = (*blob).as_ref();
        // The bytes' address and length: registered fonts are kept, so no other font's bytes can be there.
        let key = (bytes.as_ptr() as usize, bytes.len(), index);
        if let Some(id) = REGISTRY.read().expect("font registry").by_data.get(&key) {
            return *id;
        }
        let mut r = REGISTRY.write().expect("font registry");
        if let Some(id) = r.by_data.get(&key) {
            return *id;
        }
        let id = FontId(r.fonts.len() as u64);
        r.fonts.push(FontData { blob, index });
        r.by_data.insert(key, id);
        id
    }

    /// The data of a registered font (None for an id no font was registered as).
    pub fn get(id: FontId) -> Option<FontData> {
        REGISTRY.read().expect("font registry").fonts.get(id.0 as usize).cloned()
    }
}
