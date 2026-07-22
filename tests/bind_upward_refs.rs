//! Upward name referencing from bound modules — IEEE 1800-2023 §23.8
//! (upward name referencing) and §23.10.1 (a module instantiated via `bind`
//! resolves names within the scope of the bind TARGET instance, so the
//! target's — and every enclosing scope's — names are reachable upward).
//!
//! Regression for issue #27: xezim inlines a bound module as a regular child
//! of the target (§23.11), but hierarchical names whose first segment named
//! an ENCLOSING scope (by instance name, or by module definition name per
//! §23.8 "the name of a module" = nearest enclosing instance of that module)
//! did not resolve — they fell through to the unqualified-leaf fallback and
//! read X. The simulator's name resolution now retries unresolved dotted
//! names with a §23.8 upward walk over the executing scope's ancestor chain.

use xezim::simulate;

/// Issue #27 shape: a monitor bound into an empty leaf (`target_core`) reads
/// values from the bind target's enclosing scopes by MODULE DEFINITION name
/// (`dut_top.top_secret`, `sub_block.sub_secret`, §23.8 flavor) and from a
/// package (`my_pkg::pkg_secret`).
#[test]
fn bound_module_reads_upward_by_module_name() {
    const SRC: &str = r#"
package my_pkg;
  int pkg_secret = 99;
endpackage

module dut_top;
  int top_secret = 42;
  sub_block u_sub_block();
endmodule

module sub_block;
  int sub_secret = 7;
  target_core u_target_core();
endmodule

module target_core;
  // empty in the original design; receives the bound monitor
endmodule

module bind_monitor;
  int got_top = -1;
  int got_sub = -1;
  int got_pkg = -1;
  initial begin
    #1;
    // §23.8 upward references via module definition names.
    got_top = dut_top.top_secret;
    got_sub = sub_block.sub_secret;
    // Package scope reference (worked before the fix; keep it covered).
    got_pkg = my_pkg::pkg_secret;
  end
endmodule

bind target_core bind_monitor u_mon();

module tb;
  dut_top u_dut();
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    let read = |name: &str| -> i64 {
        sim.get_signal(name)
            .unwrap_or_else(|| panic!("signal {name} not found"))
            .to_u64()
            .unwrap_or_else(|| panic!("signal {name} is X/Z")) as i64
    };
    let base = "u_dut.u_sub_block.u_target_core.u_mon";
    assert_eq!(read(&format!("{base}.got_top")), 42, "dut_top.top_secret");
    assert_eq!(read(&format!("{base}.got_sub")), 7, "sub_block.sub_secret");
    assert_eq!(read(&format!("{base}.got_pkg")), 99, "my_pkg::pkg_secret");
}

/// §23.8: an upward reference by module name binds to the NEAREST enclosing
/// instance of that module. With the target instantiated under two different
/// `wrapper` instances (different parameterizations), each bound monitor
/// must read ITS OWN enclosing wrapper's value — not the first instance's.
#[test]
fn bound_module_upward_ref_is_per_instance() {
    const SRC: &str = r#"
module wrapper #(parameter int SECRET = 0);
  int secret = SECRET;
  target_core u_core();
endmodule

module target_core;
endmodule

module watcher;
  int got = -1;
  initial begin
    #1;
    // §23.8: resolves to the nearest enclosing instance of module `wrapper`
    // — a different instance for each of the two bound copies.
    got = wrapper.secret;
  end
endmodule

bind target_core watcher u_w();

module tb;
  wrapper #(.SECRET(11)) u_w1();
  wrapper #(.SECRET(22)) u_w2();
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    let read = |name: &str| -> i64 {
        sim.get_signal(name)
            .unwrap_or_else(|| panic!("signal {name} not found"))
            .to_u64()
            .unwrap_or_else(|| panic!("signal {name} is X/Z")) as i64
    };
    assert_eq!(read("u_w1.u_core.u_w.got"), 11, "monitor under u_w1");
    assert_eq!(read("u_w2.u_core.u_w.got"), 22, "monitor under u_w2");
}

/// Upward references must also work as WRITE targets (§23.8 references are
/// ordinary hierarchical names, usable on either side of an assignment), and
/// the first segment may name an instance declared in an enclosing scope
/// (§23.10.1: names visible in the bind target's scope chain).
#[test]
fn bound_module_writes_upward() {
    const SRC: &str = r#"
module dut_top;
  int ctrl = 0;
  sub_block u_sub();
endmodule

module sub_block;
  int sctrl = 0;
  target_core u_core();
endmodule

module target_core;
endmodule

module poker;
  initial begin
    #2;
    // Upward write via module definition name (§23.8).
    dut_top.ctrl = 123;
    // Upward write via an instance name declared in an enclosing scope
    // (`u_sub` lives in dut_top, two levels above the bound instance).
    u_sub.sctrl = 55;
  end
endmodule

bind target_core poker u_poke();

module tb;
  dut_top u_dut();
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    let read = |name: &str| -> i64 {
        sim.get_signal(name)
            .unwrap_or_else(|| panic!("signal {name} not found"))
            .to_u64()
            .unwrap_or_else(|| panic!("signal {name} is X/Z")) as i64
    };
    assert_eq!(read("u_dut.ctrl"), 123, "upward write by module name");
    assert_eq!(
        read("u_dut.u_sub.sctrl"),
        55,
        "upward write by instance name"
    );
}
