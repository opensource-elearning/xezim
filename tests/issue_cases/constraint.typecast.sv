
`ifndef SVTEST_DEFS_SVH
`define SVTEST_DEFS_SVH

`define SVTEST_INIT \
    int failures = 0;

`define SVTEST_CHECK(expr, msg) \
    if (!(expr)) begin \
        failures++; \
        $display("FAIL: %s", msg); \
    end

`define SVTEST_PASSFAIL \
    if (failures == 0) begin \
        $display("TEST_PASS"); \
    end else begin \
        $display("TEST_FAIL count=%0d", failures); \
        $fatal(1); \
    end

`endif

module sv_constraint_explicit_typecast;
    `SVTEST_INIT

    logic [7:0]  A; // 8-bit variable
    logic [15:0] B; // 16-bit random variable

    initial begin
        A = 8'hFF; // 255
        
        // According to IEEE Std 1800-2017 Section 11.6.1 & 11.8.2:
        // Inside the 32'(...) explicit static cast context, the sub-expression 
        // operands (A and B) should be context-determined and sized to 32 bits 
        // prior to performing the multiplication. 
        // 255 * B < 1000 can easily be solved if handled in 32-bit space.
        // If truncated to 16-bit intermediate calculation due to solver error, 
        // the constraint solver may fail or find invalid wrapped values.
        
        if (!std::randomize(B) with {
            32'(A * B) < 32'd1000;
            B > 0;
        }) begin
            `SVTEST_CHECK(1'b0, "randomize failed due to constraint solver size-casting breakdown")
        end else begin
            // Double check standard mathematical validity of the solver choice
            `SVTEST_CHECK((32'(A) * 32'(B)) < 32'd1000, "Solver bypassed the 32-bit cast expansion requirement")
        end

        `SVTEST_PASSFAIL
    end
endmodule

