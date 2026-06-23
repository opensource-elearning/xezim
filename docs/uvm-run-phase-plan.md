# UVM run-phase execution — implementation plan

Status: **design / not started.** Goal is to take a real UVM testbench
(reference target: `GettingVerilatorStartedWithUVM` `data0_test`) from "boots +
build phase" to a **full run** that matches the Verilator reference output:
build → connect → topology → run_phase (stimulus, packet collection, coverage,
scoreboard) → report → `$finish`.

This doc is grounded in the xezim internals discovered while debugging GVS
(2026-06-22). See `[[project_uvm_phasing]]` in memory for the raw findings.

---

## 1. Success criteria

For `data0_test` (UVM 1800.2-2017 v1.1, `+UVM_TESTNAME=data0_test`):

- [ ] M1 — Topology prints: build_phase + connect_phase run over the full
      component tree; `uvm_test_top` topology table matches the reference
      (env → penv_in/out → agents → driver/monitor/sequencer, scoreboard,
      coverage).
- [ ] M2 — run_phase executes: the driver↔sequencer handshake runs, stimulus
      flows through `pipe_if`, the monitors collect packets.
- [ ] M3 — Reference parity: `COLLECTED PACKETS = 76` (both monitors),
      coverage line, scoreboard, `--- UVM Report Summary ---` with
      `UVM_ERROR : 0 / UVM_FATAL : 0`, clean `$finish` (≈2us).
- [ ] Guard — static suite stays **1005/22**; the 35 sv-tests uvm-examples do
      not regress; the 6 livelocking uvm-examples ideally recover.

---

## 2. Why it doesn't work today (recap)

`run_test` resolves to `module.tasks["run_test"]` = the **method**
`uvm_root::run_test`, which the process scheduler (`run_process_stmts`,
simulator.rs:13613) inlines as a **free task** (via `bind_task_frame`:29670) →
the body runs with `this=None`. Two consequences and two execution options:

- **Source-phaser path (A):** run the real `uvm_root::run_test`. Blocked by
  (a) `this`-context (its bare `m_children` etc. don't resolve — the t=0
  `NOCOMP`), and (b) its `fork m_run_phases join_none; wait(m_phase_all_done)`
  which the scheduler never advances (runs to max-time).
- **Rust-phaser path (B):** `run_uvm_test_real` (simulator.rs:30673). Today it
  builds the tree + runs function phases, but **skips `uvm_driver` run_phase,
  has no sequencer/objection model, runs no end phases**, and currently **hangs
  in `instantiate_class(data0_test)`'s `new()`** (:30411). It structurally
  cannot reach M3 as written.

**Core insight:** the hard, shared work for *both* paths is the run-phase
runtime — nested-blocking-call suspension, the sequencer↔driver TLM handshake,
and the objection model. Phase *orchestration* (Rust loop vs source
`m_run_phases`) is comparatively small. The plan builds the shared runtime
first, then picks the orchestrator.

---

## 3. Strategic decision

**Primary: extend the Rust phaser (path B)** as the orchestrator, but make
run_phase actually execute the user's stimulus by building the shared runtime
pieces. Rationale: `run_uvm_test_real` already does build/connect/topology for
sv-tests UVM; it's contained, debuggable, and avoids depending on robust
fork/join + wait-on-variable suspension across *all* of UVM's source phasing.

**Keep path A viable as the long-term ideal** (real UVM semantics generalize to
any test), but it is gated on (1) a safe `this`-context fix and (2) general
fork/join_none + `wait(var)` resume — larger and riskier. Treat A as a later
migration once the runtime in §4 exists.

---

## 4. Work breakdown (each item = a landable change with its own gate)

### P0 — Prerequisite: nested blocking task/method-call suspension
Already designed in `[[project_nested_task_suspension]]` (user picked option
2a: inline blocking task/method calls so internal waits suspend the process
instead of spinning to a loop cap). This is the foundation — the driver's
`forever get_next_item(req)` and the sequence `start_item/finish_item`
handshake are nested blocking calls.
- Files: `run_process_stmts` (13613) inline machinery; `bind_task_frame`
  (29670) / `unwind_task_frame` / `task_cleanup` stack; add `ScopePop`-style
  teardown for method frames.
- Extend the existing free-task inline (13642-13664) to a **method**-aware
  inline (MemberAccess receiver): resolve the receiver handle, push
  `this`/`class_context` paired with a `ScopePop` cleanup that pops them
  (a `TaskCleanup` flag — this is the *narrow* version of the regressing change
  tried on 2026-06-22; do NOT touch `instance_assoc_member` globally).
- Gate: static 1005/22; the 6 livelock uvm-examples stop timing out.

### P1 — `this`-context for run_phase/method bodies (narrow)
Depends on P0's method-aware inline. The receiver for `uvm_root::run_test` is
the lazily-created singleton; for component run_phase tasks it's the component
handle (known at spawn time — see P4). Ensure every spawned/inlined method body
carries its `this`.
- Pitfall (proven): a global lazy-resolve in `instance_assoc_member` (28005)
  regresses 36 static tests. Resolve `this` at the *call/spawn site* and push
  it per-frame instead.
- Gate: GVS reaches `RNTST` then build/connect without `NOCOMP`; static 1005/22.

### P2 — `instantiate_class` constructor hang
`instantiate_class(data0_test)` → `exec_method_call(h,"new")` (:30411) loops in
the source `uvm_component::new` chain (the **factory** create path in source
run_test builds the same class fine — so it's instantiate_class's eager
property-init/ctor path that differs). Two options:
- (a) Diagnose the loop (suspect `m_set_cl_msg_args` → cmdline-processor
  `get_arg_values` interaction, or eager `property_inits`). Add a depth/iteration
  probe in `exec_method_call("new")`.
- (b) Route component construction through the working factory bridge
  (`create_component_by_name`, see :28843) instead of `instantiate_class`.
- Gate: `run_uvm_test_real` builds `uvm_test_top` and the BFS completes
  (topology BFS diagnostics clean); M1 topology prints.

### P3 — Objection model (phase-end + `$finish`)
UVM run_phase ends when all components have dropped their phase objection.
Today `raise/drop_objection` are no-ops only when `!real_uvm` (:28835); under
real_uvm they call source methods but nothing tracks phase completion.
- Add a per-phase objection counter keyed by phase (a simple `i64` is enough
  for run_phase). Intercept `phase.raise_objection`/`drop_objection`
  (`uvm_phase`/`uvm_objection`) to inc/dec it.
- The run-phase orchestrator waits until the counter returns to 0 (after at
  least one raise), then runs end phases and `$finish`.
- This also sets `m_phase_all_done` if/when path A is revisited.
- Gate: GVS run_phase terminates on its own (not max-time); `$finish` fires.

### P4 — Sequencer ↔ driver TLM handshake (the stimulus engine)
The heart of run_phase. Model the `uvm_seq_item_pull_port`/`_imp` channel:
- Driver `seq_item_port.get_next_item(req)` blocks until a sequence offers an
  item; `item_done([rsp])` completes it.
- Sequence `start_item(req)/finish_item(req)` (and `uvm_do`/`body`) offers items
  and blocks for the handshake.
- Implement as a per-sequencer request/response rendezvous (two queues +
  process suspension from P0), mirroring the `mailbox_get_waiters` pattern
  already in the scheduler (:1743). Connect via the
  `seq_item_export`↔`seq_item_port` binding established in connect_phase.
- Un-skip drivers in `run_uvm_test_real` (:30810) once the handshake exists.
- Run `default_sequence`/test-started sequences on each sequencer's run_phase.
- Gate: M2 — stimulus flows; `pipe_monitor` collects packets; the input/output
  monitors report nonzero `COLLECTED PACKETS`.

### P5 — End phases + report
Run `extract/check/report/final` over the tree after run_phase drains (P3).
Wire `uvm_report_server` summary (`UVM_INFO/WARNING/ERROR/FATAL` counts) —
xezim already routes `uvm_report_*` (eval_call :29166); accumulate counts and
emit `--- UVM Report Summary ---`.
- Gate: M3 — Report Summary matches reference; `UVM_ERROR/FATAL = 0`.

### P6 — Clocks / DUT interaction during run_phase
GVS drives `pipe_if` over `clk` (`always #5 clk`). The run-phase processes must
interleave with the clock + DUT continuous-assign/always blocks. The event loop
already advances time for spawned processes (`spawn_method_task_process` :30611);
verify the monitor's `@(posedge clk)` sampling and the driver's signal writes
land on the DUT. Mostly validation, but budget for `@(vif.cb)` clocking-block
and virtual-interface-write plumbing gaps.
- Gate: monitors collect the reference packet count (76); scoreboard matches.

---

## 5. Recommended sequencing & milestones

1. **P0 + P1** → method-aware blocking-call suspension with per-frame `this`.
   Unblocks everything; also recovers the 6 livelock uvm-examples. *(largest,
   riskiest — stage with frequent static-suite runs.)*
2. **P2** → `run_uvm_test_real` builds the tree without hanging → **M1
   (topology)**.
3. **P4 + P3** → handshake + objections → **M2 (run_phase executes, terminates)**.
4. **P5 + P6** → end phases, report, clock/DUT fidelity → **M3 (reference
   parity)**.

Each milestone is independently demoable. M1 alone is a meaningful, shippable
improvement (UVM topology for real testbenches).

---

## 6. Risk register

- **Process-suspension correctness (P0)** is the dominant risk: it touches the
  hot `run_process_stmts` path used by *every* process. Mitigation: method-aware
  inline must be additive (free-task path unchanged), gated per-frame, with
  static 1005/22 after each step. The 2026-06-22 attempt regressed 36 tests by
  changing `instance_assoc_member` globally — avoid that class of fix.
- **Sequencer/driver semantics (P4)** are intricate (arbitration, priorities,
  lock/grab, rsp). Scope to the common get_next_item/item_done + start/finish
  path first; defer arbitration/lock/grab (the 6 livelock examples exercise
  these — track separately).
- **Path A temptation:** driving source `m_run_phases` looks "more correct" but
  needs robust fork/join_none + `wait(var)` resume across all UVM phasing.
  Don't start there; migrate only after §4 runtime exists.
- **run_uvm_test_real divergence:** it's a heuristic phaser; as it grows, keep
  it behind `uses_real_uvm()` and the existing sv-tests UVM examples as
  regression anchors.

---

## 7. Validation harness

- GVS: `xezim --simulate -s top -I <UVM>/src -I rtl -I sv -I tb
  -D UVM_REPORT_DISABLE_FILE_LINE -D UVM_NO_DPI -D SVA_ON
  <UVM>/src/uvm_pkg.sv sv/pipe_pkg.sv sv/pipe_if.sv rtl/pipe.v tb/top.sv
  +UVM_TESTNAME=data0_test` (also `data1_test`). Reference output in the repo
  README.
- sv-tests static suite: `sv-tests/run_static_local.sh` (baseline 1005/22).
- sv-tests UVM examples (35) + the 6 livelock cases as a UVM regression set.
- Minimal repros: `/tmp/svc_repro{2,3,4}.sv` (the static-member/singleton
  patterns) must keep passing.

---

## 8. Out of scope (for the first full-run milestone)

DPI regex (`uvm_re_*`), full factory specialization
(`uvm_component_registry#(T,name)`), command-line verbosity/config overrides
beyond what's already bridged, register-abstraction-layer (RAL), and UVM
sequence arbitration/lock/grab. None are needed for `data0_test` M3.

---

## 9. Progress log

### 2026-06-23 — M1 build phase unblocked (branch `uvm-run-phase`)
- **for-loop signedness bug FIXED** (general, high-value): `for (int i = N-1;
  i >= 0; --i)` was infinite (for-header `int` lost its sign → `i >= 0`
  always-true). Fixed in the AST For handler — sign the var at init AND
  re-assert after each step. This is what UVM `apply_config_settings`
  (`for(int i=rq.size()-1; i>=0; --i)`) hit. Static held 1005/22.
- **Route B** (`run_test` → `run_uvm_test_real`) + **name-arg** construction
  fix: `run_uvm_test_real` now builds the **full GVS component tree**
  (env/agents/monitors/scoreboard+TLM-fifos/coverage — all "Build stage
  complete"). Was total NOCOMP-at-t=0. Route B gated on a named test
  (+UVM_TESTNAME or arg) to avoid regressing bare-`run_test()` tests.
- **Blocked at**: `uvm_resource_pool::sort_by_precedence` hangs on the 2nd
  monitor → **keystone = instance-member-queue push/size persistence bug**
  (`rq.push()` ×3 → size 1; `obj.q.push_back` reads 0). Fix this first
  (unblocks M1 connect/topology), then P3/P4 for M2/M3.
- Repros: `/tmp/forloop3.sv` (for-loop + member-queue), `/tmp/qdel.sv`
  (member-queue delete/push).

### 2026-06-23 (cont.) — through build+connect into run_phase
- KEYSTONE fixed: **user-method-over-builtin precedence** (eval_call ~28510,
  gated on `expr_assoc_name(expr).is_none()`) — uvm_queue/uvm_pool methods no
  longer shadowed by builtins; `sort_by_precedence` works.
- **default-arg application + signedness** (bind_task_frame ~29853 +
  exec_method_in_class_hierarchy ~32813): missing arg now uses the formal's
  default (not 0) and keeps its sign — fixes UVM `start(...,this_priority=-1)`
  → SEQPRI.
- GVS data0_test: **build (driver+sequencer) + connect complete, topology
  print + run_phase REACHED**. All fixes validated static 1005/22.
- **P0 boundary confirmed**: run_phase spins at t=0 — `pipe_monitor::
  collect_data` (`forever @clk`) and `uvm_sequence_base::start` run
  synchronously (exec_method_in_class_hierarchy) instead of inlined. Next:
  extend run_process_stmts' blocking-call inline to class methods (P0), then
  un-skip the driver + sequencer↔driver TLM rendezvous (P4) + objections (P3).
