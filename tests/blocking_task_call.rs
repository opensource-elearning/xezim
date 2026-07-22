//! Blocking task/method calls honour their body's timing controls.
//!
//! IEEE 1800-2023 §13.5 (footnote 42) and §13.5.5: parentheses may be omitted
//! in a subroutine call for tasks, void functions, and class methods. xezim's
//! parser represents `t;` (no parens) as a bare `Expr(Ident([t]))`, not as
//! `Expr(Call { func: Ident([t]), args: [] })`. The suspend-aware executor's
//! blocking-task inlining guards (`Stage 1`/`1b`/`1c` in `run_process_stmts`)
//! all match on `Call`, so a blocking task invoked *without* parentheses
//! bypassed inlining and fell through to the synchronous `exec_statement`,
//! which cannot honour `#delay`/`@event`/`wait`/`fork` in the body — the body
//! was silently dropped and the caller never blocked:
//!
//! ```sv
//! task t; #10; endtask
//! initial begin t; $display("after @%0t", $time); end   // printed t=0 (!)
//! ```
//!
//! The fix adds `Stage 0`: it recognises a parenless subroutine reference that
//! resolves to a free task or a class method and rewrites it into the
//! equivalent zero-argument `Call` so the existing inlining guards fire
//! uniformly. These self-checking regressions pin that behaviour for every
//! parenless call shape:
//!
//!   * `t;`            — free task, parenless
//!   * `t();`          — free task, explicit parens (control: already worked)
//!   * `c.run;`        — method via dot, parenless
//!   * `c.run();`      — method via dot, explicit parens (control)
//!   * `m;` (this-method) — bare method name on the current `this`
//!
//! and that a non-blocking task keeps the synchronous path in both forms.

use xezim::simulate;

fn messages(sim: &xezim::compiler::Simulator) -> Vec<String> {
    sim.output.iter().map(|o| o.message.clone()).collect()
}

/// The minimal reproducer: a free task whose body is just `#10`, called
/// without parentheses. The caller must block for 10 time units.
/// Pre-fix: `t;` dropped the body and the continuation ran at t=0.
const FREE_TASK_PARENLESS: &str = r#"
task automatic stall10;
  #10;
endtask
module top;
  initial begin
    stall10;
    $display("AFTER t=%0t", $time);
  end
endmodule
"#;

#[test]
fn free_task_parenless_blocks_caller() {
    let sim = simulate(FREE_TASK_PARENLESS, 200).expect("simulate failed");
    let msgs = messages(&sim);
    let line = msgs
        .iter()
        .find(|m| m.starts_with("AFTER"))
        .unwrap_or_else(|| panic!("continuation never ran; output: {:?}", msgs));
    assert!(
        line.contains("t=10"),
        "expected caller blocked until t=10, got: {}\noutput: {:?}",
        line,
        msgs
    );
}

/// Control: the same task called *with* parentheses must behave identically.
const FREE_TASK_PARENS: &str = r#"
task automatic stall10;
  #10;
endtask
module top;
  initial begin
    stall10();
    $display("AFTER t=%0t", $time);
  end
endmodule
"#;

#[test]
fn free_task_parens_blocks_caller() {
    let sim = simulate(FREE_TASK_PARENS, 200).expect("simulate failed");
    let msgs = messages(&sim);
    let line = msgs
        .iter()
        .find(|m| m.starts_with("AFTER"))
        .unwrap_or_else(|| panic!("continuation never ran; output: {:?}", msgs));
    assert!(
        line.contains("t=10"),
        "expected caller blocked until t=10, got: {}\noutput: {:?}",
        line,
        msgs
    );
}

/// A class method task called without parentheses via dot syntax must block
/// the caller. Pre-fix `c.run;` parsed as `MemberAccess` and the body was
/// dropped, so the continuation ran at t=0.
const METHOD_DOT_PARENLESS: &str = r#"
class C;
  task run;
    $display("ENTER t=%0t", $time);
    #10;
    $display("DONE t=%0t", $time);
  endtask
endclass
module top;
  initial begin
    automatic C c = new;
    c.run;
    $display("AFTER t=%0t", $time);
  end
endmodule
"#;

#[test]
fn method_dot_parenless_blocks_caller() {
    let sim = simulate(METHOD_DOT_PARENLESS, 200).expect("simulate failed");
    let msgs = messages(&sim);
    let after = msgs
        .iter()
        .find(|m| m.starts_with("AFTER"))
        .unwrap_or_else(|| panic!("continuation never ran; output: {:?}", msgs));
    // The body must run (ENTER/DONE) and the caller must block until t=10.
    assert!(
        msgs.iter().any(|m| m.starts_with("ENTER t=0")),
        "method body never entered"
    );
    assert!(
        after.contains("t=10"),
        "expected caller blocked until t=10, got: {}\noutput: {:?}",
        after,
        msgs
    );
}

/// Control: the same method called *with* parentheses.
const METHOD_DOT_PARENS: &str = r#"
class C;
  task run;
    #10;
  endtask
endclass
module top;
  initial begin
    automatic C c = new;
    c.run();
    $display("AFTER t=%0t", $time);
  end
endmodule
"#;

#[test]
fn method_dot_parens_blocks_caller() {
    let sim = simulate(METHOD_DOT_PARENS, 200).expect("simulate failed");
    let msgs = messages(&sim);
    let line = msgs
        .iter()
        .find(|m| m.starts_with("AFTER"))
        .unwrap_or_else(|| panic!("continuation never ran; output: {:?}", msgs));
    assert!(
        line.contains("t=10"),
        "expected caller blocked until t=10, got: {}\noutput: {:?}",
        line,
        msgs
    );
}

/// A blocking method called by its bare name from within the class itself
/// (implicit `this`). The body's `wait` must suspend the caller.
const THIS_METHOD_PARENLESS: &str = r#"
class C;
  int gate;
  task automatic enter;
    wait(gate == 1);
  endtask
  task run;
    gate = 0;
    fork begin #5; gate = 1; end join_none
    enter;            // bare-name this-method, parenless, blocking
    $display("AFTER t=%0t gate=%0d", $time, gate);
  endtask
endclass
module top;
  initial begin
    automatic C c = new;
    c.run;
  end
endmodule
"#;

#[test]
fn this_method_parenless_blocks_caller() {
    let sim = simulate(THIS_METHOD_PARENLESS, 200).expect("simulate failed");
    let msgs = messages(&sim);
    let line = msgs
        .iter()
        .find(|m| m.starts_with("AFTER"))
        .unwrap_or_else(|| panic!("continuation never ran; output: {:?}", msgs));
    assert!(
        line.contains("t=5") && line.contains("gate=1"),
        "expected caller blocked until t=5 with gate=1, got: {}\noutput: {:?}",
        line,
        msgs
    );
}

/// A non-blocking task (no delay/wait/fork in its body) called without
/// parentheses must keep running — the Stage-0 rewrite yields a Call that does
/// not satisfy `stmts_have_blocking`, so it falls through to `exec_statement`
/// exactly like `nb();` with explicit parens.
const NONBLOCKING_PARENLESS: &str = r#"
task automatic greet;
  $display("HELLO");
endtask
module top;
  initial begin
    greet;
    $display("AFTER greet");
  end
endmodule
"#;

#[test]
fn nonblocking_task_parenless_runs_synchronously() {
    let sim = simulate(NONBLOCKING_PARENLESS, 200).expect("simulate failed");
    let msgs = messages(&sim);
    assert!(
        msgs.iter().any(|m| m == "HELLO"),
        "non-blocking body never ran"
    );
    assert!(
        msgs.iter().any(|m| m == "AFTER greet"),
        "continuation never ran; output: {:?}",
        msgs
    );
}
