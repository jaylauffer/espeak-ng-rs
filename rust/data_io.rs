//! Bounded speech-data reads on the caller's loadngo proactor.
//!
//! One reusable buffer and one in-flight operation per reader. Completions run
//! on the host's proactor thread; they must consume/copy the chunk promptly.
//! No scheduler or timer thread is created here. The file is retained until
//! completion, including cancellation. Stop/drain the host before shutdown.
// SPDX-License-Identifier: GPL-3.0-or-later

use loadngo_proactor::{IoBuf, IoOpId, IoPort, IoResult, ProactorHandle, RawFdCompat};
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Open before submitting reads, from initialization or the host's I/O worker.
/// On Windows, the handle must support overlapped I/O for loadngo's IOCP port.
pub fn open_data_file(path: &Path) -> io::Result<Arc<File>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x4000_0000); // FILE_FLAG_OVERLAPPED
    }
    options.open(path).map(Arc::new)
}

/// An open data file registered with one proactor's port
/// ([`ProactorHandle::register`]), read with [`DataReader::read_registered`] on
/// that proactor. On IOCP a registered handle is not associated with the port
/// again on every read (about 0.8 us each on GitHub's Windows runner); the
/// other ports need no registration and this costs nothing there. Clones share
/// one registration, released when the last clone, including one held by a
/// read still in flight, is dropped.
#[derive(Clone)]
pub struct DataFile(Arc<Registration>);

struct Registration {
    /// Keeps the handle `fd` names open; never read.
    _file: Arc<File>,
    fd: RawFdCompat,
    release: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl DataFile {
    pub fn register<P: IoPort>(handle: &ProactorHandle<P>, file: Arc<File>) -> io::Result<Self> {
        let fd = handle.register(raw_file(&file))?;
        let owner = handle.clone();
        Ok(Self(Arc::new(Registration {
            _file: file,
            fd,
            release: Mutex::new(Some(Box::new(move || owner.release(fd)))),
        })))
    }
}

impl Drop for Registration {
    /// Runs before `_file` drops, so the registration ends before the handle
    /// closes.
    fn drop(&mut self) {
        let release = self
            .release
            .get_mut()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if let Some(release) = release {
            release();
        }
    }
}

fn raw_file(file: &File) -> RawFdCompat {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        file.as_raw_fd()
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        file.as_raw_handle() as RawFdCompat
    }
}

struct Pool {
    buffer: Mutex<Option<IoBuf>>,
    capacity: usize,
}

/// Clones share the same admission limit and buffer, rather than making pools.
#[derive(Clone)]
pub struct DataReader {
    pool: Arc<Pool>,
}

impl DataReader {
    /// Bound chunk size to 1 MiB. Allocate the buffer once, before any hot path.
    pub fn new(chunk_bytes: usize) -> io::Result<Self> {
        if !(1..=1024 * 1024).contains(&chunk_bytes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "speech-data chunk size must be 1..=1048576 bytes",
            ));
        }
        Ok(Self {
            pool: Arc::new(Pool {
                buffer: Mutex::new(Some(IoBuf::with_capacity(chunk_bytes))),
                capacity: chunk_bytes,
            }),
        })
    }

    pub fn is_busy(&self) -> bool {
        self.pool
            .buffer
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_none()
    }

    /// Submit a positioned read. Busy admission returns `WouldBlock`, without
    /// allocating or queuing. Returned operation ID can be passed to cancel_io;
    /// cancellation still needs the host to drain the eventual completion.
    ///
    /// The loan remains busy during the callback. Submit the next chunk after
    /// returning to the host loop. A short read or empty slice is not retried.
    pub fn read<P: IoPort>(
        &self,
        handle: &ProactorHandle<P>,
        file: Arc<File>,
        offset: u64,
        done: impl FnOnce(io::Result<&[u8]>) + Send + 'static,
    ) -> io::Result<IoOpId> {
        let fd = raw_file(&file);
        self.submit(handle, fd, file, offset, done)
    }

    /// [`read`](Self::read) for a file registered with `handle`'s proactor.
    /// The read holds `file` (and so its registration) until its completion.
    pub fn read_registered<P: IoPort>(
        &self,
        handle: &ProactorHandle<P>,
        file: DataFile,
        offset: u64,
        done: impl FnOnce(io::Result<&[u8]>) + Send + 'static,
    ) -> io::Result<IoOpId> {
        let fd = file.0.fd;
        self.submit(handle, fd, file, offset, done)
    }

    /// `keep` (the file, or its registration) is held until the completion.
    fn submit<P: IoPort>(
        &self,
        handle: &ProactorHandle<P>,
        fd: RawFdCompat,
        keep: impl Send + 'static,
        offset: u64,
        done: impl FnOnce(io::Result<&[u8]>) + Send + 'static,
    ) -> io::Result<IoOpId> {
        let buffer = self
            .pool
            .buffer
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "speech-data read already in flight",
                )
            })?;
        let pool = Arc::clone(&self.pool);
        let result = handle.read(fd, buffer, offset, move |result: IoResult| {
            let _keep = keep; // retain the descriptor until the callback finishes
            match result {
                Ok(transfer) => {
                    let returned = ReturnBuffer {
                        pool,
                        bytes: Some(transfer.buf.into_vec()),
                    };
                    done(Ok(returned.bytes.as_deref().expect("returned buffer")));
                    // Drop returns the loan, including if the callback unwinds.
                }
                Err(error) => {
                    let _returned = ReturnBuffer { pool, bytes: None };
                    done(Err(error));
                }
            }
        });
        if result.is_err() {
            // The port consumes/drops buffers on failed submission. Replenish
            // only on this exceptional path, never on successful chunk reads.
            *self.pool.buffer.lock().unwrap_or_else(|p| p.into_inner()) =
                Some(IoBuf::with_capacity(self.pool.capacity));
        }
        result
    }
}

struct ReturnBuffer {
    pool: Arc<Pool>,
    bytes: Option<Vec<u8>>,
}
impl Drop for ReturnBuffer {
    fn drop(&mut self) {
        let mut bytes = self
            .bytes
            .take()
            .unwrap_or_else(|| Vec::with_capacity(self.pool.capacity));
        bytes.resize(self.pool.capacity, 0);
        *self.pool.buffer.lock().unwrap_or_else(|p| p.into_inner()) = Some(IoBuf::from_vec(bytes));
    }
}
