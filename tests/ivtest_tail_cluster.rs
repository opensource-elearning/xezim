//! Regression ratchets for the ivtest "hard tail" cluster. Each embeds a
//! self-checking SystemVerilog source (the ivtest sources themselves are not
//! vendored) and asserts the "PASSED" marker with no "FAILED".

use xezim::simulate;

fn passes(src: &str) -> bool {
    match simulate(src, 100_000) {
        Ok(sim) => {
            let out: String = sim
                .output
                .iter()
                .map(|o| o.message.clone())
                .collect::<Vec<_>>()
                .join("\n");
            out.contains("PASSED") && !out.contains("FAILED")
        }
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// §6.21 / §13.3.1: `static` locals inside a subroutine keep ONE persistent
// storage across all runtime calls; their initializer runs only once.
// Elaboration-time constant-function calls (localparam inits) are independent.
// ---------------------------------------------------------------------------

#[test]
fn func_init_var1() {
    assert!(passes(
        r#"
module test();
function integer accumulate1(input integer value);
  static int acc = 1;
  acc = acc + value;
  return acc;
endfunction
function automatic integer accumulate2(input integer value);
  int acc = 1;
  acc = acc + value;
  return acc;
endfunction
localparam value1 = accumulate1(2);
localparam value2 = accumulate1(3);
localparam value3 = accumulate2(2);
localparam value4 = accumulate2(3);
integer value;
reg failed = 0;
initial begin
  $display("%d", value1); if (value1 !== 3) failed = 1;
  $display("%d", value2); if (value2 !== 4) failed = 1;
  $display("%d", value3); if (value3 !== 3) failed = 1;
  $display("%d", value4); if (value4 !== 4) failed = 1;
  value = accumulate1(2); $display("%d", value); if (value !== 3) failed = 1;
  value = accumulate1(3); $display("%d", value); if (value !== 6) failed = 1;
  value = accumulate2(2); $display("%d", value); if (value !== 3) failed = 1;
  value = accumulate2(3); $display("%d", value); if (value !== 4) failed = 1;
  if (failed) $display("FAILED"); else $display("PASSED");
end
endmodule
"#
    ));
}

#[test]
fn func_init_var2_named_block_static() {
    assert!(passes(
        r#"
module static test();
function integer accumulate1(input integer value);
begin:blk
  static int acc = 1;
  acc = acc + value;
  return acc;
end
endfunction
function automatic integer accumulate2(input integer value);
begin:blk
  automatic int acc = 1;
  acc = acc + value;
  return acc;
end
endfunction
localparam value1 = accumulate1(2);
localparam value2 = accumulate1(3);
localparam value3 = accumulate2(2);
localparam value4 = accumulate2(3);
integer value;
initial begin
  static reg failed = 0;
  $display("%d", value1); if (value1 !== 3) failed = 1;
  $display("%d", value2); if (value2 !== 4) failed = 1;
  $display("%d", value3); if (value3 !== 3) failed = 1;
  $display("%d", value4); if (value4 !== 4) failed = 1;
  value = accumulate1(2); $display("%d", value); if (value !== 3) failed = 1;
  value = accumulate1(3); $display("%d", value); if (value !== 6) failed = 1;
  value = accumulate2(2); $display("%d", value); if (value !== 3) failed = 1;
  value = accumulate2(3); $display("%d", value); if (value !== 4) failed = 1;
  if (failed) $display("FAILED"); else $display("PASSED");
end
endmodule
"#
    ));
}

#[test]
fn func_init_var3_automatic_module() {
    assert!(passes(
        r#"
module automatic test();
function static integer accumulate1(input integer value);
  static int acc = 1;
  acc = acc + value;
  return acc;
endfunction
function integer accumulate2(input integer value);
  int acc = 1;
  acc = acc + value;
  return acc;
endfunction
localparam value1 = accumulate1(2);
localparam value2 = accumulate1(3);
localparam value3 = accumulate2(2);
localparam value4 = accumulate2(3);
integer value;
reg failed = 0;
initial begin
  $display("%d", value1); if (value1 !== 3) failed = 1;
  $display("%d", value2); if (value2 !== 4) failed = 1;
  $display("%d", value3); if (value3 !== 3) failed = 1;
  $display("%d", value4); if (value4 !== 4) failed = 1;
  value = accumulate1(2); $display("%d", value); if (value !== 3) failed = 1;
  value = accumulate1(3); $display("%d", value); if (value !== 6) failed = 1;
  value = accumulate2(2); $display("%d", value); if (value !== 3) failed = 1;
  value = accumulate2(3); $display("%d", value); if (value !== 4) failed = 1;
  if (failed) $display("FAILED"); else $display("PASSED");
end
endmodule
"#
    ));
}

#[test]
fn task_init_var1() {
    assert!(passes(
        r#"
module test();
task accumulate1(input integer value, output integer result);
  static int acc = 1;
  acc = acc + value;
  result = acc;
endtask
task automatic accumulate2(input integer value, output integer result);
  int acc = 1;
  acc = acc + value;
  result = acc;
endtask
integer value;
reg failed = 0;
initial begin
  accumulate1(2, value); $display("%d", value); if (value !== 3) failed = 1;
  accumulate1(3, value); $display("%d", value); if (value !== 6) failed = 1;
  accumulate2(2, value); $display("%d", value); if (value !== 3) failed = 1;
  accumulate2(3, value); $display("%d", value); if (value !== 4) failed = 1;
  if (failed) $display("FAILED"); else $display("PASSED");
end
endmodule
"#
    ));
}

#[test]
fn task_init_var2_named_block_static() {
    assert!(passes(
        r#"
module static test();
task accumulate1(input integer value, output integer result);
begin:blk
  static int acc = 1;
  acc = acc + value;
  result = acc;
end
endtask
task automatic accumulate2(input integer value, output integer result);
begin:blk
  int acc = 1;
  acc = acc + value;
  result = acc;
end
endtask
integer value;
initial begin
  static reg failed = 0;
  accumulate1(2, value); $display("%d", value); if (value !== 3) failed = 1;
  accumulate1(3, value); $display("%d", value); if (value !== 6) failed = 1;
  accumulate2(2, value); $display("%d", value); if (value !== 3) failed = 1;
  accumulate2(3, value); $display("%d", value); if (value !== 4) failed = 1;
  if (failed) $display("FAILED"); else $display("PASSED");
end
endmodule
"#
    ));
}

#[test]
fn task_init_var3_automatic_module() {
    assert!(passes(
        r#"
module automatic test();
task static accumulate1(input integer value, output integer result);
  static int acc = 1;
  acc = acc + value;
  result = acc;
endtask
task accumulate2(input integer value, output integer result);
  int acc = 1;
  acc = acc + value;
  result = acc;
endtask
integer value;
reg failed = 0;
initial begin
  accumulate1(2, value); $display("%d", value); if (value !== 3) failed = 1;
  accumulate1(3, value); $display("%d", value); if (value !== 6) failed = 1;
  accumulate2(2, value); $display("%d", value); if (value !== 3) failed = 1;
  accumulate2(3, value); $display("%d", value); if (value !== 4) failed = 1;
  if (failed) $display("FAILED"); else $display("PASSED");
end
endmodule
"#
    ));
}
