//! PNG pictures as the web test compares them (`cargo xtask web test`): read (8-bit RGB or RGBA, not interlaced:
//! what KompozR's screenshots are) and written (RGBA), through miniz_oxide's zlib, as the tasks have no image crate.

use miniz_oxide::deflate::compress_to_vec_zlib;
use miniz_oxide::inflate::decompress_to_vec_zlib;

/// A picture: RGBA, the top row first.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// A PNG file's picture; why not if it isn't one this reads.
pub fn decode(bytes: &[u8]) -> Result<Picture, String> {
    if bytes.len() < 8 || &bytes[..8] != SIGNATURE {
        return Err("not a PNG".into());
    }
    let (mut at, mut header, mut data) = (8, None, Vec::new());
    while at + 8 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let kind = &bytes[at + 4..at + 8];
        let body = bytes.get(at + 8..at + 8 + length).ok_or("a chunk past the end")?;
        match kind {
            b"IHDR" => header = Some(body.to_vec()),
            b"IDAT" => data.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        at += 12 + length;
    }
    let header = header.ok_or("no IHDR")?;
    let width = u32::from_be_bytes(header[0..4].try_into().unwrap()) as usize;
    let height = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
    let (depth, color, interlace) = (header[8], header[9], header[12]);
    let channels = match color {
        6 => 4,
        2 => 3,
        _ => return Err(format!("color type {color} (only RGB and RGBA)")),
    };
    if depth != 8 || interlace != 0 {
        return Err(format!("bit depth {depth}, interlace {interlace} (only 8 bits, not interlaced)"));
    }
    let raw = decompress_to_vec_zlib(&data).map_err(|e| format!("inflate: {e:?}"))?;
    let stride = width * channels;
    if raw.len() < height * (stride + 1) {
        return Err("too little data".into());
    }
    let mut out = vec![0u8; height * stride];
    for y in 0..height {
        let filter = raw[y * (stride + 1)];
        let line = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        for x in 0..stride {
            let a = if x >= channels { out[y * stride + x - channels] as i32 } else { 0 };
            let b = if y > 0 { out[(y - 1) * stride + x] as i32 } else { 0 };
            let c = if x >= channels && y > 0 { out[(y - 1) * stride + x - channels] as i32 } else { 0 };
            let predicted = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                f => return Err(format!("filter {f}")),
            };
            out[y * stride + x] = (line[x] as i32 + predicted) as u8;
        }
    }
    let rgba = if channels == 4 { out } else { out.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect() };
    Ok(Picture { width, height, rgba })
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A picture as a PNG file (RGBA, no filters).
pub fn encode(picture: &Picture) -> Vec<u8> {
    let stride = picture.width * 4;
    let mut raw = Vec::with_capacity(picture.height * (stride + 1));
    for row in picture.rgba.chunks(stride.max(1)).take(picture.height) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut out = SIGNATURE.to_vec();
    let mut chunk = |kind: &[u8], body: &[u8]| {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    };
    let mut header = Vec::new();
    header.extend_from_slice(&(picture.width as u32).to_be_bytes());
    header.extend_from_slice(&(picture.height as u32).to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &header);
    chunk(b"IDAT", &compress_to_vec_zlib(&raw, 6));
    chunk(b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_go_through_png_and_back() {
        let picture = Picture { width: 3, height: 2, rgba: (0..24).map(|i| (i * 10) as u8).collect() };
        assert_eq!(decode(&encode(&picture)), Ok(picture));
        assert!(decode(b"nope").is_err());
    }
}
