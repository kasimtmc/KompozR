//! FreeType 2.14's diacritic adjustment database (`afadjust.c`): characters and how the hinter treats the
//! diacritics of their glyphs (moved up or down clear of the base, a tilde stretched, blue zones ignored where a
//! diacritic is attached to the base), and each style's reverse character map from glyphs to those characters.
//!
//! The database is FreeType's, in its order (which has `Q` before `?`, so that FreeType's binary search misses `Q`;
//! the search here is FreeType's too). The reverse map is the inverse of the character map for the database's
//! characters, extended as FreeType extends it with HarfBuzz: through the single and alternate substitutions of the
//! style's script's features in `GSUB`, followed in the order FreeType's hash table gives them, so that a glyph
//! reached from two characters takes the one FreeType's does.

use super::context::AutohintContext;
use super::gsub::{Bytes, collect_lookups, map_lookup, single_alternate_lookups};
use super::style::StyleClass;
use skrifa::raw::tables::layout::ScriptTags;

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

/// FreeType's hash table of numbers (`fthash.c`): open addressing in 241 buckets at first, probing downwards from a
/// key's Mocklisp hash, doubled when a third full, iterated in bucket order. Kept as FreeType keeps it because the
/// reverse character map is built in its iteration order.
pub(crate) struct FtHash {
    table: Vec<Option<(u32, u64)>>,
    limit: usize,
    used: usize,
}

impl FtHash {
    const INITIAL_SIZE: usize = 241;

    pub fn new() -> FtHash {
        FtHash { table: vec![None; Self::INITIAL_SIZE], limit: Self::INITIAL_SIZE / 3, used: 0 }
    }

    /// `hash_num_lookup`: the Mocklisp hash of a key's four bytes.
    fn hash(key: u32) -> u64 {
        let num = key as u64;
        let mut res = num & 0xFF;
        res = (res << 5) - res + ((num >> 8) & 0xFF);
        res = (res << 5) - res + ((num >> 16) & 0xFF);
        res = (res << 5) - res + ((num >> 24) & 0xFF);
        res
    }

    /// `hash_bucket`: where `key` is, or the empty bucket where it would go.
    fn bucket(table: &[Option<(u32, u64)>], key: u32) -> usize {
        let size = table.len();
        let mut i = (Self::hash(key) % size as u64) as usize;
        while let Some((k, _)) = table[i] {
            if k == key {
                break;
            }
            i = if i == 0 { size - 1 } else { i - 1 };
        }
        i
    }

    /// `hash_insert`: `data` for `key`, replacing what it had if `overwrite`. Whether the key is new.
    fn insert_with(&mut self, key: u32, data: u64, overwrite: bool) -> bool {
        let i = Self::bucket(&self.table, key);
        match &mut self.table[i] {
            Some((_, d)) => {
                if overwrite {
                    *d = data;
                }
                false
            }
            empty => {
                *empty = Some((key, data));
                if self.used >= self.limit {
                    // `hash_rehash`: twice the buckets, the nodes moved in bucket order.
                    let size = self.table.len() * 2;
                    let old = std::mem::replace(&mut self.table, vec![None; size]);
                    self.limit = self.table.len() / 3;
                    for node in old.into_iter().flatten() {
                        let j = Self::bucket(&self.table, node.0);
                        self.table[j] = Some(node);
                    }
                }
                self.used += 1;
                true
            }
        }
    }

    /// `ft_hash_num_insert`.
    pub fn insert(&mut self, key: u32, data: u64) {
        self.insert_with(key, data, true);
    }

    /// `ft_hash_num_insert_no_overwrite`: whether the key is new.
    pub fn insert_no_overwrite(&mut self, key: u32, data: u64) -> bool {
        self.insert_with(key, data, false)
    }

    /// The data of a key already in the table replaced (`*ft_hash_num_lookup(key) = data`).
    pub fn set(&mut self, key: u32, data: u64) {
        let i = Self::bucket(&self.table, key);
        if let Some((_, d)) = &mut self.table[i] {
            *d = data;
        }
    }

    /// `ft_hash_num_lookup`.
    pub fn lookup(&self, key: u32) -> Option<u64> {
        self.table[Self::bucket(&self.table, key)].map(|(_, d)| d)
    }

    /// The entries as `ft_hash_num_iterator` gives them: in bucket order.
    pub fn entries(&self) -> Vec<(u32, u64)> {
        self.table.iter().flatten().copied().collect()
    }
}

/// A style's reverse character map (`af_reverse_character_map_new`): from glyphs to the characters of the
/// adjustment database they render.
pub(crate) struct ReverseCharmap(Option<FtHash>);

impl ReverseCharmap {
    /// The map for `style` of the font behind `context`: none for a font without a Unicode character map (FreeType
    /// builds none then) or a style hinted by another writing system than the Latin one (whose metrics alone make
    /// one).
    pub fn new(context: &dyn AutohintContext, style: &StyleClass) -> ReverseCharmap {
        if context.is_symbol() || style.script.group != super::style::ScriptGroup::Default {
            return ReverseCharmap(None);
        }
        let mut map = FtHash::new();
        // The cmap's glyphs of the database's characters, all of them (GSUB data needn't keep to a script's
        // characters), a later character taking a glyph two map to.
        for &(codepoint, _) in DATABASE {
            if let Some(glyph) = context.map(codepoint)
                && glyph.to_u32() != 0
            {
                map.insert(glyph.to_u32(), codepoint as u64);
            }
        }
        // With HarfBuzz: the script's single and alternate substitutions, in lookup order, into one table.
        let gsub = context.gsub();
        let bytes = Bytes(gsub.as_ref().map_or(&[][..], |g| g.offset_data().as_bytes()));
        let single_alternate = single_alternate_lookups(bytes);
        // (A script without a HarfBuzz script, a fallback one, has no OpenType tags: no lookups.)
        let tag = style
            .script
            .layout
            .then(|| ScriptTags::from_unicode(style.script.tag).first().map(|t| u32::from_be_bytes(t.to_be_bytes())))
            .flatten();
        let mut substitutions = FtHash::new();
        for lookup in collect_lookups(bytes, tag.as_slice()) {
            if let Some(&offset) = single_alternate.get(lookup as usize)
                && offset != 0
            {
                map_lookup(bytes, &mut substitutions, offset);
            }
        }
        // Each glyph mapped from a character and substituted gives the character to its substitutes, and theirs in
        // turn, never overwriting (so that the cmap's glyphs keep theirs).
        for (glyph, value) in substitutions.entries() {
            // (Keys of the second and later substitutes aren't glyphs.)
            if glyph & 0xFFFF_0000 != 0 {
                continue;
            }
            let Some(codepoint) = map.lookup(glyph) else {
                continue;
            };
            add_substitute(glyph, value, codepoint, &mut map, &substitutions);
        }
        ReverseCharmap(Some(map))
    }

    /// The database's character `glyph` renders, if any.
    pub fn codepoint(&self, glyph: u32) -> Option<u32> {
        self.0.as_ref()?.lookup(glyph).map(|c| c as u32)
    }

    /// The adjustment flags of `glyph`'s character (0: none).
    pub fn flags(&self, glyph: u32) -> u32 {
        self.codepoint(glyph).map_or(0, adjustment)
    }
}

/// `add_substitute`: `codepoint` given to glyph `glyph`'s substitutes (`value`: the first, with the count of the
/// others above it), each new one's own substitutes in turn, depth first, as FreeType's recursion goes (here with a
/// stack of its frames: a glyph, its value, the next of its substitutes).
fn add_substitute(glyph: u32, value: u64, codepoint: u64, map: &mut FtHash, substitutions: &FtHash) {
    let mut stack = vec![(glyph, value, 0u64)];
    while let Some(frame) = stack.last_mut() {
        let (glyph, value, i) = *frame;
        if i > value >> 16 {
            stack.pop();
            continue;
        }
        frame.2 += 1;
        let substitute = if i == 0 {
            value & 0xFFFF
        } else {
            let key = glyph.wrapping_add((i as u32).wrapping_shl(16));
            match substitutions.lookup(key) {
                Some(s) => s,
                None => continue,
            }
        } as u32;
        if map.insert_no_overwrite(substitute, codepoint)
            && let Some(next) = substitutions.lookup(substitute)
        {
            stack.push((substitute, next, 0));
        }
    }
}
