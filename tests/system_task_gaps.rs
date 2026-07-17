//! Regression tests for the July-2026 missing-system-task audit.
//!
//! Group 1: unknown-system-task meta-diagnostic (once per name, never for
//!          names serviced by either dispatcher or by internals).
//! Group 2: $exit terminates like $finish.
//! Group 3: $fstrobe/$fmonitor file variants.
//! Group 4: $fread binary load (reg + memory forms).
//! Group 5: $sdf_annotate runtime annotation.
//! Group 6: $fsdbDumpfile/$fsdbDumpvars/$vcdpluson mapping.
//! Group 7: recognized-warn stubs.

use xezim::simulate;

fn u(sim: &xezim::compiler::Simulator, n: &str) -> u64 {
    sim.get_signal(n)
        .or_else(|| sim.get_signal(&format!("tb.{}", n)))
        .unwrap_or_else(|| panic!("signal not found: {}", n))
        .to_u64()
        .unwrap_or_else(|| panic!("{} not u64-able", n))
        & 0xFFFF_FFFF
}

// ---------------------------------------------------------------- group 1

#[test]
fn unknown_task_warns_once_per_name() {
    let src = r#"
module tb;
  integer n;
  initial begin
    $bogus_task(1);
    $bogus_task(2);
    n = $bogus_func(3);
    repeat (3) $another_missing;
  end
endmodule
"#;
    let sim = simulate(src, 1000).expect("simulate failed");
    let warned = sim.warned_system_task_names();
    assert!(warned.contains(&"$bogus_task".to_string()), "warned: {:?}", warned);
    assert!(warned.contains(&"$bogus_func".to_string()), "warned: {:?}", warned);
    assert!(warned.contains(&"$another_missing".to_string()), "warned: {:?}", warned);
    // unknown function returns 0, does not abort simulation
    assert_eq!(u(&sim, "n"), 0);
}

#[test]
fn handled_names_do_not_trip_unknown_warning() {
    // Function-only names in statement position (result discarded) and
    // ordinary handled tasks must NOT be reported as unknown.
    let src = r#"
module tb;
  integer x;
  initial begin
    $urandom;
    $random;
    x = $countones(8'hF0);
    $display("x=%0d", x);
    $strobe("s=%0d", x);
    $monitoroff;
  end
endmodule
"#;
    let sim = simulate(src, 1000).expect("simulate failed");
    assert!(
        sim.warned_system_task_names().is_empty(),
        "spurious unknown-task warnings: {:?}",
        sim.warned_system_task_names()
    );
}
