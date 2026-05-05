//! LLVM-backed JIT for bytecode blocks (alternative to cranelift).
//!
//! Feature-gated behind `jit-llvm`. Mirrors the public surface of
//! `super::jit::JitModule` so the simulator can swap backends at
//! runtime via `XEZIM_JIT_BACKEND={cranelift,llvm}`.
//!
//! Tradeoffs vs cranelift:
//! - Tighter machine code (LLVM's optimizer is much more thorough).
//! - 10-100× slower at JIT-compile time.
//! - Adds a heavy build dep (system LLVM 18 + inkwell crate).
//!
//! # Codegen overview
//!
//! Each `CompiledBlock`'s `Insn[]` becomes one LLVM function with
//! signature `i32 (i8*)`. The `i8*` is `*mut Simulator`. Returns
//! 0 on success, 1 if the X/Z prelude bailed (caller runs the
//! interpreter for that execution).
//!
//! Same conventions as the cranelift backend:
//! - One i64 alloca per VM register slot.
//! - Bridge functions imported via `add_global_mapping` so JIT'd
//!   code can call into Rust for signal load/store + NBA scheduling.
//! - X/Z prelude inlined via direct loads from Simulator's
//!   `signal_has_xz: Vec<u8>` byte array (same baked-pointer trick
//!   the cranelift backend uses).
//!
//! Many opcodes intentionally fall through to `Err(())` here too —
//! see `is_supported` in the cranelift module for the canonical
//! allowlist (kept identical to avoid divergent behavior).

#![allow(dead_code)]
#![allow(unused_imports)]

use super::bytecode::Insn;

#[cfg(feature = "jit-llvm")]
pub use enabled::*;
#[cfg(not(feature = "jit-llvm"))]
pub use stub::*;

/// Re-export the JitFn type so callers don't need to know which
/// backend produced it. Both backends emit `extern "C" fn(*mut u8) -> u32`.
pub type JitFn = super::jit::JitFn;

#[cfg(not(feature = "jit-llvm"))]
mod stub {
    use super::super::bytecode::Insn;
    use super::JitFn;

    pub struct LlvmJitModule;
    impl LlvmJitModule {
        pub fn new() -> Option<Self> {
            None
        }
        pub fn try_compile_with_xz(
            &mut self,
            _insns: &[Insn],
            _num_regs: u32,
            _xz_ptr: u64,
            _xz_len: u32,
        ) -> Option<JitFn> {
            None
        }
    }
}

#[cfg(feature = "jit-llvm")]
mod enabled {
    use super::super::bytecode::Insn;
    use super::JitFn;
    use inkwell::basic_block::BasicBlock;
    use inkwell::builder::Builder;
    use inkwell::context::Context;
    use inkwell::execution_engine::ExecutionEngine;
    use inkwell::module::Module;
    use inkwell::types::{BasicType, BasicTypeEnum, IntType};
    use inkwell::values::{
        BasicValueEnum, FunctionValue, IntValue, PointerValue,
    };
    use inkwell::{AddressSpace, IntPredicate, OptimizationLevel};
    use std::collections::HashSet;

    /// Owns the LLVM context, one persistent execution engine, and a
    /// monotonically-named module per compiled block. We use a fresh
    /// `Module` per block (cheaper than mutating one big module) and
    /// keep the engine alive across blocks so JIT'd function pointers
    /// stay valid.
    ///
    /// Implementation note: inkwell's `Context` lifetime is invariant.
    /// We leak it on first use to give all `Module`s a `'static`
    /// context — the simulator owns the only `LlvmJitModule` and lives
    /// for the entire process, so this is fine.
    pub struct LlvmJitModule {
        ctx: &'static Context,
        engine: ExecutionEngine<'static>,
        // Per-block Module wrappers. We must keep these alive: dropping
        // a Module disposes the underlying LLVMModule, which invalidates
        // the engine's references and the JIT'd function pointers.
        modules: Vec<Module<'static>>,
        next_id: u64,
    }

    impl LlvmJitModule {
        pub fn new() -> Option<Self> {
            inkwell::targets::Target::initialize_native(
                &inkwell::targets::InitializationConfig::default(),
            )
            .ok()?;
            let ctx: &'static Context = Box::leak(Box::new(Context::create()));
            // Bootstrap engine with an empty placeholder module — engine
            // takes ownership; we add per-block modules via add_module
            // later. Bridge symbols get bound per-block via
            // `register_globals` immediately after each `add_module`.
            let placeholder = ctx.create_module("xezim_jit_placeholder");
            let engine = placeholder
                .create_jit_execution_engine(OptimizationLevel::Default)
                .ok()?;
            Some(Self {
                ctx,
                engine,
                modules: Vec::new(),
                next_id: 0,
            })
        }

        pub fn try_compile_with_xz(
            &mut self,
            insns: &[Insn],
            num_regs: u32,
            xz_ptr: u64,
            xz_len: u32,
        ) -> Option<JitFn> {
            // Mirror cranelift's allowlist. Anything not in this set
            // returns None — interpreter handles the block.
            for insn in insns {
                if !is_supported(insn) {
                    return None;
                }
            }
            let mut input_ids: Vec<u32> = Vec::new();
            let mut seen = HashSet::new();
            for insn in insns {
                let id_opt = match insn {
                    Insn::LoadSignal(_, sid) | Insn::LoadSignalSigned(_, sid) => Some(*sid as u32),
                    _ => None,
                };
                if let Some(sid) = id_opt {
                    if seen.insert(sid) {
                        input_ids.push(sid);
                    }
                }
            }
            self.codegen_block(insns, num_regs, &input_ids, xz_ptr, xz_len)
        }

        fn codegen_block(
            &mut self,
            insns: &[Insn],
            num_regs: u32,
            input_ids: &[u32],
            xz_ptr: u64,
            xz_len: u32,
        ) -> Option<JitFn> {
            self.next_id += 1;
            let mod_name = format!("xezim_jit_mod_{}", self.next_id);
            let fn_name = format!("xezim_jit_block_{}", self.next_id);
            let module = self.ctx.create_module(&mod_name);

            // Declare bridge externs so calls type-check.
            declare_bridges(self.ctx, &module);

            let i32_t = self.ctx.i32_type();
            let i64_t = self.ctx.i64_type();
            let i8_t = self.ctx.i8_type();
            let ptr_t = self.ctx.ptr_type(AddressSpace::default());

            let fn_type = i32_t.fn_type(&[ptr_t.into()], false);
            let function = module.add_function(&fn_name, fn_type, None);
            let builder = self.ctx.create_builder();

            // Create CFG blocks: one entry, one fallback (return 1),
            // one exit (return 0), plus per-PC blocks for branch targets.
            let entry_bb = self.ctx.append_basic_block(function, "entry");
            let body_bb = self.ctx.append_basic_block(function, "body");
            let fallback_bb = self.ctx.append_basic_block(function, "fallback");
            let exit_bb = self.ctx.append_basic_block(function, "exit");

            let n = insns.len();
            let mut is_leader = vec![false; n.max(1)];
            is_leader[0] = true;
            for (i, insn) in insns.iter().enumerate() {
                if let Insn::BranchIfFalse(_, t) | Insn::Jump(t) = insn {
                    let t = *t as usize;
                    if t < n {
                        is_leader[t] = true;
                    }
                    if i + 1 < n {
                        is_leader[i + 1] = true;
                    }
                }
            }
            let mut pc_blocks: Vec<Option<BasicBlock>> = vec![None; n.max(1)];
            for (i, &ld) in is_leader.iter().enumerate() {
                if ld {
                    pc_blocks[i] =
                        Some(self.ctx.append_basic_block(function, &format!("pc_{}", i)));
                }
            }

            // ===== entry block: alloc reg slots =====
            builder.position_at_end(entry_bb);
            let sim_param = function
                .get_nth_param(0)
                .expect("sim param")
                .into_pointer_value();
            let mut reg_slots: Vec<PointerValue> = Vec::with_capacity(num_regs as usize);
            for r in 0..num_regs as usize {
                let slot = builder
                    .build_alloca(i64_t, &format!("r{}", r))
                    .ok()?;
                reg_slots.push(slot);
            }
            builder.build_unconditional_branch(body_bb).ok()?;

            // ===== prelude (X/Z input check) =====
            builder.position_at_end(body_bb);
            let entry_pc = pc_blocks[0].expect("pc 0 must exist");
            if std::env::var("XEZIM_JIT_SKIP_XZ").is_ok() || input_ids.is_empty() || xz_ptr == 0 {
                builder.build_unconditional_branch(entry_pc).ok()?;
            } else {
                // Inline prelude: load signal_has_xz[id] for each input id, OR them all.
                let xz_base_int = i64_t.const_int(xz_ptr, false);
                let xz_base = builder
                    .build_int_to_ptr(xz_base_int, ptr_t, "xz_base")
                    .ok()?;
                let mut acc: IntValue = i8_t.const_int(0, false);
                for &id in input_ids.iter() {
                    if id >= xz_len {
                        continue;
                    }
                    let off = i64_t.const_int(id as u64, false);
                    let p = unsafe {
                        builder
                            .build_in_bounds_gep(i8_t, xz_base, &[off], "xz_p")
                            .ok()?
                    };
                    let byte = builder
                        .build_load(i8_t, p, "xz_b")
                        .ok()?
                        .into_int_value();
                    acc = builder.build_or(acc, byte, "xz_acc").ok()?;
                }
                let zero = i8_t.const_int(0, false);
                let cmp = builder
                    .build_int_compare(IntPredicate::NE, acc, zero, "xz_any")
                    .ok()?;
                builder
                    .build_conditional_branch(cmp, fallback_bb, entry_pc)
                    .ok()?;
            }

            // ===== fallback block: return 1 =====
            builder.position_at_end(fallback_bb);
            builder
                .build_return(Some(&i32_t.const_int(1, false)))
                .ok()?;

            // ===== exit block: return 0 =====
            builder.position_at_end(exit_bb);
            builder
                .build_return(Some(&i32_t.const_int(0, false)))
                .ok()?;

            // ===== body: walk insns, switching blocks at leaders =====
            let mut live = false;
            for (i, insn) in insns.iter().enumerate() {
                if is_leader[i] {
                    let new_b = pc_blocks[i].expect("leader missing");
                    if live {
                        builder.build_unconditional_branch(new_b).ok()?;
                    }
                    builder.position_at_end(new_b);
                    live = true;
                }
                if !live {
                    continue;
                }
                match insn {
                    Insn::BranchIfFalse(cond, target) => {
                        let cv = builder
                            .build_load(i64_t, reg_slots[*cond as usize], "cv")
                            .ok()?
                            .into_int_value();
                        let zero = i64_t.const_int(0, false);
                        let bool_cv = builder
                            .build_int_compare(IntPredicate::NE, cv, zero, "bool_cv")
                            .ok()?;
                        let target_b = resolve_target(*target as usize, &pc_blocks, exit_bb);
                        let fall_b = if i + 1 < n {
                            pc_blocks[i + 1].unwrap_or(exit_bb)
                        } else {
                            exit_bb
                        };
                        builder
                            .build_conditional_branch(bool_cv, fall_b, target_b)
                            .ok()?;
                        live = false;
                    }
                    Insn::Jump(target) => {
                        let target_b = resolve_target(*target as usize, &pc_blocks, exit_bb);
                        builder.build_unconditional_branch(target_b).ok()?;
                        live = false;
                    }
                    other => {
                        if !emit_insn(
                            self.ctx,
                            &builder,
                            &module,
                            sim_param,
                            &reg_slots,
                            other,
                        ) {
                            return None;
                        }
                    }
                }
            }
            if live {
                builder.build_unconditional_branch(exit_bb).ok()?;
            }

            // Verify and compile.
            if module.verify().is_err() {
                if std::env::var("XEZIM_JIT_LLVM_DEBUG").is_ok() {
                    eprintln!("[JIT_LLVM] verify failed for block (insns={})", insns.len());
                    eprintln!("{}", module.print_to_string().to_string());
                }
                return None;
            }
            // Bind bridge symbols (must happen BEFORE add_module so
            // the engine can resolve them when JIT-compiling this
            // module).
            register_globals(&self.engine, &module);
            self.engine.add_module(&module).ok()?;
            // Look up the JIT'd function pointer. The Module must
            // outlive every call to this fn pointer — store it in
            // self.modules.
            let raw = unsafe { self.engine.get_function_address(&fn_name).ok()? };
            if raw == 0 {
                return None;
            }
            self.modules.push(module);
            Some(unsafe { std::mem::transmute::<usize, JitFn>(raw) })
        }
    }

    fn resolve_target<'a>(
        t: usize,
        pc_blocks: &[Option<BasicBlock<'a>>],
        exit_bb: BasicBlock<'a>,
    ) -> BasicBlock<'a> {
        if t < pc_blocks.len() {
            pc_blocks[t].unwrap_or(exit_bb)
        } else {
            exit_bb
        }
    }

    /// Bridge function declarations. Names match the symbols registered
    /// via `add_global_mapping` so JIT'd code can call them.
    fn declare_bridges<'ctx>(ctx: &'ctx Context, module: &Module<'ctx>) {
        let i32_t = ctx.i32_type();
        let i64_t = ctx.i64_type();
        let ptr_t = ctx.ptr_type(AddressSpace::default());

        // u64 xezim_jit_load_signal(*mut u8, u32)
        let load_sig = i64_t.fn_type(&[ptr_t.into(), i32_t.into()], false);
        module.add_function("xezim_jit_load_signal", load_sig, None);
        module.add_function("xezim_jit_load_array_elem", load_sig, None);

        // void xezim_jit_store_signal(*mut u8, u32, u64, u32)
        let store_sig = ctx.void_type().fn_type(
            &[ptr_t.into(), i32_t.into(), i64_t.into(), i32_t.into()],
            false,
        );
        module.add_function("xezim_jit_store_signal", store_sig, None);
        module.add_function("xezim_jit_schedule_nba", store_sig, None);

        // void xezim_jit_schedule_nba_bit_dyn(*mut u8, u32, u64, u64)
        let nba_bit_sig = ctx.void_type().fn_type(
            &[ptr_t.into(), i32_t.into(), i64_t.into(), i64_t.into()],
            false,
        );
        module.add_function("xezim_jit_schedule_nba_bit_dyn", nba_bit_sig, None);

        // void xezim_jit_schedule_nba_range_dyn(*mut u8, u32, u64, u64, u64)
        let nba_range_sig = ctx.void_type().fn_type(
            &[
                ptr_t.into(),
                i32_t.into(),
                i64_t.into(),
                i64_t.into(),
                i64_t.into(),
            ],
            false,
        );
        module.add_function("xezim_jit_schedule_nba_range_dyn", nba_range_sig, None);
        module.add_function("xezim_jit_blocking_assign_range_dyn", nba_range_sig, None);
    }

    fn register_globals<'ctx>(engine: &ExecutionEngine<'ctx>, module: &Module<'ctx>) {
        use super::super::jit::{
            xezim_jit_blocking_assign_range_dyn, xezim_jit_load_array_elem,
            xezim_jit_load_signal, xezim_jit_schedule_nba, xezim_jit_schedule_nba_bit_dyn,
            xezim_jit_schedule_nba_range_dyn, xezim_jit_store_signal,
        };
        for (name, addr) in [
            ("xezim_jit_load_signal", xezim_jit_load_signal as usize),
            ("xezim_jit_load_array_elem", xezim_jit_load_array_elem as usize),
            ("xezim_jit_store_signal", xezim_jit_store_signal as usize),
            ("xezim_jit_schedule_nba", xezim_jit_schedule_nba as usize),
            ("xezim_jit_schedule_nba_bit_dyn", xezim_jit_schedule_nba_bit_dyn as usize),
            ("xezim_jit_schedule_nba_range_dyn", xezim_jit_schedule_nba_range_dyn as usize),
            ("xezim_jit_blocking_assign_range_dyn", xezim_jit_blocking_assign_range_dyn as usize),
        ] {
            if let Some(f) = module.get_function(name) {
                engine.add_global_mapping(&f, addr);
            }
        }
    }

    /// Mirror of `super::jit::is_supported`. Kept identical so the
    /// llvm backend has the same allowlist as cranelift.
    fn is_supported(insn: &Insn) -> bool {
        use Insn::*;
        matches!(
            insn,
            LoadConst(..)
                | LoadSignal(..)
                | LoadSignalSigned(..)
                | Move(..)
                | BlockingAssign(..)
                | NbaAssign(..)
                | NbaAssignRange(..)
                | Add(..)
                | Sub(..)
                | Mul(..)
                | BitAnd(..)
                | BitOr(..)
                | BitXor(..)
                | BitXnor(..)
                | BitNot(..)
                | LogAnd(..)
                | LogOr(..)
                | LogNot(..)
                | Negate(..)
                | Eq(..)
                | Neq(..)
                | CaseEq(..)
                | Lt(..)
                | Leq(..)
                | Gt(..)
                | Geq(..)
                | Shl(..)
                | Shr(..)
                | AShr(..)
                | ReduceOr(..)
                | ReduceXor(..)
                | SetSigned(..)
                | Resize(..)
                | BitSelect(..)
                | BitSelectConst(..)
                | RangeSelect(..)
                | RangeSelectConst(..)
                | LoadArrayElem(..)
                | BranchIfFalse(..)
                | Jump(..)
                | NbaAssignBitDyn(..)
                | NbaAssignRangeDyn(..)
                | BlockingAssignRangeDyn(..)
                | Nop
        )
    }

    /// Emit one Insn into the current builder block. Returns false
    /// if the opcode isn't implemented (caller drops the whole module).
    fn emit_insn<'ctx>(
        ctx: &'ctx Context,
        b: &Builder<'ctx>,
        module: &Module<'ctx>,
        sim: PointerValue<'ctx>,
        regs: &[PointerValue<'ctx>],
        insn: &Insn,
    ) -> bool {
        let i64_t = ctx.i64_type();
        let i32_t = ctx.i32_type();

        let load_reg = |r: u16| -> Option<IntValue> {
            b.build_load(i64_t, regs[r as usize], "v").ok().map(|v| v.into_int_value())
        };
        let store_reg = |r: u16, v: IntValue| -> bool {
            b.build_store(regs[r as usize], v).is_ok()
        };

        use Insn::*;
        match insn {
            Nop => true,
            LoadConst(d, val) => {
                let bits = val.to_u64().unwrap_or(0);
                let c = i64_t.const_int(bits, false);
                store_reg(*d, c)
            }
            LoadSignal(d, sid) | LoadSignalSigned(d, sid) => {
                let f = match module.get_function("xezim_jit_load_signal") {
                    Some(f) => f,
                    None => return false,
                };
                let id_v = i32_t.const_int(*sid as u64, false);
                let call = match b.build_call(f, &[sim.into(), id_v.into()], "load") {
                    Ok(c) => c,
                    Err(_) => return false,
                };
                let r = match call.try_as_basic_value().left() {
                    Some(v) => v.into_int_value(),
                    None => return false,
                };
                store_reg(*d, r)
            }
            Move(d, s) => match load_reg(*s) {
                Some(v) => store_reg(*d, v),
                None => false,
            },
            Add(d, l, r) => emit_binop_int(b, regs, *d, *l, *r, |a, b1, n| {
                b.build_int_add(a, b1, n).ok()
            }),
            Sub(d, l, r) => emit_binop_int(b, regs, *d, *l, *r, |a, b1, n| {
                b.build_int_sub(a, b1, n).ok()
            }),
            Mul(d, l, r) => emit_binop_int(b, regs, *d, *l, *r, |a, b1, n| {
                b.build_int_mul(a, b1, n).ok()
            }),
            BitAnd(d, l, r) => emit_binop_int(b, regs, *d, *l, *r, |a, b1, n| b.build_and(a, b1, n).ok()),
            BitOr(d, l, r) => emit_binop_int(b, regs, *d, *l, *r, |a, b1, n| b.build_or(a, b1, n).ok()),
            BitXor(d, l, r) => emit_binop_int(b, regs, *d, *l, *r, |a, b1, n| b.build_xor(a, b1, n).ok()),
            BitXnor(d, l, r) => {
                let lv = match load_reg(*l) { Some(v) => v, None => return false };
                let rv = match load_reg(*r) { Some(v) => v, None => return false };
                let xor = match b.build_xor(lv, rv, "xnor_xor") { Ok(v) => v, Err(_) => return false };
                let not = match b.build_not(xor, "xnor_not") { Ok(v) => v, Err(_) => return false };
                store_reg(*d, not)
            }
            BitNot(d, s) => {
                let sv = match load_reg(*s) { Some(v) => v, None => return false };
                let n = match b.build_not(sv, "not") { Ok(v) => v, Err(_) => return false };
                store_reg(*d, n)
            }
            Negate(d, s) => {
                let sv = match load_reg(*s) { Some(v) => v, None => return false };
                let n = match b.build_int_neg(sv, "neg") { Ok(v) => v, Err(_) => return false };
                store_reg(*d, n)
            }
            LogAnd(d, l, r) => emit_logical(b, ctx, regs, *d, *l, *r, IntPredicate::NE, |a, b1, n| {
                b.build_and(a, b1, n).ok()
            }),
            LogOr(d, l, r) => emit_logical(b, ctx, regs, *d, *l, *r, IntPredicate::NE, |a, b1, n| {
                b.build_or(a, b1, n).ok()
            }),
            LogNot(d, s) => {
                let sv = match load_reg(*s) { Some(v) => v, None => return false };
                let zero = i64_t.const_int(0, false);
                let cmp = match b.build_int_compare(IntPredicate::EQ, sv, zero, "lognot")
                {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                let z = match b.build_int_z_extend(cmp, i64_t, "lognot_z") {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                store_reg(*d, z)
            }
            Eq(d, l, r) => emit_cmp(b, ctx, regs, *d, *l, *r, IntPredicate::EQ),
            Neq(d, l, r) => emit_cmp(b, ctx, regs, *d, *l, *r, IntPredicate::NE),
            CaseEq(d, l, r) => emit_cmp(b, ctx, regs, *d, *l, *r, IntPredicate::EQ),
            Lt(d, l, r) => emit_cmp(b, ctx, regs, *d, *l, *r, IntPredicate::ULT),
            Leq(d, l, r) => emit_cmp(b, ctx, regs, *d, *l, *r, IntPredicate::ULE),
            Gt(d, l, r) => emit_cmp(b, ctx, regs, *d, *l, *r, IntPredicate::UGT),
            Geq(d, l, r) => emit_cmp(b, ctx, regs, *d, *l, *r, IntPredicate::UGE),
            Shl(d, l, r) => emit_binop_int(b, regs, *d, *l, *r, |a, b1, n| {
                b.build_left_shift(a, b1, n).ok()
            }),
            Shr(d, l, r) => emit_binop_int(b, regs, *d, *l, *r, |a, b1, n| {
                b.build_right_shift(a, b1, false, n).ok()
            }),
            AShr(d, l, r) => emit_binop_int(b, regs, *d, *l, *r, |a, b1, n| {
                b.build_right_shift(a, b1, true, n).ok()
            }),
            ReduceOr(d, s) => {
                let sv = match load_reg(*s) { Some(v) => v, None => return false };
                let zero = i64_t.const_int(0, false);
                let cmp = match b.build_int_compare(IntPredicate::NE, sv, zero, "redor") {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                let z = match b.build_int_z_extend(cmp, i64_t, "redor_z") {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                store_reg(*d, z)
            }
            ReduceXor(d, s) => {
                // ctpop(x) & 1
                let sv = match load_reg(*s) { Some(v) => v, None => return false };
                let intrinsic_name = "llvm.ctpop.i64";
                let ctpop = match module.get_function(intrinsic_name) {
                    Some(f) => f,
                    None => {
                        let sig = i64_t.fn_type(&[i64_t.into()], false);
                        module.add_function(intrinsic_name, sig, None)
                    }
                };
                let call = match b.build_call(ctpop, &[sv.into()], "popcount") {
                    Ok(c) => c,
                    Err(_) => return false,
                };
                let cnt = match call.try_as_basic_value().left() {
                    Some(v) => v.into_int_value(),
                    None => return false,
                };
                let one = i64_t.const_int(1, false);
                let parity = match b.build_and(cnt, one, "redxor") {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                store_reg(*d, parity)
            }
            SetSigned(_) => true, // No-op in 2-state JIT (matches cranelift).
            Resize(reg, width) => {
                let w = *width;
                let sv = match load_reg(*reg) { Some(v) => v, None => return false };
                let mask = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
                let mask_v = i64_t.const_int(mask, false);
                let masked = match b.build_and(sv, mask_v, "resize") {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                store_reg(*reg, masked)
            }
            BitSelect(dest, base, idx) => {
                let bv = match load_reg(*base) { Some(v) => v, None => return false };
                let iv = match load_reg(*idx) { Some(v) => v, None => return false };
                let shifted = match b.build_right_shift(bv, iv, false, "bs_shr") {
                    Ok(v) => v, Err(_) => return false
                };
                let one = i64_t.const_int(1, false);
                let one_bit = match b.build_and(shifted, one, "bs_and") {
                    Ok(v) => v, Err(_) => return false
                };
                store_reg(*dest, one_bit)
            }
            BitSelectConst(dest, base, idx) => {
                let bv = match load_reg(*base) { Some(v) => v, None => return false };
                let off = i64_t.const_int(*idx as u64, false);
                let shifted = match b.build_right_shift(bv, off, false, "bsc_shr") {
                    Ok(v) => v, Err(_) => return false
                };
                let one = i64_t.const_int(1, false);
                let one_bit = match b.build_and(shifted, one, "bsc_and") {
                    Ok(v) => v, Err(_) => return false
                };
                store_reg(*dest, one_bit)
            }
            RangeSelect(dest, base, hi_r, lo_r) => {
                // (base >> lo) & ((1 << (hi - lo + 1)) - 1)
                let bv = match load_reg(*base) { Some(v) => v, None => return false };
                let hi = match load_reg(*hi_r) { Some(v) => v, None => return false };
                let lo = match load_reg(*lo_r) { Some(v) => v, None => return false };
                let shifted = match b.build_right_shift(bv, lo, false, "rs_shr") {
                    Ok(v) => v, Err(_) => return false
                };
                let one = i64_t.const_int(1, false);
                let diff = match b.build_int_sub(hi, lo, "rs_diff") {
                    Ok(v) => v, Err(_) => return false
                };
                let width = match b.build_int_add(diff, one, "rs_w") {
                    Ok(v) => v, Err(_) => return false
                };
                // mask = (1 << width) - 1, but width may be 64 → undefined shift.
                // Use cmov: mask = width >= 64 ? u64::MAX : (1<<width)-1
                let sixty_four = i64_t.const_int(64, false);
                let big = match b.build_int_compare(IntPredicate::UGE, width, sixty_four, "rs_big") {
                    Ok(v) => v, Err(_) => return false
                };
                let shifted_one = match b.build_left_shift(one, width, "rs_sl") {
                    Ok(v) => v, Err(_) => return false
                };
                let small_mask = match b.build_int_sub(shifted_one, one, "rs_mask_small") {
                    Ok(v) => v, Err(_) => return false
                };
                let max_mask = i64_t.const_int(u64::MAX, false);
                let mask = match b.build_select(big, max_mask, small_mask, "rs_mask") {
                    Ok(v) => v.into_int_value(), Err(_) => return false
                };
                let masked = match b.build_and(shifted, mask, "rs_and") {
                    Ok(v) => v, Err(_) => return false
                };
                store_reg(*dest, masked)
            }
            RangeSelectConst(dest, base, hi, lo) => {
                let bv = match load_reg(*base) { Some(v) => v, None => return false };
                let off = i64_t.const_int(*lo as u64, false);
                let shifted = match b.build_right_shift(bv, off, false, "rsc_shr") {
                    Ok(v) => v, Err(_) => return false
                };
                let width = (*hi - *lo + 1) as u64;
                let mask = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
                let mask_v = i64_t.const_int(mask, false);
                let masked = match b.build_and(shifted, mask_v, "rsc_and") {
                    Ok(v) => v, Err(_) => return false
                };
                store_reg(*dest, masked)
            }
            BlockingAssign(sig_id, val_reg, width) => {
                let f = match module.get_function("xezim_jit_store_signal") {
                    Some(f) => f, None => return false
                };
                let v = match load_reg(*val_reg) { Some(v) => v, None => return false };
                let id_v = i32_t.const_int(*sig_id as u64, false);
                let w = i32_t.const_int(*width as u64, false);
                b.build_call(f, &[sim.into(), id_v.into(), v.into(), w.into()], "")
                    .is_ok()
            }
            BlockingAssignRangeDyn(sig_id, hi_r, lo_r, val_reg) => {
                let f = match module.get_function("xezim_jit_blocking_assign_range_dyn") {
                    Some(f) => f, None => return false
                };
                let v = match load_reg(*val_reg) { Some(v) => v, None => return false };
                let hi = match load_reg(*hi_r) { Some(v) => v, None => return false };
                let lo = match load_reg(*lo_r) { Some(v) => v, None => return false };
                let id_v = i32_t.const_int(*sig_id as u64, false);
                b.build_call(
                    f,
                    &[sim.into(), id_v.into(), hi.into(), lo.into(), v.into()],
                    "",
                )
                .is_ok()
            }
            NbaAssign(sig_id, val_reg, width) => {
                let f = match module.get_function("xezim_jit_schedule_nba") {
                    Some(f) => f, None => return false
                };
                let v = match load_reg(*val_reg) { Some(v) => v, None => return false };
                let id_v = i32_t.const_int(*sig_id as u64, false);
                let w = i32_t.const_int(*width as u64, false);
                b.build_call(f, &[sim.into(), id_v.into(), v.into(), w.into()], "")
                    .is_ok()
            }
            NbaAssignRange(sig_id, hi, lo, val_reg) => {
                let f = match module.get_function("xezim_jit_schedule_nba_range_dyn") {
                    Some(f) => f, None => return false
                };
                let v = match load_reg(*val_reg) { Some(v) => v, None => return false };
                let id_v = i32_t.const_int(*sig_id as u64, false);
                let hi_v = i64_t.const_int(*hi as u64, false);
                let lo_v = i64_t.const_int(*lo as u64, false);
                b.build_call(
                    f,
                    &[sim.into(), id_v.into(), hi_v.into(), lo_v.into(), v.into()],
                    "",
                )
                .is_ok()
            }
            NbaAssignRangeDyn(sig_id, hi_r, lo_r, val_reg) => {
                let f = match module.get_function("xezim_jit_schedule_nba_range_dyn") {
                    Some(f) => f, None => return false
                };
                let v = match load_reg(*val_reg) { Some(v) => v, None => return false };
                let hi = match load_reg(*hi_r) { Some(v) => v, None => return false };
                let lo = match load_reg(*lo_r) { Some(v) => v, None => return false };
                let id_v = i32_t.const_int(*sig_id as u64, false);
                b.build_call(
                    f,
                    &[sim.into(), id_v.into(), hi.into(), lo.into(), v.into()],
                    "",
                )
                .is_ok()
            }
            NbaAssignBitDyn(sig_id, idx_r, val_r) => {
                let f = match module.get_function("xezim_jit_schedule_nba_bit_dyn") {
                    Some(f) => f, None => return false
                };
                let idx = match load_reg(*idx_r) { Some(v) => v, None => return false };
                let v = match load_reg(*val_r) { Some(v) => v, None => return false };
                let id_v = i32_t.const_int(*sig_id as u64, false);
                b.build_call(
                    f,
                    &[sim.into(), id_v.into(), idx.into(), v.into()],
                    "",
                )
                .is_ok()
            }
            LoadArrayElem(_, _, _) => {
                // Less common; emit a fallback for now. Block won't JIT
                // if it has this op (interpreter handles it).
                false
            }
            _ => false,
        }
    }

    fn emit_binop_int<'a, F>(
        b: &Builder<'a>,
        regs: &[PointerValue<'a>],
        d: u16,
        l: u16,
        r: u16,
        op: F,
    ) -> bool
    where
        F: FnOnce(IntValue<'a>, IntValue<'a>, &str) -> Option<IntValue<'a>>,
    {
        let lv = match b.build_load(b.get_insert_block().unwrap().get_context().i64_type(), regs[l as usize], "lv") {
            Ok(v) => v.into_int_value(),
            Err(_) => return false,
        };
        let rv = match b.build_load(b.get_insert_block().unwrap().get_context().i64_type(), regs[r as usize], "rv") {
            Ok(v) => v.into_int_value(),
            Err(_) => return false,
        };
        let res = match op(lv, rv, "binop") {
            Some(v) => v,
            None => return false,
        };
        b.build_store(regs[d as usize], res).is_ok()
    }

    fn emit_logical<'a, F>(
        b: &Builder<'a>,
        ctx: &'a Context,
        regs: &[PointerValue<'a>],
        d: u16,
        l: u16,
        r: u16,
        cmp: IntPredicate,
        op: F,
    ) -> bool
    where
        F: FnOnce(IntValue<'a>, IntValue<'a>, &str) -> Option<IntValue<'a>>,
    {
        let i64_t = ctx.i64_type();
        let lv = match b.build_load(i64_t, regs[l as usize], "lv") {
            Ok(v) => v.into_int_value(),
            Err(_) => return false,
        };
        let rv = match b.build_load(i64_t, regs[r as usize], "rv") {
            Ok(v) => v.into_int_value(),
            Err(_) => return false,
        };
        let zero = i64_t.const_int(0, false);
        let lb = match b.build_int_compare(cmp, lv, zero, "lb") {
            Ok(v) => v,
            Err(_) => return false,
        };
        let rb = match b.build_int_compare(cmp, rv, zero, "rb") {
            Ok(v) => v,
            Err(_) => return false,
        };
        let lb_z = match b.build_int_z_extend(lb, i64_t, "lz") {
            Ok(v) => v,
            Err(_) => return false,
        };
        let rb_z = match b.build_int_z_extend(rb, i64_t, "rz") {
            Ok(v) => v,
            Err(_) => return false,
        };
        let combined = match op(lb_z, rb_z, "lop") {
            Some(v) => v,
            None => return false,
        };
        b.build_store(regs[d as usize], combined).is_ok()
    }

    fn emit_cmp<'a>(
        b: &Builder<'a>,
        ctx: &'a Context,
        regs: &[PointerValue<'a>],
        d: u16,
        l: u16,
        r: u16,
        pred: IntPredicate,
    ) -> bool {
        let i64_t = ctx.i64_type();
        let lv = match b.build_load(i64_t, regs[l as usize], "lv") {
            Ok(v) => v.into_int_value(),
            Err(_) => return false,
        };
        let rv = match b.build_load(i64_t, regs[r as usize], "rv") {
            Ok(v) => v.into_int_value(),
            Err(_) => return false,
        };
        let cmp = match b.build_int_compare(pred, lv, rv, "cmp") {
            Ok(v) => v,
            Err(_) => return false,
        };
        let z = match b.build_int_z_extend(cmp, i64_t, "cmpz") {
            Ok(v) => v,
            Err(_) => return false,
        };
        b.build_store(regs[d as usize], z).is_ok()
    }
}
