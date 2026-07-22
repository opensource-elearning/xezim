//! Cone-of-influence test for c910 IBUF's 32-instance pattern — the
//! remaining hypothesis after rounds 23-24 ruled out precode, entry+pop
//! and casez dispatch in isolation. See docs/c910_memcpy_investigation.md.
//!
//! Unlike the prior `ifu_ibuf_entry_pop_c910` test, which used 32
//! separate `reg` declarations in ONE module, this reproduces the
//! REAL c910 IBUF structure: 32 SEPARATE INSTANCES of a child module
//! (mirroring `ct_ifu_ibuf_entry`), each owning its own registered
//! 16-bit output port. The parent reads each instance's output via
//! 32 cross-instance wires (`entry_inst_data_N` connecting to
//! `x_ct_ifu_ibuf_entry_N.entry_inst_data_v`).
//!
//! If xezim's cross-instance signal resolution loses a write to one
//! specific instance's port, this test will catch it.

use xezim::simulate;

const SRC: &str = r#"
module entry(
  input  clk,
  input  rst_b,
  input  write_en,
  input  [15:0] write_data,
  output reg [15:0] inst_data
);
  always @(posedge clk or negedge rst_b) begin
    if (!rst_b) inst_data <= 16'b0;
    else if (write_en) inst_data <= write_data;
  end
endmodule

module tb;
  reg clk = 0;
  always #5 clk = ~clk;
  reg rst_b = 0;

  reg [31:0] write_ptr;       // one-hot
  reg [15:0] write_data;
  reg        write_vld;
  wire [31:0] data_create = {32{write_vld}} & write_ptr;

  // 32 instances, each driving a parent-side wire entry_inst_data_N.
  wire [15:0] entry_inst_data_0;  wire [15:0] entry_inst_data_1;
  wire [15:0] entry_inst_data_2;  wire [15:0] entry_inst_data_3;
  wire [15:0] entry_inst_data_4;  wire [15:0] entry_inst_data_5;
  wire [15:0] entry_inst_data_6;  wire [15:0] entry_inst_data_7;
  wire [15:0] entry_inst_data_8;  wire [15:0] entry_inst_data_9;
  wire [15:0] entry_inst_data_10; wire [15:0] entry_inst_data_11;
  wire [15:0] entry_inst_data_12; wire [15:0] entry_inst_data_13;
  wire [15:0] entry_inst_data_14; wire [15:0] entry_inst_data_15;
  wire [15:0] entry_inst_data_16; wire [15:0] entry_inst_data_17;
  wire [15:0] entry_inst_data_18; wire [15:0] entry_inst_data_19;
  wire [15:0] entry_inst_data_20; wire [15:0] entry_inst_data_21;
  wire [15:0] entry_inst_data_22; wire [15:0] entry_inst_data_23;
  wire [15:0] entry_inst_data_24; wire [15:0] entry_inst_data_25;
  wire [15:0] entry_inst_data_26; wire [15:0] entry_inst_data_27;
  wire [15:0] entry_inst_data_28; wire [15:0] entry_inst_data_29;
  wire [15:0] entry_inst_data_30; wire [15:0] entry_inst_data_31;

  entry e0 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 0]), .write_data(write_data), .inst_data(entry_inst_data_0));
  entry e1 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 1]), .write_data(write_data), .inst_data(entry_inst_data_1));
  entry e2 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 2]), .write_data(write_data), .inst_data(entry_inst_data_2));
  entry e3 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 3]), .write_data(write_data), .inst_data(entry_inst_data_3));
  entry e4 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 4]), .write_data(write_data), .inst_data(entry_inst_data_4));
  entry e5 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 5]), .write_data(write_data), .inst_data(entry_inst_data_5));
  entry e6 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 6]), .write_data(write_data), .inst_data(entry_inst_data_6));
  entry e7 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 7]), .write_data(write_data), .inst_data(entry_inst_data_7));
  entry e8 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 8]), .write_data(write_data), .inst_data(entry_inst_data_8));
  entry e9 (.clk(clk), .rst_b(rst_b), .write_en(data_create[ 9]), .write_data(write_data), .inst_data(entry_inst_data_9));
  entry e10(.clk(clk), .rst_b(rst_b), .write_en(data_create[10]), .write_data(write_data), .inst_data(entry_inst_data_10));
  entry e11(.clk(clk), .rst_b(rst_b), .write_en(data_create[11]), .write_data(write_data), .inst_data(entry_inst_data_11));
  entry e12(.clk(clk), .rst_b(rst_b), .write_en(data_create[12]), .write_data(write_data), .inst_data(entry_inst_data_12));
  entry e13(.clk(clk), .rst_b(rst_b), .write_en(data_create[13]), .write_data(write_data), .inst_data(entry_inst_data_13));
  entry e14(.clk(clk), .rst_b(rst_b), .write_en(data_create[14]), .write_data(write_data), .inst_data(entry_inst_data_14));
  entry e15(.clk(clk), .rst_b(rst_b), .write_en(data_create[15]), .write_data(write_data), .inst_data(entry_inst_data_15));
  entry e16(.clk(clk), .rst_b(rst_b), .write_en(data_create[16]), .write_data(write_data), .inst_data(entry_inst_data_16));
  entry e17(.clk(clk), .rst_b(rst_b), .write_en(data_create[17]), .write_data(write_data), .inst_data(entry_inst_data_17));
  entry e18(.clk(clk), .rst_b(rst_b), .write_en(data_create[18]), .write_data(write_data), .inst_data(entry_inst_data_18));
  entry e19(.clk(clk), .rst_b(rst_b), .write_en(data_create[19]), .write_data(write_data), .inst_data(entry_inst_data_19));
  entry e20(.clk(clk), .rst_b(rst_b), .write_en(data_create[20]), .write_data(write_data), .inst_data(entry_inst_data_20));
  entry e21(.clk(clk), .rst_b(rst_b), .write_en(data_create[21]), .write_data(write_data), .inst_data(entry_inst_data_21));
  entry e22(.clk(clk), .rst_b(rst_b), .write_en(data_create[22]), .write_data(write_data), .inst_data(entry_inst_data_22));
  entry e23(.clk(clk), .rst_b(rst_b), .write_en(data_create[23]), .write_data(write_data), .inst_data(entry_inst_data_23));
  entry e24(.clk(clk), .rst_b(rst_b), .write_en(data_create[24]), .write_data(write_data), .inst_data(entry_inst_data_24));
  entry e25(.clk(clk), .rst_b(rst_b), .write_en(data_create[25]), .write_data(write_data), .inst_data(entry_inst_data_25));
  entry e26(.clk(clk), .rst_b(rst_b), .write_en(data_create[26]), .write_data(write_data), .inst_data(entry_inst_data_26));
  entry e27(.clk(clk), .rst_b(rst_b), .write_en(data_create[27]), .write_data(write_data), .inst_data(entry_inst_data_27));
  entry e28(.clk(clk), .rst_b(rst_b), .write_en(data_create[28]), .write_data(write_data), .inst_data(entry_inst_data_28));
  entry e29(.clk(clk), .rst_b(rst_b), .write_en(data_create[29]), .write_data(write_data), .inst_data(entry_inst_data_29));
  entry e30(.clk(clk), .rst_b(rst_b), .write_en(data_create[30]), .write_data(write_data), .inst_data(entry_inst_data_30));
  entry e31(.clk(clk), .rst_b(rst_b), .write_en(data_create[31]), .write_data(write_data), .inst_data(entry_inst_data_31));

  reg [31:0] retire_ptr;
  reg [15:0] pop_data;
  always @(*) begin
    case (retire_ptr)
      32'h00000001: pop_data = entry_inst_data_0;
      32'h00000002: pop_data = entry_inst_data_1;
      32'h00000004: pop_data = entry_inst_data_2;
      32'h00000008: pop_data = entry_inst_data_3;
      32'h00000010: pop_data = entry_inst_data_4;
      32'h00000020: pop_data = entry_inst_data_5;
      32'h00000040: pop_data = entry_inst_data_6;
      32'h00000080: pop_data = entry_inst_data_7;
      32'h00000100: pop_data = entry_inst_data_8;
      32'h00000200: pop_data = entry_inst_data_9;
      32'h00000400: pop_data = entry_inst_data_10;
      32'h00000800: pop_data = entry_inst_data_11;
      32'h00001000: pop_data = entry_inst_data_12;
      32'h00002000: pop_data = entry_inst_data_13;
      32'h00004000: pop_data = entry_inst_data_14;
      32'h00008000: pop_data = entry_inst_data_15;
      32'h00010000: pop_data = entry_inst_data_16;
      32'h00020000: pop_data = entry_inst_data_17;
      32'h00040000: pop_data = entry_inst_data_18;
      32'h00080000: pop_data = entry_inst_data_19;
      32'h00100000: pop_data = entry_inst_data_20;
      32'h00200000: pop_data = entry_inst_data_21;
      32'h00400000: pop_data = entry_inst_data_22;
      32'h00800000: pop_data = entry_inst_data_23;
      32'h01000000: pop_data = entry_inst_data_24;
      32'h02000000: pop_data = entry_inst_data_25;
      32'h04000000: pop_data = entry_inst_data_26;
      32'h08000000: pop_data = entry_inst_data_27;
      32'h10000000: pop_data = entry_inst_data_28;
      32'h20000000: pop_data = entry_inst_data_29;
      32'h40000000: pop_data = entry_inst_data_30;
      32'h80000000: pop_data = entry_inst_data_31;
      default     : pop_data = 16'hxxxx;
    endcase
  end

  reg [15:0] cap_e2, cap_e5, cap_e17, cap_e31;
  reg [15:0] cap_p2, cap_p5, cap_p17, cap_p31;

  initial begin
    write_ptr = 32'b0; write_data = 16'b0; write_vld = 0; retire_ptr = 32'h1;

    @(posedge clk);
    rst_b = 1;

    @(posedge clk);
    #1;
    write_ptr = 32'h00000004; write_data = 16'hd70b; write_vld = 1;
    @(posedge clk); #1;
    write_ptr = 32'h00000020; write_data = 16'h4758;
    @(posedge clk); #1;
    write_ptr = 32'h00020000; write_data = 16'h5847;
    @(posedge clk); #1;
    write_ptr = 32'h80000000; write_data = 16'he39d;
    @(posedge clk); #1;
    write_vld = 0;
    @(posedge clk); #1;

    cap_e2 = entry_inst_data_2;
    cap_e5 = entry_inst_data_5;
    cap_e17 = entry_inst_data_17;
    cap_e31 = entry_inst_data_31;

    retire_ptr = 32'h00000004; #1; cap_p2  = pop_data;
    retire_ptr = 32'h00000020; #1; cap_p5  = pop_data;
    retire_ptr = 32'h00020000; #1; cap_p17 = pop_data;
    retire_ptr = 32'h80000000; #1; cap_p31 = pop_data;

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
fn cross_instance_entry_routing_correct() {
    let sim = simulate(SRC, 500).expect("simulate failed");
    let e2 = lookup(&sim, "cap_e2") & 0xFFFF;
    let e5 = lookup(&sim, "cap_e5") & 0xFFFF;
    let e17 = lookup(&sim, "cap_e17") & 0xFFFF;
    let e31 = lookup(&sim, "cap_e31") & 0xFFFF;
    assert_eq!(
        e2, 0xd70b,
        "entry 2 (instance e2.inst_data) wrong: 0x{:04x}",
        e2
    );
    assert_eq!(
        e5, 0x4758,
        "entry 5 (instance e5.inst_data) wrong: 0x{:04x}",
        e5
    );
    assert_eq!(
        e17, 0x5847,
        "entry 17 (instance e17.inst_data) wrong: 0x{:04x}",
        e17
    );
    assert_eq!(
        e31, 0xe39d,
        "entry 31 (instance e31.inst_data) wrong: 0x{:04x}",
        e31
    );

    let p2 = lookup(&sim, "cap_p2") & 0xFFFF;
    let p5 = lookup(&sim, "cap_p5") & 0xFFFF;
    let p17 = lookup(&sim, "cap_p17") & 0xFFFF;
    let p31 = lookup(&sim, "cap_p31") & 0xFFFF;
    assert_eq!(p2, 0xd70b, "pop via entry 2 mismatch: 0x{:04x}", p2);
    assert_eq!(p5, 0x4758, "pop via entry 5 mismatch: 0x{:04x}", p5);
    assert_eq!(p17, 0x5847, "pop via entry 17 mismatch: 0x{:04x}", p17);
    assert_eq!(p31, 0xe39d, "pop via entry 31 mismatch: 0x{:04x}", p31);
}
