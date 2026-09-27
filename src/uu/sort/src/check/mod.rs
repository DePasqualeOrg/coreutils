// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Check if a file is ordered.
//!
//! On most platforms this uses a multi-threaded reader. On WASI without
//! threads, a synchronous variant is used instead. The two implementations
//! live in sibling modules and are selected via cfg at the module boundary.

use std::cmp::Ordering;
use std::ffi::OsStr;

use itertools::Itertools;
use uucore::error::UResult;

use crate::{GlobalSettings, SortError, chunks::Chunk, compare_by, open};

#[cfg(not(wasi_no_threads))]
mod threaded;
#[cfg(not(wasi_no_threads))]
use threaded as runner;

#[cfg(wasi_no_threads)]
mod sync;
#[cfg(wasi_no_threads)]
use sync as runner;

/// Check if the file at `path` is ordered.
pub fn check(path: &OsStr, settings: &GlobalSettings) -> UResult<()> {
    let max_allowed_cmp = if settings.unique {
        // If `unique` is enabled, the previous line must compare _less_ to the next one.
        Ordering::Less
    } else {
        // Otherwise, the line previous line must compare _less or equal_ to the next one.
        Ordering::Equal
    };
    let file = open(path)?;
    let chunk_size = if settings.buffer_size < 100 * 1024 {
        // when the buffer size is smaller than 100KiB we choose it instead of the default.
        // this improves testability.
        settings.buffer_size
    } else {
        100 * 1024
    };

    runner::check(path, settings, max_allowed_cmp, file, chunk_size)
}

/// Checks `chunk` against the end of `prev_chunk` (if any) and within itself.
///
/// Returns [`SortError::Disorder`] for the first line that violates the requested
/// ordering. `line_idx` is the running line count and is advanced as lines are checked.
fn check_chunk(
    path: &OsStr,
    settings: &GlobalSettings,
    max_allowed_cmp: Ordering,
    prev_chunk: Option<&Chunk>,
    chunk: &Chunk,
    line_idx: &mut usize,
) -> UResult<()> {
    *line_idx += 1;
    if let Some(prev_chunk) = prev_chunk {
        // Check if the first element of the new chunk is greater than the last
        // element from the previous chunk
        let prev_last = prev_chunk.lines().last().unwrap();
        let new_first = chunk.lines().first().unwrap();

        if compare_by(
            prev_last,
            new_first,
            settings,
            prev_chunk.line_data(),
            chunk.line_data(),
        ) > max_allowed_cmp
        {
            return Err(SortError::Disorder {
                file: path.to_owned(),
                line_number: *line_idx,
                line: String::from_utf8_lossy(new_first.line).into_owned(),
                silent: settings.check_silent,
            }
            .into());
        }
    }

    for (a, b) in chunk.lines().iter().tuple_windows() {
        *line_idx += 1;
        if compare_by(a, b, settings, chunk.line_data(), chunk.line_data()) > max_allowed_cmp {
            return Err(SortError::Disorder {
                file: path.to_owned(),
                line_number: *line_idx,
                line: String::from_utf8_lossy(b.line).into_owned(),
                silent: settings.check_silent,
            }
            .into());
        }
    }

    Ok(())
}
