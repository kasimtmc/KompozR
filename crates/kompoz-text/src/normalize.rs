//! Unicode normalization form C (canonical decomposition, canonical ordering, canonical composition: UAX #15), over
//! the shaper's canonical (de)composition and combining class data — what Java's `Normalizer.normalize(s, NFC)` does,
//! which Android's dead keys combine through (`KeyCharacterMap.getDeadChar`).

/// The canonical decomposition of `c`, appended to `out` (recursively, Hangul syllables included).
fn decompose_into(c: u32, out: &mut Vec<u32>) {
    match harfrust::unicode::decompose(c) {
        Some(harfrust::unicode::Decomposed::Singleton(a)) => decompose_into(a, out),
        Some(harfrust::unicode::Decomposed::Pair(a, b)) => {
            decompose_into(a, out);
            decompose_into(b, out);
        }
        None => out.push(c),
    }
}

fn ccc(c: u32) -> u8 {
    harfrust::unicode::combining_class(c)
}

/// `text` in normalization form C.
pub fn to_nfc(text: &str) -> String {
    // Canonical decomposition.
    let mut chars = Vec::with_capacity(text.len());
    for c in text.chars() {
        decompose_into(c as u32, &mut chars);
    }
    // Canonical ordering: each run of non-starters sorted (stably) by combining class.
    let mut i = 0;
    while i < chars.len() {
        if ccc(chars[i]) == 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && ccc(chars[i]) != 0 {
            i += 1;
        }
        chars[start..i].sort_by_key(|&c| ccc(c));
    }
    // Canonical composition: a character joins the last starter unless something between them blocks it (a
    // starter, or a combining mark of its class or higher).
    let mut out: Vec<u32> = Vec::with_capacity(chars.len());
    let mut starter: Option<usize> = None;
    let mut last_class: i16 = -1;
    for &c in &chars {
        let class = ccc(c) as i16;
        if let Some(s) = starter {
            let blocked = last_class != -1 && (last_class == 0 || last_class >= class);
            if !blocked && let Some(composed) = harfrust::unicode::compose(out[s], c) {
                out[s] = composed;
                continue;
            }
        }
        if class == 0 {
            starter = Some(out.len());
            last_class = -1;
        } else {
            last_class = class;
        }
        out.push(c);
    }
    out.into_iter().filter_map(char::from_u32).collect()
}
