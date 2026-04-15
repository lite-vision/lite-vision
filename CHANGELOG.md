# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Core
- Conformance tests for deterministic hashing
- Domain-separated hash verification
- 32-byte hash output verification
- Derive seed determinism

#### Truth Plane (BFT Consensus + Ledger)
- **Consensus**: ConsensusEngine with Propose/PreVote/PreCommit phases
- **State**: State machine with account management, transactions
- **RPC**: JSON-RPC server with async TCP
- **Validators**: Validator set with staking and slashing
- **Cryptography**: Key generation, signing, verification
- **Governance**: Proposals and voting

#### Intelligence Plane (GPU Compute)
- **Jobs**: JobTicket, JobExecutor with budget enforcement
- **Operators**: Operator capabilities and registration
- **Routing**: Thalamus sparse routing
- **Receipts**: Attestation and metering
- **Verification**: VerificationEngine with redundancy
- **Disputes**: Fraud proof and dispute resolution
- **Health Monitoring**: HealthCheck, HealthReport, HealthMonitor

#### RPACK (Render Packet)
- **Container**: RPackBuilder with chunk management
- **Scene IR**: Scene graph with nodes, materials, animations
- **Assets**: FetchHint, RendererFallbackPolicy
- **Delta**: PatchSet, PatchOperation

#### Storage
- **Memory Model**: Dual-plane memory (Ephemeral/Regional/Committed)
- **CRDTs**: GCounter, PNCounter, ORSet, LWWRegister
- **Partitions**: PartitionManager with rebalancing
- **Artifacts**: ArtifactStore with integrity proofs

#### Network
- **P2P**: Peer management, connection handling
- **NetworkNode**: Unified networking with plane modes
- **Protocol**: Message types and handling
- **Message Queue**: Topic-based pub/sub

#### SDK & CLI
- **SDK**: Client for job submission
- **CLI**: Commands for node, validator, operator, job

### Test Coverage
- 266+ unit tests across all crates
- Comprehensive serialization tests
- Hash computation verification
- State machine transitions

## [0.1.0] - 2024-01-01

### Added
- Initial workspace setup with 8 crates
- Basic Cargo.toml configuration
- Foundation for three-plane architecture

[Unreleased]: https://github.com/lite-vision/lite-vision/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lite-vision/lite-vision/releases/tag/v0.1.0