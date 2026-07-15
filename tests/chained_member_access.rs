//! Method calls through chained member access resolve to the right object.
//!
//! IEEE 1800-2023 §8.15/§13.5.5: a method reached through a multi-level
//! member-access path — `o.in.getval()`, `h.q.put(x)` — must dispatch on the
//! FULL receiver prefix (`o.in`, `h.q`), binding `this` to the object that
//! actually OWNS the method. xezim's parser flattens a dotted call into a
//! single `Call{ func: Ident([seg0, seg1, ..., method]) }`. The flattened-
//! Ident dispatch in `eval_call_inner` resolved only `path[0]` (the OUTERMOST
//! object) and looked the method up there, ignoring the intermediate segments.
//! For `o.in.getval()` that bound `this` to `o` (Outer has no `getval`) and
//! silently returned 0; `o.in.setval(x)` wrote nothing. The fix walks the
//! receiver prefix `path[0..len-1]` through the heap (each intermediate
//! segment is a class-handle property) to reach the owning instance.
//!
//! These self-checking regressions pin the fix for nested-object method
//! dispatch (read, write, and queue operations through 2+ member levels):

use xezim::simulate;

fn messages(sim: &xezim::compiler::Simulator) -> Vec<String> {
    sim.output.iter().map(|o| o.message.clone()).collect()
}

fn assert_pass(sim: &xezim::compiler::Simulator, tag: &str) {
    let msgs = messages(sim);
    let pass = msgs.iter().any(|m| m.contains(&format!("{tag}_PASS")));
    let fail = msgs.iter().find(|m| m.contains(&format!("{tag}_FAIL")));
    assert!(
        pass,
        "expected {tag}_PASS in output\nfail line: {fail:?}\nfull output: {msgs:?}"
    );
}

/// Two classes: `Outer` holds an `Inner` member (a class handle). A method
/// defined on `Inner` is called through the 2-level path `o.in.method()`.
const INNER_CLASS: &str = r#"
class Inner;
  int val;
  function void setval(int v); val = v; endfunction
  function int getval(); return val; endfunction
endclass
class Outer;
  Inner in;
  function new; in = new; endfunction
endclass
module top;
  Outer o;
  initial begin
    o = new;
    o.in.val = 42;
    if (o.in.getval() == 42) $display("READ_PASS getval=%0d", o.in.getval());
    else                      $display("READ_FAIL getval=%0d", o.in.getval());
  end
endmodule
"#;

/// A method WRITE (`setval`) through `o.in` must land on the Inner object
/// and be readable back via the plain field.
#[test]
fn nested_method_read() {
    let sim = simulate(INNER_CLASS, 200).expect("simulate failed");
    assert_pass(&sim, "READ");
}

const NESTED_WRITE: &str = r#"
class Inner;
  int val;
  function void setval(int v); val = v; endfunction
endclass
class Outer;
  Inner in;
  function new; in = new; endfunction
endclass
module top;
  Outer o;
  initial begin
    o = new;
    o.in.setval(77);
    if (o.in.val == 77) $display("WRITE_PASS val=%0d", o.in.val);
    else                $display("WRITE_FAIL val=%0d", o.in.val);
  end
endmodule
"#;

#[test]
fn nested_method_write() {
    let sim = simulate(NESTED_WRITE, 200).expect("simulate failed");
    assert_pass(&sim, "WRITE");
}

/// write via method then read via method — both through the 2-level path —
/// confirming `this` is consistently the Inner object.
const ROUNDTRIP: &str = r#"
class Inner;
  int val;
  function void setval(int v); val = v; endfunction
  function int getval(); return val; endfunction
endclass
class Outer;
  Inner in;
  function new; in = new; endfunction
endclass
module top;
  Outer o;
  initial begin
    o = new;
    o.in.setval(99);
    if (o.in.getval() == 99) $display("RT_PASS getval=%0d", o.in.getval());
    else                     $display("RT_FAIL getval=%0d", o.in.getval());
  end
endmodule
"#;

#[test]
fn nested_method_roundtrip() {
    let sim = simulate(ROUNDTRIP, 200).expect("simulate failed");
    assert_pass(&sim, "RT");
}

/// A queue wrapped in a class (`Q`), itself held as a member of another
/// class (`Hopper`). `put`/`sz`/`popf` are called through the 2-level path
/// `h.q.method()`. This is the shape of the UVM phase hopper's mailbox/queue
/// handoff and was the dominant STALL blocker.
const HOPPER_QUEUE: &str = r#"
class Q;
  int items[$];
  function void put(int ph); items.push_back(ph); endfunction
  function int sz(); return items.size(); endfunction
  function int popf(); return items.pop_front(); endfunction
endclass
class Hopper;
  Q q;
  function new; q = new; endfunction
endclass
module top;
  initial begin
    automatic Hopper h = new;
    h.q.put(10);
    h.q.put(20);
    if (h.q.sz() == 2 && h.q.popf() == 10)
      $display("HOPPER_PASS sz=%0d", h.q.sz());
    else
      $display("HOPPER_FAIL sz=%0d", h.q.sz());
  end
endmodule
"#;

#[test]
fn nested_queue_methods() {
    let sim = simulate(HOPPER_QUEUE, 200).expect("simulate failed");
    assert_pass(&sim, "HOPPER");
}

/// The full hopper pattern: a forever-loop reading phases from a queue and
/// forking a delayed handler per phase, with the parent `wait`-ing on a
/// shared counter incremented by the fork children. This combines the
/// chained-member-access fix with the §6.21 fork-variable write-back and
/// the §9.7.4 level-sensitive `wait()`.
const FULL_HOPPER: &str = r#"
class Q;
  int items[$];
  task get(output int ph);
    wait(items.size() != 0);
    ph = items.pop_front();
  endtask
  function void put(int ph); items.push_back(ph); endfunction
endclass
class Hopper;
  Q q;
  int phases_done;
  task run_phases;
    int ph;
    fork
      begin
        forever begin
          q.get(ph);
          fork
            automatic int phase = ph;
            begin
              #phase;
              this.phases_done = this.phases_done + 1;
            end
          join_none
        end
      end
    join_none
    wait(phases_done > 0);
  endtask
  function new; q = new; phases_done = 0; endfunction
endclass
module top;
  initial begin
    automatic Hopper h = new;
    h.q.put(10);
    h.run_phases;
    if (h.phases_done >= 1) $display("FH_PASS phases_done=%0d at=%0t", h.phases_done, $time);
    else                    $display("FH_FAIL phases_done=%0d at=%0t", h.phases_done, $time);
  end
endmodule
"#;

#[test]
fn full_hopper_pattern() {
    let sim = simulate(FULL_HOPPER, 2000).expect("simulate failed");
    assert_pass(&sim, "FH");
}
