use serde::{Deserialize, Serialize};
use async_trait::async_trait;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelSpec {
    pub id: [u8; 32],
    pub version: u32,
    pub name: String,
    pub input_schema: Vec<KernelParam>,
    pub output_schema: Vec<KernelParam>,
    pub compute_bound: u64,
    pub memory_bound: u64,
    pub deterministic: bool,
    pub resource_profile_hash: [u8; 32],
}

impl KernelSpec {
    pub fn compute_id(name: &str, version: u32, metadata: &[u8]) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(name.as_bytes());
        hasher.update(&version.to_le_bytes());
        hasher.update(metadata);
        *hasher.finalize().as_bytes()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelParam {
    pub name: String,
    pub param_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelInput {
    pub job_id: [u8; 32],
    pub kernel_id: [u8; 32],
    pub deterministic_seed: Option<[u8; 32]>,
    pub budget: u64,
    pub memory_limit: u64,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelOutput {
    pub job_id: [u8; 32],
    pub output_hash: [u8; 32],
    pub compute_used: u64,
    pub memory_used: u64,
    pub output: Vec<u8>,
    pub deterministic: bool,
}

impl KernelOutput {
    pub fn compute_output_hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&self.output);
        hasher.update(&self.compute_used.to_le_bytes());
        *hasher.finalize().as_bytes()
    }
}

#[async_trait]
pub trait Kernel: Send + Sync {
    fn spec(&self) -> &KernelSpec;
    async fn execute(&self, input: &[u8]) -> Result<Vec<u8>, String>;
    fn version(&self) -> u32;
}

pub struct KernelRegistry {
    kernels: Vec<KernelBox>,
}

type KernelBox = Box<dyn Kernel>;

impl KernelRegistry {
    pub fn new() -> Self {
        Self { kernels: Vec::new() }
    }

    pub fn register<K: Kernel + 'static>(&mut self, kernel: K) {
        self.kernels.push(Box::new(kernel));
    }

    pub fn get(&self, id: &[u8; 32]) -> Option<&dyn Kernel> {
        self.kernels.iter().find(|k| k.spec().id == *id).map(|k| k.as_ref() as _)
    }

    pub fn list(&self) -> Vec<&KernelSpec> {
        self.kernels.iter().map(|k| k.spec()).collect()
    }
}

impl Default for KernelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelExecutionContext {
    pub job_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub block_height: u64,
    pub deterministic_seed: Option<[u8; 32]>,
    pub budget: u64,
    pub memory_limit: u64,
    pub start_time: u64,
}

impl KernelExecutionContext {
    pub fn new(job_id: [u8; 32], operator_id: [u8; 32], block_height: u64) -> Self {
        Self {
            job_id,
            operator_id,
            block_height,
            deterministic_seed: None,
            budget: 0,
            memory_limit: 0,
            start_time: 0,
        }
    }

    pub fn with_deterministic_seed(mut self, seed: [u8; 32]) -> Self {
        self.deterministic_seed = Some(seed);
        self
    }

    pub fn with_budget(mut self, budget: u64) -> Self {
        self.budget = budget;
        self
    }

    pub fn with_memory_limit(mut self, limit: u64) -> Self {
        self.memory_limit = limit;
        self
    }

    pub fn is_exhausted(&self) -> bool {
        if self.budget > 0 {
            return true;
        }
        false
    }
}

pub enum SandboxMode {
    Native,
    Wasm,
    Gpu,
}

pub trait Sandbox: Send + Sync {
    fn load(&mut self, code: &[u8]) -> Result<(), String>;
    fn execute(&mut self, input: &[u8], budget: u64) -> Result<Vec<u8>, String>;
    fn get_used_resources(&self) -> (u64, u64);
}

pub struct NativeSandbox;

impl Sandbox for NativeSandbox {
    fn load(&mut self, _code: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn execute(&mut self, input: &[u8], _budget: u64) -> Result<Vec<u8>, String> {
        Ok(input.to_vec())
    }

    fn get_used_resources(&self) -> (u64, u64) {
        (0, 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceLimitType {
    ComputeUnits,
    MemoryBytes,
    Duration,
    GPUCycles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimit {
    pub limit_type: ResourceLimitType,
    pub value: u64,
}

impl ResourceLimit {
    pub fn compute_units(value: u64) -> Self {
        Self {
            limit_type: ResourceLimitType::ComputeUnits,
            value,
        }
    }

    pub fn memory_bytes(value: u64) -> Self {
        Self {
            limit_type: ResourceLimitType::MemoryBytes,
            value,
        }
    }
}

pub struct KernelExecutor {
    sandbox: NativeSandbox,
    context: Option<KernelExecutionContext>,
}

impl KernelExecutor {
    pub fn new() -> Self {
        Self {
            sandbox: NativeSandbox,
            context: None,
        }
    }

    pub fn with_context(mut self, context: KernelExecutionContext) -> Self {
        self.context = Some(context);
        self
    }

    pub fn execute(&mut self, spec: &KernelSpec, input: Vec<u8>) -> Result<KernelOutput, String> {
        if let Some(ctx) = &self.context {
            if ctx.budget < spec.compute_bound {
                return Err("Insufficient budget for kernel execution".to_string());
            }
            if ctx.memory_limit > 0 && ctx.memory_limit < spec.memory_bound {
                return Err("Insufficient memory for kernel execution".to_string());
            }
        }

        let start = std::time::Instant::now();
        
        let output = self.sandbox.execute(&input, spec.compute_bound)?;
        
        let elapsed = start.elapsed().as_millis() as u64;
        
        let output_hash = {
            use blake3::Hasher;
            let mut hasher = Hasher::new();
            hasher.update(&output);
            *hasher.finalize().as_bytes()
        };

        Ok(KernelOutput {
            job_id: self.context.as_ref().map(|c| c.job_id).unwrap_or([0u8; 32]),
            output_hash,
            compute_used: elapsed,
            memory_used: 0,
            output,
            deterministic: spec.deterministic,
        })
    }
}

impl Default for KernelExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_spec_id() {
        let id = KernelSpec::compute_id("test-kernel", 1, b"metadata");
        assert_eq!(id.len(), 32);
        
        let id2 = KernelSpec::compute_id("test-kernel", 1, b"metadata");
        assert_eq!(id, id2);
        
        let id3 = KernelSpec::compute_id("test-kernel", 2, b"metadata");
        assert_ne!(id, id3);
    }

    #[test]
    fn test_kernel_output_hash() {
        let output = KernelOutput {
            job_id: [1u8; 32],
            output_hash: [0u8; 32],
            compute_used: 100,
            memory_used: 50,
            output: vec![1, 2, 3, 4],
            deterministic: true,
        };
        
        let hash = output.compute_output_hash();
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_execution_context() {
        let ctx = KernelExecutionContext::new([1u8; 32], [2u8; 32], 100)
            .with_budget(1000)
            .with_memory_limit(1024);
        
        assert_eq!(ctx.job_id, [1u8; 32]);
        assert_eq!(ctx.operator_id, [2u8; 32]);
        assert_eq!(ctx.block_height, 100);
        assert_eq!(ctx.budget, 1000);
        assert_eq!(ctx.memory_limit, 1024);
    }

    #[test]
    fn test_kernel_executor() {
        let spec = KernelSpec {
            id: [0u8; 32],
            version: 1,
            name: "test".to_string(),
            input_schema: vec![],
            output_schema: vec![],
            compute_bound: 1000,
            memory_bound: 1024,
            deterministic: true,
            resource_profile_hash: [0u8; 32],
        };

        let ctx = KernelExecutionContext::new([1u8; 32], [2u8; 32], 100)
            .with_budget(2000)
            .with_memory_limit(2048);

        let mut executor = KernelExecutor::new().with_context(ctx);
        
        let result = executor.execute(&spec, vec![1, 2, 3]);
        assert!(result.is_ok());
        
        let output = result.unwrap();
        assert_eq!(output.job_id, [1u8; 32]);
        assert!(output.deterministic);
    }

    #[test]
    fn test_resource_limits() {
        let compute_limit = ResourceLimit::compute_units(1000);
        assert_eq!(compute_limit.limit_type, ResourceLimitType::ComputeUnits);
        assert_eq!(compute_limit.value, 1000);

        let memory_limit = ResourceLimit::memory_bytes(1024 * 1024);
        assert_eq!(memory_limit.limit_type, ResourceLimitType::MemoryBytes);
    }
}
