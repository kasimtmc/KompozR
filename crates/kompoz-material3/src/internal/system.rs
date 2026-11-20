//! An object's identity hash, as Compose's internal System.kt (its JVM actual: `System.identityHashCode`): what a
//! class whose instances are equal only to themselves hashes by (the minimum interactive size's element).

/// The identity hash of `instance` (Compose's internal `identityHashCode`): of its address, 0 for none (as Java's
/// for `null`); the same for the same instance, as long as it lives.
pub(crate) fn identity_hash_code<T: ?Sized>(instance: Option<&T>) -> i32 {
    match instance {
        None => 0,
        Some(instance) => {
            let address = instance as *const T as *const () as usize as u64;
            ((address ^ (address >> 32)) as u32 & 0x7fff_ffff) as i32
        }
    }
}
