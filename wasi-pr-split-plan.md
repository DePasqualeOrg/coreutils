<!-- spell-checker:ignore wasip wasip2 wasmtime uucore -->

# WASI PR tracking

## Aggregate branch

The private `stacked-prs` branch is the downstream integration reference for `swift-wasm-runtime`. It is not intended to be submitted as one PR.

Current base: `upstream/main` at `f24d6aa07` on September 27, 2026.

Current layers:

1. Draft PR #13806, squashed from its tip `5e2a8ab3a` without the #13910 commits and ported to the current base.
2. Draft PR #13913, squashed from its tip `f65f68f65` and ported to the current base.
3. Residual WASI test-gap documentation, the local integration helper, and the WASIp2 `env` feature gate.
4. This tracking documentation.

Changes from merged PRs are inherited from current upstream and are not duplicated. The previous aggregates are retained at `backup/stacked-prs-pre-refresh-2026-09-27` and `backup/stacked-prs-pre-refresh-2026-09-01`.

The PR branches on GitHub were not rebased in the September 27 refresh; they remain on `48a20138f`. The ported layers in this branch are the reference for refreshing them.

Porting notes from the September 27 refresh:

- `sort`: upstream merged #13910 as `41d664136` with the follow-up `379829d07`, which passes the write-error context into `write_all_to`. Both runners now take that context; the synchronous runner applies it only to write errors. Upstream's private output-copy fix (`f27705c51`), `-o` truncation errors (`4acc97ffc`), empty-input output (`22d61c385`) and lazy input opening with `check_inputs` (`34485925d`) are carried into the split modules. Replacing an output file that is also an input stays in `uumain`, before `check_inputs`.
- `cp`: upstream `13deb9633` added a partial WASI fix (ENOSYS as an optional-metadata error, `Metadata`-based source times, `--reflink=auto` accepted). The layer's WASI metadata path supersedes the first two, so upstream's `source_times` helper is removed, and the `--reflink` behavior is adopted in `platform/wasi.rs`. The WASI workflow now runs all of `test_cp::`, so the layer's explicit test list was dropped.

## PR status

- [x] Core WASI platform cleanup merged: PR #12503
- [x] `cat` coverage merged: PR #13802
- [x] `touch` timestamps merged: PR #13803
- [x] `cp` symlink support merged: PR #13804
- [x] `tail` coverage merged: PR #13805
- [x] Final `sort --merge` flush fix merged: PR #13910
- [x] Complete synchronous `sort` reference open as draft: PR #13806
- [x] Complete `cp` metadata reference open as draft: PR #13913
- [x] Aggregate PR #11712 retired after the replacement branches were pushed

Detailed plans:

- `SORT_WASI_SIX_PR_PLAN.md`
- `CP_WASI_THREE_PR_PLAN.md`

## Refresh checklist

- [x] Fetch current upstream and fork refs.
- [x] Preserve the previous branch tips under dated backup refs.
- [x] Rebuild each open PR from its correct current base.
- [x] Remove changes already merged upstream.
- [x] Retain only useful residual changes from PR #11712.
- [x] Review and verify the reconstructed `sort` and `cp` branches.
- [x] Run repository hooks for every reconstructed layer.
- [x] Complete a final aggregate diff review and focused downstream build and test pass.
- [x] Force-push the revised PR branches and refreshed aggregate with exact leases.
- [x] Close superseded aggregate PR #11712 without deleting its branch.
