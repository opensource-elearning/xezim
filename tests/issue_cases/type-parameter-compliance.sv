// ============================================================================
// SystemVerilog Type Parameter Compliance Test Suite (IEEE Std 1800-2017 Sec 6.20.3)
// ============================================================================

module sv_type_parameter_compliance_tb;

  int error_count = 0;

  // --------------------------------------------------------------------------
  // 1. Define Various Flavors of User-Created Types
  // --------------------------------------------------------------------------
  
  // A. User-defined struct type
  typedef struct packed {
    logic [7:0] id;
    logic       valid;
  } my_struct_t;

  // B. User-defined unpacked array type
  typedef int my_array_t [];

  // C. User-defined Class Type
  class Packet;
    int payload;
    function new();
      this.payload = 32'hAABBCCDD;
    endfunction
  endclass

  // --------------------------------------------------------------------------
  // 2. Generic Container using LRM 6.20.3 Type Parameters
  // --------------------------------------------------------------------------
  // This class acts as our test vehicle, accepting ANY type as a parameter.
  class GenericWrapper #(parameter type T = int);
    T data;

    function void set(T val);
      this.data = val;
    endfunction

    function T get();
      return this.data;
    endfunction
    
    // Specifically for class type parameters: evaluates if T is a class handle 
    // and instantiates it dynamically inside the container.
    function void instantiate_if_class();
      // If T is a class, 'new' allocates it. 
      // Safe to call only when parameterized with an object type.
      this.data = new(); 
    endfunction
  endclass

  // --------------------------------------------------------------------------
  // 3. Main Test Execution
  // --------------------------------------------------------------------------
  initial begin
    $display("================================================================");
    $display("STARTING IEEE STD 1800-2017 SEC 6.20.3 TYPE PARAMETER COMPLIANCE");
    $display("================================================================");

    test_builtin_types();
    test_user_structs();
    test_user_arrays();
    test_class_as_type_parameter();

    $display("================================================================");
    if (error_count == 0) begin
      $display("TEST PASSED: Simulator handles all type parameter variations.");
    end else begin
      $display("TEST FAILED: %0d type parameter errors detected.", error_count);
    end
    $display("================================================================");
    $finish;
  end

  // --------------------------------------------------------------------------
  // Test Case A: Built-in Types (int, real, string)
  // --------------------------------------------------------------------------
  task test_builtin_types();
    GenericWrapper #(int)    int_box;
    GenericWrapper #(string) str_box;
    
    $display("[TEST] Verifying built-in types as parameters...");
    
    int_box = new();
    int_box.set(12345);
    if (int_box.get() != 12345) begin
      $display("[ERROR] Built-in 'int' type parameter failed.");
      error_count++;
    end

    str_box = new();
    str_box.set("SystemVerilog");
    if (str_box.get() != "SystemVerilog") begin
      $display("[ERROR] Built-in 'string' type parameter failed.");
      error_count++;
    end
  endtask

  // --------------------------------------------------------------------------
  // Test Case B: User-defined Structs
  // --------------------------------------------------------------------------
  task test_user_structs();
    GenericWrapper #(my_struct_t) struct_box;
    my_struct_t tx_data, rx_data;

    $display("[TEST] Verifying user-defined packed struct as parameter...");
    
    tx_data.id    = 8'h5A;
    tx_data.valid = 1'b1;

    struct_box = new();
    struct_box.set(tx_data);
    rx_data = struct_box.get();

    if (rx_data.id != 8'h5A || rx_data.valid != 1'b1) begin
      $display("[ERROR] User-defined struct type parameter failed layout retention.");
      error_count++;
    end
  endtask

  // --------------------------------------------------------------------------
  // Test Case C: User-defined Arrays
  // --------------------------------------------------------------------------
  task test_user_arrays();
    
    GenericWrapper #(my_array_t) array_box;
    my_array_t tx_arr; 
    my_array_t rx_arr;

    tx_arr = '{100, 200, 300};
    
    $display("[TEST] Verifying user-defined unpacked array as parameter...");

    array_box = new();
    array_box.set(tx_arr);
    rx_arr = array_box.get();

    if (rx_arr[0] != 100 || rx_arr[1] != 200 || rx_arr[2] != 300) begin
      $display("[ERROR] User-defined array type parameter mapping failed.");
      error_count++;
    end
  endtask

  // --------------------------------------------------------------------------
  // Test Case D: Classes passed as Type Parameters
  // --------------------------------------------------------------------------
  task test_class_as_type_parameter();
    // Parameterizing the container with our custom 'Packet' class type
    GenericWrapper #(Packet) class_box;
    Packet rx_packet;

    $display("[TEST] Verifying custom CLASS handles as type parameters...");

    class_box = new();
    
    // Call internal method that treats type 'T' as an allocatable class
    class_box.instantiate_if_class();
    
    // Retrieve the newly allocated class object handle
    rx_packet = class_box.get();

    if (rx_packet == null) begin
      $display("[ERROR] Class type parameter failed to instantiate via inner 'new()'.");
      error_count++;
    end else if (rx_packet.payload != 32'hAABBCCDD) begin
      $display("[ERROR] Properties inside the type-parameterized class are corrupted.");
      error_count++;
    end
  endtask

endmodule
