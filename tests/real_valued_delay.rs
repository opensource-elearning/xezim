//! A real-valued delay `#(real_expr)` must schedule at the rounded real time,
//! not read the value's raw IEEE-754 bits as a tick count.
//!
//! The clock-generator fast path (`initial begin c=0; forever #d c=~c; end`)
//! extracted its half-period with `eval_expr(d).to_u64()`, which for a real
//! `d` (e.g. `refclk_period/2` = 20833.0) returned ~4.6e18 — so the clock never
//! fired and a testbench watchdog reported "refclk is stuck". It now routes the
//! delay through `eval_delay_ticks` (real-aware, §3.14.3 precision rounding).

use xezim::simulate;

fn count_toggles(delay: &str) -> u64 {
    let src = format!(
        r#"
`timescale 1ps/1ps
module t;
  reg clk; real period = 41666.0; int n = 0;
  initial begin clk = 0; forever {} clk = ~clk; end
  initial begin repeat (6) @(posedge clk) n++; done = 1; end
  initial #2000000 done = 1;   // watchdog
  bit done = 0;
endmodule
"#,
        delay
    );
    let sim = simulate(&src, 3_000_000).expect("simulate failed");
    sim.get_signal("t.n")
        .or_else(|| sim.get_signal("n"))
        .and_then(|v| v.to_u64())
        .expect("n unreadable")
}

/// Every real-valued delay form must actually advance the clock — a real
/// literal, a real variable, and real arithmetic — matching the plain int form.
#[test]
fn real_valued_clock_delays_toggle() {
    assert_eq!(count_toggles("#20833"), 6, "int delay baseline");
    assert_eq!(count_toggles("#(20833.0)"), 6, "real literal delay");
    assert_eq!(count_toggles("#(period)"), 6, "real variable delay");
    assert_eq!(count_toggles("#(period/2)"), 6, "real division delay");
}
