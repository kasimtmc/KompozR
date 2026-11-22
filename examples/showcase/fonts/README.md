# The showcase's comparison fonts

The fonts the showcase sets its text in when its shots are compared with the C++ Kompoz showcase's
(`showcase::shots`): a fontconfig configuration of only this directory, so no other font of the machine takes part.
The C++ showcase reads the same faces by path (Noto Sans Regular, Medium and Bold bundled next to its binary, which
are these; Noto Sans Mono, Noto Sans Arabic and Noto Sans Symbols and Symbols 2 from /usr/share/fonts/noto).

All are under the SIL Open Font License 1.1 (`OFL.txt`), Copyright 2022 The Noto Project Authors
(https://github.com/notofonts), none with a Reserved Font Name. Nothing was downloaded: they are subsets of the fonts
installed on the machine the C++ pictures were made on (Arch Linux `noto-fonts 1:2026.11.01-1`: Noto Sans 2.015, Noto
Sans Mono 2.014, Noto Sans Arabic 2.013, Noto Sans Symbols 2.003, Noto Sans Symbols 2 2.008), keeping every table but
the characters the showcase doesn't use. KompozR draws the showcase with these subsets exactly as with the whole fonts
(its shots byte for byte the same; checked when they were made).

| File | Size | Characters kept |
| --- | --- | --- |
| `NotoSans-Regular.ttf`, `NotoSans-Medium.ttf`, `NotoSans-Bold.ttf`, `NotoSansMono-Regular.ttf` | 77–92 KB | U+0020–007E, U+00A0–017F (Latin-1, Latin Extended-A: Turkish), U+2010–2027, U+2030–205E (punctuation), U+20A0–20C0 (currency: ₺), U+2190–2193 (arrows), U+2212 |
| `NotoSansArabic-Regular.ttf` | 82 KB | U+0020–007E, U+00A0, U+0600–06FF, U+FE70–FEFF, U+200C–200F |
| `NotoSansSymbols-Regular.ttf` | 7 KB | U+0020, U+2190–21FF (←) |
| `NotoSansSymbols2-Regular.ttf` | 34 KB | U+0020, U+2600–26FF (★) |

Made with fontTools 4.68's pyftsubset, each as:

```
pyftsubset /usr/share/fonts/noto/<file> --unicodes="<the characters above>" --layout-features='*' \
    --name-IDs='*' --name-languages='*' --notdef-outline --glyph-names --output-file=<file>
```
