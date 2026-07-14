//! Issue #23: §6.16 string-method compliance gaps.
//!
//!   compare()/icompare()  were unimplemented — both returned 0, which made
//!                         compare() look case-insensitive.
//!   block-local `real`    was not registered in real_signals, so an
//!                         assignment rounded through real_to_int and
//!                         `real r; r = 3.14159;` read back 3.0 — breaking
//!                         realtoa()/atoreal() round-trips through locals.

use xezim::simulate;

const SRC: &str = r#"
module tb;
  int cmp_eq, cmp_ne, icmp_eq, cmp_lt, cmp_gt;
  bit real_ok, realtoa_ok, atoreal_ok;
  initial begin
    string s1, s2, s;
    s1 = "SystemVerilog";
    s2 = "systemverilog";
    cmp_eq  = s1.compare(s1);
    cmp_ne  = s1.compare(s2);
    icmp_eq = s1.icompare(s2);
    cmp_lt  = "abc".compare("abd");
    cmp_gt  = "abd".compare("abc");
    begin
      real r, r_out;
      r = 3.14159;
      real_ok = (r > 3.14 && r < 3.15);
      s.realtoa(r);
      realtoa_ok = (s.substr(0, 3) == "3.14");
      s = "2.71828";
      r_out = s.atoreal();
      atoreal_ok = (r_out > 2.718 && r_out < 2.719);
    end
  end
endmodule
"#;

fn i(sim: &xezim::compiler::Simulator, n: &str) -> i64 {
    let v = sim
        .get_signal(n)
        .or_else(|| sim.get_signal(&format!("tb.{}", n)))
        .unwrap_or_else(|| panic!("signal not found: {}", n));
    v.to_u64().unwrap_or_else(|| panic!("{} not u64-able", n)) as u32 as i32 as i64
}

#[test]
fn compare_is_case_sensitive_and_icompare_is_not() {
    let sim = simulate(SRC, 100).expect("simulate failed");
    assert_eq!(i(&sim, "cmp_eq"), 0, "compare() of equal strings is 0");
    assert_ne!(i(&sim, "cmp_ne"), 0, "compare() must be case-SENSITIVE");
    assert_eq!(i(&sim, "icmp_eq"), 0, "icompare() ignores case");
    assert!(i(&sim, "cmp_lt") < 0, "compare() is negative when receiver sorts first");
    assert!(i(&sim, "cmp_gt") > 0, "compare() is positive when receiver sorts last");
}

#[test]
fn block_local_reals_keep_their_fraction() {
    let sim = simulate(SRC, 100).expect("simulate failed");
    assert_eq!(i(&sim, "real_ok"), 1, "local real assignment must not round to int");
    assert_eq!(i(&sim, "realtoa_ok"), 1, "realtoa() of a local real keeps the fraction");
    assert_eq!(i(&sim, "atoreal_ok"), 1, "atoreal() into a local real keeps the fraction");
}
