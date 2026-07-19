//! Deterministic, seedable PRNG used for procedural generation.
//!
//! Implements Xoshiro128++ with a 128-bit state, seeded from a `u64`.
//! Pure Rust, no `unsafe`, no dependencies. Deterministic across
//! platforms given the same seed, which lets the maze be reproduced
//! from a URL parameter.

/// 128-bit Xoshiro128++ state with a 32-bit output per step.
pub struct Rng {
    state: [u32; 4],
}

impl Rng {
    /// Seed from a single `u64`. Splits it into four 16-bit limbs so
    /// a zero seed still produces a nonzero, well-mixed state.
    pub fn from_seed(seed: u64) -> Self {
        let limbs = [
            (seed & 0xFFFF) as u32,
            ((seed >> 16) & 0xFFFF) as u32,
            ((seed >> 32) & 0xFFFF) as u32,
            ((seed >> 48) & 0xFFFF) as u32,
        ];

        // Xoshiro128 is undefined for an all-zero state.
        let state = if limbs == [0, 0, 0, 0] {
            [1, 0, 0, 1]
        } else {
            [limbs[0], limbs[1], limbs[2], limbs[3]]
        };

        Self { state }
    }

    fn rotl(x: u32, k: u32) -> u32 {
        x.rotate_left(k)
    }

    /// Produce the next 32-bit pseudo-random value.
    pub fn next_u32(&mut self) -> u32 {
        let result = self.state[0]
            .wrapping_add(self.state[3])
            .wrapping_add(Self::rotl(self.state[1], 7));

        let t = self.state[1] << 9;

        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];

        self.state[2] ^= t;
        self.state[3] = Self::rotl(self.state[3], 11);

        result
    }

    /// Returns a pseudo-random integer in `[0, bound)`.
    pub fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        self.next_u32() % bound
    }

    /// Returns a pseudo-random integer in the inclusive `[low, high]`.
    pub fn between(&mut self, low: u32, high: u32) -> u32 {
        if high < low {
            return low;
        }
        low + self.below(high - low + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn same_seed_produces_same_sequence() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);

        for _ in 0..32 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn zero_seed_is_non_degenerate() {
        let mut rng = Rng::from_seed(0);
        // The first several values should be nonzero and varied.
        let first = rng.next_u32();
        assert_ne!(first, 0);

        let second = rng.next_u32();
        assert_ne!(second, first);
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = Rng::from_seed(7);
        for _ in 0..256 {
            let value = rng.below(10);
            assert!(value < 10);
        }
    }
}
