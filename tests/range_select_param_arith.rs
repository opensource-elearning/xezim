//! Test xezim's handling of `[param:param-N]` range selects — the pattern
//! used in c910 ct_idu_is_dp.v:4863 for AIQ0 create-data field extraction.
//! These were flagged as the top suspect for the c910 memcpy REMUW dispatch
//! hang (per /home/bondan/.claude/.../memory/project_c910_memcpy_divuw_dispatch.md).
//!
//! If xezim's `compile_expr::RangeSelect` with parameter-arithmetic bounds
//! mis-computes the slice width or position, these tests fail.

use xezim::simulate;

fn lookup_one_of(sim: &xezim::compiler::Simulator, names: &[&str]) -> xezim_core::value::Value {
    for n in names {
        if let Some(v) = sim.get_signal(n) {
            return v.clone();
        }
    }
    panic!("none of these signal names found: {:?}", names);
}

/// Simple param + const arithmetic: `[P:P-8]` should be a 9-bit slice.
const SRC_PARAM_ARITH: &str = r#"
module tb;
  parameter P = 20;
  reg  [31:0] src = 32'h00000000;
  wire [P:P-8] y = src[P:P-8];   // expect 9-bit slice
  initial begin
    #1;
    src = 32'h00FFFFFF;
    #1;
    $finish;
  end
endmodule
"#;

#[test]
fn range_select_const_param_arith_width() {
    let sim = simulate(SRC_PARAM_ARITH, 100).expect("simulate failed");
    let y = lookup_one_of(&sim, &["tb.y", "y"]);
    assert_eq!(y.width, 9, "slice [P:P-8] with P=20 should be 9 bits");
    let v = y.to_u64().expect("y should be defined") & 0x1FF;
    // src = 0x00FFFFFF. bits [20:12] of src = bits 12..20 = ones since 0x00FFFFFF & 0x1FF000 != 0
    // bit 20 = 0, bit 19..12 = (0x00FFFFFF >> 12) & 0xFF = 0xFF, so bits [20:12] = 0_1111_1111 = 0xFF
    let expected = ((0x00FFFFFF_u32 >> 12) & 0x1FF) as u64;
    assert_eq!(
        v, expected,
        "y should be src[20:12] = 0x{expected:03X}, got 0x{v:03X}"
    );
}

/// Mimic c910 pattern: extract a wb-bit at the LOW end of a wide slice.
const SRC_WIDE_PARAM_SLICE: &str = r#"
module tb;
  parameter IS_WIDTH       = 271;
  parameter IS_SRC0_DATA   = 44;   // matches c910 ct_idu_is_dp.v
  parameter AIQ0_WIDTH     = 227;
  parameter AIQ0_SRC0_DATA = 66;

  reg  [IS_WIDTH-1:0]   is_data;
  wire [8:0] aiq_slice =
       is_data[IS_SRC0_DATA : IS_SRC0_DATA-8];        // 9-bit slice
  // wb-bit at position AIQ0_SRC0_DATA-7 in the 227-bit packet — this is
  // the bit the c910 ALLOC PATH uses for src0 wb (= bit 59).
  wire wb_bit = aiq_slice[1];

  initial begin
    #1;
    is_data = 271'b0;
    is_data[37] = 1'b1;   // = IS_SRC0_DATA-7, the SRC0_WB bit in c910 IS layout
    #1;
    $finish;
  end
endmodule
"#;

#[test]
fn range_select_extracts_wb_bit() {
    let sim = simulate(SRC_WIDE_PARAM_SLICE, 100).expect("simulate failed");
    let slice = lookup_one_of(&sim, &["tb.aiq_slice", "aiq_slice"]);
    assert_eq!(slice.width, 9);
    let v = slice.to_u64().expect("slice should be defined") & 0x1FF;
    // is_data has bit 37 set. IS_SRC0_DATA=44, so slice is bits[44:36].
    // bit 37 is at position 37-36 = 1 of the slice → slice = 9'b000000010 = 2.
    assert_eq!(v, 0b10, "slice should be 0b10 (bit 1 set); got 0b{v:09b}");

    let wb = lookup_one_of(&sim, &["tb.wb_bit", "wb_bit"]);
    let w = wb.to_u64().expect("wb_bit defined") & 1;
    assert_eq!(w, 1, "wb_bit should be 1");
}

/// Even closer to c910: `{N{en}} & slice` after the slice.
const SRC_REPLICATE_THEN_SLICE: &str = r#"
module tb;
  parameter W = 227;
  parameter P = 59;  // SRC0_WB bit position
  reg  [W-1:0]  data;
  reg           en;
  wire [W-1:0]  out = {W{en}} & data;
  wire          wb_at_p = out[P];

  initial begin
    en   = 1;
    data = 227'b0;
    data[59] = 1;
    #1;
    $finish;
  end
endmodule
"#;

#[test]
fn replicate_and_then_bit_select_at_param() {
    let sim = simulate(SRC_REPLICATE_THEN_SLICE, 100).expect("simulate failed");
    let wb = lookup_one_of(&sim, &["tb.wb_at_p", "wb_at_p"]);
    let w = wb.to_u64().expect("wb defined") & 1;
    assert_eq!(w, 1, "wb_at_p should be 1 (replicate(en=1) AND data[59]=1)");
}
