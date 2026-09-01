<!-- spell-checker:ignore cfgs nonthreaded wasip wasip2 wasmtime Rayon -->

# WASI `sort` plan

## Current structure

PR [#13910](https://github.com/uutils/coreutils/pull/13910) contains the independent final `sort --merge` flush-error fix. Its current tip is `82e437edd`.

Draft PR [#13806](https://github.com/uutils/coreutils/pull/13806) retains the complete synchronous WASI implementation as a consolidated reference. It was rebuilt directly on #13910 and pushed at `27896f108`.

The earlier six-PR decomposition remains a useful set of possible review boundaries, but it is not an active submission sequence:

1. Final merge output errors: submitted as #13910.
2. Synchronous `--check` support.
3. Behavior-preserving merge restructuring.
4. Synchronous `--merge` support.
5. Bounded synchronous external sorting.
6. Threaded-WASI selection.

Adding new follow-mode behavior is outside this work. The integration annotations instead use conditional WASI expectations where the behavior is supported and ignore only tests that depend on unavailable runtime capabilities.

## Consolidated reference scope

- Synchronous ordered-file checking, merging, and bounded external sorting for ordinary WASIp1 and WASIp2 targets.
- Existing threaded implementations and Rayon for the exact `wasm32-wasip1-threads` target.
- Shared module boundaries that keep synchronous and threaded algorithms next to one another.
- Platform-independent output-error fixes and focused regressions.
- Selective WASI integration annotations rather than disabling the suite en masse.

Rust currently exposes the same relevant compile-time cfg values for threaded and nonthreaded WASI targets, so target detection uses the exact `wasm32-wasip1-threads` target triple instead of atomics cfgs.

## Verification

- [x] Reviewed the complete diff against #13910 and resolved all findings.
- [x] Passed formatting and native, WASIp1, WASIp2, and `wasm32-wasip1-threads` checks and Clippy.
- [x] Passed 30 native crate tests and 199 native integration tests.
- [x] Passed 29 WASIp1 unit tests.
- [x] Passed 176 Wasmtime integration tests; the remaining 23 are explicitly ignored for unsupported runtime behavior.
- [x] Passed the repository pre-commit hooks.
- [x] Push the rebuilt draft after the final aggregate and downstream review.

## Repository constraints

- Never read or copy GNU coreutils source.
- Every behavior change must have a meaningful Rust regression test.
- Keep maintainer replies human-written.
- Run dependency code and tests in the development container.

## Decision record

- 2026-08-13: The original draft was divided conceptually into six independently reviewable behaviors.
- 2026-08-13: Synchronous algorithms remained in domain-specific `sync.rs` files rather than a WASI syscall adapter.
- 2026-09-01: The consolidated draft was rebuilt on the maintainer-updated #13910 history, reviewed, and reverified.
- 2026-09-01: Threaded selection was limited to the exact threaded target because compiler cfgs do not distinguish the two WASIp1 targets.
