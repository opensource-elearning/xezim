//! §13.4.1: a function's return value takes the function's declared type. A
//! `function signed [7:0]` must truncate its result to 8 bits (150 -> -106); a
//! `function [3:0]` to 4 bits. The return variable was sized 32 and never
//! narrowed, so `f = a + b` returned the full-width sum — which broke self-
//! checking tests that compared a byte add against a narrow-return golden
//! function (ivtest sbyte_test / sshortint_test / ubyte_test / ushortint_test,
//! masked while $random was stuck at 0).

use xezim::simulate;

fn output_of(sim: &xezim::compiler::Simulator) -> String {
    sim.output
        .iter()
        .map(|o| o.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn function_return_truncates_to_declared_width() {
    const SRC: &str = r#"
module top;
  function signed [7:0] s_sum(input signed [7:0] a, b); s_sum = a + b; endfunction
  function byte signed  b_sum(input byte signed a, b);  b_sum = a + b; endfunction
  function [3:0]        u_nib(input [7:0] x);           u_nib = x;     endfunction
  function int          i_sum(input int a, b);          i_sum = a + b; endfunction
  initial begin
    $display("S %0d", s_sum(8'sd100, 8'sd50));   // 150 -> -106
    $display("B %0d", b_sum(8'sd100, 8'sd50));   // -106
    $display("N %0d", u_nib(8'hF5));             // 0xF5 -> 0x5 = 5
    $display("I %0d", i_sum(1000, 2000));        // 3000 (32-bit unaffected)
  end
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    let out = output_of(&sim);
    assert!(
        out.contains("S -106"),
        "signed[7:0] return must wrap to 8 bits:\n{}",
        out
    );
    assert!(
        out.contains("B -106"),
        "byte signed return must wrap:\n{}",
        out
    );
    assert!(
        out.contains("N 5"),
        "[3:0] return must truncate to 4 bits:\n{}",
        out
    );
    assert!(
        out.contains("I 3000"),
        "int return must be unaffected:\n{}",
        out
    );
}
