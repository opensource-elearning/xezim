//! `ref` queue / dynamic-array argument writeback for TYPEDEF'D collection
//! formals, and typedef'd collection class MEMBERS (§13.5.2, §6.18).
//!
//! Siblings of the assoc-array fixes in tests/ref_arg_assoc_writeback.rs:
//!
//! 1. A class-LOCAL `typedef int q_t[$];` used as a `ref q_t out_q` formal
//!    bound as a scalar — the dims live on the class's typedef table, which
//!    the formal-binding path never consulted. The callee's `push_back`s were
//!    lost on return.
//! 2. Same for a class-local dynamic-array typedef (`typedef int d_t[];`,
//!    `ref d_t d` with `d = new[4]` in the callee), including a caller local
//!    declared with the class-scoped name (`Ph::d_t dd;`).
//! 3. A class PROPERTY declared with a MODULE-level collection typedef
//!    (`typedef int q_t[$]; class Ph; protected q_t m_q;`) was never
//!    classified as a queue member (`elaborate_class` resolves dims only from
//!    the class's OWN local typedefs), so it got no per-instance storage:
//!    `foreach (m_q[i])` inside a method iterated one junk element and the
//!    ref-writeback caller saw `q.size()==1` with value 0 instead of the
//!    pushed contents.

use xezim::simulate;

fn messages(sim: &xezim::compiler::Simulator) -> Vec<String> {
    sim.output.iter().map(|o| o.message.clone()).collect()
}

fn assert_pass(sim: &xezim::compiler::Simulator, tag: &str) {
    let msgs = messages(sim);
    let pass = msgs.iter().any(|m| m.contains(&format!("{tag}_PASS")));
    let fail = msgs.iter().find(|m| m.contains(&format!("{tag}_FAIL")));
    assert!(
        pass,
        "expected {tag}_PASS in output\nfail line: {fail:?}\nfull output: {msgs:?}"
    );
}

/// Class-local queue typedef as a `ref` formal: the callee pushes the
/// protected member's contents into the caller's queue (values checked, not
/// just the size).
#[test]
fn class_local_typedef_queue_ref_writeback() {
    const SRC: &str = r#"
class Ph;
  typedef int q_t[$];
  protected q_t m_q;
  function void add(int v); m_q.push_back(v); endfunction
  function void get_all(ref q_t out_q);
    foreach (m_q[i]) out_q.push_back(m_q[i]);
  endfunction
endclass
module top;
  initial begin
    int n; Ph h = new;
    int tmp[$];
    h.add(10); h.add(20); h.add(30);
    h.get_all(tmp);
    if (tmp.size() == 3 && tmp[0] == 10 && tmp[1] == 20 && tmp[2] == 30)
      $display("QREF_PASS");
    else
      $display("QREF_FAIL size=%0d", tmp.size());
  end
endmodule
"#;
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_pass(&sim, "QREF");
}

/// Same shape routed through ANOTHER method whose local is declared with the
/// class-local typedef itself (`q_t tmp;` inside `check`).
#[test]
fn class_local_typedef_queue_ref_via_method_local() {
    const SRC: &str = r#"
class Ph;
  typedef int q_t[$];
  protected q_t m_q;
  function void add(int v); m_q.push_back(v); endfunction
  function void get_all(ref q_t out_q);
    foreach (m_q[i]) out_q.push_back(m_q[i]);
  endfunction
  function void check(output int n);
    q_t tmp;
    get_all(tmp);
    n = tmp.size();
  endfunction
endclass
module top;
  initial begin
    int n; Ph h = new;
    h.add(10); h.add(20); h.add(30);
    h.check(n);
    if (n == 3) $display("QLOC_PASS");
    else $display("QLOC_FAIL n=%0d", n);
  end
endmodule
"#;
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_pass(&sim, "QLOC");
}

/// Class-local dynamic-array typedef as a `ref` formal, with the caller's
/// actual declared via the class-scoped name (`Ph::d_t dd;`). The callee's
/// `new[4]` + element writes must reach the caller.
#[test]
fn class_local_typedef_darray_ref_writeback() {
    const SRC: &str = r#"
class Ph;
  typedef int d_t[];
  function void fill(ref d_t d);
    d = new[4]; foreach (d[i]) d[i] = i*10;
  endfunction
endclass
module top;
  initial begin
    Ph h = new; int n; int sum;
    begin
      Ph::d_t dd;
      h.fill(dd);
      n = dd.size();
      foreach (dd[i]) sum += dd[i];
    end
    if (n == 4 && sum == 60) $display("DREF_PASS");
    else $display("DREF_FAIL n=%0d sum=%0d", n, sum);
  end
endmodule
"#;
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_pass(&sim, "DREF");
}

/// MODULE-level collection typedefs: a queue-typedef'd class property must be
/// a real per-instance queue member (§6.18) — `foreach` over it inside a
/// method sees the pushed elements, and both the queue and dynamic-array ref
/// formals write back to the caller.
#[test]
fn module_typedef_property_and_ref_writeback() {
    const SRC: &str = r#"
typedef int q_t[$];
typedef int d_t[];
class Ph;
  protected q_t m_q;
  function void add(int v); m_q.push_back(v); endfunction
  function void get_all(ref q_t out_q);
    foreach (m_q[i]) out_q.push_back(m_q[i]);
  endfunction
  function void fill(ref d_t d);
    d = new[4]; foreach (d[i]) d[i] = i*10;
  endfunction
endclass
module top;
  initial begin
    int n1, n2; q_t tmp; d_t dd;
    Ph h = new;
    h.add(10); h.add(20); h.add(30);
    h.get_all(tmp); n1 = tmp.size();
    h.fill(dd); n2 = dd.size();
    if (n1 == 3 && tmp[0] == 10 && tmp[1] == 20 && tmp[2] == 30 && n2 == 4 && dd[3] == 30)
      $display("CTL_PASS");
    else
      $display("CTL_FAIL q=%0d d=%0d", n1, n2);
  end
endmodule
"#;
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_pass(&sim, "CTL");
}

/// Two instances of a class with a module-typedef'd queue property keep
/// INDEPENDENT per-instance storage (the classification must not collapse
/// them onto one shared global).
#[test]
fn module_typedef_queue_property_per_instance() {
    const SRC: &str = r#"
typedef int q_t[$];
class Ph;
  protected q_t m_q;
  function void add(int v); m_q.push_back(v); endfunction
  function int total;
    int s;
    foreach (m_q[i]) s += m_q[i];
    return s;
  endfunction
  function int count; return m_q.size(); endfunction
endclass
module top;
  initial begin
    Ph a = new; Ph b = new;
    a.add(1); a.add(2);
    b.add(100);
    if (a.count() == 2 && b.count() == 1 && a.total() == 3 && b.total() == 100)
      $display("INST_PASS");
    else
      $display("INST_FAIL a=%0d/%0d b=%0d/%0d", a.count(), a.total(), b.count(), b.total());
  end
endmodule
"#;
    let sim = simulate(SRC, 1000).expect("simulate failed");
    assert_pass(&sim, "INST");
}
