//! Images from http URLs through libcurl (C++'s image loader on Linux), against a tiny HTTP server of the test's own
//! on 127.0.0.1 (no network, no TLS): a PNG, redirects followed, errors (404, 500, refused) giving nothing, a slow
//! body waited for, what was downloaded kept in the disk cache (only what decodes) and served from it, and a stalled
//! download given up as the app quits. Skipped where libcurl can't be loaded, unless `KOMPOZ_CURL_REQUIRED=1`. One
//! thread, in sequence (the loader's state is the process's).

#[cfg(target_os = "linux")]
mod linux {
    use kompoz_platform::image_loader::{
        image_disk_cache, load_image_async, load_now, start_image_disk_cache, stop_image_loader,
    };
    use kompoz_platform::linux::curl;
    use kompoz_platform::system::set_cache_directory;
    use kompoz_ui::platform::ImageRequestSize;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock, mpsc};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    /// The test picture: 48 × 32, a gradient (so it isn't one color), as a PNG.
    fn png() -> &'static [u8] {
        static PNG: OnceLock<Vec<u8>> = OnceLock::new();
        PNG.get_or_init(|| {
            let (w, h) = (48, 32);
            let mut rgba = Vec::with_capacity(w * h * 4);
            for y in 0..h {
                for x in 0..w {
                    rgba.extend_from_slice(&[(x * 5) as u8, (y * 7) as u8, 0x80, 0xff]);
                }
            }
            kompoz_graphics::encode_png(w as i32, h as i32, &rgba).expect("a PNG")
        })
    }

    /// A tiny HTTP/1.1 server on 127.0.0.1 (a thread per connection, each answered and closed): what each path was
    /// asked for counted. Stopped when dropped (stalled answers given up).
    struct Server {
        port: u16,
        hits: Arc<Mutex<HashMap<String, usize>>>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    fn respond(mut stream: TcpStream, path: &str, stop: &AtomicBool) {
        let head = |status: &str, extra: &str, length: usize| {
            format!("HTTP/1.1 {status}\r\nContent-Length: {length}\r\nConnection: close\r\n{extra}\r\n")
        };
        let png = png();
        let _ = match path {
            "/image.png" => stream
                .write_all(head("200 OK", "Content-Type: image/png\r\n", png.len()).as_bytes())
                .and_then(|_| stream.write_all(png)),
            "/moved" => stream.write_all(head("302 Found", "Location: /image.png\r\n", 0).as_bytes()),
            "/moved-twice" => stream.write_all(head("301 Moved Permanently", "Location: /moved\r\n", 0).as_bytes()),
            "/missing.png" => {
                let body = b"<html>not here</html>";
                stream
                    .write_all(head("404 Not Found", "Content-Type: text/html\r\n", body.len()).as_bytes())
                    .and_then(|_| stream.write_all(body))
            }
            "/broken.png" => stream.write_all(head("500 Internal Server Error", "", 0).as_bytes()),
            "/not-an-image.png" => {
                let body = b"hello, this is no picture at all";
                stream.write_all(head("200 OK", "", body.len()).as_bytes()).and_then(|_| stream.write_all(body))
            }
            // The body in four parts, 150 ms apart.
            "/slow.png" => {
                let mut result = stream.write_all(head("200 OK", "", png.len()).as_bytes());
                for part in png.chunks(png.len().div_ceil(4)) {
                    std::thread::sleep(Duration::from_millis(150));
                    result = result.and_then(|_| stream.write_all(part)).and_then(|_| stream.flush());
                }
                result
            }
            // A little of a big body, then nothing more (until the server stops).
            "/stalled.png" => {
                let result = stream
                    .write_all(head("200 OK", "", 1 << 20).as_bytes())
                    .and_then(|_| stream.write_all(&png[..64]))
                    .and_then(|_| stream.flush());
                let start = Instant::now();
                while !stop.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(60) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                result
            }
            _ => stream.write_all(head("404 Not Found", "", 0).as_bytes()),
        };
    }

    /// The path of a request read from `stream` (its first line's), once its head is all there.
    fn request_path(stream: &mut TcpStream) -> Option<String> {
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buffer).ok().filter(|&n| n > 0)?;
            request.extend_from_slice(&buffer[..n]);
        }
        let text = String::from_utf8_lossy(&request);
        let line = text.lines().next()?;
        let target = line.strip_prefix("GET ")?.split(' ').next()?;
        Some(target.split('?').next().unwrap_or(target).to_string()) // (the query only tells requests apart)
    }

    impl Server {
        fn start() -> Server {
            let listener = TcpListener::bind("127.0.0.1:0").expect("a port on 127.0.0.1");
            let port = listener.local_addr().unwrap().port();
            let hits: Arc<Mutex<HashMap<String, usize>>> = Arc::default();
            let stop = Arc::new(AtomicBool::new(false));
            let (their_hits, their_stop) = (hits.clone(), stop.clone());
            let thread = std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if their_stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(mut stream) = stream else { continue };
                    let (hits, stop) = (their_hits.clone(), their_stop.clone());
                    std::thread::spawn(move || {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let Some(path) = request_path(&mut stream) else { return };
                        *hits.lock().unwrap().entry(path.clone()).or_default() += 1;
                        respond(stream, &path, &stop);
                    });
                }
            });
            Server { port, hits, stop, thread: Some(thread) }
        }

        fn url(&self, path: &str) -> String {
            format!("http://127.0.0.1:{}{path}", self.port)
        }

        fn hits(&self, path: &str) -> usize {
            self.hits.lock().unwrap().get(path).copied().unwrap_or(0)
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(("127.0.0.1", self.port)); // (wakes its accept)
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// Whether libcurl can be loaded here; none and KOMPOZ_CURL_REQUIRED=1: a failure.
    pub fn curl_available() -> bool {
        match curl::load() {
            Ok(()) => true,
            Err(why) => {
                assert!(
                    std::env::var("KOMPOZ_CURL_REQUIRED").as_deref() != Ok("1"),
                    "KOMPOZ_CURL_REQUIRED=1 but libcurl can't be loaded: {why}"
                );
                println!("skipped: libcurl can't be loaded ({why})");
                false
            }
        }
    }

    fn never() -> bool {
        false
    }

    /// A GET as C++ makes it: a body only from a 2xx response, redirects followed, errors giving nothing, a slow body
    /// waited for.
    pub fn downloads_follow_redirects_and_fail_on_errors() {
        let server = Server::start();
        let response = curl::get(&server.url("/image.png"), never).expect("the transfer");
        assert_eq!((response.status, response.body.as_slice()), (200, png()));
        assert_eq!(curl::http_get(&server.url("/image.png")).as_deref(), Some(png()));
        // Redirects followed (a 301 to a 302 to the picture).
        assert_eq!(curl::http_get(&server.url("/moved-twice")).as_deref(), Some(png()));
        assert_eq!((server.hits("/moved-twice"), server.hits("/moved")), (1, 1));
        // A 404's page and a 500 are no picture: nothing.
        let missing = curl::get(&server.url("/missing.png"), never).expect("the transfer");
        assert_eq!((missing.status, missing.body.as_slice()), (404, &b"<html>not here</html>"[..]));
        assert_eq!(curl::http_get(&server.url("/missing.png")), None);
        assert_eq!(curl::http_get(&server.url("/broken.png")), None);
        // Nothing listening: nothing (refused at once).
        let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let t0 = Instant::now();
        assert!(curl::get(&format!("http://127.0.0.1:{closed}/image.png"), never).is_err());
        assert_eq!(curl::http_get(&format!("http://127.0.0.1:{closed}/image.png")), None);
        assert!(t0.elapsed() < Duration::from_secs(5));
        // A slow one, waited for whole.
        let t0 = Instant::now();
        assert_eq!(curl::http_get(&server.url("/slow.png")).as_deref(), Some(png()));
        assert!(t0.elapsed() >= Duration::from_millis(550), "{:?}", t0.elapsed());
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kompoz-http-tests-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// The image loader's URLs through libcurl (its client without being set): downloaded once, kept in the disk
    /// cache (only what decodes), then shown from it; what was kept and doesn't decode forgotten.
    pub fn downloads_are_kept_in_the_disk_cache() {
        let server = Server::start();
        let cache = scratch("cache");
        set_cache_directory(Some(cache.clone()));
        start_image_disk_cache("Test app", 1 << 20);
        let disk = image_disk_cache().expect("a disk cache");
        assert_eq!(disk.directory(), cache.join("images"));
        let any = ImageRequestSize::default();

        let url = server.url("/image.png");
        let image = load_now(&url, any).expect("the picture");
        assert_eq!((image.width(), image.height()), (48, 32));
        assert_eq!(disk.get(&url).as_deref(), Some(png()));
        assert!(load_now(&url, any).is_some());
        assert_eq!(server.hits("/image.png"), 1, "the second time came from the disk cache");
        // Kept under the URL asked for, redirects or not.
        assert!(load_now(&server.url("/moved"), any).is_some());
        assert_eq!(disk.get(&server.url("/moved")).as_deref(), Some(png()));
        assert_eq!((server.hits("/moved"), server.hits("/image.png")), (1, 2)); // (followed to the picture)
        // Not a picture, or an error: nothing, nothing kept.
        for path in ["/not-an-image.png", "/missing.png", "/broken.png"] {
            assert!(load_now(&server.url(path), any).is_none(), "{path}");
            assert_eq!(disk.get(&server.url(path)), None, "{path}");
        }
        // Kept but no longer decoding: forgotten (and fetched again the next time).
        let spoiled = server.url("/spoiled.png");
        disk.put(&spoiled, b"this was a picture once, honestly");
        assert!(load_now(&spoiled, any).is_none());
        assert_eq!(disk.get(&spoiled), None);
        assert_eq!(server.hits("/spoiled.png"), 0);
        // Asynchronously, on the loader's threads, scaled down for the size asked (from the cache: no new hit).
        let (sender, received) = mpsc::channel();
        let small = ImageRequestSize { width: 12, height: 8, fill: false };
        load_image_async(&url, small, Box::new(move |image| sender.send(image).unwrap()));
        let image = received.recv_timeout(Duration::from_secs(10)).expect("done called").expect("the picture");
        assert_eq!((image.width(), image.height()), (12, 8));
        assert_eq!(server.hits("/image.png"), 2);
        // And through the loader from the network, a new URL.
        let (sender, received) = mpsc::channel();
        load_image_async(&server.url("/slow.png"), any, Box::new(move |image| sender.send(image).unwrap()));
        assert!(received.recv_timeout(Duration::from_secs(10)).expect("done called").is_some());
        assert_eq!(server.hits("/slow.png"), 1);
        let _ = std::fs::remove_dir_all(&cache);
    }

    /// The app quits while downloads stall: each is given up (by libcurl's progress callback, about once a second),
    /// the loads waiting dropped, nothing told; the loader's threads end soon.
    pub fn quitting_gives_downloads_up() {
        let server = Server::start();
        let cache = scratch("quit");
        set_cache_directory(Some(cache.clone()));
        start_image_disk_cache("Test app", 1 << 20);
        // One fetched straight, four through the loader (three workers busy, one waiting).
        let url = server.url("/stalled.png");
        let straight = {
            let url = url.clone();
            std::thread::spawn(move || curl::http_get(&url))
        };
        let told = Arc::new(Mutex::new(0));
        for i in 0..4 {
            let told = told.clone();
            let url = format!("{url}?{i}");
            load_image_async(&url, ImageRequestSize::default(), Box::new(move |_| *told.lock().unwrap() += 1));
        }
        std::thread::sleep(Duration::from_millis(500)); // (all of them under way)
        assert_eq!(server.hits("/stalled.png"), 4);
        let t0 = Instant::now();
        stop_image_loader();
        let stopped = t0.elapsed();
        assert!(stopped < Duration::from_millis(2500), "the loader stopped after {stopped:?}");
        assert_eq!(straight.join().unwrap(), None);
        assert!(t0.elapsed() < Duration::from_millis(2500), "the download stopped after {:?}", t0.elapsed());
        assert_eq!(*told.lock().unwrap(), 0, "nothing is told after quitting");
        println!("  stalled downloads given up in {:?}", t0.elapsed());
        // An app started again in the process loads again.
        start_image_disk_cache("Test app", 1 << 20);
        assert_eq!(curl::http_get(&server.url("/image.png")).as_deref(), Some(png()));
        let _ = std::fs::remove_dir_all(&cache);
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        // The environment's proxy and image cache aren't the test's (before any thread starts).
        for name in ["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"] {
            // SAFETY: nothing else runs yet: no other thread reads the environment.
            unsafe { std::env::remove_var(name) };
        }
        // SAFETY: as above.
        unsafe { std::env::remove_var("KOMPOZ_IMAGE_CACHE") };
        if !linux::curl_available() {
            return;
        }
        let tests: &[(&str, fn())] = &[
            ("downloads_follow_redirects_and_fail_on_errors", linux::downloads_follow_redirects_and_fail_on_errors),
            ("downloads_are_kept_in_the_disk_cache", linux::downloads_are_kept_in_the_disk_cache),
            ("quitting_gives_downloads_up", linux::quitting_gives_downloads_up),
        ];
        let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
        let mut failed = 0;
        for (name, test) in tests {
            if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                continue;
            }
            let ok = std::panic::catch_unwind(test).is_ok();
            failed += usize::from(!ok);
            println!("{} {name}", if ok { "ok    " } else { "FAILED" });
        }
        if failed > 0 {
            std::process::exit(1);
        }
    }
}
