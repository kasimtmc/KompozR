//! Two numbers in one `i64` (Compose's `androidx.compose.ui.util` packing helpers, InlineClassHelper.kt): what
//! Compose's value classes pack their pairs into (an offset's x and y, a size's width and height), and what a
//! library keeps two numbers of one state in.

/// Two floats in one long: `val1`'s bits in the upper 32, `val2`'s in the lower (Compose's `packFloats`).
pub const fn pack_floats(val1: f32, val2: f32) -> i64 {
    let v1 = val1.to_bits() as i64;
    let v2 = val2.to_bits() as i64;
    (v1 << 32) | (v2 & 0xFFFF_FFFF)
}

/// The first float of [`pack_floats`]' long (Compose's `unpackFloat1`).
pub const fn unpack_float1(value: i64) -> f32 {
    f32::from_bits((value >> 32) as u32)
}

/// The first float's absolute value (its sign bit cleared; Compose's `unpackAbsFloat1`).
pub const fn unpack_abs_float1(value: i64) -> f32 {
    f32::from_bits(((value >> 32) & 0x7FFF_FFFF) as u32)
}

/// The second float of [`pack_floats`]' long (Compose's `unpackFloat2`).
pub const fn unpack_float2(value: i64) -> f32 {
    f32::from_bits((value & 0xFFFF_FFFF) as u32)
}

/// The second float's absolute value (Compose's `unpackAbsFloat2`).
pub const fn unpack_abs_float2(value: i64) -> f32 {
    f32::from_bits((value & 0x7FFF_FFFF) as u32)
}

/// Two ints in one long (Compose's `packInts`).
pub const fn pack_ints(val1: i32, val2: i32) -> i64 {
    ((val1 as i64) << 32) | (val2 as i64 & 0xFFFF_FFFF)
}

/// The first int of [`pack_ints`]' long (Compose's `unpackInt1`).
pub const fn unpack_int1(value: i64) -> i32 {
    (value >> 32) as i32
}

/// The second int of [`pack_ints`]' long (Compose's `unpackInt2`).
pub const fn unpack_int2(value: i64) -> i32 {
    (value & 0xFFFF_FFFF) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_and_ints_round_trip() {
        let v = pack_floats(-1.5, 2.25);
        assert_eq!((unpack_float1(v), unpack_float2(v)), (-1.5, 2.25));
        assert_eq!((unpack_abs_float1(v), unpack_abs_float2(pack_floats(0.0, -3.0))), (1.5, 3.0));
        let i = pack_ints(-7, -9);
        assert_eq!((unpack_int1(i), unpack_int2(i)), (-7, -9));
        assert_eq!(pack_ints(1, 2), (1 << 32) | 2);
    }
}
