use serde::{Deserialize, Serialize};

use crate::DOMAIN_SEPARATOR;

pub type Hash32 = [u8; 32];
pub type BlockHeight = u64;
pub type ValidatorId = Hash32;
pub type OperatorId = Hash32;
pub type JobId = Hash32;
pub type PartitionId = u32;
pub type Timestamp = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeightRef(pub BlockHeight);

impl BlockHeightRef {
    pub fn new(height: BlockHeight) -> Self {
        Self(height)
    }

    pub fn value(&self) -> BlockHeight {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl Default for BlockHeightRef {
    fn default() -> Self {
        Self(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round(pub u32);

impl Round {
    pub fn new(round: u32) -> Self {
        Self(round)
    }

    pub fn value(&self) -> u32 {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl Default for Round {
    fn default() -> Self {
        Self(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSeparatedHash {
    pub domain: Vec<u8>,
    pub data: Vec<u8>,
    pub output: Hash32,
}

impl DomainSeparatedHash {
    pub fn new(domain: &[u8], data: &[u8]) -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(DOMAIN_SEPARATOR);
        hasher.update(domain);
        hasher.update(data);
        Self {
            domain: domain.to_vec(),
            data: data.to_vec(),
            output: *hasher.finalize().as_bytes(),
        }
    }

    pub fn derive_seed(domain: &[u8], prev_hash: Hash32, height: BlockHeight) -> Hash32 {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(DOMAIN_SEPARATOR);
        hasher.update(domain);
        hasher.update(&prev_hash);
        hasher.update(&height.to_le_bytes());
        *hasher.finalize().as_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genesis {
    pub height: BlockHeight,
    pub timestamp: Timestamp,
    pub chain_id: Hash32,
}

impl Default for Genesis {
    fn default() -> Self {
        Self {
            height: 0,
            timestamp: 0,
            chain_id: [0u8; 32],
        }
    }
}

impl Genesis {
    pub fn new(chain_id: Hash32) -> Self {
        Self {
            height: 0,
            timestamp: 0,
            chain_id,
        }
    }

    pub fn with_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = timestamp;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_seed_deterministic() {
        let hash1 = DomainSeparatedHash::derive_seed(b"leader", [1u8; 32], 1);
        let hash2 = DomainSeparatedHash::derive_seed(b"leader", [1u8; 32], 1);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_derive_seed_different_inputs() {
        let hash1 = DomainSeparatedHash::derive_seed(b"leader", [1u8; 32], 1);
        let hash2 = DomainSeparatedHash::derive_seed(b"leader", [2u8; 32], 1);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_domain_separated_hash() {
        let hash = DomainSeparatedHash::new(b"test", b"data");
        assert_eq!(hash.domain, b"test");
        assert_eq!(hash.data, b"data");
        assert_eq!(hash.output.len(), 32);
    }

    #[test]
    fn test_domain_separation_different_domains() {
        let hash1 = DomainSeparatedHash::new(b"domain1", b"data");
        let hash2 = DomainSeparatedHash::new(b"domain2", b"data");
        assert_ne!(hash1.output, hash2.output);
    }

    #[test]
    fn test_domain_separation_different_data() {
        let hash1 = DomainSeparatedHash::new(b"domain", b"data1");
        let hash2 = DomainSeparatedHash::new(b"domain", b"data2");
        assert_ne!(hash1.output, hash2.output);
    }

    #[test]
    fn test_domain_separation_deterministic() {
        for _ in 0..100 {
            let hash1 = DomainSeparatedHash::new(b"test", b"data");
            let hash2 = DomainSeparatedHash::new(b"test", b"data");
            assert_eq!(hash1.output, hash2.output);
        }
    }
}
