# Lite-Vision

A decentralized AI rendering network that separates GPU-powered intelligence from CPU-secured consensus.

## Architecture

Lite-Vision implements a three-plane architecture:

- **Truth Plane**: CPU-secured consensus, settlement, and economic enforcement
- **Intelligence Plane**: GPU-powered AI inference and structured artifact generation
- **Render Plane**: Client-side pixel rendering (never computed on network)

### Core Design Principles

1. **Separation of Concerns**: GPU handles cognition, CPU handles consensus
2. **Hash-Anchored Artifacts**: Network commits to RPACK hashes, not pixels
3. **Client-Side Rendering**: Lite-Vision produces structured data; clients render pixels
4. **Economic Accountability**: Every operation is metered, attested, and potentially disputed

## Crates

| Crate | Purpose |
|-------|---------|
| `lite-vision-truth` | BFT consensus, validator set, state management |
| `lite-vision-intelligence` | Job execution, kernel management, routing, receipts, disputes |
| `lite-vision-rpack` | Render Packet container format, Scene IR, assets |
| `lite-vision-storage` | CRDTs, partition management, memory model |
| `lite-vision-net` | P2P protocols, messaging |
| `lite-vision-sdk` | Client APIs for interacting with the network |
| `lite-vision-observability` | Logging, metrics, tracing, replay |

## Implemented Specifications

### Truth Plane (0100-series)
- LV-SPEC-0100: Truth Plane Architecture Overview
- LV-SPEC-0101: BFT Consensus Protocol
- LV-SPEC-0102: Validator Set, Staking, and Slashing
- LV-SPEC-0103: Block/Transaction Format
- LV-SPEC-0104: Cryptography and Key Management
- LV-SPEC-0105: Governance and Parameter Management
- LV-SPEC-0106: Storage Pruning and Archival
- LV-SPEC-0107: Cross-Partition/Cross-Plane Messaging

### Intelligence Plane (0200-series)
- LV-SPEC-0200: Intelligence Plane Architecture Overview
- LV-SPEC-0201: Operator Node Lifecycle
- LV-SPEC-0202: Kernel Interface and Sandbox
- LV-SPEC-0203: Task/Job Model and Execution
- LV-SPEC-0204: Routing and Sparse Activation (Thalamus)
- LV-SPEC-0205: Receipts, Metering, and Attestation
- LV-SPEC-0206: Verification and Redundancy Policy
- LV-SPEC-0207: Challenge/Dispute Protocol

### Render Plane (0300-series)
- LV-SPEC-0300: Render Offload Model and Guarantees
- LV-SPEC-0301: RPACK Container Format
- LV-SPEC-0302: Scene IR Schema (Semantic-First)
- LV-SPEC-0303: Asset Referencing, Caching, and Fetch Protocol

### Storage (0400-series)
- LV-SPEC-0400: Intelligence Memory Model
- LV-SPEC-0401: Regional Memory CRDTs and Merge Semantics
- LV-SPEC-0402: Partition Management

### Network/SDK (0500-series)
- LV-SPEC-0500: Network Protocols
- LV-SPEC-0501: SDK and Client APIs

## Building

```bash
cd lite-vision
cargo build --release
```

## Testing

Run all tests:

```bash
cargo test --workspace
```

Run tests for a specific crate:

```bash
cargo test -p lite-vision-intelligence
cargo test -p lite-vision-rpack
cargo test -p lite-vision-truth
```

## Key Concepts

### RPACK (Render Packet)
The canonical output artifact containing:
- Scene IR (semantic-first intermediate representation)
- Asset references (content-addressed)
- Metadata (determinism flags, constraints)

### Receipts
Attestations of job execution including:
- Input/output hashes
- Resource usage metering
- TEE attestations
- Deterministic seeds

### Dispute Protocol
Time-bond challenge mechanism allowing:
- Fraud proofs with challenger bonds
- Escalating verification levels
- Deterministic adjudication
- Slashing distribution

## License

DOSL - See License File
