//! §23.3.2 arrays of module instances: `m[N:0] (vec[N:0], scalar, ...)` must
//! expand into N instances with vector ports bit-distributed (element k gets
//! bit k) and scalar ports broadcast. Previously the `[N:0]` was ignored, a
//! whole vector landed on a scalar port, and nothing connected — arrayed flop
//! banks (a common config/PLL idiom) silently did nothing.

use xezim::simulate;

fn out_of(sim: &xezim::compiler::Simulator) -> String {
    sim.output
        .iter()
        .map(|o| o.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn comb_array_bit_distributes() {
    const SRC: &str = r#"
module cmb(output logic q, input logic d); assign q = d; endmodule
module top;
  logic [3:0] out, din;
  cmb m[3:0] (out[3:0], din[3:0]);
  initial begin din = 4'hA; #1 $display("R %h", out); end
endmodule
"#;
    let out = out_of(&simulate(SRC, 100).expect("sim"));
    assert!(
        out.contains("R a"),
        "each element drives out[k]=din[k]:\n{}",
        out
    );
}

#[test]
fn ff_array_with_scalar_clock_broadcast() {
    const SRC: &str = r#"
module dff(output logic q, input logic clk, input logic d);
  always_ff @(posedge clk) q <= d;
endmodule
module top;
  logic clk; logic [3:0] out, din;
  dff m[3:0] (out[3:0], clk, din[3:0]);   // clk broadcast, out/din distribute
  initial begin
    clk = 0; din = 4'hC;
    #1 clk = 1; #1 clk = 0;
    #1 $display("F %h", out);
  end
endmodule
"#;
    let out = out_of(&simulate(SRC, 100).expect("sim"));
    assert!(
        out.contains("F c"),
        "flop bank latches din per bit (want F c):\n{}",
        out
    );
}

/// Non-zero-based / offset range: `m[3:1]` connected to `out[3:1]` must map
/// element to the ABSOLUTE bit (out[1..3]), not a nested select out[3:1][k].
#[test]
fn array_offset_range_absolute_bit() {
    const SRC: &str = r#"
module cmb(output logic q, input logic d); assign q = d; endmodule
module top;
  logic [3:0] out, din;
  cmb a0 (out[0], din[0]);
  cmb m[3:1] (out[3:1], din[3:1]);
  initial begin din = 4'hA; #1 $display("O %b", out); end
endmodule
"#;
    let out = out_of(&simulate(SRC, 100).expect("sim"));
    assert!(
        out.contains("O 1010"),
        "offset-range array drives absolute bits:\n{}",
        out
    );
}
