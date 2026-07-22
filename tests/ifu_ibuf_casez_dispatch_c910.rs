//! Cone-of-influence test for c910 `ct_ifu_ibuf.v`'s casez dispatch tree
//! at lines 7918-8362 — the third remaining hypothesis from round 23 of
//! the c910 memcpy investigation. See docs/c910_memcpy_investigation.md.
//!
//! The dispatch tree's selector is the 5-bit concatenation
//! `{pop_h0_32_start, pop_h1_32_start, pop_h2_32_start,
//!   pop_h3_32_start, pop_h4_32_start}`. Each case item uses `?`
//! wildcards on the high-index bits, e.g. `5'b000??`, `5'b001??`,
//! `5'b01?0?`, etc. The body of each arm muxes the halfword data
//! streams pop_h0..pop_h4 into the three output instructions
//! ibuf_pop_inst0/inst1/inst2 with widths 32 bits each.
//!
//! Verilog casez semantics (IEEE 1800 §12.5.1): `?` (which lex-maps to
//! LogicBit::Z) matches anything on either side. xezim's bytecode op
//! `CasezEq` (bytecode.rs:606) calls `Value::casez_eq` (value.rs:961)
//! which treats Z bits on either side as don't-care — verified correct.
//!
//! This test exercises the exact case-selector shape (5-bit concat of
//! 5 single-bit signals) and the wildcarded patterns used in the c910
//! dispatch tree. Walks through all 32 possible {h0,h1,h2,h3,h4}_32_start
//! combinations and asserts the dispatch arm picks the documented
//! "ibuf_pop3_half_num" value for each.

use xezim::simulate;

const SRC: &str = r#"
module tb;
  reg h0_32, h1_32, h2_32, h3_32, h4_32;
  reg [2:0] half_num;

  // Stripped-down dispatch from ct_ifu_ibuf.v:7918+ — keeps only the
  // half_num field, which encodes the arm's selection. Patterns and
  // their half_num values are copied verbatim from the c910 RTL.
  always @(*) begin
    casez ({h0_32, h1_32, h2_32, h3_32, h4_32})
      5'b000??: half_num = 3'b011;  // 3 RVC, no 32-bit
      5'b001??: half_num = 3'b100;  // h2 starts 32-bit, h3 is its upper half
      5'b01?0?: half_num = 3'b100;  // h1 starts 32-bit (h2 = upper); h3 is RVC
      5'b01?1?: half_num = 3'b101;  // h1 starts 32-bit; h3 also starts 32-bit (h4 upper)
      5'b1?0??: half_num = 3'b100;  // h0 starts 32-bit; h2..h4 mixed
      5'b1?10?: half_num = 3'b101;  // h0 32-bit; h2 32-bit; h4 is RVC
      5'b1?11?: half_num = 3'b110;  // h0 32-bit; h2 32-bit; h4 32-bit
      default : half_num = 3'b111;
    endcase
  end

  // Capture half_num for each of the 32 combos into 32 separate regs
  // (since there's no array-element accessor in the test API).
  reg [2:0] cap_lo, cap_hi;
  reg [95:0] cap_lo_packed;  // 3 bits × 32 = 96 bits
  reg [4:0] j;

  integer i;
  initial begin
    cap_lo_packed = 96'b0;
    for (i = 0; i < 32; i = i + 1) begin
      {h0_32, h1_32, h2_32, h3_32, h4_32} = i[4:0];
      #1;
      cap_lo_packed = cap_lo_packed | ({93'b0, half_num} << (i*3));
    end
    $finish;
  end
endmodule
"#;

fn lookup_wide(sim: &xezim::compiler::Simulator, name: &str) -> Vec<u32> {
    let v = sim
        .get_signal(name)
        .or_else(|| sim.get_signal(&format!("tb.{}", name)))
        .unwrap_or_else(|| panic!("signal not found: {}", name));
    // Unpack 96-bit cap_lo_packed into 32 3-bit slots, LSB-first.
    let mut out = Vec::with_capacity(32);
    for i in 0u32..32 {
        // Read 3 bits at position i*3.
        let bit0 = v.get_bit((i * 3) as usize);
        let bit1 = v.get_bit(((i * 3) + 1) as usize);
        let bit2 = v.get_bit(((i * 3) + 2) as usize);
        let b0 = matches!(bit0, xezim_core::value::LogicBit::One) as u32;
        let b1 = matches!(bit1, xezim_core::value::LogicBit::One) as u32;
        let b2 = matches!(bit2, xezim_core::value::LogicBit::One) as u32;
        out.push(b0 | (b1 << 1) | (b2 << 2));
    }
    out
}

/// Expected half_num for each 5-bit input value, derived by hand-applying
/// the casez wildcard rules in priority order (first-match wins).
fn expected(idx: u32) -> u64 {
    let h0 = (idx >> 4) & 1;
    let h1 = (idx >> 3) & 1;
    let h2 = (idx >> 2) & 1;
    let h3 = (idx >> 1) & 1;
    let h4 = idx & 1;
    let _ = h4;
    let _ = h3;
    if h0 == 0 && h1 == 0 && h2 == 0 {
        0b011
    } else if h0 == 0 && h1 == 0 && h2 == 1 {
        0b100
    } else if h0 == 0 && h1 == 1 && h3 == 0 {
        0b100
    } else if h0 == 0 && h1 == 1 && h3 == 1 {
        0b101
    } else if h0 == 1 && h2 == 0 {
        0b100
    } else if h0 == 1 && h2 == 1 && h3 == 0 {
        0b101
    } else if h0 == 1 && h2 == 1 && h3 == 1 {
        0b110
    } else {
        0b111
    }
}

#[test]
fn casez_dispatch_tree_picks_correct_arm_for_all_32_combos() {
    let sim = simulate(SRC, 500).expect("simulate failed");
    let caps = lookup_wide(&sim, "cap_lo_packed");
    let mut bad = Vec::new();
    for idx in 0u32..32 {
        let got = caps[idx as usize] as u64;
        let exp = expected(idx);
        if got != exp {
            bad.push((idx, got, exp));
        }
    }
    assert!(
        bad.is_empty(),
        "casez dispatch mismatches: {:?}",
        bad.iter()
            .map(|(i, g, e)| format!("idx={:05b} got=0b{:03b} exp=0b{:03b}", i, g, e))
            .collect::<Vec<_>>()
    );
}
