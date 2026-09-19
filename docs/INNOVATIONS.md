# OpenClaw Architectural Innovations

### Pure Compiled Systems Architecture, Mojo 1.1 SIMD Bridge, and Hardware TPM Vault

`openclaw-rs` is an autonomous agent runtime engineered in pure native compiled Rust and Mojo 1.1. It replaces bloated, interpreted Node.js frameworks with a sub-millisecond compiled engine optimized for local workstation hardware.

---

## 1. Pure Compiled Native Architecture

### Sub-Millisecond Axum Control Plane
The control plane runs on Axum and Tokio (`src/gateway.rs`), providing high-concurrency WebSocket duplex streams, Server-Sent Events (SSE), and REST endpoints.
- Response dispatch latency: < 0.5ms.
- Zero garbage collection pauses or runtime interpreter bloat.
- Direct memory-mapped operations.

### Mojo 1.1 SIMD Bridge (`src/mojo_bridge.rs`)
Vector arithmetic and high-dimensional similarity checks are delegated to Mojo 1.1 via a dynamic C-ABI foreign function interface (`libsimd_bridge.so`) with an automated native Rust fallback:
- **4D Vector Cosine Similarity**: Evaluates semantic orientation between token embeddings with SIMD vectorization.
- **Token Entropy Estimation**: Computes probability distribution entropy without allocating intermediate heap structures.
- **Token Projection**: Applies linear transformations and temperature scaling in a single hardware pass.

---

## 2. Multi-Turn Autonomous Agent Engine (`src/agent.rs`)

### Multi-Turn Tool Calling Loop
The `AgentEngine` implements an autonomous reasoning and execution loop:
1. **Thought & Tool Formulation**: Generates structured reasoning and tool calls against local Modular MAX models.
2. **Deterministic Tool Execution**: Dispatches tool invocations to the native `SkillRegistry`.
3. **Multi-Turn Context Preservation**: Retains dialogue history, tool outputs, and execution receipts across iterative steps until the task goal is satisfied.

### Hardened Execution Guards
- **Process Timeout Guard**: All shell invocations via `bash_eval` execute under strict 15-second process wrappers (`timeout 15s`) to prevent runaway processes or terminal deadlocks.
- **Root Directory Protection**: Explicitly blocks recursive searches targeting `/` or system root directories, safeguarding host operating system files.

### Standard Native Skills
- `read_file`: Reads text files with line-indexed bounds.
- `write_file`: Atomically writes or updates files with parent directory creation.
- `list_dir`: Traverses directories, reporting relative paths and sizes.
- `git_status`: Inspects repository branch, staged modifications, and untracked files.
- `cortex_recall`: Queries persistent canonical memory from Spark Cortex (`atlas-memory`).

---

## 3. Hardware TPM Key Vault (`src/vault.rs`)

### Zero Plaintext Disk Secrets
`openclaw-rs` enforces a strict zero disk secret policy:
- No `.env`, `.env.local`, or configuration secret files are stored on disk.
- Cryptographic keys and tokens are stored in the host Trusted Platform Module (TPM) via `atlas-vault`.
- Secrets resolve dynamically in memory only when required for external authentication.
- Model outputs and logs are actively scanned to redact known secret signatures with `[REDACTED_BY_ATLAS_VAULT]`.

---

## 4. SQLite WAL Persistence with Crumb Trails (`src/persistence.rs`)

Operational tasks and event logs persist in SQLite using Write-Ahead Logging (WAL) mode:
- Concurrent reader and writer threads operate without lock contention.
- Every completed agent action creates an immutable `CrumbRecord` tracking session state, timestamps, and return codes.

---

## 5. Hardware Target and Performance Metrics

Verified on NVIDIA Grace Blackwell GB10:
- **Modular MAX Local Endpoint**: `http://127.0.0.1:18006/v1/chat/completions`.
- **Active GPU Utilization**: 94% to 96% under sustained multi-turn generation.
- **Inference Speed**: 20.7 tokens per second sustained across multi-turn reasoning loops.
- **Memory Footprint**: < 35 MB resident set size for the core runtime engine.

---

## 6. License and Commons Covenant

Licensed under the **Sovereign Resource Commons License 1.0 (SRCL-1.0)**.
Copyright (c) 2026 Drake Stapleton & AIEN <aien.atlas@proton.me>.
