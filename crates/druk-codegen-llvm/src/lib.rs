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

use inkwell::context::Context;
use inkwell::OptimizationLevel;

use druk_diagnostics::Diagnostic;
use druk_ir::Module as IrModule;

use codegen::CodeGen;
use runtime_fns::RuntimeFns;

/// Compiles `module` and runs its top-level program via LLVM's JIT.
pub fn run(module: &IrModule) -> Result<(), Diagnostic> {
    let context = Context::create();
    let llvm_module = context.create_module("druk");
    let builder = context.create_builder();
    let value_ty = context.struct_type(
        &[context.i64_type().into(), context.i64_type().into()],
        false,
    );

    let runtime = RuntimeFns::declare(&context, &llvm_module);

    let mut functions = Vec::with_capacity(module.functions.len());
    for function in &module.functions {
        let param_types = vec![value_ty.into(); function.arity];
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
            &function.body,
            false,
        )?;
    }
    codegen.compile_function(main_value, 0, module.main.locals, &module.main.body, true)?;

    let engine = llvm_module
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|err| {
            Diagnostic::error(format!("failed to create the JIT execution engine: {err}"))
        })?;
    codegen.runtime.map_into(&engine);

    unsafe {
        let jit_main = engine
            .get_function::<unsafe extern "C" fn()>("main")
            .map_err(|err| Diagnostic::error(format!("failed to locate compiled main: {err}")))?;
        jit_main.call();
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
        run(&ir).expect("module should compile and run");
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
