//! IEEE 1800-2017 timescale: §3.14 (units/precision), §20.3 ($time/$realtime
//! scaled to the calling module's unit), §21.2.1.4 ($printtimescale),
//! §21.3.5 ($timeformat / %t).
//!
//! Before these fixes:
//!   - `$time`/`$realtime` always reported nanoseconds, ignoring the module's
//!     time unit (`timescale 1us/1ns` → `$time` == 5000, should be 5).
//!   - a `timeunit`/`timeprecision` DECLARATION did not scale delays at all
//!     (`#5` in a `timeunit 1us` module ran as 5 ns, not 5 us).
//!   - `$timeformat` was ignored by `%t`, and `%t` of a real printed a
//!     float artifact (`42.00000000000001`).
//!   - `$printtimescale` produced no output.

use xezim::simulate;

fn out(src: &str) -> String {
    let sim = simulate(src, 10_000_000).expect("simulate failed");
    sim.output.iter().map(|o| o.message.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn time_scales_to_the_module_unit() {
    // `#5` in a 1us module is 5us; $time in us units is 5.
    let o = out(r#"
`timescale 1us/1ns
module m; initial begin #5; $display("T=%0d R=%0f", $time, $realtime); end endmodule
"#);
    assert!(o.contains("T=5 R=5.000000"), "got: {}", o);
}

#[test]
fn time_scales_to_a_ten_ns_unit() {
    // `#5` in a 10ns module is 50ns; $time in 10ns units is 5.
    let o = out(r#"
`timescale 10ns/1ns
module m; initial begin #5; $display("T=%0d", $time); end endmodule
"#);
    assert!(o.contains("T=5"), "got: {}", o);
}

#[test]
fn each_module_reports_time_in_its_own_unit() {
    let o = out(r#"
`timescale 1ns/1ns
module fast; initial begin #100; $display("F=%0d", $time); end endmodule
`timescale 1us/1us
module slow; initial begin #1; $display("S=%0d", $time); end endmodule
module top; fast f(); slow s(); endmodule
"#);
    assert!(o.contains("F=100"), "fast should read 100 ns: {}", o);
    assert!(o.contains("S=1"), "slow should read 1 us: {}", o);
}

#[test]
fn a_timeunit_declaration_scales_delays() {
    // The 1us module's `#5` (5us = 5000ns) must fire between the 1ns
    // reference's #10 and #10010, proving the delay was scaled — not run as 5ns.
    let o = out(r#"
`timescale 1ns/1ns
module top; initial begin #10; $display("R10"); #10000; $display("R10010"); end endmodule
module u; timeunit 1us; timeprecision 1ns; initial begin #5; $display("UDONE"); end endmodule
module wrap; top t(); u uu(); endmodule
"#);
    let i10 = o.find("R10").unwrap();
    let iu = o.find("UDONE").expect("UDONE missing");
    let i10010 = o.find("R10010").unwrap();
    assert!(i10 < iu && iu < i10010, "UDONE must land between R10 and R10010: {}", o);
}

#[test]
fn timeformat_is_honoured_by_percent_t() {
    let o = out(r#"
`timescale 1ns/1ns
module m; initial begin
  $timeformat(-9, 3, "ns", 10);
  #42;
  $display("A[%t]", $realtime);
  $timeformat(-6, 3, " us", 0);
  #1458;                 // now at 1500 ns
  $display("B[%t]", $realtime);
end endmodule
"#);
    // "42.000ns" is 8 chars; width 10 → 2 leading spaces.
    assert!(o.contains("A[  42.000ns]"), "3-decimal, width-10, suffix: {}", o);
    assert!(o.contains("B[1.500 us]"), "1500ns shown as 1.500 us: {}", o);
}

#[test]
fn percent_t_has_no_float_artifact() {
    let o = out(r#"
`timescale 1ns/1ns
module m; initial begin #42; $display("[%t]", $realtime); end endmodule
"#);
    // §21.3.5: with no $timeformat call the %t minimum field width is 20,
    // so a clean "42" arrives right-justified in a 20-char field.
    assert!(
        o.contains(&format!("[{:>20}]", "42")),
        "default %t must print a clean 42 in a 20-wide field: {}",
        o
    );
    assert!(!o.contains("42.0000"), "float artifact leaked: {}", o);
}

#[test]
fn printtimescale_reports_the_scope_timescale() {
    let o = out(r#"
`timescale 1us/10ns
module m; initial $printtimescale();
endmodule
"#);
    assert!(o.contains("is 1us / 10ns"), "got: {}", o);
}

// §3.14.1: sub-nanosecond precision is honoured — the simulation tick is the
// finest precision declared anywhere, not a fixed 1 ns. (Fixed as a side
// effect of the per-module timescale rework; there is no longer a 1 ns floor.)

#[test]
fn one_picosecond_precision_is_honoured() {
    let o = out(r#"
`timescale 1ns/1ps
module m; initial begin
  #0.5;  $display("A=%0f", $realtime);   // 0.5 ns = 500 ps
  #0.001; $display("B=%0f", $realtime);  // + 1 ps
end endmodule
"#);
    assert!(o.contains("A=0.500000"), "0.5ns must be exact under 1ps precision: {}", o);
    assert!(o.contains("B=0.501000"), "a 1ps step must advance time: {}", o);
}

#[test]
fn a_picosecond_timescale_counts_in_picoseconds() {
    let o = out(r#"
`timescale 1ps/1ps
module m; initial begin #500; $display("T=%0d", $time); end endmodule
"#);
    assert!(o.contains("T=500"), "#500 under 1ps must read 500, not 0: {}", o);
}

#[test]
fn femtosecond_precision_works() {
    let o = out(r#"
`timescale 1ps/1fs
module m; initial begin #1; $display("T=%0d R=%0f", $time, $realtime); end endmodule
"#);
    assert!(o.contains("T=1"), "1ps unit, fs precision: {}", o);
}

#[test]
fn sub_ns_precision_emits_no_warning() {
    // The old "sim ticks are 1ns" warning was stale and is gone.
    let sim = simulate("`timescale 1ns/1ps\nmodule m; initial #1.5 $display(\"x\"); endmodule", 100)
        .expect("simulate failed");
    let joined = sim.output.iter().map(|o| o.message.clone()).collect::<Vec<_>>().join("\n");
    assert!(!joined.contains("ticks are 1ns"), "stale sub-ns warning leaked: {}", joined);
}
