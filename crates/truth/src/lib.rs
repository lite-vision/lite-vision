pub mod consensus;
pub mod state;
pub mod validator;
pub mod block;
pub mod transaction;
pub mod cryptography;
pub mod governance;
pub mod storage;
pub mod messaging;
pub mod cross_partition;
pub mod validator_set;
pub mod rpc;
pub mod sync;
pub mod pruning;
pub mod settlement; // NEW - Economic settlement primitives

#[cfg(test)]
pub mod integration; // Integration tests

pub use consensus::{ConsensusState, Vote, VoteType, QuorumCertificate};
pub use state::State;
pub use validator_set::{ValidatorSet, Validator, ValidatorStatus, ValidatorMetadata};
pub use block::{Block, BlockHeader};
pub use transaction::{Transaction, TransactionType, TxError};
pub use cryptography::*;
pub use storage::Storage;
pub use messaging::Message;
pub use cross_partition::GlobalStateRoot;
pub use rpc::*;
pub use sync::*;
pub use pruning::*;
pub use settlement::{SettlementState, JobEscrow, SlashingConditions};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");