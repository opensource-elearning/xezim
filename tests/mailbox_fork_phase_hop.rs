//! Mailbox producer/consumer + fork-automatic propagation regression.
//!
//! Two IEEE 1800.1-2023 scheduling bugs that deadlocked a fork-driven
//! mailbox phase-hop loop (the mechanism the UVM phase scheduler uses, but
//! reproduced here in plain SystemVerilog):
//!
//! 1. **`mailbox.put` in expression context did not unblock a parked `get`.**
//!    §15.4.3/§15.4.5: a `put` stores in FIFO order and unblocks any process
//!    waiting in a `get`/`peek`. xezim had two `put` implementations —
//!    `exec_method_call` (which delivered to a parked waiter) and
//!    `eval_call_inner` (statement/expression context, which only pushed to
//!    the queue). A forked producer's `put` took the `eval_call_inner` path,
//!    so the consumer's parked `get` never resumed and the loop deadlocked.
//!
//! 2. **Fork-child-local propagation clobbered a value the parent modified
//!    after the fork.** §9.3.2: a fork child's *writes* to the parent's
//!    automatic variables are visible to the parent. xezim models this by
//!    giving each child a *copy* of the parent's locals and merging the
//!    child's frames back when it finishes — but it merged *every* local,
//!    including ones the child only inherited (never wrote). A mailbox `get`
//!    delivery into the parent while a `fork ... join_none` child was still
//!    running was overwritten by the child's stale inherited copy on finish,
//!    so the loop re-read the previous handle forever.
//!
//! Both are exercised by a fork-driven mailbox phase-hop loop (no UVM library
//! dependency). The test asserts the loop ADVANCES through successive handles
//! rather than spinning on the first one or deadlocking.

use xezim::simulate;

const SRC: &str = r#"
class C;
  int id;
  function new(int i); id = i; endfunction
endclass

module top;
  mailbox mb = new();

  // A forever loop that gets the next handle from a mailbox, forks a worker
  // that uses it and schedules the successor, then yields with #0.
  task automatic run_loop;
    forever begin
      C phase;
      mb.get(phase);
      $display("LOOP got phase.id=%0d at %0t", phase.id, $time);
      fork
        begin
          automatic C succ = new(phase.id + 1);
          #5;
          mb.put(succ);
        end
      join_none
      #0;
      if (phase.id >= 103) begin
        $display("DONE at %0t", $time);
        $finish;
      end
    end
  endtask

  initial begin
    automatic C seed = new(100);
    mb.put(seed);
    run_loop();
  end
endmodule
"#;

fn messages(sim: &xezim::compiler::Simulator) -> Vec<String> {
    sim.output.iter().map(|o| o.message.clone()).collect()
}

#[test]
fn forked_producer_put_wakes_parked_get_and_loop_advances() {
    // Bug 1 alone (no delivery) deadlocks: only "LOOP got phase.id=100" prints
    // and the sim ends at t=0. Bug 2 alone (delivery but clobber) spins: the
    // loop re-reads phase.id=100 forever. Both fixed -> the loop advances
    // 100 -> 101 -> 102 -> 103 -> DONE at t=15.
    let sim = simulate(SRC, 1000).expect("simulate failed");
    let msgs = messages(&sim);

    let ids: Vec<i64> = msgs
        .iter()
        .filter_map(|m| m.strip_prefix("LOOP got phase.id="))
        .filter_map(|s| s.split_whitespace().next())
        .filter_map(|s| s.parse::<i64>().ok())
        .collect();
    assert!(
        ids.starts_with(&[100, 101, 102, 103]),
        "expected phase to advance 100->101->102->103, got: {:?}\noutput: {:?}",
        ids,
        msgs
    );
    assert!(
        msgs.iter().any(|m| m.starts_with("DONE at 15")),
        "expected DONE at t=15, output: {:?}",
        msgs
    );
}

#[test]
fn mailbox_put_in_fork_unblocks_empty_mailbox_get() {
    // Focused check for bug 1 in isolation: a consumer parked on an empty
    // mailbox must be woken by a *separately forked* producer's put (the
    // eval_call_inner path). Pre-fix this deadlocked at t=0.
    let src = r#"
module top;
  mailbox mb = new();
  initial begin
    int x;
    fork
      begin
        #10;
        mb.put(42);
      end
    join_none
    mb.get(x);          // parks at t=0 (empty)
    $display("CONSUMER got %0d at %0t", x, $time);
  end
endmodule
"#;
    let sim = simulate(src, 200).expect("simulate failed");
    let msgs = messages(&sim);
    let got = msgs
        .iter()
        .find(|m| m.starts_with("CONSUMER got"))
        .unwrap_or_else(|| panic!("consumer never woke; output: {:?}", msgs));
    assert!(
        got.contains("got 42 at 10"),
        "expected consumer woke at t=10 with 42, got: {}",
        got
    );
}
