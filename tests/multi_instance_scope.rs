//! An AST-evaluated process (here a constrained `std::randomize`) inside a
//! *multiply-instantiated* module must resolve its bare signal names against
//! ITS OWN instance, and `%m` must report that instance's scope. Previously
//! every instance's process shared one name-resolution scope, so the 2nd
//! instance's `std::randomize(idx)` wrote the 1st instance's signal (leaving
//! its own X) and `%m` always printed the top module name.

use xezim::simulate;

const SRC: &str = r#"
module probe();
  bit [3:0] idx;
  initial begin
    #1;
    void'(std::randomize(idx) with { idx <= 6; });
    $display("%m idx=%0d", idx);
  end
endmodule
module TB;
  probe p0();
  probe p1();
  initial #5 $finish;
endmodule
"#;

fn get(sim: &xezim::compiler::Simulator, name: &str) -> Option<u64> {
    sim.get_signal(name).and_then(|v| v.to_u64())
}

#[test]
fn multiply_instantiated_randomize_and_m_are_per_instance() {
    let sim = simulate(SRC, 100).expect("simulate failed");
    // Both instances' own signal must be randomized within the constraint —
    // neither left unwritten (X ⇒ to_u64 None), and both <= 6.
    let a = get(&sim, "p0.idx").expect("p0.idx unresolved/X");
    let b = get(&sim, "p1.idx").expect("p1.idx unresolved/X — 2nd instance not randomized");
    assert!(a <= 6, "p0.idx={} violates constraint", a);
    assert!(b <= 6, "p1.idx={} violates constraint", b);
    // %m must report each instance's scope, not just the top.
    let joined: String = sim
        .output
        .iter()
        .map(|o| o.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("TB.p0"),
        "%m did not report TB.p0 scope:\n{}",
        joined
    );
    assert!(
        joined.contains("TB.p1"),
        "%m did not report TB.p1 scope:\n{}",
        joined
    );
}

/// `%m` inside a *package* function must report the package-qualified
/// hierarchy (`<pkg>.<func>`), matching real simulators — not the top-module
/// name. This is what UVM's `uvm_pkg::uvm_instance_scope()` relies on: it
/// strips `%m` down to the package name (`uvm_pkg`) to scope its factory and
/// config_db. Before the fix xezim returned the top module name, which left
/// `uvm_instance_scope` empty and broke the entire genuine-UVM-library path
/// (`UVM_ERROR [SCPSTR] Illegal name ...`, then a time-0 stall).
#[test]
fn percent_m_in_package_function_is_package_qualified() {
    const SRC: &str = r#"
package my_pkg;
  function string where_am_i();
    string s;
    $swrite(s, "%m");
    return s;
  endfunction
endpackage
module top;
  import my_pkg::*;
  initial begin
    $display("PKG_SCOPE=[%s]", where_am_i());
    $finish;
  end
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    let joined: String = sim
        .output
        .iter()
        .map(|o| o.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("PKG_SCOPE=[my_pkg.where_am_i]"),
        "%m in a package function did not report the package-qualified scope:\n{}",
        joined
    );
}

/// `%m` in a plain module-level function (no package) reports the top-module
/// hierarchy (`<top-module>.<func>`). Guards the `func_decl_scope`-absent
/// branch of the `%m` formatter and ensures the module instance path is
/// unchanged by the package-function fix.
#[test]
fn percent_m_in_module_function_is_module_qualified() {
    const SRC: &str = r#"
module top;
  function string where_am_i();
    string s;
    $swrite(s, "%m");
    return s;
  endfunction
  initial begin
    $display("MOD_SCOPE=[%s]", where_am_i());
    $finish;
  end
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    let joined: String = sim
        .output
        .iter()
        .map(|o| o.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("MOD_SCOPE=[top.where_am_i]"),
        "%m in a module function did not report the top-module-qualified scope:\n{}",
        joined
    );
}
