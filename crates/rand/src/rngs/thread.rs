// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Thread-local random number generator

use std::cell::UnsafeCell;
use std::marker::PhantomData;

use super::std::Core;
use crate::rngs::adapter::ReseedingRng;
use crate::rngs::OsRng;
use crate::{CryptoRng, Error, RngCore, SeedableRng};

// Rationale for using `UnsafeCell` in `ThreadRng`:
//
// Previously we used a `RefCell`, with an overhead of ~15%. There will only
// ever be one mutable reference to the interior of the `UnsafeCell`, because
// we only have such a reference inside `next_u32`, `next_u64`, etc. Within a
// single thread (which is the definition of `ThreadRng`), there will only ever
// be one of these methods active at a time.
//
// A possible scenario where there could be multiple mutable references is if
// `ThreadRng` is used inside `next_u32` and co. But the implementation is
// completely under our control. We just have to ensure none of them use
// `ThreadRng` internally, which is nonsensical anyway. We should also never run
// `ThreadRng` in destructors of its implementation, which is also nonsensical.


// Number of generated bytes after which to reseed `ThreadRng`.
// According to benchmarks, reseeding has a noticable impact with thresholds
// of 32 kB and less. We choose 64 kB to avoid significant overhead.
const THREAD_RNG_RESEED_THRESHOLD: u64 = 1024 * 64;

/// The type returned by [`thread_rng`], essentially just a reference to the
/// PRNG in thread-local memory.
///
/// `ThreadRng` uses the same PRNG as [`StdRng`] for security and performance.
/// As hinted by the name, the generator is thread-local. `ThreadRng` is a
/// handle to this generator and thus supports `Copy`, but not `Send` or `Sync`.
///
/// Unlike `StdRng`, `ThreadRng` uses the  [`ReseedingRng`] wrapper to reseed
/// the PRNG from fresh entropy every 64 kiB of random data.
/// [`OsRng`] is used to provide seed data.
///
/// Note that the reseeding is done as an extra precaution against side-channel
/// attacks and mis-use (e.g. if somehow weak entropy were supplied initially).
/// The PRNG algorithms used are assumed to be secure.
///
/// [`ReseedingRng`]: crate::rngs::adapter::ReseedingRng
/// [`StdRng`]: crate::rngs::StdRng
#[derive(Copy, Clone, Debug)]
pub struct ThreadRng {
    // type is neither Send nor Sync
    _phantom: PhantomData<*const ()>,
}

thread_local!(
    static THREAD_RNG_KEY: UnsafeCell<ReseedingRng<Core, OsRng>> = {
        let r = Core::from_rng(OsRng).unwrap_or_else(|err|
                panic!("could not initialize thread_rng: {}", err));
        let rng = ReseedingRng::new(r,
                                    THREAD_RNG_RESEED_THRESHOLD,
                                    OsRng);
        UnsafeCell::new(rng)
    }
);

/// Retrieve the lazily-initialized thread-local random number generator,
/// seeded by the system. Intended to be used in method chaining style,
/// e.g. `thread_rng().gen::<i32>()`, or cached locally, e.g.
/// `let mut rng = thread_rng();`.  Invoked by the `Default` trait, making
/// `ThreadRng::default()` equivalent.
///
/// For more information see [`ThreadRng`].
pub fn thread_rng() -> ThreadRng {
    let _phantom = Default::default();
    ThreadRng { _phantom }
}

impl Default for ThreadRng {
    fn default() -> ThreadRng {
        thread_rng()
    }
}

impl ThreadRng {
    #[inline(always)]
    fn rng(&mut self) -> &mut ReseedingRng<Core, OsRng> {
        let ptr = THREAD_RNG_KEY.with(|rng| rng.get());
        unsafe { &mut *ptr }
    }
}

impl RngCore for ThreadRng {
    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        self.rng().next_u32()
    }

    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        self.rng().next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.rng().fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.rng().try_fill_bytes(dest)
    }
}

impl CryptoRng for ThreadRng {}


#[cfg(test)]
mod test {
    #[test]
    fn test_thread_rng() {
        use crate::Rng;
        let mut r = crate::thread_rng();
        r.gen::<i32>();
        assert_eq!(r.gen_range(0, 1), 0);
    }
}