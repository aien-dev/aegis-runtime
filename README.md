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
|   - Sub-millisecond TTFT  |     |     - Task Evaluation       |     |      - TTFT < 50ms          |
+---------------------------+     +--------------+--------------+     +-----------------------------+
                                                 |
                                  +--------------v--------------+
                                  |   Mojo 1.1 SIMD Bridge      |
                                  |   - Vector Cosine & Entropy |
                                  |   - SQLite WAL Persistence  |
                                  +-----------------------------+
```

## Core Subsystems

1. **Axum Control Plane**:
   Full duplex WebSockets, Server-Sent Events (SSE), and REST APIs bound to port 18096.

2. **Heartbeat Engine**:
   Autonomous polling loop waking every 30 seconds to inspect system state, dispatch background tasks, and sync memory.

3. **Local Modular MAX Inference**:
   Direct local loopback connection to Modular MAX on port 18006 (`atlas-lightning-omni`).

4. **Mojo 1.1 SIMD Kernels**:
   Vector cosine similarity, Shannon entropy, and linear projection using Mojo SIMD vector primitives via C-ABI FFI (`libloading`).

5. **Hardware TPM Vault**:
   Zero plaintext secrets on disk. In-memory secret resolution via `atlas-vault` with automatic stream redaction (`[REDACTED_BY_ATLAS_VAULT]`).

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

## Mathematical Verification

All SIMD mathematical kernels are formally checked against standard Python implementations:
```bash
python3 tests/verify_simd_math.py
cargo test --lib mojo_bridge
```

## License

Licensed under the **Sovereign Resource Commons License 1.0 (SRCL-1.0)** (Apache-2.0 WITH LLVM-exception). See [LICENSE](LICENSE) for terms.
