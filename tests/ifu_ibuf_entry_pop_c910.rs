//! Cone-of-influence test for c910 `ct_ifu_ibuf.v` entry-write + pop-mux
//! pattern — the second remaining hypothesis from round 23 of the c910
//! memcpy investigation. See docs/c910_memcpy_investigation.md.
//!
//! Reproduces the IBUF's two key patterns in isolation:
//!  1. 32 separate registered 16-bit signals (`entry_inst_data_N`), each
//!     updated on a shared posedge clock when its per-entry one-hot
//!     `entry_data_create[N]` bit is high.
//!  2. A one-hot 32-way `case` selector that reads one entry's data into
//!     `pop_data` based on `ibuf_retire_pointer[31:0]`.
//!
//! Mimics the actual c910 IBUF (`ct_ifu_ibuf.v:5687-5718` for pop-mux and
//! `ct_ifu_ibuf_entry.v:309-353` for the entry register update). The c910
//! testbench's `gated_clk_cell` passes clk_in straight through to clk_out
//! (verified at gated_clk_cell.v:47), so the gating logic is moot —
//! every entry sees forever_cpuclk directly.
//!
//! The test writes a known sequence of halfwords into a known sequence of
//! entry indices, then advances the retire pointer through them and
//! verifies pop_data matches. If xezim mis-routes the per-entry
//! `entry_data_create[N]` flag, mis-resolves a cross-instance reg, or
//! mis-evaluates the one-hot pop case, the test catches it.

use xezim::simulate;

const SRC: &str = r#"
module tb;
  reg clk = 0;
  always #5 clk = ~clk;
  reg rst_b = 0;

  // 32 separate 16-bit registers — same shape as 32 instances of
  // ct_ifu_ibuf_entry's entry_inst_data_v[15:0].
  reg [15:0] entry_0;  reg [15:0] entry_1;  reg [15:0] entry_2;  reg [15:0] entry_3;
  reg [15:0] entry_4;  reg [15:0] entry_5;  reg [15:0] entry_6;  reg [15:0] entry_7;
  reg [15:0] entry_8;  reg [15:0] entry_9;  reg [15:0] entry_10; reg [15:0] entry_11;
  reg [15:0] entry_12; reg [15:0] entry_13; reg [15:0] entry_14; reg [15:0] entry_15;
  reg [15:0] entry_16; reg [15:0] entry_17; reg [15:0] entry_18; reg [15:0] entry_19;
  reg [15:0] entry_20; reg [15:0] entry_21; reg [15:0] entry_22; reg [15:0] entry_23;
  reg [15:0] entry_24; reg [15:0] entry_25; reg [15:0] entry_26; reg [15:0] entry_27;
  reg [15:0] entry_28; reg [15:0] entry_29; reg [15:0] entry_30; reg [15:0] entry_31;

  reg [31:0] create_ptr;       // one-hot
  reg [15:0] create_data;      // halfword to write
  reg        create_vld;       // write enable

  reg [31:0] retire_ptr;       // one-hot
  reg [15:0] pop_data;         // mux output

  // 32 per-entry data_create flags from the create_ptr one-hot, gated
  // by create_vld. Same shape as the c910 IBUF's per-entry
  // `entry_data_create[N]` driving `ct_ifu_ibuf_entry.v` instances.
  wire [31:0] data_create = {32{create_vld}} & create_ptr;

  // 32 sync registers, each updates only when its bit of data_create is high.
  always @(posedge clk or negedge rst_b) begin
    if (!rst_b) begin
      entry_0  <= 16'b0; entry_1  <= 16'b0; entry_2  <= 16'b0; entry_3  <= 16'b0;
      entry_4  <= 16'b0; entry_5  <= 16'b0; entry_6  <= 16'b0; entry_7  <= 16'b0;
      entry_8  <= 16'b0; entry_9  <= 16'b0; entry_10 <= 16'b0; entry_11 <= 16'b0;
      entry_12 <= 16'b0; entry_13 <= 16'b0; entry_14 <= 16'b0; entry_15 <= 16'b0;
      entry_16 <= 16'b0; entry_17 <= 16'b0; entry_18 <= 16'b0; entry_19 <= 16'b0;
      entry_20 <= 16'b0; entry_21 <= 16'b0; entry_22 <= 16'b0; entry_23 <= 16'b0;
      entry_24 <= 16'b0; entry_25 <= 16'b0; entry_26 <= 16'b0; entry_27 <= 16'b0;
      entry_28 <= 16'b0; entry_29 <= 16'b0; entry_30 <= 16'b0; entry_31 <= 16'b0;
    end else begin
      if (data_create[ 0]) entry_0  <= create_data;
      if (data_create[ 1]) entry_1  <= create_data;
      if (data_create[ 2]) entry_2  <= create_data;
      if (data_create[ 3]) entry_3  <= create_data;
      if (data_create[ 4]) entry_4  <= create_data;
      if (data_create[ 5]) entry_5  <= create_data;
      if (data_create[ 6]) entry_6  <= create_data;
      if (data_create[ 7]) entry_7  <= create_data;
      if (data_create[ 8]) entry_8  <= create_data;
      if (data_create[ 9]) entry_9  <= create_data;
      if (data_create[10]) entry_10 <= create_data;
      if (data_create[11]) entry_11 <= create_data;
      if (data_create[12]) entry_12 <= create_data;
      if (data_create[13]) entry_13 <= create_data;
      if (data_create[14]) entry_14 <= create_data;
      if (data_create[15]) entry_15 <= create_data;
      if (data_create[16]) entry_16 <= create_data;
      if (data_create[17]) entry_17 <= create_data;
      if (data_create[18]) entry_18 <= create_data;
      if (data_create[19]) entry_19 <= create_data;
      if (data_create[20]) entry_20 <= create_data;
      if (data_create[21]) entry_21 <= create_data;
      if (data_create[22]) entry_22 <= create_data;
      if (data_create[23]) entry_23 <= create_data;
      if (data_create[24]) entry_24 <= create_data;
      if (data_create[25]) entry_25 <= create_data;
      if (data_create[26]) entry_26 <= create_data;
      if (data_create[27]) entry_27 <= create_data;
      if (data_create[28]) entry_28 <= create_data;
      if (data_create[29]) entry_29 <= create_data;
      if (data_create[30]) entry_30 <= create_data;
      if (data_create[31]) entry_31 <= create_data;
    end
  end

  // One-hot 32-way pop-mux — same shape as ct_ifu_ibuf.v:5687-5718.
  always @(*) begin
    case (retire_ptr)
      32'h00000001: pop_data = entry_0;
      32'h00000002: pop_data = entry_1;
      32'h00000004: pop_data = entry_2;
      32'h00000008: pop_data = entry_3;
      32'h00000010: pop_data = entry_4;
      32'h00000020: pop_data = entry_5;
      32'h00000040: pop_data = entry_6;
      32'h00000080: pop_data = entry_7;
      32'h00000100: pop_data = entry_8;
      32'h00000200: pop_data = entry_9;
      32'h00000400: pop_data = entry_10;
      32'h00000800: pop_data = entry_11;
      32'h00001000: pop_data = entry_12;
      32'h00002000: pop_data = entry_13;
      32'h00004000: pop_data = entry_14;
      32'h00008000: pop_data = entry_15;
      32'h00010000: pop_data = entry_16;
      32'h00020000: pop_data = entry_17;
      32'h00040000: pop_data = entry_18;
      32'h00080000: pop_data = entry_19;
      32'h00100000: pop_data = entry_20;
      32'h00200000: pop_data = entry_21;
      32'h00400000: pop_data = entry_22;
      32'h00800000: pop_data = entry_23;
      32'h01000000: pop_data = entry_24;
      32'h02000000: pop_data = entry_25;
      32'h04000000: pop_data = entry_26;
      32'h08000000: pop_data = entry_27;
      32'h10000000: pop_data = entry_28;
      32'h20000000: pop_data = entry_29;
      32'h40000000: pop_data = entry_30;
      32'h80000000: pop_data = entry_31;
      default     : pop_data = 16'hxxxx;
    endcase
  end

  // Captured pop_data at each step, so the test can read them afterwards.
  reg [15:0] cap0, cap1, cap2, cap3;
  reg [15:0] cap_e2, cap_e5, cap_e17, cap_e31;

  initial begin
    create_ptr = 32'b0; create_data = 16'b0; create_vld = 0; retire_ptr = 32'h1;
    cap0 = 0; cap1 = 0; cap2 = 0; cap3 = 0;
    cap_e2 = 0; cap_e5 = 0; cap_e17 = 0; cap_e31 = 0;

    @(posedge clk);
    rst_b = 1;

    // Round 1: write 0xd70b into entry 2, 0x4758 into entry 5,
    // 0x5847 into entry 17, 0xe39d into entry 31. Same pattern as
    // 4 distinct halfwords landing in 4 distinct one-hot entries.
    @(posedge clk);
    #1;  // settle propagation of create_ptr/create_vld before clock edge
    create_ptr = 32'h00000004;  // bit 2 set → entry_2
    create_data = 16'hd70b;
    create_vld = 1;
    @(posedge clk);  // posedge fires: entry_2 <= d70b
    #1;
    create_ptr = 32'h00000020;  // bit 5
    create_data = 16'h4758;
    @(posedge clk);
    #1;
    create_ptr = 32'h00020000;  // bit 17
    create_data = 16'h5847;
    @(posedge clk);
    #1;
    create_ptr = 32'h80000000;  // bit 31
    create_data = 16'he39d;
    @(posedge clk);
    #1;
    create_vld = 0;
    @(posedge clk);

    // Read entries directly + via pop mux.
    cap_e2  = entry_2;
    cap_e5  = entry_5;
    cap_e17 = entry_17;
    cap_e31 = entry_31;

    retire_ptr = 32'h00000004; #1; cap0 = pop_data;  // entry 2 → 0xd70b
    retire_ptr = 32'h00000020; #1; cap1 = pop_data;  // entry 5 → 0x4758
    retire_ptr = 32'h00020000; #1; cap2 = pop_data;  // entry 17 → 0x5847
    retire_ptr = 32'h80000000; #1; cap3 = pop_data;  // entry 31 → 0xe39d

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
fn ibuf_entry_write_and_pop_mux_routes_correctly() {
    let sim = simulate(SRC, 500).expect("simulate failed");

    // Direct register reads.
    let e2  = lookup(&sim, "cap_e2")  & 0xFFFF;
    let e5  = lookup(&sim, "cap_e5")  & 0xFFFF;
    let e17 = lookup(&sim, "cap_e17") & 0xFFFF;
    let e31 = lookup(&sim, "cap_e31") & 0xFFFF;
    assert_eq!(e2,  0xd70b, "entry_2 should be 0xd70b, got 0x{:04x}", e2);
    assert_eq!(e5,  0x4758, "entry_5 should be 0x4758, got 0x{:04x}", e5);
    assert_eq!(e17, 0x5847, "entry_17 should be 0x5847, got 0x{:04x}", e17);
    assert_eq!(e31, 0xe39d, "entry_31 should be 0xe39d, got 0x{:04x}", e31);

    // Pop-mux reads.
    let p0 = lookup(&sim, "cap0") & 0xFFFF;
    let p1 = lookup(&sim, "cap1") & 0xFFFF;
    let p2 = lookup(&sim, "cap2") & 0xFFFF;
    let p3 = lookup(&sim, "cap3") & 0xFFFF;
    assert_eq!(p0, 0xd70b, "pop_data for entry 2 mismatch: 0x{:04x}",  p0);
    assert_eq!(p1, 0x4758, "pop_data for entry 5 mismatch: 0x{:04x}",  p1);
    assert_eq!(p2, 0x5847, "pop_data for entry 17 mismatch: 0x{:04x}", p2);
    assert_eq!(p3, 0xe39d, "pop_data for entry 31 mismatch: 0x{:04x}", p3);
}
