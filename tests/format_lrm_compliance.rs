//! Issues #24 and #25: LRM §21.2.1 / §21.3 string-formatting compliance.
//!
//!   $swrite/$sformat   were a silent no-op outside PURE_SV_LRM mode, and only
//!                      the FIRST format string in an argument list was
//!                      honoured — every later argument was dropped.
//!   %m                 consumed an argument slot, so `$display("%m")` (no
//!                      further args) printed nothing at all.
//!   %d/%o              defaulted to minimal width instead of the §21.2.1.3
//!                      type width; '-' and '+' flags were echoed literally.
//!   %.2f/%.1e          precision was not parsed; %e used Rust's `e2`
//!                      exponent form instead of C's `e+02`.
//!   %u/%z              unformatted dumps were unimplemented, and the string
//!                      pipeline dropped NUL bytes / UTF-8-expanded bytes
//!                      above 0x7F.
//!   %v                 strength format was unimplemented (drive strengths
//!                      were parsed but discarded).

use xezim::simulate;

const FMT: &str = r#"
module tb;
  string s;
  bit [3:0] part_a;
  initial begin
    // §21.3.3 $sformat writes its destination in the DEFAULT mode.
    $sformat(s, "Value is %0d and status is %s", 42, "OK");
    $display("T1=[%s]", s);
    // §21.2.1.2: several format strings interleave in one $swrite; nothing
    // is dropped and no implicit spaces appear.
    $swrite(s, "Hex %h ", 8'hA5, "Bin %b ", 4'b1100, "Done.");
    $display("T2=[%s]", s);
    // Self-referencing append: the old value is read before the write.
    s = "Initial ";
    $swrite(s, s, "Appended ", "42");
    $display("T3=[%s]", s);
    // $swriteb renders unconsumed args in binary.
    part_a = 4'b1010;
    $swriteb(s, part_a);
    $display("T4=[%s]", s);
    // Flags and widths (§21.2.1.2/.3): minimal %0b, zero-pad, left-justify,
    // forced sign, and the type-width default for %d.
    $sformat(s, "%0b|%016b|%-08d|%+08d|%d", 8'b01101001, 8'b01101001, 123, 123, 123);
    $display("T5=[%s]", s);
    // %o full width is ceil(bits/3); %c honours width; %.2f / %.1e precision.
    $sformat(s, "%o|%04c|%.2f|%.1e", 8'o75, "c", 3.14159, 100.0);
    $display("T6=[%s]", s);
    // %m consumes no argument — bare and inline forms both print the scope.
    $display("T7=[%m]");
  end
endmodule
"#;

const UNFORMATTED: &str = r#"
module tb;
  string s;
  int len_u, b0, b1, b2, b3;
  int len_z, z0, z4, z5;
  initial begin
    // §21.2.1.4 %u: aval as raw little-endian bytes, whole-word padded.
    s = $sformatf("%u", 24'b01000001_01000010_00000010);
    len_u = s.len();
    b0 = s.getc(0); b1 = s.getc(1); b2 = s.getc(2); b3 = s.getc(3);
    // §21.2.1.4 %z: per word the bval (x/z mask) then the aval, little-endian.
    s = $sformatf("%z", 12'b1010_0101_zx10);
    len_z = s.len();
    z0 = s.getc(0); z4 = s.getc(4); z5 = s.getc(5);
  end
endmodule
"#;

const STRENGTH: &str = r#"
module tb;
  wire net_strong_1, net_pull_0, net_supply_z, net_weak_x;
  string s;
  assign (strong1, strong0) net_strong_1 = 1'b1;
  assign (pull1,   pull0)   net_pull_0   = 1'b0;
  assign (strong1, highz0)  net_supply_z = 1'bz;
  assign (weak1,   weak0)   net_weak_x   = 1'bx;
  initial begin
    #1;
    $display("V=[%v|%v|%v|%v]", net_strong_1, net_pull_0, net_supply_z, net_weak_x);
  end
endmodule
"#;

fn line(sim: &xezim::compiler::Simulator, tag: &str) -> String {
    sim.output
        .iter()
        .map(|o| o.message.clone())
        .find(|m| m.starts_with(tag))
        .unwrap_or_else(|| panic!("no output line starting with {}", tag))
}

fn u(sim: &xezim::compiler::Simulator, n: &str) -> u64 {
    sim.get_signal(n)
        .or_else(|| sim.get_signal(&format!("tb.{}", n)))
        .unwrap_or_else(|| panic!("signal not found: {}", n))
        .to_u64()
        .unwrap_or_else(|| panic!("{} not u64-able", n))
        & 0xFFFF_FFFF
}

#[test]
fn swrite_sformat_write_their_destination_with_interleaved_formats() {
    let sim = simulate(FMT, 100).expect("simulate failed");
    assert_eq!(line(&sim, "T1="), "T1=[Value is 42 and status is OK]");
    assert_eq!(line(&sim, "T2="), "T2=[Hex a5 Bin 1100 Done.]");
    assert_eq!(line(&sim, "T3="), "T3=[Initial Appended 42]");
    assert_eq!(line(&sim, "T4="), "T4=[1010]");
    assert_eq!(
        line(&sim, "T5="),
        "T5=[1101001|0000000001101001|123     |+0000123|        123]"
    );
    assert_eq!(line(&sim, "T6="), "T6=[075|000c|3.14|1.0e+02]");
    assert_eq!(line(&sim, "T7="), "T7=[tb]");
}

#[test]
fn unformatted_u_and_z_dumps_produce_word_padded_byte_streams() {
    let sim = simulate(UNFORMATTED, 100).expect("simulate failed");
    assert_eq!(u(&sim, "len_u"), 4, "%u pads to a whole 32-bit word");
    assert_eq!(u(&sim, "b0"), 0x02);
    assert_eq!(u(&sim, "b1"), 0x42);
    assert_eq!(u(&sim, "b2"), 0x41);
    assert_eq!(u(&sim, "b3"), 0x00, "trailing pad NUL is content");
    assert_eq!(u(&sim, "len_z"), 8, "%z emits bval+aval words");
    assert_eq!(u(&sim, "z0"), 0x0C, "bval word low byte (x/z mask)");
    assert_eq!(u(&sim, "z4"), 0x56, "aval word low byte");
    assert_eq!(u(&sim, "z5"), 0x0A, "aval word high byte");
}

#[test]
fn strength_format_reports_the_driving_strength() {
    let sim = simulate(STRENGTH, 100).expect("simulate failed");
    assert_eq!(line(&sim, "V="), "V=[St1|Pu0|HiZ|WeX]");
}
