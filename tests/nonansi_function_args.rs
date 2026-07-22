//! §13.4.2: non-ANSI function/task argument binding. The body `input`/`output`
//! declarations (`function f; input int x; …`) were parsed to Null statements
//! and only their names kept for the strict-check — `fd.ports` stayed EMPTY, so
//! arguments never bound and every non-ANSI function's inputs read X. Now the
//! body port declarations populate `ports`, so binding works.

use xezim::simulate;

fn out_of(sim: &xezim::compiler::Simulator) -> String {
    sim.output
        .iter()
        .map(|o| o.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn nonansi_function_arg_binding() {
    const SRC: &str = r#"
module top;
  function integer add2; input integer x; input integer y; add2 = x + y; endfunction
  function integer w; input [6:0] a; input [3:0] b; w = a + b; endfunction
  function real rdiv; input [6:0] n; rdiv = 1000.0 / (n + 1); endfunction
  initial begin
    $display("A %0d", add2(3, 5));           // 8
    $display("W %0d", w(7'd32, 4'd15));       // 47
    $display("R %.3f", rdiv(7'd9));           // 100.000
  end
endmodule
"#;
    let out = out_of(&simulate(SRC, 100).expect("sim"));
    assert!(out.contains("A 8"), "non-ANSI 2-arg int:\n{}", out);
    assert!(out.contains("W 47"), "non-ANSI mixed-width:\n{}", out);
    assert!(out.contains("R 100.000"), "non-ANSI real return:\n{}", out);
}

/// A non-ANSI function with a body `int i;` local *after* the input port (the
/// br962 shape) — the local must not be treated as a port, and the input binds.
#[test]
fn nonansi_function_with_body_local() {
    const SRC: &str = r#"
module top;
  function integer f; input [7:0] data; int i; begin i = data; f = i + 1; end endfunction
  initial $display("F %0d", f(8'd41));   // 42
endmodule
"#;
    let out = out_of(&simulate(SRC, 100).expect("sim"));
    assert!(
        out.contains("F 42"),
        "non-ANSI fn with body local:\n{}",
        out
    );
}
