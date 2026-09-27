// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Multi-threaded ordered-file check: a reader thread streams chunks while
//! the main thread compares the boundary between consecutive chunks.

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::io::Read;
use std::iter;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

use uucore::error::UResult;

use crate::{
    GlobalSettings,
    chunks::{self, Chunk, RecycledChunk},
};

pub(super) fn check(
    path: &OsStr,
    settings: &GlobalSettings,
    max_allowed_cmp: Ordering,
    file: Box<dyn Read + Send>,
    chunk_size: usize,
) -> UResult<()> {
    let (recycled_sender, recycled_receiver) = sync_channel(2);
    let (loaded_sender, loaded_receiver) = sync_channel(2);
    let reader_join_handle = thread::spawn({
        let settings = settings.clone();
        move || reader(file, &recycled_receiver, &loaded_sender, &settings)
    });
    for _ in 0..2 {
        let _ = recycled_sender.send(RecycledChunk::new(chunk_size));
    }

    let mut prev_chunk: Option<Chunk> = None;
    let mut line_idx = 0;
    let mut result: UResult<()> = Ok(());
    // Note that we iterate over a reference, so that `loaded_receiver` is still alive
    // once we stop: `chunks::read` unwraps its `send`, so dropping our end while the
    // reader thread is still going would panic it. Since we stop at the *first*
    // disorder, the reader is usually still working at that point, so we shut it down
    // in an orderly fashion below instead of just dropping our end.
    'outer: for chunk in &loaded_receiver {
        if let Err(e) = super::check_chunk(
            path,
            settings,
            max_allowed_cmp,
            prev_chunk.as_ref(),
            &chunk,
            &mut line_idx,
        ) {
            result = Err(e);
            break 'outer;
        }
        if let Some(prev_chunk) = prev_chunk.take() {
            let _ = recycled_sender.send(prev_chunk.recycle());
        }

        prev_chunk = Some(chunk);
    }

    // Stop handing out buffers, so the reader runs out of work, then drain anything it
    // has already produced. This lets its in-flight `send` complete instead of failing,
    // and terminates because the reader can only own the (at most two) recycled chunks
    // that are still outstanding.
    drop(recycled_sender);
    while loaded_receiver.recv().is_ok() {}

    // A read error ends the input early, so without this the part read so far
    // would be checked and reported as sorted.
    let reader_result = reader_join_handle.join().unwrap();
    result.and(reader_result)
}

/// The function running on the reader thread.
fn reader(
    mut file: Box<dyn Read + Send>,
    receiver: &Receiver<RecycledChunk>,
    sender: &SyncSender<Chunk>,
    settings: &GlobalSettings,
) -> UResult<()> {
    let mut carry_over = vec![];
    for recycled_chunk in receiver {
        let should_continue = chunks::read(
            sender,
            recycled_chunk,
            None,
            &mut carry_over,
            &mut file,
            &mut iter::empty(),
            settings.line_ending.into(),
            settings,
        )?;
        if !should_continue {
            break;
        }
    }
    Ok(())
}
