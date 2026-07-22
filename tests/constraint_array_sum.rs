//! §18.5.8.2 — array-element-coupling sum constraints (`arr.sum() == K`,
//! `arr.sum() with (item) == K`, and the explicit chain `a[0]+a[1]+a[2] == K`).
//! These couple multiple rand array elements to a constant total. They used to
//! be treated as "unmodeled": the plain `.sum()` form was SKIPPED (silently
//! passing while violated); the `with`/explicit forms fell back to rejection
//! sampling, which cannot hit a point target in a wide domain. `randomize()`
//! now DISTRIBUTES a random valid assignment so the total lands exactly on the
//! target (respecting per-element `inside` ranges and elements already pinned),
//! and infeasible targets correctly fail.

use xezim::simulate;

fn lines(src: &str, tag: &str) -> Vec<String> {
    let sim = simulate(src, 1000).expect("sim");
    sim.output
        .iter()
        .filter(|o| o.message.starts_with(tag))
        .map(|o| o.message.clone())
        .collect()
}

/// Plain `arr.sum() == K` on a fixed array: every draw sums to K and varies.
#[test]
fn plain_sum_method_hits_target() {
    let src = "\
class C; rand bit[7:0] a[3]; constraint cx { a.sum() == 20; } endclass\n\
module t; initial begin C c=new();\n\
  for (int n=0;n<8;n++) begin\n\
    int r=c.randomize(); int s=c.a[0]+c.a[1]+c.a[2];\n\
    $display(\"R r=%0d s=%0d\", r, s);\n\
  end $finish; end endmodule";
    let out = lines(src, "R ");
    assert_eq!(out.len(), 8);
    for l in &out {
        assert_eq!(
            l, "R r=1 s=20",
            "plain .sum() must solve to the exact target"
        );
    }
}

/// `arr.sum() with (int'(item)) == K` plus per-element `inside` range.
#[test]
fn sum_with_identity_and_element_range() {
    let src = "\
class C; rand bit[7:0] a[]; constraint cx {\n\
  a.size()==4; a.sum() with (int'(item)) == 100;\n\
  foreach(a[i]) a[i] inside {[10:40]}; } endclass\n\
module t; initial begin C c=new();\n\
  for (int n=0;n<10;n++) begin\n\
    int r=c.randomize(); int s=0; int bad=0;\n\
    foreach(c.a[i]) begin s+=c.a[i]; if(c.a[i]<10||c.a[i]>40) bad=1; end\n\
    $display(\"R r=%0d s=%0d bad=%0d\", r, s, bad);\n\
  end $finish; end endmodule";
    let out = lines(src, "R ");
    assert_eq!(out.len(), 10);
    for l in &out {
        assert_eq!(
            l, "R r=1 s=100 bad=0",
            "sum with identity + range must hold"
        );
    }
}

/// Explicit element chain `a[0]+a[1]+a[2] == K` solves at FULL width (no
/// truncation to the element width).
#[test]
fn explicit_element_chain_full_width() {
    let src = "\
class C; rand int a[3]; constraint cx {\n\
  a[0]+a[1]+a[2] == 30; foreach(a[i]) a[i] inside {[0:20]}; } endclass\n\
module t; initial begin C c=new();\n\
  for (int n=0;n<8;n++) begin\n\
    int r=c.randomize(); int s=c.a[0]+c.a[1]+c.a[2];\n\
    $display(\"R r=%0d s=%0d\", r, s);\n\
  end $finish; end endmodule";
    let out = lines(src, "R ");
    assert_eq!(out.len(), 8);
    for l in &out {
        assert_eq!(l, "R r=1 s=30", "explicit chain must solve at full width");
    }
}

/// An infeasible sum target must make randomize() FAIL (return 0), not silently
/// succeed: 3 elements each in [0:5] cannot sum to 50.
#[test]
fn infeasible_sum_target_fails() {
    let src = "\
class C; rand bit[7:0] a[3]; constraint cx {\n\
  a.sum() == 50; foreach(a[i]) a[i] inside {[0:5]}; } endclass\n\
module t; initial begin C c=new();\n\
  int r=c.randomize();\n\
  $display(\"R r=%0d\", r);\n\
  $finish; end endmodule";
    assert_eq!(lines(src, "R ")[0], "R r=0", "infeasible sum must return 0");
}
