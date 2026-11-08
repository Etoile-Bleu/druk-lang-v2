//! Writes a finished LLVM module out as a native object file in-process,
//! through LLVM's own `TargetMachine`, so `druk build` needs no separate
//! external code generator on the machine that runs it.

use std::path::Path;

use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};

use druk_diagnostics::Diagnostic;

use crate::OptLevel;

/// The CPU is "generic" so the binary still runs on a different machine than
/// the one that built it, and the relocation model is PIC so a default
/// position-independent link on Linux accepts the object.
pub(crate) fn emit_object(
    module: &Module<'_>,
    object_path: &Path,
    opt: OptLevel,
) -> Result<(), Diagnostic> {
    if object_path.to_str().is_none() {
        return Err(Diagnostic::error(format!(
            "the path {} has characters druk cannot write an object file to",
            object_path.display()
        )));
    }

    Target::initialize_native(&InitializationConfig::default()).map_err(|err| {
        Diagnostic::error(format!("could not set up LLVM for this machine: {err}"))
    })?;

    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple)
        .map_err(|err| Diagnostic::error(format!("LLVM does not support this machine: {err}")))?;
    let machine = target
        .create_target_machine(
            &triple,
            "generic",
            "",
            opt.to_inkwell(),
            RelocMode::PIC,
            CodeModel::Default,
        )
        .ok_or_else(|| {
            Diagnostic::error("LLVM could not create a code generator for this machine")
        })?;

    module.set_triple(&triple);
    module.set_data_layout(&machine.get_target_data().get_data_layout());

    machine
        .write_to_file(module, FileType::Object, object_path)
        .map_err(|err| {
            Diagnostic::error(format!(
                "LLVM failed to write the native object file: {err}"
            ))
        })
}
