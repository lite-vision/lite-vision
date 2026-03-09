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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelParam {
    pub name: String,
    pub param_type: String,
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
