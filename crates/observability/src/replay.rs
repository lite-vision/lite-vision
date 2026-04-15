use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_gpu_cycles: u64,
    pub max_cpu_cycles: u64,
    pub max_memory_bytes: u64,
    pub max_execution_time_ms: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_gpu_cycles: u64::MAX,
            max_cpu_cycles: u64::MAX,
            max_memory_bytes: 64 * 1024 * 1024 * 1024,
            max_execution_time_ms: 300000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl KernelVersion {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn to_tuple(&self) -> (u16, u16, u16) {
        (self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBundle {
    pub version: u16,
    pub job_id: [u8; 32],
    pub kernel_id: [u8; 32],
    pub kernel_version: KernelVersion,
    pub deterministic_seed: [u8; 32],
    pub input_hash: [u8; 32],
    pub input_bytes: Vec<u8>,
    pub initial_state_hash: Option<[u8; 32]>,
    pub execution_context: Vec<u8>,
    pub expected_output_hash: [u8; 32],
    pub resource_caps: ResourceLimits,
    pub capture_timestamp_ms: u64,
    pub capture_signature: Option<Vec<u8>>,
}

impl ReplayBundle {
    pub fn new(
        job_id: [u8; 32],
        kernel_id: [u8; 32],
        kernel_version: KernelVersion,
        deterministic_seed: [u8; 32],
        input_bytes: Vec<u8>,
        initial_state_hash: Option<[u8; 32]>,
        execution_context: Vec<u8>,
        resource_caps: ResourceLimits,
    ) -> Self {
        let input_hash = Self::hash_bytes(&input_bytes);

        Self {
            version: 1,
            job_id,
            kernel_id,
            kernel_version,
            deterministic_seed,
            input_hash,
            input_bytes,
            initial_state_hash,
            execution_context,
            expected_output_hash: [0u8; 32],
            resource_caps,
            capture_timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            capture_signature: None,
        }
    }

    pub fn with_expected_output(mut self, output_hash: [u8; 32]) -> Self {
        self.expected_output_hash = output_hash;
        self
    }

    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.capture_signature = Some(signature);
        self
    }

    pub fn hash_bytes(data: &[u8]) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(data);
        *hasher.finalize().as_bytes()
    }

    pub fn compute_bundle_hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();

        hasher.update(&self.version.to_le_bytes());
        hasher.update(&self.job_id);
        hasher.update(&self.kernel_id);
        hasher.update(&self.kernel_version.major.to_le_bytes());
        hasher.update(&self.kernel_version.minor.to_le_bytes());
        hasher.update(&self.kernel_version.patch.to_le_bytes());
        hasher.update(&self.deterministic_seed);
        hasher.update(&self.input_hash);
        hasher.update(&self.input_bytes);
        if let Some(state_hash) = self.initial_state_hash {
            hasher.update(&state_hash);
        }
        hasher.update(&self.execution_context);
        hasher.update(&self.expected_output_hash);
        hasher.update(&self.resource_caps.max_gpu_cycles.to_le_bytes());
        hasher.update(&self.resource_caps.max_cpu_cycles.to_le_bytes());
        hasher.update(&self.resource_caps.max_memory_bytes.to_le_bytes());
        hasher.update(&self.resource_caps.max_execution_time_ms.to_le_bytes());
        hasher.update(&self.capture_timestamp_ms.to_le_bytes());

        *hasher.finalize().as_bytes()
    }

    pub fn verify(&self) -> Result<(), ReplayError> {
        if self.version == 0 {
            return Err(ReplayError::InvalidVersion);
        }

        let _computed_hash = self.compute_bundle_hash();
        if self.input_hash != Self::hash_bytes(&self.input_bytes) {
            return Err(ReplayError::InputHashMismatch);
        }

        Ok(())
    }

    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    pub job_id: [u8; 32],
    pub execution_output_hash: [u8; 32],
    pub execution_time_ms: u64,
    pub resource_usage: ResourceUsage,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceUsage {
    pub gpu_cycles_used: u64,
    pub cpu_cycles_used: u64,
    pub memory_bytes_used: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    InvalidVersion,
    InputHashMismatch,
    OutputHashMismatch,
    ExecutionTimeout,
    ResourceExceeded,
    NetworkRequired,
    DeterminismViolation,
    SignatureInvalid,
    StorageError(String),
}

pub struct ReplayEngine {
    bundles: std::collections::HashMap<[u8; 32], ReplayBundle>,
    retention_window_blocks: u64,
}

impl ReplayEngine {
    pub fn new(retention_window_blocks: u64) -> Self {
        Self {
            bundles: std::collections::HashMap::new(),
            retention_window_blocks,
        }
    }

    pub fn capture(&mut self, bundle: ReplayBundle) -> Result<[u8; 32], ReplayError> {
        bundle.verify()?;
        let hash = bundle.compute_bundle_hash();
        self.bundles.insert(bundle.job_id, bundle);
        Ok(hash)
    }

    pub fn get_bundle(&self, job_id: &[u8; 32]) -> Option<&ReplayBundle> {
        self.bundles.get(job_id)
    }

    pub fn verify_replay(
        &self,
        job_id: &[u8; 32],
        output_hash: [u8; 32],
        resource_usage: ResourceUsage,
    ) -> Result<ReplayResult, ReplayError> {
        let bundle = self
            .bundles
            .get(job_id)
            .ok_or(ReplayError::StorageError("Bundle not found".to_string()))?;

        let output_matches = output_hash == bundle.expected_output_hash;

        if !output_matches {
            return Ok(ReplayResult {
                job_id: *job_id,
                execution_output_hash: output_hash,
                execution_time_ms: 0,
                resource_usage,
                success: false,
                error_message: Some("Output hash mismatch - fraud detected".to_string()),
            });
        }

        if resource_usage.gpu_cycles_used > bundle.resource_caps.max_gpu_cycles {
            return Err(ReplayError::ResourceExceeded);
        }

        if resource_usage.cpu_cycles_used > bundle.resource_caps.max_cpu_cycles {
            return Err(ReplayError::ResourceExceeded);
        }

        if resource_usage.memory_bytes_used > bundle.resource_caps.max_memory_bytes {
            return Err(ReplayError::ResourceExceeded);
        }

        Ok(ReplayResult {
            job_id: *job_id,
            execution_output_hash: output_hash,
            execution_time_ms: 0,
            resource_usage,
            success: true,
            error_message: None,
        })
    }

    pub fn prune_old_bundles(&mut self, current_block_height: u64, safe_window: u64) {
        let threshold = current_block_height.saturating_sub(safe_window);
        self.bundles.retain(|_, bundle| {
            let bundle_block = bundle.capture_timestamp_ms / 15000;
            bundle_block > threshold
        });
    }

    pub fn export_bundle(&self, job_id: &[u8; 32]) -> Option<Vec<u8>> {
        self.bundles.get(job_id).map(|b| b.to_canonical_bytes())
    }

    pub fn import_bundle(&mut self, data: Vec<u8>) -> Result<[u8; 32], ReplayError> {
        let bundle: ReplayBundle =
            bincode::deserialize(&data).map_err(|e| ReplayError::StorageError(e.to_string()))?;

        bundle.verify()?;
        let hash = bundle.compute_bundle_hash();
        self.bundles.insert(bundle.job_id, bundle);
        Ok(hash)
    }
}

pub struct DeterministicExecutor;

impl DeterministicExecutor {
    pub fn execute<F>(
        seed: [u8; 32],
        input: &[u8],
        _resource_caps: &ResourceLimits,
        f: F,
    ) -> Result<Vec<u8>, ReplayError>
    where
        F: FnOnce(&[u8], u64) -> Vec<u8>,
    {
        let output = f(input, Self::seed_to_u64(seed));
        Ok(output)
    }

    fn seed_to_u64(seed: [u8; 32]) -> u64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&seed[..8]);
        u64::from_le_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_bundle_creation() {
        let bundle = ReplayBundle::new(
            [1u8; 32],
            [2u8; 32],
            KernelVersion::new(1, 0, 0),
            [3u8; 32],
            vec![4u8; 100],
            Some([5u8; 32]),
            vec![6u8; 50],
            ResourceLimits::default(),
        );

        assert_eq!(bundle.version, 1);
        assert_eq!(bundle.job_id, [1u8; 32]);
    }

    #[test]
    fn test_bundle_hash_determinism() {
        let bundle1 = ReplayBundle::new(
            [1u8; 32],
            [2u8; 32],
            KernelVersion::new(1, 0, 0),
            [3u8; 32],
            vec![4u8; 100],
            None,
            vec![],
            ResourceLimits::default(),
        );

        let bundle2 = ReplayBundle::new(
            [1u8; 32],
            [2u8; 32],
            KernelVersion::new(1, 0, 0),
            [3u8; 32],
            vec![4u8; 100],
            None,
            vec![],
            ResourceLimits::default(),
        );

        assert_eq!(bundle1.compute_bundle_hash(), bundle2.compute_bundle_hash());
    }

    #[test]
    fn test_bundle_verification() {
        let bundle = ReplayBundle::new(
            [1u8; 32],
            [2u8; 32],
            KernelVersion::new(1, 0, 0),
            [3u8; 32],
            vec![4u8; 100],
            None,
            vec![],
            ResourceLimits::default(),
        )
        .with_expected_output([5u8; 32]);

        assert!(bundle.verify().is_ok());
    }

    #[test]
    fn test_replay_engine_capture_and_verify() {
        let mut engine = ReplayEngine::new(1000);

        let bundle = ReplayBundle::new(
            [1u8; 32],
            [2u8; 32],
            KernelVersion::new(1, 0, 0),
            [3u8; 32],
            vec![4u8; 100],
            None,
            vec![],
            ResourceLimits::default(),
        )
        .with_expected_output([5u8; 32]);

        let hash = engine.capture(bundle).unwrap();

        let result = engine.verify_replay(&[1u8; 32], [5u8; 32], ResourceUsage::default());

        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[test]
    fn test_replay_fraud_detection() {
        let mut engine = ReplayEngine::new(1000);

        let bundle = ReplayBundle::new(
            [1u8; 32],
            [2u8; 32],
            KernelVersion::new(1, 0, 0),
            [3u8; 32],
            vec![4u8; 100],
            None,
            vec![],
            ResourceLimits::default(),
        )
        .with_expected_output([5u8; 32]);

        engine.capture(bundle).unwrap();

        let result = engine.verify_replay(&[1u8; 32], [6u8; 32], ResourceUsage::default());

        assert!(result.is_ok());
        assert!(!result.unwrap().success);
    }

    #[test]
    fn test_deterministic_executor() {
        let seed = [1u8; 32];
        let input = vec![2u8; 10];

        let result = DeterministicExecutor::execute(
            seed,
            &input,
            &ResourceLimits::default(),
            |inp, seed_val| {
                let mut output = inp.to_vec();
                output.push((seed_val % 256) as u8);
                output
            },
        );

        assert!(result.is_ok());
    }
}
