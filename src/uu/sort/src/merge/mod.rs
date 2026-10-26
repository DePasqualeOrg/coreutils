// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Merge already sorted files.
//!
//! On most platforms this uses a multi-threaded reader/merger setup. On WASI
//! without threads, a synchronous variant is used instead. The two
//! implementations live in sibling modules and are selected via cfg at the
//! module boundary.

use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    rc::Rc,
};

use uucore::error::{FromIo, UResult};

use crate::{
    GlobalSettings, Output, STDIN_FILE, SortError,
    chunks::Chunk,
    current_open_fd_count, fd_soft_limit, open,
    tmp_dir::{TmpDirWrapper, TmpPath},
};

#[cfg(not(wasi_no_threads))]
mod threaded;
#[cfg(not(wasi_no_threads))]
use threaded as runner;

#[cfg(wasi_no_threads)]
mod sync;
#[cfg(wasi_no_threads)]
use sync as runner;

/// If the output file occurs in the input files as well, copy the contents of the output file
/// and replace its occurrences in the inputs with that copy, which is returned.
fn replace_output_file_in_input_files(
    files: &mut [OsString],
    output: Option<&OsStr>,
    snapshot_stdin: bool,
    tmp_dir: &mut TmpDirWrapper,
) -> UResult<Vec<TmpPath>> {
    let mut copy: Option<TmpPath> = None;
    let mut stdin_copy: Option<TmpPath> = None;
    if let Some(Ok(output_path)) = output.map(|path| Path::new(path).canonicalize()) {
        for file in files {
            if file == STDIN_FILE {
                if snapshot_stdin && stdin_copy.is_none() {
                    let (mut copy_file, copy_path) = tmp_dir.next_file()?;
                    io::copy(&mut io::stdin().lock(), &mut copy_file).map_err(|error| {
                        SortError::ReadFailed {
                            path: PathBuf::from(STDIN_FILE),
                            error,
                        }
                    })?;
                    *file = copy_path.path().into();
                    stdin_copy = Some(copy_path);
                }
            } else if Path::new(file)
                .canonicalize()
                .is_ok_and(|file_path| file_path == output_path)
            {
                if let Some(copy) = &copy {
                    *file = copy.path().into();
                } else {
                    // Write through the descriptor `next_file` just opened rather
                    // than `fs::copy`, which would put the source's permission bits
                    // on the temporary file and undo the 0600 it was created with.
                    let (mut copy_file, copy_path) = tmp_dir.next_file()?;
                    let mut source = File::open(&output_path)
                        .map_err(|error| SortError::OpenTmpFileFailed { error })?;
                    io::copy(&mut source, &mut copy_file)
                        .map_err(|error| SortError::OpenTmpFileFailed { error })?;
                    *file = copy_path.path().into();
                    copy = Some(copy_path);
                }
            }
        }
    }
    Ok(copy.into_iter().chain(stdin_copy).collect())
}

/// Open the merge input `file`. `copy` is the copy of the output file returned by
/// [`replace_output_file_in_input_files`].
fn open_input(file: &OsStr, copies: &[TmpPath]) -> UResult<Box<dyn Read + Send>> {
    match copies.iter().find(|copy| Path::new(file) == copy.path()) {
        Some(copy) => Ok(Box::new(
            copy.open()
                .map_err(|error| SortError::OpenTmpFileFailed { error })?,
        )),
        None => open(file),
    }
}

/// Determine the effective merge batch size, enforcing a minimum and respecting the
/// file-descriptor soft limit after reserving stdio/output and a safety margin.
fn effective_merge_batch_size(settings: &GlobalSettings) -> usize {
    const MIN_BATCH_SIZE: usize = 2;
    const RESERVED_TMP_OUTPUT: usize = 1;
    const RESERVED_CTRL_C: usize = 2;
    const RESERVED_RANDOM_SOURCE: usize = 1;
    const SAFETY_MARGIN: usize = 1;
    let mut batch_size = settings.merge_batch_size.max(MIN_BATCH_SIZE);

    if let Some(limit) = fd_soft_limit() {
        let open_fds = current_open_fd_count().unwrap_or(3);
        let mut reserved = RESERVED_TMP_OUTPUT + RESERVED_CTRL_C + SAFETY_MARGIN;
        if settings.salt.is_some() {
            reserved = reserved.saturating_add(RESERVED_RANDOM_SOURCE);
        }
        let available_inputs = limit.saturating_sub(open_fds.saturating_add(reserved));
        if available_inputs >= MIN_BATCH_SIZE {
            batch_size = batch_size.min(available_inputs);
        } else {
            batch_size = MIN_BATCH_SIZE;
        }
    }

    batch_size
}

/// Merge pre-sorted `Box<dyn Read>`s.
///
/// If `settings.merge_batch_size` is greater than the length of `files`, intermediate files will be used.
/// If `settings.compress_prog` is `Some`, intermediate files will be compressed with it.
pub fn merge(
    files: &mut [OsString],
    settings: &GlobalSettings,
    output: Output,
    tmp_dir: &mut TmpDirWrapper,
) -> UResult<()> {
    let copy = replace_output_file_in_input_files(files, output.as_output_name(), true, tmp_dir)?;
    let files = files
        .iter()
        .map(|file| open_input(file, &copy).map(|inner| PlainMergeInput { inner }));
    if !runner::SUPPORTS_COMPRESSION && settings.compress_prog.is_some() {
        let _ = writeln!(
            io::stderr(),
            "sort: warning: --compress-program is ignored on this platform"
        );
        return merge_with_file_limit::<_, _, WriteablePlainTmpFile>(
            files, settings, output, tmp_dir,
        );
    }

    if settings.compress_prog.is_none() {
        merge_with_file_limit::<_, _, WriteablePlainTmpFile>(files, settings, output, tmp_dir)
    } else {
        merge_with_file_limit::<_, _, WriteableCompressedTmpFile>(files, settings, output, tmp_dir)
    }
}

/// Merge and write to output, dispatching to the active runner.
fn do_merge_to_output<M: MergeInput + 'static>(
    files: impl Iterator<Item = UResult<M>>,
    settings: &GlobalSettings,
    output: Output,
) -> UResult<()> {
    let merger = runner::merge_without_limit(files, settings)?;
    let ctx = output.write_failed_context();
    let mut out = output.into_write()?;
    merger.write_all_to(settings, &mut out, &ctx)?;
    flush_writer(&mut out).map_err_context(ctx)
}

// Merge already sorted `MergeInput`s.
pub fn merge_with_file_limit<
    M: MergeInput + 'static,
    F: ExactSizeIterator<Item = UResult<M>>,
    Tmp: WriteableTmpFile + 'static,
>(
    mut files: F,
    settings: &GlobalSettings,
    output: Output,
    tmp_dir: &mut TmpDirWrapper,
) -> UResult<()> {
    let batch_size = effective_merge_batch_size(settings);
    debug_assert!(batch_size >= 2);

    if files.len() <= batch_size {
        do_merge_to_output(files, settings, output)
    } else {
        let mut temporary_files = vec![];
        while files.len() > 0 {
            // Create the output before opening this batch's inputs, so that
            // creating it doesn't compete with them for file descriptors.
            let mut tmp_file =
                Tmp::create(tmp_dir.next_file()?, settings.compress_prog.as_deref())?;
            let merger = runner::merge_without_limit(files.by_ref().take(batch_size), settings)?;
            merger.write_all_to(settings, tmp_file.as_write(), || "write failed".into())?;
            temporary_files.push(tmp_file.finished_writing()?);
        }
        merge_with_file_limit::<_, _, Tmp>(
            temporary_files
                .into_iter()
                .map(Box::new(|c: Tmp::Closed| c.reopen())
                    as Box<
                        dyn FnMut(Tmp::Closed) -> UResult<<Tmp::Closed as ClosedTmpFile>::Reopened>,
                    >),
            settings,
            output,
            tmp_dir,
        )
    }
}

/// A struct to keep track of the previous line we encountered.
///
/// This is required for deduplication purposes.
pub(super) struct PreviousLine {
    pub chunk: Rc<Chunk>,
    pub line_idx: usize,
    // Only the threaded merger reads this back to recycle chunks.
    #[cfg_attr(wasi_no_threads, allow(dead_code))]
    pub file_number: usize,
}

/// Wait for the child to exit and check its exit code.
fn check_child_success(mut child: Child, program: &str) -> UResult<()> {
    if matches!(child.wait().map(|e| e.code()), Ok(Some(0) | None) | Err(_)) {
        Ok(())
    } else {
        Err(SortError::CompressProgTerminatedAbnormally {
            prog: program.to_owned(),
        }
        .into())
    }
}

/// A temporary file that can be written to.
pub trait WriteableTmpFile: Sized {
    type Closed: ClosedTmpFile;
    type InnerWrite: Write;
    fn create(file: (File, TmpPath), compress_prog: Option<&str>) -> UResult<Self>;
    /// Closes the temporary file.
    fn finished_writing(self) -> UResult<Self::Closed>;
    fn as_write(&mut self) -> &mut Self::InnerWrite;
}
/// A temporary file that is (temporarily) closed, but can be reopened.
pub trait ClosedTmpFile {
    type Reopened: MergeInput;
    /// Reopens the temporary file.
    fn reopen(self) -> UResult<Self::Reopened>;
}
/// A pre-sorted input for merging.
pub trait MergeInput: Send {
    type InnerRead: Read;
    /// Cleans this `MergeInput` up.
    /// Implementations may delete the backing file.
    fn finished_reading(self) -> UResult<()>;
    fn as_read(&mut self) -> &mut Self::InnerRead;
}

pub struct WriteablePlainTmpFile {
    path: TmpPath,
    file: BufWriter<File>,
}
pub struct ClosedPlainTmpFile {
    path: TmpPath,
}
pub struct PlainTmpMergeInput {
    path: TmpPath,
    file: File,
}

fn flush_writer(writer: &mut impl Write) -> io::Result<()> {
    writer.flush()
}

impl WriteableTmpFile for WriteablePlainTmpFile {
    type Closed = ClosedPlainTmpFile;
    type InnerWrite = BufWriter<File>;

    fn create((file, path): (File, TmpPath), _: Option<&str>) -> UResult<Self> {
        Ok(Self {
            file: BufWriter::new(file),
            path,
        })
    }

    fn finished_writing(mut self) -> UResult<Self::Closed> {
        flush_writer(&mut self.file)?;
        Ok(ClosedPlainTmpFile { path: self.path })
    }

    fn as_write(&mut self) -> &mut Self::InnerWrite {
        &mut self.file
    }
}
impl ClosedTmpFile for ClosedPlainTmpFile {
    type Reopened = PlainTmpMergeInput;
    fn reopen(self) -> UResult<Self::Reopened> {
        Ok(PlainTmpMergeInput {
            file: self
                .path
                .open()
                .map_err(|error| SortError::OpenTmpFileFailed { error })?,
            path: self.path,
        })
    }
}
impl MergeInput for PlainTmpMergeInput {
    type InnerRead = File;

    fn finished_reading(self) -> UResult<()> {
        // we ignore failures to delete the temporary file,
        // because there is a race at the end of the execution and the whole
        // temporary directory might already be gone.
        // Close the file first: removing it briefly takes another descriptor.
        drop(self.file);
        self.path.remove();
        Ok(())
    }

    fn as_read(&mut self) -> &mut Self::InnerRead {
        &mut self.file
    }
}

pub struct WriteableCompressedTmpFile {
    path: TmpPath,
    compress_prog: String,
    child: Child,
    child_stdin: BufWriter<ChildStdin>,
}
pub struct ClosedCompressedTmpFile {
    path: TmpPath,
    compress_prog: String,
}
pub struct CompressedTmpMergeInput {
    path: TmpPath,
    compress_prog: String,
    child: Child,
    child_stdout: ChildStdout,
}
impl WriteableTmpFile for WriteableCompressedTmpFile {
    type Closed = ClosedCompressedTmpFile;
    type InnerWrite = BufWriter<ChildStdin>;

    fn create((file, path): (File, TmpPath), compress_prog: Option<&str>) -> UResult<Self> {
        let compress_prog = compress_prog.unwrap();
        let mut command = Command::new(compress_prog);
        command.stdin(Stdio::piped()).stdout(file);
        let mut child = command
            .spawn()
            .map_err(|err| SortError::CompressProgExecutionFailed {
                prog: compress_prog.to_owned(),
                error: err,
            })?;
        let child_stdin = child.stdin.take().unwrap();
        Ok(Self {
            path,
            compress_prog: compress_prog.to_owned(),
            child,
            child_stdin: BufWriter::new(child_stdin),
        })
    }

    fn finished_writing(mut self) -> UResult<Self::Closed> {
        flush_writer(&mut self.child_stdin)?;
        drop(self.child_stdin);
        check_child_success(self.child, &self.compress_prog)?;
        Ok(ClosedCompressedTmpFile {
            path: self.path,
            compress_prog: self.compress_prog,
        })
    }

    fn as_write(&mut self) -> &mut Self::InnerWrite {
        &mut self.child_stdin
    }
}

impl ClosedTmpFile for ClosedCompressedTmpFile {
    type Reopened = CompressedTmpMergeInput;

    fn reopen(self) -> UResult<Self::Reopened> {
        let mut command = Command::new(&self.compress_prog);
        // mirroring what is done for ClosedPlainTmpFile
        let file = self
            .path
            .open()
            .map_err(|error| SortError::OpenTmpFileFailed { error })?;
        command.stdin(file).stdout(Stdio::piped()).arg("-d");
        let mut child = command
            .spawn()
            .map_err(|err| SortError::CompressProgExecutionFailed {
                prog: self.compress_prog.clone(),
                error: err,
            })?;
        let child_stdout = child.stdout.take().unwrap();
        Ok(CompressedTmpMergeInput {
            path: self.path,
            compress_prog: self.compress_prog,
            child,
            child_stdout,
        })
    }
}
impl MergeInput for CompressedTmpMergeInput {
    type InnerRead = ChildStdout;

    fn finished_reading(self) -> UResult<()> {
        // Explicitly close stdout before waiting on the child process.
        #[allow(clippy::drop_non_drop)]
        drop(self.child_stdout);
        check_child_success(self.child, &self.compress_prog)?;
        self.path.remove();
        Ok(())
    }

    fn as_read(&mut self) -> &mut Self::InnerRead {
        &mut self.child_stdout
    }
}

pub struct PlainMergeInput<R: Read + Send> {
    inner: R,
}
impl<R: Read + Send> MergeInput for PlainMergeInput<R> {
    type InnerRead = R;
    fn finished_reading(self) -> UResult<()> {
        Ok(())
    }
    fn as_read(&mut self) -> &mut Self::InnerRead {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, BufWriter, Write};

    use super::flush_writer;

    struct FailOnFlush;

    impl Write for FailOnFlush {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("flush failed"))
        }
    }

    #[test]
    fn flush_writer_propagates_flush_errors() {
        let mut writer = BufWriter::new(FailOnFlush);
        writer.write_all(b"buffered data").unwrap();

        let error = flush_writer(&mut writer).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::Other);
        let _ = writer.into_parts();
    }

    /// When the output file is also an input it is copied to a temporary file
    /// first. That copy must stay private to the owner: `fs::copy` would carry the
    /// source's 0644 over and undo what `next_file` created the file with.
    #[test]
    #[cfg(unix)]
    fn output_copy_keeps_the_tmp_file_private() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::Path;

        use super::replace_output_file_in_input_files;
        use crate::tmp_dir::TmpDirWrapper;

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        fs::write(&out, b"a\n").unwrap();
        fs::set_permissions(&out, fs::Permissions::from_mode(0o644)).unwrap();

        let mut tmp_dir = TmpDirWrapper::new(dir.path().to_owned());
        let mut files = vec![out.clone().into_os_string()];
        replace_output_file_in_input_files(&mut files, Some(out.as_os_str()), false, &mut tmp_dir)
            .unwrap();

        let copy = Path::new(&files[0]);
        assert_ne!(copy, out, "the input was not replaced by a copy");
        assert_eq!(
            fs::metadata(copy).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read(copy).unwrap(), b"a\n");
    }

    /// The copy of the output file is only read while it is the file sort
    /// created: once the temporary directory's path leads elsewhere, the file
    /// found at its name there is not read.
    #[test]
    #[cfg(all(unix, not(target_os = "redox")))]
    fn output_copy_is_not_read_through_a_replaced_tmp_dir() {
        use std::fs;

        use super::{open_input, replace_output_file_in_input_files};
        use crate::tmp_dir::TmpDirWrapper;

        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("0"), b"other\n").unwrap();
        let out = dir.path().join("out");
        fs::write(&out, b"a\n").unwrap();

        let mut tmp_dir = TmpDirWrapper::new(dir.path().to_owned());
        let mut files = vec![out.clone().into_os_string()];
        let copy = replace_output_file_in_input_files(
            &mut files,
            Some(out.as_os_str()),
            false,
            &mut tmp_dir,
        )
        .unwrap();
        let tmp = copy.first().unwrap().path().parent().unwrap();
        fs::rename(tmp, dir.path().join("moved")).unwrap();
        std::os::unix::fs::symlink(other.path(), tmp).unwrap();

        assert!(open_input(&files[0], &copy).is_err());
    }

    /// A temporary file that is being merged when the directory's path is made
    /// to lead elsewhere: removing it afterwards leaves the file of the same
    /// name there alone.
    #[test]
    #[cfg(all(unix, not(target_os = "redox")))]
    fn finished_input_is_not_removed_through_a_replaced_tmp_dir() {
        use std::fs;

        use super::{ClosedTmpFile, MergeInput, WriteablePlainTmpFile, WriteableTmpFile};
        use crate::tmp_dir::TmpDirWrapper;

        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("0"), b"other\n").unwrap();

        let mut tmp_dir = TmpDirWrapper::new(dir.path().to_owned());
        let input = WriteablePlainTmpFile::create(tmp_dir.next_file().unwrap(), None)
            .unwrap()
            .finished_writing()
            .unwrap()
            .reopen()
            .unwrap();
        let tmp = input.path.path().parent().unwrap().to_owned();
        fs::rename(&tmp, dir.path().join("moved")).unwrap();
        std::os::unix::fs::symlink(other.path(), &tmp).unwrap();
        input.finished_reading().unwrap();

        assert_eq!(fs::read(other.path().join("0")).unwrap(), b"other\n");
    }
}
