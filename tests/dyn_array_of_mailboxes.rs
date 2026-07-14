//! Issue #17 follow-ups: mailbox aggregates that dropped data.
//!
//!   mailbox mbx[][16]     dynamic OUTER dim with a fixed trailing dim — the
//!                         `[16]` was dropped, foreach bound only `i` (the
//!                         inner var stayed X) and every element read 0.
//!   mailbox mb[2][4]      fixed 2-D array — `infer_lhs_width` returned 1 for
//!                         a nested-index lvalue, so `mb[i][j] = new()`
//!                         truncated the handle to a single bit.
//!   iface.mbox.put(x)     a container reached through a flattened
//!                         hierarchical path dispatched on `path[0]` and the
//!                         put/get was a silent no-op.

use xezim::simulate;

const DYN_OUTER: &str = r#"
module tb;
  mailbox #(int) mbx_array[][16];
  int fails;
  int iters;
  initial begin
    mbx_array = new[5];
    foreach (mbx_array[i, j]) mbx_array[i][j] = new();
    foreach (mbx_array[i, x]) begin
      int data;
      data = i + x;
      mbx_array[i][x].put(data);
    end
    foreach (mbx_array[i, x]) begin
      int data;
      mbx_array[i][x].get(data);
      if (data !== i + x) fails++;
      iters++;
    end
  end
endmodule
"#;

const FIXED_2D: &str = r#"
module tb;
  mailbox #(int) mb[2][4];
  int fails;
  int distinct;
  initial begin
    foreach (mb[i, j]) mb[i][j] = new();
    // Handles must be distinct non-null objects (they were 1-bit truncated).
    begin
      int seen[$];
      foreach (mb[i, j]) begin
        if (mb[i][j] == null) fails++;
        foreach (seen[k]) if (seen[k] == mb[i][j]) fails++;
        seen.push_back(mb[i][j]);
      end
      distinct = seen.size();
    end
    foreach (mb[i, j]) mb[i][j].put(10 * i + j);
    foreach (mb[i, j]) begin
      int data;
      mb[i][j].get(data);
      if (data !== 10 * i + j) fails++;
    end
  end
endmodule
"#;

const IFACE_MBOX: &str = r#"
interface test_if;
  mailbox #(int) ping_mbox;
  initial ping_mbox = new();
endinterface

module tb;
  test_if tif ();
  int got;
  int ok;
  int n;
  initial begin
    #1;
    ok = tif.ping_mbox.try_put(42);
    n = tif.ping_mbox.num();
    tif.ping_mbox.get(got);
  end
endmodule
"#;

fn u(sim: &xezim::compiler::Simulator, n: &str) -> u64 {
    sim.get_signal(n)
        .or_else(|| sim.get_signal(&format!("tb.{}", n)))
        .unwrap_or_else(|| panic!("signal not found: {}", n))
        .to_u64()
        .unwrap_or_else(|| panic!("{} not u64-able", n))
        & 0xFFFF_FFFF
}

#[test]
fn dynamic_array_of_fixed_mailbox_rows_round_trips_every_element() {
    let sim = simulate(DYN_OUTER, 100).expect("simulate failed");
    assert_eq!(u(&sim, "iters"), 80, "foreach must visit 5 x 16 elements");
    assert_eq!(u(&sim, "fails"), 0, "every mailbox must return its own datum");
}

#[test]
fn fixed_2d_mailbox_array_elements_hold_distinct_handles() {
    let sim = simulate(FIXED_2D, 100).expect("simulate failed");
    assert_eq!(u(&sim, "distinct"), 8, "expected 2 x 4 distinct mailboxes");
    assert_eq!(u(&sim, "fails"), 0, "handle or data mismatch");
}

#[test]
fn mailbox_inside_an_interface_is_reachable_hierarchically() {
    let sim = simulate(IFACE_MBOX, 100).expect("simulate failed");
    assert_eq!(u(&sim, "ok"), 1, "try_put through tif.ping_mbox must succeed");
    assert_eq!(u(&sim, "n"), 1, "num() must see the queued item");
    assert_eq!(u(&sim, "got"), 42, "get must return the queued value");
}
