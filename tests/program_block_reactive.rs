//! Program-block initial statements run through the same executor as module
//! initials — so blocking task/method calls suspend and resume correctly.
//!
//! IEEE 1800.1-2023 §24 (`program` blocks): a `program` block's `initial`
//! processes are scheduled in the *reactive* region. xezim drains them from
//! `pending_reactive`. Previously `drain_reactive_region` ran each statement
//! through the bare `exec_statement` loop, which has no blocking-task inlining
//! and no method `this`-binding. A `program` initial that called a task whose
//! body blocked (`#delay`/`@event`/`wait`) ran that body SYNCHRONOUSLY — the
//! blocking control was not honoured, the caller's continuation ran at t=0,
//! and (for the genuine UVM path) `this` came back null.
//!
//! The fix routes the queued statements through `run_process_stmts` with a
//! fresh pid — exactly the executor module initials use — so a `program top`
//! behaves like a `module top`. These pure-SV regressions pin that:
//!
//!   1. A bare-name blocking task (with a `wait` body) called from a program
//!      initial SUSPENDS until the condition holds, then runs its
//!      continuation (pre-fix: the `wait` was ignored, continuation ran at t0).
//!   2. A `#delay` task body suspends the program initial, advancing time
//!      before the continuation.

use xezim::simulate;

fn messages(sim: &xezim::compiler::Simulator) -> Vec<String> {
    sim.output.iter().map(|o| o.message.clone()).collect()
}

/// A program-block initial that calls a blocking task (containing `wait`) must
/// suspend and resume, running its continuation *after* the wait releases.
/// Pre-fix the `wait` was not honoured: "RESUMED t=0 q=0".
const WAIT_TASK: &str = r#"
program top;
  int q, reached;
  task automatic hop;
    wait(q == 1);     // blocks until the forked raiser sets q
  endtask
  initial begin
    q = 0; reached = 0;
    fork
      begin
        #5;
        q = 1;
      end
    join_none
    hop();            // bare-name blocking task call
    reached = 1;      // continuation: must run AFTER the wait releases
    $display("RESUMED t=%0t q=%0d reached=%0d", $time, q, reached);
  end
endprogram
"#;

#[test]
fn program_blocking_wait_task_suspends_and_resumes() {
    let sim = simulate(WAIT_TASK, 200).expect("simulate failed");
    let msgs = messages(&sim);
    let line = msgs.iter().find(|m| m.starts_with("RESUMED")).unwrap_or_else(|| {
        panic!("initial never resumed; output: {:?}", msgs)
    });
    // The wait must block until q==1 at t=5; the continuation runs at t=5.
    // Pre-fix: "RESUMED t=0 q=0 reached=1" (the wait ran synchronously).
    assert!(
        line.contains("t=5") && line.contains("q=1") && line.contains("reached=1"),
        "expected resume at t=5 with q=1, got: {}\noutput: {:?}",
        line,
        msgs
    );
}

/// A `#delay` inside a task called from a program initial must advance time
/// before the caller's continuation. Pre-fix the body ran without suspending.
const DELAY_TASK: &str = r#"
program top;
  int t_after;
  task automatic stall;
    #7;
  endtask
  initial begin
    stall();
    t_after = $time;
    $display("AFTER delay t_after=%0d at %0t", t_after, $time);
  end
endprogram
"#;

#[test]
fn program_blocking_delay_task_advances_time() {
    let sim = simulate(DELAY_TASK, 200).expect("simulate failed");
    let msgs = messages(&sim);
    let line = msgs.iter().find(|m| m.starts_with("AFTER delay")).unwrap_or_else(|| {
        panic!("continuation never ran; output: {:?}", msgs)
    });
    assert!(
        line.contains("t_after=7"),
        "expected time advanced to 7 across the blocking task call, got: {}\noutput: {:?}",
        line,
        msgs
    );
}

/// Program-top must match module-top for the same blocking-task construct —
/// the whole point of routing both through the same executor.
const MOD_EQUIV: &str = r#"
module top;
  int q, reached;
  task automatic hop;
    wait(q == 1);
  endtask
  initial begin
    q = 0; reached = 0;
    fork begin #5; q = 1; end join_none
    hop();
    reached = 1;
    $display("MOD_RESUMED t=%0t q=%0d reached=%0d", $time, q, reached);
  end
endmodule
"#;

#[test]
fn module_top_blocking_wait_task_is_the_reference() {
    let sim = simulate(MOD_EQUIV, 200).expect("simulate failed");
    let msgs = messages(&sim);
    let line = msgs.iter().find(|m| m.starts_with("MOD_RESUMED")).unwrap_or_else(|| {
        panic!("module initial never resumed; output: {:?}", msgs)
    });
    // Reference behaviour: the program-top case above must match this exactly.
    assert!(
        line.contains("t=5") && line.contains("q=1") && line.contains("reached=1"),
        "module reference should resume at t=5, got: {}\noutput: {:?}",
        line,
        msgs
    );
}
