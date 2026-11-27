//! http(s) downloads on Windows through WinHTTP (C++ Kompoz's Windows builds used WinINet: common/image_loader.cpp's
//! `httpGet`): what the image loader fetches URLs with ([`http_get`], its client until
//! [`set_http_client`](crate::image_loader::set_http_client) says otherwise). The system's proxy settings, redirects
//! followed (WinHTTP's own policy: not from https to http), a body only from a 2xx response, 30 seconds at most
//! (each phase 10, as C++'s), a download given up as the app quits. An FFI module: its `unsafe` blocks say why they
//! hold.

#![allow(unsafe_code)]

use super::sys::wide;
use ::windows::Win32::Networking::WinHttp::{
    URL_COMPONENTS, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_INTERNET_SCHEME_HTTPS, WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpCrackUrl, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
};
use ::windows::core::PCWSTR;
use std::ffi::c_void;
use std::time::{Duration, Instant};

/// A response: its status and body.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Response {
    pub status: i64,
    pub body: Vec<u8>,
}

/// A WinHTTP handle (session, connection, request), closed when it goes.
struct Internet(*mut c_void);

impl Internet {
    fn made(raw: *mut c_void, what: &str) -> Result<Internet, String> {
        if raw.is_null() { Err(format!("{what} failed ({})", super::sys::last_error())) } else { Ok(Internet(raw)) }
    }
}

impl Drop for Internet {
    fn drop(&mut self) {
        // SAFETY: a handle WinHTTP gave, closed once (a request before its connection before its session: they
        // are dropped in that order).
        let _ = unsafe { WinHttpCloseHandle(self.0) };
    }
}

/// The longest a download takes in all, and each of its phases (resolving, connecting, sending, each receive).
const TOTAL: Duration = Duration::from_secs(30);
const PHASE_MS: i32 = 10_000;

/// A GET of `url` (redirects followed, 30 seconds at most in all), given up when `cancelled` says so (asked between
/// the body's parts): the last response's status and body, whatever the status; why not if the transfer failed.
pub fn get(url: &str, cancelled: fn() -> bool) -> Result<Response, String> {
    let deadline = Instant::now() + TOTAL;
    let url_w: Vec<u16> = url.encode_utf16().collect();
    // The URL's parts (pointers into `url_w`: their lengths asked for as non-zero).
    let mut parts = URL_COMPONENTS {
        dwStructSize: std::mem::size_of::<URL_COMPONENTS>() as u32,
        dwSchemeLength: u32::MAX,
        dwHostNameLength: u32::MAX,
        dwUrlPathLength: u32::MAX,
        dwExtraInfoLength: u32::MAX,
        ..Default::default()
    };
    // SAFETY: `parts` is written with pointers into `url_w`, which lives until the end of this function.
    unsafe { WinHttpCrackUrl(&url_w, 0, &mut parts) }.map_err(|e| format!("not a URL ({e})"))?;
    let slice = |p: ::windows::core::PWSTR, n: u32| -> Vec<u16> {
        if p.is_null() || n == 0 {
            return Vec::new();
        }
        // SAFETY: WinHttpCrackUrl pointed `p` at `n` characters inside `url_w`, still alive.
        let mut v = unsafe { std::slice::from_raw_parts(p.0, n as usize) }.to_vec();
        v.push(0);
        v
    };
    let host = slice(parts.lpszHostName, parts.dwHostNameLength);
    let mut object = slice(parts.lpszUrlPath, parts.dwUrlPathLength);
    object.pop();
    object.extend(slice(parts.lpszExtraInfo, parts.dwExtraInfoLength));
    if object.last() != Some(&0) {
        object.push(0);
    }
    if object.len() == 1 {
        object = wide("/");
    }
    if host.is_empty() {
        return Err("no host".into());
    }
    let agent = wide("KompozR");
    // (Each call below: the wide strings live through the calls; the handles are alive, held by `Internet`s,
    // closed in reverse order.)
    let session = Internet::made(
        // SAFETY: see above.
        unsafe {
            WinHttpOpen(PCWSTR(agent.as_ptr()), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0)
        },
        "WinHttpOpen",
    )
    // (Windows before 8.1 has no automatic proxy: the default one.)
    .or_else(|_| {
        Internet::made(
            // SAFETY: see above.
            unsafe {
                WinHttpOpen(
                    PCWSTR(agent.as_ptr()),
                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                    PCWSTR::null(),
                    PCWSTR::null(),
                    0,
                )
            },
            "WinHttpOpen",
        )
    })?;
    // SAFETY: see above.
    let _ = unsafe { WinHttpSetTimeouts(session.0, PHASE_MS, PHASE_MS, PHASE_MS, PHASE_MS) };
    let connection =
        // SAFETY: see above.
        Internet::made(unsafe { WinHttpConnect(session.0, PCWSTR(host.as_ptr()), parts.nPort, 0) }, "WinHttpConnect")?;
    let secure = parts.nScheme == WINHTTP_INTERNET_SCHEME_HTTPS;
    let flags = if secure { WINHTTP_FLAG_SECURE } else { WINHTTP_OPEN_REQUEST_FLAGS(0) };
    let verb = wide("GET");
    let request = Internet::made(
        // SAFETY: see above (no version: HTTP/1.1; no referrer; any type accepted).
        unsafe {
            WinHttpOpenRequest(
                connection.0,
                PCWSTR(verb.as_ptr()),
                PCWSTR(object.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                flags,
            )
        },
        "WinHttpOpenRequest",
    )?;
    // SAFETY: see above (no headers, no body).
    unsafe { WinHttpSendRequest(request.0, None, None, 0, 0, 0) }.map_err(|e| format!("send failed ({e})"))?;
    // SAFETY: see above.
    unsafe { WinHttpReceiveResponse(request.0, std::ptr::null_mut()) }.map_err(|e| format!("no response ({e})"))?;
    let mut status = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let mut index = 0u32;
    // SAFETY: see above; `status` (its size said) is written as a number.
    unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut c_void),
            &mut size,
            &mut index,
        )
    }
    .map_err(|e| format!("no status ({e})"))?;
    let mut body = Vec::new();
    loop {
        if cancelled() {
            return Err("given up".into());
        }
        if Instant::now() > deadline {
            return Err("timed out".into());
        }
        let mut available = 0u32;
        // SAFETY: see above; `available` is written.
        unsafe { WinHttpQueryDataAvailable(request.0, &mut available) }.map_err(|e| format!("read failed ({e})"))?;
        if available == 0 {
            break;
        }
        let start = body.len();
        body.resize(start + available as usize, 0);
        let mut read = 0u32;
        // SAFETY: see above; the buffer has room for `available` bytes from `start`; `read` is written.
        unsafe { WinHttpReadData(request.0, body[start..].as_mut_ptr() as *mut c_void, available, &mut read) }
            .map_err(|e| format!("read failed ({e})"))?;
        body.truncate(start + read as usize);
        if read == 0 {
            break;
        }
    }
    drop(request);
    drop(connection);
    drop(session);
    Ok(Response { status: status as i64, body })
}

/// The image loader's HTTP client: the body of a 2xx response to a GET of `url`, given up as the app quits
/// ([`stopping`](crate::image_loader::stopping)); none if it failed (logged).
pub fn http_get(url: &str) -> Option<Vec<u8>> {
    match get(url, crate::image_loader::stopping) {
        Ok(response) => (response.status / 100 == 2).then_some(response.body),
        Err(why) => {
            crate::system::log(&format!("kompoz: {url}: {why}"));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A server of the test's own on 127.0.0.1 answering each connection with `answers` in turn (a request read up to
    /// its blank line first).
    fn serve(answers: Vec<Vec<u8>>) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for answer in answers {
                let Ok((mut s, _)) = listener.accept() else { return };
                let mut request = Vec::new();
                let mut b = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut b) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&b[..n]),
                    }
                }
                let _ = s.write_all(&answer);
            }
        });
        port
    }

    fn answer(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
        let mut a =
            format!("HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len())
                .into_bytes();
        a.extend_from_slice(body);
        a
    }

    #[test]
    fn bodies_statuses_and_redirects() {
        let body: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let port = serve(vec![
            answer("200 OK", "", &body),
            answer("404 Not Found", "", b"yok"),
            answer("302 Found", "Location: /there\r\n", b""),
            answer("200 OK", "", b"burada"),
        ]);
        let base = format!("http://127.0.0.1:{port}");
        let r = get(&format!("{base}/image.png?size=2"), || false).unwrap();
        assert_eq!((r.status, r.body.len()), (200, body.len()));
        assert_eq!(r.body, body);
        let r = get(&format!("{base}/none"), || false).unwrap();
        assert_eq!((r.status, r.body.as_slice()), (404, b"yok".as_slice()));
        let r = get(&format!("{base}/moved"), || false).unwrap();
        assert_eq!((r.status, r.body.as_slice()), (200, b"burada".as_slice()));
        // Given up: none read.
        let port = serve(vec![answer("200 OK", "", b"x")]);
        assert!(get(&format!("http://127.0.0.1:{port}/"), || true).is_err());
        assert!(get("nothing", || false).is_err());
    }
}
