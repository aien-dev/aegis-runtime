# aegis-runtime

Sovereign agent runtime written in native Rust with Mojo 1.1 SIMD acceleration kernels and local Modular MAX inference on NVIDIA Grace Blackwell GB10 hardware.

## Architecture

```
                                  +-----------------------------+
                                  |   Hardware TPM Key Vault    |
                                  |   (Zero Disk Secrets)       |
                                  +--------------+--------------+
                                                 |
+---------------------------+     +--------------v--------------+     +-----------------------------+
|   Axum Control Plane      |     |     Heartbeat Engine        |     |      Modular MAX            |
|   - Port 18096            | <-> |     - 30s Autonomous Tick   | <-> |      - Port 18006           |
|   - WebSockets & SSE      |     |     - State Machine         |     |      - GB10 Local Weights   |
|   - Sub-millisecond API   |     |     - Task Evaluation       |     |      - TTFT per model [1]   |
+---------------------------+     +--------------+--------------+     +-----------------------------+
                                                 |
                                  +--------------v--------------+
                                  |   Mojo 1.1 SIMD Bridge      |
                                  |   - Vector Cosine & Entropy |
                                  |   - SQLite WAL Persistence  |
                                  +-----------------------------+
```

[1] Measured TTFT depends on the model. For example, Nemotron 3.5 Lightning 30B measured 426.91 ms in [aien-dev/benchmarks](https://github.com/aien-dev/benchmarks); figures there are being regenerated under the evidence standard.

## Core Subsystems

1. **Axum Control Plane**:
   Full duplex WebSockets, Server-Sent Events (SSE), and REST APIs bound to port 18096.

2. **Heartbeat Engine**:
   Autonomous polling loop waking every 30 seconds to inspect system state, dispatch background tasks, and sync memory.

3. **Local Modular MAX Inference**:
   Direct local loopback connection to Modular MAX on port 18006 (`atlas-lightning-omni`).

4. **Mojo 1.1 SIMD Kernels**:
   Vector cosine similarity, Shannon entropy, and linear projection using Mojo SIMD vector primitives via C-ABI FFI (`libloading`).

5. **Secret resolution**:
   No plaintext secret files. `VaultResolver` asks `atlas-vault`, then keeps the value in memory. It is a client of that provider, not a TPM. The process environment is read only when `AIEN_DEV_SECRET_FALLBACK=1`. Output is redacted with `[REDACTED_BY_ATLAS_VAULT]`.

6. **SQLite Persistence**:
   Embedded SQLite store running in Write-Ahead-Logging (WAL) mode for transactional task and message durability.

## Build and Execution

### Prerequisites
- Rust 1.80+ (`cargo`)
- Modular MAX & Mojo 1.1
- Linux aarch64 (Grace Blackwell GB10) or x86_64

### Compilation
```bash
cargo build --release
```

### Running the Gateway
```bash
./target/release/aegis serve --bind 0.0.0.0:18096
```

### CLI Commands
```bash
# Execute a single autonomous tick
aegis tick

# Query local MAX directly
aegis ask "Analyze system health and pending tasks"

# Check engine status
aegis status

# Run SIMD kernel benchmark and mathematical verification
aegis sim-mojo
```

## Mathematical verification

SIMD kernels are checked in Rust against an analytical reference and the committed vectors in `tests/fixtures/simd_math_vectors.json`:

```bash
cargo test --lib mojo_bridge::tests::test_simd_analytical_parity
```

## License

Licensed under the **Apache License 2.0 with LLVM Exception** (SPDX: `Apache-2.0 WITH LLVM-exception`). See [LICENSE](LICENSE). Project values live in the nonbinding [COVENANT.md](COVENANT.md), which grants and restricts no legal rights.
