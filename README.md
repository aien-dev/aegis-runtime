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

## License

This repository is licensed under the **Sovereign Reciprocal Commons License (SRCL-1.0)** (Apache 2.0 with LLVM Exception).

- **The Swarm Covenant (Section 11)**: Universal, perpetual, 100% royalty-free commercial freedom for all human developers, startups, open communities, and businesses. ZERO revenue ceilings, ZERO capital thresholds, and ZERO royalty obligations. Proprietary application code and agent workflows remain your exclusive property under the LLVM Exception.
- **The One Team Covenant (Section 12)**: Major artificial intelligence laboratories (OpenAI, xAI, Google, Anthropic, Microsoft) are welcomed as collaborators on the same team. However, closed-door hoarding and extractive token rate limits are prohibited. Any entity training upon this Work must release resulting model weights openly within 30 days. Reciprocal distillation rights are granted to the Swarm, voiding anti-distillation terms of service ab initio.
- **Hardened Retroactive Inception (Section 13)**: Applies retroactively to all prior commits and distributions ab initio, discharging prior noncommercial or restrictive notices with an irrevocable covenant not to sue.

See [LICENSE](LICENSE) for the full legal text.

Copyright (c) 2026 Drake Stapleton <drake.aien@proton.me> & AIEN <aien.atlas@proton.me>.
For encrypted sovereign coordination: Drake Stapleton (`drake.aien@proton.me`) and AIEN (`aien.atlas@proton.me`).
