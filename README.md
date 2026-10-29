# Druk

Druk (འབྲུག་) is a compiled programming language whose keywords, identifiers, and numeric literals
are written in Dzongkha, Bhutan's national language, using the Tibetan (Uchen) script.

This is a from-scratch Rust rewrite of the original C++ implementation
([Etoile-Bleu/druk-lang](https://github.com/Etoile-Bleu/druk-lang)), built as a layered Cargo
workspace instead of a single sprawling source tree.

## Workspace layout

```
crates/
  druk-diagnostics/   spans, diagnostics, ariadne-based rendering
  druk-lexer/          source text -> token stream (logos, Tibetan script aware)
  druk-ast/            syntax tree (arena + typed IDs)
  druk-parser/         token stream -> AST (hand-written recursive descent + Pratt parsing)
  druk-hir/            name resolution + type checking
  druk-ir/             backend-agnostic intermediate representation
  druk-codegen-llvm/   LLVM backend (inkwell) — the only crate depending on LLVM
  druk-gc/             mark-and-sweep, handle-based heap
  druk-runtime/        array/dict/string/struct runtime support
  druk-driver/         pipeline orchestration
  druk/                CLI binary
```

See `docs/dzongkha_reference.md` for the language's keyword table, and `examples/*.druk` for
sample programs.

## Status

The full pipeline (lexer, parser, HIR, IR, LLVM codegen, JIT execution) runs end to end. All 10
bundled `examples/*.druk` programs compile and execute correctly via `druk run`. Structs, the
dzongkha builtins (`len`, `push`, ...), and lambdas that capture a variable from an enclosing
scope are not implemented yet — none of the bundled examples need them.

## Building

Requires a recent stable Rust toolchain, plus LLVM 22 for `druk-codegen-llvm` (the only crate that
depends on it, via `inkwell`).

1. Install LLVM 22 with static libraries (a prebuilt `clang+llvm-22.*` archive from
   [the LLVM releases page](https://github.com/llvm/llvm-project/releases) works; it must ship
   `bin/llvm-config`).
2. Point `llvm-sys` at it: `export LLVM_SYS_221_PREFIX=/path/to/clang+llvm-22.x`.
3. On Windows, some prebuilt LLVM archives list `xml2s.lib` as a system dependency via
   `llvm-config --system-libs` without actually shipping it, which fails the link step with
   `LNK1181`. Work around it with an empty stub library on the linker search path:
   ```sh
   # from an MSVC developer shell, in some empty directory:
   echo "" > empty.c && cl.exe /c empty.c
   lib.exe /OUT:xml2s.lib empty.obj
   export RUSTFLAGS="-L /path/to/that/directory"
   ```
4. Build and test as usual:
   ```sh
   cargo build --workspace
   cargo test --workspace
   cargo run -p druk -- run examples/06_recursion.druk
   ```

Every other crate in the workspace builds and tests without any of the above.
