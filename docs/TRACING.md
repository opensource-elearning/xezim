# Enabling waveform / trace dumps in xezim

xezim can emit three kinds of trace during `--simulate`:

| Format | How it's turned on | Default file | Notes |
| --- | --- | --- | --- |
| **VCD** | `$dumpfile` / `$dumpvars` in the SV testbench | `dump.vcd` | Standard Value Change Dump; opens in GTKWave, Surfer, etc. |
| **XTrace v1.0** | `--xtrace <file>` on the command line | *(none — flag required)* | Text "minimal" profile: dictionary + per-cycle signal deltas. Optional zstd compression and scope filtering. Runs independently of / alongside VCD. |
| **AITRACE-T** | `--aitrace` flag **+** `$dumpfile`/`$dumpvars` in the testbench | `dump.aitrace` | Same trigger as VCD, but `--aitrace` makes `$dumpvars` emit AITRACE-T text instead of VCD. |

All three can be sped up with a background writer thread: pass `--threads 2` (or more) — VCD/AITRACE/XTrace formatting and I/O then run off the simulation thread.

---

## VCD

VCD is driven entirely from the SystemVerilog testbench, just like other simulators.

```systemverilog
module tb;
  // ... DUT, clocks, stimulus ...
  initial begin
    $dumpfile("waves.vcd");   // optional; defaults to dump.vcd
    $dumpvars;                // dump everything
    // ... or restrict:  $dumpvars(0, tb.dut.cpu_core);
  end
endmodule
```

Run it:

```bash
xezim --simulate -s tb --max-time 100000 tb.sv dut.sv
# -> waves.vcd
```

`$dumpvars` argument forms:

* `$dumpvars;` — dump **all** signals.
* `$dumpvars(0);` — same (the depth argument is currently ignored — always "all children").
* `$dumpvars(0, <scope_or_signal> [, ...]);` — restrict to signals whose hierarchical
  name equals one of the arguments or sits underneath it (`<scope>.` prefix).

> **Gotcha:** signals declared in the *top* module are stored without a module prefix
> (e.g. `clk`, not `tb.clk`), so `$dumpvars(0, tb)` will match nothing. Use a **sub-module**
> path (`$dumpvars(0, tb.dut)`) or no scope argument at all.

`$dumpoff` / `$dumpon` suspend and resume value-change recording.

---

## XTrace

XTrace is enabled purely from the command line — no testbench changes needed.

```bash
xezim --simulate -s tb --max-time 100000 --xtrace run.xt tb.sv dut.sv
# -> run.xt   (XTrace v1.0, @format text, @profile minimal)
```

### Compression

If the filename ends in `.zst` or `.zstd`, the byte stream is zstd-compressed
(the file is a single zstd frame — decompress with `zstd -d run.xt.zst`, or any
zstd library):

```bash
xezim --simulate -s tb --xtrace run.xt.zst tb.sv dut.sv
```

Trace text is highly repetitive, so this typically shrinks the file ~8–15×.

### Scope filtering

By default XTrace dumps **every** named signal — on large designs that's a huge file.
Restrict it to one or more hierarchical scopes with `--xtrace-scope` (repeatable):

```bash
xezim --simulate -s tb \
  --xtrace cpu.xt.zst \
  --xtrace-scope x_soc.x_cpu_sub_system.x_cpu_top \
  --xtrace-scope x_soc.x_dma \
  tb.sv dut.sv
```

A signal is included if its hierarchical name equals a scope exactly **or** starts with
`<scope>.`. This both shrinks the file and speeds up the dump (fewer change comparisons,
far less I/O). Combine it with `.zst` for the smallest output.

> Use the **stored** signal-name prefix here (top module stripped, e.g. `x_soc.x_apb.x_uart`),
> not the elaboration path — you can see the exact names in the `M,` / `S,` records of an
> unfiltered `*.xt` dict section.

### What's in the file

```
@xtrace 1.0
@format text
@producer xezim <version>
@timescale 1ns
@design <top>
@profile minimal

@section dict
M,m0,/tb
M,m1,/tb/x_soc
...
S,s0,m0,clk,bit
S,s1,m0,cnt,u8
...

@section trace
T,+0
N,full,s0=0x0,s1=0x0,...        # t=0 snapshot
T,+5
P,s0=0x1,s1=0x1                 # packed deltas (>1 change)
T,+5
D,s0,0x0                        # single delta
...
@section end
```

`T,+N` advances simulation time by `N`; `P,`/`D,` records carry signal-id → value changes
(`0xHEX`, or `0b…` per-bit when X/Z bits are present). Only changed signals are emitted.

---

## AITRACE

Same trigger as VCD (`$dumpfile`/`$dumpvars` in the testbench), but add `--aitrace` and
`$dumpvars` emits AITRACE-T text instead of a VCD:

```bash
xezim --simulate --aitrace -s tb --max-time 100000 tb.sv dut.sv
# -> dump.aitrace   (or whatever $dumpfile() named it)
```

---

## Quick reference

```text
--xtrace <file>          Emit an XTrace v1.0 dump (.zst/.zstd suffix => zstd-compressed)
--xtrace-scope <hier>    Restrict the XTrace dump to signals under <hier> (repeatable)
--aitrace                Make $dumpfile/$dumpvars emit AITRACE-T instead of VCD
--threads <n>            n>=2: offload VCD/AITRACE/XTrace writing to a background thread
```

VCD has no command-line switch — it is controlled by `$dumpfile`/`$dumpvars` in the design.
