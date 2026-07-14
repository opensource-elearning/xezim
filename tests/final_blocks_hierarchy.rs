//! Issue #22: `final` procedures across the hierarchy (§9.2.3), and 2-state
//! defaults inside inlined submodules (§6.8).
//!
//!   - Submodule `final` blocks were dropped at inlining, so only the top
//!     module's finals ever executed.
//!   - An inlined submodule's variable declarations were created with an
//!     all-X default even for 2-state types, so a `bit [15:0]` counter
//!     started at X and `cnt++` stayed X forever.

use xezim::simulate;

const SRC: &str = r#"
module submodule_A (input logic clk);
  bit [15:0] transaction_count;
  always_ff @(posedge clk) transaction_count++;
  final $display("FINAL_A");
endmodule

module submodule_B;
  final $display("FINAL_B1");
  final $display("FINAL_B2");
endmodule

module tb_top;
  bit clk = 0;
  bit [15:0] seen_count;
  submodule_A u_sub_a (.clk(clk));
  submodule_B u_sub_b ();
  always #5 clk = ~clk;
  initial begin
    repeat (4) @(posedge clk);
    #0;
    seen_count = u_sub_a.transaction_count;
    $finish;
  end
  final $display("FINAL_TOP");
endmodule
"#;

fn has_line(sim: &xezim::compiler::Simulator, tag: &str) -> bool {
    sim.output.iter().any(|o| o.message.contains(tag))
}

#[test]
fn final_blocks_run_in_every_module_of_the_hierarchy() {
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert!(has_line(&sim, "FINAL_TOP"), "top-level final must run");
    assert!(has_line(&sim, "FINAL_A"), "submodule A final must run");
    assert!(has_line(&sim, "FINAL_B1"), "submodule B final 1 must run");
    assert!(has_line(&sim, "FINAL_B2"), "submodule B final 2 must run");
}

#[test]
fn two_state_declarations_in_submodules_default_to_zero() {
    let sim = simulate(SRC, 1000).expect("simulate failed");
    let v = sim
        .get_signal("seen_count")
        .or_else(|| sim.get_signal("tb_top.seen_count"))
        .expect("seen_count not found")
        .to_u64()
        .expect("seen_count is X — the submodule bit counter never left X");
    assert_eq!(v & 0xFFFF, 4, "4 posedges must count to 4");
}
