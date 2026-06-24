//! Second-pass `should_fail` lint.
//!
//! Additive error detection for illegal SystemVerilog constructs that the main
//! parse/elaborate path is too permissive to reject. Runs AFTER a successful
//! elaboration in `--compile` mode and, on any violation, emits an error and a
//! non-zero exit — so sv-tests `should_fail` cases are correctly rejected.
//!
//! IMPORTANT: this pass NEVER changes the existing compile/elaborate behavior.
//! It only inspects the already-built AST + elaborated module and *adds*
//! diagnostics, so a clean design cannot regress unless a check is imprecise.
//! Every check is kept conservative (only fire on a definite LRM violation),
//! and the whole pass is validated to hold the static-suite baseline (1005).

use xezim_core::ast::decl::{
    ClassDeclaration, ClassItem, ClassMethodKind, ClassQualifier, ModuleItem, TypedefDeclaration,
};
use xezim_core::ast::expr::{ExprKind, NumberLiteral};
use xezim_core::ast::types::DataType;
use xezim_core::elaborate::ElaboratedModule;
use xezim_core::SourceDefinition;

/// Run the second-pass lint over every top-level definition. Returns a list of
/// error messages (empty == clean).
pub fn lint_should_fail(defs: &[&SourceDefinition], elab: &ElaboratedModule) -> Vec<String> {
    let mut errs = Vec::new();
    for def in defs {
        match def {
            SourceDefinition::Class(c) => check_class(c, &mut errs),
            SourceDefinition::Typedef(t) => check_typedef(t, elab, &mut errs),
            SourceDefinition::Module(m) => {
                for it in &m.items {
                    check_module_item(it, elab, &mut errs);
                }
            }
            SourceDefinition::Interface(m) => {
                for it in &m.items {
                    check_module_item(it, elab, &mut errs);
                }
            }
            SourceDefinition::Program(m) => {
                for it in &m.items {
                    check_module_item(it, elab, &mut errs);
                }
            }
            SourceDefinition::Package(_) => {}
        }
    }
    errs
}

/// Classes can appear nested inside module/interface/program bodies.
fn check_module_item(item: &ModuleItem, elab: &ElaboratedModule, errs: &mut Vec<String>) {
    match item {
        ModuleItem::ClassDeclaration(c) => check_class(c, errs),
        ModuleItem::TypedefDeclaration(t) => check_typedef(t, elab, errs),
        ModuleItem::DataDeclaration(d) => check_enum_type(&d.data_type, elab, errs),
        ModuleItem::NetDeclaration(d) => check_enum_type(&d.data_type, elab, errs),
        _ => {}
    }
}

/// §6.19: in an enum with an explicit base type, a member whose value is a
/// *sized* literal constant must match the base-type width.
fn check_enum_type(dt: &DataType, elab: &ElaboratedModule, errs: &mut Vec<String>) {
    let DataType::Enum(et) = dt else { return };
    let Some(base) = &et.base_type else { return };
    let w = xezim_core::elaborate::resolve_type_width(
        base,
        Some(&elab.parameters),
        Some(&elab.typedefs),
    );
    if w == 0 {
        return;
    }
    for m in &et.members {
        if let Some(init) = &m.init {
            if let ExprKind::Number(NumberLiteral::Integer { size: Some(s), .. }) = &init.kind {
                if *s != w {
                    errs.push(format!(
                        "enum member '{}': sized literal width {} differs from the enum base \
                         width {} (LRM 1800-2017 §6.19)",
                        m.name.name, s, w
                    ));
                }
            }
        }
    }
}

/// §8.20: a `pure virtual` method (or any method qualified `pure`) is legal
/// only inside a *virtual* (abstract) class or an interface class. A concrete
/// class declaring one is an error.
fn check_class(c: &ClassDeclaration, errs: &mut Vec<String>) {
    if !c.virtual_kw && !c.is_interface {
        for item in &c.items {
            if let ClassItem::Method(m) = item {
                let pure = m.qualifiers.contains(&ClassQualifier::Pure)
                    || matches!(m.kind, ClassMethodKind::PureVirtual(_));
                if pure {
                    errs.push(format!(
                        "class '{}': a pure virtual method is illegal in a non-virtual, \
                         non-interface class (LRM 1800-2017 §8.20)",
                        c.name.name
                    ));
                    break;
                }
            }
        }
    }
    // Recurse into nested classes regardless of this class's kind.
    for item in &c.items {
        if let ClassItem::Class(nested) = item {
            check_class(nested, errs);
        }
    }
}

/// §6.18: a non-forward `typedef <T> name;` whose base type `<T>` is a bare,
/// undeclared simple identifier is an error. Conservative: only fires when the
/// base is a single-segment name (no `::`), is not a built-in keyword type, and
/// is absent from every elaborated type namespace (typedefs, classes, enums,
/// interfaces, packages, parameters).
fn check_typedef(t: &TypedefDeclaration, elab: &ElaboratedModule, errs: &mut Vec<String>) {
    if t.forward {
        return;
    }
    check_enum_type(&t.data_type, elab, errs);
    if let DataType::TypeReference { name, .. } = &t.data_type {
        // package-qualified (`pkg::T`) — out of scope for this conservative check
        if name.scope.is_some() {
            return;
        }
        let n = &name.name.name;
        if n.is_empty() {
            return;
        }
        if is_builtin_type(n) {
            return;
        }
        let known = elab.typedefs.contains_key(n)
            || elab.classes.contains_key(n)
            || elab.enum_members.contains_key(n)
            || elab.interfaces.contains(n)
            || elab.packages.contains(n)
            || elab.parameters.contains_key(n);
        if !known {
            errs.push(format!(
                "typedef '{}': base type '{}' is not declared (LRM 1800-2017 §6.18)",
                t.name.name, n
            ));
        }
    }
}

fn is_builtin_type(n: &str) -> bool {
    matches!(
        n,
        "bit" | "logic" | "reg" | "byte" | "shortint" | "int" | "longint" | "integer"
            | "time" | "real" | "shortreal" | "realtime" | "string" | "chandle" | "event"
            | "void" | "wire" | "tri" | "wand" | "wor" | "uwire" | "signed" | "unsigned"
            | "genvar" | "type" | "enum" | "struct" | "union" | "process" | "supply0"
            | "supply1"
    )
}
