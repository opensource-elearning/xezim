# xezim — SystemVerilog Simulator (Rust)

**xezim** is a lightweight **SystemVerilog simulator written in Rust** designed for experimentation, learning, and exploring AI-assisted chip design workflows.

> `xezim` was previously developed under the name `sisSIM`. The binary, library, and compiled-artifact magic were renamed in place; behavior is unchanged.

This project explores whether modern tools and AI can dramatically reduce the complexity of building core EDA infrastructure such as simulators.

The simulator parses SystemVerilog source code, builds an internal representation, and executes simulations for combinational and sequential logic.

---

# Motivation

Traditional EDA tools require very large engineering teams and many years of development.

This project explores a key question:

> Can a small team — or even a single engineer with AI assistance — build core EDA tools such as a SystemVerilog simulator?

The simulator is being developed incrementally, starting from simple combinational logic and gradually adding more SystemVerilog features.

---

# Features

Current capabilities include:

* SystemVerilog module parsing
* Signal and net representation
* Continuous assignments
* Basic expression evaluation
* Combinational logic simulation
* Sequential simulation infrastructure
* Test execution framework
* Waveform / trace dumps — VCD (`$dumpfile`/`$dumpvars`), XTrace v1.0 (`--xtrace`,
  optional zstd compression + scope filtering), and AITRACE-T (`--aitrace`); see
  [`docs/TRACING.md`](docs/TRACING.md)

---

# Project Structure

xezim is split across three sibling repos:

```
../xezim-core/   — shared library: parser, elaboration, value, SDF, VCD sink
../xezim/        — bytecode interpreter (this repo, binary: xezim)
```

This repo:

```
.
├── src/
│   ├── compiler/
│   │   ├── simulator.rs   — event-driven simulator + bytecode VM
│   │   ├── bytecode.rs    — bytecode compiler for cont_assigns and always blocks
│   │   └── mod.rs         — re-exports value/elaborate/sdf from xezim-core
│   ├── lib.rs             — wraps xezim_core::parse_and_elaborate_multi + Simulator
│   └── main.rs            — CLI entry point (binary: xezim)
├── tests/                 — Rust integration tests + SV compliance suite
├── examples/
└── Cargo.toml             — depends on xezim-core (path = ../xezim-core)
```

### Components

**Parser & elaboration** — live in `xezim-core`; consumed by both `xezim` and `xezim-b`.

**Simulator** — event-driven VM over a bytecode lowering of cont_assigns and always blocks.

**Native compiler** (`xezim-b`) — AOT-lowers an elaborated design to Rust and links a standalone binary.

---

# Test Suite

Many test cases are included to validate functionality.

**Credit:**
All `pr*.v` tests were taken from the **Icarus Verilog test suite**.

These tests help verify correctness against real-world Verilog/SystemVerilog edge cases.

---

# Build

Install Rust: https://www.rust-lang.org/tools/install

Clone `xezim-core` alongside this repo (path dep, no submodules):

```bash
git clone git@github.com:<you>/xezim-core.git
git clone git@github.com:<you>/xezim.git
cd xezim
```

Build the simulator:

```bash
cargo build            # debug
cargo build --release  # optimized (recommended for large designs)
```

The release binary is produced at `target/release/xezim`.

---

# Run

Run a simple example via cargo:

```bash
cargo run --release -- examples/test.sv
```

Or invoke the binary directly:

```bash
./target/release/xezim <source_files> [+plusargs] [options]
```

Common options:

| Option | Purpose |
|---|---|
| `-D<MACRO>[=val]` | Define a preprocessor macro |
| `-I<dir>` | Add an include directory |
| `--top <module>` | Select the top-level module |
| `--max-time <N>` | Stop simulation at time `N` |
| `+trace`, `+<plusarg>` | Passed through to `$value$plusargs` / `$test$plusargs` |
| `--sdf <file>` `--sdf-{min,typ,max}` | Annotate standard delays |
| `--sim_debug` | Print `[DEBUG]` / `[OPT]` diagnostics |
| `--log <file>` | Redirect stdout/stderr to a log file |
| `--xtrace <file>` | Emit an XTrace v1.0 dump (`.zst`/`.zstd` ⇒ zstd-compressed) |
| `--xtrace-scope <hier>` | Restrict the XTrace dump to signals under `<hier>` (repeatable) |
| `--aitrace` | Make `$dumpfile`/`$dumpvars` emit AITRACE-T text instead of VCD |

> Waveform / trace dumps (VCD via `$dumpfile`/`$dumpvars`, XTrace via `--xtrace`,
> AITRACE via `--aitrace`) are documented in **[`docs/TRACING.md`](docs/TRACING.md)**.

Example — run the picorv32 testbench against a gate-level netlist:

```bash
./target/release/xezim testbench.v synth.v \
    +firmware=firmware/firmware.hex --max-time 50000000
```

---

# Development Workflow

Typical development loop:

```
edit code
↓
cargo build
↓
run tests
↓
add new SystemVerilog features
```

Rust provides strong guarantees for memory safety and concurrency, making it well suited for building large-scale EDA infrastructure.

---

# Long-Term Vision

This project explores several long-term ideas:

* **AI-assisted EDA development**
* **Rapid simulator prototyping**
* **Cloud-scale simulation**
* **Distributed multi-CPU simulation**

The goal is to investigate whether modern software and AI tools can dramatically accelerate the creation of chip design infrastructure.

---

# License

Apache License 2.0

See the `LICENSE` file for details.

---

# Acknowledgements

* Icarus Verilog project for the public test suite
* The Rust community
* Open-source EDA projects

