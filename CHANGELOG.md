# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Truth Plane (0100-series)
- **LV-SPEC-0102**: Validator Set, Staking, and Slashing
  - Validator struct with stake, reputation, status tracking
  - Staking pool management
  - Slashing conditions and penalty calculation
  - Validator set rotation logic

- **LV-SPEC-0107**: Cross-Partition and Cross-Plane Messaging
  - Message types: Request, Response, Broadcast, Relay
  - Cross-partition message routing
  - Plane bridge communication

#### Intelligence Plane (0200-series)
- **LV-SPEC-0203**: Task/Job Model and Execution Semantics
  - JobTicket with QoS, priority, budget constraints
  - JobQueue with priority scheduling
  - ExecutionMode (Eager, Lazy, Speculative)
  - Job state machine

- **LV-SPEC-0204**: Routing and Sparse Activation (Thalamus)
  - SparseRouter with activation thresholds
  - ThalamusRegion for hierarchical routing
  - RouteTable with destination tracking

- **LV-SPEC-0205**: Receipts, Metering, and Attestation
  - Receipt struct with full attestation schema
  - ResourceUsage for GPU/CPU/bandwidth metering
  - MeteringEngine for receipt submission/verification
  - OperatorMetering for operator statistics

- **LV-SPEC-0206**: Verification and Redundancy Policy
  - VerificationPolicy (None, Random, Deterministic, Comprehensive)
  - RedundancyPolicy configuration
  - VerificationEngine with multi-level verification
  - VerificationResult with consensus tracking

- **LV-SPEC-0207**: Challenge/Dispute Protocol
  - FraudProof with evidence bundles
  - DisputeEngine with validation, escalation, resolution
  - SlashDistribution for challenger/verifier/treasury splits
  - Appeal mechanism with time windows
  - Escrow freezing during disputes

#### Render Plane (0300-series)
- **LV-SPEC-0300**: Render Offload Model and Guarantees
  - RendererConstraints with determinism knobs
  - RendererProfile (Soft, Deterministic, Reference)
  - RenderEngine for RPACK validation
  - Anti-coupling guarantees

- **LV-SPEC-0301**: RPACK Container Format
  - Magic bytes and version validation
  - Chunked binary layout with ChunkTable
  - Content addressing via content_hash
  - Compression and encryption hooks
  - Streaming support

- **LV-SPEC-0302**: Scene IR Schema (Semantic-First)
  - SceneIR with nodes, edges, cameras, lights, materials
  - Node semantic types (Object, Actor, Environment, etc.)
  - Transform model with quaternion rotation
  - Constraints (ParentConstraint, LookAt, IKChain, etc.)
  - Animation channels and keyframes
  - DAG validation (acyclic graph check)
  - Reference validation

- **LV-SPEC-0303**: Asset Referencing, Caching, and Fetch Protocol
  - AssetDescriptor with content-addressed IDs
  - Multi-tier cache (Memory, Disk, Regional)
  - LRU eviction policy
  - FetchHint for transport optimization
  - RendererFallbackPolicy for missing assets
  - Offline mode support
  - AssetRegistry for tracking descriptors

#### Storage (0400-series)
- **LV-SPEC-0401**: Regional Memory CRDTs and Merge Semantics
  - RegionalMemory with CRDT counters, sets, registers
  - LWW (Last-Writer-Wins) register
  - G-Counter, PN-Counter
  - LWWMap, ORSet
  - Merge operations with causal ordering

- **LV-SPEC-0402**: Partition Management
  - PartitionManager for shard distribution
  - Partition with key range and replica tracking
  - Rebalance operations
  - Partition metadata and statistics

#### Network/SDK (0500-series)
- **LV-SPEC-0501**: SDK and Client APIs
  - Client for job submission and receipt retrieval
  - JobClient with async job management
  - Receipt verification APIs

### Test Coverage
- 266+ unit tests across all crates
- Comprehensive coverage of:
  - Serialization/deserialization
  - Hash computation and verification
  - State machine transitions
  - Error handling and edge cases
  - Cache eviction behavior

## [0.1.0] - 2024-01-01

### Added
- Initial workspace setup with 8 crates
- Basic Cargo.toml configuration
- Foundation for all three-plane architecture

[Unreleased]: https://github.com/lite-vision/lite-vision/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lite-vision/lite-vision/releases/tag/v0.1.0
