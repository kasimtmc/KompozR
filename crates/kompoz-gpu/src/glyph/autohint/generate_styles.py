#!/usr/bin/env python3
"""Generates styles.rs, the automatic hinter's scripts, styles, blue strings and character ranges, from FreeType's
own tables in a FreeType source tree (2.14.3's was used, with 2.15.0's range changes), in the shape skrifa's
generated tables have:

  src/autofit/afscript.h       the scripts: HarfBuzz script, hinting direction, standard characters
  src/autofit/afstyles.h       the styles: writing system, script, blue stringset, coverage (an OpenType feature)
  src/autofit/afcover.h        the coverages' feature tags
  src/autofit/afblue.dat       the blue strings and each script's stringset with its properties
  src/autofit/afranges.c       each script's base and non-base character ranges
  src/autofit/hb-script-list.h HarfBuzz's script tags

The configuration is FreeType's default (include/freetype/config/ftoption.h): AF_CONFIG_OPTION_CJK and
AF_CONFIG_OPTION_INDIC defined, AF_CONFIG_OPTION_CJK_BLUE_HANI_VERT not.

The character ranges are flattened as FreeType assigns them (afglobal.c's af_face_globals_compute_style_coverage):
styles in order, a character taking the first style whose script's base ranges hold it, marked non-base when its
script's non-base ranges hold it too. Then FreeType 2.15.0's changes to its ranges are applied (FREETYPE_2_15
below): the C++ Kompoz's goldens were made with FreeType 2.15.0, whose sources aren't at hand; its ranges were read
from the system library's style map (the `glyph-to-script-map` property) of fonts mapping every code point of
planes 0 to 16 to a glyph of its own, which differs from 2.14.3's only there.

Nothing is downloaded. Run it from anywhere:

  python3 crates/kompoz-gpu/src/glyph/autohint/generate_styles.py ~/Projects/kompoz/third_party/freetype

The output is the same byte for byte on every run.
"""

import os
import re
import sys

DEFINED = {"AF_CONFIG_OPTION_CJK", "AF_CONFIG_OPTION_INDIC"}


def preprocess(text):
    """The lines of a C file (or afblue.dat) that FreeType's default configuration keeps."""
    out, stack = [], []
    for line in text.split("\n"):
        s = line.strip()
        if s.startswith("#ifdef"):
            stack.append(s.split()[1] in DEFINED)
        elif s.startswith("#ifndef"):
            stack.append(s.split()[1] not in DEFINED)
        elif s.startswith("#if"):
            stack.append(s.split()[1] != "0")
        elif s.startswith("#else"):
            stack[-1] = not stack[-1]
        elif s.startswith("#endif"):
            stack.pop()
        elif all(stack):
            out.append(line)
    return out


def c_string(literal):
    """A C string literal's text, its escapes undone, as UTF-8."""
    raw = bytearray()
    i = 0
    while i < len(literal):
        c = literal[i]
        if c == "\\":
            n = literal[i + 1]
            if n == "x":
                j = i + 2
                while j < len(literal) and literal[j] in "0123456789abcdefABCDEF" and j < i + 4:
                    j += 1
                raw.append(int(literal[i + 2 : j], 16))
                i = j
                continue
            raw.extend({"n": b"\n", "\\": b"\\", '"': b'"'}.get(n, n.encode()))
            i += 2
            continue
        raw.extend(c.encode())
        i += 1
    return raw.decode("utf-8")


def read(tree, name):
    with open(os.path.join(tree, "src", "autofit", name), encoding="utf-8") as f:
        return f.read()


def strip_comments(text):
    return re.sub(r"/\*.*?\*/", "", text, flags=re.S)


def hb_scripts(tree):
    tags = {"HB_SCRIPT_INVALID": None}
    for m in re.finditer(r"(HB_SCRIPT_\w+)\s*=\s*HB_TAG\s*\('(.)','(.)','(.)','(.)'\)", read(tree, "hb-script-list.h")):
        tags[m.group(1)] = "".join(m.group(2, 3, 4, 5))
    return tags


def scripts(tree):
    text = strip_comments("\n".join(preprocess(read(tree, "afscript.h"))))
    out = []
    pattern = r"SCRIPT\(\s*(\w+),\s*(\w+),\s*\"((?:[^\"\\]|\\.)*)\",\s*(\w+),\s*(\w+),\s*\"((?:[^\"\\]|\\.)*)\"\s*\)"
    for m in re.finditer(pattern, text):
        lower, upper, name, hb, direction, chars = m.groups()
        out.append(
            {
                "lower": lower,
                "upper": upper,
                "name": c_string(name),
                "hb": hb,
                "top_to_bottom": direction == "HINTING_TOP_TO_BOTTOM",
                "std_chars": c_string(chars),
            }
        )
    return out


def coverages(tree):
    text = strip_comments("\n".join(preprocess(read(tree, "afcover.h"))))
    out = {}
    for m in re.finditer(r"COVERAGE\(\s*(\w+),\s*(\w+),\s*\"([^\"]*)\",\s*'(.)',\s*'(.)',\s*'(.)',\s*'(.)'\s*\)", text):
        out[m.group(2)] = (m.group(3), "".join(m.group(4, 5, 6, 7)))
    return out


def styles(tree):
    text = strip_comments("\n".join(preprocess(read(tree, "afstyles.h")))).replace("\\\n", " ")
    # The meta style's expansion: its STYLE_LATIN lines, in order.
    meta = re.search(r"#define\s+META_STYLE_LATIN\(.*?\)(.*?)\n\s*\n", text, flags=re.S).group(1)
    meta_coverages = re.findall(r"STYLE_LATIN\(\s*s,\s*S,\s*\w+,\s*\w+,\s*ds,\s*\"[^\"]*\",\s*(\w+)\s*\)", meta)
    body = text[text.index(meta) + len(meta) :]
    out = []
    pattern = (
        r"(?:STYLE\(\s*\w+,\s*\w+,\s*\"[^\"]*\",\s*AF_WRITING_SYSTEM_(\w+),\s*AF_SCRIPT_(\w+),\s*AF_BLUE_STRINGSET_(\w+),"
        r"\s*AF_COVERAGE_(\w+)\s*\))|(?:META_STYLE_LATIN\(\s*\w+,\s*(\w+),\s*\"[^\"]*\"\s*\))"
        r"|(?:STYLE_DEFAULT_INDIC\(\s*\w+,\s*(\w+),\s*\"[^\"]*\"\s*\))"
    )
    for m in re.finditer(pattern, body):
        if m.group(6):
            # (Its blue stringset is 0, which the Indic writing system doesn't read.)
            out.append({"system": "INDIC", "script": m.group(6), "stringset": None, "coverage": "DEFAULT"})
        elif m.group(5):
            for coverage in meta_coverages:
                out.append({"system": "LATIN", "script": m.group(5), "stringset": m.group(5), "coverage": coverage})
        else:
            out.append(
                {"system": m.group(1), "script": m.group(2), "stringset": m.group(3), "coverage": m.group(4)}
            )
    return out


def blues(tree):
    lines = preprocess(read(tree, "afblue.dat"))
    lines = [l for l in lines if not l.strip().startswith("//")]
    strings, stringsets = {}, {}
    section = None
    current = None
    block = ""
    for line in lines:
        s = line.strip()
        if s.startswith("AF_BLUE_STRING_ENUM"):
            section = "strings"
            continue
        if s.startswith("AF_BLUE_STRINGSET_ENUM"):
            section = "stringsets"
            continue
        if not s:
            continue
        if section == "strings":
            if s.startswith('"'):
                strings[current] += c_string(s[1:-1])
            elif re.match(r"^AF_BLUE_STRING_\w+$", s):
                current = s[len("AF_BLUE_STRING_") :]
                strings[current] = ""
        elif section == "stringsets":
            if re.match(r"^AF_BLUE_STRINGSET_\w+$", s):
                current = s[len("AF_BLUE_STRINGSET_") :]
                stringsets[current] = []
            else:
                block += " " + s
                if block.count("{") == block.count("}"):
                    m = re.match(r"\s*\{\s*AF_BLUE_STRING_(\w+),\s*(.*)\}\s*$", block)
                    block = ""
                    name, props = m.group(1), m.group(2).strip()
                    if name == "MAX":
                        continue
                    props = [] if props == "0" else [p.strip() for p in props.split("|")]
                    stringsets[current].append((strings[name], props))
    return stringsets


# A blue string property's zones, as skrifa's BlueZones names them.
PROPERTIES = {
    "AF_BLUE_PROPERTY_LATIN_TOP": "TOP",
    "AF_BLUE_PROPERTY_LATIN_SUB_TOP": "SUB_TOP",
    "AF_BLUE_PROPERTY_LATIN_NEUTRAL": "NEUTRAL",
    "AF_BLUE_PROPERTY_LATIN_X_HEIGHT": "X_HEIGHT",
    "AF_BLUE_PROPERTY_LATIN_LONG": "LONG",
    "AF_BLUE_PROPERTY_LATIN_CAPITAL_BOTTOM": "CAPITAL_BOTTOM",
    "AF_BLUE_PROPERTY_LATIN_SMALL_BOTTOM": "SMALL_BOTTOM",
    "AF_BLUE_PROPERTY_CJK_TOP": "TOP",
    "AF_BLUE_PROPERTY_CJK_HORIZ": "HORIZONTAL",
    "AF_BLUE_PROPERTY_CJK_RIGHT": "RIGHT",
}

# The dummy writing system (the `none` script) hints nothing: a style without blue zones is left unhinted.
GROUPS = {"LATIN": "Default", "CJK": "Cjk", "INDIC": "Indic", "DUMMY": "Default"}


def ranges(tree):
    text = strip_comments("\n".join(preprocess(read(tree, "afranges.c"))))
    out = {}
    for m in re.finditer(r"af_(\w+?)_(nonbase_)?uniranges\[\]\s*=\s*\{(.*?)\};", text, flags=re.S):
        recs = [
            (int(a, 16), int(b, 16))
            for a, b in re.findall(r"AF_UNIRANGE_REC\(\s*(0x[0-9A-Fa-f]+|0),\s*(0x[0-9A-Fa-f]+|0)\s*\)", m.group(3))
        ]
        recs = [r for r in recs if r != (0, 0)]
        out[(m.group(1), bool(m.group(2)))] = recs
    return out


def rust_string(text):
    """`text` as the inside of a Rust string literal, invisible characters (joiners) escaped."""
    return "".join(c if c.isprintable() else "\\u{%x}" % ord(c) for c in text)


# FreeType 2.15.0's character ranges where they differ from 2.14.3's: (first, last, the script whose style the
# characters take (None: no style's), whether non-base).
FREETYPE_2_15 = [
    (0x0558, 0x0558, "ARMN", True),
    (0x058B, 0x058C, "ARMN", True),
    (0x05C8, 0x05C9, "HEBR", True),
    (0x107C0, 0x107FB, None, False),
    (0x10ECB, 0x10ECF, "ARAB", True),
    (0x10EF0, 0x10EFC, "ARAB", True),
    (0x11DF0, 0x11DF0, "BENG", True),
    (0x11DF1, 0x11DFF, "BENG", False),
    (0x1DFCD, 0x1DFFF, "LATP", False),
]


def zones(props):
    names = [PROPERTIES[p] for p in props]
    if not names:
        return "BlueZones::NONE"
    out = "BlueZones::" + names[0]
    for n in names[1:]:
        out += ".union(BlueZones::" + n + ")"
    return out


def generate(tree):
    hb = hb_scripts(tree)
    script_list = scripts(tree)
    script_index = {s["upper"]: i for i, s in enumerate(script_list)}
    style_list = styles(tree)
    cover = coverages(tree)
    stringsets = blues(tree)
    uniranges = ranges(tree)
    group_of = {}
    for st in style_list:
        group_of[st["script"]] = GROUPS[st["system"]]

    buf = "// Generated by generate_styles.py from FreeType's afscript.h, afstyles.h, afcover.h, afblue.dat,\n"
    buf += "// afranges.c and hb-script-list.h (2.14.3), with FreeType 2.15.0's range changes; don't edit.\n\n"
    buf += "#[rustfmt::skip]\n"
    buf += "pub const SCRIPT_CLASSES: &[ScriptClass] = &[\n"
    for s in script_list:
        tag = hb[s["hb"]]
        layout = tag is not None
        if tag is None:
            tag = (s["lower"].capitalize() + "    ")[:4]
        buf += "    ScriptClass {\n"
        buf += '        name: "{}",\n'.format(s["name"])
        buf += "        group: ScriptGroup::{},\n".format(group_of.get(s["upper"], "Default"))
        buf += '        tag: Tag::new(b"{}"),\n'.format(tag)
        buf += "        layout: {},\n".format(str(layout).lower())
        buf += "        hint_top_to_bottom: {},\n".format(str(s["top_to_bottom"]).lower())
        buf += '        std_chars: "{}",\n'.format(rust_string(s["std_chars"]))
        bs = stringsets.get(s["upper"], [])
        if bs:
            buf += "        blues: &[\n"
            for string, props in bs:
                buf += '            ("{}", {}),\n'.format(rust_string(string), zones(props))
            buf += "        ],\n"
        else:
            buf += "        blues: &[],\n"
        buf += "    },\n"
    buf += "];\n\n"
    buf += "#[allow(unused)]\nimpl ScriptClass {\n"
    for i, s in enumerate(script_list):
        buf += "    pub const {}: usize = {};\n".format(s["upper"], i)
    buf += "}\n\n"

    buf += "#[rustfmt::skip]\n"
    buf += "pub const STYLE_CLASSES: &[StyleClass] = &[\n"
    style_names = []
    for i, st in enumerate(style_list):
        script = script_list[script_index[st["script"]]]
        if st["coverage"] == "DEFAULT":
            name, feature = script["name"], "None"
            style_names.append(st["script"])
        else:
            desc, tag = cover[st["coverage"]]
            name, feature = script["name"] + " " + desc, 'Some(Tag::new(b"{}"))'.format(tag)
            style_names.append(st["script"] + "_" + tag.upper())
        buf += '    StyleClass {{ name: "{}", index: {}, script: &SCRIPT_CLASSES[{}], feature: {} }},\n'.format(
            name, i, script_index[st["script"]], feature
        )
    buf += "];\n\n"
    buf += "#[allow(unused)]\nimpl StyleClass {\n"
    for i, n in enumerate(style_names):
        buf += "    pub const {}: usize = {};\n".format(n, i)
    buf += "}\n\n"

    char_map = {}
    for i, st in enumerate(style_list):
        if st["coverage"] != "DEFAULT":
            continue
        lower = script_list[script_index[st["script"]]]["lower"]
        bases = set()
        for first, last in uniranges.get((lower, False), []):
            for ch in range(first, last + 1):
                if ch not in char_map:
                    char_map[ch] = (i, False)
                    bases.add(ch)
        for first, last in uniranges.get((lower, True), []):
            for ch in range(first, last + 1):
                if ch in bases:
                    char_map[ch] = (i, True)
    default_style = {st["script"]: i for i, st in enumerate(style_list) if st["coverage"] == "DEFAULT"}
    for first, last, script, non_base in FREETYPE_2_15:
        for ch in range(first, last + 1):
            if script is None:
                char_map.pop(ch, None)
            else:
                char_map[ch] = (default_style[script], non_base)
    merged = []
    for ch in sorted(char_map):
        props = char_map[ch]
        if merged and ch == merged[-1][1] + 1 and merged[-1][2] == props:
            merged[-1] = (merged[-1][0], ch, props)
        else:
            merged.append((ch, ch, props))
    buf += "#[rustfmt::skip]\n"
    buf += "pub(super) const STYLE_RANGES: &[StyleRange] = &[\n"
    for first, last, (index, non_base) in merged:
        buf += "    {}({}, {}, {}),\n".format("non_base_range" if non_base else "base_range", first, last, index)
    buf += "];\n"
    return buf


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: generate_styles.py FREETYPE_SOURCE_TREE")
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "styles.rs")
    with open(out, "w", encoding="utf-8") as f:
        f.write(generate(sys.argv[1]))
