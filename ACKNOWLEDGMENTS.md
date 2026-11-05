# Acknowledgments

This project did not start from nothing. This file records, as specifically as possible, where
its ideas, architecture, and individual engineering decisions came from: the original project it
rewrites, the compilers whose structure it borrows, the crates it depends on, and the research
that shaped specific decisions along the way.

The standard for inclusion here is concrete and verifiable: a real source that a real decision in
this codebase can be traced back to, not a vague "informed by general knowledge of compilers."
See the "What this file does not claim" section at the bottom for the honest limits of that
standard.

## The original project

Druk v2 is a from-scratch Rust rewrite of
[Etoile-Bleu/druk-lang](https://github.com/Etoile-Bleu/druk-lang), the original C++
implementation. The core idea the whole project exists for, a programming language whose
keywords, identifiers, and numerals are written in Dzongkha rather than English, comes from that
project, not from this rewrite. The rewrite exists because the original's architecture had
degraded (236 C++ files with no clear module boundaries, debug artifacts and a large zip archive
committed to the repo, no CI), not because the underlying idea needed to change.

## The educational and policy context this project responds to

Druk doesn't exist in a vacuum; the reason it exists at all traces back to a specific set of
facts about computer science education in Bhutan:

- ICT has been a mandatory subject from kindergarten (PP) through class XII in Bhutanese schools
  since 2021, under the Ministry of Education's ICT Flagship program: Scratch first, then Python,
  with a Dzongkha-script keyboard and typing tutorials introduced from the earliest grades. See
  the Ministry of Education's own [ICT Curriculum site](https://sites.google.com/moe.gov.bt/thinley/home).
- Despite that early start, senior-level uptake stays low: around 6% of class XI-XII students
  choose computer science as a subject, well under the 10% target set by the Bhutan Council for
  School Examinations and Assessment (BCSEA), and some schools still teach the subject without any
  working technology actually available in the classroom. See
  [The Bhutanese, "Digital schools"](https://thebhutanese.bt/digital-schools/).
- Dzongkha, Bhutan's national language, is written in the Tibetan Uchen script. No programming
  language existed in that script before this project: students have been learning to program in a
  language and a script that are not their own, even though computer-science-education research
  finds that fluency in a student's first language eases learning early programming concepts. The
  closest prior example is [Hedy](https://2022.ecoop.org/details/ecoop-2022-papers/36/Hedy-Creating-a-gradual-programming-language),
  a Dutch gradual teaching language that now ships keywords localized into 47 languages (Arabic,
  right-to-left, among them), whose own team found from real classroom use that error messages a
  student can actually understand in their own language matter at least as much as translating the
  keywords themselves.

This is the direct motivation behind Druk's keywords, identifiers, and numerals being written in
Dzongkha rather than transliterated or kept in English, and behind this project's own "plain
diagnostics a student can read and act on" priority ranking above translation (see issue #37).

## Architecture: how the compiler is laid out

The layered, one-directional crate graph (`druk-diagnostics` -> `druk-lexer` -> `druk-ast` ->
`druk-parser` -> `druk-hir` -> `druk-ir` -> `druk-codegen-llvm`/`druk-gc`/`druk-runtime` ->
`druk-driver` -> `druk`) is modeled directly on how `rustc` itself is split into layered crates,
and on how `rust-analyzer` structures its own frontend:

- [rustc-dev-guide: compiler source layout](https://rustc-dev-guide.rust-lang.org/compiler-src.html):
  the general principle of splitting a compiler into crates with a strict dependency direction,
  mirrored in this project's own crate graph.
- [rustc-dev-guide: incremental compilation in detail](https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation-in-detail.html):
  the "pure function from input to output, identified by an ID" shape that `druk-hir`'s checker
  follows, chosen specifically so a future move to `salsa`-style incremental queries would be a
  mechanical change rather than a rewrite.
- The **arena + typed-ID** pattern used throughout `druk-ast` (`ExprId(u32)` etc. instead of
  `Box<Node>` recursion) is the same pattern `rust-analyzer` and the [`la-arena`](https://docs.rs/la-arena)
  / [`id-arena`](https://docs.rs/id-arena) crates use internally, chosen for the same reason:
  no recursive lifetime/ownership problems in Rust, and it's the prerequisite shape if an
  incremental query engine is ever added.
- The `CodegenBackend` trait boundary between `druk-ir` and `druk-codegen-llvm` plays the same
  role as [`rustc_codegen_ssa`](https://rustc-dev-guide.rust-lang.org/backend/backend-agnostic.html)
  does in `rustc`: an abstraction layer that lets the frontend (lexer through HIR/IR) stay
  completely ignorant of which backend (LLVM here; Cranelift or GCC in `rustc`'s case) actually
  emits code.

## Direct dependencies

Every crate this project links against, and why it was chosen:

| Crate | Used for | Why this one |
|---|---|---|
| [`logos`](https://docs.rs/logos) | `druk-lexer`'s tokenizer | Compiles its rules to a DFA at build time (zero runtime regex cost) and is Unicode-aware by default, though Tibetan-script identifier/keyword matching still needed explicit `#[regex]` ranges rather than relying on generic Unicode identifier classes, see the [logos Unicode support docs](https://logos.maciej.codes/unicode-support.html). |
| [`ariadne`](https://docs.rs/ariadne) | `druk-diagnostics`' rendered error output | Rustc-style multi-span diagnostics, lighter weight than `miette` for this project's needs. |
| [`inkwell`](https://github.com/TheDan64/inkwell) | `druk-codegen-llvm`'s safe-ish LLVM bindings | The standard safe wrapper over `llvm-sys` for Rust; see [sdiehl's Compiler Crates guide on inkwell](https://sdiehl.github.io/compiler-crates/inkwell.html) and [createlang.rs](https://createlang.rs/01_calculator/basic_llvm.html) for the introductory patterns `druk-codegen-llvm`'s `CodeGen { context, module, builder }` shape follows. The crash reported in [TheDan64/inkwell#736](https://github.com/TheDan64/inkwell/issues/736) is why `druk build` once shelled out to `llc`; its cause turned out to be a C-runtime mismatch on Windows, fixed by building with a static C runtime (see "Research that shaped specific decisions" below). |
| [`insta`](https://docs.rs/insta) | Snapshot testing across every crate, and the end-to-end golden-output tests in `crates/druk/tests/examples.rs` | The established Rust pattern for asserting on AST/diagnostic/output shapes that change form often, rather than hand-written `assert_eq!` on complex structures. |
| [`thiserror`](https://docs.rs/thiserror) | Structured error types across the pipeline | Standard, low-ceremony derive for Rust error enums; keeps every user-facing failure a typed `Diagnostic` instead of a panic. |
| [`clap`](https://docs.rs/clap) | The `druk` CLI's argument parsing | The de facto standard Rust CLI argument parser; keeps the `druk` binary itself free of hand-rolled arg parsing so it stays a thin driver wrapper, per `ARCHITECTURE.md`. |
| [`tracing`](https://docs.rs/tracing) / [`tracing-subscriber`](https://docs.rs/tracing-subscriber) | Structured logging through the pipeline | Standard structured-logging pair for Rust, chosen over plain `log` for span-based context across compiler passes. |
| [`find-msvc-tools`](https://docs.rs/find-msvc-tools) | `druk-driver`'s link step on Windows | Finds `link.exe` and the environment it needs (`LIB`, `PATH`) through the Visual Studio installation records. It is the same discovery `rustc` and the `cc` crate use, so `druk build` works from a plain terminal and no Rust toolchain is needed to link. |

## Research that shaped specific decisions

- **GC design**: the handle/index-based heap in `druk-gc` (no raw pointers between GC-managed
  objects, ever) follows the pattern documented in
  [lox-lang's GC implementation](https://docs.rs/lox-lang/latest/src/lox_lang/vm/gc.rs.html) and
  discussed in the ["Crafting Interpreters" garbage collection chapter](https://timmastny.rbind.io/blog/garbage-collection-crafting-interpreters/)
  write-up: using handles/indices rather than raw references is the standard way to avoid Rust's
  ownership rules fighting a graph of GC objects.
- **Parser shape**: the hand-written recursive-descent parser with Pratt-style operator-precedence
  expression parsing, rather than a parser generator (`lalrpop`/`pest`), follows the same reasoning
  `rustc` itself documents for its own hand-written parser: better control over error messages and
  recovery, no artificial lookahead limits. See
  [sdiehl's parser comparison](https://sdiehl.github.io/compiler-crates/parsing-comparison.html).
- **Testing pipeline**: the golden end-to-end `examples/*.druk` snapshot suite (issues #100/#101)
  was built after explicitly researching how professional Rust compiler projects structure
  end-to-end output verification, landing on `insta` snapshots of real `druk run` stdout rather
  than exit-code-only checks, which is how the project caught a deliberately introduced
  `BinOp::Add` -> `Sub` regression that an exit-code-only test had missed.
- **Educational-language design comparisons**: when auditing whether Druk's own feature set was
  missing anything essential for a beginner-facing language, [Hedy](https://www.hedy.org)'s own
  published design rationale was the concrete reference point, in particular its documented,
  deliberate choice to delay syntax sugar like augmented assignment (`+=`) for cognitive-load
  reasons. This is cited directly in issue discussions comparing Druk's feature set against it.
- **Repo health baseline**: the absence of a `SECURITY.md` was flagged against the
  [OpenSSF Scorecard](https://github.com/ossf/scorecard) and
  [Linux Foundation](https://www.linuxfoundation.org) baseline checklists for open-source project
  health, and against LLVM's own repository (which carries one), not against a generic sense of
  "best practice."
- **Static C runtime on Windows**: building `druk` with `-C target-feature=+crt-static`, and the
  build-time guard that enforces it, come from tracing the intermittent crash in
  [inkwell#736](https://github.com/TheDan64/inkwell/issues/736) to a mismatch between the C runtime
  the official prebuilt LLVM libraries request (`RuntimeLibrary=MT_StaticRelease`, read with
  `llvm-readobj --coff-directives`) and Rust's dynamic default. The general pattern, that LLVM and
  the project consuming it must agree on one C runtime, is described in an
  [LLVM-on-Windows thread on the Mesa mailing list](https://lists.freedesktop.org/archives/mesa-dev/2015-April/081604.html),
  and `llvm-sys` itself ties its system-library linking to `crt-static` in its
  `build.rs` (repository: [taricorp/llvm-sys.rs](https://gitlab.com/taricorp/llvm-sys.rs)), which is
  what makes this the supported combination rather than a workaround.
- **Linking without a Rust toolchain**: `druk build` runs the platform's linker directly, and
  gets the native libraries the runtime needs from `rustc --print=native-static-libs` at build
  time instead of hard-coding them. On Windows the runtime is linked against the static C
  runtime, so built programs do not import the Visual C++ runtime DLLs; the choice between
  static and dynamic is the one described in the
  [Rust reference's linkage chapter](https://doc.rust-lang.org/reference/linkage.html), which
  also recommends inspecting the resulting binary, as the project's tests do.

## What this file does not claim

A large amount of the day-to-day engineering in this project (bug fixes, refactors, most test
additions) came from reading this codebase's own code and reasoning about it directly, not from
an external source, and is not listed here for that reason: crediting "general programming
knowledge" or "how compilers typically work" as a source would make this file meaningless.
Likewise, exploratory research that informed judgment calls (roadmap prioritization, what to build
next) without pointing at one traceable decision in the code is deliberately left out rather than
padded in with a long list of search results. This file is kept honest by only growing when a
specific source can be named for a specific piece of code, not on a schedule or because growing it
looks thorough.

## Keeping this file current

Whenever a piece of code in this repository is written by consulting a specific external source
(a library's own documentation beyond its basic API signature, a blog post or article describing
an algorithm or pattern, another open-source project's implementation, a paper, a Stack Overflow
answer that supplied the actual approach, and so on), that source is added here, in the section
it best fits, as part of the same change, not as a follow-up.
