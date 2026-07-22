//! §10.6 force / release / assign / deassign against edge-triggered blocks.
//!
//! Two fixes: (1) the compiled VM's blocking-assign fast path wrote signal_table
//! directly, bypassing the forced-target guard, so an `always @(posedge clk)
//! q=d` overrode a `force`/`assign` on q. (2) after `release`/`deassign`, a
//! gateable flop whose data input hadn't changed was skipped by the EVENT_EDGE
//! optimization (its "Q unchanged" invariant was broken by the force), leaving Q
//! stuck at the forced value — fixed by invalidating flop snapshots on release.

use xezim::simulate;

fn output_of(sim: &xezim::compiler::Simulator) -> String {
    sim.output
        .iter()
        .map(|o| o.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `force` overrides a blocking-assign flop, and `release` lets it resume.
#[test]
fn force_overrides_and_release_restores_blocking_flop() {
    const SRC: &str = r#"
module top;
  reg clk, d, q;
  always @(posedge clk) q = d;
  initial begin
    clk=0; d=0; q=0;
    force q = 1;
    d=0; clk=0; #1 clk=1; #1 clk=0;
    $display("F %b", q);                 // 1: force wins over the flop
    release q;
    d=0; clk=0; #1 clk=1; #1 clk=0;
    $display("R %b", q);                 // 0: flop resumes, q<=d=0
  end
endmodule
"#;
    let out = output_of(&simulate(SRC, 100).expect("sim"));
    assert!(
        out.contains("F 1"),
        "force must override the flop:\n{}",
        out
    );
    assert!(
        out.contains("R 0"),
        "release must let the flop resume:\n{}",
        out
    );
}

/// Same for a non-blocking flop and `assign`/`deassign`.
#[test]
fn assign_overrides_and_deassign_restores_nba_flop() {
    const SRC: &str = r#"
module top;
  reg clk, d, q;
  always @(posedge clk) q <= d;
  initial begin
    clk=0; d=0; q=0;
    assign q = 1;
    d=0; clk=0; #1 clk=1; #1 clk=0;
    $display("A %b", q);                 // 1: assign wins
    deassign q;
    d=0; clk=0; #1 clk=1; #1 clk=0;
    $display("D %b", q);                 // 0: flop resumes
  end
endmodule
"#;
    let out = output_of(&simulate(SRC, 100).expect("sim"));
    assert!(
        out.contains("A 1"),
        "assign must override the flop:\n{}",
        out
    );
    assert!(
        out.contains("D 0"),
        "deassign must let the flop resume:\n{}",
        out
    );
}
