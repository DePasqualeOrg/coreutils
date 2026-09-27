<!-- spell-checker:ignore relatime wasip wasmtime -->

# `cp` update and timestamp PR plan

## Context

Draft PR [#13913](https://github.com/uutils/coreutils/pull/13913) combined WASI timestamp support with several cross-platform `cp` fixes. On September 27, 2026 its tests were run against `upstream/main` at `f24d6aa07` in the Linux container, natively and under wasmtime, and every scenario was compared with GNU coreutils 9.7 (Debian trixie) and 9.1. Most of its fixes are still needed, and two of them turned out to be cross-platform bugs rather than WASI gaps. This plan replaces #13913 with small PRs, one per defect, and supersedes `CP_WASI_THREE_PR_PLAN.md`. None of the defects has an upstream issue.

Maintainers prefer small PRs, so each PR fixes one defect and carries its own tests. Draft PRs open with the preamble "Please ignore this PR until it is marked as ready for review.", a horizontal rule, and a short description; they are not monitored until the owner marks them ready.

## PRs

| PR | Title | Depends on | Status |
|---|---|---|---|
| A0 | [#14891](https://github.com/uutils/coreutils/pull/14891) `cp: keep copying a directory after skipping a file` | – | Draft |
| A | [#14893](https://github.com/uutils/coreutils/pull/14893) `cp: decide an --update=older skip before touching the destination` | A0 | Draft |
| B | [#14892](https://github.com/uutils/coreutils/pull/14892) `cp: preserve the source's access time` | – | Draft |
| C | `cp: preserve directory access times` | B merged | Planned |
| D | `cp: preserve symlink timestamps on WASI` | B merged | Planned |

#13913 was closed on September 27, 2026, pointing to A0, A and B.

### A0: recursive copy stops at the first skipped file

`copy_directory` propagates every `copy_direntry` error with `?`, and a skip is reported as `CpError::Skipped`. `cp -R -n`, `cp -R --update=none` and a declined `cp -R -i` therefore stop at the first existing file and leave the rest of the tree uncopied; `-n` and `--update=none` still exit 0. GNU copies the remaining files and exits 1 only after a declined prompt. The fix treats a skip in `copy_direntry` as "not copied" and continues, keeping exit status 1 for a declined prompt. PR A needs this, because it reports `--update=older` skips the same way.

### A: `--update=older` touches the destination before deciding to skip

`handle_existing_dest` backs up or removes an existing destination and only then does `handle_copy_mode` compare modification times and ask the overwrite policy. After a skip, `copy_file` still applies permissions and attributes and records the file as copied. GNU leaves a skipped destination untouched in every case below.

- `--update=older --backup` creates a backup of a destination it then skips.
- `--update=older --remove-destination` deletes a newer destination and copies the older source over it.
- `-u -p` and `--attributes-only -u -p` give a skipped destination the source's mode and times.
- `--update=older -i --remove-destination` deletes the destination without prompting.
- `--no-clobber --update=older --preserve=links` hard-links a later link of the source to the protected destination.
- `-l -u` and `-s -u` fail with "File exists" instead of skipping a newer destination.
- `--update=older --remove-destination` removes a destination that is a hard link or symlink to the source, where GNU leaves it alone.

The fix makes the whole skip decision (modification times, then the overwrite policy) in `handle_existing_dest` before any backup or removal, and returns early from `copy_file` on a skip. A source that is a hard link to an already handled file is still linked, and a file skipped only because it is not newer is still remembered for later links, as GNU does. A file protected by `--no-clobber` is not remembered. A source whose recorded destination is the current destination (two hard-linked operands copied to the same name) goes through the age check instead of being linked. With `--no-clobber`, a later hard link is not linked over an existing destination either. A missing source still reports "cannot stat".

### B: `cp -p` loses the source's access time

`copy_attributes` reads the source's times after the copy has read the file, so on a `relatime` mount the preserved access time is the time of the copy. GNU keeps the original. This affects Linux and WASI. The fix passes the metadata `copy_file` read before copying to the timestamp step. Five existing tests, plus the SELinux-only `test_cp_preserve_selinux`, compared the destination with source metadata read after the copy and now read it before.

### C: directory access times (after B)

Recursive copies read each directory before its attributes are copied, and `--progress` reads the whole tree before copying anything, so directory access times need to be captured before either read. This needs capture in `copydir.rs` and before the `--progress` scan. #13913's `DirectoryTimesTracker` is a WASI-only reference for the capture points. `test_cp_parents_with_permissions_copy_dir` still reads `dir/p1` and `dir/p1/p2` after the copy; C must move those reads before it.

Symlink access times have the same problem and belong with C or in a PR of their own: path lookups that follow a symlink (`is_dir`, `metadata`, `canonicalize`) read it and update its access time long before `copy_file` takes its snapshot, so `cp -P -p` gives the new link the time of the copy. GNU 9.7 keeps the original. `test_copy_through_dangling_symlink_no_dereference_permissions` reads the source after the copy and hides this; the fix must read it before. Decide the split when implementing C.

### D: symlink timestamps on WASI (after B)

On WASI, `cp -P --preserve=timestamps` of a symlink, `cp -s --preserve=timestamps`, and preserving timestamps through a destination symlink fail with "Wasm not implemented", because `filetime` has no WASI backend. #13913's `set_timestamps` in `platform/wasi.rs` (rustix `utimensat` with or without `AT_SYMLINK_NOFOLLOW`) is the reference.

## Found during review, not yet planned

These are pre-existing and outside A and B. Each needs its own GNU check and PR.

- `--update=none-fail` goes through the backup and removal in `handle_existing_dest` before failing, so `--remove-destination` removes the destination and copies, and `--backup` makes a backup before failing. This is the same "touches the destination before deciding" defect as A, for another update mode.
- `-u --attributes-only` with a newer source copies the file's data, because `CopyMode::Update` takes precedence over `AttrOnly`. GNU 9.7 changes only the attributes.
- `cp -a a/. b/. dest/` where `a/f` and `b/f` are hard links removes the just-copied `dest/f` and then fails to link it to itself with "No such file or directory". GNU 9.7 exits 0 with `dest/f` in place. Checking whether `copied_files` maps the source to `dest` itself before the removal avoids the data loss.
- `cp -i --remove-destination f l`, where `l` is a symlink to `f`, removes `l` before prompting, so answering "n" still loses it. GNU 9.7 prompts first and keeps the link, with or without `-u`.
- A recursive `-a -n` does not record a skipped file in `copied_files`, so a later hard link to it is copied as an independent file. Not checked against GNU.

## Dropped from #13913

- The `--interactive --update=older --preserve=links` prompt: GNU links the second name without prompting, so the draft's test was wrong.
- The WASI-only source time snapshots for `--progress`: B and C fix the cause on every platform.
- The guard against truncating through a symlink destination when SELinux context preservation fails: it may be a real bug, but it has no test and cannot be verified without an SELinux host.

## Checklist

- [x] A0 implemented, reviewed, tested natively and under wasmtime, and opened as a draft.
- [x] A implemented on A0, reviewed, tested, and opened as a draft.
- [x] B implemented, reviewed, tested, and opened as a draft.
- [x] #13913 closed with links to A0, A and B.
- [ ] C and D implemented after B merges.

## Decision record

- 2026-09-27: #13913's tests were run against current upstream and compared with GNU. The draft was split by defect instead of by mechanism, which avoids the artificial abstractions that stopped the August split. A0 was added after finding that recursive copies stop at the first skipped file.
- 2026-09-27: A0, A and B went through three, five and three review rounds and were opened as drafts. B covers regular files only; symlink access times need capture before cp's first path lookup and moved to the C stage.
