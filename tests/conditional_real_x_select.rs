//! §11.3.1/§11.4.11 — a conditional `cond ? a : b` whose result is REAL and
//! whose `cond` is X/Z must return a defined real value, NOT a per-bit merge of
//! the operands' IEEE-754 bit patterns (which produced garbage like `4.65e18`
//! for `1000.0`). Surfaced by a PLL model: `sel ? vcofbperiod : vcofb*mult_out`
//! with `sel`=X made `plloutperiod` a nonsense clock period.

use xezim::simulate;

fn out(src: &str) -> String {
    let sim = simulate(src, 1000).expect("simulate failed");
    sim.output
        .iter()
        .map(|o| o.message.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn real_conditional_with_x_condition_is_a_branch_not_garbage() {
    let o = out(r#"
module t;
  reg sel; real a = 1000.0, mout = 2.0, r;
  initial begin
    sel = 1'bx; r = sel ? a : a*mout; $display("X=%g", r);   // else branch -> 2000
    sel = 1'b1; r = sel ? a : a*mout; $display("T=%g", r);   // 1000
    sel = 1'b0; r = sel ? a : a*mout; $display("F=%g", r);   // 2000
    $finish;
  end
endmodule
"#);
    assert!(
        o.contains("X=2000"),
        "x-select real must be a defined branch (2000), not bit-garbage; got: {}",
        o
    );
    assert!(
        o.contains("T=1000") && o.contains("F=2000"),
        "known selects still correct; got: {}",
        o
    );
}

/// An integral conditional with an X condition still does the per-bit merge.
#[test]
fn integral_conditional_with_x_still_merges() {
    let o = out(r#"
module t;
  reg sel; reg [3:0] a=4'b1100, b=4'b1010, r;
  initial begin sel=1'bx; r = sel ? a : b; $display("R=%b", r); $finish; end
endmodule
"#);
    assert!(
        o.contains("R=1xx0"),
        "integral x-select keeps per-bit merge; got: {}",
        o
    );
}
