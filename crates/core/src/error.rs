use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Invalid hash: {0}")]
    InvalidHash(String),

    #[error("Invalid encoding: {0}")]
    InvalidEncoding(String),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u32, actual: u32 },

    #[error("Canonical encoding violation: {0}")]
    CanonicalViolation(String),

    #[error("Determinism violation: {0}")]
    DeterminismViolation(String),

    #[error("Overflow: {0}")]
    Overflow(String),

    #[error("Underflow: {0}")]
    Underflow(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CoreError::InvalidHash("test".to_string());
        assert_eq!(err.to_string(), "Invalid hash: test");
    }

    #[test]
    fn test_checksum_mismatch_error() {
        let err = CoreError::ChecksumMismatch {
            expected: "abc".to_string(),
            actual: "def".to_string(),
        };
        assert_eq!(err.to_string(), "Checksum mismatch: expected abc, got def");
    }

    #[test]
    fn test_version_mismatch_error() {
        let err = CoreError::VersionMismatch {
            expected: 2,
            actual: 1,
        };
        assert_eq!(err.to_string(), "Version mismatch: expected 2, got 1");
    }
}
