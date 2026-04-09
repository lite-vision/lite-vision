use crate::types::{BlockHeight, Hash32};
use serde::{Deserialize, Serialize};

pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub fn new(seed: Hash32) -> Self {
        let mut state = 0u64;
        for (i, byte) in seed.iter().enumerate() {
            if i < 8 {
                state = state.wrapping_add((*byte as u64).wrapping_mul(1u64 << (i * 8)));
            }
        }
        if state == 0 {
            state = 0x853aa_1f0a8b_4c2d;
        }
        Self { state }
    }

    pub fn from_block_seed(domain: &[u8], prev_hash: Hash32, height: BlockHeight) -> Self {
        let seed = crate::types::DomainSeparatedHash::derive_seed(domain, prev_hash, height);
        Self::new(seed)
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(0x5851_f42d_4c95_7f63)
            .wrapping_add(0x1405_7b92_1f4e_31be);
        (self.state >> 32) as u32
    }

    pub fn rand_u32(&mut self) -> u32 {
        self.next_u32()
    }

    pub fn rand_u64(&mut self) -> u64 {
        let lo = self.next_u32() as u64;
        let hi = self.next_u32() as u64;
        lo | (hi << 32)
    }

    pub fn rand_u128(&mut self) -> u128 {
        let lo = self.rand_u64() as u128;
        let hi = self.rand_u64() as u128;
        lo | (hi << 64)
    }

    pub fn rand_range(&mut self, min: u32, max: u32) -> u32 {
        if max <= min {
            return min;
        }
        let range = max - min + 1;
        let mut result = self.rand_u32();
        result = result % range;
        min + result
    }

    pub fn fill_bytes(&mut self, buffer: &mut [u8]) {
        for chunk in buffer.chunks_mut(4) {
            let val = self.rand_u32();
            for (i, byte) in chunk.iter_mut().enumerate() {
                *byte = (val >> (i * 8)) as u8;
            }
        }
    }

    pub fn choose<'a, T: Sized>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            return None;
        }
        let idx = self.rand_range(0, slice.len() as u32 - 1) as usize;
        Some(&slice[idx])
    }

    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.rand_range(0, i as u32) as usize;
            slice.swap(i, j);
        }
    }
}

impl Default for DeterministicRng {
    fn default() -> Self {
        Self::new([0u8; 32])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalTime {
    height: BlockHeight,
    view: u32,
}

impl LogicalTime {
    pub fn new(height: BlockHeight, view: u32) -> Self {
        Self { height, view }
    }

    pub fn height(&self) -> BlockHeight {
        self.height
    }

    pub fn view(&self) -> u32 {
        self.view
    }

    pub fn next_view(&self) -> Self {
        Self {
            height: self.height,
            view: self.view + 1,
        }
    }

    pub fn next_height(&self) -> Self {
        Self {
            height: self.height + 1,
            view: 0,
        }
    }

    pub fn is_after(&self, other: &LogicalTime) -> bool {
        if self.height != other.height {
            self.height > other.height
        } else {
            self.view > other.view
        }
    }
}

impl Default for LogicalTime {
    fn default() -> Self {
        Self { height: 0, view: 0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampReader;

impl TimestampReader {
    pub fn logical_now(_height: BlockHeight, _view: u32) -> LogicalTime {
        LogicalTime::default()
    }

    pub fn genesis_timestamp() -> TimestampReader {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_rng_seed() {
        let seed = [1u8; 32];
        let mut rng1 = DeterministicRng::new(seed);
        let mut rng2 = DeterministicRng::new(seed);

        for _ in 0..1000 {
            assert_eq!(rng1.rand_u32(), rng2.rand_u32());
        }
    }

    #[test]
    fn test_deterministic_rng_different_seeds() {
        let mut rng1 = DeterministicRng::new([1u8; 32]);
        let mut rng2 = DeterministicRng::new([2u8; 32]);

        assert_ne!(rng1.rand_u32(), rng2.rand_u32());
    }

    #[test]
    fn test_deterministic_rng_range() {
        let mut rng = DeterministicRng::new([0u8; 32]);
        for _ in 0..10000 {
            let val = rng.rand_range(5, 10);
            assert!(val >= 5 && val <= 10);
        }
    }

    #[test]
    fn test_deterministic_rng_choose() {
        let mut rng = DeterministicRng::new([0u8; 32]);
        let items = vec![1, 2, 3, 4, 5];

        let chosen = rng.choose(&items);
        assert!(chosen.is_some());
        assert!(items.contains(chosen.unwrap()));
    }

    #[test]
    fn test_deterministic_rng_shuffle() {
        let mut rng1 = DeterministicRng::new([0u8; 32]);
        let mut rng2 = DeterministicRng::new([0u8; 32]);

        let mut arr1 = vec![1, 2, 3, 4, 5];
        let mut arr2 = vec![1, 2, 3, 4, 5];

        rng1.shuffle(&mut arr1);
        rng2.shuffle(&mut arr2);

        assert_eq!(arr1, arr2);
    }

    #[test]
    fn test_logical_time_ordering() {
        let t1 = LogicalTime::new(1, 5);
        let t2 = LogicalTime::new(2, 0);
        let t3 = LogicalTime::new(1, 10);

        assert!(t2.is_after(&t1));
        assert!(t3.is_after(&t1));
        assert!(!t1.is_after(&t2));
    }

    #[test]
    fn test_logical_time_transitions() {
        let t = LogicalTime::new(1, 5);

        let next_view = t.next_view();
        assert_eq!(next_view.height(), 1);
        assert_eq!(next_view.view(), 6);

        let next_height = t.next_height();
        assert_eq!(next_height.height(), 2);
        assert_eq!(next_height.view(), 0);
    }
}
