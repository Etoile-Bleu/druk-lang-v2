# Contributing to Druk

Thank you for your interest in contributing to Druk. This document provides guidelines for participating in the project.

## Git Workflow

### Branch Naming

Create branches with clear, descriptive names:

- `feat/<feature-name>` - New features or enhancements
- `fix/<bug-description>` - Bug fixes
- `docs/<doc-name>` - Documentation changes
- `refactor/<scope>` - Code refactoring without changing behavior
- `test/<test-scope>` - Adding or modifying tests
- `chore/<task>` - Dependencies, configuration, build changes
- `ci/<scope>` - Continuous integration changes
- `build/<scope>` - Build system changes

Examples:

```
feat/struct-literals
fix/lexer-tibetan-digit-overflow
docs/update-dzongkha-reference
test/array-set-bounds
chore/bump-inkwell
ci/add-fuzzing-workflow
```

### Creating a Pull Request

1. Push your branch to GitHub
2. Open a PR with a clear title matching commit format (see below)
3. Reference related issues: `Closes #123` or `Related to #456`
4. Fill out the PR template completely
5. Ensure all CI checks pass (format, lint, tests)
6. Request review from at least one maintainer

The PR title must follow the commit message format. Example:

```
feat(parser): add struct literal syntax
fix(gc): correct free-list reuse after a nested collection
docs(architecture): document the CodegenBackend port boundary
```

### Branch Protection Rules

The `main` branch has the following protections:

- Direct pushes are blocked (PRs only)
- All GitHub Actions checks must pass (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`, on both Linux and Windows)
- Branch must be up to date with `main`

## Commit Conventions

Commit messages follow the Conventional Commits format: `<type>(<scope>): <subject>`

### Types

- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation
- `style` - Code formatting (no logic changes, e.g. `cargo fmt`)
- `refactor` - Code refactoring without changing behavior
- `test` - Adding or modifying tests
- `chore` - Dependency, config, or tooling changes
- `perf` - Performance improvements
- `build` - Build system changes
- `ci` - Continuous integration changes

### Scope (optional)

Indicates the affected crate or layer:

- `lexer` - `druk-lexer` (tokenizing Dzongkha source)
- `ast` - `druk-ast` (the arena-based syntax tree)
- `parser` - `druk-parser` (recursive descent + Pratt parsing)
- `hir` - `druk-hir` (name resolution and type checking)
- `ir` - `druk-ir` (the backend-agnostic intermediate representation)
- `codegen` - `druk-codegen-llvm` (the LLVM backend)
- `gc` - `druk-gc` (the mark-and-sweep heap)
- `runtime` - `druk-runtime` (the `extern "C"` ABI boundary)
- `driver` - `druk-driver` (pipeline orchestration)
- `cli` - `druk` (the command-line binary)
- `diagnostics` - `druk-diagnostics` (spans, errors, rendering)

### Subject

- Use imperative mood: "add struct support" not "added struct support"
- Do not capitalize the first letter
- Do not end with a period
- Limit to 50 characters
- Be specific and descriptive

### Examples

```
feat(parser): add struct literal syntax
fix(codegen): pass RtValue through pointers instead of sret/byval
docs(readme): document the LLVM setup prerequisites
refactor(parser): extract statement parsing into its own module
test(gc): cover collection triggered during a nested call
chore(deps): bump inkwell to 0.10
```

### Body and Footer (optional)

If the commit needs more explanation, add a blank line followed by the body:

```
fix(codegen): pass RtValue through pointers instead of sret/byval

The Windows x64 sret/byval convention this crate hardcoded is wrong
on the x86-64 SysV ABI (Linux, macOS), which returns and passes the
same 16-byte struct directly in registers instead of through a
hidden pointer. Passing RtValue through an explicit pointer on every
platform removes the ambiguity entirely.

Closes #19
```

## Code Standards

Run these before committing:

```bash
cargo fmt              # Format
cargo clippy --workspace --all-targets -- -D warnings   # Lint, warnings are errors
cargo test --workspace # Run tests
```

- No `unwrap()`, `expect()`, or `panic!()` outside of tests or the dedicated `ice!()` path for
  genuine compiler-bug invariants (this is a workspace-level deny `clippy` lint, not a style
  preference) — see `ErrorHandling.md` in the `DrukCompiler` reference if you have access to it,
  or simply: every user-facing failure (bad token, parse error, unresolved name, type mismatch) is
  a `Diagnostic` returned through a `Result`, never a panic.
- `druk-lexer`, `druk-ast`, `druk-parser`, `druk-hir`, and `druk-ir` never depend on `inkwell` or
  `llvm-sys`. Only `druk-codegen-llvm` may use LLVM types.
- `druk-gc` never exposes a raw pointer across its public API — heap objects are referenced by
  `Handle` (an index), never by `*mut T`.
- No comments in the code except `// SAFETY:` above an `unsafe` block, explaining the invariant
  that makes it sound. Naming and small functions carry the explanation instead.
- New lexer tokens, AST nodes, or IR instructions ship with a test at the layer they were added,
  and if they're user-visible syntax, an `examples/*.druk` golden test exercising them end to end.

### Testing

- Write tests for new features and bug fixes
- Aim for meaningful coverage, not line coverage
- Use descriptive test names that state what's being verified, not just what's being called
- A construct with no test coverage is not done

CI enforces formatting, linting, and tests on every push and PR. Code that doesn't pass will fail
the build.

## Submitting Changes

1. Ensure all tests pass locally (`cargo test --workspace`)
2. Verify `cargo fmt --check` and `cargo clippy` are clean
3. Commit with the proper message format
4. Push to your branch
5. Open a PR and wait for review

## PR Review Checklist

Before requesting review, verify:

- [ ] Branch name follows convention
- [ ] Commits have proper format
- [ ] All code changes are intentional (no accidentally committed debug code)
- [ ] Tests added or updated for new code
- [ ] No leftover `println!`/`dbg!`/debug code
- [ ] Documentation updated if needed (README, ARCHITECTURE.md, docs/)
- [ ] All CI checks pass
- [ ] Branch is up to date with `main`

## Questions or Issues

If you have questions about contributing, open a discussion or an issue. We're here to help.

Thank you for contributing to Druk!
