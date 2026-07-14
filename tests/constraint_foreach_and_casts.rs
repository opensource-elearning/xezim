//! Constraints-engine gaps from issues #28 and #29.
//!
//! #28 — IEEE 1800-2017 §18.5.7 iterative (foreach) constraints: a
//! `foreach (arr[i]) <body>` inside a randomize constraint block must bind
//! the index variable and constrain EVERY element. Equality bodies
//! (`arr[i] == i + 5`) and relational bodies were silently ignored — only
//! the `arr[i] inside {…}` shape was solved — so elements kept unconstrained
//! random values.
//!
//! #29 — §18.3 constraint expressions containing §6.24.1 size casts: the
//! parser lowers `32'(expr)` to a `$__xz_size_cast` SystemCall, which the
//! solver's structural analysis did not see through, so a constraint like
//! `32'(A * B) < 32'd1000` never narrowed B. Per §11.6.1 the cast context
//! sizes the operands to 32 bits BEFORE the multiply, so the solver must
//! reason in the widened domain (affine decomposition in i64).

use xezim::simulate;

fn u(sim: &xezim::compiler::Simulator, n: &str) -> u64 {
    sim.get_signal(n)
        .or_else(|| sim.get_signal(&format!("tb.{}", n)))
        .unwrap_or_else(|| panic!("signal not found: {}", n))
        .to_u64()
        .unwrap_or_else(|| panic!("{} not u64-able", n))
}

/// #28's exact shape: §18.5.7 foreach with a per-element EQUALITY body via
/// scope randomize. Every element is fully determined by its index.
#[test]
fn foreach_equality_constraint_std_randomize() {
    const SRC: &str = r#"
module tb;
  typedef bit [31:0] u32_t;
  u32_t test_array [4];
  int failures = 0;
  initial begin
    void'(std::randomize(test_array) with {
      foreach (test_array[i]) {
        test_array[i] == i + 5;
      }
    });
    foreach (test_array[i])
      if (test_array[i] != i + 5) failures++;
  end
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    assert_eq!(
        u(&sim, "failures"),
        0,
        "§18.5.7 foreach equality body must pin every element to i + 5"
    );
}

/// §18.5.7 foreach with RELATIONAL bodies (`arr[i] > lo; arr[i] < hi;`)
/// via scope randomize: elements must land inside the narrowed interval,
/// repeatedly.
#[test]
fn foreach_relational_constraint_std_randomize() {
    const SRC: &str = r#"
module tb;
  bit [15:0] arr [8];
  int violations = 0;
  initial begin
    for (int trial = 0; trial < 50; trial++) begin
      void'(std::randomize(arr) with {
        foreach (arr[i]) {
          arr[i] > 16'd100;
          arr[i] <= 16'd110;
        }
      });
      foreach (arr[i])
        if (!(arr[i] > 100 && arr[i] <= 110)) violations++;
    end
  end
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    assert_eq!(
        u(&sim, "violations"),
        0,
        "§18.5.7 foreach relational body must keep every element in (100, 110]"
    );
}

/// #29's exact shape: a §6.24.1 size cast wrapping an arithmetic compound in
/// an inline constraint (`32'(A * B) < 32'd1000`). Per §11.6.1 the solver
/// must treat the product in the widened 32-bit domain and narrow B to
/// [1, 3] (255 * B < 1000), not bypass the constraint.
#[test]
fn size_cast_over_product_in_inline_constraint() {
    const SRC: &str = r#"
module tb;
  logic [7:0]  A;
  logic [15:0] B;
  int violations = 0;
  int rand_failures = 0;
  initial begin
    A = 8'hFF; // 255
    for (int trial = 0; trial < 50; trial++) begin
      if (!std::randomize(B) with {
        32'(A * B) < 32'd1000;
        B > 0;
      }) rand_failures++;
      else if (!((32'(A) * 32'(B)) < 32'd1000 && B > 0)) violations++;
    end
  end
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    assert_eq!(u(&sim, "rand_failures"), 0, "randomize must succeed");
    assert_eq!(
        u(&sim, "violations"),
        0,
        "§6.24.1/§11.6.1: 32'(A * B) < 1000 must narrow B to [1, 3]"
    );
}

/// §18.5.7 foreach equality body in a CLASS constraint block — the same #28
/// gap via `obj.randomize()`: the rand-array pool pass must not re-seed
/// elements the per-index solver already pinned.
#[test]
fn foreach_equality_constraint_class_randomize() {
    const SRC: &str = r#"
module tb;
  class pkt;
    rand bit [7:0] arr [4];
    constraint c { foreach (arr[i]) arr[i] == i * 3 + 1; }
  endclass
  int failures = 0;
  int rand_failures = 0;
  initial begin
    pkt p = new();
    for (int trial = 0; trial < 20; trial++) begin
      if (!p.randomize()) rand_failures++;
      foreach (p.arr[i])
        if (p.arr[i] != i * 3 + 1) failures++;
    end
  end
endmodule
"#;
    let sim = simulate(SRC, 100).expect("simulate failed");
    assert_eq!(u(&sim, "rand_failures"), 0, "class randomize must succeed");
    assert_eq!(
        u(&sim, "failures"),
        0,
        "§18.5.7 class foreach equality body must pin every element"
    );
}
