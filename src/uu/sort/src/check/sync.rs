// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Synchronous ordered-file check for targets without thread support.

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::io::Read;
use std::iter;

use uucore::error::UResult;

use crate::{
    GlobalSettings,
    chunks::{self, Chunk, RecycledChunk},
};

pub(super) fn check(
    path: &OsStr,
    settings: &GlobalSettings,
    max_allowed_cmp: Ordering,
    mut file: Box<dyn Read + Send>,
    chunk_size: usize,
) -> UResult<()> {
    let separator = settings.line_ending.into();
    let mut carry_over = vec![];
    let mut prev_chunk: Option<Chunk> = None;
    let mut spare_recycled: Option<RecycledChunk> = None;
    let mut line_idx = 0;

    loop {
        let recycled = spare_recycled
            .take()
            .unwrap_or_else(|| RecycledChunk::new(chunk_size));

        let (chunk, should_continue) = chunks::read_to_chunk(
            recycled,
            None,
            &mut carry_over,
            &mut file,
            &mut iter::empty(),
            separator,
            settings,
        )?;

        let Some(chunk) = chunk else {
            break;
        };

        super::check_chunk(
            path,
            settings,
            max_allowed_cmp,
            prev_chunk.as_ref(),
            &chunk,
            &mut line_idx,
        )?;
        if let Some(prev) = prev_chunk.take() {
            spare_recycled = Some(prev.recycle());
        }

        prev_chunk = Some(chunk);

        if !should_continue {
            break;
        }
    }
    Ok(())
}
