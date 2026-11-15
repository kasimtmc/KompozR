# kompoz-text's test fonts

Fonts the real-font tests shape with, through a font collection of only these (`FontFamilyResolverImpl::registered_only()`,
see `tests/common/mod.rs`), so no test depends on the machine's fonts. All are under the SIL Open Font License 1.1
(`OFL.txt`); none declares a Reserved Font Name. Nothing was downloaded: the drawn fonts are made by `generate.py`
with fontTools, the others are subsets of fonts installed on the machine they were made on (Arch Linux packages
`noto-fonts 1:2026.10.01-1`, `noto-fonts-emoji 1:2.057-1`, `cantarell-fonts 1:0.311-1`).

| File | Size | What it is | What the tests use it for |
| --- | --- | --- | --- |
| `KompozTest.ttf` | 28 KB | Drawn: every glyph a box 500 units wide from −200 to 800 of a 1000-unit em; hhea and OS/2 ascent 800, descent −200, line gap 250, USE_TYPO_METRICS; underline 100 below (to its top) and 50 thick, strikeout 300 above and 50 thick, x-height 500, cap height 700; no layout tables. Basic Latin, Latin-1, Latin Extended-A, Hebrew, Arabic and U+2764 (a heart as text). | The fake backend as a font: real shaping must give the fake's advances, offsets, clusters, metrics and boxes; the text font for ❤ with VS15. |
| `KompozCOLRv0.ttf`, `KompozCOLRv1.ttf`, `KompozSbix.ttf` | 2 KB each | Drawn: U+E000 and U+2B50 colored by COLR v0 layers, a COLR v1 paint graph (clip boxes 100..900 × 0..800), or 16 px sbix PNGs. | Color kinds and color glyphs' bounds. |
| `NotoSans-Subset.ttf` | 28 KB | Noto Sans Regular 2.015: ASCII, `çğıöşüÇĞİÖŞÜéÉâ`, U+0301, `…`, NBSP. | The default family's text, kerning and features, metrics. |
| `NotoSansArabic-Subset.ttf` | 21 KB | Noto Sans Arabic Regular 2.013: U+0621–U+0652 (letters and harakat), U+0660–U+0669, `،`, `؟`, space. | Joining forms, right-to-left clusters, marks, the Arabic fallback. |
| `NotoSansDevanagari-Subset.ttf` | 20 KB | Noto Sans Devanagari Regular 2.007: क ष त न म स र ह द, ि े ् ा ी, ◌, space (the conjuncts among them kept). | Vowel-sign reordering, conjuncts, the Devanagari fallback. |
| `NotoSansHebrew-Subset.ttf` | 6 KB | Noto Sans Hebrew Regular 3.001: U+05D0–U+05EA, space. | The Hebrew fallback, right-to-left text. |
| `NotoColorEmoji-Subset.ttf` | 30 KB | Noto Color Emoji 2.057 (CBDT bitmaps): 👍 😀 ❤ and the sequences 👨‍👩‍👧, 🇹🇷, 🇩🇪, 🏴󠁧󠁢󠁥󠁮󠁧󠁿, 👍🏽, 1⃣, 1️⃣, ❤️ with only their own ligatures. | Emoji sequences as one glyph, emoji presentation and VS16, bitmap color kind. |
| `Cantarell-VF-Subset.otf` | 11 KB | Cantarell variable 0.311 (CFF2, `wght` 100–800, default 400): A–Z, a–z, space. Its hhea line (983 / −217 / 0) differs from its OS/2 one and USE_TYPO_METRICS is off. | Variation settings as normalised coordinates, metrics from hhea. |

## Making them again

```
python3 crates/kompoz-text/tests/fonts/generate.py
```

It needs fontTools (4.68 was used) and Pillow (for the sbix PNGs), and reads the system fonts from
`/usr/share/fonts/noto/` and `/usr/share/fonts/cantarell/`. The output is the same byte for byte on every run with
the same inputs (timestamps are fixed). Each subset is what this pyftsubset command makes:

```
pyftsubset SOURCE --unicodes=… --layout-features='*' --name-IDs='*' --name-languages='*' --notdef-outline \
  --no-hinting --desubroutinize --output-file=TARGET
```

with the code points listed above (`generate.py` has them exactly). For Noto Color Emoji the layout closure is off
(`--no-layout-closure`) and the sequences' ligature glyphs are named with `--glyphs=…` (found in its GSUB by
`emoji_ligatures`), since the family members and the skin tone would otherwise pull in every sequence made of them.
