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

module sv_constraint_foreach ;

  typedef bit [31:0] u32_t ;

  // Unpacked fixed-size array to isolate loop mechanics from dynamic allocation variables
  u32_t test_array[4];

  initial begin
    // Instantiate macro to track test failures
    `SVTEST_INIT

    $display("--- Starting Minimal 'foreach' Constraint Verification ---");

    // Randomize using a highly predictable incremental mathematical constraint
    void'(std::randomize(test_array) with {
      foreach (test_array[i]) {
        test_array[i] == i + 5; 
      }
    });

    // Display results for immediate human debugging visibility
    $display("Resulting Array: %p", test_array);

    // Automated Self-Checking Logic
    // Expected values: test_array[0]=5, test_array[1]=6, test_array[2]=7, test_array[3]=8
    foreach (test_array[i]) begin
      `SVTEST_CHECK(
        (test_array[i] == i + 5), 
        $sformatf("Foreach loop error! test_array[%0d] expected %0d, but got %0d", i, (i + 5), test_array[i])
      )
    end

    // Evaluate global status and conclude test
    `SVTEST_PASSFAIL
  end

endmodule
