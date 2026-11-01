//! LLVM backend: lowers a [`druk_ir::Module`] into LLVM IR and JIT-executes
//! it. The only crate in the workspace allowed to depend on LLVM: every
//! `unsafe` call here is either an `inkwell` builder call whose only
//! failure mode is a malformed IR (an internal compiler error, reported via
//! [`druk_diagnostics::ice`]) or a raw pointer operation whose contract is
//! documented at the call site.
//!
//! Every Druk runtime value is represented uniformly as a `{i64, i64}`
//! struct (tag, payload) matching [`druk_runtime::RtValue`]'s `repr(C)`
//! layout byte for byte, so the generated code and the hand-written runtime
//! agree on the ABI without either side needing type-directed codegen.

mod codegen;
mod runtime_fns;

use std::path::Path;

use inkwell::context::Context;
use inkwell::AddressSpace;
use inkwell::OptimizationLevel;

use druk_diagnostics::Diagnostic;
use druk_ir::Module as IrModule;

use codegen::{CodeGen, MainKind};
use runtime_fns::RuntimeFns;

/// Compiles `module` and runs its top-level program via LLVM's JIT, with
/// `args` as the program's own `argv`/`argc`.
pub fn run(module: &IrModule, args: &[String]) -> Result<(), Diagnostic> {
    let context = Context::create();
    let llvm_module = context.create_module("druk");
    let builder = context.create_builder();
    let value_ty = context.struct_type(
        &[context.i64_type().into(), context.i64_type().into()],
        false,
    );

    let runtime = RuntimeFns::declare(&context, &llvm_module);

    let ptr_t = context.ptr_type(AddressSpace::default());
    let mut functions = Vec::with_capacity(module.functions.len());
    for function in &module.functions {
        let mut param_types: Vec<inkwell::types::BasicMetadataTypeEnum> = vec![ptr_t.into()];
        for _ in 0..function.arity {
            param_types.push(value_ty.into());
        }
        let fn_type = value_ty.fn_type(&param_types, false);
        functions.push(llvm_module.add_function(&function.name, fn_type, None));
    }
    let main_type = context.void_type().fn_type(&[], false);
    let main_value = llvm_module.add_function("main", main_type, None);

    let mut codegen = CodeGen {
        context: &context,
        builder: &builder,
        value_ty,
        runtime,
        functions,
        locals_ptr: None,
        locals_ty: None,
    };

    for (index, function) in module.functions.iter().enumerate() {
        codegen.compile_function(
            codegen.functions[index],
            function.arity,
            function.locals,
            &function.capture_slots,
            &function.body,
            MainKind::NotMain,
        )?;
    }
    codegen.compile_function(
        main_value,
        0,
        module.main.locals,
        &[],
        &module.main.body,
        MainKind::Jit,
    )?;

    let engine = llvm_module
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|err| {
            Diagnostic::error(format!("failed to create the JIT execution engine: {err}"))
        })?;
    codegen.runtime.map_into(&engine);
    druk_runtime::set_args(args.to_vec());

    unsafe {
        let jit_main = engine
            .get_function::<unsafe extern "C" fn()>("main")
            .map_err(|err| Diagnostic::error(format!("failed to locate compiled main: {err}")))?;
        jit_main.call();
    }

    Ok(())
}

/// Compiles `module` to a native object file at `object_path`, for the
/// current host (cross-compilation to another target is out of scope for
/// now). The emitted `main` has the real C entry signature
/// `main(argc, argv) -> i32`, since the final linked executable's actual
/// OS process entry point is this `main`, unlike the JIT path's `run`,
/// where the already-running `druk` process's own `main` calls into it.
/// Linking the object file against `druk-runtime` into a finished
/// executable is the caller's job (see `druk-driver`), since it needs a
/// system linker this crate has no reason to know how to invoke.
///
/// This shells out to `llc` (LLVM's standalone static compiler, which must
/// be on `PATH`) to turn the module into an object file, rather than
/// using inkwell's own `TargetMachine::write_to_file`/
/// `write_to_memory_buffer`: that binding crashes non-deterministically on
/// this backend's own generated modules, independently of LLVM version,
/// `RelocMode`/`CodeModel`, or output method, confirmed by feeding the
/// exact same IR to `llc` directly instead, which compiles it correctly
/// and reliably every time. This points at a bug in inkwell's `TargetMachine`
/// bindings for this LLVM version rather than in the generated IR itself or
/// in LLVM's own backend, so this backend goes around it by writing
/// bitcode (a format inkwell's own, simpler serialization is reliable for)
/// and asking the real `llc` binary to do the risky part instead.
pub fn compile_to_object(module: &IrModule, object_path: &Path) -> Result<(), Diagnostic> {
    let context = Context::create();
    let llvm_module = context.create_module("druk");
    let builder = context.create_builder();
    let value_ty = context.struct_type(
        &[context.i64_type().into(), context.i64_type().into()],
        false,
    );

    let runtime = RuntimeFns::declare(&context, &llvm_module);

    let ptr_t = context.ptr_type(AddressSpace::default());
    let mut functions = Vec::with_capacity(module.functions.len());
    for function in &module.functions {
        let mut param_types: Vec<inkwell::types::BasicMetadataTypeEnum> = vec![ptr_t.into()];
        for _ in 0..function.arity {
            param_types.push(value_ty.into());
        }
        let fn_type = value_ty.fn_type(&param_types, false);
        functions.push(llvm_module.add_function(&function.name, fn_type, None));
    }
    let i32_t = context.i32_type();
    let main_type = i32_t.fn_type(&[i32_t.into(), ptr_t.into()], false);
    let main_value = llvm_module.add_function("main", main_type, None);

    let mut codegen = CodeGen {
        context: &context,
        builder: &builder,
        value_ty,
        runtime,
        functions,
        locals_ptr: None,
        locals_ty: None,
    };

    for (index, function) in module.functions.iter().enumerate() {
        codegen.compile_function(
            codegen.functions[index],
            function.arity,
            function.locals,
            &function.capture_slots,
            &function.body,
            MainKind::NotMain,
        )?;
    }
    codegen.compile_function(
        main_value,
        0,
        module.main.locals,
        &[],
        &module.main.body,
        MainKind::Native,
    )?;

    let bitcode_path = object_path.with_extension("bc");
    if !llvm_module.write_bitcode_to_path(&bitcode_path) {
        return Err(Diagnostic::error(format!(
            "failed to write bitcode to {}",
            bitcode_path.display()
        )));
    }

    // Deliberately "generic" rather than the exact host CPU: the point of
    // a native build is a binary that still runs on a different machine
    // than the one that compiled it, and tuning codegen to the build
    // machine's own CPU risks an illegal-instruction crash on an older
    // one. A later `--target-cpu` flag can offer the tighter,
    // build-machine-only tuning for whoever explicitly wants it.
    // `-relocation-model=pic`: `rustc`'s own final link defaults to a
    // position-independent executable (`-pie`) on Linux, which rejects an
    // object file compiled with `llc`'s own default (non-PIC) relocation
    // model with "relocation R_X86_64_32 cannot be used against local
    // symbol". Windows PE objects aren't affected by this distinction
    // either way, so this is safe to pass unconditionally rather than
    // branching on target OS.
    let result = std::process::Command::new("llc")
        .args([
            "-filetype=obj",
            "-mcpu=generic",
            "-O2",
            "-relocation-model=pic",
        ])
        .arg(&bitcode_path)
        .arg("-o")
        .arg(object_path)
        .output();

    let _ = std::fs::remove_file(&bitcode_path);

    let output = result.map_err(|err| {
        Diagnostic::error(format!(
            "failed to run llc (is LLVM's `llc` on PATH?): {err}"
        ))
    })?;
    if !output.status.success() {
        return Err(Diagnostic::error(format!(
            "llc failed to emit a native object file:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every compiled program in this test binary shares one process-wide
    /// heap and frame stack (both are statics inside `druk-runtime`). Now
    /// that `Heap::collect` actually reclaims memory, two of these tests
    /// running concurrently on different threads (the default `cargo test`
    /// behavior) could collect an object another test is still using, and
    /// `collection_triggered_inside_a_nested_call_spares_a_live_outer_local`
    /// deliberately forces a collection, so every test funnels through this
    /// lock via `compile_and_run` to serialize against it.
    static TEST_HEAP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn compile_and_run(source: &str) {
        let _guard = TEST_HEAP_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (ast, parse_diagnostics) = druk_parser::parse(source);
        assert!(
            parse_diagnostics.is_empty(),
            "parse diagnostics: {parse_diagnostics:?}"
        );
        let hir = druk_hir::check(ast).expect("source should type-check");
        let ir = druk_ir::lower(&hir).expect("hir should lower");
        run(&ir, &[]).expect("module should compile and run");
    }

    #[test]
    fn runs_hello_world() {
        compile_and_run(r#"བཀོད་ "Hello, Druk Lang!";"#);
    }

    #[test]
    fn runs_arithmetic() {
        compile_and_run(
            "གྲངས་ x = ༡༠;\nགྲངས་ y = ༥;\nབཀོད་ x + y;\nབཀོད་ x - y;\nབཀོད་ x * y;\nབཀོད་ x / y;",
        );
    }

    #[test]
    fn runs_recursion() {
        compile_and_run(
            "ལས་འགན་ fib(གྲངས་ n) { གལ་སྲིད་ (n <= ༡) སླར་ལོག་ n; སླར་ལོག་ fib(n - ༡) + fib(n - ༢); } བཀོད་ fib(༧);",
        );
    }

    #[test]
    fn runs_arrays() {
        compile_and_run(
            "གྲངས་[] numbers = [༡, ༢, ༣, ༤, ༥];\nབཀོད་ numbers[༠];\nབཀོད་ numbers[༤];\nnumbers[༢] = ༩༩;\nབཀོད་ numbers[༢];",
        );
    }

    #[test]
    fn runs_structs() {
        compile_and_run(
            "གྲངས་ point = {x: ༡, y: ༢};\nབཀོད་ point.x;\nབཀོད་ point.y;\npoint.y = ༩༩;\nབཀོད་ point.y;\nབཀོད་ point.missing;",
        );
    }

    #[test]
    fn runs_lambdas() {
        compile_and_run("གྲངས་ multiply = (a, b) -> a * b;\nབཀོད་ multiply(༦, ༧);");
    }

    #[test]
    fn runs_closures_that_capture_a_variable() {
        compile_and_run("གྲངས་ n = ༥;\nགྲངས་ add_n = m -> m + n;\nབཀོད་ add_n(༣);");
    }

    #[test]
    fn a_while_loop_with_many_iterations_does_not_overflow_the_stack() {
        // Every expression evaluated inside a loop body used to `alloca` at
        // the builder's current position, i.e. inside the loop's own basic
        // block: an `alloca` executed on every pass through a block grows
        // the native stack by another slot each time, since nothing
        // promotes or reclaims it without an optimization pass, which this
        // backend never runs (`OptimizationLevel::None`). A loop with more
        // than a few thousand iterations of ordinary arithmetic used to
        // overflow the stack well before this count. See
        // `CodeGen::build_entry_alloca`.
        compile_and_run(
            "གྲངས་ total = ༠;\nགྲངས་ i = ༠;\nཡང་བསྐྱར་ (i < ༡༠༠༠༠༠) {\n    total = total + i;\n    i = i + ༡;\n}\nབཀོད་ total;",
        );
    }

    #[test]
    fn runs_len_push_pop_and_typeof_on_an_array() {
        compile_and_run(
            "གྲངས་[] xs = [༡, ༢, ༣];\nབཀོད་ རིང་ཚད(xs);\nསྣོན་(xs, ༤);\nབཀོད་ རིང་ཚད(xs);\nབཀོད་ བཏོན་(xs);\nབཀོད་ རིང་ཚད(xs);\nབཀོད་ རིགས་(xs);",
        );
    }

    #[test]
    fn runs_a_closure_created_and_called_without_being_stored_in_a_local() {
        compile_and_run("གྲངས་ n = ༧;\nབཀོད་ (m -> m + n)(༡);");
    }

    #[test]
    fn runs_a_closure_created_inside_a_function_and_called_after_it_returns() {
        compile_and_run(
            "ལས་འགན་ make_adder(གྲངས་ base) {\n    སླར་ལོག་ m -> m + base;\n}\nགྲངས་ add_five = make_adder(༥);\nབཀོད་ add_five(༡༠༠);",
        );
    }

    #[test]
    fn collection_triggered_inside_a_nested_call_spares_a_live_outer_local() {
        compile_and_run(
            r#"
            ལས་འགན་ allocate_garbage() {
                ཡིག་འབྲུ་ junk = "garbage";
            }

            ལས་འགན་ run() {
                ཡིག་འབྲུ་ kept = "kept-across-collection";
                གྲངས་ i = ༠;
                ཡང་བསྐྱར་ (i < ༣༠༠) {
                    allocate_garbage();
                    i = i + ༡;
                }
                བཀོད་ kept;
            }

            run();
            "#,
        );
    }

    #[test]
    fn emits_an_object_file_for_every_bundled_example() {
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
        let entries = std::fs::read_dir(examples_dir).expect("examples dir exists");
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.extension().map(|ext| ext == "druk").unwrap_or(false) {
                let source = std::fs::read_to_string(&path).expect("example file is readable");
                let (ast, parse_diagnostics) = druk_parser::parse(&source);
                assert!(parse_diagnostics.is_empty());
                let hir = druk_hir::check(ast).expect("source should type-check");
                let ir = druk_ir::lower(&hir).expect("hir should lower");

                let mut object_path = std::env::temp_dir();
                object_path.push(format!(
                    "druk_object_test_{}_{}.o",
                    std::process::id(),
                    path.file_stem().unwrap_or_default().to_string_lossy()
                ));
                let result = compile_to_object(&ir, &object_path);
                assert!(
                    result.is_ok(),
                    "failed to emit an object for {path:?}: {result:?}"
                );
                assert!(object_path.exists());
                let _ = std::fs::remove_file(&object_path);
            }
        }
    }

    #[test]
    fn runs_every_bundled_example() {
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
        let entries = std::fs::read_dir(examples_dir).expect("examples dir exists");
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.extension().map(|ext| ext == "druk").unwrap_or(false) {
                let source = std::fs::read_to_string(&path).expect("example file is readable");
                println!("--- {path:?} ---");
                compile_and_run(&source);
            }
        }
    }
}
