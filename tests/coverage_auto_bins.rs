//! §19.5.1/§19.6 — coverage of AUTO-binned coverpoints and crosses must be the
//! fraction of auto bins hit, NOT 100% the moment anything is sampled. A 3-bit
//! coverpoint has 8 auto bins; a 2x2-bit cross has 16.

use xezim::simulate;

fn cov(src: &str) -> String {
    let sim = simulate(src, 1000).expect("simulate failed");
    sim.output
        .iter()
        .map(|o| o.message.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn auto_bin_coverpoint_is_fraction_hit() {
    let o = cov(r#"
module t;
  bit [2:0] a;
  covergroup cg; cp: coverpoint a; endgroup
  cg c = new;
  initial begin
    a=0; c.sample(); a=3; c.sample(); a=7; c.sample();
    $display("COV=%0.2f", c.get_coverage());
    $finish;
  end
endmodule
"#);
    assert!(
        o.contains("COV=37.50"),
        "3 of 8 auto bins = 37.5%; got: {}",
        o
    );
}

#[test]
fn auto_cross_coverage_is_fraction_hit() {
    let o = cov(r#"
module t;
  bit [1:0] a, b;
  covergroup cg; ca: coverpoint a; cb: coverpoint b; axb: cross ca, cb; endgroup
  cg c = new;
  initial begin
    a=0;b=0; c.sample(); a=1;b=2; c.sample();
    // ca=2/4=50, cb=2/4=50, axb=2/16=12.5 -> mean 37.5
    $display("COV=%0.2f", c.get_coverage());
    $finish;
  end
endmodule
"#);
    assert!(
        o.contains("COV=37.50"),
        "cross must count 2 of 16 bins; got: {}",
        o
    );
}

/// §19.7 option.at_least = N: a bin is covered only when hit >= N times.
#[test]
fn option_at_least_gates_bin_coverage() {
    let o = cov(r#"
module t;
  bit [1:0] a;
  covergroup cg;
    option.at_least = 2;
    cp: coverpoint a { bins b0={0}; bins b1={1}; }
  endgroup
  cg c = new;
  initial begin
    a=0; c.sample();               // b0 hit once (needs 2)
    a=1; c.sample(); c.sample();   // b1 hit twice -> covered
    $display("COV=%0.2f", c.get_coverage());
    $finish;
  end
endmodule
"#);
    assert!(
        o.contains("COV=50.00"),
        "at_least=2: only b1 covered -> 50%; got: {}",
        o
    );
}

/// §19.7 option.weight: coverpoints are weighted in the covergroup mean.
#[test]
fn option_weight_reweights_the_mean() {
    let o = cov(r#"
module t;
  bit [1:0] a, b;
  covergroup cg;
    ca: coverpoint a { option.weight=3; bins x={0}; }               // 100%
    cb: coverpoint b { option.weight=1; bins y={1}; bins z={2}; }   // 0%
  endgroup
  cg c = new;
  initial begin a=0; b=0; c.sample(); $display("COV=%0.2f", c.get_coverage()); $finish; end
endmodule
"#);
    assert!(
        o.contains("COV=75.00"),
        "weighted (3*100+1*0)/4 = 75; got: {}",
        o
    );
}
