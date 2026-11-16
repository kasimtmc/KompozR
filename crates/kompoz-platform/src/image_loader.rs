//! Images for `AsyncImage` and image resources (C++'s common/image_loader): read from the app's files or fetched
//! from an http(s) URL, decoded (kompoz-graphics' PNG, JPEG, WebP, BMP decoders, scaled down as they are asked for),
//! on a few worker threads; what was downloaded kept in a disk cache in the app's cache directory ([`DiskCache`],
//! the least recently used going first) for the next time and the next run. Fetching is the platform's
//! ([`set_http_client`]: libcurl on Linux); without a client, URLs that aren't in the cache don't load.

use crate::system::{app_cache_directory, log, read_file};
use kompoz_graphics::{ImageBitmap, ImageRequestSize as DecodeSize, decode_image, save_png};
use kompoz_ui::platform::ImageRequestSize;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::time::SystemTime;

// ---- The disk cache ----------------------------------------------------------------------------------------------

/// Each file starts with this mark, then the key's length (7 bits a byte, the high bit: more), then the key (two
/// keys with the same file name aren't mixed up), then the bytes.
const MARK: &[u8; 4] = b"KZD1";

/// A file name of a key: its FNV-1a hash, 16 hex digits.
fn file_name(key: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn is_hex(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// An entry's file name.
fn is_entry(name: &str) -> bool {
    name.len() == 16 && is_hex(name)
}

/// What a write leaves until it is renamed into place ("<entry>.<n>.tmp").
fn is_temp(name: &str) -> bool {
    name.len() > 21 && is_hex(&name[..16]) && name.as_bytes()[16] == b'.' && name.ends_with(".tmp")
}

fn header(key: &str) -> Vec<u8> {
    let mut out = MARK.to_vec();
    let mut n = key.len() as u64;
    loop {
        out.push((n & 0x7f) as u8 | if n >= 0x80 { 0x80 } else { 0 });
        if n < 0x80 {
            break;
        }
        n >>= 7;
    }
    out.extend_from_slice(key.as_bytes());
    out
}

#[derive(Clone, Copy)]
struct Entry {
    bytes: u64,
    used: u64,
}

struct CacheState {
    opened: bool,
    files: HashMap<String, Entry>,
    total: u64,
    clock: u64,
    temp: u64,
}

/// Files kept in a directory by key, the least recently used removed once they add up to more than a size (C++'s
/// DiskCache, Coil's): where downloaded images are kept, so they show again without the network, after the app is
/// restarted too. Any thread may use it.
pub struct DiskCache {
    directory: PathBuf,
    max_bytes: u64,
    state: Mutex<CacheState>,
}

impl DiskCache {
    /// A cache in `directory` (made when first used), up to `max_bytes`.
    pub fn new(directory: PathBuf, max_bytes: u64) -> DiskCache {
        DiskCache {
            directory,
            max_bytes,
            state: Mutex::new(CacheState { opened: false, files: HashMap::new(), total: 0, clock: 0, temp: 0 }),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CacheState> {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.open(&mut s);
        s
    }

    /// On first use: what is already there, the oldest first; writes left unfinished removed.
    fn open(&self, s: &mut CacheState) {
        if s.opened {
            return;
        }
        s.opened = true;
        let _ = fs::create_dir_all(&self.directory);
        let mut found: Vec<(String, u64, SystemTime)> = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.directory) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Ok(meta) = entry.metadata() else { continue };
                if !meta.is_file() {
                    continue;
                }
                if is_temp(&name) {
                    let _ = fs::remove_file(entry.path());
                } else if is_entry(&name) {
                    found.push((name, meta.len(), meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
                }
            }
        }
        found.sort_by_key(|f| f.2);
        for (name, bytes, _) in found {
            s.clock += 1;
            s.files.insert(name, Entry { bytes, used: s.clock });
            s.total += bytes;
        }
        self.trim(s);
    }

    /// The bytes kept for `key` (which counts as used now), if any.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut s = self.lock();
        let name = file_name(key);
        s.files.get(&name)?;
        let path = self.directory.join(&name);
        let expected = header(key);
        let mut head = vec![0u8; expected.len()];
        let read = fs::File::open(&path).and_then(|mut f| {
            f.read_exact(&mut head)?;
            let mut rest = Vec::new();
            f.read_to_end(&mut rest)?;
            Ok(rest)
        });
        let Ok(bytes) = read else {
            forget(&mut s, &name); // gone, or cut short
            return None;
        };
        if head != expected {
            if &head[..4] != MARK {
                forget(&mut s, &name);
            }
            return None; // another key's (or not ours)
        }
        s.clock += 1;
        let used = s.clock;
        if let Some(e) = s.files.get_mut(&name) {
            e.used = used;
        }
        // (The order, for the next run.)
        if let Ok(f) = fs::File::options().write(true).open(&path) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(bytes)
    }

    /// Keeps `bytes` for `key` (none if they would never fit), the least recently used going while too much is
    /// kept.
    pub fn put(&self, key: &str, bytes: &[u8]) {
        let head = header(key);
        let size = (head.len() + bytes.len()) as u64;
        if size > self.max_bytes {
            return;
        }
        let mut s = self.lock();
        let name = file_name(key);
        // Written beside it and renamed into place: a reader (or the next run, after a crash) never sees half of it.
        s.temp += 1;
        let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        let temp = self.directory.join(format!("{name}.{}.tmp", stamp % 1_000_000_007 + s.temp));
        let written = fs::File::create(&temp).and_then(|mut f| {
            f.write_all(&head)?;
            f.write_all(bytes)?;
            f.flush()
        });
        let path = self.directory.join(&name);
        if written.is_err() || fs::rename(&temp, &path).is_err() {
            let _ = fs::remove_file(&temp);
            return;
        }
        if let Some(old) = s.files.get(&name).copied() {
            s.total -= old.bytes;
        }
        s.clock += 1;
        let used = s.clock;
        s.files.insert(name, Entry { bytes: size, used });
        s.total += size;
        self.trim(&mut s);
    }

    /// Forgets `key`.
    pub fn remove(&self, key: &str) {
        let mut s = self.lock();
        let name = file_name(key);
        if s.files.contains_key(&name) {
            let _ = fs::remove_file(self.directory.join(&name));
            forget(&mut s, &name);
        }
    }

    /// Forgets everything.
    pub fn clear(&self) {
        let mut s = self.lock();
        for name in s.files.keys() {
            let _ = fs::remove_file(self.directory.join(name));
        }
        s.files.clear();
        s.total = 0;
    }

    /// The bytes kept.
    pub fn size(&self) -> u64 {
        self.lock().total
    }

    fn trim(&self, s: &mut CacheState) {
        if s.total <= self.max_bytes {
            return;
        }
        let mut by_use: Vec<(u64, String)> = s.files.iter().map(|(n, e)| (e.used, n.clone())).collect();
        by_use.sort();
        for (_, name) in by_use {
            if s.total <= self.max_bytes {
                break;
            }
            let _ = fs::remove_file(self.directory.join(&name));
            forget(s, &name);
        }
    }
}

fn forget(s: &mut CacheState, name: &str) {
    if let Some(e) = s.files.remove(name) {
        s.total -= e.bytes;
    }
}

// ---- Fetching, decoding ----------------------------------------------------------------------------------------

/// What fetches an http(s) URL's body (none if it failed): the platform's HTTP client.
pub type HttpGet = fn(&str) -> Option<Vec<u8>>;

/// The platform's own HTTP client, used until [`set_http_client`] says otherwise: libcurl on Linux (as C++ Kompoz's
/// Linux builds link it, the headless platform's too), loaded when a URL is first fetched.
#[cfg(target_os = "linux")]
const PLATFORM_HTTP: Option<HttpGet> = Some(crate::linux::curl::http_get);
#[cfg(not(target_os = "linux"))]
const PLATFORM_HTTP: Option<HttpGet> = None;

static HTTP: RwLock<Option<HttpGet>> = RwLock::new(PLATFORM_HTTP);
static DISK: RwLock<Option<Arc<DiskCache>>> = RwLock::new(None);
static STOPPING: AtomicBool = AtomicBool::new(false);

/// The HTTP client from now on (the platform's own until then: Linux's libcurl); none: URLs load only from the disk
/// cache.
pub fn set_http_client(get: Option<HttpGet>) {
    *HTTP.write().unwrap_or_else(|e| e.into_inner()) = get;
}

/// Whether the app is quitting (what a long download checks to stop early).
pub fn stopping() -> bool {
    STOPPING.load(Ordering::Relaxed)
}

fn is_url(model: &str) -> bool {
    model.starts_with("http://") || model.starts_with("https://")
}

fn decode_size(size: ImageRequestSize) -> DecodeSize {
    DecodeSize { width: size.width, height: size.height, fill: size.fill }
}

/// An image file's bytes decoded for `size` (C++'s `decodeImage`): scaled down as the request allows.
pub fn decode(bytes: &[u8], size: ImageRequestSize) -> Option<ImageBitmap> {
    decode_image(bytes, decode_size(size))
}

/// `KOMPOZ_IMAGE_LOG`: where each image came from, how big.
fn image_log() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("KOMPOZ_IMAGE_LOG").is_some())
}

/// `model` (a path or an http(s) URL) loaded now, for `size` (C++'s `loadNow`): a URL from the disk cache, else
/// fetched (and kept if it decodes).
pub fn load_now(model: &str, size: ImageRequestSize) -> Option<ImageBitmap> {
    let disk = if is_url(model) { DISK.read().unwrap_or_else(|e| e.into_inner()).clone() } else { None };
    let mut from = "file";
    let mut bytes = None;
    if let Some(disk) = &disk
        && let Some(kept) = disk.get(model)
    {
        bytes = Some(kept);
        from = "disk";
    }
    if bytes.is_none() {
        if is_url(model) {
            from = "network";
            let get = *HTTP.read().unwrap_or_else(|e| e.into_inner());
            match get {
                Some(get) => bytes = get(model),
                None => log(&format!("kompoz: no HTTP client in this build: {model}")),
            }
        } else {
            bytes = read_file(model);
        }
    }
    let Some(bytes) = bytes.filter(|b| !b.is_empty()) else {
        log(&format!("kompoz: can't load {model}"));
        return None;
    };
    let image = decode(&bytes, size);
    match (&image, &disk) {
        (None, Some(disk)) if from == "disk" => disk.remove(model),
        (Some(_), Some(disk)) if from == "network" => disk.put(model, &bytes), // (only what could be decoded)
        _ => {}
    }
    if image.is_none() {
        log(&format!("kompoz: can't decode {model}"));
    }
    if image_log() {
        let (w, h) = image.as_ref().map_or((0, 0), |i| (i.width(), i.height()));
        log(&format!(
            "kompoz: image {model} from {from}, {} bytes, decoded {w}x{h} for {}x{}{}",
            bytes.len(),
            size.width,
            size.height,
            if size.fill { " (fill)" } else { "" }
        ));
    }
    image
}

type Done = Box<dyn FnOnce(Option<ImageBitmap>) + Send>;

struct Job {
    model: String,
    size: ImageRequestSize,
    done: Done,
}

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

struct Loader {
    queue: Mutex<Queue>,
    wake: Condvar,
}

fn loader() -> &'static Loader {
    static LOADER: OnceLock<Loader> = OnceLock::new();
    LOADER.get_or_init(|| Loader { queue: Mutex::new(Queue::default()), wake: Condvar::new() })
}

/// A worker thread: loads in turn until the app quits.
fn work() {
    let l = loader();
    loop {
        let job = {
            let mut q = l.queue.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if stopping() {
                    return;
                }
                if let Some(job) = q.jobs.pop_front() {
                    break job;
                }
                q = l.wake.wait(q).unwrap_or_else(|e| e.into_inner());
            }
        };
        let image = load_now(&job.model, job.size);
        if !stopping() {
            (job.done)(image);
        }
    }
}

/// Loads `model` (a path or an http(s) URL) for `size` away from the UI thread and calls `done` with it, from a
/// worker thread (C++'s `loadImageAsync`): three workers take the loads in turn.
pub fn load_image_async(model: &str, size: ImageRequestSize, done: Done) {
    if stopping() {
        return;
    }
    let l = loader();
    let mut q = l.queue.lock().unwrap_or_else(|e| e.into_inner());
    if q.threads.is_empty() {
        for i in 0..3 {
            match std::thread::Builder::new().name(format!("kompoz-images-{i}")).spawn(work) {
                Ok(t) => q.threads.push(t),
                Err(_) => log("kompoz: can't start an image loader thread"),
            }
        }
    }
    q.jobs.push_back(Job { model: model.to_string(), size, done });
    l.wake.notify_one();
}

/// Where downloaded images are kept (C++'s `startImageDiskCache`): the app's cache directory's `images`
/// (`KOMPOZ_IMAGE_CACHE` instead, empty for none), up to `max_bytes` (0: none). Before the first load, as an app
/// starts (one stopped before in the process loads again).
pub fn start_image_disk_cache(app_name: &str, max_bytes: u64) {
    STOPPING.store(false, Ordering::Relaxed);
    let directory = match std::env::var_os("KOMPOZ_IMAGE_CACHE") {
        Some(own) => (!own.is_empty()).then(|| PathBuf::from(own)),
        None => app_cache_directory(app_name).map(|d| d.join("images")),
    };
    let cache = directory.filter(|_| max_bytes > 0).map(|d| Arc::new(DiskCache::new(d, max_bytes)));
    *DISK.write().unwrap_or_else(|e| e.into_inner()) = cache;
}

/// The disk cache images are kept in, if any.
pub fn image_disk_cache() -> Option<Arc<DiskCache>> {
    DISK.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// At exit (C++'s `stopImageLoader`): loads waiting are dropped, the running ones stop soon, the workers end.
pub fn stop_image_loader() {
    STOPPING.store(true, Ordering::Relaxed);
    let l = loader();
    let threads = {
        let mut q = l.queue.lock().unwrap_or_else(|e| e.into_inner());
        q.jobs.clear();
        std::mem::take(&mut q.threads)
    };
    l.wake.notify_all();
    for t in threads {
        let _ = t.join();
    }
}

/// An image asset or a file next to the binary, decoded (C++'s `loadImageFile`); none if it can't be.
pub fn load_image_file(path: &str) -> Option<ImageBitmap> {
    let image = read_file(path).and_then(|bytes| decode(&bytes, ImageRequestSize::default()));
    if image.is_none() {
        log(&format!("kompoz: can't load image {path}"));
    }
    image
}

/// A screenshot from pixels read back (RGBA, top row first), as a PNG (C++'s `savePixels`).
pub fn save_pixels(path: &str, width: i32, height: i32, rgba: &[u8]) {
    let ok = save_png(path, width, height, rgba).is_ok();
    log(&format!("kompoz: screenshot {path} {}", if ok { "saved" } else { "not saved" }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kompoz-disk-cache-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn the_disk_cache_keeps_the_most_recently_used() {
        let dir = temp_dir("lru");
        let k = |i: usize| format!("https://example.org/{i}.png");
        let entry = (header(&k(0)).len() + 100) as u64;
        let cache = DiskCache::new(dir.clone(), entry * 3);
        for i in 0..3 {
            cache.put(&k(i), &[i as u8; 100]);
        }
        assert_eq!(cache.size(), entry * 3);
        assert_eq!(cache.get(&k(0)), Some(vec![0; 100])); // (now the most recently used)
        cache.put(&k(3), &[3; 100]); // 1 goes
        assert_eq!(cache.get(&k(1)), None);
        assert!(cache.get(&k(0)).is_some() && cache.get(&k(2)).is_some() && cache.get(&k(3)).is_some());
        // Too big for it: not kept. Removed: gone.
        cache.put("big", &vec![0; entry as usize * 4]);
        assert_eq!(cache.get("big"), None);
        cache.remove(&k(0));
        assert_eq!(cache.get(&k(0)), None);
        // The next run finds them, the order kept; a write left unfinished goes.
        fs::write(dir.join(format!("{}.12345.tmp", file_name("x"))), b"half").unwrap();
        let again = DiskCache::new(dir.clone(), entry * 3);
        assert_eq!(again.size(), entry * 2);
        assert_eq!(again.get(&k(3)), Some(vec![3; 100]));
        assert!(fs::read_dir(&dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")));
        again.clear();
        assert_eq!(again.size(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn keys_sharing_a_file_name_are_told_apart() {
        let dir = temp_dir("keys");
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        cache.put("a", b"first");
        // Another key whose file is the same one (written by hand: FNV-1a collisions are rare).
        let path = dir.join(file_name("a"));
        let mut other = header("b");
        other.extend_from_slice(b"second");
        fs::write(&path, other).unwrap();
        assert_eq!(cache.get("a"), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
