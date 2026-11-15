//! FreeType 2.14's diacritic adjustments in its automatic hinter (afadjust.c, and aflatin.c's
//! af_glyph_hints_apply_vertical_separation_adjustments), which skrifa's port of the hinter predates: after hinting,
//! an accent above (below) its base glyph that has come within a pixel of it is moved up (down) until a pixel lies
//! between them, by the adjustment database's flags for the glyph's character.
//!
//! The glyph's character comes from the font's cmap, as FreeType's reverse character map has it without HarfBuzz
//! (FreeType with HarfBuzz also follows the script's GSUB substitutions from those characters: a glyph reached only
//! through one, a small capital, say, isn't adjusted here). The accent's height is checked against the hinter's blue
//! zones at the size as FreeType checks it. FreeType's other 2.14 diacritic steps run inside the hinter, before edges
//! are fitted (a tilde stretched and aligned; blue zones ignored below a cedilla or ogonek, above a hook), so they
//! can't be added after it: glyphs flagged for them are hinted as FreeType 2.13 hinted them.
//!
//! The database is FreeType's, in its order (which has `Q` before `?`, so that FreeType's binary search misses `Q`;
//! the search here is FreeType's too). FreeType is under the FreeType License (or the GPL); this follows its code.

use super::raster::FixedOutline;
use skrifa::outline::autohint::{BlueZones, GlyphStyles, Scale, ScaledBlue, UnscaledStyleMetrics};
use skrifa::outline::{SmoothMode, Target};
use skrifa::raw::TableProvider;
use skrifa::raw::types::F2Dot14;
use skrifa::{Font, FontRef, GlyphId, MetadataProvider};
use std::collections::HashMap;

/// Push the topmost contour up a pixel clear of the others.
pub const UP: u32 = 0x01;
/// Push the bottommost contour down a pixel clear of the others.
pub const DOWN: u32 = 0x02;
/// Push the contour below the topmost one up, with it.
pub const UP2: u32 = 0x04;
/// Push the contour above the bottommost one down, with it.
pub const DOWN2: u32 = 0x08;
/// The topmost contour is a tilde.
pub const TILDE_TOP: u32 = 0x10;
/// The bottommost contour is a tilde.
pub const TILDE_BOTTOM: u32 = 0x20;
/// The contour below the topmost one is a tilde.
pub const TILDE_TOP2: u32 = 0x40;
/// The contour above the bottommost one is a tilde.
pub const TILDE_BOTTOM2: u32 = 0x80;
/// Blue zones ignored above an uppercase base.
pub const IGNORE_CAPITAL_TOP: u32 = 0x100;
/// Blue zones ignored below an uppercase base.
pub const IGNORE_CAPITAL_BOTTOM: u32 = 0x200;
/// Blue zones ignored above a lowercase base.
pub const IGNORE_SMALL_TOP: u32 = 0x400;
/// Blue zones ignored below a lowercase base.
pub const IGNORE_SMALL_BOTTOM: u32 = 0x800;
/// No check of the diacritic's height.
pub const NO_HEIGHT_CHECK: u32 = 0x1000;

/// FreeType's adjustment database: characters and their flags, in FreeType's order.
#[rustfmt::skip]
static DATABASE: &[(u32, u32)] = &[
    // C0 Controls and Basic Latin
    (0x21, UP | NO_HEIGHT_CHECK),
    (0x51, IGNORE_CAPITAL_BOTTOM),
    (0x3F, UP | NO_HEIGHT_CHECK),
    (0x69, UP),
    (0x6A, UP),
    // C1 Controls and Latin-1 Supplement
    (0xA1, UP),
    (0xA6, UP | NO_HEIGHT_CHECK),
    (0xAA, UP),
    (0xBA, UP),
    (0xBF, UP),
    (0xC0, UP),
    (0xC1, UP),
    (0xC2, UP),
    (0xC3, UP | TILDE_TOP),
    (0xC4, UP),
    (0xC5, UP),
    (0xC7, IGNORE_CAPITAL_BOTTOM),
    (0xC8, UP),
    (0xC9, UP),
    (0xCA, UP),
    (0xCB, UP),
    (0xCC, UP),
    (0xCD, UP),
    (0xCE, UP),
    (0xCF, UP),
    (0xD1, UP | TILDE_TOP),
    (0xD2, UP),
    (0xD3, UP),
    (0xD4, UP),
    (0xD5, UP | TILDE_TOP),
    (0xD6, UP),
    (0xD8, IGNORE_CAPITAL_TOP | IGNORE_CAPITAL_BOTTOM),
    (0xD9, UP),
    (0xDA, UP),
    (0xDB, UP),
    (0xDC, UP),
    (0xDD, UP),
    (0xE0, UP),
    (0xE1, UP),
    (0xE2, UP),
    (0xE3, UP | TILDE_TOP),
    (0xE4, UP),
    (0xE5, UP),
    (0xE7, IGNORE_SMALL_BOTTOM),
    (0xE8, UP),
    (0xE9, UP),
    (0xEA, UP),
    (0xEB, UP),
    (0xEC, UP),
    (0xED, UP),
    (0xEE, UP),
    (0xEF, UP),
    (0xF1, UP | TILDE_TOP),
    (0xF2, UP),
    (0xF3, UP),
    (0xF4, UP),
    (0xF5, UP | TILDE_TOP),
    (0xF6, UP),
    (0xF8, IGNORE_SMALL_TOP | IGNORE_SMALL_BOTTOM),
    (0xF9, UP),
    (0xFA, UP),
    (0xFB, UP),
    (0xFC, UP),
    (0xFD, UP),
    (0xFF, UP),
    // Latin Extended-A
    (0x100, UP),
    (0x101, UP),
    (0x102, UP),
    (0x103, UP),
    (0x104, IGNORE_CAPITAL_BOTTOM),
    (0x105, IGNORE_SMALL_BOTTOM),
    (0x106, UP),
    (0x107, UP),
    (0x108, UP),
    (0x109, UP),
    (0x10A, UP),
    (0x10B, UP),
    (0x10C, UP),
    (0x10D, UP),
    (0x10E, UP),
    (0x112, UP),
    (0x113, UP),
    (0x114, UP),
    (0x115, UP),
    (0x116, UP),
    (0x117, UP),
    (0x118, IGNORE_CAPITAL_BOTTOM),
    (0x119, IGNORE_SMALL_BOTTOM),
    (0x11A, UP),
    (0x11B, UP),
    (0x11C, UP),
    (0x11D, UP),
    (0x11E, UP),
    (0x11F, UP),
    (0x120, UP),
    (0x121, UP),
    (0x122, DOWN),
    (0x123, UP),
    (0x124, UP),
    (0x125, UP),
    (0x128, UP | TILDE_TOP),
    (0x129, UP | TILDE_TOP),
    (0x12A, UP),
    (0x12B, UP),
    (0x12C, UP),
    (0x12D, UP),
    (0x12E, IGNORE_CAPITAL_BOTTOM),
    (0x12F, UP | IGNORE_SMALL_BOTTOM),
    (0x130, UP),
    (0x133, UP),
    (0x134, UP),
    (0x135, UP),
    (0x136, DOWN),
    (0x137, DOWN),
    (0x139, UP),
    (0x13A, UP),
    (0x13B, DOWN),
    (0x13C, DOWN),
    (0x143, UP),
    (0x144, UP),
    (0x145, DOWN),
    (0x146, DOWN),
    (0x147, UP),
    (0x148, UP),
    (0x14C, UP),
    (0x14D, UP),
    (0x14E, UP),
    (0x14F, UP),
    (0x150, UP),
    (0x151, UP),
    (0x154, UP),
    (0x155, UP),
    (0x156, DOWN),
    (0x157, DOWN),
    (0x158, UP),
    (0x159, UP),
    (0x15A, UP),
    (0x15B, UP),
    (0x15C, UP),
    (0x15D, UP),
    (0x15E, IGNORE_CAPITAL_BOTTOM),
    (0x15F, IGNORE_SMALL_BOTTOM),
    (0x160, UP),
    (0x161, UP),
    (0x162, IGNORE_CAPITAL_BOTTOM),
    (0x163, IGNORE_SMALL_BOTTOM),
    (0x164, UP),
    (0x168, UP | TILDE_TOP),
    (0x169, UP | TILDE_TOP),
    (0x16A, UP),
    (0x16B, UP),
    (0x16C, UP),
    (0x16D, UP),
    (0x16E, UP),
    (0x16F, UP),
    (0x170, UP),
    (0x171, UP),
    (0x172, IGNORE_CAPITAL_BOTTOM),
    (0x173, IGNORE_SMALL_BOTTOM),
    (0x174, UP),
    (0x175, UP),
    (0x176, UP),
    (0x177, UP),
    (0x178, UP),
    (0x179, UP),
    (0x17A, UP),
    (0x17B, UP),
    (0x17C, UP),
    (0x17D, UP),
    (0x17E, UP),
    // Latin Extended-B
    (0x187, IGNORE_CAPITAL_TOP),
    (0x188, IGNORE_SMALL_TOP),
    (0x1A0, IGNORE_CAPITAL_TOP),
    (0x1A1, IGNORE_SMALL_TOP),
    (0x1A5, IGNORE_SMALL_TOP),
    (0x1AB, IGNORE_SMALL_BOTTOM),
    (0x1AE, IGNORE_CAPITAL_BOTTOM),
    (0x1AF, IGNORE_CAPITAL_TOP),
    (0x1B0, IGNORE_SMALL_TOP),
    (0x1B4, IGNORE_SMALL_TOP),
    (0x1C3, UP | NO_HEIGHT_CHECK),
    (0x1C4, UP),
    (0x1CC, UP),
    (0x1CD, UP),
    (0x1CE, UP),
    (0x1CF, UP),
    (0x1D0, UP),
    (0x1D1, UP),
    (0x1D2, UP),
    (0x1D3, UP),
    (0x1D4, UP),
    (0x1D5, UP2),
    (0x1D6, UP2),
    (0x1D7, UP2),
    (0x1D8, UP2),
    (0x1D9, UP2),
    (0x1DA, UP2),
    (0x1DB, UP2),
    (0x1DC, UP2),
    (0x1DE, UP2),
    (0x1DF, UP2),
    (0x1E0, UP2),
    (0x1E1, UP2),
    (0x1E2, UP),
    (0x1E3, UP),
    (0x1E6, UP),
    (0x1E7, UP),
    (0x1E8, UP),
    (0x1E9, UP),
    (0x1EA, IGNORE_CAPITAL_BOTTOM),
    (0x1EB, IGNORE_SMALL_BOTTOM),
    (0x1EC, UP | IGNORE_CAPITAL_BOTTOM),
    (0x1ED, UP | IGNORE_SMALL_BOTTOM),
    (0x1EE, UP),
    (0x1EF, UP),
    (0x1F0, UP),
    (0x1F4, UP),
    (0x1F5, UP),
    (0x1F8, UP),
    (0x1F9, UP),
    (0x1FA, UP2),
    (0x1FB, UP2),
    (0x1FC, UP),
    (0x1FD, UP),
    (0x1FE, UP),
    (0x1FF, UP),
    (0x200, UP),
    (0x201, UP),
    (0x202, UP),
    (0x203, UP),
    (0x204, UP),
    (0x205, UP),
    (0x206, UP),
    (0x207, UP),
    (0x208, UP),
    (0x209, UP),
    (0x20A, UP),
    (0x20B, UP),
    (0x20C, UP),
    (0x20D, UP),
    (0x20E, UP),
    (0x20F, UP),
    (0x210, UP),
    (0x211, UP),
    (0x212, UP),
    (0x213, UP),
    (0x214, UP),
    (0x215, UP),
    (0x216, UP),
    (0x217, UP),
    (0x218, DOWN),
    (0x219, DOWN),
    (0x21A, DOWN),
    (0x21B, DOWN),
    (0x21E, UP),
    (0x21F, UP),
    (0x224, IGNORE_CAPITAL_BOTTOM),
    (0x225, IGNORE_SMALL_BOTTOM),
    (0x226, UP),
    (0x227, UP),
    (0x228, IGNORE_CAPITAL_BOTTOM),
    (0x229, IGNORE_SMALL_BOTTOM),
    (0x22A, UP2),
    (0x22B, UP2),
    (0x22C, UP2),
    (0x22D, UP2),
    (0x22E, UP),
    (0x22F, UP),
    (0x230, UP2),
    (0x231, UP2),
    (0x232, UP),
    (0x233, UP),
    (0x23A, IGNORE_CAPITAL_TOP | IGNORE_CAPITAL_BOTTOM),
    (0x23B, IGNORE_CAPITAL_TOP | IGNORE_CAPITAL_BOTTOM),
    (0x23F, IGNORE_SMALL_BOTTOM),
    (0x240, IGNORE_SMALL_BOTTOM),
    (0x249, UP),
    // IPA Extensions
    (0x256, IGNORE_SMALL_BOTTOM),
    (0x260, IGNORE_SMALL_TOP),
    (0x267, IGNORE_SMALL_BOTTOM),
    (0x268, UP),
    (0x272, IGNORE_SMALL_BOTTOM),
    (0x273, IGNORE_SMALL_BOTTOM),
    (0x27B, IGNORE_SMALL_BOTTOM),
    (0x27D, IGNORE_SMALL_BOTTOM),
    (0x282, IGNORE_SMALL_BOTTOM),
    (0x288, IGNORE_SMALL_BOTTOM),
    (0x290, IGNORE_SMALL_BOTTOM),
    (0x29B, IGNORE_SMALL_TOP),
    (0x2A0, IGNORE_SMALL_TOP),
    // Spacing Modifier Letters
    (0x2B2, UP),
    (0x2B5, IGNORE_SMALL_BOTTOM),
    // Greek and Coptic
    (0x390, UP2),
    (0x3AA, UP),
    (0x3AB, UP),
    (0x3AC, UP),
    (0x3AD, UP),
    (0x3AE, UP),
    (0x3AF, UP),
    (0x3B0, UP2),
    (0x3CA, UP),
    (0x3CB, UP),
    (0x3CC, UP),
    (0x3CD, UP),
    (0x3CE, UP),
    (0x3CF, IGNORE_CAPITAL_BOTTOM),
    (0x3D4, UP),
    (0x3D7, IGNORE_SMALL_BOTTOM),
    (0x3D9, IGNORE_SMALL_BOTTOM),
    (0x3E2, IGNORE_CAPITAL_BOTTOM),
    (0x3E3, IGNORE_SMALL_BOTTOM),
    (0x3F3, UP),
    // Cyrillic
    (0x400, UP),
    (0x401, UP),
    (0x403, UP),
    (0x407, UP),
    (0x40C, UP),
    (0x40D, UP),
    (0x40E, UP),
    (0x40F, IGNORE_CAPITAL_BOTTOM),
    (0x419, UP),
    (0x426, IGNORE_CAPITAL_BOTTOM),
    (0x429, IGNORE_CAPITAL_BOTTOM),
    (0x439, UP),
    (0x446, IGNORE_SMALL_BOTTOM),
    (0x449, IGNORE_SMALL_BOTTOM),
    (0x450, UP),
    (0x451, UP),
    (0x453, UP),
    (0x456, UP),
    (0x457, UP),
    (0x458, UP),
    (0x45C, UP),
    (0x45D, UP),
    (0x45E, UP),
    (0x45F, IGNORE_SMALL_BOTTOM),
    (0x476, UP),
    (0x477, UP),
    (0x47C, UP2),
    (0x47D, UP2),
    (0x47E, UP),
    (0x47F, UP),
    (0x480, IGNORE_CAPITAL_BOTTOM),
    (0x481, IGNORE_SMALL_BOTTOM),
    (0x48A, UP | IGNORE_CAPITAL_BOTTOM),
    (0x48B, UP | IGNORE_SMALL_BOTTOM),
    (0x490, IGNORE_CAPITAL_TOP),
    (0x491, IGNORE_SMALL_TOP),
    (0x496, IGNORE_CAPITAL_BOTTOM),
    (0x497, IGNORE_SMALL_BOTTOM),
    (0x498, IGNORE_CAPITAL_BOTTOM),
    (0x499, IGNORE_SMALL_BOTTOM),
    (0x49A, IGNORE_CAPITAL_BOTTOM),
    (0x49B, IGNORE_SMALL_BOTTOM),
    (0x4A2, IGNORE_CAPITAL_BOTTOM),
    (0x4A3, IGNORE_SMALL_BOTTOM),
    (0x4AA, IGNORE_CAPITAL_BOTTOM),
    (0x4AB, IGNORE_SMALL_BOTTOM),
    (0x4AC, IGNORE_CAPITAL_BOTTOM),
    (0x4AD, IGNORE_SMALL_BOTTOM),
    (0x4B2, IGNORE_CAPITAL_BOTTOM),
    (0x4B3, IGNORE_SMALL_BOTTOM),
    (0x4B4, IGNORE_CAPITAL_BOTTOM),
    (0x4B5, IGNORE_SMALL_BOTTOM),
    (0x4B6, IGNORE_CAPITAL_BOTTOM),
    (0x4B7, IGNORE_SMALL_BOTTOM),
    (0x4BE, IGNORE_CAPITAL_BOTTOM),
    (0x4BF, IGNORE_SMALL_BOTTOM),
    (0x4C1, UP),
    (0x4C2, UP),
    (0x4C5, IGNORE_CAPITAL_BOTTOM),
    (0x4C6, IGNORE_SMALL_BOTTOM),
    (0x4C9, IGNORE_CAPITAL_BOTTOM),
    (0x4CA, IGNORE_SMALL_BOTTOM),
    (0x4CB, IGNORE_CAPITAL_BOTTOM),
    (0x4CC, IGNORE_SMALL_BOTTOM),
    (0x4CD, IGNORE_CAPITAL_BOTTOM),
    (0x4CE, IGNORE_SMALL_BOTTOM),
    (0x4D0, UP),
    (0x4D1, UP),
    (0x4D2, UP),
    (0x4D3, UP),
    (0x4D6, UP),
    (0x4D7, UP),
    (0x4DA, UP),
    (0x4DB, UP),
    (0x4DC, UP),
    (0x4DD, UP),
    (0x4DE, UP),
    (0x4DF, UP),
    (0x4E2, UP),
    (0x4E3, UP),
    (0x4E4, UP),
    (0x4E5, UP),
    (0x4E6, UP),
    (0x4E7, UP),
    (0x4EA, UP),
    (0x4EB, UP),
    (0x4EC, UP),
    (0x4ED, UP),
    (0x4EE, UP),
    (0x4EF, UP),
    (0x4F0, UP),
    (0x4F1, UP),
    (0x4F2, UP),
    (0x4F3, UP),
    (0x4F4, UP),
    (0x4F5, UP),
    (0x4F6, IGNORE_CAPITAL_BOTTOM),
    (0x4F7, IGNORE_SMALL_BOTTOM),
    (0x4F8, UP),
    (0x4F9, UP),
    (0x4FA, IGNORE_CAPITAL_BOTTOM),
    (0x4FB, IGNORE_SMALL_BOTTOM),
    // Cyrillic Supplement
    (0x506, IGNORE_CAPITAL_BOTTOM),
    (0x507, IGNORE_SMALL_BOTTOM),
    (0x524, IGNORE_CAPITAL_BOTTOM),
    (0x525, IGNORE_SMALL_BOTTOM),
    (0x526, IGNORE_CAPITAL_BOTTOM),
    (0x527, IGNORE_SMALL_BOTTOM),
    (0x52E, IGNORE_CAPITAL_BOTTOM),
    (0x52F, IGNORE_SMALL_BOTTOM),
    // Cherokee
    (0x13A5, UP),
    // Phonetic Extensions
    (0x1D09, DOWN),
    (0x1D4E, DOWN),
    (0x1D51, IGNORE_SMALL_BOTTOM),
    (0x1D62, UP),
    // Phonetic Extensions Supplement
    (0x1D80, IGNORE_SMALL_BOTTOM),
    (0x1D81, IGNORE_SMALL_BOTTOM),
    (0x1D82, IGNORE_SMALL_BOTTOM),
    (0x1D84, IGNORE_SMALL_BOTTOM),
    (0x1D85, IGNORE_SMALL_BOTTOM),
    (0x1D86, IGNORE_SMALL_BOTTOM),
    (0x1D87, IGNORE_SMALL_BOTTOM),
    (0x1D89, IGNORE_SMALL_BOTTOM),
    (0x1D8A, IGNORE_SMALL_BOTTOM),
    (0x1D8C, IGNORE_SMALL_BOTTOM),
    (0x1D8D, IGNORE_SMALL_BOTTOM),
    (0x1D8E, IGNORE_SMALL_BOTTOM),
    (0x1D8F, IGNORE_SMALL_BOTTOM),
    (0x1D90, IGNORE_SMALL_BOTTOM),
    (0x1D91, IGNORE_SMALL_BOTTOM),
    (0x1D92, IGNORE_SMALL_BOTTOM),
    (0x1D93, IGNORE_SMALL_BOTTOM),
    (0x1D94, IGNORE_SMALL_BOTTOM),
    (0x1D95, IGNORE_SMALL_BOTTOM),
    (0x1D96, UP | IGNORE_SMALL_BOTTOM),
    (0x1D97, IGNORE_SMALL_BOTTOM),
    (0x1D98, IGNORE_SMALL_BOTTOM),
    (0x1D99, IGNORE_SMALL_BOTTOM),
    (0x1D9A, IGNORE_SMALL_BOTTOM),
    (0x1DA4, UP),
    (0x1DA8, UP),
    (0x1DA9, IGNORE_SMALL_BOTTOM),
    (0x1DAA, IGNORE_SMALL_BOTTOM),
    (0x1DAC, IGNORE_SMALL_BOTTOM),
    (0x1DAE, IGNORE_SMALL_BOTTOM),
    (0x1DAF, IGNORE_SMALL_BOTTOM),
    (0x1DB3, IGNORE_SMALL_BOTTOM),
    (0x1DB5, IGNORE_SMALL_BOTTOM),
    (0x1DBC, IGNORE_SMALL_BOTTOM),
    // Latin Extended Additional
    (0x1E00, DOWN),
    (0x1E01, DOWN),
    (0x1E02, UP),
    (0x1E03, UP),
    (0x1E04, DOWN),
    (0x1E05, DOWN),
    (0x1E06, DOWN),
    (0x1E07, DOWN),
    (0x1E08, UP | IGNORE_CAPITAL_BOTTOM),
    (0x1E09, UP | IGNORE_SMALL_BOTTOM),
    (0x1E0A, UP),
    (0x1E0B, UP),
    (0x1E0C, DOWN),
    (0x1E0D, DOWN),
    (0x1E0E, DOWN),
    (0x1E0F, DOWN),
    (0x1E10, DOWN),
    (0x1E11, DOWN),
    (0x1E12, DOWN),
    (0x1E13, DOWN),
    (0x1E14, UP2),
    (0x1E15, UP2),
    (0x1E16, UP2),
    (0x1E17, UP2),
    (0x1E18, DOWN),
    (0x1E19, DOWN),
    (0x1E1A, DOWN | TILDE_BOTTOM),
    (0x1E1B, DOWN | TILDE_BOTTOM),
    (0x1E1C, UP | IGNORE_CAPITAL_BOTTOM),
    (0x1E1D, UP | IGNORE_SMALL_BOTTOM),
    (0x1E1E, UP),
    (0x1E1F, UP),
    (0x1E20, UP),
    (0x1E21, UP),
    (0x1E22, UP),
    (0x1E23, UP),
    (0x1E24, DOWN),
    (0x1E25, DOWN),
    (0x1E26, UP),
    (0x1E27, UP),
    (0x1E28, IGNORE_CAPITAL_BOTTOM),
    (0x1E29, IGNORE_SMALL_BOTTOM),
    (0x1E2A, DOWN),
    (0x1E2B, DOWN),
    (0x1E2C, DOWN | TILDE_BOTTOM),
    (0x1E2D, UP | DOWN | TILDE_BOTTOM),
    (0x1E2E, UP2),
    (0x1E2F, UP2),
    (0x1E30, UP),
    (0x1E31, UP),
    (0x1E32, DOWN),
    (0x1E33, DOWN),
    (0x1E34, DOWN),
    (0x1E35, DOWN),
    (0x1E36, DOWN),
    (0x1E37, DOWN),
    (0x1E38, UP | DOWN),
    (0x1E39, UP | DOWN),
    (0x1E3A, DOWN),
    (0x1E3B, DOWN),
    (0x1E3C, DOWN),
    (0x1E3D, DOWN),
    (0x1E3E, UP),
    (0x1E3F, UP),
    (0x1E40, UP),
    (0x1E41, UP),
    (0x1E42, DOWN),
    (0x1E43, DOWN),
    (0x1E44, UP),
    (0x1E45, UP),
    (0x1E46, DOWN),
    (0x1E47, DOWN),
    (0x1E48, DOWN),
    (0x1E49, DOWN),
    (0x1E4A, DOWN),
    (0x1E4B, DOWN),
    (0x1E4C, UP2 | TILDE_TOP2),
    (0x1E4D, UP2 | TILDE_TOP2),
    (0x1E4E, UP2 | TILDE_TOP2),
    (0x1E4F, UP2 | TILDE_TOP2),
    (0x1E50, UP2),
    (0x1E51, UP2),
    (0x1E52, UP2),
    (0x1E53, UP2),
    (0x1E54, UP),
    (0x1E55, UP),
    (0x1E56, UP),
    (0x1E57, UP),
    (0x1E58, UP),
    (0x1E59, UP),
    (0x1E5A, DOWN),
    (0x1E5B, DOWN),
    (0x1E5C, UP | DOWN),
    (0x1E5D, UP | DOWN),
    (0x1E5E, DOWN),
    (0x1E5F, DOWN),
    (0x1E60, UP),
    (0x1E61, UP),
    (0x1E62, DOWN),
    (0x1E63, DOWN),
    (0x1E64, UP),
    (0x1E65, UP),
    (0x1E66, UP),
    (0x1E67, UP),
    (0x1E68, UP | DOWN),
    (0x1E69, UP | DOWN),
    (0x1E6A, UP),
    (0x1E6B, UP),
    (0x1E6C, DOWN),
    (0x1E6D, DOWN),
    (0x1E6E, DOWN),
    (0x1E6F, DOWN),
    (0x1E70, DOWN),
    (0x1E71, DOWN),
    (0x1E72, DOWN),
    (0x1E73, DOWN),
    (0x1E74, DOWN | TILDE_BOTTOM),
    (0x1E75, DOWN | TILDE_BOTTOM),
    (0x1E76, DOWN),
    (0x1E77, DOWN),
    (0x1E78, UP2 | TILDE_TOP2),
    (0x1E79, UP2 | TILDE_TOP2),
    (0x1E7A, UP2),
    (0x1E7B, UP2),
    (0x1E7C, UP | TILDE_TOP),
    (0x1E7D, UP | TILDE_TOP),
    (0x1E7E, DOWN),
    (0x1E7F, DOWN),
    (0x1E80, UP),
    (0x1E81, UP),
    (0x1E82, UP),
    (0x1E83, UP),
    (0x1E84, UP),
    (0x1E85, UP),
    (0x1E86, UP),
    (0x1E87, UP),
    (0x1E88, DOWN),
    (0x1E89, DOWN),
    (0x1E8A, UP),
    (0x1E8B, UP),
    (0x1E8C, UP),
    (0x1E8D, UP),
    (0x1E8E, UP),
    (0x1E8F, UP),
    (0x1E90, UP),
    (0x1E91, UP),
    (0x1E92, DOWN),
    (0x1E93, DOWN),
    (0x1E94, DOWN),
    (0x1E95, DOWN),
    (0x1E96, DOWN),
    (0x1E97, UP),
    (0x1E98, UP),
    (0x1E99, UP),
    (0x1E9A, UP),
    (0x1E9B, UP),
    (0x1EA0, DOWN),
    (0x1EA1, DOWN),
    (0x1EA2, UP),
    (0x1EA3, UP),
    (0x1EA4, UP2),
    (0x1EA5, UP2),
    (0x1EA6, UP2),
    (0x1EA7, UP2),
    (0x1EA8, UP2),
    (0x1EA9, UP2),
    (0x1EAA, UP2 | TILDE_TOP),
    (0x1EAB, UP2 | TILDE_TOP),
    (0x1EAC, UP | DOWN),
    (0x1EAD, UP | DOWN),
    (0x1EAE, UP2),
    (0x1EAF, UP2),
    (0x1EB0, UP2),
    (0x1EB1, UP2),
    (0x1EB2, UP2),
    (0x1EB3, UP2),
    (0x1EB4, UP2 | TILDE_TOP),
    (0x1EB5, UP2 | TILDE_TOP),
    (0x1EB6, UP | DOWN),
    (0x1EB7, UP | DOWN),
    (0x1EB8, DOWN),
    (0x1EB9, DOWN),
    (0x1EBA, UP),
    (0x1EBB, UP),
    (0x1EBC, UP | TILDE_TOP),
    (0x1EBD, UP | TILDE_TOP),
    (0x1EBE, UP2),
    (0x1EBF, UP2),
    (0x1EC0, UP2),
    (0x1EC1, UP2),
    (0x1EC2, UP2),
    (0x1EC3, UP2),
    (0x1EC4, UP2 | TILDE_TOP),
    (0x1EC5, UP2 | TILDE_TOP),
    (0x1EC6, UP | DOWN),
    (0x1EC7, UP | DOWN),
    (0x1EC8, UP),
    (0x1EC9, UP),
    (0x1ECA, DOWN),
    (0x1ECB, UP | DOWN),
    (0x1ECC, DOWN),
    (0x1ECD, DOWN),
    (0x1ECE, UP),
    (0x1ECF, UP),
    (0x1ED0, UP2),
    (0x1ED1, UP2),
    (0x1ED2, UP2),
    (0x1ED3, UP2),
    (0x1ED4, UP2),
    (0x1ED5, UP2),
    (0x1ED6, UP2 | TILDE_TOP),
    (0x1ED7, UP2 | TILDE_TOP),
    (0x1ED8, UP | DOWN),
    (0x1ED9, UP | DOWN),
    (0x1EDA, UP | IGNORE_CAPITAL_TOP),
    (0x1EDB, UP | IGNORE_SMALL_TOP),
    (0x1EDC, UP | IGNORE_CAPITAL_TOP),
    (0x1EDD, UP | IGNORE_SMALL_TOP),
    (0x1EDE, UP | IGNORE_CAPITAL_TOP),
    (0x1EDF, UP | IGNORE_SMALL_TOP),
    (0x1EE0, UP | TILDE_TOP | IGNORE_CAPITAL_TOP),
    (0x1EE1, UP | TILDE_TOP | IGNORE_SMALL_TOP),
    (0x1EE2, DOWN | IGNORE_CAPITAL_TOP),
    (0x1EE3, DOWN | IGNORE_SMALL_TOP),
    (0x1EE4, DOWN),
    (0x1EE5, DOWN),
    (0x1EE6, UP),
    (0x1EE7, UP),
    (0x1EE8, UP | IGNORE_CAPITAL_TOP),
    (0x1EE9, UP | IGNORE_SMALL_TOP),
    (0x1EEA, UP | IGNORE_CAPITAL_TOP),
    (0x1EEB, UP | IGNORE_SMALL_TOP),
    (0x1EEC, UP | IGNORE_CAPITAL_TOP),
    (0x1EED, UP | IGNORE_SMALL_TOP),
    (0x1EEE, UP | TILDE_TOP | IGNORE_CAPITAL_TOP),
    (0x1EEF, UP | TILDE_TOP | IGNORE_SMALL_TOP),
    (0x1EF0, DOWN | IGNORE_CAPITAL_TOP),
    (0x1EF1, DOWN | IGNORE_SMALL_TOP),
    (0x1EF2, UP),
    (0x1EF3, UP),
    (0x1EF4, DOWN),
    (0x1EF5, DOWN),
    (0x1EF6, UP),
    (0x1EF7, UP),
    (0x1EF8, UP | TILDE_TOP),
    (0x1EF9, UP | TILDE_TOP),
    // Greek Extended
    (0x1F00, UP),
    (0x1F01, UP),
    (0x1F02, UP),
    (0x1F03, UP),
    (0x1F04, UP),
    (0x1F05, UP),
    (0x1F06, UP2 | TILDE_TOP),
    (0x1F07, UP2 | TILDE_TOP),
    (0x1F10, UP),
    (0x1F11, UP),
    (0x1F12, UP),
    (0x1F13, UP),
    (0x1F14, UP),
    (0x1F15, UP),
    (0x1F20, UP),
    (0x1F21, UP),
    (0x1F22, UP),
    (0x1F23, UP),
    (0x1F24, UP),
    (0x1F25, UP),
    (0x1F26, UP2 | TILDE_TOP),
    (0x1F27, UP2 | TILDE_TOP),
    (0x1F30, UP),
    (0x1F31, UP),
    (0x1F32, UP),
    (0x1F33, UP),
    (0x1F34, UP),
    (0x1F35, UP),
    (0x1F36, UP2 | TILDE_TOP),
    (0x1F37, UP2 | TILDE_TOP),
    (0x1F40, UP),
    (0x1F41, UP),
    (0x1F42, UP),
    (0x1F43, UP),
    (0x1F44, UP),
    (0x1F45, UP),
    (0x1F50, UP),
    (0x1F51, UP),
    (0x1F52, UP),
    (0x1F53, UP),
    (0x1F54, UP),
    (0x1F55, UP),
    (0x1F56, UP2 | TILDE_TOP),
    (0x1F57, UP2 | TILDE_TOP),
    (0x1F60, UP),
    (0x1F61, UP),
    (0x1F62, UP),
    (0x1F63, UP),
    (0x1F64, UP),
    (0x1F65, UP),
    (0x1F66, UP2 | TILDE_TOP),
    (0x1F67, UP2 | TILDE_TOP),
    (0x1F70, UP),
    (0x1F71, UP),
    (0x1F72, UP),
    (0x1F73, UP),
    (0x1F74, UP),
    (0x1F75, UP),
    (0x1F76, UP),
    (0x1F77, UP),
    (0x1F78, UP),
    (0x1F79, UP),
    (0x1F7A, UP),
    (0x1F7B, UP),
    (0x1F7C, UP),
    (0x1F7D, UP),
    (0x1F80, UP | DOWN),
    (0x1F81, UP | DOWN),
    (0x1F82, UP | DOWN),
    (0x1F83, UP | DOWN),
    (0x1F84, UP | DOWN),
    (0x1F85, UP | DOWN),
    (0x1F86, UP2 | TILDE_TOP | DOWN),
    (0x1F87, UP2 | TILDE_TOP | DOWN),
    (0x1F88, DOWN),
    (0x1F89, DOWN),
    (0x1F8A, DOWN),
    (0x1F8B, DOWN),
    (0x1F8C, DOWN),
    (0x1F8D, DOWN),
    (0x1F8E, DOWN),
    (0x1F8F, DOWN),
    (0x1F90, UP | DOWN),
    (0x1F91, UP | DOWN),
    (0x1F92, UP | DOWN),
    (0x1F93, UP | DOWN),
    (0x1F94, UP | DOWN),
    (0x1F95, UP | DOWN),
    (0x1F96, UP2 | TILDE_TOP | DOWN),
    (0x1F97, UP2 | TILDE_TOP | DOWN),
    (0x1F98, DOWN),
    (0x1F99, DOWN),
    (0x1F9A, DOWN),
    (0x1F9B, DOWN),
    (0x1F9C, DOWN),
    (0x1F9D, DOWN),
    (0x1F9E, DOWN),
    (0x1F9F, DOWN),
    (0x1FA0, UP | DOWN),
    (0x1FA1, UP | DOWN),
    (0x1FA2, UP | DOWN),
    (0x1FA3, UP | DOWN),
    (0x1FA4, UP | DOWN),
    (0x1FA5, UP | DOWN),
    (0x1FA6, UP2 | TILDE_TOP | DOWN),
    (0x1FA7, UP2 | TILDE_TOP | DOWN),
    (0x1FA8, DOWN),
    (0x1FA9, DOWN),
    (0x1FAA, DOWN),
    (0x1FAB, DOWN),
    (0x1FAC, DOWN),
    (0x1FAD, DOWN),
    (0x1FAE, DOWN),
    (0x1FAF, DOWN),
    (0x1FB0, UP),
    (0x1FB1, UP),
    (0x1FB2, UP | DOWN),
    (0x1FB3, DOWN),
    (0x1FB4, UP | DOWN),
    (0x1FB6, UP | TILDE_TOP),
    (0x1FB7, UP | TILDE_TOP | DOWN),
    (0x1FB8, UP),
    (0x1FB9, UP),
    (0x1FBC, DOWN),
    (0x1FC2, UP | DOWN),
    (0x1FC3, DOWN),
    (0x1FC4, UP | DOWN),
    (0x1FC6, UP | TILDE_TOP),
    (0x1FC7, UP | TILDE_TOP | DOWN),
    (0x1FCC, DOWN),
    (0x1FD0, UP),
    (0x1FD1, UP),
    (0x1FD2, UP2),
    (0x1FD3, UP2),
    (0x1FD6, UP | TILDE_TOP),
    (0x1FD7, UP2 | TILDE_TOP),
    (0x1FD8, UP),
    (0x1FD9, UP),
    (0x1FE0, UP),
    (0x1FE1, UP),
    (0x1FE2, UP2),
    (0x1FE3, UP2),
    (0x1FE4, UP),
    (0x1FE5, UP),
    (0x1FE6, UP | TILDE_TOP),
    (0x1FE7, UP2 | TILDE_TOP),
    (0x1FE8, UP),
    (0x1FE9, UP),
    (0x1FF2, UP | DOWN),
    (0x1FF3, DOWN),
    (0x1FF4, UP | DOWN),
    (0x1FF6, UP | TILDE_TOP),
    (0x1FF7, UP | TILDE_TOP | DOWN),
    (0x1FFC, DOWN),
    // General Punctuation
    (0x203C, UP | NO_HEIGHT_CHECK),
    (0x203D, UP | NO_HEIGHT_CHECK),
    (0x2047, UP | NO_HEIGHT_CHECK),
    (0x2048, UP | NO_HEIGHT_CHECK),
    (0x2049, UP | NO_HEIGHT_CHECK),
    // Superscripts and Subscripts
    (0x2071, UP),
    // Currency Symbols
    (0x20AB, DOWN),
    (0x20C0, DOWN),
    // Number Forms
    (0x2170, UP),
    (0x2171, UP),
    (0x2172, UP),
    (0x2173, UP),
    (0x2175, UP),
    (0x2176, UP),
    (0x2177, UP),
    (0x2178, UP),
    (0x217A, UP),
    (0x217B, UP),
    // Latin Extended-C
    (0x2C64, IGNORE_CAPITAL_BOTTOM),
    (0x2C67, IGNORE_CAPITAL_BOTTOM),
    (0x2C68, IGNORE_SMALL_BOTTOM),
    (0x2C69, IGNORE_CAPITAL_BOTTOM),
    (0x2C6A, IGNORE_SMALL_BOTTOM),
    (0x2C6B, IGNORE_CAPITAL_BOTTOM),
    (0x2C6C, IGNORE_SMALL_BOTTOM),
    (0x2C6E, IGNORE_CAPITAL_BOTTOM),
    (0x2C7C, UP),
    (0x2C7E, IGNORE_CAPITAL_BOTTOM),
    (0x2C7F, IGNORE_CAPITAL_BOTTOM),
    // Coptic
    (0x2CC2, UP),
    (0x2CC3, UP),
    // Supplemental Punctuation
    (0x2E18, UP),
    (0x2E2E, UP | NO_HEIGHT_CHECK),
    // Cyrillic Extended-B
    (0xA640, IGNORE_CAPITAL_BOTTOM),
    (0xA641, IGNORE_SMALL_BOTTOM),
    (0xA642, IGNORE_CAPITAL_BOTTOM),
    (0xA643, IGNORE_SMALL_BOTTOM),
    (0xA680, IGNORE_CAPITAL_TOP),
    (0xA681, IGNORE_SMALL_TOP),
    (0xA688, IGNORE_CAPITAL_BOTTOM),
    (0xA689, IGNORE_SMALL_BOTTOM),
    (0xA68A, IGNORE_CAPITAL_BOTTOM),
    (0xA68B, IGNORE_SMALL_BOTTOM),
    (0xA68E, IGNORE_CAPITAL_BOTTOM),
    (0xA68F, IGNORE_SMALL_BOTTOM),
    (0xA690, IGNORE_CAPITAL_BOTTOM),
    (0xA691, IGNORE_SMALL_BOTTOM),
    (0xA696, IGNORE_CAPITAL_BOTTOM),
    (0xA697, IGNORE_SMALL_BOTTOM),
    // Latin Extended-D
    (0xA726, IGNORE_CAPITAL_BOTTOM),
    (0xA727, IGNORE_SMALL_BOTTOM),
    (0xA756, IGNORE_CAPITAL_BOTTOM),
    (0xA758, IGNORE_CAPITAL_BOTTOM),
    (0xA771, IGNORE_SMALL_BOTTOM),
    (0xA772, IGNORE_SMALL_BOTTOM),
    (0xA773, IGNORE_SMALL_BOTTOM),
    (0xA774, IGNORE_SMALL_BOTTOM),
    (0xA776, IGNORE_SMALL_BOTTOM),
    (0xA790, IGNORE_CAPITAL_BOTTOM),
    (0xA791, IGNORE_SMALL_BOTTOM),
    (0xA794, IGNORE_SMALL_BOTTOM),
    (0xA795, IGNORE_SMALL_BOTTOM),
    (0xA7C0, IGNORE_CAPITAL_TOP | IGNORE_CAPITAL_BOTTOM),
    (0xA7C1, IGNORE_SMALL_TOP | IGNORE_SMALL_BOTTOM),
    (0xA7C4, IGNORE_CAPITAL_BOTTOM),
    (0xA7C5, IGNORE_CAPITAL_BOTTOM),
    (0xA7C6, IGNORE_CAPITAL_BOTTOM),
    (0xA7CC, IGNORE_CAPITAL_TOP | IGNORE_CAPITAL_BOTTOM),
    (0xA7CD, IGNORE_SMALL_TOP | IGNORE_SMALL_BOTTOM),
    // Latin Extended-E
    (0xAB3C, IGNORE_SMALL_BOTTOM),
    (0xAB46, IGNORE_SMALL_BOTTOM),
    (0xAB5C, IGNORE_SMALL_BOTTOM),
    (0xAB66, IGNORE_SMALL_BOTTOM),
    (0xAB67, IGNORE_SMALL_BOTTOM),
];

/// The flags of `codepoint` (0: none), found as FreeType finds them: by a binary search of the database as it is
/// ordered.
pub fn adjustment(codepoint: u32) -> u32 {
    let (mut low, mut high) = (0usize, DATABASE.len() - 1);
    while high >= low {
        let mid = (low + high) / 2;
        let (c, flags) = DATABASE[mid];
        if c < codepoint {
            low = mid + 1;
        } else if c > codepoint {
            if mid == 0 {
                return 0;
            }
            high = mid - 1;
        } else {
            return flags;
        }
    }
    0
}

/// FreeType's reverse character map from the cmap: each glyph the database's characters map to, by the character
/// (a later one in the database taking a glyph two of them map to).
pub fn reverse_charmap(font: &Font) -> HashMap<u32, u32> {
    let charmap = font.charmap();
    let mut map = HashMap::new();
    for &(c, _) in DATABASE {
        if let Some(glyph) = char::from_u32(c).and_then(|ch| charmap.map_unicode(ch))
            && glyph.to_u32() != 0
        {
            map.insert(glyph.to_u32(), c);
        }
    }
    map
}

/// Scripts whose blue zones include an uppercase bottom (the zone after the first top one) and a lowercase bottom
/// (the zone after the x-height one), as FreeType's afblue.dat marks them; others have neither.
const BOTTOM_BLUES: &[(&[u8; 4], bool, bool)] = &[
    (b"Adlm", true, true),
    (b"Armn", true, true),
    (b"Cans", false, true),
    (b"Copt", true, true),
    (b"Cyrl", true, false),
    (b"Dsrt", true, true),
    (b"Glag", true, true),
    (b"Grek", true, false),
    (b"Latn", true, true),
    (b"Latb", true, false),
    (b"Latp", true, false),
    (b"Medf", true, true),
    (b"Nkoo", false, true),
    (b"Osge", true, true),
    (b"Shaw", false, true),
];

/// How tall a diacritic may be and still be moved (FreeType's accent_height_limit, 26.6): two thirds of the
/// lowercase zones' height, else half the uppercase zones', else four tenths of the em; from the hinter's scaled
/// blue zones at `ppem` for the style of `glyph`. None when the hinter has no style for the glyph.
///
/// FreeType marks the bottom zones in its blue strings; skrifa's blues carry no such mark, and leave out a string
/// the font has no characters of. So a string counts as present when the font maps any of its characters, the
/// computed blues are matched to the present strings in order, and the bottom ones are the strings after the first
/// top one and after the x-height one, where the script has them.
pub fn accent_height_limit(
    font: &FontRef,
    coords: &[F2Dot14],
    styles: &GlyphStyles,
    glyph: u32,
    ppem: u32,
) -> Option<i32> {
    let style = styles.get(GlyphId::new(glyph))?.style_class()?;
    let unscaled = UnscaledStyleMetrics::new(font, coords, style);
    let units_per_em = font.head().map(|h| h.units_per_em()).unwrap_or(1000) as i32;
    let target = Target::Smooth { mode: SmoothMode::Light, symmetric_rendering: true, preserve_linear_metrics: false };
    let scale = Scale::new(ppem as f32, units_per_em, font.attributes().style, target, style.script.group);
    let scaled = unscaled.scale(scale);
    let blues = scaled.vertical_metrics().blues();
    let charmap = font.charmap();
    let present: Vec<usize> = style
        .script
        .blues
        .iter()
        .enumerate()
        .filter(|(_, (s, _))| s.chars().filter(|c| !c.is_whitespace() && *c != '|').any(|c| charmap.map(c).is_some()))
        .map(|(i, _)| i)
        .collect();
    let fallback = super::raster::mul_fix((units_per_em * 4 / 10) as i64, scaled.scale.y_scale as i64) as i32;
    if present.len() != blues.len() {
        return Some(fallback);
    }
    let blue_at =
        |string: usize| -> Option<&ScaledBlue> { present.iter().position(|&i| i == string).map(|k| &blues[k]) };
    let (capital, small) = BOTTOM_BLUES
        .iter()
        .find(|(tag, _, _)| style.script.tag.to_be_bytes() == **tag)
        .map_or((false, false), |&(_, c, s)| (c, s));
    let strings = style.script.blues;
    // The first string with these zones whose blue is active.
    let first = |zones: BlueZones| -> Option<(usize, &ScaledBlue)> {
        (0..strings.len())
            .filter(|&i| strings[i].1.contains(zones))
            .find_map(|i| blue_at(i).filter(|b| b.is_active).map(|b| (i, b)))
    };
    let shoot = |b: &ScaledBlue| b.overshoot.scaled;
    // The bottom zone: the string after the first one with `zones`, if the script marks it, active.
    let bottom = |zones: BlueZones, marked: bool| -> Option<&ScaledBlue> {
        if !marked {
            return None;
        }
        let i = (0..strings.len()).find(|&i| strings[i].1.contains(zones))? + 1;
        blue_at(i).filter(|b| b.is_active)
    };
    let small_top = first(BlueZones::ADJUSTMENT).map(|(_, b)| b);
    let small_bottom = bottom(BlueZones::ADJUSTMENT, small);
    if let (Some(top), Some(bottom)) = (small_top, small_bottom) {
        return Some(2 * (shoot(top) - shoot(bottom)) / 3);
    }
    let capital_top = first(BlueZones::TOP).map(|(_, b)| b);
    let capital_bottom = bottom(BlueZones::TOP, capital);
    if let (Some(top), Some(bottom)) = (capital_top, capital_bottom) {
        return Some((shoot(top) - shoot(bottom)) / 2);
    }
    Some(fallback)
}

/// Each contour's lowest and highest y (none: (MAX, MIN) for a contour of two points or fewer).
fn vertical_extrema(outline: &FixedOutline) -> Vec<(i64, i64)> {
    let mut first = 0usize;
    outline
        .contours
        .iter()
        .map(|&last| {
            let range = first..=last;
            first = last + 1;
            if range.end() - range.start() < 2 {
                return (i64::MAX, i64::MIN);
            }
            outline.points[range]
                .iter()
                .fold((i64::MAX, i64::MIN), |(lo, hi), p| (lo.min(p.y as i64), hi.max(p.y as i64)))
        })
        .collect()
}

/// The contour indices' point ranges.
fn contour_ranges(outline: &FixedOutline) -> Vec<std::ops::RangeInclusive<usize>> {
    let mut first = 0usize;
    outline
        .contours
        .iter()
        .map(|&last| {
            let r = first..=last;
            first = last + 1;
            r
        })
        .collect()
}

/// Whether contour `index` overlaps horizontally the box of the others (contours of two points or fewer left out).
fn overlaps_horizontally(outline: &FixedOutline, index: usize) -> bool {
    let (mut c_min, mut c_max, mut o_min, mut o_max) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for (i, range) in contour_ranges(outline).into_iter().enumerate() {
        if range.end() - range.start() < 2 {
            continue;
        }
        for p in &outline.points[range] {
            let x = p.x as i64;
            if i == index {
                c_min = c_min.min(x);
                c_max = c_max.max(x);
            } else {
                o_min = o_min.min(x);
                o_max = o_max.max(x);
            }
        }
    }
    (o_min <= c_max && c_max <= o_max) || (o_min <= c_min && c_min <= o_max) || (c_max >= o_max && c_min <= o_min)
}

fn highest(ext: &[(i64, i64)]) -> usize {
    let (mut best, mut best_min, mut best_max) = (0usize, i64::MAX, i64::MIN);
    for (i, &(lo, hi)) in ext.iter().enumerate() {
        if hi > best_max || (hi == best_max && lo > best_min) {
            (best, best_min, best_max) = (i, lo, hi);
        }
    }
    best
}

fn second_highest(ext: &[(i64, i64)]) -> usize {
    if ext.len() < 3 {
        return 0;
    }
    let top = highest(ext);
    let top_min = ext[top].0;
    let (mut best, mut best_max) = (0usize, i64::MIN);
    for (i, &(lo, hi)) in ext.iter().enumerate() {
        if i != top && hi > best_max && lo < top_min {
            (best, best_max) = (i, hi);
        }
    }
    best
}

fn lowest(ext: &[(i64, i64)]) -> usize {
    let (mut best, mut best_min, mut best_max) = (0usize, i64::MAX, i64::MIN);
    for (i, &(lo, hi)) in ext.iter().enumerate() {
        if lo < best_min || (lo == best_min && hi < best_max) {
            (best, best_min, best_max) = (i, lo, hi);
        }
    }
    best
}

fn second_lowest(ext: &[(i64, i64)]) -> usize {
    if ext.len() < 3 {
        return 0;
    }
    let bottom = lowest(ext);
    let bottom_max = ext[bottom].1;
    let (mut best, mut best_min) = (0usize, i64::MAX);
    for (i, &(lo, hi)) in ext.iter().enumerate() {
        if i != bottom && lo < best_min && hi > bottom_max {
            (best, best_min) = (i, lo);
        }
    }
    best
}

/// Contours (not flat) wholly above `limit` (by `ext`) moved `delta` up; below it, with `up` false, moved down.
fn move_contours(outline: &mut FixedOutline, ext: &[(i64, i64)], limit: i64, delta: i64, up: bool) {
    for (i, range) in contour_ranges(outline).into_iter().enumerate() {
        let (lo, hi) = ext[i];
        let beyond = if up { lo > limit } else { hi < limit };
        if lo < hi && beyond {
            let d = if up { delta } else { -delta } as i32;
            for p in &mut outline.points[range] {
                p.y = p.y.wrapping_add(d);
            }
        }
    }
}

fn pix_round(v: i64) -> i64 {
    (v + 32) & !63
}

/// FreeType's af_glyph_hints_apply_vertical_separation_adjustments on a hinted outline: with `flags` (the glyph's
/// character's), the accent above (or the one below the topmost, with UP2) moved up so that a pixel separates its
/// lowest point from the highest of any contour below it, with whatever lies as high; the same downwards; a tilde
/// re-centred on the pixel grid; nothing for a diacritic that doesn't overlap the rest across, or is taller than
/// `accent_height_limit` (26.6).
pub fn separate(outline: &mut FixedOutline, flags: u32, accent_height_limit: i64) {
    let (up, up2, down, down2) = (flags & UP != 0, flags & UP2 != 0, flags & DOWN != 0, flags & DOWN2 != 0);
    let n = outline.contours.len();
    if !(((up || down) && n >= 2) || ((up2 || down2) && n >= 3)) {
        return;
    }
    let ext = vertical_extrema(outline);
    if (up && n >= 2) || (up2 && n >= 3) {
        let high = if up2 { second_highest(&ext) } else { highest(&ext) };
        if !overlaps_horizontally(outline, high) {
            return;
        }
        let (high_min, high_max) = ext[high];
        let high_height = high_max.wrapping_sub(high_min);
        if high_height > accent_height_limit {
            return;
        }
        let mut min_distance = 64i64;
        for (i, &(lo, hi)) in ext.iter().enumerate() {
            if i == high {
                continue;
            }
            let distance = high_min.wrapping_sub(hi);
            if distance < 64 && distance < min_distance && lo < high_min {
                min_distance = distance;
            }
        }
        let amount = 64 - min_distance;
        let (is_tilde, is_tilde2) = (flags & TILDE_TOP != 0, flags & TILDE_TOP2 != 0);
        let mut centering = 0i64;
        if is_tilde || is_tilde2 {
            let tilde = if up || is_tilde2 { high } else { highest(&ext) };
            let tilde_height = ext[tilde].1.wrapping_sub(ext[tilde].0);
            let mut pos = high_min.wrapping_add(amount);
            if up2 && is_tilde {
                pos += high_height;
            }
            if pos % 64 == 0 && tilde_height < 3 * 64 {
                centering = (pix_round(tilde_height) - tilde_height) / 2;
            }
        }
        let calculated = if (up && is_tilde) || (up2 && is_tilde2) { amount + centering } else { amount };
        if calculated != 0 && calculated >= -2 && (calculated <= 66 || amount <= 66) {
            let min_y_limit = high_min.wrapping_sub(high_height / 8);
            move_contours(outline, &ext, min_y_limit, amount, true);
            if up2 && is_tilde {
                move_contours(outline, &ext, min_y_limit.wrapping_add(high_height), centering, true);
            }
        }
    }
    if (down && n >= 2) || (down2 && n >= 3) {
        let low = if down2 { second_lowest(&ext) } else { lowest(&ext) };
        if !overlaps_horizontally(outline, low) {
            return;
        }
        let (low_min, low_max) = ext[low];
        let low_height = low_max.wrapping_sub(low_min);
        if low_height > accent_height_limit {
            return;
        }
        let mut min_distance = 64i64;
        for (i, &(lo, hi)) in ext.iter().enumerate() {
            if i == low {
                continue;
            }
            let distance = lo.wrapping_sub(low_max);
            if distance < 64 && distance < min_distance && hi > low_max {
                min_distance = distance;
            }
        }
        let amount = 64 - min_distance;
        let (is_tilde, is_tilde2) = (flags & TILDE_BOTTOM != 0, flags & TILDE_BOTTOM2 != 0);
        let mut centering = 0i64;
        if is_tilde || is_tilde2 {
            let tilde = if down || is_tilde2 { low } else { lowest(&ext) };
            let tilde_height = ext[tilde].1.wrapping_sub(ext[tilde].0);
            let mut pos = low_max.wrapping_sub(amount);
            if down2 && is_tilde {
                pos -= low_height;
            }
            if pos % 64 == 0 && tilde_height < 3 * 64 {
                centering = (pix_round(tilde_height) - tilde_height) / 2;
            }
        }
        let calculated = if (down && is_tilde) || (down2 && is_tilde2) { amount + centering } else { amount };
        if calculated != 0 && calculated >= -2 && (calculated <= 66 || amount <= 66) {
            let max_y_limit = low_max.wrapping_add(low_height / 8);
            move_contours(outline, &ext, max_y_limit, amount, false);
            if down2 && is_tilde {
                move_contours(outline, &ext, max_y_limit.wrapping_sub(low_height), centering, false);
            }
        }
    }
}
