//! Full path-cone synthetic for the c910 create_en bug.
//!
//! Replicates the cascading port-connection / cont-assign chain
//! between CPU's biu_pad_awvalid and wid_for_axi4's create_en:
//!
//!   cpu_top (out) biu_pad_awvalid
//!     ↓ (port connection to soc/cpu_sub_system_axi)
//!   cpu_sub_system_axi.biu_pad_awvalid (port input)
//!     ↓ (cont-assign: tmp_biu_pad_awvalid = biu_pad_awvalid)
//!   cpu_sub_system_axi.tmp_biu_pad_awvalid (wire)
//!     ↓ (port connection: .biu_pad_awvalid(tmp_biu_pad_awvalid))
//!   wid_for_axi4.biu_pad_awvalid (port input)
//!     ↓ (cont-assign: create_en = biu_pad_awvalid && pad_biu_awready)
//!   wid_for_axi4.create_en (output)
//!
//! AND a similar chain for pad_biu_awready (different scope but
//! converging at create_en).
//!
//! Mimics what the c910 RTL does. Tests whether xezim's
//! port-connection + cont-assign chain correctly propagates
//! values through multiple hops AND triggers the downstream
//! `&&` cont-assign to re-evaluate.

use xezim::simulate;

const SRC: &str = r#"
`timescale 1ns/100ps

module wid_for_axi4(
  input  biu_pad_awvalid,
  input  pad_biu_awready,
  output create_en
);
  // Same bug-line pattern from c910 wid_for_axi4.v:76
  assign create_en = biu_pad_awvalid && pad_biu_awready;
endmodule

module cpu_sub_system_axi(
  input  biu_pad_awvalid,
  input  pad_biu_awready,
  output create_en
);
  // tmp_ cont-assigns (like c910 cpu_sub_system_axi.v:380-385)
  wire tmp_biu_pad_awvalid;
  wire tmp_pad_biu_awready;
  assign tmp_biu_pad_awvalid = biu_pad_awvalid;
  assign tmp_pad_biu_awready = pad_biu_awready;

  // Instance with port connections
  wid_for_axi4 wid_for_axi4 (
    .biu_pad_awvalid(tmp_biu_pad_awvalid),
    .pad_biu_awready(tmp_pad_biu_awready),
    .create_en(create_en)
  );
endmodule

module x_cpu_top(
  input  awvalid_from_pipeline,
  input  awready_from_interconnect,
  output biu_pad_awvalid,
  output create_en
);
  // Pretend the cpu_top wraps the cpu_sub_system_axi
  assign biu_pad_awvalid = awvalid_from_pipeline;
  wire pad_biu_awready = awready_from_interconnect;

  cpu_sub_system_axi x_css (
    .biu_pad_awvalid(biu_pad_awvalid),
    .pad_biu_awready(pad_biu_awready),
    .create_en(create_en)
  );
endmodule

module tb;
  reg awvalid_from_pipeline = 0;
  reg awready_from_interconnect = 0;
  wire biu_pad_awvalid;
  wire create_en;

  x_cpu_top u_top (
    .awvalid_from_pipeline(awvalid_from_pipeline),
    .awready_from_interconnect(awready_from_interconnect),
    .biu_pad_awvalid(biu_pad_awvalid),
    .create_en(create_en)
  );

  reg [31:0] saw_create_en_x;
  reg [31:0] saw_create_en_zero;
  reg [31:0] saw_create_en_one;
  reg [31:0] num_create_en_should_be_zero_but_was_x;
  reg [31:0] num_create_en_should_be_one_but_was_x;

  initial begin
    saw_create_en_x = 0;
    saw_create_en_zero = 0;
    saw_create_en_one = 0;
    num_create_en_should_be_zero_but_was_x = 0;
    num_create_en_should_be_one_but_was_x = 0;

    #10;
    // case: both 0 → create_en should be 0
    awvalid_from_pipeline = 0; awready_from_interconnect = 0;
    #10;
    if (create_en === 1'b0) saw_create_en_zero = saw_create_en_zero + 1;
    else if (create_en === 1'bx) num_create_en_should_be_zero_but_was_x = num_create_en_should_be_zero_but_was_x + 1;
    if (create_en === 1'bx) saw_create_en_x = saw_create_en_x + 1;

    // case: 0 && 1 → 0
    awvalid_from_pipeline = 0; awready_from_interconnect = 1;
    #10;
    if (create_en === 1'b0) saw_create_en_zero = saw_create_en_zero + 1;
    else if (create_en === 1'bx) num_create_en_should_be_zero_but_was_x = num_create_en_should_be_zero_but_was_x + 1;
    if (create_en === 1'bx) saw_create_en_x = saw_create_en_x + 1;

    // case: 1 && 0 → 0
    awvalid_from_pipeline = 1; awready_from_interconnect = 0;
    #10;
    if (create_en === 1'b0) saw_create_en_zero = saw_create_en_zero + 1;
    else if (create_en === 1'bx) num_create_en_should_be_zero_but_was_x = num_create_en_should_be_zero_but_was_x + 1;
    if (create_en === 1'bx) saw_create_en_x = saw_create_en_x + 1;

    // case: 1 && 1 → 1
    awvalid_from_pipeline = 1; awready_from_interconnect = 1;
    #10;
    if (create_en === 1'b1) saw_create_en_one = saw_create_en_one + 1;
    else if (create_en === 1'bx) num_create_en_should_be_one_but_was_x = num_create_en_should_be_one_but_was_x + 1;
    if (create_en === 1'bx) saw_create_en_x = saw_create_en_x + 1;

    $finish;
  end
endmodule
"#;

fn lookup(sim: &xezim::compiler::Simulator, name: &str) -> u64 {
    sim.get_signal(name)
        .or_else(|| sim.get_signal(&format!("tb.{}", name)))
        .unwrap_or_else(|| panic!("signal not found: {}", name))
        .to_u64()
        .unwrap_or_else(|| panic!("signal {} not u64-able", name))
}

#[test]
fn cont_assign_propagates_through_4_hop_chain() {
    let sim = simulate(SRC, 200).expect("simulate failed");

    let zero_count = lookup(&sim, "saw_create_en_zero") & 0xFFFFFFFF;
    let one_count = lookup(&sim, "saw_create_en_one") & 0xFFFFFFFF;
    let x_count = lookup(&sim, "saw_create_en_x") & 0xFFFFFFFF;
    let stuck_zero = lookup(&sim, "num_create_en_should_be_zero_but_was_x") & 0xFFFFFFFF;
    let stuck_one = lookup(&sim, "num_create_en_should_be_one_but_was_x") & 0xFFFFFFFF;

    assert_eq!(
        x_count, 0,
        "create_en was X {} times — reproduces c910 wid_for_axi4 bug. \
         {} cases expected 0 got X, {} cases expected 1 got X",
        x_count, stuck_zero, stuck_one
    );
    assert_eq!(
        zero_count, 3,
        "Expected 3 cases of create_en=0, got {}",
        zero_count
    );
    assert_eq!(
        one_count, 1,
        "Expected 1 case of create_en=1, got {}",
        one_count
    );
}
