use crate::types::DomainSeparatedHash;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceResult {
    pub passed: bool,
    pub test_name: String,
    pub message: String,
}

impl ConformanceResult {
    pub fn pass(test_name: &str) -> Self {
        Self {
            passed: true,
            test_name: test_name.to_string(),
            message: "PASSED".to_string(),
        }
    }

    pub fn fail(test_name: &str, message: &str) -> Self {
        Self {
            passed: false,
            test_name: test_name.to_string(),
            message: message.to_string(),
        }
    }
}

pub struct ConformanceTest;

impl ConformanceTest {
    pub fn test_domain_separation() -> ConformanceResult {
        let hash1 = DomainSeparatedHash::derive_seed(b"leader", [1u8; 32], 1);
        let hash2 = DomainSeparatedHash::derive_seed(b"leader", [1u8; 32], 1);

        if hash1 != hash2 {
            return ConformanceResult::fail(
                "domain_separation",
                "Domain separated hash not deterministic",
            );
        }

        let hash3 = DomainSeparatedHash::derive_seed(b"leader", [1u8; 32], 2);

        if hash1 == hash3 {
            return ConformanceResult::fail(
                "domain_separation",
                "Different inputs produce same hash",
            );
        }

        ConformanceResult::pass("domain_separation")
    }

    pub fn test_hash_32_bytes() -> ConformanceResult {
        let hash = DomainSeparatedHash::new(b"test", b"data");

        if hash.output.len() != 32 {
            return ConformanceResult::fail("hash_32_bytes", "Hash not 32 bytes");
        }

        ConformanceResult::pass("hash_32_bytes")
    }

    pub fn test_derive_seed_deterministic() -> ConformanceResult {
        let seed1 = DomainSeparatedHash::derive_seed(b"validator", [1u8; 32], 100);
        let seed2 = DomainSeparatedHash::derive_seed(b"validator", [1u8; 32], 100);

        if seed1 != seed2 {
            return ConformanceResult::fail("derive_seed", "Seed not deterministic");
        }

        ConformanceResult::pass("derive_seed")
    }

    pub fn run_all() -> Vec<ConformanceResult> {
        vec![
            Self::test_domain_separation(),
            Self::test_hash_32_bytes(),
            Self::test_derive_seed_deterministic(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_separation() {
        let result = ConformanceTest::test_domain_separation();
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn test_hash_32_bytes() {
        let result = ConformanceTest::test_hash_32_bytes();
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn test_derive_seed() {
        let result = ConformanceTest::test_derive_seed_deterministic();
        assert!(result.passed, "{}", result.message);
    }

    #[test]
    fn test_all() {
        let results = ConformanceTest::run_all();
        for result in &results {
            assert!(result.passed, "{}: {}", result.test_name, result.message);
        }
    }
}
