//! The identity hash code, as Compose's internal System.kt and its JVM actual (`System.identityHashCode`).

/// A hash of `instance`'s identity (Compose's internal `identityHashCode`, the JVM's `System.identityHashCode`): of
/// its address, the same for the same instance; 0 for none (null). Compose's only user is
/// `PaneExpansionStateKey.Default`'s `hashCode`; KompozR's key is an enum whose `Hash` is derived, so this stays for
/// what else hashes an instance by identity.
#[allow(dead_code)]
pub(crate) fn identity_hash_code<T: ?Sized>(instance: Option<&T>) -> i32 {
    match instance {
        None => 0,
        Some(instance) => {
            let address = instance as *const T as *const () as usize as u64;
            // (the address's bits folded into an Int, as a JVM's identity hash is an Int)
            ((address >> 32) ^ address) as i32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_instance_hashes_the_same_and_null_is_zero() {
        let a = Box::new(1);
        let b = Box::new(1);
        assert_eq!(identity_hash_code(Some(&*a)), identity_hash_code(Some(&*a)));
        assert_ne!(identity_hash_code(Some(&*a)), identity_hash_code(Some(&*b)));
        assert_eq!(identity_hash_code::<i32>(None), 0);
    }
}
