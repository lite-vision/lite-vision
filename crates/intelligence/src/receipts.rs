use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub version: u32,
    pub job_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub kernel_id: [u8; 32],
    pub kernel_version: (u16, u16, u16),
    pub input_hash: [u8; 32],
    pub deterministic_seed: Option<[u8; 32]>,
    pub execution_nonce: u64,
    pub output_hash: [u8; 32],
    pub resource_hash: [u8; 32],
    pub execution_mode: ExecutionMode,
    pub start_block_height: u64,
    pub end_block_height: u64,
    pub signature: Vec<u8>,
    pub attestation: Option<Attestation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    Soft,
    Deterministic,
}

impl Receipt {
    pub fn new(
        job_id: [u8; 32],
        operator_id: [u8; 32],
        kernel_id: [u8; 32],
        kernel_version: (u16, u16, u16),
        input_hash: [u8; 32],
        output_hash: [u8; 32],
        resources: &ResourceUsage,
        execution_mode: ExecutionMode,
        start_block_height: u64,
        end_block_height: u64,
    ) -> Self {
        let resource_hash = resources.hash();

        Self {
            version: 1,
            job_id,
            operator_id,
            kernel_id,
            kernel_version,
            input_hash,
            deterministic_seed: None,
            execution_nonce: 0,
            output_hash,
            resource_hash,
            execution_mode,
            start_block_height,
            end_block_height,
            signature: Vec::new(),
            attestation: None,
        }
    }

    pub fn with_deterministic_seed(mut self, seed: [u8; 32]) -> Self {
        self.deterministic_seed = Some(seed);
        self
    }

    pub fn with_nonce(mut self, nonce: u64) -> Self {
        self.execution_nonce = nonce;
        self
    }

    pub fn sign(&mut self, signature: Vec<u8>) {
        self.signature = signature;
    }

    pub fn with_attestation(mut self, attestation: Attestation) -> Self {
        self.attestation = Some(attestation);
        self
    }

    pub fn hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();

        hasher.update(&self.version.to_le_bytes());
        hasher.update(&self.job_id);
        hasher.update(&self.operator_id);
        hasher.update(&self.kernel_id);
        hasher.update(&self.kernel_version.0.to_le_bytes());
        hasher.update(&self.kernel_version.1.to_le_bytes());
        hasher.update(&self.kernel_version.2.to_le_bytes());
        hasher.update(&self.input_hash);

        if let Some(seed) = self.deterministic_seed {
            hasher.update(&seed);
        }

        hasher.update(&self.execution_nonce.to_le_bytes());
        hasher.update(&self.output_hash);
        hasher.update(&self.resource_hash);

        let mode_byte: u8 = match self.execution_mode {
            ExecutionMode::Soft => 0,
            ExecutionMode::Deterministic => 1,
        };
        hasher.update(&[mode_byte]);

        hasher.update(&self.start_block_height.to_le_bytes());
        hasher.update(&self.end_block_height.to_le_bytes());

        *hasher.finalize().as_bytes()
    }

    pub fn verify_signature(&self, public_key: &[u8]) -> bool {
        !self.signature.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub gpu_cycles: u64,
    pub vram_bytes: u64,
    pub cpu_cycles: u64,
    pub memory_bytes: u64,
    pub bandwidth_bytes: u64,
    pub execution_time_ms: u64,
}

impl ResourceUsage {
    pub fn new() -> Self {
        Self {
            gpu_cycles: 0,
            vram_bytes: 0,
            cpu_cycles: 0,
            memory_bytes: 0,
            bandwidth_bytes: 0,
            execution_time_ms: 0,
        }
    }

    pub fn add_gpu_cycles(&mut self, cycles: u64) {
        self.gpu_cycles += cycles;
    }

    pub fn add_vram(&mut self, bytes: u64) {
        self.vram_bytes = self.vram_bytes.max(bytes);
    }

    pub fn add_cpu_cycles(&mut self, cycles: u64) {
        self.cpu_cycles += cycles;
    }

    pub fn add_memory(&mut self, bytes: u64) {
        self.memory_bytes = self.memory_bytes.max(bytes);
    }

    pub fn add_bandwidth(&mut self, bytes: u64) {
        self.bandwidth_bytes += bytes;
    }

    pub fn add_time(&mut self, ms: u64) {
        self.execution_time_ms += ms;
    }

    pub fn hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();

        hasher.update(&self.gpu_cycles.to_le_bytes());
        hasher.update(&self.vram_bytes.to_le_bytes());
        hasher.update(&self.cpu_cycles.to_le_bytes());
        hasher.update(&self.memory_bytes.to_le_bytes());
        hasher.update(&self.bandwidth_bytes.to_le_bytes());
        hasher.update(&self.execution_time_ms.to_le_bytes());

        *hasher.finalize().as_bytes()
    }
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    pub report: Vec<u8>,
    pub signature: Vec<u8>,
    pub timestamp: u64,
    pub tee_type: TEEType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TEEType {
    None,
    SEV,
    SGX,
    TrustZone,
    Nitro,
}

pub struct MeteringEngine {
    pub receipts: HashMap<[u8; 32], Receipt>,
    pub resource_totals: HashMap<[u8; 32], ResourceUsage>,
    pub operator_usage: HashMap<[u8; 32], OperatorMetering>,
}

impl MeteringEngine {
    pub fn new() -> Self {
        Self {
            receipts: HashMap::new(),
            resource_totals: HashMap::new(),
            operator_usage: HashMap::new(),
        }
    }

    pub fn submit_receipt(&mut self, receipt: Receipt) -> Result<[u8; 32], MeteringError> {
        let receipt_hash = receipt.hash();

        if self.receipts.contains_key(&receipt_hash) {
            return Err(MeteringError::DuplicateReceipt);
        }

        self.receipts.insert(receipt_hash, receipt.clone());

        let usage = ResourceUsage::new();
        self.resource_totals.insert(receipt_hash, usage);

        let operator_metering = self
            .operator_usage
            .entry(receipt.operator_id)
            .or_insert_with(OperatorMetering::new);

        operator_metering.total_jobs += 1;

        Ok(receipt_hash)
    }

    pub fn get_receipt(&self, receipt_hash: &[u8; 32]) -> Option<&Receipt> {
        self.receipts.get(receipt_hash)
    }

    pub fn get_receipt_for_job(&self, job_id: &[u8; 32]) -> Option<&Receipt> {
        self.receipts.values().find(|r| r.job_id == *job_id)
    }

    pub fn verify_receipt(
        &self,
        receipt: &Receipt,
        public_key: &[u8],
    ) -> Result<bool, MeteringError> {
        if !self.receipts.contains_key(&receipt.hash()) {
            return Err(MeteringError::ReceiptNotFound);
        }

        Ok(receipt.verify_signature(public_key))
    }

    pub fn calculate_fee(&self, receipt: &Receipt, price_per_cycle: u64) -> u64 {
        let usage = self
            .resource_totals
            .get(&receipt.hash())
            .cloned()
            .unwrap_or_default();

        let gpu_fee = (usage.gpu_cycles / 1_000_000) * price_per_cycle;
        let cpu_fee = (usage.cpu_cycles / 1_000_000) * (price_per_cycle / 10);

        gpu_fee + cpu_fee
    }

    pub fn get_operator_metering(&self, operator_id: &[u8; 32]) -> Option<&OperatorMetering> {
        self.operator_usage.get(operator_id)
    }
}

impl Default for MeteringEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorMetering {
    pub total_jobs: u64,
    pub successful_jobs: u64,
    pub failed_jobs: u64,
    pub total_gpu_cycles: u64,
    pub total_cpu_cycles: u64,
    pub total_fees_earned: u64,
}

impl OperatorMetering {
    pub fn new() -> Self {
        Self {
            total_jobs: 0,
            successful_jobs: 0,
            failed_jobs: 0,
            total_gpu_cycles: 0,
            total_cpu_cycles: 0,
            total_fees_earned: 0,
        }
    }

    pub fn record_success(&mut self, gpu_cycles: u64, cpu_cycles: u64, fee: u64) {
        self.total_jobs += 1;
        self.successful_jobs += 1;
        self.total_gpu_cycles += gpu_cycles;
        self.total_cpu_cycles += cpu_cycles;
        self.total_fees_earned += fee;
    }

    pub fn record_failure(&mut self) {
        self.total_jobs += 1;
        self.failed_jobs += 1;
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_jobs == 0 {
            return 0.0;
        }
        (self.successful_jobs as f64 / self.total_jobs as f64) * 100.0
    }
}

impl Default for OperatorMetering {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeteringError {
    DuplicateReceipt,
    ReceiptNotFound,
    InvalidSignature,
    ResourceExceeded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receipt_creation() {
        let receipt = Receipt::new(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            (1, 0, 0),
            [4u8; 32],
            [5u8; 32],
            &ResourceUsage::new(),
            ExecutionMode::Soft,
            100,
            110,
        );

        assert_eq!(receipt.version, 1);
        assert_eq!(receipt.job_id, [1u8; 32]);
    }

    #[test]
    fn test_receipt_with_deterministic_seed() {
        let receipt = Receipt::new(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            (1, 0, 0),
            [4u8; 32],
            [5u8; 32],
            &ResourceUsage::new(),
            ExecutionMode::Deterministic,
            100,
            110,
        )
        .with_deterministic_seed([6u8; 32]);

        assert!(receipt.deterministic_seed.is_some());
    }

    #[test]
    fn test_receipt_hash() {
        let receipt = Receipt::new(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            (1, 0, 0),
            [4u8; 32],
            [5u8; 32],
            &ResourceUsage::new(),
            ExecutionMode::Soft,
            100,
            110,
        );

        let hash1 = receipt.hash();
        let hash2 = receipt.hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_resource_usage() {
        let mut resources = ResourceUsage::new();

        resources.add_gpu_cycles(1_000_000);
        resources.add_vram(8_000_000_000);
        resources.add_cpu_cycles(100_000);
        resources.add_memory(16_000_000_000);
        resources.add_bandwidth(1_000_000);
        resources.add_time(500);

        assert_eq!(resources.gpu_cycles, 1_000_000);
        assert_eq!(resources.vram_bytes, 8_000_000_000);
    }

    #[test]
    fn test_resource_hash() {
        let mut resources = ResourceUsage::new();
        resources.add_gpu_cycles(1000);

        let hash1 = resources.hash();
        let hash2 = resources.hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_metering_engine_submit() {
        let mut engine = MeteringEngine::new();

        let receipt = Receipt::new(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            (1, 0, 0),
            [4u8; 32],
            [5u8; 32],
            &ResourceUsage::new(),
            ExecutionMode::Soft,
            100,
            110,
        );

        let result = engine.submit_receipt(receipt);
        assert!(result.is_ok());
    }

    #[test]
    fn test_metering_engine_duplicate() {
        let mut engine = MeteringEngine::new();

        let receipt = Receipt::new(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            (1, 0, 0),
            [4u8; 32],
            [5u8; 32],
            &ResourceUsage::new(),
            ExecutionMode::Soft,
            100,
            110,
        );

        engine.submit_receipt(receipt.clone()).unwrap();

        let result = engine.submit_receipt(receipt);
        assert!(matches!(result, Err(MeteringError::DuplicateReceipt)));
    }

    #[test]
    fn test_metering_engine_get_by_job() {
        let mut engine = MeteringEngine::new();

        let receipt = Receipt::new(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            (1, 0, 0),
            [4u8; 32],
            [5u8; 32],
            &ResourceUsage::new(),
            ExecutionMode::Soft,
            100,
            110,
        );

        engine.submit_receipt(receipt).unwrap();

        let found = engine.get_receipt_for_job(&[1u8; 32]);
        assert!(found.is_some());
    }

    #[test]
    fn test_operator_metering() {
        let mut metering = OperatorMetering::new();

        metering.record_success(1_000_000, 100_000, 100);
        metering.record_success(2_000_000, 200_000, 200);
        metering.record_failure();

        assert_eq!(metering.total_jobs, 3);
        assert_eq!(metering.successful_jobs, 2);
        assert_eq!(metering.failed_jobs, 1);
        assert_eq!(metering.total_fees_earned, 300);
    }

    #[test]
    fn test_operator_success_rate() {
        let mut metering = OperatorMetering::new();

        metering.record_success(1_000_000, 100_000, 100);
        metering.record_success(2_000_000, 200_000, 200);
        metering.record_failure();

        let rate = metering.success_rate();
        assert!((rate - 66.66666666666667).abs() < 0.001);
    }

    #[test]
    fn test_attestation() {
        let attestation = Attestation {
            report: vec![1, 2, 3],
            signature: vec![4, 5, 6],
            timestamp: 1000,
            tee_type: TEEType::SGX,
        };

        assert_eq!(attestation.tee_type, TEEType::SGX);
    }

    #[test]
    fn test_receipt_verify_signature() {
        let receipt = Receipt::new(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            (1, 0, 0),
            [4u8; 32],
            [5u8; 32],
            &ResourceUsage::new(),
            ExecutionMode::Soft,
            100,
            110,
        );

        assert!(!receipt.verify_signature(&[1, 2, 3]));

        let mut signed_receipt = receipt;
        signed_receipt.sign(vec![1, 2, 3]);

        assert!(signed_receipt.verify_signature(&[1, 2, 3]));
    }
}
