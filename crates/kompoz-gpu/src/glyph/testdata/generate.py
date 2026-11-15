#!/usr/bin/env python3
"""Generates the glyph tests' COLR v1 paint font, KompozPaints.ttf: one glyph for each kind of paint the CPU
painter draws, each a 1000-unit square (clip box 0..1000 both ways) on a 1000-unit em, its colours from one palette
(0 red, 1 blue, 2 green, 3 yellow, 4 cyan; 0xFFFF the foreground):

U+E001  a linear gradient across, red at x 0 to blue at x 1000 (padded)
U+E002  a radial gradient from the centre, red at radius 0 to blue at radius 500 (padded)
U+E003  a sweep gradient round the centre, red at 0 degrees to blue at 360 (counter-clockwise, padded)
U+E004  the left half, green, translated 500 units right
U+E005  blue composited SrcIn onto a red square 250..750
U+E006  cyan composited Multiply onto yellow
U+E007  the foreground colour over the square
U+E008  a linear gradient red at x 0 to blue at x 500, repeated
U+E009  the square in green, clipped to its box 250..750 by a narrower clip box
U+E00A  a linear gradient red to blue, rotated a quarter turn round the centre (so it runs bottom to top)

Nothing is downloaded. Run it from anywhere: `python3 crates/kompoz-gpu/src/glyph/testdata/generate.py`
(fontTools 4.x; 4.68 was used). The output is the same byte for byte on every run.
"""

import os

from fontTools.colorLib.builder import buildCOLR, buildCPAL
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables import otTables

HERE = os.path.dirname(os.path.abspath(__file__))
# 2026-01-01T00:00:00Z in the font epoch.
TIMESTAMP = 3_850_329_600
P = otTables.PaintFormat


def box(x0, y0, x1, y1):
    pen = TTGlyphPen(None)
    pen.moveTo((x0, y0))
    pen.lineTo((x0, y1))
    pen.lineTo((x1, y1))
    pen.lineTo((x1, y0))
    pen.closePath()
    return pen.glyph()


def solid(index, alpha=1.0):
    return {"Format": P.PaintSolid, "PaletteIndex": index, "Alpha": alpha}


def line(stops, extend="pad"):
    return {"Extend": extend, "ColorStop": [{"StopOffset": o, "PaletteIndex": i, "Alpha": 1.0} for o, i in stops]}


def glyph(name, paint):
    return {"Format": P.PaintGlyph, "Glyph": name, "Paint": paint}


def linear(x0, x1, extend="pad"):
    return {
        "Format": P.PaintLinearGradient,
        "ColorLine": line([(0.0, 0), (1.0, 1)], extend),
        "x0": x0,
        "y0": 0,
        "x1": x1,
        "y1": 0,
        "x2": x0,
        "y2": 1000,
    }


def main():
    codes = list(range(0xE001, 0xE00B))
    names = [f"uni{c:04X}" for c in codes]
    glyphs = [".notdef", "square", "inner", "left"] + names
    fb = FontBuilder(1000, isTTF=True)
    fb.setupGlyphOrder(glyphs)
    fb.setupCharacterMap({c: n for c, n in zip(codes, names)})
    fb.setupHorizontalMetrics({g: (1000, 0) for g in glyphs})
    fb.setupHorizontalHeader(ascent=1000, descent=0, lineGap=0)
    family = "Kompoz Paints"
    fb.setupNameTable(
        {
            "copyright": "Copyright 2026 the KompozR authors",
            "familyName": family,
            "styleName": "Regular",
            "uniqueFontIdentifier": f"KompozR:{family}",
            "fullName": f"{family} Regular",
            "psName": "KompozPaints-Regular",
            "version": "Version 1.000",
            "licenseDescription": "This Font Software is licensed under the SIL Open Font License, Version 1.1.",
            "licenseInfoURL": "https://openfontlicense.org",
        }
    )
    fb.setupOS2(sTypoAscender=1000, sTypoDescender=0, usWinAscent=1000, usWinDescent=0, achVendID="KMPZ", version=4)
    fb.setupPost()
    square = box(0, 0, 1000, 1000)
    fb.setupGlyf(
        {
            ".notdef": box(0, 0, 500, 700),
            "square": square,
            "inner": box(250, 250, 750, 750),
            "left": box(0, 0, 500, 1000),
            **{n: square for n in names},
        }
    )
    paints = {
        "uniE001": glyph("square", linear(0, 1000)),
        "uniE002": glyph(
            "square",
            {
                "Format": P.PaintRadialGradient,
                "ColorLine": line([(0.0, 0), (1.0, 1)]),
                "x0": 500,
                "y0": 500,
                "r0": 0,
                "x1": 500,
                "y1": 500,
                "r1": 500,
            },
        ),
        "uniE003": glyph(
            "square",
            {
                "Format": P.PaintSweepGradient,
                "ColorLine": line([(0.0, 0), (1.0, 1)]),
                "centerX": 500,
                "centerY": 500,
                "startAngle": 0.0,
                "endAngle": 360.0,
            },
        ),
        "uniE004": {"Format": P.PaintTranslate, "dx": 500, "dy": 0, "Paint": glyph("left", solid(2))},
        "uniE005": {
            "Format": P.PaintComposite,
            "CompositeMode": "src_in",
            "SourcePaint": glyph("square", solid(1)),
            "BackdropPaint": glyph("inner", solid(0)),
        },
        "uniE006": {
            "Format": P.PaintComposite,
            "CompositeMode": "multiply",
            "SourcePaint": glyph("square", solid(4)),
            "BackdropPaint": glyph("square", solid(3)),
        },
        "uniE007": glyph("square", solid(0xFFFF)),
        "uniE008": glyph("square", linear(0, 500, "repeat")),
        "uniE009": glyph("square", solid(2)),
        "uniE00A": {
            "Format": P.PaintRotateAroundCenter,
            "angle": 90.0,
            "centerX": 500,
            "centerY": 500,
            "Paint": glyph("square", linear(0, 1000)),
        },
    }
    clips = {n: (0, 0, 1000, 1000) for n in names}
    clips["uniE009"] = (250, 250, 750, 750)
    # Each glyph's left side bearing is where its outline starts (what places it on its origin).
    glyf, hmtx = fb.font["glyf"], fb.font["hmtx"]
    for name, (advance, _) in list(hmtx.metrics.items()):
        outline = glyf[name]
        outline.recalcBounds(glyf)
        hmtx.metrics[name] = (advance, getattr(outline, "xMin", 0))
    fb.font["COLR"] = buildCOLR(paints, version=1, clipBoxes=clips)
    red, blue, green, yellow, cyan = (1, 0, 0, 1), (0, 0, 1, 1), (0, 1, 0, 1), (1, 1, 0, 1), (0, 1, 1, 1)
    fb.font["CPAL"] = buildCPAL([[red, blue, green, yellow, cyan]])
    fb.font["head"].created = TIMESTAMP
    fb.font["head"].modified = TIMESTAMP
    fb.font.recalcTimestamp = False
    path = os.path.join(HERE, "KompozPaints.ttf")
    fb.save(path)
    print(f"{os.path.basename(path)}: {os.path.getsize(path)} bytes")


if __name__ == "__main__":
    main()
