//! CLASS / STRUCT behaviors recovered from the ivtest class/struct cluster.
//! Each case embeds the self-checking ivtest source inline and asserts the
//! "PASSED" marker (and absence of "FAILED"). Covers §7.2 packed structs
//! (member access on nets, member-target continuous assigns), and §8 classes
//! (property width/signedness truncation).

use xezim::simulate;

fn passes(src: &str) -> bool {
    match simulate(src, 100_000) {
        Ok(sim) => {
            let out: String = sim
                .output
                .iter()
                .map(|o| o.message.clone())
                .collect::<Vec<_>>()
                .join("\n");
            out.contains("PASSED") && !out.contains("FAILED")
        }
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// §7.2.1: packed-struct-typed NET — member read must slice the parent net.
// (ivltests/struct2.v)
// ---------------------------------------------------------------------------
#[test]
fn struct2_packed_struct_net_whole_assign_member_read() {
    assert!(passes(
        r#"
module main;
   struct packed { logic [7:0] high; logic [7:0] low; } word1;
   wire struct packed { logic [7:0] high; logic [7:0] low; } word2;
   assign word2 = word1;
   initial begin
      word1 = 16'haa_55;
      if (word1.high !== 8'haa || word1.low !== 8'h55) begin
         $display("FAILED: word1 = %h", word1); $finish;
      end
      #1;
      if (word2.high !== 8'haa || word2.low !== 8'h55) begin
         $display("FAILED: word2.high = %h, word2.low = %h", word2.high, word2.low); $finish;
      end
      $display("PASSED");
   end
endmodule
"#
    ));
}

// §7.2.1: member-target continuous assign into a packed-struct net.
// (ivltests/struct3.v)
#[test]
fn struct3_packed_struct_net_member_target_assign() {
    assert!(passes(
        r#"
module main;
   struct packed { logic [7:0] high; logic [7:0] low; } word1;
   wire struct packed { logic [7:0] high; logic [7:0] low; } word2;
   assign word2.high = word1.high;
   assign word2.low  = word1.low;
   initial begin
      word1 = 16'haa_55;
      if (word1.high !== 8'haa || word1.low !== 8'h55) begin
         $display("FAILED: word1"); $finish;
      end
      #1;
      if (word2.high !== 8'haa || word2.low !== 8'h55) begin
         $display("FAILED: word2.high = %h, word2.low = %h", word2.high, word2.low); $finish;
      end
      $display("PASSED");
   end
endmodule
"#
    ));
}

// §7.2: reading a packed-struct net member into a part-select, and a nonblocking
// write to a variable packed struct. (ivltests/struct8.v)
#[test]
fn struct8_packed_struct_net_member_partselect() {
    assert!(passes(
        r#"
module main;
   wire struct packed { logic m1; logic [7:0] m8; } foo;
   assign foo = {1'b1, 8'ha5};
   struct packed { logic [3:0] m4; logic [7:0] m8; } bar;
   initial begin
      #1;
      bar.m8 <= foo.m8[7:0];
      bar.m4 <= foo.m8[7:4];
      #1 $display("bar8=%h, bar4=%h", bar.m8, bar.m4);
      if (bar.m8 !== 8'ha5) begin $display("FAILED"); $finish; end
      if (bar.m4 !== 4'ha) begin $display("FAILED"); $finish; end
      $display("PASSED");
   end
endmodule
"#
    ));
}

// §7.2: continuous assign whose RHS reads a variable packed-struct member must
// re-fire when the whole struct is written. (ivltests/struct9.v)
#[test]
fn struct9_contassign_reads_struct_member() {
    assert!(passes(
        r#"
module main;
   wire [4:0] foo;
   struct packed { logic [3:0] bar4; logic [3:0] bar0; } bar;
   assign foo = bar.bar0;
   initial begin
      bar = 'h5a;
      #1 if (bar.bar0 !== 4'ha || bar.bar4 != 4'h5) begin
         $display("FAILED -- bar.bar0=%b, bar.bar4=%b", bar.bar0, bar.bar4); $finish;
      end
      if (foo !== 5'h0a) begin $display("FAILED -- foo=%b", foo); $finish; end
      $display("PASSED");
   end
endmodule
"#
    ));
}

// ---------------------------------------------------------------------------
// §7.2: concatenation LHS distributing bits into packed-struct-member net
// slices (`{word3.high, word3.low} = {word1.low, word1.high}`). Requires the
// concat parts' widths to come from the packed-struct field layout.
// (ivltests/struct3b.v)
#[test]
fn struct3b_concat_lhs_into_struct_member_nets() {
    assert!(passes(
        r#"
module main;
   struct packed { logic [7:0] high; logic [7:0] low; } word1;
   wire struct packed { logic [7:0] high; logic [7:0] low; } word2, word3;
   assign word2.high = word1.high;
   assign word2.low  = word1.low;
   assign {word3.high, word3.low} = {word1.low, word1.high};
   initial begin
      word1 = 16'haa_55;
      if (word1.high !== 8'haa || word1.low !== 8'h55) begin $display("FAILED: word1"); $finish; end
      #1;
      if (word2.high !== 8'haa || word2.low !== 8'h55) begin $display("FAILED: word2"); $finish; end
      if (word3.low !== 8'haa || word3.high !== 8'h55) begin $display("FAILED: word3"); $finish; end
      $display("PASSED");
   end
endmodule
"#
    ));
}

// §7.4.2: packed ARRAY of packed struct (variable) — element and member
// read/write with absolute bit offsets. (ivltests/struct_packed_array.v)
#[test]
fn struct_packed_array_var() {
    assert!(passes(
        r#"
module main;
   typedef struct packed { logic [3:0] high; logic [3:0] low; } word;
   typedef word [1:0] dword;
   dword foo;
   int   idx;
   initial begin
      foo[0].low = 1; foo[0].high = 2; foo[1].low = 3; foo[1].high = 4;
      if (foo !== 16'h4321) begin $display("FAILED -- foo=%h", foo); $finish; end
      if (foo[0] !== 8'h21) begin $display("FAILED -- foo[0]=%h", foo[0]); $finish; end
      if (foo[1] !== 8'h43) begin $display("FAILED -- foo[1]=%h", foo[1]); $finish; end
      if (foo[0].low !== 4'h1 || foo[0].high !== 4'h2) begin $display("FAILED lo"); $finish; end
      if (foo[1].low !== 4'h3 || foo[1].high !== 4'h4) begin $display("FAILED hi"); $finish; end
      idx = 0;
      if (foo[idx].low !== 4'h1) begin $display("FAILED idx0"); $finish; end
      idx = 1;
      if (foo[idx].high !== 8'h4) begin $display("FAILED idx1"); $finish; end
      $display("PASSED");
   end
endmodule
"#
    ));
}

// §7.4.2: packed ARRAY of packed struct declared as a NET, driven by member-
// target continuous assigns (`wire dword foo; assign foo[0].low = 1;`).
// (ivltests/struct_packed_array2.v)
#[test]
fn struct_packed_array2_net() {
    assert!(passes(
        r#"
module main;
   typedef struct packed { logic [3:0] high; logic [3:0] low; } word;
   typedef word [1:0] dword;
   wire dword foo;
   int  idx;
   assign foo[0].low = 1; assign foo[0].high = 2;
   assign foo[1].low = 3; assign foo[1].high = 4;
   initial begin
      #1;
      if (foo !== 16'h4321) begin $display("FAILED -- foo=%h", foo); $finish; end
      if (foo[0] !== 8'h21 || foo[1] !== 8'h43) begin $display("FAILED elem"); $finish; end
      if (foo[0].low !== 4'h1 || foo[1].high !== 4'h4) begin $display("FAILED field"); $finish; end
      idx = 1;
      if (foo[idx].high !== 8'h4) begin $display("FAILED idx"); $finish; end
      $display("PASSED");
   end
endmodule
"#
    ));
}

// §8: class property assignment truncates/sign-extends to the declared type.
// (ivltests/sv_class2.v — byte signed/unsigned)
// ---------------------------------------------------------------------------
#[test]
fn sv_class2_property_byte_truncation() {
    assert!(passes(
        r#"
program main;
   class foo_t ;
      byte signed a;
      byte unsigned b;
   endclass : foo_t
   foo_t obj;
   initial begin
      obj = new;
      obj.a = 'hfff;
      obj.b = 'hfff;
      if (obj.a != -1 || obj.b != 255) begin
         $display("FAILED -- obj.a=%0d, obj.b=%0d", obj.a, obj.b); $finish;
      end
      obj.a = obj.a + 1;
      obj.b = obj.b + 1;
      if (obj.a != 0 || obj.b != 0) begin
         $display("FAILED -- inc obj.a=%0d, obj.b=%0d", obj.a, obj.b); $finish;
      end
      $display("PASSED");
      $finish;
   end
endprogram
"#
    ));
}

// §8: shortint signed/unsigned property truncation. (ivltests/sv_class3.v)
#[test]
fn sv_class3_property_shortint_truncation() {
    assert!(passes(
        r#"
program main;
   class foo_t ;
      shortint signed a;
      shortint unsigned b;
   endclass : foo_t
   foo_t obj;
   initial begin
      obj = new;
      obj.a = 'hf_ffff;
      obj.b = 'hf_ffff;
      if (obj.a != -1 || obj.b != 65535) begin
         $display("FAILED -- obj.a=%0d, obj.b=%0d", obj.a, obj.b); $finish;
      end
      obj.a = obj.a + 1;
      obj.b = obj.b + 1;
      if (obj.a != 0 || obj.b != 0) begin
         $display("FAILED -- inc"); $finish;
      end
      $display("PASSED");
      $finish;
   end
endprogram
"#
    ));
}

// §8: mixed property types (byte/int/real/string) each behave per declared type.
// (ivltests/sv_class8.v)
#[test]
fn sv_class8_property_mixed_types() {
    assert!(passes(
        r#"
program main;
   class foo_t ;
      byte a;
      int  b;
      real c;
      string d;
   endclass : foo_t
   foo_t obj;
   initial begin
      obj = new;
      obj.a = 'hf_ff;
      obj.b = 'hf_ffffffff;
      obj.c = -1.5;
      obj.d = "-1";
      if (obj.a != -1 || obj.b != -1 || obj.c != -1.5 || obj.d != "-1") begin
         $display("FAILED -- obj.a=%0d, obj.b=%0d, obj.c=%f, obj.d=%0s", obj.a, obj.b, obj.c, obj.d); $finish;
      end
      obj.a = obj.a + 1;
      obj.b = obj.b + 1;
      obj.c = obj.c + 1.5;
      if (obj.a != 0 || obj.b != 0 || obj.c != 0.0) begin
         $display("FAILED -- inc"); $finish;
      end
      $display("PASSED");
      $finish;
   end
endprogram
"#
    ));
}

// §8: byte property truncation alongside a nested class-handle property.
// (ivltests/sv_class10.v)
#[test]
fn sv_class10_property_truncation_with_nested_class() {
    assert!(passes(
        r#"
program main;
   class bar_t;
      int a;
      int b;
   endclass
   class foo_t ;
      byte a;
      bar_t b;
   endclass : foo_t
   foo_t obj;
   bar_t tmp;
   initial begin
      obj = new;
      obj.a = 'hf_ff;
      obj.b = new;
      tmp = obj.b;
      tmp.a = 0;
      tmp.b = 1;
      if (obj.a != -1) begin $display("FAILED -- obj.a=%0d", obj.a); $finish; end
      if (tmp.a != 0 || tmp.b != 1) begin $display("FAILED -- tmp"); $finish; end
      $display("PASSED");
      $finish;
   end
endprogram
"#
    ));
}

// §8.9/§8.7: static property (shared cell, readable via null handle) + a
// property initializer overridden by the constructor. (ivltests/sv_class19.v)
#[test]
fn sv_class19_static_property_and_shallow_copy() {
    assert!(passes(
        r#"
program main;
   class foo_t ;
      static int int_incr = 1;
      int       int_value = 42;
      function new();
         int_value = int_value + int_incr;
      endfunction
   endclass : foo_t
   foo_t obj1;
   foo_t obj2;
   initial begin
      if (obj1.int_incr !== 1) begin $display("FAILED == obj1.int_incr=%0d.", obj1.int_incr); $finish; end
      obj1 = new;
      if (obj1.int_value !== 43) begin $display("FAILED -- obj1.int_value=%0d.", obj1.int_value); $finish; end
      obj2 = new obj1;
      if (obj2.int_value !== 43) begin $display("FAILED -- obj2.int_value=%0d.", obj2.int_value); $finish; end
      obj1.int_incr = 2;
      if (obj1.int_incr !== 2) begin $display("FAILED == obj1.int_incr=%0d", obj1.int_incr); $finish; end
      if (obj2.int_incr !== 2) begin $display("FAILED == obj2.int_incr=%0d", obj2.int_incr); $finish; end
      $display("PASSED");
      $finish;
   end
endprogram
"#
    ));
}

// §8.7: implicit super.new — a derived constructor with no explicit super.new
// still runs the base constructor. (ivltests/sv_class20.v)
#[test]
fn sv_class20_implicit_super_new() {
    assert!(passes(
        r#"
program main;
   class base_t ;
      int int_value;
      function new(); int_value = 42; endfunction
   endclass : base_t
   class foo_t extends base_t ;
      string str_value;
      function new(); str_value = "42"; endfunction
   endclass : foo_t
   foo_t obj1;
   initial begin
      obj1 = new;
      if (obj1.int_value !== 42) begin $display("FAILED -- obj1.int_value = %0d", obj1.int_value); $finish; end
      if (obj1.str_value != "42") begin $display("FAILED -- obj1.str_value = %0s", obj1.str_value); $finish; end
      $display("PASSED");
      $finish;
   end
endprogram
"#
    ));
}

// §8.7: implicit super.new passes the `extends Base(args)` value arguments.
// (ivltests/sv_class22.v)
#[test]
fn sv_class22_implicit_super_new_with_extends_args() {
    assert!(passes(
        r#"
program main;
   class base_t ;
      int int_value;
      function new(int val); int_value = val; endfunction
   endclass : base_t
   class foo_t extends base_t(42) ;
      string str_value;
      function new(); str_value = "42"; endfunction
   endclass : foo_t
   foo_t obj1;
   initial begin
      obj1 = new;
      if (obj1.int_value !== 42) begin $display("FAILED -- obj1.int_value = %0d", obj1.int_value); $finish; end
      if (obj1.str_value != "42") begin $display("FAILED -- obj1.str_value = %0s", obj1.str_value); $finish; end
      $display("PASSED");
      $finish;
   end
endprogram
"#
    ));
}
