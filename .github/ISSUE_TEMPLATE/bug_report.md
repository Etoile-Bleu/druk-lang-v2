---
name: Bug Report
about: Report a bug to help us improve
title: "[BUG] "
labels: bug
---

## Description

A clear and concise description of what the bug is.

## Steps to Reproduce

Steps to reproduce the behavior, ideally with a minimal `.druk` snippet:

1. Save this source as `repro.druk`:
   ```
   // minimal Druk source that triggers the bug
   ```
2. Run `druk run repro.druk` (or `druk check repro.druk`)
3. See error / wrong output / crash

## Expected Behavior

A clear and concise description of what you expected to happen.

## Actual Behavior

A clear and concise description of what actually happened. Paste the full error message or
diagnostic output if there is one.

## Environment

- OS: [e.g. Windows 11, Ubuntu 24.04, macOS 15]
- Rust version: [`rustc --version`]
- LLVM version: [e.g. 22.1.0-rc3]
- Druk version / commit: [`git rev-parse HEAD` or the release tag]

## Additional Context

Add any other context about the problem here. For example:

- Does the issue occur consistently or intermittently?
- Does it happen only with `druk run` (JIT) or also with `druk check`?
- Is it specific to one platform?
- Has this worked before (e.g. in the original C++ implementation)?

## Severity

- [ ] Critical (compiler crashes, miscompiles, or produces undefined behavior)
- [ ] High (a documented language feature doesn't work)
- [ ] Medium (confusing diagnostic, workaround exists)
- [ ] Low (minor issue, cosmetic)
