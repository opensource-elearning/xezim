//! Sibling-shape audit of the July-2026 issue fixes: each earlier fix probed
//! at its analogous declaration sites / dispatch paths.
//!
//!   local `T a[][N]`     procedural dyn-outer + fixed-trailing dims
//!   `q[$][N]`            queue rows spread into `q[i][j]` element slots
//!   iface queue          hierarchical `tif.q.push_back` / registration
//!   generate final       genvar substitution into final blocks
//!   2-state array elems  zero defaults (top + inlined), foreach scope
//!   hier element R/W     MemberAccess-based `u_h.mem[q]` read AND write
//!   submodule comb       dep graph includes the scope-resolved read ids

use xezim::simulate;

const SRC: &str = r#"
module holder;
  bit [7:0] mem[4];
endmodule

module sub2s;
  bit [7:0] mem[4];
  int sum;
  initial begin
    #1;
    foreach (mem[i]) mem[i]++;
    sum = 0;
    foreach (mem[i]) sum += mem[i];
  end
endmodule

module combsub (input logic trig);
  int x;
  always_comb x = trig ? 7 : 1;
endmodule

module subgen;
  genvar g;
  generate for (g = 0; g < 2; g++) begin : gb
    final $display("GENFINAL=%0d", g);
  end endgenerate
endmodule

interface qif;
  int q[$];
endinterface

module tb;
  qif tif ();
  holder u_h ();
  sub2s u_s ();
  subgen u_g ();
  logic trig = 0;
  combsub u_c (.trig(trig));

  int lmb_fails, q2_fails, ifq_size, ifq0, hier_sum, comb_x;
  int q2_rows;
  initial begin
    // local dynamic-outer mailbox array with a fixed trailing dim
    begin
      mailbox #(int) lm[][2];
      int d;
      lm = new[3];
      foreach (lm[i,j]) lm[i][j] = new();
      foreach (lm[i,j]) lm[i][j].put(10*i+j);
      lmb_fails = 0;
      foreach (lm[i,j]) begin
        lm[i][j].get(d);
        if (d !== 10*i+j) lmb_fails++;
      end
    end
    // queue of fixed arrays: rows spread into q[i][j]
    begin
      int q[$][4];
      int row[4];
      row = '{1,2,3,4};  q.push_back(row);
      row = '{5,6,7,8};  q.push_back(row);
      q2_rows = q.size();
      q2_fails = 0;
      foreach (q[i,j]) if (q[i][j] !== 4*i+j+1) q2_fails++;
    end
    // interface queue through the hierarchical path
    #1;
    tif.q.push_back(7);
    tif.q.push_back(8);
    ifq_size = tif.q.size();
    ifq0 = tif.q[0];
    // MemberAccess-based element write AND read with a foreach var
    foreach (u_h.mem[k]) u_h.mem[k] = 8'(20 + k);
    hier_sum = 0;
    foreach (u_h.mem[k]) hier_sum += u_h.mem[k];
    // submodule always_comb re-fires when its port input changes
    trig = 1;
    #1;
    comb_x = u_c.x;
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
fn local_dyn_outer_mailbox_rows_round_trip() {
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_eq!(u(&sim, "lmb_fails"), 0);
}

#[test]
fn queue_of_fixed_arrays_spreads_rows_into_element_slots() {
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_eq!(u(&sim, "q2_rows"), 2);
    assert_eq!(u(&sim, "q2_fails"), 0);
}

#[test]
fn interface_queue_is_reachable_hierarchically() {
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_eq!(u(&sim, "ifq_size"), 2, "tif.q.push_back must grow the queue");
    assert_eq!(u(&sim, "ifq0"), 7);
}

#[test]
fn genvar_reaches_final_blocks_inside_generate() {
    let sim = simulate(SRC, 1000).expect("simulate failed");
    let outs: Vec<String> = sim.output.iter().map(|o| o.message.clone()).collect();
    assert!(outs.iter().any(|m| m.contains("GENFINAL=0")), "{:?}", outs);
    assert!(outs.iter().any(|m| m.contains("GENFINAL=1")), "{:?}", outs);
}

#[test]
fn two_state_elements_and_foreach_work_inside_submodules() {
    let sim = simulate(SRC, 1000).expect("simulate failed");
    // 4 elements zero-init'd then ++ once each.
    assert_eq!(u(&sim, "u_s.sum"), 4, "bit[7:0] mem[4] must count 0->1 each");
}

#[test]
fn hierarchical_element_write_and_read_with_loop_var() {
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_eq!(u(&sim, "hier_sum"), 20 + 21 + 22 + 23);
}

#[test]
fn submodule_always_comb_refires_on_port_change() {
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_eq!(u(&sim, "comb_x"), 7, "comb block must re-fire after trig=1");
}
