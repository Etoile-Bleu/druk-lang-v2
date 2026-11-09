#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let (ast, diagnostics) = druk_parser::parse(&source);
    if diagnostics.is_empty() {
        let _ = druk_hir::check(ast);
    }
});
