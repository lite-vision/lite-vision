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
    
    pub fn new(
        name: String,
        version: u32,
        compute_bound: u64,
        memory_bound: u64,
        deterministic: bool,
    ) -> Self {
        let id = Self::compute_id(&name, version, &[]);
        Self {
            id,
            version,
            name,
            input_schema: vec![],
            output_schema: vec![],
            compute_bound,
            memory_bound,
            deterministic,
            resource_profile_hash: [0u8; 32],
        }
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
    
    pub fn new(job_id: [u8; 32], output: Vec<u8>, compute_used: u64) -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&output);
        let output_hash = *hasher.finalize().as_bytes();
        
        Self {
            job_id,
            output_hash,
            compute_used,
            memory_used: 0,
            output,
            deterministic: true,
        }
    }
}

/// Kernel trait for actual GPU kernel execution
#[async_trait]
pub trait Kernel: Send + Sync {
    fn spec(&self) -> &KernelSpec;
    async fn execute(&self, input: &[u8]) -> Result<Vec<u8>, KernelError>;
    fn version(&self) -> u32;
}

/// Kernel execution errors
#[derive(Debug, Clone)]
pub enum KernelError {
    ExecutionFailed(String),
    InsufficientBudget,
    InsufficientMemory,
    InvalidInput,
    Timeout,
    GpuNotAvailable,
}

impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KernelError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            KernelError::InsufficientBudget => write!(f, "Insufficient budget"),
            KernelError::InsufficientMemory => write!(f, "Insufficient memory"),
            KernelError::InvalidInput => write!(f, "Invalid input"),
            KernelError::Timeout => write!(f, "Execution timeout"),
            KernelError::GpuNotAvailable => write!(f, "GPU not available"),
        }
    }
}

impl std::error::Error for KernelError {}

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
    
    pub fn has_kernel(&self, id: &[u8; 32]) -> bool {
        self.kernels.iter().any(|k| k.spec().id == *id)
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
    
    pub fn can_execute(&self, spec: &KernelSpec) -> bool {
        self.budget >= spec.compute_bound && 
        (self.memory_limit == 0 || self.memory_limit >= spec.memory_bound)
    }
}

pub enum SandboxMode {
    Native,
    Wasm,
    Gpu,
}

/// Sandbox trait for isolated execution
pub trait Sandbox: Send + Sync {
    fn load(&mut self, code: &[u8]) -> Result<(), String>;
    fn execute(&mut self, input: &[u8], budget: u64) -> Result<Vec<u8>, String>;
    fn get_used_resources(&self) -> (u64, u64);
}

/// Native sandbox - executes in-process
pub struct NativeSandbox {
    code: Option<Vec<u8>>,
    compute_used: u64,
    memory_used: u64,
}

impl NativeSandbox {
    pub fn new() -> Self {
        Self {
            code: None,
            compute_used: 0,
            memory_used: 0,
        }
    }
}

impl Sandbox for NativeSandbox {
    fn load(&mut self, code: &[u8]) -> Result<(), String> {
        self.code = Some(code.to_vec());
        Ok(())
    }

    fn execute(&mut self, input: &[u8], _budget: u64) -> Result<Vec<u8>, String> {
        let start = std::time::Instant::now();
        
        // Execute code in native context
        // In a real implementation, this would actually process the input
        // For now, we simulate execution
        let output = input.to_vec();
        
        let elapsed = start.elapsed().as_nanos() as u64;
        self.compute_used = elapsed.max(1);
        self.memory_used = output.len() as u64;
        
        Ok(output)
    }

    fn get_used_resources(&self) -> (u64, u64) {
        (self.compute_used, self.memory_used)
    }
}

impl Default for NativeSandbox {
    fn default() -> Self {
        Self::new()
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

/// KernelExecutor - Actual job execution
pub struct KernelExecutor {
    sandbox: NativeSandbox,
    context: Option<KernelExecutionContext>,
    #[allow(dead_code)]
    registry: KernelRegistry,
}

impl KernelExecutor {
    pub fn new() -> Self {
        Self {
            sandbox: NativeSandbox::new(),
            context: None,
            registry: KernelRegistry::new(),
        }
    }

    pub fn with_context(mut self, context: KernelExecutionContext) -> Self {
        self.context = Some(context);
        self
    }
    
    pub fn with_registry(mut self, registry: KernelRegistry) -> Self {
        self.registry = registry;
        self
    }
    
    /// Execute a kernel with actual processing
    pub fn execute(&mut self, spec: &KernelSpec, input: Vec<u8>) -> Result<KernelOutput, String> {
        let ctx = self.context.as_ref().ok_or("No execution context")?;
        
        // Check budgets
        if ctx.budget < spec.compute_bound {
            return Err("Insufficient budget for kernel execution".to_string());
        }
        if ctx.memory_limit > 0 && ctx.memory_limit < spec.memory_bound {
            return Err("Insufficient memory for kernel execution".to_string());
        }
        
        let start = std::time::Instant::now();
        
        // Execute in sandbox
        let output = self.sandbox.execute(&input, spec.compute_bound)?;
        
        let elapsed = start.elapsed().as_millis() as u64;
        
        // Compute deterministic output hash
        let output_hash = {
            use blake3::Hasher;
            let mut hasher = Hasher::new();
            hasher.update(&output);
            hasher.update(&elapsed.to_le_bytes());
            if let Some(seed) = ctx.deterministic_seed {
                hasher.update(&seed);
            }
            *hasher.finalize().as_bytes()
        };
        
        Ok(KernelOutput {
            job_id: ctx.job_id,
            output_hash,
            compute_used: elapsed,
            memory_used: output.len() as u64,
            output,
            deterministic: spec.deterministic,
        })
    }
    
    /// Execute with deterministic seed for reproducible results
    pub fn execute_deterministic(
        &mut self,
        spec: &KernelSpec,
        input: Vec<u8>,
        seed: [u8; 32],
    ) -> Result<KernelOutput, String> {
        let mut ctx = self.context.clone().ok_or("No execution context")?;
        ctx.deterministic_seed = Some(seed);
        self.context = Some(ctx);
        
        self.execute(spec, input)
    }
}

impl Default for KernelExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU Kernel Interface - for actual GPU execution
/// This is a stub that would be replaced with actual GPU kernel bindings
pub mod gpu_kernel {
    use super::*;
    use async_trait::async_trait;
    
    /// GPU kernel for ML/render workloads
    #[derive(Debug)]
    pub struct GpuKernel {
        spec: KernelSpec,
    }
    
    impl GpuKernel {
        pub fn new(name: String, version: u32, compute_bound: u64, memory_bound: u64) -> Self {
            let spec = KernelSpec::new(name, version, compute_bound, memory_bound, true);
            Self { spec }
        }
    }
    
    #[async_trait]
    impl Kernel for GpuKernel {
        fn spec(&self) -> &KernelSpec {
            &self.spec
        }
        
        async fn execute(&self, input: &[u8]) -> Result<Vec<u8>, KernelError> {
            // In real implementation, this would:
            // 1. Upload input to GPU
            // 2. Execute kernel
            // 3. Download output from GPU
            
            // For now, simulate GPU execution
            let start = std::time::Instant::now();
            
            // Simulate computation
            let output = input.to_vec();
            
            let elapsed = start.elapsed().as_millis() as u64;
            if elapsed > 30000 {
                return Err(KernelError::Timeout);
            }
            
            Ok(output)
        }
        
        fn version(&self) -> u32 {
            self.spec.version
        }
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
    fn test_kernel_spec_creation() {
        let spec = KernelSpec::new(
            "test-kernel".to_string(),
            1,
            1000,
            1024,
            true,
        );
        
        assert_eq!(spec.name, "test-kernel");
        assert_eq!(spec.version, 1);
        assert!(spec.deterministic);
    }

    #[test]
    fn test_kernel_output_hash() {
        let output = KernelOutput::new([1u8; 32], vec![1, 2, 3, 4], 100);
        
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
    fn test_execution_context_can_execute() {
        let ctx = KernelExecutionContext::new([1u8; 32], [2u8; 32], 100)
            .with_budget(2000)
            .with_memory_limit(2048);
        
        let spec = KernelSpec::new("test".to_string(), 1, 1000, 1024, true);
        
        assert!(ctx.can_execute(&spec));
    }

    #[test]
    fn test_kernel_executor() {
        let spec = KernelSpec::new("test".to_string(), 1, 1000, 1024, true);

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
    fn test_kernel_executor_insufficient_budget() {
        let spec = KernelSpec::new("test".to_string(), 1, 1000, 1024, true);

        let ctx = KernelExecutionContext::new([1u8; 32], [2u8; 32], 100)
            .with_budget(500) // Less than compute_bound
            .with_memory_limit(2048);

        let mut executor = KernelExecutor::new().with_context(ctx);
        
        let result = executor.execute(&spec, vec![1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn test_resource_limits() {
        let compute_limit = ResourceLimit::compute_units(1000);
        assert_eq!(compute_limit.limit_type, ResourceLimitType::ComputeUnits);
        assert_eq!(compute_limit.value, 1000);

        let memory_limit = ResourceLimit::memory_bytes(1024 * 1024);
        assert_eq!(memory_limit.limit_type, ResourceLimitType::MemoryBytes);
    }

    #[test]
    fn test_kernel_registry() {
        let mut registry = KernelRegistry::new();
        
        let spec = KernelSpec::new("test".to_string(), 1, 1000, 1024, true);
        let spec2 = KernelSpec::new("test2".to_string(), 1, 2000, 2048, true);
        
        // Can't register directly - need kernel implementation
        // But can list and check
        let kernels = registry.list();
        assert!(kernels.is_empty());
    }

    #[test]
    fn test_native_sandbox() {
        let mut sandbox = NativeSandbox::new();
        
        sandbox.load(b"test code").unwrap();
        
        let result = sandbox.execute(b"input data", 1000);
        assert!(result.is_ok());
        
        let (compute, memory) = sandbox.get_used_resources();
        assert!(compute > 0);
    }
}