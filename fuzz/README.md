# Fuzzing

Three `cargo-fuzz` targets feed arbitrary bytes (decoded lossily as UTF-8) to the front end:

- `lexer`: `druk_lexer::tokenize`
- `parser`: `druk_parser::parse`
- `check`: `parse`, then `druk_hir::check` when parsing succeeded

The compiler must answer any input with diagnostics. A panic, stack overflow, hang or out-of-memory is a bug.

## Run locally

Needs nightly Rust and `cargo install cargo-fuzz`.

    cargo +nightly fuzz run parser -- examples -max_total_time=60

On Windows (MSVC) the sanitizer DLL must be on `PATH`, for example
`<VS>\VC\Tools\MSVC\<version>\bin\Hostx64\x64` (it holds `clang_rt.asan_dynamic-x86_64.dll`).

The `examples` argument seeds the run with `examples/*.druk`; inputs the fuzzer finds interesting are kept in `fuzz/corpus/` (not committed). A crash is saved under `fuzz/artifacts/<target>/`; replay it with
`cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<file>`, then add a regression test next to the fix.

CI runs every target nightly (5 minutes) and for 60 seconds on pull requests that touch the front end (`.github/workflows/fuzz.yml`).
