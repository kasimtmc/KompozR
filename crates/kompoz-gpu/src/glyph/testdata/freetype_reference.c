/*
 * The FreeType glyph bitmaps the glyph rasterizer's parity test compares against (freetype-reference.bin):
 * what the C++ Kompoz's font system (platform/common/font_system.cpp) made of the same glyphs with FreeType,
 * which its text goldens were drawn with. Light-hinted outlines (FT_LOAD_TARGET_LIGHT, FT_RENDER_MODE_LIGHT),
 * stretched and slanted ones (FT_Set_Transform), stroked ones (FT_Stroker), blurred ones (raster.hpp's
 * blurCoverage), COLRv0 glyphs (FT_LOAD_COLOR, made straight) and CBDT bitmaps (colorBitmap's box filter). And
 * more of FreeType's automatic hinter: every character of its diacritic adjustment database, at several sizes;
 * the glyphs no character maps to (reached through layout features), by index; a font without instructions
 * hinted by default (the hinter's normal mode); the fonts' glyph style maps.
 *
 * Made again (from the crate's directory, with the FreeType the C++ goldens were made with: Arch Linux's
 * freetype2 2.15.0):
 *
 *   cc -O2 -o /tmp/freetype_reference src/glyph/testdata/freetype_reference.c $(pkg-config --cflags --libs freetype2) -lm
 *   /tmp/freetype_reference src/glyph/testdata/freetype-reference.bin
 *
 * A record: u8 channels (1: coverage, 4: straight RGBA), u16 name length, the name (what was drawn: a mode,
 * the font's path from the crate's directory, px, the character and the mode's parameters, space separated),
 * i32 left, i32 top (FreeType's bitmap_left and bitmap_top: y up), u32 width, u32 height, then the pixels,
 * rows top down. Little endian. A `styles` record (2 channels) is a font's glyph style map instead: a row of
 * u16 values, a glyph's each (the automatic hinter's `glyph-to-script-map` property).
 */

#include <ft2build.h>
#include FT_FREETYPE_H
#include FT_GLYPH_H
#include FT_STROKER_H
#include FT_DRIVER_H
#include FT_MODULE_H

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static FT_Library library;
static FILE* out;

static void put(const void* p, size_t n) { fwrite(p, 1, n, out); }
static void put_u8(uint8_t v) { put(&v, 1); }
static void put_u16(uint16_t v) { put(&v, 2); }
static void put_i32(int32_t v) { put(&v, 4); }
static void put_u32(uint32_t v) { put(&v, 4); }

static void record(int channels, const char* name, int left, int top, int width, int height, const uint8_t* pixels,
                   int pitch) {
    put_u8((uint8_t)channels);
    put_u16((uint16_t)strlen(name));
    put(name, strlen(name));
    put_i32(left);
    put_i32(top);
    put_u32((uint32_t)width);
    put_u32((uint32_t)height);
    for (int y = 0; y < height; ++y) put(pixels + (size_t)y * (size_t)pitch, (size_t)width * (size_t)channels);
}

static FT_Face face_of(const char* path) {
    static char loaded[8][256];
    static FT_Face faces[8];
    for (int i = 0; i < 8; ++i)
        if (faces[i] && strcmp(loaded[i], path) == 0) return faces[i];
    for (int i = 0; i < 8; ++i)
        if (!faces[i]) {
            if (FT_New_Face(library, path, 0, &faces[i]) != 0) {
                fprintf(stderr, "can't open %s\n", path);
                exit(1);
            }
            snprintf(loaded[i], sizeof loaded[i], "%s", path);
            return faces[i];
        }
    exit(1);
}

/* An 8-bit gray bitmap as a record (empty glyphs too). */
static void gray(const char* name, const FT_Bitmap* b, int left, int top) {
    record(1, name, left, top, (int)b->width, (int)b->rows, b->buffer, b->pitch);
}

/* font_system.cpp's glyphAt for an outline glyph: light hinted and rendered. */
static void light(const char* path, int px, unsigned cp) {
    FT_Face face = face_of(path);
    FT_Set_Pixel_Sizes(face, 0, (FT_UInt)px);
    if (FT_Load_Glyph(face, FT_Get_Char_Index(face, cp), FT_LOAD_TARGET_LIGHT) != 0) exit(2);
    if (FT_Render_Glyph(face->glyph, FT_RENDER_MODE_LIGHT) != 0) exit(3);
    char name[256];
    snprintf(name, sizeof name, "light %s %d %X", path, px, cp);
    gray(name, &face->glyph->bitmap, face->glyph->bitmap_left, face->glyph->bitmap_top);
}

/* A glyph by its index, light hinted as an outline glyph (glyphs no character maps to: small capitals and other
 * forms that layout features reach). */
static void glyph_light(const char* path, int px, unsigned gid) {
    FT_Face face = face_of(path);
    FT_Set_Pixel_Sizes(face, 0, (FT_UInt)px);
    if (FT_Load_Glyph(face, gid, FT_LOAD_TARGET_LIGHT) != 0) exit(2);
    if (FT_Render_Glyph(face->glyph, FT_RENDER_MODE_LIGHT) != 0) exit(3);
    char name[256];
    snprintf(name, sizeof name, "glyph %s %d %X", path, px, gid);
    gray(name, &face->glyph->bitmap, face->glyph->bitmap_left, face->glyph->bitmap_top);
}

/* An outline glyph hinted by default (FT_LOAD_DEFAULT) and rendered normally: for a font without TrueType
 * instructions, the automatic hinter in its normal mode, fitting stems across too (as the layers of a COLRv0 glyph
 * are hinted). */
static void normal(const char* path, int px, unsigned cp) {
    FT_Face face = face_of(path);
    FT_Set_Pixel_Sizes(face, 0, (FT_UInt)px);
    if (FT_Load_Glyph(face, FT_Get_Char_Index(face, cp), FT_LOAD_DEFAULT) != 0) exit(2);
    if (FT_Render_Glyph(face->glyph, FT_RENDER_MODE_NORMAL) != 0) exit(3);
    char name[256];
    snprintf(name, sizeof name, "default %s %d %X", path, px, cp);
    gray(name, &face->glyph->bitmap, face->glyph->bitmap_left, face->glyph->bitmap_top);
}

/* The automatic hinter's style of each glyph of a font (its `glyph-to-script-map` property, of a face of its own
 * so that no glyph's hinting has changed it yet): a style index, 0x2000 if the character map gave it, 0x4000
 * non-base, 0x8000 a digit. */
static void styles(const char* path) {
    FT_Face face;
    if (FT_New_Face(library, path, 0, &face) != 0) exit(1);
    FT_Prop_GlyphToScriptMap prop;
    prop.face = face;
    if (FT_Property_Get(library, "autofitter", "glyph-to-script-map", &prop) != 0) exit(4);
    int n = (int)face->num_glyphs;
    uint8_t* values = malloc((size_t)n * 2 + 1);
    for (int g = 0; g < n; ++g) {
        values[2 * g] = (uint8_t)(prop.map[g] & 0xFF);
        values[2 * g + 1] = (uint8_t)(prop.map[g] >> 8);
    }
    char name[256];
    snprintf(name, sizeof name, "styles %s", path);
    record(2, name, 0, 0, n, 1, values, n * 2);
    free(values);
    FT_Done_Face(face);
}

/* font_system.cpp's rasterize in a style: stretched and slanted (FT_Set_Transform: hinted, then transformed),
 * then stroked if asked (FT_Stroker: butt caps, half the width each side), rendered light. */
static void styled(const char* path, int px, unsigned cp, int stroke_px, int join, float miter, float scale_x,
                   float skew_x, int blur) {
    FT_Face face = face_of(path);
    FT_Set_Pixel_Sizes(face, 0, (FT_UInt)px);
    int transformed = scale_x != 1 || skew_x != 0;
    FT_Matrix transform = {(FT_Fixed)(scale_x * 65536), (FT_Fixed)(skew_x * 65536), 0, 0x10000};
    if (transformed) FT_Set_Transform(face, &transform, NULL);
    int plain = stroke_px == 0 && !transformed;
    if (FT_Load_Glyph(face, FT_Get_Char_Index(face, cp), FT_LOAD_TARGET_LIGHT | (plain ? 0 : FT_LOAD_NO_BITMAP)) != 0)
        exit(2);
    if (transformed) FT_Set_Transform(face, NULL, NULL);
    FT_GlyphSlot slot = face->glyph;
    FT_Bitmap bitmap;
    int left, top;
    FT_Glyph glyph = NULL;
    if (stroke_px == 0) {
        if (FT_Render_Glyph(slot, FT_RENDER_MODE_LIGHT) != 0) exit(3);
        bitmap = slot->bitmap;
        left = slot->bitmap_left;
        top = slot->bitmap_top;
    } else {
        FT_Stroker stroker;
        if (FT_Get_Glyph(slot, &glyph) != 0 || FT_Stroker_New(library, &stroker) != 0) exit(4);
        FT_Stroker_Set(stroker, (FT_Fixed)(stroke_px * 32), FT_STROKER_LINECAP_BUTT,
                       join == 'r' ? FT_STROKER_LINEJOIN_ROUND
                       : join == 'b' ? FT_STROKER_LINEJOIN_BEVEL
                                     : FT_STROKER_LINEJOIN_MITER,
                       (FT_Fixed)(miter * 65536));
        if (FT_Glyph_Stroke(&glyph, stroker, 1) != 0 || FT_Glyph_To_Bitmap(&glyph, FT_RENDER_MODE_LIGHT, NULL, 1) != 0)
            exit(5);
        FT_Stroker_Done(stroker);
        FT_BitmapGlyph b = (FT_BitmapGlyph)glyph;
        bitmap = b->bitmap;
        left = b->left;
        top = b->top;
    }
    char name[256];
    if (blur > 0) {
        /* font_system.cpp's blurredGlyph: raster.hpp's blurCoverage over the coverage padded by 3 sigma. */
        float sigma = 0.57735f * (float)blur + 0.5f;
        int pad = (int)ceilf(sigma * 3);
        int w = (int)bitmap.width, h = (int)bitmap.rows, cw = w + 2 * pad, ch = h + 2 * pad;
        float* a = calloc((size_t)cw * (size_t)ch, sizeof(float));
        float* t = calloc((size_t)cw * (size_t)ch, sizeof(float));
        for (int row = 0; row < h; ++row)
            for (int col = 0; col < w; ++col)
                a[(size_t)(row + pad) * (size_t)cw + (size_t)(col + pad)] =
                    (float)bitmap.buffer[row * bitmap.pitch + col] / 255.f;
        float ideal = sqrtf(12 * sigma * sigma / 3 + 1);
        int wl = (int)floorf(ideal);
        if (wl % 2 == 0) --wl;
        int m = (int)lroundf((12 * sigma * sigma - 3.f * (float)(wl * wl) - 12.f * (float)wl - 9) /
                             (-4.f * (float)wl - 4));
#define AT(v, x, y)                                                                                                   \
    v[(size_t)((y) < 0 ? 0 : (y) > ch - 1 ? ch - 1 : (y)) * (size_t)cw + (size_t)((x) < 0 ? 0 : (x) > cw - 1 ? cw - 1 : (x))]
        for (int pass = 0; pass < 3; ++pass) {
            int k = ((pass < m ? wl : wl + 2) - 1) / 2;
            float n = (float)(2 * k + 1);
            for (int y = 0; y < ch; ++y) {
                float sum = 0;
                for (int x = -k; x <= k; ++x) sum += AT(a, x, y);
                for (int x = 0; x < cw; ++x) {
                    t[(size_t)y * (size_t)cw + (size_t)x] = sum / n;
                    sum += AT(a, x + k + 1, y) - AT(a, x - k, y);
                }
            }
            for (int x = 0; x < cw; ++x) {
                float sum = 0;
                for (int y = -k; y <= k; ++y) sum += AT(t, x, y);
                for (int y = 0; y < ch; ++y) {
                    a[(size_t)y * (size_t)cw + (size_t)x] = sum / n;
                    sum += AT(t, x, y + k + 1) - AT(t, x, y - k);
                }
            }
        }
        uint8_t* alpha = malloc((size_t)cw * (size_t)ch);
        for (size_t i = 0; i < (size_t)cw * (size_t)ch; ++i)
            alpha[i] = (uint8_t)lroundf((a[i] < 0 ? 0 : a[i] > 1 ? 1 : a[i]) * 255);
        snprintf(name, sizeof name, "blur %s %d %X %d %c %g %g %g %d", path, px, cp, stroke_px, join, miter, scale_x,
                 skew_x, blur);
        record(1, name, left - pad, top + pad, cw, ch, alpha, cw);
        free(a);
        free(t);
        free(alpha);
    } else {
        snprintf(name, sizeof name, "styled %s %d %X %d %c %g %g %g", path, px, cp, stroke_px, join, miter, scale_x,
                 skew_x);
        gray(name, &bitmap, left, top);
    }
    if (glyph) FT_Done_Glyph(glyph);
}

/* font_system.cpp's scalableColorGlyph: a COLRv0 glyph composited by FreeType (premultiplied BGRA), made
 * straight RGBA. */
static void colr(const char* path, int px, unsigned cp) {
    FT_Face face = face_of(path);
    FT_Set_Pixel_Sizes(face, 0, (FT_UInt)px);
    if (FT_Load_Glyph(face, FT_Get_Char_Index(face, cp), FT_LOAD_COLOR) != 0) exit(2);
    FT_GlyphSlot slot = face->glyph;
    if (FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL) != 0) exit(3);
    char name[256];
    snprintf(name, sizeof name, "colr %s %d %X", path, px, cp);
    if (slot->bitmap.pixel_mode != FT_PIXEL_MODE_BGRA) {
        gray(name, &slot->bitmap, slot->bitmap_left, slot->bitmap_top);
        return;
    }
    int w = (int)slot->bitmap.width, h = (int)slot->bitmap.rows;
    uint8_t* rgba = malloc((size_t)w * (size_t)h * 4 + 1);
    for (int y = 0; y < h; ++y)
        for (int x = 0; x < w; ++x) {
            const uint8_t* s = slot->bitmap.buffer + y * slot->bitmap.pitch + x * 4;
            uint8_t* d = &rgba[((size_t)y * (size_t)w + (size_t)x) * 4];
            float a = s[3] / 255.f;
            d[0] = (uint8_t)(a > 0 ? fminf(255.f, s[2] / a) : 0);
            d[1] = (uint8_t)(a > 0 ? fminf(255.f, s[1] / a) : 0);
            d[2] = (uint8_t)(a > 0 ? fminf(255.f, s[0] / a) : 0);
            d[3] = s[3];
        }
    record(4, name, slot->bitmap_left, slot->bitmap_top, w, h, rgba, w * 4);
    free(rgba);
}

/* font_system.cpp's colorBitmap: a bitmap font's glyph from its first strike, scaled to px by a box filter. */
static void bitmap(const char* path, int px, unsigned cp) {
    FT_Face face = face_of(path);
    FT_Select_Size(face, 0);
    float strike = (float)face->available_sizes[0].y_ppem / 64.f;
    if (strike <= 0) strike = (float)face->available_sizes[0].height;
    float k = strike > 0 ? (float)px / strike : 1;
    if (FT_Load_Glyph(face, FT_Get_Char_Index(face, cp), FT_LOAD_COLOR) != 0) exit(2);
    FT_GlyphSlot slot = face->glyph;
    if (slot->format != FT_GLYPH_FORMAT_BITMAP && FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL) != 0) exit(3);
    const FT_Bitmap* b = &slot->bitmap;
    int sw = (int)b->width, sh = (int)b->rows;
    int bgra = b->pixel_mode == FT_PIXEL_MODE_BGRA;
    int w = (int)lroundf((float)sw * k), h = (int)lroundf((float)sh * k);
    if (w < 1) w = 1;
    if (h < 1) h = 1;
    uint8_t* rgba = malloc((size_t)w * (size_t)h * 4);
    for (int y = 0; y < h; ++y)
        for (int x = 0; x < w; ++x) {
            int x0 = x * sw / w, x1 = (x + 1) * sw / w, y0 = y * sh / h, y1 = (y + 1) * sh / h;
            if (x1 < x0 + 1) x1 = x0 + 1;
            if (y1 < y0 + 1) y1 = y0 + 1;
            float sum[4] = {0, 0, 0, 0};
            for (int sy = y0; sy < y1; ++sy)
                for (int sx = x0; sx < x1; ++sx) {
                    if (bgra) {
                        const uint8_t* p = b->buffer + sy * b->pitch + sx * 4;
                        sum[0] += p[2], sum[1] += p[1], sum[2] += p[0], sum[3] += p[3];
                    } else {
                        uint8_t a = b->buffer[sy * b->pitch + sx];
                        sum[0] += a, sum[1] += a, sum[2] += a, sum[3] += a;
                    }
                }
            float n = (float)((x1 - x0) * (y1 - y0)), a = sum[3] / n;
            uint8_t* d = &rgba[((size_t)y * (size_t)w + (size_t)x) * 4];
            for (int c = 0; c < 3; ++c) d[c] = (uint8_t)(a > 0 ? fminf(255.f, sum[c] / n * 255.f / a) : 0);
            d[3] = (uint8_t)a;
        }
    char name[256];
    snprintf(name, sizeof name, "bitmap %s %d %X", path, px, cp);
    record(4, name, (int)lroundf((float)slot->bitmap_left * k), (int)lroundf((float)slot->bitmap_top * k), w, h, rgba,
           w * 4);
    free(rgba);
}

/* The code points of a UTF-8 string, once each, in order. */
static int code_points(const char* s, unsigned* cps, int max) {
    int n = 0;
    for (const unsigned char* p = (const unsigned char*)s; *p;) {
        unsigned cp;
        if (*p < 0x80) cp = *p++;
        else if (*p < 0xE0) cp = (unsigned)(p[0] & 0x1F) << 6 | (p[1] & 0x3F), p += 2;
        else if (*p < 0xF0) cp = (unsigned)(p[0] & 0x0F) << 12 | (unsigned)(p[1] & 0x3F) << 6 | (p[2] & 0x3F), p += 3;
        else
            cp = (unsigned)(p[0] & 0x07) << 18 | (unsigned)(p[1] & 0x3F) << 12 | (unsigned)(p[2] & 0x3F) << 6 |
                 (p[3] & 0x3F),
            p += 4;
        int seen = 0;
        for (int i = 0; i < n; ++i) seen |= cps[i] == cp;
        if (!seen && cp != ' ' && n < max) cps[n++] = cp;
    }
    return n;
}

int main(int argc, char** argv) {
    if (argc != 2) {
        fprintf(stderr, "usage: %s OUTPUT\n", argv[0]);
        return 1;
    }
    if (FT_Init_FreeType(&library) != 0) return 1;
    out = fopen(argv[1], "wb");
    if (!out) return 1;
    const char* regular = "tests/fonts/Regular.ttf";
    const char* bold = "tests/fonts/Bold.ttf";
    static unsigned cps[1024];
    /* The text golden's glyphs at its sizes (and at the sizes of a 2.625 density's 14 sp, 37 px). */
    int n = code_points("Kompoz İçerik ğüşöç ÇĞİÖŞÜ Shadow small 11 sp spaced turn", cps, 256);
    int sizes[] = {11, 14, 16, 18, 24, 28, 37};
    for (size_t s = 0; s < sizeof sizes / sizeof sizes[0]; ++s)
        for (int i = 0; i < n; ++i) light(regular, sizes[s], cps[i]);
    n = code_points("Bold 0123", cps, 256);
    for (int i = 0; i < n; ++i) light(bold, 18, cps[i]);
    /* Printable ASCII at two more sizes. */
    for (unsigned cp = 0x21; cp < 0x7F; ++cp) light(regular, 13, cp), light(regular, 32, cp);
    /* Stretched and slanted; stroked by each join; blurred (the text golden's shadow, and a stroked one's). */
    n = code_points("KoAgW@", cps, 256);
    for (int i = 0; i < n; ++i) {
        styled(regular, 24, cps[i], 0, 'm', 4, 1.4f, 0, 0);
        styled(regular, 24, cps[i], 0, 'm', 4, 1, -0.25f, 0);
        styled(regular, 24, cps[i], 0, 'm', 4, 0.8f, 0.3f, 0);
        styled(regular, 24, cps[i], 2, 'm', 4, 1, 0, 0);
        styled(regular, 24, cps[i], 2, 'r', 4, 1, 0, 0);
        styled(regular, 24, cps[i], 3, 'b', 4, 1, 0, 0);
        styled(regular, 16, cps[i], 1, 'm', 4, 1, 0, 0);
    }
    n = code_points("Shadow", cps, 256);
    for (int i = 0; i < n; ++i) {
        styled(regular, 28, cps[i], 0, 'm', 4, 1, 0, 4);
        styled(regular, 16, cps[i], 0, 'm', 4, 1, 0, 2);
        styled(regular, 24, cps[i], 2, 'm', 4, 1, 0, 3);
    }
    /* Colour glyphs: COLRv0 layers, CBDT emoji. */
    const char* colr0 = "../kompoz-text/tests/fonts/KompozCOLRv0.ttf";
    const char* emoji = "../kompoz-text/tests/fonts/NotoColorEmoji-Subset.ttf";
    int colr_sizes[] = {12, 16, 24, 40};
    for (size_t s = 0; s < sizeof colr_sizes / sizeof colr_sizes[0]; ++s) {
        colr(colr0, colr_sizes[s], 0xE000);
        colr(colr0, colr_sizes[s], 0x2B50);
        bitmap(emoji, colr_sizes[s], 0x1F44D);
        bitmap(emoji, colr_sizes[s], 0x1F600);
    }
    /* FreeType 2.14's diacritic adjustments: every character of its adjustment database the font has, at two sizes
     * (and in bold at one); the Turkish letters at many sizes; the characters whose tilde is stretched or whose
     * blue zones are ignored above or below their base (a cedilla, an ogonek, a hook) at four. */
    static const char database[] =
        "!?Qij¡¦ªº¿ÀÁÂÃÄÅÇÈÉÊËÌÍÎÏÑÒÓÔÕÖØÙÚÛÜÝàáâãäåçèéêë"
        "ìíîïñòóôõöøùúûüýÿĀāĂăĄąĆćĈĉĊċČčĎĒēĔĕĖėĘęĚěĜĝĞğĠġ"
        "ĢģĤĥĨĩĪīĬĭĮįİĳĴĵĶķĹĺĻļŃńŅņŇňŌōŎŏŐőŔŕŖŗŘřŚśŜŝŞşŠš"
        "ŢţŤŨũŪūŬŭŮůŰűŲųŴŵŶŷŸŹźŻżŽžƇƈƠơƥƫƮƯưƴǃǄǌǍǎǏǐǑǒǓǔǕ"
        "ǖǗǘǙǚǛǜǞǟǠǡǢǣǦǧǨǩǪǫǬǭǮǯǰǴǵǸǹǺǻǼǽǾǿȀȁȂȃȄȅȆȇȈȉȊȋȌȍ"
        "ȎȏȐȑȒȓȔȕȖȗȘșȚțȞȟȤȥȦȧȨȩȪȫȬȭȮȯȰȱȲȳȺȻȿɀɉɖɠɧɨɲɳɻɽʂʈʐ"
        "ʛʠʲʵΐΪΫάέήίΰϊϋόύώϏϔϗϙϳЀЁЃЇЌЍЎЏЙЦЩйцщѐёѓіїјќѝўџѶѷ"
        "ѼѽѾѿҀҁҊҋҐґҖҗҘҙҚқҢңҪҫҬҭҲҳҴҵҶҷҾҿӁӂӅӆӉӊӋӌӍӎӐӑӒӓӖӗӚӛ"
        "ӜӝӞӟӢӣӤӥӦӧӪӫӬӭӮӯӰӱӲӳӴӵӶӷӸӹӺӻԆԇԤԥԦԧԮԯᴉᵎᵑᵢᶀᶁᶂᶄᶅᶆᶇᶉ"
        "ᶊᶌᶍᶎᶏᶐᶑᶒᶓᶔᶕᶖᶗᶘᶙᶚᶤᶨᶩᶪᶬᶮᶯᶳᶵᶼḀḁḂḃḄḅḆḇḈḉḊḋḌḍḎḏḐḑḒḓḔḕ"
        "ḖḗḘḙḚḛḜḝḞḟḠḡḢḣḤḥḦḧḨḩḪḫḬḭḮḯḰḱḲḳḴḵḶḷḸḹḺḻḼḽḾḿṀṁṂṃṄṅ"
        "ṆṇṈṉṊṋṌṍṎṏṐṑṒṓṔṕṖṗṘṙṚṛṜṝṞṟṠṡṢṣṤṥṦṧṨṩṪṫṬṭṮṯṰṱṲṳṴṵ"
        "ṶṷṸṹṺṻṼṽṾṿẀẁẂẃẄẅẆẇẈẉẊẋẌẍẎẏẐẑẒẓẔẕẖẗẘẙẚẛẠạẢảẤấẦầẨẩ"
        "ẪẫẬậẮắẰằẲẳẴẵẶặẸẹẺẻẼẽẾếỀềỂểỄễỆệỈỉỊịỌọỎỏỐốỒồỔổỖỗỘộ"
        "ỚớỜờỞởỠỡỢợỤụỦủỨứỪừỬửỮữỰựỲỳỴỵỶỷỸỹἀἁἂἃἄἅἆἇἐἑἒἓἔἕἠἡ"
        "ἢἣἤἥἦἧἰἱἲἳἴἵἶἷὀὁὂὃὄὅὐὑὒὓὔὕὖὗὠὡὢὣὤὥὦὧὰάὲέὴήὶίὸόὺύ"
        "ὼώᾀᾁᾂᾃᾄᾅᾆᾇᾈᾉᾊᾋᾌᾍᾎᾏᾐᾑᾒᾓᾔᾕᾖᾗᾘᾙᾚᾛᾜᾝᾞᾟᾠᾡᾢᾣᾤᾥᾦᾧᾨᾩᾪᾫᾬᾭ"
        "ᾮᾯᾰᾱᾲᾳᾴᾶᾷᾸᾹᾼῂῃῄῆῇῌῐῑῒΐῖῗῘῙῠῡῢΰῤῥῦῧῨῩῲῳῴῶῷῼ‼‽⁇⁈⁉ⁱ"
        "₫⃀ⱤⱧⱨⱩⱪⱫⱬⱮⱼⱾⱿ⸘⸮ꙀꙁꙂꙃꚀꚁꚈꚉꚊꚋꚎꚏꚐꚑꚖꚗꜦꜧꝖꝘꝱꝲꝳꝴꝶꞐꞑꞔꞕꟀꟁꟄꟅ"
        "Ᶎꬼꭆꭜꭦꭧ";
    n = code_points(database, cps, 1024);
    for (int i = 0; i < n; ++i) light(regular, 11, cps[i]), light(regular, 16, cps[i]);
    n = code_points("çşÇŞğĞıİöÖüÜ", cps, 1024);
    int turkish_sizes[] = {8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 24, 28, 32, 37, 48};
    for (size_t s = 0; s < sizeof turkish_sizes / sizeof turkish_sizes[0]; ++s)
        for (int i = 0; i < n; ++i) light(regular, turkish_sizes[s], cps[i]);
    static const char tildes_and_attached[] =
        "ÃÇÑÕØãçñõøĄąĘęĨĩĮįŞşŢţŨũŲųƇƈƠơƥƫƮƯưƴǪǫǬǭȤȥȨȩȺȻȿɀ"
        "ɖɠɧɲɳɻɽʂʈʐʛʠʵϏϗϙЏЦЩцщџҀҁҊҋҐґҖҗҘҙҚқҢңҪҫҬҭҲҳҴҵҶҷҾҿ"
        "ӅӆӉӊӋӌӍӎӶӷӺӻԆԇԤԥԦԧԮԯᵑᶀᶁᶂᶄᶅᶆᶇᶉᶊᶌᶍᶎᶏᶐᶑᶒᶓᶔᶕᶖᶗᶘᶙᶚᶩᶪᶬ"
        "ᶮᶯᶳᶵᶼḈḉḚḛḜḝḨḩḬḭṌṍṎṏṴṵṸṹṼṽẪẫẴẵẼẽỄễỖỗỚớỜờỞởỠỡỢợỨứỪ"
        "ừỬửỮữỰựỸỹἆἇἦἧἶἷὖὗὦὧᾆᾇᾖᾗᾦᾧᾶᾷῆῇῖῗῦῧῶῷⱤⱧⱨⱩⱪⱫⱬⱮⱾⱿꙀꙁꙂ"
        "ꙃꚀꚁꚈꚉꚊꚋꚎꚏꚐꚑꚖꚗꜦꜧꝖꝘꝱꝲꝳꝴꝶꞐꞑꞔꞕꟀꟁꟄꟅꟆꬼꭆꭜꭦꭧ";
    n = code_points(tildes_and_attached, cps, 1024);
    int special_sizes[] = {9, 12, 15, 21};
    for (size_t s = 0; s < sizeof special_sizes / sizeof special_sizes[0]; ++s)
        for (int i = 0; i < n; ++i) light(regular, special_sizes[s], cps[i]);
    for (int i = 0; i < n; ++i) light(bold, 14, cps[i]);
    /* The glyphs no character maps to (reached through layout features: their diacritics adjusted by the
     * characters FreeType's reverse character map follows GSUB's single and alternate substitutions to). */
    {
        FT_Face face = face_of(regular);
        static unsigned char mapped[65536];
        FT_UInt gid;
        for (FT_ULong cp = FT_Get_First_Char(face, &gid); gid != 0; cp = FT_Get_Next_Char(face, cp, &gid))
            if (gid < 65536) mapped[gid] = 1;
        for (FT_Long g = 1; g < face->num_glyphs && g < 65536; ++g)
            if (!mapped[g]) glyph_light(regular, 14, (unsigned)g);
    }
    /* The automatic hinter in its normal mode (a font without instructions, hinted by default). */
    const char* unhinted = "../kompoz-text/tests/fonts/NotoSans-Subset.ttf";
    {
        FT_Face face = face_of(unhinted);
        int default_sizes[] = {11, 14, 18, 24};
        for (size_t s = 0; s < sizeof default_sizes / sizeof default_sizes[0]; ++s) {
            FT_UInt gid;
            for (FT_ULong cp = FT_Get_First_Char(face, &gid); gid != 0; cp = FT_Get_Next_Char(face, cp, &gid))
                if (cp != ' ') normal(unhinted, default_sizes[s], (unsigned)cp);
        }
    }
    /* The style maps of the test fonts. */
    const char* style_fonts[] = {
        regular,
        "tests/fonts/Medium.ttf",
        bold,
        unhinted,
        "../kompoz-text/tests/fonts/NotoSansArabic-Subset.ttf",
        "../kompoz-text/tests/fonts/NotoSansDevanagari-Subset.ttf",
        "../kompoz-text/tests/fonts/NotoSansHebrew-Subset.ttf",
        "../kompoz-text/tests/fonts/KompozTest.ttf",
        "../kompoz-text/tests/fonts/Cantarell-VF-Subset.otf",
        colr0,
        emoji,
    };
    for (size_t i = 0; i < sizeof style_fonts / sizeof style_fonts[0]; ++i) styles(style_fonts[i]);
    fclose(out);
    FT_Done_FreeType(library);
    return 0;
}
