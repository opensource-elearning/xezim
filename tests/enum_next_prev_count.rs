//! §6.19.6: enum `next(N)` / `prev(N)` step N places with wrapping. Only the
//! no-argument form was handled (the dispatch was gated on `args.is_empty()`),
//! so `c.next(2)` fell through to an invalid value (empty .name()).

use xezim::simulate;

fn output_of(sim: &xezim::compiler::Simulator) -> String {
    sim.output
        .iter()
        .map(|o| o.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn enum_next_prev_with_count() {
    const SRC: &str = r#"
module top;
  typedef enum logic [2:0] { RED=1, GREEN=2, BLUE=4 } color_t;
  color_t c;
  initial begin
    c=RED;  $display("N0 %s", c.next(0).name());   // RED
    c=RED;  $display("N2 %s", c.next(2).name());   // BLUE
    c=RED;  $display("N3 %s", c.next(3).name());   // RED (wrap)
    c=RED;  $display("N4 %s", c.next(4).name());   // GREEN (wrap)
    c=BLUE; $display("P2 %s", c.prev(2).name());   // RED
    c=RED;  $display("NA %s", c.next().name());    // GREEN (no-arg still ok)
  end
endmodule
"#;
    let out = output_of(&simulate(SRC, 100).expect("sim"));
    for want in [
        "N0 RED", "N2 BLUE", "N3 RED", "N4 GREEN", "P2 RED", "NA GREEN",
    ] {
        assert!(out.contains(want), "missing `{}`:\n{}", want, out);
    }
}
