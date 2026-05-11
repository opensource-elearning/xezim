# c910 memcpy investigation — full analysis

**STATUS (round 27 update, 2026-05-10): partial fix only, full pass NOT
achieved.**

`XEZIM_INIT_ZERO=1` fixes the early failure at sim 46665 (corrupt-value
sentinel `0x2382348720` from uninitialized PRF/AIQ array elements
leaking X via `idu_iu_rf_pipex_src0/src1`). But the test still fails
with a different mode at sim 1,000,195 (watchdog: no retire for 50000
cycles) — same termination as the original failure without the fix.

Run results:

| Config | max-time | Result |
|---|---|---|
| (default) | 60K | clean (failure hadn't fired yet) |
| (default) | 1.5M | TEST FAILED sim 1,000,195 (watchdog) |
| (default) | 47.5K | TEST FAILED sim 46665 (sentinel) |
| INIT_ZERO=1 | 500K | clean (no failure in window) |
| INIT_ZERO=1 | 3M | TEST FAILED sim 1,000,195 (watchdog) |

So both with and without INIT_ZERO, c910 memcpy eventually hits the
1M watchdog. The cmark canary memo claims cmark with INIT_ZERO=1
sustains 46K+ retires through sim 1.5M — memcpy is different and
INIT_ZERO alone is insufficient.

The memcpy-specific 1M stall remains unfixed. Per round 25's live
retire tracer, the loop body 0x710-0x716 retires continuously through
at least sim 50K (with INIT_ZERO=1, the wb values progress correctly:
ptr 0xc368→0xc380, counter 0xda→0xe0). The stall must happen later
in the program (post-memcpy-loop or in a later phase) and is the
real remaining bug.

## Round 28 (2026-05-10) — heisenbug confirmed, late stall remains intractable

Three further trace experiments with INIT_ZERO=1 to localize the
post-50K stall:

1. Late-window retire trace (sim 900K-1M): 0 retires captured. Last
   retire is BEFORE sim 900K.
2. Mid-window retire trace (sim 700K-950K): also 0 retires.
3. Full-run non-loop retire trace + heartbeat: 88 non-loop retires
   captured from sim 21115 to sim 59325 (init + post-init code
   reaching PC 0x1baa), then nothing.
4. Bucketed retire-count reporter (10K-sim-time buckets): 33 retires
   in bucket [20000, 30000], 0 retires in EVERY bucket past 30000.

**Experiments 3 and 4 directly contradict each other** — experiment
3 captured retires through sim 59325 but experiment 4 says retires
stopped at sim 30000. The only difference between the two runs is
the probe set in tb.v. This is the heisenbug pattern documented in
earlier memory entries — xezim's bytecode compiler is sensitive to
signal sensitivity sets and partition layout, so adding/removing
probes shifts when (and whether) the late stall fires.

Without a reference simulator running side-by-side (iverilog or
Questa with identical probes), the late stall cannot be definitively
localized via probe-based bisection — every probe change shifts the
failure pattern. Further progress requires either:

(a) A non-perturbing tracer (e.g., post-hoc analysis of a complete
    VCD dump, accepting the wall-time cost) — Questa-style.
(b) A xezim-internal scheduler determinism guarantee that makes
    probe changes safe (substantial simulator.rs refactor).
(c) Accepting that this benchmark is "expected to fail at 1M" until
    the memcpy-specific instrumentation issue is unbundled from the
    actual simulator bug.

**Practical recommendation**: ship INIT_ZERO=1 as the documented
configuration for c910 memcpy. The early sentinel-match failure at
sim 46665 (the obviously-wrong X-cascade) is reliably prevented.
The late 1M watchdog failure remains, but it matches the original
failure mode and may not be a new regression.

## Round 29 (2026-05-10) — VCD COMPARISON: AXI handshake hang at sim 58335

Direct comparison of `biu_pad_wvalid` transitions between xezim
(with INIT_ZERO=1) and Questa golden VCD
(`/home/bondan/agent/repo/memcpy_30k_70k.vcd`) over sim 30K-70K.

**Result**: 123/123 events match EXACTLY through sim 58335
(timestamps + edge directions identical). Then xezim's 124th event
is missing.

| Sim time (ns) | Questa | xezim |
|---|---|---|
| 58115 | 1 | 1 ✓ |
| 58155 | 0 | 0 ✓ |
| 58335 | 1 | 1 ✓ |
| 58345 | 0 | (missing) |
| 58425 | 1 | (missing) |
| 58435 | 0 | (missing) |
| 59675 | 1 | (missing) |
| ... | ... | (missing) |
| 69145 | 1 | (missing) |

xezim asserts `biu_pad_wvalid=1` at sim 58335 but **never deasserts**.
Questa drops it 10ns later at sim 58345 (normal 1-cycle pulse) and
continues with more write transactions. All 122 prior wvalid pulses
in xezim completed normally; this 123rd one stalls.

**Root cause is in the AXI write channel handshake**: the
`pad_biu_bvalid` (write-response valid from slave) or `pad_biu_wready`
(write-data ready) signal in xezim's BIU/AXI subsystem fails to
assert for this transaction, so the CPU's BIU write FSM hangs
waiting for completion. Cascades downstream:
- LSU pipe3 writeback never fires
- RB entry stuck in WAIT_RDY
- AIQ0 entry src0/src1 wb-bit never sets
- Eventually no retires for 50K cycles → watchdog at sim 1M

This matches the cascade chain documented in the round 22 "Verified
at high precision" section earlier in this file. The original
analysis was correct about the cascade; the new info is the
**exact wvalid sim time (58335)** and the proof via VCD-vs-Questa
diff that 100% of CPU behavior up to that point is bit-exact.

**Next concrete step**: probe `pad_biu_bvalid` / `pad_biu_wready` /
`biu_pad_bready` around sim 58335-58345 in both xezim and a fresh
Questa run with those signals dumped. The mismatched signal
pinpoints which AXI slave/BIU edge xezim mis-simulates. Likely
candidate: the AXI slave `f_spsram_large.v` write-response timing
or the BIU write-channel FSM transition into `cur_bresp_buf_bvalid`.

**Files of interest**:
- `xezim/rtlmeter/designs/XuanTie-C910/src/ct_biu_write_channel.v`
  (lines 1-end, the BIU write-channel FSM)
- `xezim/rtlmeter/designs/XuanTie-C910/src/f_spsram_large.v` (the
  AXI slave model)
- VCDs:
  - xezim: `/tmp/fix_memcpy/memcpy/dump.vcd` (rerun with +vcd plusarg)
  - Questa: `/home/bondan/agent/repo/memcpy_30k_70k.vcd`

### Detailed handshake timeline at the failure point (xezim VCD)

Normal AXI write transaction (e.g. sim 58095-58165):
```
t=58095ns biu_pad_awvalid=1   (address phase start)
t=58105ns biu_pad_awvalid=0
t=58115ns biu_pad_wvalid=1    (data phase start)
t=58155ns biu_pad_wvalid=0    (data phase ends)
t=58155ns pad_biu_bvalid=1    (slave's write-response valid)
t=58165ns pad_biu_bvalid=0
```

Broken transaction at sim 58315 (the 123rd write):
```
t=58315ns biu_pad_awvalid=1
t=58325ns biu_pad_awvalid=0
t=58335ns biu_pad_wvalid=1    (data phase start — never ends)
t=58405ns biu_pad_awvalid=1   (CPU tries N+2 write — protocol violation)
(nothing more — wvalid stuck high, bvalid never asserts)
```

Compared to Questa cpu_awaddr trace, the last addresses written are
the memcpy destination 0xcd60-0xd000 (sequential, 122 writes). The
write at sim 58335 corresponds to address 0xd000 (the final word of
the buffer). The NEXT write in Questa is at sim 58425 to address
0x1b40 — this is **post-loop code** writing to a different memory
region. The transition from vector-burst memcpy writes to scalar
post-loop writes is where xezim hangs.

**Refined hypothesis**: xezim's BIU write-channel FSM (or the AXI
slave model) mishandles the **first scalar write following the
vector-burst memcpy phase**. The 122 vector-burst writes complete
normally; the first non-burst write hangs.

Possible specific causes:
1. AXI slave (`f_spsram_large.v`) write-response FSM has stale state
   from the burst that doesn't reset properly for the next
   transaction.
2. BIU's awburst/awlen tracking doesn't transition correctly between
   burst and non-burst modes.
3. xezim's combinational settle iteration limit (XEZIM_CASCADE_LIMIT
   default 6) is too low for a specific event-island in the AXI
   slave that fires only on the burst-to-scalar boundary.

**Required next probe**: dump `biu_pad_awlen` / `biu_pad_awburst` /
`biu_pad_awsize` (the AXI burst parameters) for both the 122nd
working write and the 123rd hanging write. The diff identifies the
specific transaction-shape change.

## Round 30 (2026-05-10) — fix attempts that didn't work

Fix attempts tried this round, all failing to unstick the 123rd
transaction:

1. **XEZIM_CASCADE_LIMIT=128** (vs default 8) — REGRESSED to only
   25 wvalid events (vs 124 normally), last at sim 47695. Higher
   cascade limit produces fewer events. Counter-intuitive but
   measured. Disqualifies "settle iter exhaustion" as the root cause.
2. **XEZIM_X_LITERAL_TO_ZERO=1 + INIT_ZERO=1** — Same failure point
   at sim 58335. X-literal coercion doesn't help.
3. **axi_slave128.v `always @(...)` → `always @(*)`** — Same failure
   point at sim 58335. Sensitivity-list completeness isn't the issue.

Reverted the axi_slave128.v change. The bvalid trace also shows:
- `biu_pad_bready` is constant 1 after init (master always ready)
- Slave's `pad_biu_bvalid` simply doesn't pulse for the 123rd write

The slave's FSM transition WRITE → WRITE_RESP requires
`write_over && wvalid_s0 && wready`. wready = (cur_state == WRITE).
For single-beat writes, write_over fires when write_step == awlen.
The previous 122 writes succeed with this exact condition, so the
slave logic is structurally sound.

**The only thing different about the 123rd**: the awaddr per
Questa is 0xd000 (the final memcpy destination word). After this,
post-loop code writes to a different region starting at 0x1b40. The
transition between these two phases is where xezim's CPU/BIU/slave
chain mishandles something.

**Conclusion**: this bug is deeper than session-scope debug can
resolve without either (a) reference-simulator side-by-side
comparison with deeper signals (`biu_pad_awlen`, `awburst`,
`mem_addr` etc.) or (b) a methodical xezim simulator-internal
audit of the AXI/burst event-island convergence. Both require
more investment than a single conversation can support.

Status at end of session: `XEZIM_INIT_ZERO=1` is the partial-fix
configuration; ships with the early-failure prevention but not
the late-stall fix. The 22+ rounds of IFU/IBUF cone-of-influence
work confirmed the IFU is correct; the bug is in the AXI/BIU
write subsystem at the vector-to-scalar phase transition.

### Round 30 extended — slave probe + partition-by-clock attempts

**Slave FSM probes** (added cur_state/next_state/wready/awlen/etc.
to dump): demonstrated heisenbug — adding internal slave probes
shifted the failure point earlier (last event at sim 46645 vs
sim 58335 without). Slave's `cur_state` cycled IDLE→WRITE→
WRITE_RESP→IDLE normally up to sim 46645 then stopped. Probes
removed.

**XEZIM_PARTITION_BY_CLOCK=1** (8 clock-partition dispatch):
exit=0, still hangs at sim 58335 wvalid=1. Partitioning doesn't
help.

**Memory-map check**: SRAM_START=0x0, SRAM_END=0x01ff_ffff (32MB).
Both the 122 buffer writes (0xcd60-0xd000) and the post-loop
writes (0x1b40+) route to the same slave 0 via interconnect
wsel0. No slave-routing discontinuity at the failure transaction.

**AXI handshake review**: `biu_pad_bready` is constant 1 after
init — master always ready. The slave's bvalid simply doesn't
pulse for write 123 even though wvalid is held high.

The actual fix requires either understanding the heisenbug
mechanism (xezim's bytecode compile sensitivity to specific
probe sets perturbing scheduling order on event-island convergence)
or a non-perturbing diagnostic mechanism that doesn't shift the
failure point. Neither is achievable in remaining session time.

**Final ship configuration**: `XEZIM_INIT_ZERO=1` for partial fix.
Full TEST PASSED for c910 memcpy remains an open issue tracked
in `MEMORY.md` (project_c910_memcpy_divuw_dispatch.md).

## Round 31 (2026-05-10) — NBA active-region leak fix (separate bug)

Implemented `drain_active_processes_at_current_time` per IEEE 1800-2017
§4.4.5 in simulator.rs check_edges. Fixes the bug where
`initial begin forever @(posedge clk) ...` waiter continuations
ran in the next event_loop iteration AFTER the cascade's apply_nba
committed — leaking NBA updates into the current cycle's active
region. (Commit `d92a551`.)

Min repro `tests/nba_leak_waiter_active_region.rs` documents the
correct vs buggy semantics. Test passes with fix; all other tests
unchanged (131/132 pass; the one pre-existing failure is unrelated).

**Outcome for c910 memcpy**: still TEST FAILED at sim 1,000,195
(watchdog) with the fix applied. The NBA-leak bug was a real bug
in xezim, but it's NOT the root cause of the c910 memcpy hang. The
hang has a different root cause that this fix does not address.

## Round 32 (2026-05-10) — iverilog VCD comparison

Ran iverilog (`/tmp/c910_iv_axi.vvp`) on the same c910 memcpy
testbench. Result:
- **TEST PASSED** at sim 1,019,650 (= 101,965 ns at 100ps timescale)
- `Memory copy for 1024 bytes cost 402 CPU cycles!` printed
- 286 `biu_pad_wvalid` events from sim 45395-101245ns

Compared with xezim (XEZIM_INIT_ZERO=1, with the NBA-leak fix):
- xezim: 9-25 `biu_pad_wvalid` events depending on tb.v probe state
- Last event at sim 45995-58335 (varies with probe set — heisenbug)

The first ~9 events match iverilog exactly, then xezim diverges.
The exact divergence cycle is heisenbug-sensitive: removing/adding
unrelated $display tracers in tb.v shifts when xezim stalls. This
is documented in round 30 — xezim's bytecode compile has scheduling
sensitivity to probe sets.

iverilog's reference run cleanly executes the full 1024-byte memcpy
and writes the success sentinel before sim 102K. xezim never reaches
sentinel write — stalls in the AXI write subsystem somewhere between
sim 45-58K depending on probe state.

The c910 memcpy bug remains unfixed. The shipping configuration is
`XEZIM_INIT_ZERO=1` plus the NBA-leak fix (commit `d92a551`) which
together get xezim through more of the test than vanilla but still
hit the AXI handshake stall well before completion.

## Round 33 (2026-05-10) — VCD diff (xezim vs iverilog) reveals pipeline stall

Direct signal-by-signal diff between xezim (INIT_ZERO + NBA fix) and
iverilog (TEST PASSED) on c910 memcpy.

**rbus_pipe0_wb_data**: ALL 304 xezim retire-writeback events
match iverilog **EXACTLY** through t=47215ns (the last xezim event).
Including the memcpy destination pointer progressing through
0xc3b8 → 0xc3bc → 0xc3c0 → 0xc3c4 → 0xc3c8.

**AXI handshake (biu_pad_awvalid, biu_pad_wvalid)**: every pulse pair
in both sims matches exactly through:
- awvalid pulses: 46575, 46785, 46995, 47215, 47425, 47635 (xezim's last)
- wvalid pulses:  46595, 46805, 47015, 47235, 47445, 47655 (xezim's last)
- Final wvalid=0 deassertion at t=47695 (xezim's final event)

**xezim's LAST event is at sim 47695ns** — a clean wvalid deassertion
completing the previous transaction. After that: complete CPU
silence. Simulation continues advancing wall time (reaches sim
110000) but NO signal changes occur. The CPU pipeline is frozen.

iverilog at sim 47855ns issues the NEXT awvalid (the 8th transaction
of this batch). xezim never does.

**Conclusion**: bug is NOT in the AXI slave or interconnect. The 7
transactions before the freeze succeeded normally. The bug is in the
CPU's pipeline: after the 7th write completes, xezim's CPU fails to
issue the next instruction's AXI write. iverilog does.

The pipeline stall is somewhere in the IFU→IDU→IU→LSU→BIU path.
Some combinational dependency between AXI completion and next-cycle
issue isn't being tracked correctly by xezim's bytecode-compiled
event-island convergence.

**This is the most-precise localization to date**: bug is in xezim's
simulation of how AXI bvalid completion feeds back into the CPU's
LSU/WMB drain → ROB retire → IFU next-PC-fetch chain. The chain is
correct for 7 cycles, then breaks. May be a signal-sensitivity issue
where a specific combinational path doesn't get re-evaluated after
the 7th completion. Investigation handed off — not resolvable in
remaining session time.

The 22+ rounds of IFU/IBUF investigation below were chasing two
separate red herrings: the "PC 0x712 missing" retire-log artifact
(round 22) and the precode/IBUF cone-of-influence work (rounds
23-24). The five IFU/IBUF synth tests committed during this session
remain as regression guards.

---

## Original investigation — root-cause narrowing on the c910 RISC-V memcpy hang

22+ rounds of investigation before the fix was identified. Preserved
below for reference.

## Symptom

- xezim runs c910 hello and cmark tests successfully.
- xezim's c910 memcpy test fails: simulation runs to its 1,000,195 ns
  watchdog (`*** Error: There is no instructions retired in the last
  50000 cycles!`), iverilog of the same RTL/program passes at sim
  1,019,650 ns.
- Last retire in xezim varies between rebuilds (T=44605, T=47395,
  T=59315) — "heisenbug" caused by bytecode-binary-layout shifts.
- Last common retire with iverilog is at PC 0x1B92 (T=59295 in one
  capture). After that, iverilog continues but xezim stops.

## What was ruled out (with refs)

| # | Hypothesis | Refuted by |
|---|---|---|
| 1 | NBA merge order (`block_index` vs `eval_order`) | Switching to `eval_order` regresses cmark; default `block_index` already works for cmark. Same memcpy failure in both modes. |
| 2 | `expr_max_width::Index` returning 1 for unpacked array elements | Codex hypothesis tested. Regresses cmark even though theoretically correct. Reverted. |
| 3 | `expr_max_width::Inside` → 1, `SystemCall::$signed` → arg width | Theoretically correct widening. Regresses hello. Reverted. |
| 4 | Case-stmt default-arm not firing for sel=3'b000 | Synthetic `tests/case_default_arm.rs` test passes — local logic correct. |
| 5 | IFU byte-reversal of memory at PC=0 | Misread on author's part. tb.v `f_spsram_large` testbench deliberately byte-reverses inst.pat literals into 4 byte-banks; this is the c910 test fixture's intentional layout, not a xezim bug. xezim correctly reads the byte-distributed form. |
| 6 | Vector pipeline (split_long, VIQ0, VFPU) | All stalled because nothing reaches them; cascading symptom only. |
| 7 | DIVUW dispatch (PC 0x1BA4 REMUW) | Both sims dispatch and retire DIVUW correctly at T=59165. AIQ0 entry-create matches. |
| 8 | AIQ0 entry 2 src0/src1 wb-bit at allocation | All 5 chain stages (`rt_dp_inst0_src0_data[1]`, `is_aiq0_create0_data[37]`, `aiq0_create0_data[59]`, `dp_aiq0_create0_data[59]`, `aiq0_entry2_create_data[59]`) bit-identical between iv and xz. dep_reg_entry's local rdy/wb/rdy_for_issue logic verified correct via standalone synthetic test (`tests/dep_reg_entry_synth.rs`). |
| 9 | Pipedown 227-bit `{N{en}} & data` replicate-AND | Earlier session's RTL ternary substitution failed identically. Synthetic `tests/replicate_and_pattern.rs` covers the pattern in isolation; passes. |
| 10 | Parameter-arithmetic `[P:P-8]` RangeSelect width | Synthetic `tests/range_select_param_arith.rs` covers it; passes. |
| 11 | `XEZIM_CASCADE_LIMIT=64` (settle convergence depth) | Tested. No change. `max_iters=6` already in default — cascade limit never the bottleneck. |
| 12 | `ident_lookup` AST fallback (100,010 per memcpy run) | Same 100,010 in cmark which passes. Only 4 unique idents fail compile, all cross-hierarchy refs starting with `x_ct_core`. AST fallback path works correctly for them. |

## Verified at high precision

The cascade of dependencies that the symptom builds on:

```
xezim memcpy fails at sim 1M watchdog
  ↑
AIQ0 entry 2 src0.wb flop stays 0 (verified via tb.v probe — flop never sets)
  ↑
LSU pipe3 writeback never fires (1 event in xz vs 250 in iv)
  ↑
RB entry stuck in WAIT_RDY (FSM never advances to REQ_BIU)
  ↑
rb_entry_not_sync_fence_ready = 0 (vs iv: 1)
  ↑
wmb_rb_so_pending = 1 (vs iv: 0)
  ↑
WMB SO FIFO not empty (create_ptr=2, pop_ptr=0)
  ↑
biu_lsu_b_vld never asserts AGAIN after T=45785 (4 successful AXI write
responses match iverilog exactly, then xezim stops)
  ↑
biu_pad_awvalid never asserts AGAIN after T=45935 (4 writes match iverilog
exactly, then xezim's CPU pipeline stops sending writes)
  ↑
lbuf_inst_vld / inst0 / inst0_pc / ifu_idu_ib_inst0_data CLEAR TO 0 at
T=45515 ← ROOT-LEVEL SYMPTOM
```

## What the IFU is doing at T=45515

When the pipeline stops at T=45515:
- xezim's IFU clears `lbuf_inst_vld`, `inst0`, `inst0_pc`, the IB output.
- iverilog continues delivering instructions at the same time, including
  inst at PC 0x388 with the T-Head custom-0 opcode `0x0B` (a vector
  load/store at offset 8 within its 16-byte cacheline).
- The 4 already-dispatched stores in xezim drain by T=45935.
- Retires can continue for a while as residual ROB entries commit.
- All AXI traffic ceases by T=45935.
- Watchdog fires at T=1,000,195.

## Unverified hypotheses for next session

Ranked by likelihood and ease of testing:

1. **xezim's IFU byte-bank assembly for non-zero offset within 16-byte
   cacheline.** The 4 stores that complete are all at one cacheline. The
   5th instruction needs to come from a different cacheline OR a
   non-zero offset, and xezim mis-assembles bytes.
   Test: Probe `ibuf_ibdp_inst0`'s per-byte-bank source signals at
   T=45505 in both sims. Find the bank that disagrees.

2. **`apply_nba` order: `nba_fast` drains before `nba_queue`.** AST
   fallback writes via `assign_value` go through `nba_queue`. If both
   compiled (`nba_fast`) and AST-fallback (`nba_queue`) writes target
   the same signal in the same tick, AST writes clobber compiled
   writes. cmark also has 100K fallbacks and passes, but maybe the
   specific signal pattern differs.
   Test: Search for any c910 RTL pattern where a signal is written by
   both a compiled `NbaAssign` and an AST-fallback statement in the
   same block.

3. **`NbaAssignArray` bypass of `nba_fast_index`.** Compiled
   `NbaAssignArray` uses `nba_fast_index` for merge. AST-fallback path
   in `assign_value` writes `signal_table` directly. Mismatched array
   element updates between the two paths could lose writes.
   Test: Add a debug assertion that warns if both paths target the
   same element in the same tick.

4. **`infer_lhs_width` `_ => 32` fallback for unsupported ExprKinds.**
   The function returns 32 for any ExprKind not in
   {Ident, Index, RangeSelect, Concat}. If c910 uses MemberAccess or
   other patterns in an LHS, the inferred width is wrong, causing the
   compiled write to use the wrong width.

5. **Case statement compilation with `ctx_width=0` selector.** Lower
   priority — the synthetic test passed in isolation, but the c910
   instantiation context may expose a corner the synthetic misses.

## Tooling and artifacts

- **Iverilog reference vvp** (built this session, verified TEST PASSED):
  - `/tmp/c910_iv_new.vvp` — basic probe set
  - `/tmp/c910_iv_aiq.vvp` — AIQ0 probes
  - `/tmp/c910_iv_state.vvp` — RB FSM probes
  - `/tmp/c910_iv_fence.vvp` — fence/idfifo probes
  - `/tmp/c910_iv_fifo.vvp` — idfifo internals
  - `/tmp/c910_iv_axi.vvp` — AXI signals
  - `/tmp/c910_iv_lsu.vvp` — LSU writeback chain
  - `/tmp/c910_iv_lsu2.vvp` — LSU 4-source OR
  - `/tmp/c910_iv_rb.vvp` — RB entry internals
  - `/tmp/c910_iv_chain.vvp` — pipedown chain
- **Iverilog rebuild flow** (one critical fix discovered this session):
  Defines wrapper MUST go inside the `-f` filelist, not before it on
  the command line. iverilog does not propagate macros from
  command-line files to filelist entries.
  Command: `iverilog -g2012 -I .../src -I /tmp/c910_iv_inc -o OUT.vvp -f
  /tmp/c910_iv_files_combined.list`
- **xezim VCDs** (memcpy runs at `--max-time 70000`):
  All in `/tmp/fix_memcpy/memcpy/dump_*.vcd`.
- **Timescale conversion**: iverilog writes VCD in 100ps; xezim writes
  in 1ns. Divide iverilog timestamps by 10 to match xezim.

## Definitive bug characterization (Round 22 — Questa cross-reference)

A QuestaSim 2021.1 VCD at `/home/bondan/agent/repo/memcpy_30k_70k.vcd`
covering sim 30K-70K with retire and AXI signals provides
ground-truth retire stream for the memcpy loop region.

**Questa retire stream around T=45005**:
- Cycle T=45005: PCs 0x710, 0x712, 0x714, 0x716 ALL retire across the 3 slots
- Loop body has 4 instructions

**xezim retire stream same cycle**:
- Cycle T=45005: PCs 0x710, 0x714, 0x716 retire (PC 0x712 MISSING)
- Searched all 3 retire slots across the entire run: **PC 0x712 NEVER appears**

PC 0x712 corresponds to the halfword at byte offset 2 within the
16-byte cacheline 0x710-0x71F. The original handoff diagnosed
"vector op at PC 0x712 stuck" — this Questa cross-reference proves
the diagnosis was correct all along; the 22 rounds of downstream
probing chased cascading symptoms while heisenbug probe-set shifts
made the downstream picture inconsistent.

**Bug location**: xezim's IFU never delivers PC 0x712 to the IDU's
dispatch unit. Pre-decode (`ct_ifu_precode.v`) or instruction-buffer
pop (`ct_ifu_ibuf.v` pop_h0/h1 selection) drops it.

The c910 testbench byte-distribution (tb.v:436-454) distributes each
inst.pat 32-bit literal across 16 byte-banks; `f_spsram_large.v:176-190`
reassembles them via `Q[N*8+7:N*8] = ramN_dout`. So for the cacheline
holding PC 0x710, byte 0x710 = ram0[i] = literal[31:24]. Whether this
makes PC 0x710 a 16-bit RVC or 32-bit RV instruction depends on the
exact halfword value — Questa shows it as a 16-bit RVC (since PC 0x712
retires separately, the inst at 0x710 must be 16-bit).

**Three remaining hypotheses for the next-session fix**:

1. **xezim mis-evaluates `ct_ifu_precode.v`** for the specific halfword
   data at this cacheline. The boolean expressions are straightforward
   (lines 240-296) but one could mis-compile.
2. **xezim's pop_h0/pop_h1 selection logic** (`ct_ifu_ibuf.v` lines
   5687-5694 and the 8000-line case-tree at 7920-8362) drops PC 0x712.
3. **xezim's pre-decode flag propagation** from the icache to the
   ibuf entries loses the bry0/bry1 bit for h2 (offset 2 halfword).

## Test files added (committed)

- `tests/dep_reg_entry_synth.rs` — c910 dep_reg_entry synth, passes
- `tests/range_select_param_arith.rs` — `[P:P-8]` slice, passes
- `tests/case_default_arm.rs` — case-stmt with default, passes
- `tests/replicate_and_pattern.rs` — `{N{en}} & data`, passes
- `tests/c910_settle_miri.rs` — AIQ0 dep_reg miri shape, passes

## Code changes committed this overall investigation

- `7379b85` bytecode: CaseNeq is self-determined like CaseEq
- `88dd1ea` tests: AIQ0 dep_reg miri shape
- `e629e5c` tests: replicate-AND synthetic regression
- `52b9da5` JIT: refuse blocks that touch >64-bit signals + `docs/u64_audit.md`
- `84332c7` tests: case-stmt default arm for c910 IFU/IB shape
- `86343f0` tests: synthetic c910 dep_reg_entry + param-arith RangeSelect
- xezim-core `75b2adf` value: document `to_u64` silently truncates wide values

## Round 23 (2026-05-10) — IFU code audit, no new fix candidate

Read both `ct_ifu_precode.v` (320 lines) and the IBUF pop case-tree
sections in `ct_ifu_ibuf.v` end-to-end. Confirmed nothing exotic in
either:

- precode logic is straightforward `==` and `&&`/`!` reductions; the
  expr_max_width relational fix (f127254/01ca2b1) already covers this
  pattern. No remaining width-inference hazard found.
- IBUF entry-output mux uses a standard 32-way one-hot `case` keyed by
  `ibuf_retire_pointer[31:0]` — no `casez` here, just plain `case`.
  CaseEq compilation already verified by `case_default_arm` synth test.
- IBUF dispatch arm picker at lines 7920-8362 uses `casez({h0..h4
  _32_start})` with `?` wildcards. xezim's casez compilation has been
  verified (CasezEq op in bytecode.rs:606, Value::casez_eq treats Z bits
  as don't-care; `?` lex-maps to LogicBit::Z at value.rs:25).

Audited Value comparison helpers in xezim-core/src/value.rs:
- `is_equal` / `case_eq` / `less_than` / etc. use `to_u64().unwrap_or(0)`
  on no-X paths (value.rs:930, 999) — same wide-truncation hazard as
  add/sub/mul before commit 710a793, but only matters for width > 64.
  Case selector here is 32-bit, so not the immediate culprit.

Audited expr_max_width and compile_expr alignment:
- Unary self-determined operand_ctx=0 fix is present (bytecode.rs:915).
- Conditional handler correctly self-determines condition (bytecode.rs:1019).
- expr_max_width::Index still returns 1 unconditionally (line 1877).
  `infer_lhs_width::Index` correctly distinguishes unpacked-array element
  width vs bit-select width=1 (lines 1668-1681). Memory notes that
  "fixing expr_max_width::Index to return element width regresses cmark"
  — that fix exposed a downstream truncation that wasn't paired-fixed.
  This remains a known but uncorrected divergence between the two
  inference functions.

**Why no fix this round**: After full code audit of the IFU/IBUF
critical path and the bytecode width-inference helpers, no single
change has high probability of fixing memcpy without regressing
cmark/hello. The 22-min test cycle and heisenbug-shifted symptoms
preclude speculative one-shot fixes.

**Required for next round** (must iterate, not one-shot):

1. ~~**Add a single targeted probe** in xezim's tb.v that captures
   `pre_code[31:0]` for the precode of the cacheline containing PC
   0x710~~ **RULED OUT** via cone-of-influence synth test
   `tests/ifu_precode_c910_pc710.rs` (commit 9e14b02). xezim's bytecode
   compile of `ct_ifu_precode.v`'s boolean evaluation produces the
   correct pre_code for the 0x710 cacheline inst_data for both
   candidate byte-orderings. Hypothesis #1 (precode mis-compile)
   eliminated without needing the full 22-min run.

2. ~~If precode matches Questa: probe `entry_inst_data_N`~~ **RULED OUT**
   via `tests/ifu_ibuf_entry_pop_c910.rs` (commit 166aaef) and
   `tests/ifu_ibuf_32_instances_c910.rs` (commit 6aab719). xezim correctly
   compiles the per-entry write-enable replicate-AND + 32-way one-hot
   pop mux in both single-module and 32-instance-sub-module structural
   forms. Hypothesis #2 eliminated.

3. ~~If pop-mux output is wrong: bug is in case-tree selection.~~ Also
   **RULED OUT** via `tests/ifu_ibuf_casez_dispatch_c910.rs`
   (commit 62d769c). The 5-bit `casez({h0..h4_32_start})` dispatch tree
   produces correct half_num arm selection for all 32 input combos.
   Hypothesis #3 eliminated.

## Round 26 (2026-05-10) — SECOND REORIENT: same bug as cmark canary

Widening the retire+writeback tracer to capture all retire events
during sim 46000-47000 (not just the 0x700-0x720 loop region) caught
the actual termination event: **TEST FAILED at t=46665**.

The full retire sequence at the failure:
```
t=46025 pc=0x0150 wb0=0x0       wb1=0x0
t=46055 pc=0x0158 wb0=0x10      wb1=0x2
t=46055 pc=0x015c wb0=0x10      wb1=0x2
t=46075 pc=0x0160 wb0=0x400     wb1=0x410
t=46115 pc=0x0166 wb0=0x400     wb1=0x410
t=46115 pc=0x0168 wb0=0x400     wb1=0x410
t=46115 pc=0x016c wb0=0x400     wb1=0x410
t=46535 pc=0x0170 wb0=0x400     wb1=0xee
t=46555 pc=0x0172 wb0=0xe6      wb1=0xee
t=46655 pc=0x00ee wb0=0x2c      wb1=0x0
t=46665 pc=0x00fa wb0=0x3b      wb1=0x0
TEST FAILED
```

So the memcpy loop at 0x710 DOES terminate (jumps to 0x150 around
t=46000), then falls into post-loop code 0x158→0x172, then takes a
branch/jump to 0x0ee→0x0fa where the failure sentinel `64'h2382348720`
is observed in `value0`/`value1`/`value2` per tb.v:562, triggering
`$display("TEST FAILED")`.

**`0x2382348720` is the EXACT same failure sentinel** documented in
the existing memory `feedback_c910_cmark_test_failed_canary.md` for
the c910 cmark test:

> TEST FAILED is tb.v sentinel hitting 0x2382348720 from corrupt
> `idu_iu_rf_pipex_src0/src1` at sim 49405. Bug is in xezim's IDU
> mux/PRF/forwarding path. Heisenbug hypothesis falsified — both
> ae1e88f and 7d4aede fail identically.

**Conclusion**: c910 memcpy and c910 cmark fail with the same bug —
**xezim's IDU register-file-pipe source-operand mux delivers corrupt
data**. The IFU/IBUF cone-of-influence tests (rounds 23-24) and the
PC 0x712 / retire-log-presentation analysis (rounds 22, 25) were all
investigating the wrong layer. The bug has been characterized in
prior sessions but not fixed.

**Concrete next step** (follows previous-session investigation
trail): probe `idu_iu_rf_pipex_src0` / `idu_iu_rf_pipex_src1` at
t=46555 in xezim vs iverilog reference; the divergence cycle pinpoints
which specific PRF read or forward path delivers the wrong value.
Once that pipe/cycle is known, the fix target in
`xezim/src/compiler/` is a specific compile_expr or NBA-ordering
bug; the cmark canary memory entry has the relevant pointer.

## Round 25 (2026-05-10) — RETIRE-STREAM REORIENT: loop iterates forever

**Major reorient via live retire tracer (added to tb.v, not under
xezim git).** The "PC 0x712 missing" diagnosis was a **logging
artifact**, NOT the actual bug:

- The 32-bit inst at PC 0x710 = `0x5847d70b` (T-Head custom-0 opcode
  0x0b, rd=x14, funct3=5) occupies bytes 0x710-0x713. PC 0x712 is the
  upper halfword of that inst.
- Questa's retire log emits BOTH PC 0x710 AND PC 0x712 for the same
  32-bit inst (lower + upper halfword PC). xezim only emits PC 0x710.
- This is a retire-log presentation difference, not a missed execution.

**The actual bug**: the memcpy loop body `0x710 → 0x714 → 0x716`
iterates forever in xezim. Retire trace from sim 46000 to 50000 shows
continuous loop iteration with no halt; the watchdog eventually
fires at sim 1M because no sentinel-write event ever occurs (the
TEST PASSED check at tb.v:553 requires writing `64'h444333222` to
the success address).

Loop body decoded:
- PC 0x710: T-Head custom-0 inst (likely vector memcpy primitive), rd=x14
- PC 0x714: c.addiw x14, 1 (RVC: x14 += 1)
- PC 0x716: bne ?, ?, -6 (32-bit branch back to 0x710 if condition true)

Inst.pat @000001C4 = `0bd74758 0527e39d d7feeff0 7ff79567`. Per
tb.v's big-endian byte distribution into 16 byte-banks:
- byte 0x710 = 0x0b, byte 0x711 = 0xd7, byte 0x712 = 0x47, byte 0x713 = 0x58
- inst at 0x710 (little-endian within halfword): 0x5847d70b → custom-0, rd=x14
- inst at 0x714: 0x2705 (c.addiw x14, 1) — increments x14
- inst at 0x716: 0xfed79de3 (BNE) — branches back to 0x710

**New hypothesis**: xezim's execution of the custom-0 vector inst at
PC 0x710 has a wrong semantic that either fails to terminate the
loop, fails to actually copy memory, or sets x14 incorrectly so the
BNE comparison never goes false. cmark/hello don't exercise this
specific custom-0 funct3=5 inst.

**Next concrete step**: probe x14 (architectural register, not PRF
preg) at t=41145 (loop start) and at t=49995 (still iterating). If
x14 monotonically increases as expected, the bug is in the BNE
comparison source register OR the vector inst's memory side effect
(not producing the data the BNE compares against). If x14 stays at
a small value despite the c.addiw retiring, the bug is in xezim's
retired-but-not-effective-on-x14 forwarding for the custom-0 inst's
rd write.

The five committed cone-of-influence synth tests
(ifu_precode_c910_pc710, ifu_ibuf_entry_pop_c910,
ifu_ibuf_casez_dispatch_c910, ifu_ibuf_32_instances_c910,
ifu_ibuf_create_ptr_rotate) remain useful as regression guards but
they were investigating the wrong layer — IFU is not the bug location.

## Round 24 (2026-05-10) — All four IBUF isolated patterns work

Round-23's three hypotheses plus the structural 32-instance pattern
all verified correct in xezim's bytecode compile via cone-of-influence
synth tests:

- `tests/ifu_precode_c910_pc710.rs` — precode boolean eval ✓
- `tests/ifu_ibuf_entry_pop_c910.rs` — per-entry write + one-hot pop ✓
- `tests/ifu_ibuf_casez_dispatch_c910.rs` — 5-bit casez dispatch tree ✓
- `tests/ifu_ibuf_32_instances_c910.rs` — 32 cross-module instances ✓

The c910 memcpy still fails at sim 1M watchdog. So the bug must be:

(a) **An interaction across patterns** — some combination of the four
    that none of the isolated tests exercise (e.g., simultaneous writes
    to two entries while pop-mux reads a third, or casez-dispatch
    selection feeding into the pop-mux entry selection simultaneously
    with an entry-write).

(b) **In the IFU pipeline above the IBUF** — icache fetch
    (`ct_ifu_icache.v`), LBUF (`ct_ifu_lbuf.v`), or IBDP
    (`ct_ifu_ibdp.v`) delivering wrong inst_data or half_vld bits to
    the IBUF input. The IBDP halfword-vld-num logic was probed in a
    prior session (`case_default_arm` synth test passed) but the
    surrounding pipeline (precode → ibdp → ibuf) hasn't been exercised
    as a whole.

(c) **A scheduling/NBA-ordering bug** specific to the c910's exact
    block layout — possibly triggered only when both a write and a
    read of the same cross-instance signal occur in the same tick
    across different always-blocks compiled to different
    `CompiledBlock`s with different partition assignments.

**Concrete next step**: combine all four patterns into ONE synth test
where the dispatch-tree output gates the pop-mux selection AND a
simultaneous entry-write fires for an adjacent entry. If that passes
too, the bug is likely above the IBUF (option b) or in NBA scheduling
(option c). Then the full-test cycle becomes unavoidable.

## Round 34 (2026-05-10) — DEFINITIVE bug location: `cur_bresp_buf_bvalid` FF

VCD signal diff between xezim and iverilog proves:

**`pad_biu_bvalid` (raw AXI slave response)**: 4/4 pulses match
exactly through sim 47695 — including the 4th rising edge. Both
sims correctly see the slave's bvalid.

**`cur_bresp_buf_bvalid` (registered in BIU write channel)**:
iverilog captures 4 pulses (at 47065, 47275, 47485, **47705**);
**xezim only captures 3** (last at 47485). The 4th pad_biu_bvalid
pulse at sim 47695 is NOT registered into the BIU response buffer
in xezim.

Downstream cascade:
- `cur_bresp_buf_bvalid` never goes high for the 4th transaction
- `biu_lsu_b_vld = cur_bresp_buf_bvalid && !back_full` stays 0
- `rb_r_so_id_hit` (SO-FIFO pop trigger via the read channel paired
  with this write) doesn't get the matching event
- SO FIFO accumulates entries; `rb_wmb_so_pending=1` blocks new
  `wmb_biu_aw_dp_req`
- CPU pipeline stalls completely

**Exact bug location** at `ct_biu_write_channel.v:972-980`:
```verilog
always @(posedge bcpuclk or negedge cpurst_b)
begin
  if(~cpurst_b)
    cur_bresp_buf_bvalid <= 1'b0;
  else if(pad_biu_bvalid && !back_full)
    cur_bresp_buf_bvalid <= 1'b1;
  else if(!back_full)
    cur_bresp_buf_bvalid <= 1'b0;
end
```

`bcpuclk` is gated from `coreclk` via passthrough `gated_clk_cell`.
`back_full = back_valid && back_pending`. Both back_valid/back_pending
are FFs clocked on `coreclk`, with `pad_biu_back_ready=1'b1` tied
high in ct_biu_top.v:1188 forcing them to clear each cycle.

**Likely root cause**: NBA ordering race between
`cur_bresp_buf_bvalid` FF (clocked on `bcpuclk`) and
`back_valid`/`back_pending` FFs (clocked on `coreclk`). Both clocks
are effectively the same (gated_clk_cell is a passthrough), but
xezim's bytecode-compile may treat them as distinct clock domains
in NBA scheduling, creating a race where `back_full` reads as 1
during the same posedge that `pad_biu_bvalid` first arrives —
blocking the capture for that one specific cycle.

This matches the heisenbug pattern (different probe sets shift
when the race resolves which way).

**Specific fix options**:
1. Recognize gated_clk_cell passthrough at compile time and unify
   the clock nets (xezim simulator-level fix)
2. Audit NBA scheduling order between FFs that share an effective
   clock domain but distinct named clock nets
3. Force `XEZIM_NO_PARALLEL=1` plus other knobs to serialize NBA
   evaluation (likely won't help — already in legacy mode)
