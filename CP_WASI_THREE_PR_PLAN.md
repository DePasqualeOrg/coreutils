<!-- spell-checker:ignore nonrecursive wasip wasip2 wasmtime -->

# WASI `cp` PR plan

## Context

PR [#13804](https://github.com/uutils/coreutils/pull/13804) reduced the original WASI `cp` work to symlink creation and has merged. Attempts to divide the remaining timestamp work produced artificial abstractions that obscured the metadata flow, so draft PR [#13913](https://github.com/uutils/coreutils/pull/13913) remains a coherent combined reference implementation for a future contributor to split or adapt.

The draft was reconstructed directly on `upstream/main` at `48a20138f` on September 1, 2026. Its local tip is `7b5e2b182`.

## Submitted work

### WASI symlink creation

Title: `cp: support symlinks on WASI`

Status: Merged as [#13804](https://github.com/uutils/coreutils/pull/13804).

### Consolidated timestamp reference

Current title: `cp: fix metadata handling on WASI`

Scope:

- Preserve regular-file, copied-symlink, dereferenced-symlink, and `--symbolic-link` timestamps on WASI.
- Preserve root and nested directory timestamps during recursive copies.
- Capture source times before `--progress` scans and reject stale snapshots.
- Preserve destination follow and no-follow behavior.
- Leave an existing destination unchanged when `--update` skips a copy.
- Handle unsupported optional WASI metadata operations consistently.
- Refuse to follow a source replaced by a symlink when dereferencing is disabled.
- Keep `cp` policy in `cp.rs` and WASI stat and timestamp operations in `platform/wasi.rs`.

Status: Draft [#13913](https://github.com/uutils/coreutils/pull/13913), rebuilt and pushed at `7b5e2b182`.

## Verification

- [x] Reviewed the complete diff against current `upstream/main` and resolved all findings.
- [x] Passed formatting and native, WASIp1, and WASIp2 checks and Clippy.
- [x] Passed all five WASIp1 unit tests, including no-follow and pre-epoch conversion coverage.
- [x] Passed all 13 focused Wasmtime integration tests.
- [x] Passed 373 of 376 native integration tests; the same three sparse-allocation assertions fail on unmodified upstream in the container.
- [x] Passed the repository pre-commit hooks.
- [x] Push the rebuilt draft after the final aggregate and downstream review.

## Possible future extraction

The reference implementation may help a future contributor separate regular-file timestamps, symlink timestamps and destination follow policy, and timestamp capture before `--progress` scans. These are reference boundaries, not an active submission plan.

## Decision record

- 2026-08-11: The original change was divided into symlink creation, nonrecursive timestamps, and recursive timestamps.
- 2026-08-13: Further splitting produced artificial abstractions, so the remaining work was retained as one polished reference draft.
- 2026-09-01: The draft was rebuilt directly on current upstream, reviewed, and reverified; pre-1970 fractional timestamp conversion and import and lint issues found during review were fixed.
