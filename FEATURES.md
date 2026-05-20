# xezim — Feature Support

This document inventories the SystemVerilog and UVM features supported by the
`xezim` simulator. Status legend:

- ✅ **Supported** — implemented and exercised by tests.
- ⚠️ **Partial** — parses and/or runs, but with documented limitations.
- ❌ **Not supported** — not implemented (or intentionally excluded).

SystemVerilog 2023 features are gated behind the `--sv2023` command-line flag.

---

## SystemVerilog 2017 (IEEE 1800-2017)

### Data types

| Feature | Status |
|---|---|
| `logic` / `bit` / `reg` | ✅ |
| Integer types (`byte`, `shortint`, `int`, `longint`, `integer`, `time`) | ✅ |
| `real` / `shortreal` / `realtime` | ✅ |
| Packed & unpacked arrays | ✅ |
| Dynamic arrays | ✅ |
| Queues | ✅ |
| Associative arrays | ✅ |
| Packed & unpacked structs | ✅ |
| Unions | ✅ |
| Enums | ✅ |
| Strings | ✅ |
| `typedef` | ✅ |
| User-defined nettypes | ✅ |

### Procedural constructs

| Feature | Status |
|---|---|
| `always` / `always_comb` / `always_ff` / `always_latch` | ✅ |
| `initial`, `final` | ✅ |
| Blocking & non-blocking assignment (with intra-assignment delay) | ✅ |
| `fork` / `join` / `join_any` / `join_none` | ✅ |
| `disable` | ✅ |
| `case` / `casez` / `casex` / `unique` / `priority` | ✅ |
| Loops (`for` / `while` / `do-while` / `repeat` / `forever` / `foreach`) | ✅ |
| Event control (`posedge` / `negedge` / `edge` / `@*`) | ✅ |

### Tasks & functions

| Feature | Status |
|---|---|
| `automatic` / `static` lifetime | ✅ |
| `ref` / `input` / `output` / `inout` arguments | ✅ |
| Default arguments | ✅ |
| Recursion | ✅ |

### Classes & OOP

| Feature | Status |
|---|---|
| Inheritance | ✅ |
| Virtual & pure-virtual methods | ✅ |
| Abstract classes | ✅ |
| Parameterized classes | ✅ |
| Static members | ✅ |
| `this` / `super` | ✅ |
| Constructors | ✅ |
| Polymorphic dispatch | ✅ |

### Randomization

| Feature | Status |
|---|---|
| `rand` / `randc` | ✅ |
| Constraint blocks | ✅ |
| `randomize()` | ✅ |
| `inside`, `dist`, `solve … before` | ✅ |
| Constraint solver | ⚠️ Handles ranges & equality; no implication / `unique` / `foreach` / soft-constraint solving |

### Verification

| Feature | Status |
|---|---|
| Immediate `assert` / `assume` / `cover` | ✅ |
| Covergroup / coverpoint / cross / bins | ✅ |
| Concurrent assertions (property / sequence) | ⚠️ Parsed; limited runtime evaluation |

### Other

| Feature | Status |
|---|---|
| Interfaces, modports, clocking blocks | ✅ |
| Generate (`if` / `for` / `case`), `genvar` | ✅ |
| Parameters, `localparam`, `defparam` | ✅ |
| Mailbox, semaphore, event | ✅ |
| DPI import/export (`--dpi-lib`) | ✅ |
| System tasks/functions (~95: `$display` family, `$monitor`, `$time`, `$cast`, `$bits`, file I/O, `$value$plusargs`, math, etc.) | ✅ |
| Preprocessor (`` `define ``, `` `ifdef ``, `` `include ``, macros) | ✅ |
| VCD + AITRACE / XTrace waveform dumping | ✅ |
| `$random` | ⚠️ Stub (returns 0) |

---

## SystemVerilog 2023 (IEEE 1800-2023)

Gated behind the `--sv2023` flag. Compliance suite: **12/13 pass**, 1 ignored.

| Feature | Status |
|---|---|
| Triple-quoted string literals | ✅ |
| `ref static` task args (with NBA writeback redirection) | ✅ |
| Logical operators in `` `ifdef `` conditions | ✅ |
| Array `.map` / `.find` / `.unique` / `.min` / `.max` with iterator-name binding | ✅ |
| `.find(item, idx)` explicit index iterator | ✅ |
| `type … extends` parametric class constraint | ✅ |
| `class :final` specifier | ✅ |
| Method `:initial` / `:extends` / `:final` specifiers | ✅ |
| `rand real` constraints | ✅ |
| `$timeunit` / `$timeprecision` / `$realtime` / `$stime` | ✅ |
| `$inferred_clock` / `$inferred_disable` / `$global_clock` / `$*_gclk` family | ✅ |
| `inside` with ± tolerance range | ✅ |
| Parameter associative arrays | ✅ |
| `disable fork` | ✅ |
| `unique` / `unique0` / `priority` case-violation detection | ✅ |
| Class-handle static dereference (e.g. `registry#(byte)::singleton.field`) | ✅ |
| `arr[idx].field` on packed-struct arrays | ✅ |
| Soft packed union (`union soft packed`) | ❌ Excluded — caused an OOM in elaboration; compliance test is `#[ignore]`d |

---

## UVM

UVM is **not built in** — it is compiled as ordinary SystemVerilog source (the
`uvm-1.2` package). The simulator provides thin shims for the most common
patterns; it does not implement the UVM verification infrastructure.

### Supported

| Feature | Status |
|---|---|
| `run_test()` bootstrap | ✅ |
| Type factory — `ClassName::type_id::create()` | ✅ |
| `UVM_ACTIVE` / `UVM_PASSIVE` constants | ✅ |
| `uvm_report_info` / `_warning` / `_error` / `_fatal` output | ✅ |
| `get_is_active()` | ✅ |
| Component class hierarchy, virtual methods, randomization | ✅ |

### Partial / stubbed

| Feature | Status |
|---|---|
| Phasing | ⚠️ Only `build_phase` → `connect_phase` → `run_phase` run; other ~12 phases absent |
| Objections (`raise_objection` / `drop_objection`) | ⚠️ No-ops |
| TLM | ⚠️ `analysis_port.write()` forwarded to scoreboards; `connect()` / `seq_item_port` are stubs |
| Sequencer / driver handshake (`get_next_item` / `item_done`) | ⚠️ No-ops |

### Not supported

| Feature | Status |
|---|---|
| `uvm_config_db` | ❌ |
| Factory *overrides* | ❌ |
| Sequences / sequencer execution | ❌ |
| Register Abstraction Layer (RAL) | ❌ |
| Callbacks | ❌ |
| Resource database | ❌ |
| Command-line processor | ❌ |
| Print / copy / compare | ❌ |

**Bottom line:** small, self-contained UVM tests that use OOP, randomization,
and `run_test` will run. Tests depending on the verification infrastructure
(phasing beyond 3 phases, objections, TLM, sequences, config DB, RAL) will
stub out or fail.
