<!-- spell-checker:ignore wasip wasip2 wasmtime uucore -->

# WASI PR tracking

## Aggregate branch

The private `stacked-prs` branch is the downstream integration reference for `swift-wasm-runtime`. It is not intended to be submitted as one PR.

Current base: `upstream/main` at `48a20138f` on September 1, 2026.

Current layers:

1. Open PR #13910 at `82e437edd`.
2. Rebuilt draft PR #13806 at `27896f108`.
3. Rebuilt draft PR #13913, represented in the stack by `4e4d05b95` from source tip `7b5e2b182`.
4. Residual WASI test-gap documentation, the local integration helper, and the WASIp2 `env` feature gate at `2eee883ce`.

Changes from merged PRs are inherited from current upstream and are not duplicated. The previous aggregate is retained at `backup/stacked-prs-pre-refresh-2026-09-01`.

## PR status

- [x] Core WASI platform cleanup merged: PR #12503
- [x] `cat` coverage merged: PR #13802
- [x] `touch` timestamps merged: PR #13803
- [x] `cp` symlink support merged: PR #13804
- [x] `tail` coverage merged: PR #13805
- [x] Final `sort --merge` flush fix open: PR #13910
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
