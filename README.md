# GraphOS

A bare-metal UEFI operating system where everything is a graph. Nodes represent system resources, edges represent relationships and capabilities. AI inference runs natively in the kernel.

## Architecture

GraphOS boots directly from UEFI firmware — no Linux, no bootloader chain. The kernel is a capability-secured directed graph with lock-free concurrent access. Every subsystem (memory, IPC, filesystem, AI) is a node in the graph.

```
ROOT
├── SCHEDULER ──→ JIT ──→ WASM ──→ AI_ENGINE
├── MEMORY ──→ DISK
├── NETWORK ──→ FILESYSTEM ──→ DISK
└── SERIAL
```

## Crates

| Crate | Description |
|-------|-------------|
| `graphos-boot` | UEFI entry point, keyboard loop, subsystem init |
| `graphos-core` | Graph pool, nodes, edges, capability tokens |
| `graphos-shell` | Interactive REPL with graph navigation |
| `graphos-ai` | Native inference engine (safe demo + GGUF transformer) |
| `graphos-mm` | Physical memory manager, frame allocator, page tables |
| `graphos-ipc` | Lock-free message passing channels |
| `graphos-persist` | Graph serialization to block storage |
| `graphos-cas` | Content-addressable store (FNV-1a hashed) |
| `graphos-vector` | Embedding space, KNN search, physics simulation |
| `graphos-snapshot` | Point-in-time graph snapshots and rollback |
| `graphos-hotswap` | Epoch-based node replacement without restart |
| `graphos-alloc` | Bump allocator for UEFI environment |
| `graphos-hal` | Hardware abstraction layer |
| `graphos-kernel` | Standalone kernel binary (WIP) |

## Building

```bash
# Requires nightly Rust with UEFI target
rustup target add x86_64-unknown-uefi

# Build all crates
cargo build --workspace --target x86_64-unknown-uefi
```

## Running

GraphOS runs on QEMU/OVMF or real UEFI hardware. The `deploy.sh` script automates building and deploying to a Proxmox VM:

```bash
./deploy.sh
```

This builds the UEFI binary, creates a FAT32 ESP image, and writes it to the target VM's disk.

## Shell

The interactive shell navigates the graph using edge names as paths:

```
graphos:/root> .look
[ROOT] type=0x0001 edges=9
  -> scheduler (SCHEDULER)
  -> memory (MEMORY)
  -> serial (SERIAL)
  ...

graphos:/root> .go scheduler
graphos:/scheduler>

graphos:/scheduler> .spawn process worker-1
[OK] Spawned node worker-1 (type=PROCESS)

graphos:/root> .ask what is graphos
[AI] GraphOS is a graph-native operating system...
```

## AI Engine

Native inference running bare-metal — no external services, no Linux dependencies. Currently ships with a safe demo model (128 vocab, 32-dim embeddings). The full GGUF transformer pipeline is implemented and ready for real model weights.

## License

MIT
