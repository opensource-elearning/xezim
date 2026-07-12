//! Audit fallout from the string/ref-queue debug session. Five defect
//! classes, all silent:
//!
//! 1. A local `string q[$]` / `string s` marked its NAME in `string_signals`
//!    and never unmarked it, so a later same-named `int` local in another
//!    frame got string treatment (`%p` printed char codes as strings, `s[i]`
//!    took the char-index path, `{s, ...}` byte-concatenated).
//! 2. §13.4 — a free (module/package) function body ran with the CALLER's
//!    class context live, so a bare name in the body that collided with a
//!    caller property resolved to the property (aliasing). Combined with the
//!    ref-queue copy-out this CLOBBERED the property with the stale formal.
//! 3. `o.q.push_back(x)` on a native queue property (flattened `[o, q,
//!    push_back]` path) fell through to a phantom queue named `o`; the
//!    property was never touched. Mutation builtins now walk the handle chain
//!    (and the property outranks a same-named module-scope queue).
//! 4. `%p` on a property queue (`o.q`) printed a raw 0 — the instance-scoped
//!    `<handle>#q` name was never resolved.
//! 5. `size()` on a never-touched module-scope queue read the (0, 63)
//!    registration placeholder → 64 instead of 0.

use xezim::simulate;

fn out(src: &str) -> String {
    let sim = simulate(src, 10_000).expect("simulate failed");
    sim.output.iter().map(|o| o.message.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_nonstring_local_clears_a_stale_string_marking() {
    // f1's `string q[$]` / `string s` must not leak string treatment into
    // f2's same-named int locals.
    let o = out(r#"
package p;
  function automatic void f1(); string q[$]; string s; q.push_back("a"); s = "zz"; endfunction
  function automatic void f2();
    int q[$]; int s;
    q.push_back(65); q.push_back(66);
    s = 6;
    $display("Q=%p S=%0d BIT=%0d CAT=%0d", q, s, s[1], {s, 2'b01});
  endfunction
endpackage
module m; initial begin p::f1(); p::f2(); end endmodule
"#);
    assert!(o.contains("Q='{65, 66}"), "stale string-queue marking: {}", o);
    assert!(o.contains("S=6 BIT=1 CAT=25"), "stale scalar string marking: {}", o);
}

#[test]
fn a_free_function_does_not_see_the_callers_class_context() {
    // Inside p::f the bare `q` is the formal, NOT the caller's property; the
    // ref writeback then carries the appended values back to the property.
    let o = out(r#"
package p; function automatic void f(ref int q[$]); q.push_back(7); endfunction endpackage
class c;
  int q[$];
  function void go(); q.push_back(5); p::f(q); $display("IN=%0d", q.size()); endfunction
endclass
module m; initial begin
  c o = new;
  o.q.push_back(1);
  o.go();
  $display("POST=%0d %p", o.q.size(), o.q);
end endmodule
"#);
    assert!(o.contains("IN=3"), "ref property-queue writeback: {}", o);
    assert!(o.contains("POST=3 '{1, 5, 7}"), "property queue end state: {}", o);
}

#[test]
fn queue_property_mutation_from_outside_the_class() {
    let o = out(r#"
class c; int q[$]; endclass
module m; initial begin
  c o = new;
  o.q.push_back(5);
  void'(o.q.push_back(9));
  $display("N=%0d P=%p E=%0d,%0d", o.q.size(), o.q, o.q[0], o.q[1]);
end endmodule
"#);
    assert!(o.contains("N=2"), "o.q.push_back landed on a phantom: {}", o);
    assert!(o.contains("P='{5, 9}"), "%p on a property queue: {}", o);
    assert!(o.contains("E=5,9"), "property queue element reads: {}", o);
}

#[test]
fn a_property_queue_outranks_a_same_named_module_queue() {
    let o = out(r#"
class c; int qq[$]; endclass
module m;
  int qq[$];
  initial begin
    c o = new;
    o.qq.push_back(9);
    $display("PROP=%0d MOD=%0d", o.qq.size(), qq.size());
  end
endmodule
"#);
    assert!(o.contains("PROP=1 MOD=0"), "shadowing order wrong: {}", o);
}

#[test]
fn an_untouched_module_scope_queue_is_empty() {
    let o = out(r#"
module m;
  int q[$];
  initial begin
    $display("S0=%0d", q.size());
    q.push_back(5);
    $display("S1=%0d", q.size());
  end
endmodule
"#);
    assert!(o.contains("S0=0"), "fresh queue must be size 0, not 64: {}", o);
    assert!(o.contains("S1=1"), "size after push: {}", o);
}
