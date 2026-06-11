<!-- spell-checker:ignore wasmtime rustix uucore wasip wasm utimensat fsext mdbook cfg cspell symlink symlinks no-thread -->

# WASI PR split plan

## Status (June 2026)

All branches were rebased onto current `upstream/main` on 2026-06-11. During the rebase window, two sibling efforts landed upstream in full and their branches were deleted: `wasi-ln-symlink` (ln WASI support, merged as PR #11713 with later refinements absorbed) and `wasi-integration-tests` (the wasmtime integration-test infrastructure). `wasi-support` carries 16 commits (hashes below refreshed to the rebased values); `wasi-core-platform-abstractions` carries 4. `stacked-prs` is rebuilt as `upstream/main` + merge of `wasi-support` + one `feature(wasip2)` crate-attr commit for the wasm32-wasip2 builds (see swift-wasm-runtime/docs/wasip2-migration-plan.md — the wasip2 attr is deliberately kept out of `wasi-support` so the split PRs stay wasip2-free).

This document tracks how to split the current `wasi-support` integration branch into smaller reviewable PRs while keeping `wasi-support` as the stacked reference branch.

## Strategy

- Keep `wasi-support` as the canonical end-to-end integration branch until the split is complete.
- Peel off one logical PR at a time from latest `upstream/main`.
- Wait for each split PR to merge before preparing the next one.
- After each merge, fetch `upstream/main`, rebase `wasi-support` onto it, and drop the merged changes from the remaining stack.
- Keep each split PR independently buildable and reviewable; do not expand CI coverage in a PR until the runtime support it depends on has merged.
- Treat PR #11712 as the reference for the full intended behavior, not as the final review vehicle once the smaller PRs exist.

## Tracking

| Order | Status | Topic | Candidate branch | Depends on | Notes |
| --- | --- | --- | --- | --- | --- |
| 1 | In progress | WASI core platform abstractions | `wasi-core-platform-abstractions` | latest `upstream/main` | Small cross-cutting compile/platform support with no CI expansion. |
| 2 | Planned | `sort` no-thread fallback | `wasi-sort-no-threads` | PR 1 merged | Per-utility PR using the final split-module organization, not the earlier verbose-cfg shape. |
| 3 | Planned | `cat` unsafe-overwrite fallback | `wasi-cat-unsafe-overwrite` | PR 1 merged | Tiny per-utility PR for the WASI `is_unsafe_overwrite` stub and comment. |
| 4 | Planned | `touch` WASI timestamps | `wasi-touch-timestamps` | PR 1 merged | `rustix::fs::utimensat` support and `touch -` unsupported-platform behavior. |
| 5 | Planned | `cp` WASI symlinks and timestamps | `wasi-cp-symlinks-timestamps` | PRs 1 and 4 merged, or PR 1 if duplicated narrowly | Keep separate from `touch`; reviewers already asked for symlink-related changes to be split. |
| 6 | Planned | WASI integration test enablement | `wasi-enable-more-tests` | PRs 1-5 merged | CI list expansion, test annotations, cspell fixes, and WASI gap documentation. |
| 7 | Planned | Local WASI Docker verification helper | `wasi-local-test-helper` | none, but easiest after PR 6 | Optional local tooling; keep separate from runtime behavior. |

## Split 1: WASI core platform abstractions

Goal: add the smallest shared platform support needed for later utility-specific WASI work without enabling more CI tests yet.

Candidate scope:

- `src/uucore/src/lib/features/fs.rs`: make `FileInformation` use the `rustix` stat path for WASI.
- `src/uucore/src/lib/features/fsext.rs`: make WASI use the same `UResult<Vec<MountInfo>>` shape as other platforms and return an empty mount list.
- `src/uucore/src/lib/mods/io.rs`: keep the cleaned up `into_stdio` cfg split if it remains relevant after slicing.
- `src/uu/env/src/native_int_str.rs`: use `std::os::wasi::ffi` for WASI native string conversions.
- `src/uu/tail/src/platform/mod.rs`: add WASI `ProcessChecker` stubs if `tail` compilation needs them before test enablement.

Candidate source commits from `wasi-support`:

- `5b4537379 Add wasm32-wasi platform support`, selected hunks only.
- `8eb81ed9f Replace unstable wasi_ext with stable libc calls`, selected `uucore` hunks only if they belong in the foundation.
- `f10b0dc2a uucore: use raw pointers for libc stat calls to satisfy clippy`, only if still applicable after selecting the final stat implementation.
- `0feb4bd1d Fix clippy warnings for wasm32-wasip1 target`, selected foundation hunks only.

Excluded from this split:

- `sort` algorithm or module changes.
- `cp` timestamp preservation behavior.
- `touch` timestamp behavior.
- `.github/workflows/wasi.yml` CI expansion.
- Broad test ignore annotations.

Suggested verification:

- `cargo check -p uucore --target wasm32-wasip1`
- `cargo check -p uu_env --target wasm32-wasip1`
- `cargo check -p uu_tail --target wasm32-wasip1`
- Any host checks required by touched crates.

## Split 2: `sort` no-thread fallback

Goal: make `uu_sort` work on WASI Preview 1 without atomics/threads while preserving the threaded `rayon` path on normal targets and `wasm32-wasip1-threads`.

Candidate scope:

- Move `rayon` behind `cfg(not(all(target_os = "wasi", not(target_feature = "atomics"))))`.
- Add `src/uu/sort/build.rs` and `wasi_no_threads`.
- Add `src/uu/sort/src/parallel.rs` for threaded versus sequential sorting.
- Add sync implementations for `check`, `ext_sort`, and `merge`, and split threaded implementations into sibling modules.
- Add `uucore::fs::wasi_default_tmp_dir` here if it is still needed by the `sort` temp-file fallback, since that helper is motivated by `sort` behavior.
- Add the WASI ignore for locale-dependent `sort` tests only if the PR includes enough test execution to expose it; otherwise defer it to Split 6.

Candidate source commits from `wasi-support`:

- `f91d20998 sort: add synchronous fallback for WASI without thread support`.
- `bad9a3515 sort: add wasi_no_threads cfg alias to reduce predicate verbosity`.
- `c1a1fde59 sort: isolate WASI sync code into sibling modules`.
- `defaacf3c Add integration tests for cat/sort/tail/touch + underlying fixes`, selected `sort` temp-dir hunks only if needed.
- `de7fbc179 tests/sort: ignore test_consistent_sorting_with_i18n_collate on WASI`, probably deferred to Split 6 unless needed here.

Suggested verification:

- `cargo test -p uu_sort`
- `cargo check -p uu_sort --target wasm32-wasip1`
- `cargo check -p uu_sort --target wasm32-wasip1-threads`

## Split 3: `cat` unsafe-overwrite fallback

Goal: make the `cat` platform abstraction compile cleanly for WASI while documenting why unsafe-overwrite detection remains stubbed there.

Candidate scope:

- `src/uu/cat/src/platform/mod.rs`: add or update the WASI `is_unsafe_overwrite` stub and keep the comment precise about wasmtime host-descriptor stat behavior.

Candidate source commits from `wasi-support`:

- `defaacf3c Add integration tests for cat/sort/tail/touch + underlying fixes`, selected `cat` hunks only.
- `8f4588cfb Fix cspell errors in CI`, only if selected comments introduce the spelling issue.

Suggested verification:

- `cargo check -p uu_cat --target wasm32-wasip1`
- `cargo check -p uu_cat`

## Split 4: `touch` WASI timestamps

Goal: make the timestamp behavior used by `touch` work on WASI without relying on unimplemented `filetime` paths.

Candidate scope:

- `src/uu/touch/Cargo.toml`: ensure `rustix` is available for WASI.
- `src/uu/touch/src/error.rs`: add `UnsupportedPlatformFeature`.
- `src/uu/touch/src/touch.rs`: provide WASI replacements for `set_file_times`, `set_symlink_file_times`, and metadata-to-`FileTime` conversion, and return `UnsupportedPlatformFeature` for `touch -`.
- Add narrow positive tests if they can run without the broader CI expansion; otherwise defer broad annotations to Split 6.

Candidate source commits from `wasi-support`:

- `defaacf3c Add integration tests for cat/sort/tail/touch + underlying fixes`, selected `touch` hunks only.
- `0feb4bd1d Fix clippy warnings for wasm32-wasip1 target`, selected `touch` hunks only.
- `8f4588cfb Fix cspell errors in CI`, only if selected comments/docs introduce the spelling issue.

Suggested verification:

- `cargo check -p uu_touch --target wasm32-wasip1`
- `cargo check -p uu_touch`
- Focused host tests for touched `touch` behavior if practical.
- WASI integration tests for `touch` only if they are stable enough to include before Split 6.

## Split 5: `cp` WASI symlinks and timestamps

Goal: make `cp` symlink creation and timestamp preservation work on WASI while keeping the review focused on `cp`.

Candidate scope:

- `src/uu/cp/Cargo.toml`: add the WASI `rustix` dependency.
- `src/uu/cp/src/cp.rs`: use `rustix::fs::utimensat` for WASI timestamp preservation, use `rustix::fs::symlink` on WASI, and keep `set_timestamps` as a reviewable extraction.
- Add narrow positive tests if they can run without the broader CI expansion; otherwise defer broad annotations to Split 6.

Candidate source commits from `wasi-support`:

- `defaacf3c Add integration tests for cat/sort/tail/touch + underlying fixes`, selected `cp` hunks only.
- `54c902f3b cp: extract WASI timestamp logic into set_timestamps function`.
- `0feb4bd1d Fix clippy warnings for wasm32-wasip1 target`, selected `cp` hunks only.
- `8f4588cfb Fix cspell errors in CI`, only if selected comments/docs introduce the spelling issue.

Suggested verification:

- `cargo check -p uu_cp --target wasm32-wasip1`
- `cargo check -p uu_cp`
- Focused host tests for touched `cp` behavior if practical.
- WASI integration tests for `cp` only if they are stable enough to include before Split 6.

## Split 6: WASI integration test enablement

Goal: turn on additional WASI integration coverage after the runtime support has merged.

Candidate scope:

- `.github/workflows/wasi.yml`: add `test_cat::`, `test_sort::`, `test_tail::`, and `test_touch::` to the WASI test list as appropriate.
- `tests/by-util/test_cat.rs`: annotate WASI-incompatible tests with clear skip reasons.
- `tests/by-util/test_comm.rs`: annotate locale-dependent tests if they are caught by the expanded WASI run.
- `tests/by-util/test_sort.rs`: annotate locale, process, host-path, sysinfo, and resource-limit gaps.
- `tests/by-util/test_tail.rs`: annotate follow-mode, errno, host-path, and unsupported-platform gaps.
- `tests/by-util/test_touch.rs`: annotate pre-epoch, timezone, root, stdout, permission, non-UTF-8, and device-file gaps.
- `docs/src/wasi-test-gaps.md`: document the skip categories introduced by the test annotations.
- Fold any cspell fixes into this split rather than keeping a separate spelling-only PR.

Candidate source commits from `wasi-support`:

- `defaacf3c Add integration tests for cat/sort/tail/touch + underlying fixes`, selected workflow/test/docs hunks only.
- `39c3f99b9 Annotate more WASI-incompatible tests caught on Linux`.
- `cbf8b7019 Clarify WASI symlink and guest-path test gaps`.
- `8f4588cfb Fix cspell errors in CI`.
- `df0b53301 tail: ignore test_gnu_args_f under wasi_runner`.
- `5de232ce1 tail: ignore test_follow_inotify_only_regular under wasi_runner`.
- `de7fbc179 tests/sort: ignore test_consistent_sorting_with_i18n_collate on WASI`.

Suggested verification:

- The WASI workflow command from `.github/workflows/wasi.yml`.
- Focused host integration tests for any files touched by annotations if practical.
- Documentation spell-check if the CI job normally runs it for docs/test text.

## Split 7: local WASI Docker verification helper

Goal: provide local Linux-oriented reproduction tooling without coupling it to runtime support or CI enablement.

Candidate scope:

- `util/run-wasi-tests-docker.sh`
- Any README note only if reviewers ask for one.

Candidate source commits from `wasi-support`:

- `fa28e45a1 util: add run-wasi-tests-docker.sh for local Linux verification`.

Suggested verification:

- Shellcheck or local script syntax check if available.
- Run the helper only when Docker is available and the required image/network access is acceptable.

## Operating checklist for each split

- [ ] Fetch latest `upstream/main`.
- [ ] Create a fresh split branch from `upstream/main`.
- [ ] Cherry-pick with `-n` or reconstruct selected hunks from `wasi-support`.
- [ ] Commit the split with a focused message and no unrelated history.
- [ ] Run the split-specific verification listed above.
- [ ] Push and open the PR only after explicit authorization in the current conversation.
- [ ] After merge, fetch `upstream/main`, rebase `wasi-support`, and confirm the remaining stack still builds.
- [ ] Update this plan with the PR URL, merge status, and any scope changes.

## Open decisions

- Before preparing each new split, rebase `wasi-support` onto current `upstream/main`; the draft reference branch can drift behind upstream while individual splits are reviewed.
- Decide whether Split 2 should be split further into a mechanical `sort` module refactor followed by the actual no-thread fallback if reviewers still find the per-utility `sort` PR too large.
- Update the draft stacked PR description if it remains visible as the reference branch; it still contains stale references to unstable `wasi_ext`, `symlink_path`, and older split assumptions.
