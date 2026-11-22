#![forbid(unsafe_code)]
//! Deterministic `SplitMix64` PRNG for `rotameter seed`.

/// Deterministic `SplitMix64` generator for repeatable synthetic datasets.
pub(crate) struct SeedRng {
    state: u64,
}

impl SeedRng {
    pub(crate) const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub(crate) fn range_usize(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        usize::try_from(self.next_u64() % u64::try_from(upper).unwrap_or(1)).unwrap_or(0)
    }

    pub(crate) fn range_i64(&mut self, min: i64, max: i64) -> i64 {
        if max <= min {
            return min;
        }
        let span = u64::try_from(max.saturating_sub(min).saturating_add(1)).unwrap_or(1);
        let offset = i64::try_from(self.next_u64() % span).unwrap_or(0);
        min.saturating_add(offset)
    }

    pub(crate) fn next_bytes16(&mut self) -> [u8; 16] {
        let hi = self.next_u64().to_be_bytes();
        let lo = self.next_u64().to_be_bytes();
        let mut out = [0_u8; 16];
        out[..8].copy_from_slice(&hi);
        out[8..].copy_from_slice(&lo);
        out
    }

    pub(crate) fn pick<'a, T>(&mut self, items: &'a [(u64, T)]) -> &'a T {
        let total: u64 = items.iter().map(|(w, _)| *w).sum();
        let mut roll = if total == 0 {
            0
        } else {
            self.next_u64() % total
        };
        for (weight, item) in items {
            if roll < *weight {
                return item;
            }
            roll -= *weight;
        }
        &items[0].1
    }
}
