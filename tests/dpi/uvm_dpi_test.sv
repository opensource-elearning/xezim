// uvm_dpi_test.sv — SystemVerilog driver for the UVM DPI smoke test.
//
// Imports the C functions defined in uvm_dpi_test.c and exercises each
// VPI/DPI primitive the minimum UVM surface needs:
//   - svDpiVersion
//   - vpi_get_vlog_info
//   - svGetScopeFromName / svGetNameFromScope
//   - svGetScope / svSetScope
//   - vpi_register_cb / vpi_remove_cb
//   - vpi_get (vpiType, vpiSize, vpiSigned)
//
// Output format matches the other DPI tests in this directory: each
// sub-test prints `RESULT: <label>: <value>` and the test driver
// asserts that all expected `RESULT: PASSED` lines appear. If any
// sub-test fails, the driver falls through to `$display("TEST_FAIL")`.

module uvm_dpi_test;
  // The C-side driver module below.
  import "DPI-C" function string uvm_dpi_test_version();
  import "DPI-C" function int    uvm_dpi_test_vlog_info(output int argc);
  import "DPI-C" function string uvm_dpi_test_scope_roundtrip(input string name);
  import "DPI-C" function int    uvm_dpi_test_scope_active(input string name);
  import "DPI-C" function chandle uvm_dpi_test_register_value_change(
      input string signal_name, output int sig_id);
  import "DPI-C" function chandle uvm_dpi_test_register_reset();
  import "DPI-C" function int    uvm_dpi_test_remove_cb(input chandle handle);
  import "DPI-C" function int    uvm_dpi_test_value_change_count();
  import "DPI-C" function int    uvm_dpi_test_reset_count();
  import "DPI-C" function int    uvm_dpi_test_vpi_get(
      input string signal_name,
      output int vpi_type,
      output int vpi_size,
      output int vpi_signed);

  // A signal so vpi_get has something to inspect and the value-change
  // callback has something to fire on.
  logic [31:0] counter;

  int failures = 0;

  initial begin
    int rc;
    string s;
    int n;
    int argc;
    chandle cb_vc;
    chandle cb_rst;
    int vt;
    int vs;
    int vsi;

    // ---- 1. svDpiVersion ----------------------------------------------
    s = uvm_dpi_test_version();
    $display("RESULT: dpi_version: %s", s);
    if (s == "" || s == "FAIL: empty version") begin
      $display("FAIL: svDpiVersion returned empty");
      failures++;
    end

    // ---- 2. vpi_get_vlog_info -----------------------------------------
    rc = uvm_dpi_test_vlog_info(argc);
    $display("RESULT: vlog_info_rc: %0d argc=%0d", rc, argc);
    if (rc != 0 || argc <= 0) begin
      $display("FAIL: vpi_get_vlog_info returned rc=%0d argc=%0d", rc, argc);
      failures++;
    end

    // ---- 3. scope round-trip ------------------------------------------
    s = uvm_dpi_test_scope_roundtrip("uvm_pkg");
    n = s.len();
    $display("RESULT: scope_roundtrip: %s (n=%0d)", s, n);
    if (n <= 0 || s != "uvm_pkg") begin
      $display("FAIL: scope round-trip mismatch (got '%s' n=%0d)", s, n);
      failures++;
    end

    // ---- 4. active scope set/get --------------------------------------
    rc = uvm_dpi_test_scope_active("uvm_test_top");
    $display("RESULT: scope_active: %0d", rc);
    if (rc != 0) begin
      $display("FAIL: scope_active returned rc=%0d", rc);
      failures++;
    end

    // ---- 5. vpi_register_cb (value change) ----------------------------
    counter = 32'h0;
    cb_vc = uvm_dpi_test_register_value_change("counter", vt);
    $display("RESULT: register_vc: %0d (sig_id=%0d)", cb_vc == null ? -1 : 0, vt);
    if (cb_vc == null) begin
      $display("FAIL: could not register value-change callback");
      failures++;
    end

    // ---- 6. vpi_register_cb (start-of-reset) --------------------------
    cb_rst = uvm_dpi_test_register_reset();
    $display("RESULT: register_rst: %0d", cb_rst == null ? -1 : 0);
    if (cb_rst == null) begin
      $display("FAIL: could not register reset callback");
      failures++;
    end

    // ---- 7. vpi_get (vpiType / vpiSize / vpiSigned) -------------------
    rc = uvm_dpi_test_vpi_get("counter", vt, vs, vsi);
    $display("RESULT: vpi_get: rc=%0d type=%0d size=%0d signed=%0d",
             rc, vt, vs, vsi);
    if (rc != 0 || vs != 32) begin
      $display("FAIL: vpi_get unexpected rc=%0d size=%0d", rc, vs);
      failures++;
    end

    // ---- 8. trigger a value change; callback fires --------------------
    // The DPI call returns void and the value-change dispatch happens
    // inside write_sig! on the next assign. We assign counter, then
    // let time advance so the callback fires synchronously.
    counter = 32'h12345678;
    #1;
    $display("RESULT: vc_count_after_assign: %0d",
             uvm_dpi_test_value_change_count());
    if (uvm_dpi_test_value_change_count() < 1) begin
      $display("FAIL: value-change callback did not fire");
      failures++;
    end

    // ---- 9. vpi_remove_cb ---------------------------------------------
    rc = uvm_dpi_test_remove_cb(cb_vc);
    $display("RESULT: remove_vc: %0d", rc);
    if (rc != 1) begin
      $display("FAIL: vpi_remove_cb returned %0d", rc);
      failures++;
    end
    rc = uvm_dpi_test_remove_cb(cb_rst);
    $display("RESULT: remove_rst: %0d", rc);
    if (rc != 1) begin
      $display("FAIL: vpi_remove_cb (reset) returned %0d", rc);
      failures++;
    end

    // ---- final -------------------------------------------------------
    if (failures == 0)
      $display("RESULT: PASSED");
    else
      $display("RESULT: FAILED with %0d errors", failures);
    $finish;
  end
endmodule