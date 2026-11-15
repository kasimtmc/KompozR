#!/usr/bin/env python3
"""Generates kompoz-text's test fonts (see README.md): KompozTest.ttf and three tiny color fonts drawn here with
fontTools, and subsets of fonts installed on the system (Noto Sans and its Arabic, Devanagari and Hebrew
companions, Noto Color Emoji, Cantarell's variable font). Nothing is downloaded.

Run it from anywhere: `python3 crates/kompoz-text/tests/fonts/generate.py` (fontTools 4.x and Pillow).
"""

import io
import os
import sys

from fontTools.colorLib.builder import buildCOLR, buildCPAL
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.subset import Options, Subsetter
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables import otTables

HERE = os.path.dirname(os.path.abspath(__file__))
NOTO = "/usr/share/fonts/noto"
CANTARELL = "/usr/share/fonts/cantarell/Cantarell-VF.otf"
# 2026-01-01T00:00:00Z in the font epoch (seconds since 1904-01-01): the generated fonts are the same byte for byte
# on every run.
TIMESTAMP = 3_850_329_600


def box(x0, y0, x1, y1):
    pen = TTGlyphPen(None)
    pen.moveTo((x0, y0))
    pen.lineTo((x0, y1))
    pen.lineTo((x1, y1))
    pen.lineTo((x1, y0))
    pen.closePath()
    return pen.glyph()


def finish(fb, path):
    # Each glyph's left side bearing is where its outline starts (what places it on its origin).
    glyf, hmtx = fb.font["glyf"], fb.font["hmtx"]
    for name, (advance, _) in list(hmtx.metrics.items()):
        glyph = glyf[name]
        glyph.recalcBounds(glyf)
        hmtx.metrics[name] = (advance, getattr(glyph, "xMin", 0))
    fb.font["head"].created = TIMESTAMP
    fb.font["head"].modified = TIMESTAMP
    fb.font.recalcTimestamp = False
    fb.save(path)
    print(f"{os.path.basename(path)}: {os.path.getsize(path)} bytes")


def setup(fb, family, glyphs, cmap, advance, ascent, descent, line_gap, use_typo):
    fb.setupGlyphOrder(glyphs)
    fb.setupCharacterMap(cmap)
    fb.setupHorizontalMetrics({g: (advance, 0) for g in glyphs})
    fb.setupHorizontalHeader(ascent=ascent, descent=descent, lineGap=line_gap)
    fb.setupNameTable(
        {
            "copyright": "Copyright 2026 the KompozR authors",
            "familyName": family,
            "styleName": "Regular",
            "uniqueFontIdentifier": f"KompozR:{family}",
            "fullName": f"{family} Regular",
            "psName": family.replace(" ", "") + "-Regular",
            "version": "Version 1.000",
            "licenseDescription": "This Font Software is licensed under the SIL Open Font License, Version 1.1.",
            "licenseInfoURL": "https://openfontlicense.org",
        }
    )
    fb.setupOS2(
        sTypoAscender=ascent,
        sTypoDescender=descent,
        sTypoLineGap=line_gap,
        usWinAscent=ascent,
        usWinDescent=-descent,
        sxHeight=500,
        sCapHeight=700,
        yStrikeoutPosition=300,
        yStrikeoutSize=50,
        fsSelection=(0x40 | 0x80) if use_typo else 0x40,
        usWeightClass=400,
        achVendID="KMPZ",
        version=4,
    )
    fb.setupPost(underlinePosition=-100, underlineThickness=50)


def kompoz_test():
    """Every glyph a box 500 wide from the descent (-200) to the ascent (800) of a 1000 unit em, the line gap 250:
    the fake backend's measures (half an em a character, ascent 0.8, descent 0.2, leading 0.25) as a real font.
    Basic Latin, Latin-1, Latin Extended-A, Hebrew, Arabic and U+2764 (a heart drawn as text); no layout tables."""
    codes = list(range(0x20, 0x7F)) + list(range(0xA0, 0x180)) + list(range(0x591, 0x5F5))
    codes += list(range(0x600, 0x700)) + [0x2764]
    glyphs = [".notdef"] + [f"uni{c:04X}" for c in codes]
    fb = FontBuilder(1000, isTTF=True)
    setup(fb, "Kompoz Test", glyphs, {c: f"uni{c:04X}" for c in codes}, 500, 800, -200, 250, True)
    the_box = box(0, -200, 500, 800)
    fb.setupGlyf({g: the_box for g in glyphs})
    finish(fb, os.path.join(HERE, "KompozTest.ttf"))


def color_font(name, kind):
    """A tiny color font of two glyphs: U+E000 and U+2B50 (a star, an emoji by default), 1000 units wide, its
    color drawn as `kind` says: COLR version 0 layers, a COLR version 1 paint graph, or sbix PNG bitmaps."""
    glyphs = [".notdef", "uE000", "u2B50", "layer.a", "layer.b"]
    cmap = {0xE000: "uE000", 0x2B50: "u2B50"}
    fb = FontBuilder(1000, isTTF=True)
    setup(fb, name, glyphs, cmap, 1000, 800, -200, 0, True)
    fb.setupGlyf(
        {
            ".notdef": box(0, 0, 500, 700),
            "uE000": box(100, 0, 900, 800),
            "u2B50": box(100, 0, 900, 800),
            "layer.a": box(100, 0, 900, 800),
            "layer.b": box(300, 200, 700, 600),
        }
    )
    if kind == "colr0":
        fb.font["COLR"] = buildCOLR({"uE000": [("layer.a", 0), ("layer.b", 1)], "u2B50": [("layer.a", 1)]}, version=0)
        fb.font["CPAL"] = buildCPAL([[(1.0, 0.0, 0.0, 1.0), (0.0, 0.0, 1.0, 1.0)]])
    elif kind == "colr1":
        def solid(index):
            return {"Format": otTables.PaintFormat.PaintSolid, "PaletteIndex": index, "Alpha": 1.0}

        def layer(glyph, index):
            return {"Format": otTables.PaintFormat.PaintGlyph, "Paint": solid(index), "Glyph": glyph}

        fb.font["COLR"] = buildCOLR(
            {
                "uE000": {
                    "Format": otTables.PaintFormat.PaintColrLayers,
                    "Layers": [layer("layer.a", 0), layer("layer.b", 1)],
                },
                "u2B50": layer("layer.a", 1),
            },
            version=1,
            clipBoxes={"uE000": (100, 0, 900, 800), "u2B50": (100, 0, 900, 800)},
        )
        fb.font["CPAL"] = buildCPAL([[(1.0, 0.0, 0.0, 1.0), (0.0, 0.0, 1.0, 1.0)]])
    elif kind == "sbix":
        from fontTools.ttLib.tables.sbixGlyph import Glyph as SbixGlyph
        from fontTools.ttLib.tables.sbixStrike import Strike
        from PIL import Image

        def png(color):
            out = io.BytesIO()
            Image.new("RGBA", (16, 16), color).save(out, format="PNG", optimize=True)
            return out.getvalue()

        sbix = newTable("sbix")
        sbix.version = 1
        sbix.flags = 1
        strike = Strike(ppem=16, resolution=72)
        for glyph, color in (("uE000", (255, 0, 0, 255)), ("u2B50", (0, 0, 255, 255))):
            strike.glyphs[glyph] = SbixGlyph(
                glyphName=glyph, graphicType="png ", originOffsetX=0, originOffsetY=0, imageData=png(color)
            )
        sbix.strikes = {16: strike}
        fb.font["sbix"] = sbix
    finish(fb, os.path.join(HERE, f"{name.replace(' ', '')}.ttf"))


def subset(source, target, unicodes, glyphs=(), layout_closure=True):
    """pyftsubset SOURCE --unicodes=… [--glyphs=…] [--no-layout-closure] --layout-features='*'
    --name-IDs='*' --name-languages='*' --notdef-outline --no-hinting --desubroutinize --output-file=TARGET"""
    options = Options()
    options.layout_features = ["*"]
    options.name_IDs = ["*"]
    options.name_languages = ["*"]
    options.notdef_outline = True
    options.hinting = False
    options.desubroutinize = True
    options.layout_closure = layout_closure
    font = TTFont(source, recalcTimestamp=False)
    subsetter = Subsetter(options)
    subsetter.populate(unicodes=unicodes, glyphs=list(glyphs))
    subsetter.subset(font)
    font["head"].modified = TIMESTAMP
    font.save(target)
    print(f"{os.path.basename(target)}: {os.path.getsize(target)} bytes")


def chars(text):
    return sorted({ord(c) for c in text})


def emoji_ligatures(font, sequences):
    """The glyphs Noto Color Emoji's GSUB makes of each sequence (its ligatures, by their components' glyphs)."""
    cmap = font.getBestCmap()
    wanted = set()
    for sequence in sequences:
        names = tuple(cmap[ord(c)] for c in sequence if ord(c) in cmap)
        if len(names) > 1:
            wanted.add(names)
    found = set()
    for lookup in font["GSUB"].table.LookupList.Lookup:
        for sub in lookup.SubTable:
            if lookup.LookupType == 7:
                sub = sub.ExtSubTable
            if not isinstance(sub, otTables.LigatureSubst):
                continue
            for first, ligatures in sub.ligatures.items():
                for lig in ligatures:
                    if (first, *lig.Component) in wanted:
                        found.add(lig.LigGlyph)
    return sorted(found)


def main():
    kompoz_test()
    color_font("Kompoz COLRv0", "colr0")
    color_font("Kompoz COLRv1", "colr1")
    color_font("Kompoz Sbix", "sbix")

    latin = "".join(chr(c) for c in range(0x20, 0x7F)) + "\u00E7\u011F\u0131\u00F6\u015F\u00FC\u00C7\u011E\u0130\u00D6\u015E\u00DC\u00E9\u00C9\u00E2\u0301\u2026\u00A0"
    subset(f"{NOTO}/NotoSans-Regular.ttf", os.path.join(HERE, "NotoSans-Subset.ttf"), chars(latin))
    arabic = " \u060C\u061F" + "".join(chr(c) for c in range(0x621, 0x653)) + "".join(chr(c) for c in range(0x660, 0x66A))
    subset(f"{NOTO}/NotoSansArabic-Regular.ttf", os.path.join(HERE, "NotoSansArabic-Subset.ttf"), chars(arabic))
    devanagari = " \u0915\u0937\u0924\u0928\u092E\u0938\u0930\u0939\u0926\u093F\u0947\u094D\u093E\u0940\u25CC"
    subset(
        f"{NOTO}/NotoSansDevanagari-Regular.ttf", os.path.join(HERE, "NotoSansDevanagari-Subset.ttf"), chars(devanagari)
    )
    hebrew = " " + "".join(chr(c) for c in range(0x5D0, 0x5EB))
    subset(f"{NOTO}/NotoSansHebrew-Regular.ttf", os.path.join(HERE, "NotoSansHebrew-Subset.ttf"), chars(hebrew))

    # The emoji the tests use: a ZWJ family, two flags, England's tag sequence, a skin tone, a keycap, a heart with
    # VS16, a thumbs up and a grinning face. The family members and the skin tone combine into many more sequences
    # than these, so the layout closure stays off and only these sequences' ligatures are kept.
    sequences = [
        "\U0001F468\u200D\U0001F469\u200D\U0001F467",
        "\U0001F1F9\U0001F1F7",
        "\U0001F1E9\U0001F1EA",
        "\U0001F3F4\U000E0067\U000E0062\U000E0065\U000E006E\U000E0067\U000E007F",
        "\U0001F44D\U0001F3FD",
        "1\u20E3",
        "1\uFE0F\u20E3",
        "\u2764\uFE0F",
    ]
    emoji = f"{NOTO}/NotoColorEmoji.ttf"
    ligatures = emoji_ligatures(TTFont(emoji), sequences)
    singles = chars("".join(sequences) + "\U0001F44D\U0001F600\u2764")
    subset(
        emoji, os.path.join(HERE, "NotoColorEmoji-Subset.ttf"), singles, glyphs=ligatures, layout_closure=False
    )

    letters = " " + "".join(chr(c) for c in range(0x41, 0x5B)) + "".join(chr(c) for c in range(0x61, 0x7B))
    subset(CANTARELL, os.path.join(HERE, "Cantarell-VF-Subset.otf"), chars(letters))


if __name__ == "__main__":
    sys.exit(main())
