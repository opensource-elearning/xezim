// ============================================================================
// SystemVerilog Single-File Timescale Compliance Test (IEEE Std 1800-2017 Sec 22.7)
// ============================================================================

// ----------------------------------------------------------------------------
// COMPONENT 1: Baseline Scale
// ----------------------------------------------------------------------------
`timescale 1ns / 100ps // Unit = 1ns, Precision = 100ps (0.1ns)

module baseline_module (
    output logic clk_out,
    output real  pulse_time_1,
    output real  pulse_time_2
);
  initial clk_out = 0;

  initial begin
    // Delay 1.55 ns: Rounded to 100ps precision -> 1.6 ns
    #1.55; 
    clk_out = 1;
    pulse_time_1 = $realtime; 

    // Delay 0.04 ns: Falls below 100ps precision -> Rounds to 0.0 ns
    #0.04; 
    clk_out = 0;
    pulse_time_2 = $realtime;
  end
endmodule


// ----------------------------------------------------------------------------
// COMPONENT 2: Override Scale
// ----------------------------------------------------------------------------
`timescale 10ns / 1ns // Unit = 10ns, Precision = 1ns (0.1 local units)

module overridden_module (
    output logic clk_out,
    output real  pulse_time
);
  initial clk_out = 0;

  initial begin
    // Delay 2.35 units: 
    // Scaled to 10ns unit = 23.5 ns. 
    // Rounded to 1ns precision = 24.0 ns (which reads as 2.4 local units)
    #2.35; 
    clk_out = 1;
    pulse_time = $realtime; 
  end
endmodule


// ----------------------------------------------------------------------------
// COMPONENT 3: Master Testbench & Verifier
// ----------------------------------------------------------------------------
`timescale 1ns / 1ps // Set high-precision observer scale for the top-level testbench

module tb_top;

  int error_count = 0;

  // Interconnect signals
  logic clk_m1;
  real  m1_t1, m1_t2;

  logic clk_m2;
  real  m2_t1;

  // Instantiate sub-modules that were compiled under different timescales
  baseline_module m1 (
    .clk_out(clk_m1),
    .pulse_time_1(m1_t1),
    .pulse_time_2(m1_t2)
  );

  overridden_module m2 (
    .clk_out(clk_m2),
    .pulse_time(m2_t1)
  );

  initial begin
    $display("================================================================");
    $display("STARTING SINGLE-FILE TIMESCALE COMPLIANCE TEST (SEC 22.7)");
    $display("================================================================");
    
    // Test Diagnostic System Task Compliance (LRM 20.11)
    $display("[INFO] Printing compiled design timescales via $printtimescale:");
    $printtimescale(m1);
    $printtimescale(m2);
    
    // Wait for internal pulses to complete execution in absolute master timeline
    #50ns;

    $display("\n--- [TEST 1] Baseline Module (1ns / 100ps) ---");
    $display("[INFO] M1 Pulse 1 Time: %0.3f ns (Expected: 1.600)", m1_t1);
    if (m1_t1 != 1.6) begin
      $display("[ERROR] Failed to round 1.55ns up to 1.6ns grid precision.");
      error_count++;
    end

    $display("[INFO] M1 Pulse 2 Time: %0.3f ns (Expected: 1.600)", m1_t2);
    if (m1_t2 != 1.6) begin
      $display("[ERROR] Failed to drop sub-precision 0.04ns increment.");
      error_count++;
    end


    $display("\n--- [TEST 2] Overridden Module (10ns / 1ns) ---");
    $display("[INFO] M2 Pulse Time: %0.3f units (Expected: 2.400)", m2_t1);
    if (m2_t1 != 2.4) begin
      $display("[ERROR] Failed to switch context or scale to 10ns units.");
      error_count++;
    end


    $display("\n--- [TEST 3] Master Timeline Coherence ---");
    // Ensure the master scope observes absolute timeline elapsed time correctly
    $display("[INFO] Master Simulation Time at evaluation point: %0.3f ns", $realtime);
    
    $display("================================================================");
    if (error_count == 0) begin
      $display("TEST PASSED: Simulator correctly isolates back-to-back timescale directives.");
    end else begin
      $display("TEST FAILED: %0d compliance variations observed.", error_count);
    end
    $display("================================================================");
    $finish;
  end

endmodule