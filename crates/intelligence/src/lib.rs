pub mod operator;
pub mod job;
pub mod kernel;
pub mod routing;
pub mod receipts;
pub mod verification;
pub mod dispute;
pub mod memory;
pub mod render;
pub mod receipt; // Legacy receipt type

#[cfg(test)]
pub mod integration; // Integration tests

pub use operator::*;
pub use job::*;
pub use kernel::*;
pub use routing::{RouteRequest, RouteDecision, Thalamus, OperatorScore};
pub use receipts::*;
pub use verification::*;
pub use dispute::{FraudProof, EvidenceBundle, DisputeInitiate};
pub use memory::*;
pub use render::*;
// Note: 'receipt' (singular) conflicts with receipts module export
// Use receipts::Receipt for the main receipt type

pub use job::{ExecutionMode, Budget, ExecutionCost, QoSClass, VerificationPolicy, CancellationPolicy, Job, JobStatus, JobResult, JobExecutor, JobError};
pub use routing::QoSClass as RoutingQoSClass;
pub use verification::QoSClass as VerificationQoSClass;

/// Kernel Interface - GPU kernel execution
pub use kernel::{KernelSpec, KernelParam, KernelInput, KernelOutput, KernelExecutor, KernelExecutionContext, KernelRegistry, Kernel, KernelError};

/// Verification Engine - Sampling-based verification
pub use verification::VerificationEngine;

/// Dispute Resolver - Fraud proof handling
pub use dispute::DisputeEngine;

const VERSION: &str = env!("CARGO_PKG_VERSION");