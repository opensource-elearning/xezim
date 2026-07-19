//! Regression: a continuous assignment to a 2D/ND UNPACKED array element
//! whose OUTER index is 0 (`assign m[0][j] = ...`) was silently dropped.
//!
//! Root cause (compiler.rs `flattened_outer_zero_signal_id`): a genuine
//! unpacked 2D array `logic [7:0] m [2][2]` also carries a bogus scalar
//! signal named `m`. The bytecode LHS compiler's flattening short-circuit
//! fired for `m[0][j]` (outer index literal 0) and compiled it as a
//! bit-select write on that scalar — the element write never landed, so
//! `m[0][*]` read back X. `m[1][j]` (index != 0) bailed to the interpreter
//! and worked, so only the row-0 (and plane-0) writes were lost. Procedural
//! writes and 1D / packed-2D assigns were unaffected.
//!
//! Fix: the compiler now knows the set of 2D/ND unpacked-array base names
//! (`set_multi_dim_arrays`) and the short-circuit bails for them, matching
//! the non-zero-index path.

use xezim::simulate;

fn output_of(sim: &xezim::compiler::Simulator) -> String {
    sim.output
        .iter()
        .map(|o| o.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `assign m[0][j]` on a 2D unpacked array drives the element (was X).
#[test]
fn cont_assign_2d_row0_drives_element() {
    const SRC: &str = r#"
module top;
  logic [7:0] m [2][2];
  assign m[0][0] = 8'hA0;
  assign m[0][1] = 8'hA1;
  assign m[1][0] = 8'hB0;
  assign m[1][1] = 8'hB1;
  initial #1 $display("R %02h %02h %02h %02h", m[0][0], m[0][1], m[1][0], m[1][1]);
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    let out = output_of(&sim);
    assert!(
        out.contains("R a0 a1 b0 b1"),
        "row-0 cont-assign must drive the element (want R a0 a1 b0 b1):\n{}",
        out
    );
}

/// The same via a for-generate (the shape that surfaced it): nested genvar
/// loop writing `m[i][j]` — the i=0 row must not vanish.
#[test]
fn cont_assign_2d_in_nested_generate() {
    const SRC: &str = r#"
module top;
  logic [7:0] m [2][2];
  genvar i, j;
  generate for (i = 0; i < 2; i = i + 1) begin : r
    for (j = 0; j < 2; j = j + 1) begin : c
      assign m[i][j] = 8'h20 + i*2 + j;
    end
  end endgenerate
  initial #1 $display("G %02h %02h %02h %02h", m[0][0], m[0][1], m[1][0], m[1][1]);
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    let out = output_of(&sim);
    assert!(
        out.contains("G 20 21 22 23"),
        "nested-generate 2D cont-assign must fill every element (want G 20 21 22 23):\n{}",
        out
    );
}

/// 3D (arrays_nd) plane-0 element write must also land.
#[test]
fn cont_assign_3d_plane0_drives_element() {
    const SRC: &str = r#"
module top;
  logic [7:0] m [2][2][2];
  assign m[0][0][0] = 8'h11;
  assign m[0][1][0] = 8'h22;
  assign m[1][0][0] = 8'h33;
  assign m[0][0][1] = 8'h44;
  initial #1 $display("T %02h %02h %02h %02h",
                      m[0][0][0], m[0][1][0], m[1][0][0], m[0][0][1]);
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    let out = output_of(&sim);
    assert!(
        out.contains("T 11 22 33 44"),
        "3D plane-0 cont-assign must drive the element (want T 11 22 33 44):\n{}",
        out
    );
}
