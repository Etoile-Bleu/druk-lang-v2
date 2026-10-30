---
name: Feature Request
about: Suggest an idea for this project
title: "[FEATURE] "
labels: enhancement
---

## Description

A clear and concise description of the feature you're requesting.

## Problem Statement

Explain the problem this feature solves. Who would benefit from it (language users writing
`.druk` programs, compiler contributors, tooling integrators)?

## Proposed Solution

Describe how this feature should work. Provide a Druk source example if it's a language feature,
or the expected crate/API shape if it's a compiler-internal feature.

## Alternative Approaches

Describe any alternative solutions or features you've considered.

## Acceptance Criteria

What needs to be true for this feature to be considered complete?

- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Affected Layer

Which part of the pipeline does this touch?

- [ ] Lexer (`druk-lexer`)
- [ ] Parser / AST (`druk-parser`, `druk-ast`)
- [ ] Name resolution / type checking (`druk-hir`)
- [ ] IR lowering (`druk-ir`)
- [ ] LLVM codegen (`druk-codegen-llvm`)
- [ ] GC / runtime (`druk-gc`, `druk-runtime`)
- [ ] CLI / driver (`druk`, `druk-driver`)
- [ ] Tooling (LSP, editor extension, playground, not yet part of this repo)
- [ ] Other: ___________

## Additional Context

Add any other context, links, or information that might be useful.

## Priority

- [ ] Must have (blocks a real use case)
- [ ] Should have (important but not blocking)
- [ ] Nice to have (can be deferred)
