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

Early rewrite in progress — see the architecture plan this workspace was scaffolded from.

## Building

Requires a recent stable Rust toolchain. `druk-codegen-llvm` additionally requires LLVM installed
(see that crate's own notes once implemented — not yet wired up).

```sh
cargo build --workspace
cargo test --workspace
```
