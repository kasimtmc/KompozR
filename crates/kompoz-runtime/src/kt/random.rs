//! Kotlin's `kotlin.random` as the Kotlin syntax needs it: a [`Random`] of a seed (Kotlin's `XorWowRandom`, the
//! same numbers as Kotlin's for the same seed), its `nextInt`s, and `MutableList.shuffle(random)` (Kotlin's
//! algorithm: the same order as Kotlin's for the same seed), on a `Vec` and on a state list.

use crate::SnapshotStateList;
use std::cell::Cell;

/// Kotlin's `Random` of a seed (its `XorWowRandom`: George Marsaglia's xorwow, the state Kotlin makes of the seed,
/// 64 numbers dropped as Kotlin's constructor drops them). Its numbers come through `&self`, as Kotlin's object
/// changes as it is used.
#[derive(Debug)]
pub struct Random {
    x: Cell<i32>,
    y: Cell<i32>,
    z: Cell<i32>,
    w: Cell<i32>,
    v: Cell<i32>,
    addend: Cell<i32>,
}

/// A random of `seed` (Kotlin's `Random(seed: Int)`: `XorWowRandom(seed, seed shr 31)`).
/// Kotlin: `Random(seed: Int): Random`
#[allow(non_snake_case)]
pub fn Random(seed: i32) -> Random {
    Random::xor_wow(seed, seed >> 31)
}

impl Random {
    /// Kotlin's `Random(seed: Long)`: `XorWowRandom(seed.toInt(), seed.shr(32).toInt())`.
    pub fn of_long(seed: i64) -> Random {
        Random::xor_wow(seed as i32, (seed >> 32) as i32)
    }

    /// Kotlin's `XorWowRandom(seed1, seed2)`.
    fn xor_wow(seed1: i32, seed2: i32) -> Random {
        let r = Random {
            x: Cell::new(seed1),
            y: Cell::new(seed2),
            z: Cell::new(0),
            w: Cell::new(0),
            v: Cell::new(!seed1),
            addend: Cell::new((seed1 << 10) ^ ((seed2 as u32) >> 4) as i32),
        };
        assert!(
            (r.x.get() | r.y.get() | r.z.get() | r.w.get() | r.v.get()) != 0,
            "Initial state must have at least one non-zero element."
        );
        // (some numbers dropped: what a seed starts with looks less like it)
        for _ in 0..64 {
            r.next_int();
        }
        r
    }

    /// The next number, any `Int`.
    /// Kotlin: `nextInt(): Int`
    pub fn next_int(&self) -> i32 {
        // Equivalent to the xorxow algorithm from George Marsaglia's paper "Xorshift RNGs".
        let mut t = self.x.get();
        t ^= ((t as u32) >> 2) as i32;
        self.x.set(self.y.get());
        self.y.set(self.z.get());
        self.z.set(self.w.get());
        let v0 = self.v.get();
        self.w.set(v0);
        t = (t ^ (t << 1)) ^ v0 ^ (v0 << 4);
        self.v.set(t);
        self.addend.set(self.addend.get().wrapping_add(6362437));
        t.wrapping_add(self.addend.get())
    }

    /// The next `bit_count` random bits (the upper ones of the next number).
    /// Kotlin: `nextBits(bitCount: Int): Int`
    pub fn next_bits(&self, bit_count: i32) -> i32 {
        take_upper_bits(self.next_int(), bit_count)
    }

    /// A number from 0 until `until` (Kotlin's `nextInt(until)`).
    /// Kotlin: `nextInt(until: Int): Int`
    #[track_caller]
    pub fn next_int_until(&self, until: i32) -> i32 {
        self.next_int_in(0, until)
    }

    /// A number from `from` until `until` (Kotlin's `nextInt(from, until)`).
    /// Kotlin: `nextInt(from: Int, until: Int): Int`
    #[track_caller]
    pub fn next_int_in(&self, from: i32, until: i32) -> i32 {
        if until <= from {
            super::panic_kt("IllegalArgumentException", &format!("Random range is empty: [{from}, {until})."));
        }
        let n = until.wrapping_sub(from);
        if n > 0 || n == i32::MIN {
            let rnd = if n.isolate_lowest_one() == n {
                self.next_bits(fast_log2(n))
            } else {
                loop {
                    let bits = ((self.next_int() as u32) >> 1) as i32;
                    let v = bits % n;
                    if bits.wrapping_sub(v).wrapping_add(n - 1) >= 0 {
                        break v;
                    }
                }
            };
            from.wrapping_add(rnd)
        } else {
            loop {
                let rnd = self.next_int();
                if (from..until).contains(&rnd) {
                    return rnd;
                }
            }
        }
    }
}

/// Kotlin's `Int.takeUpperBits(bitCount)`.
fn take_upper_bits(value: i32, bit_count: i32) -> i32 {
    if bit_count == 0 {
        return 0;
    }
    (((value as u32) >> (32 - bit_count)) as i32) & (bit_count.wrapping_neg() >> 31)
}

/// Kotlin's `fastLog2(value)`: `31 - value.countLeadingZeroBits()`.
fn fast_log2(value: i32) -> i32 {
    31 - value.leading_zeros() as i32
}

/// Kotlin's `MutableList.shuffle(random)` on a `Vec`: from the last index down, each element swapped with one at an
/// index `random` picks up to it.
pub trait KtShuffle {
    /// Kotlin: `shuffle(random: Random)`
    fn shuffle(&mut self, random: &Random);
}

impl<T> KtShuffle for Vec<T> {
    fn shuffle(&mut self, random: &Random) {
        for i in (1..self.len()).rev() {
            let j = random.next_int_until(i as i32 + 1) as usize;
            self.swap(i, j);
        }
    }
}

impl<T: Clone + Send + Sync + 'static> SnapshotStateList<T> {
    /// Kotlin's `MutableList.shuffle(random)` on a state list, as on a `Vec` (one change of the list).
    /// Kotlin: `shuffle(random: Random)`
    pub fn shuffle(&self, random: &Random) {
        self.update(|v| KtShuffle::shuffle(v, random));
    }
}
