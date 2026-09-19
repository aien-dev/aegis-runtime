# openclaw-rs

> Ultra-high-performance sovereign agent runtime in native Rust, Mojo 1.1 SIMD kernels, and Modular MAX local inference on NVIDIA Grace Blackwell GB10.

`openclaw-rs` is the modern, compiled reimagining of OpenClaw. Where legacy implementations relied on heavy Node.js runtimes and incurred massive recurring cloud LLM bills, `openclaw-rs` delivers sub-millisecond execution, autonomous proactive heartbeats, and zero cloud API fees through local hardware acceleration.

## Architecture

```
                                  +-----------------------------+
                                  |   Hardware TPM Key Vault    |
                                  |   (Zero Disk Secrets)       |
                                  +--------------+--------------+
                                                 |
+---------------------------+     +--------------v--------------+     +-----------------------------+
|   Axum Control Plane      |     |     Heartbeat Engine        |     |      Modular MAX            |
|   - Port 18096            | <-> |     - 60s Autonomous Tick   | <-> |      - Port 18006           |
|   - WebSockets & SSE      |     |     - Proactive Colleague   |     |      - GB10 Local Weights   |
|   - Sub-millisecond TTFT  |     |     - State Machine         |     |      - TTFT < 50ms          |
+---------------------------+     +--------------+--------------+     +-----------------------------+
                                                 |
                                  +--------------v--------------+
                                  |   Mojo 1.1 SIMD Bridge      |
                                  |   - Vector Cosine & Norms   |
                                  |   - SQLite WAL Persistence  |
                                  +-----------------------------+
```

## Key Capabilities

1. **Sub-Millisecond Axum Control Plane**:
   Built on Axum and Tokio with full duplex WebSockets, Server-Sent Events (SSE), and REST APIs on port 18096.

2. **Proactive Heartbeat Loop**:
   Acts as an autonomous digital colleague rather than a passive chatbot. Wakes on a 60-second period to inspect state, trigger automated workflows, and synchronize memory.

3. **Local Modular MAX Inference**:
   Direct integration with Modular MAX on port 18006. Eliminates cloud LLM bills and slashes Time-To-First-Token (TTFT) to under 50ms on Grace Blackwell hardware.

4. **Mojo 1.1 SIMD Kernels**:
   Leverages Mojo 1.1 vector operations with zero-copy C-ABI FFI into Rust for high-throughput embedding calculations and cosine similarity.

5. **Hardware TPM Vault**:
   Zero plaintext secrets on disk. In-memory resolution via `atlas-vault` with automated redaction (`[REDACTED_BY_ATLAS_VAULT]`).

6. **SQLite Write-Ahead-Logging (WAL)**:
   High concurrency, durable local storage with embedded SQLite in WAL mode.

## Installation and Build

### Prerequisites
- Rust 1.80+ (`cargo`)
- Modular MAX & Mojo 1.1
- Linux aarch64 or x86_64

### Compilation
```bash
cargo build --release
```

### Running the Gateway
```bash
./target/release/openclaw serve --bind 0.0.0.0:18096
```

### Command Line Usage
```bash
# Execute a single autonomous tick
openclaw tick

# Query local MAX directly
openclaw ask "Analyze system health and pending tasks"

# Check engine and MAX status
openclaw status
```

## Licensing

`openclaw-rs` is licensed under the **Sovereign Reciprocal Commons License (SRCL-1.0)** based on Apache 2.0 with LLVM Exceptions and Reciprocal AI Weight Release.

- **Startups & Individuals**: Free commercial and noncommercial use for entities with annual revenue or capital under $25,000,000 USD.
- **Reciprocal Source**: Modifications running SaaS or cloud services must be open-sourced under these same terms.
- **Reciprocal AI Weight Release**: Closed foundation model labs training on this repository must release the resulting model weights openly under the *Jacobsen v. Katzer* doctrine.
- **Retroactive Inception**: Applies retroactively to all prior commits and distributions *ab initio*.
