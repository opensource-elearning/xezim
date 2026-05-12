//! xezim — SystemVerilog bytecode interpreter.
//!
//! Parsing, elaboration, and shared runtime primitives live in the
//! sibling `xezim-core` crate. This crate adds the event-driven
//! interpreter (`simulator`) and bytecode IR (`bytecode`).
//!
//! For ahead-of-time native compilation, use the `xezim-b` crate.

pub mod compiler;
pub mod tbb;

// Re-export xezim-core surface so existing `xezim::...` paths keep working.
pub use xezim_core::{
    ast, diagnostics, lexer, log_eprintln, log_println, parse, parse_and_elaborate_multi,
    parse_str, preprocessor, read_compiled, set_log_file, sv_parser, tokenize_file, write_compiled,
    ParseResult, SourceDefinition, XEZIM_BYTECODE_MAGIC,
};

/// Simulate a single source string.
pub fn simulate(source: &str, max_time: u64) -> Result<compiler::Simulator, String> {
    simulate_multi(
        &[source.to_string()],
        max_time,
        None,
        &[],
        &[],
        None,
        false,
        None,
        None,
        &[],
        false,
        &[],
        1,
        None,
        &[],
        None,
        None,
        None,
        None,
        false,
    )
}

pub fn simulate_multi(
    sources: &[String],
    max_time: u64,
    top_module_name: Option<&str>,
    include_dirs: &[String],
    source_paths: &[String],
    settle_limit: Option<u32>,
    activity_mon: bool,
    sdf_file: Option<&str>,
    sdf_select: Option<xezim_core::sdf::DelaySelect>,
    defines: &[(String, Option<String>)],
    aitrace: bool,
    plusargs: &[String],
    threads: usize,
    xtrace_file: Option<&str>,
    xtrace_scopes: &[String],
    emit_hypergraph: Option<&str>,
    load_partition: Option<&str>,
    write_profile: Option<&str>,
    profile_input: Option<&str>,
    collapse_islands: bool,
) -> Result<compiler::Simulator, String> {
    let total_start = std::time::Instant::now();
    let compilation_start = std::time::Instant::now();
    let (definitions, elab) = parse_and_elaborate_multi(
        sources,
        top_module_name,
        include_dirs,
        source_paths,
        defines,
    )?;

    // Drop the parsed-AST table now that elaborate has produced ElaboratedModule.
    // Nothing downstream (Simulator::new, sim.run, SDF parse) needs it. Without
    // this the AHashMap<String, SourceDefinition> (Rc<ModuleDeclaration> for
    // ~hundreds of c910 modules) sits in RSS for the entire 3-min simulation.
    // Measured: c910 hello peak 9.98 GB → ~8 GB after explicit drop.
    drop(definitions);

    let mut sim = compiler::Simulator::new(elab, max_time);
    if let Some(limit) = settle_limit {
        sim.settle_limit = limit;
    }
    sim.activity_mon = activity_mon;
    sim.aitrace_mode = aitrace;
    sim.xtrace_file = xtrace_file.map(|s| s.to_string());
    sim.xtrace_scopes = xtrace_scopes.to_vec();
    sim.set_plusargs(plusargs);
    sim.set_threads(threads);

    if let Some(sdf_path) = sdf_file {
        let sdf_content = std::fs::read_to_string(sdf_path)
            .map_err(|e| format!("Cannot read SDF file '{}': {}", sdf_path, e))?;
        let sdf = xezim_core::sdf::parse_sdf(&sdf_content)
            .map_err(|e| format!("SDF parse error: {}", e))?;
        let select = sdf_select.unwrap_or(xezim_core::sdf::DelaySelect::Typ);
        let sim_timescale = 1e-9;
        let annotation = xezim_core::sdf::annotate_sdf(&sdf, sim_timescale, select);
        sim.sdf_annotation = Some(annotation);
    }
    sim.compile();
    eprintln!(
        "[PHASE] compilation: {:.1}ms",
        compilation_start.elapsed().as_secs_f64() * 1000.0
    );

    if let Some(path) = emit_hypergraph {
        let t = std::time::Instant::now();
        // Phase-2 profile-guided emission: --profile-input takes
        // precedence; falls back to static weights without it.
        let prof = if let Some(pp) = profile_input {
            match compiler::simulator::Phase2Profile::load_from_file(pp) {
                Ok(p) => {
                    eprintln!(
                        "[PART] using profile {} ({} blocks, {} signals)",
                        pp,
                        p.edge_block_exec_ns.len(),
                        p.signal_toggle_count.len()
                    );
                    Some(p)
                }
                Err(e) => {
                    eprintln!("[PART] failed to load profile {}: {} — falling back to static", pp, e);
                    None
                }
            }
        } else {
            None
        };
        // Phase-3 island analysis (optional). Computes which blocks
        // MUST be co-located across cores (async-reset cones, comb
        // SCCs) and collapses them into super-vertices in the emitted
        // hypergraph. Without --collapse-islands, every block is its
        // own vertex (Phase 1/2 behavior).
        let islands = if collapse_islands {
            Some(sim.compute_phase3_islands())
        } else {
            None
        };
        let result = sim.emit_edge_block_hypergraph_full(
            path,
            prof.as_ref(),
            islands.as_deref(),
        );
        match result {
            Ok((nv, ne)) => eprintln!(
                "[PART] hypergraph written to {} ({} vertices, {} hyperedges, weights={}, islands={}) in {:.1}ms",
                path,
                nv,
                ne,
                if prof.is_some() { "profile" } else { "static" },
                if islands.is_some() { "phase3" } else { "off" },
                t.elapsed().as_secs_f64() * 1000.0
            ),
            Err(e) => eprintln!("[PART] failed to write hypergraph to {}: {}", path, e),
        }
    }
    if let Some(path) = load_partition {
        let t = std::time::Instant::now();
        match sim.load_partition_file(path) {
            Ok((n, parts)) => eprintln!(
                "[PART] loaded partition from {} ({} assignments, k={}) in {:.1}ms",
                path,
                n,
                parts,
                t.elapsed().as_secs_f64() * 1000.0
            ),
            Err(e) => eprintln!("[PART] failed to load partition from {}: {}", path, e),
        }
    }

    let simulation_start = std::time::Instant::now();
    sim.simulate();
    eprintln!(
        "[PHASE] simulation: {:.1}ms",
        simulation_start.elapsed().as_secs_f64() * 1000.0
    );

    if let Some(path) = write_profile {
        let t = std::time::Instant::now();
        match sim.write_phase2_profile(path) {
            Ok(()) => eprintln!(
                "[PART] profile written to {} in {:.1}ms (set XEZIM_EDGE_BLOCK_STATS=1 to populate)",
                path,
                t.elapsed().as_secs_f64() * 1000.0
            ),
            Err(e) => eprintln!("[PART] failed to write profile to {}: {}", path, e),
        }
    }

    let total_elapsed = total_start.elapsed();
    eprintln!(
        "[PHASE] total: {:.1}ms",
        total_elapsed.as_secs_f64() * 1000.0
    );
    eprintln!("------------------------------");
    eprintln!("Simulation finished at time {}", sim.time);
    Ok(sim)
}
