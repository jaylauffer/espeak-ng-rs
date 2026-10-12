//! The engine's file reads: phoneme data, dictionaries, voices, sound icons
//! and MBROLA tables; bounded compiler output uses the same proactor.
//!
//! With the `proactor` feature every read goes through one process-wide
//! loadngo proactor (io_uring on Linux, falling back to epoll where io_uring
//! is refused; kqueue on Apple and BSD; IOCP on Windows). Each file is read
//! in bounded chunks by [`DataReader`], and the calling thread drives the
//! proactor until its read completes. That is a blocking wait on the
//! completion, with no sleep or polling. Without the feature, or if no
//! proactor can be created, reads use `std::fs`; [`backend`] reports which.
//!
//! Directory listing and metadata stay synchronous: the proactor has no such
//! operations.
// SPDX-License-Identifier: GPL-3.0-or-later
use std::io;
use std::path::Path;
use std::{
    fs::File,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

/// Chunk size for proactor reads.
pub const CHUNK_BYTES: usize = 256 * 1024;

/// Which mechanism the engine's reads use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Backend {
    /// The platform proactor (io_uring, kqueue or IOCP).
    Platform,
    /// Linux epoll, where io_uring could not be set up.
    Epoll,
    /// Synchronous `std::fs`: no proactor feature or no proactor.
    Blocking,
}

/// Reads the whole file at `path`, refusing directories and files longer
/// than `limit` (`InvalidData`). The length is taken from the metadata
/// first, as the owners' bounds checks expect; a file that shrinks while it
/// is read returns what was there.
pub fn read_file(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    read_file_into(path, limit, &mut bytes)?;
    Ok(bytes)
}

/// Reuse the caller's retained storage. Metadata admission precedes clearing;
/// a failed read may leave partial bytes, which must not be parsed/published.
pub fn read_file_into(path: &Path, limit: usize, bytes: &mut Vec<u8>) -> io::Result<()> {
    let metadata = std::fs::metadata(path)?;
    if metadata.is_dir() {
        return Err(io::ErrorKind::IsADirectory.into());
    }
    let length = usize::try_from(metadata.len())
        .ok()
        .filter(|&length| length <= limit)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "engine file exceeds its bound")
        })?;
    bytes.clear();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
    imp::read(path, length, bytes)
}

/// The mechanism reads use (creating the proactor on first use).
pub fn backend() -> Backend {
    imp::backend()
}

/// Open/admit before changing an aligned resident buffer. Exact reads reuse the
/// engine's completion chunks and copy straight into initialized caller storage,
/// eliminating a second whole-file allocation. Drop unregisters before close.
pub struct FileReader {
    registered: imp::Registration,
    file: Arc<File>,
    length: usize,
}
impl FileReader {
    pub fn open(path: &Path, limit: usize) -> io::Result<Self> {
        let metadata = std::fs::metadata(path)?;
        if metadata.is_dir() {
            return Err(io::ErrorKind::IsADirectory.into());
        }
        let length = usize::try_from(metadata.len())
            .ok()
            .filter(|n| *n <= limit)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(all(windows, feature = "proactor"))]
        if backend() != Backend::Blocking {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x4000_0000);
        }
        let file = Arc::new(options.open(path)?);
        let registered = imp::register(file.clone())?;
        Ok(Self {
            registered,
            file,
            length,
        })
    }
    pub fn length(&self) -> usize {
        self.length
    }
    /// Storage is initialized and exactly the admitted length. Failed I/O may
    /// leave a prefix written; its owner must invalidate before publication.
    pub fn read_exact_into(&self, bytes: &mut [u8]) -> io::Result<()> {
        if bytes.len() != self.length {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        imp::read_exact_into(&self.registered, &self.file, bytes)
    }
}

/// One compiler output at a time, with retained bounded staging storage. File
/// writes have one flight and drive the existing engine port on the caller.
/// Open/metadata/close remain OS primitives; no worker or timer is introduced.
pub struct Writer {
    registered: imp::Registration,
    file: Arc<File>,
    bytes: MutexGuard<'static, Vec<u8>>,
    offset: u64,
    failed: bool,
}
impl Writer {
    pub fn create(path: &Path) -> io::Result<Self> {
        static BUFFER: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
        let bytes = BUFFER
            .get_or_init(|| Mutex::new(Vec::new()))
            .try_lock()
            .map_err(|_| io::Error::from(io::ErrorKind::WouldBlock))?;
        Self::create_with_buffer(path, bytes, true)
    }
    /// The phoneme compiler owns up to five simultaneous outputs. Buffers are
    /// retained globally, admission never waits, and the shared port still
    /// permits only one write flight. The default single-writer API is unchanged.
    pub(crate) fn create_compiler_output(path: &Path) -> io::Result<Self> {
        static BUFFERS: OnceLock<[Mutex<Vec<u8>>; 5]> = OnceLock::new();
        let buffers = BUFFERS.get_or_init(|| std::array::from_fn(|_| Mutex::new(Vec::new())));
        let bytes = buffers
            .iter()
            .find_map(|buffer| buffer.try_lock().ok())
            .ok_or_else(|| io::Error::from(io::ErrorKind::WouldBlock))?;
        Self::create_with_buffer(path, bytes, false)
    }
    fn create_with_buffer(
        path: &Path,
        mut bytes: MutexGuard<'static, Vec<u8>>,
        readable: bool,
    ) -> io::Result<Self> {
        bytes.clear();
        if bytes.capacity() < CHUNK_BYTES {
            bytes
                .try_reserve_exact(CHUNK_BYTES)
                .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        }
        imp::writer_ready()?;
        let mut options = std::fs::OpenOptions::new();
        options
            .read(readable)
            .write(true)
            .create(true)
            .truncate(true);
        #[cfg(all(windows, feature = "proactor"))]
        if backend() != Backend::Blocking {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x4000_0000);
        }
        let file = Arc::new(options.open(path)?);
        let registered = imp::register(file.clone())?;
        Ok(Self {
            registered,
            file,
            bytes,
            offset: 0,
            failed: false,
        })
    }
    pub fn position(&self) -> u64 {
        self.offset + self.bytes.len() as u64
    }
    #[cfg(test)]
    pub(crate) fn test_buffer(&self) -> (*const u8, usize) {
        (self.bytes.as_ptr(), self.bytes.capacity())
    }
    pub fn write(&mut self, mut data: &[u8]) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("compiler output already failed"));
        }
        self.position()
            .checked_add(data.len() as u64)
            .filter(|n| *n <= i32::MAX as u64)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        while !data.is_empty() {
            let n = data.len().min(CHUNK_BYTES - self.bytes.len());
            self.bytes.extend_from_slice(&data[..n]);
            data = &data[n..];
            if self.bytes.len() == CHUNK_BYTES {
                self.flush()?;
            }
        }
        Ok(())
    }
    pub fn flush(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("compiler output already failed"));
        }
        if !self.bytes.is_empty() {
            if let Err(error) = imp::write(&self.file, &self.registered, self.offset, &self.bytes) {
                self.failed = true;
                return Err(error);
            }
            self.offset += self.bytes.len() as u64;
            self.bytes.clear();
        }
        Ok(())
    }
    pub fn seek(&mut self, position: u64) -> io::Result<()> {
        if position > i32::MAX as u64 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        self.flush()?;
        self.offset = position;
        Ok(())
    }
    /// Complete before releasing registration/handle. Drop never starts I/O.
    pub fn finish(mut self) -> io::Result<()> {
        self.flush()
    }
}

#[cfg(not(feature = "proactor"))]
mod imp {
    use super::Backend;
    use std::io::{self, Read};
    use std::path::Path;
    use std::{
        fs::File,
        io::{Seek, SeekFrom, Write},
        sync::Arc,
    };
    pub type Registration = ();
    pub fn writer_ready() -> io::Result<()> {
        Ok(())
    }
    pub fn register(_: Arc<File>) -> io::Result<Registration> {
        Ok(())
    }
    pub fn write(file: &Arc<File>, _: &Registration, offset: u64, bytes: &[u8]) -> io::Result<()> {
        let mut file = &**file;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(bytes)
    }

    pub fn read(path: &Path, length: usize, bytes: &mut Vec<u8>) -> io::Result<()> {
        std::fs::File::open(path)?
            .take(length as u64)
            .read_to_end(bytes)?;
        Ok(())
    }

    pub fn read_exact_into(
        _registration: &Registration,
        file: &Arc<File>,
        bytes: &mut [u8],
    ) -> io::Result<()> {
        let mut file = &**file;
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(bytes)
    }

    pub fn backend() -> Backend {
        Backend::Blocking
    }
}

#[cfg(feature = "proactor")]
mod imp {
    use super::{Backend, CHUNK_BYTES};
    use crate::data_io::{open_data_file, DataFile, DataReader};
    use loadngo_proactor::{new_platform_proactor, IoBuf, IoPort, PlatformPort, Proactor};
    use std::fs::File;
    use std::io::{self, Read, Seek, SeekFrom, Write};
    use std::path::Path;
    use std::sync::{Arc, Mutex, OnceLock};

    enum Engine {
        Platform(Proactor<PlatformPort>, DataReader, Slot, WriteSlot),
        #[cfg(target_os = "linux")]
        Epoll(
            Proactor<loadngo_proactor::EpollPort>,
            DataReader,
            Slot,
            WriteSlot,
        ),
        Blocking,
    }

    fn engine() -> &'static Mutex<Engine> {
        static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();
        ENGINE.get_or_init(|| {
            let reader = DataReader::new(CHUNK_BYTES).ok();
            let engine = match (new_platform_proactor(), reader.clone()) {
                (Ok(proactor), Some(reader)) => {
                    Engine::Platform(proactor, reader, slot(), write_slot())
                }
                #[cfg(target_os = "linux")]
                (Err(_), Some(reader)) => match loadngo_proactor::EpollPort::new() {
                    Ok(port) => Engine::Epoll(Proactor::new(port), reader, slot(), write_slot()),
                    Err(_) => Engine::Blocking,
                },
                _ => Engine::Blocking,
            };
            Mutex::new(engine)
        })
    }

    pub fn backend() -> Backend {
        match &*engine().lock().unwrap_or_else(|p| p.into_inner()) {
            Engine::Platform(..) => Backend::Platform,
            #[cfg(target_os = "linux")]
            Engine::Epoll(..) => Backend::Epoll,
            Engine::Blocking => Backend::Blocking,
        }
    }

    pub fn read(path: &Path, length: usize, bytes: &mut Vec<u8>) -> io::Result<()> {
        let engine = engine().lock().unwrap_or_else(|p| p.into_inner());
        match &*engine {
            Engine::Platform(proactor, reader, slot, _) => {
                read_with(proactor, reader, slot, path, length, bytes)
            }
            #[cfg(target_os = "linux")]
            Engine::Epoll(proactor, reader, slot, _) => {
                read_with(proactor, reader, slot, path, length, bytes)
            }
            Engine::Blocking => {
                std::fs::File::open(path)?
                    .take(length as u64)
                    .read_to_end(bytes)?;
                Ok(())
            }
        }
    }

    pub fn read_exact_into(
        registration: &Registration,
        file: &Arc<File>,
        bytes: &mut [u8],
    ) -> io::Result<()> {
        let engine = engine().lock().unwrap_or_else(|p| p.into_inner());
        let mut at = 0;
        let length = bytes.len();
        let mut append = |chunk: &[u8]| {
            bytes[at..at + chunk.len()].copy_from_slice(chunk);
            at += chunk.len();
        };
        let used = match &*engine {
            Engine::Platform(proactor, reader, slot, _) => read_chunks(
                proactor,
                reader,
                slot,
                registration.as_ref().ok_or(io::ErrorKind::InvalidInput)?,
                length,
                &mut append,
            )?,
            #[cfg(target_os = "linux")]
            Engine::Epoll(proactor, reader, slot, _) => read_chunks(
                proactor,
                reader,
                slot,
                registration.as_ref().ok_or(io::ErrorKind::InvalidInput)?,
                length,
                &mut append,
            )?,
            Engine::Blocking => {
                let mut file = &**file;
                file.seek(SeekFrom::Start(0))?;
                return file.read_exact(bytes);
            }
        };
        if used != length {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(())
    }

    pub type Registration = Option<DataFile>;
    struct WriteCompletion {
        buffer: Option<Vec<u8>>,
        result: Option<io::Result<usize>>,
        busy: bool,
    }
    type WriteSlot = Arc<Mutex<WriteCompletion>>;
    fn write_slot() -> WriteSlot {
        Arc::new(Mutex::new(WriteCompletion {
            buffer: None,
            result: None,
            busy: false,
        }))
    }
    pub fn writer_ready() -> io::Result<()> {
        let engine = engine().lock().unwrap_or_else(|p| p.into_inner());
        let slot = match &*engine {
            Engine::Platform(_, _, _, slot) => Some(slot),
            #[cfg(target_os = "linux")]
            Engine::Epoll(_, _, _, slot) => Some(slot),
            Engine::Blocking => None,
        };
        if slot.is_some_and(|s| s.lock().unwrap_or_else(|p| p.into_inner()).busy) {
            Err(io::ErrorKind::WouldBlock.into())
        } else {
            Ok(())
        }
    }
    pub fn register(file: Arc<File>) -> io::Result<Registration> {
        let engine = engine().lock().unwrap_or_else(|p| p.into_inner());
        match &*engine {
            Engine::Platform(p, _, _, _) => DataFile::register(&p.handle(), file).map(Some),
            #[cfg(target_os = "linux")]
            Engine::Epoll(p, _, _, _) => DataFile::register(&p.handle(), file).map(Some),
            Engine::Blocking => Ok(None),
        }
    }
    pub fn write(
        file: &Arc<File>,
        registered: &Registration,
        offset: u64,
        bytes: &[u8],
    ) -> io::Result<()> {
        let engine = engine().lock().unwrap_or_else(|p| p.into_inner());
        match &*engine {
            Engine::Platform(p, _, _, slot) => write_with(
                p,
                slot,
                registered.as_ref().ok_or(io::ErrorKind::InvalidInput)?,
                offset,
                bytes,
            ),
            #[cfg(target_os = "linux")]
            Engine::Epoll(p, _, _, slot) => write_with(
                p,
                slot,
                registered.as_ref().ok_or(io::ErrorKind::InvalidInput)?,
                offset,
                bytes,
            ),
            Engine::Blocking => {
                let mut file = &**file;
                file.seek(SeekFrom::Start(offset))?;
                file.write_all(bytes)
            }
        }
    }
    fn write_with<P: IoPort>(
        proactor: &Proactor<P>,
        slot: &WriteSlot,
        file: &DataFile,
        offset: u64,
        bytes: &[u8],
    ) -> io::Result<()> {
        let handle = proactor.handle();
        let mut written = 0;
        while written < bytes.len() {
            let buffer = {
                let mut state = slot.lock().unwrap_or_else(|p| p.into_inner());
                if state.busy {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                let mut buffer = state.buffer.take().unwrap_or_default();
                buffer.clear();
                buffer
                    .try_reserve_exact(bytes.len() - written)
                    .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
                buffer.extend_from_slice(&bytes[written..]);
                state.result = None;
                state.busy = true;
                buffer
            };
            let done = slot.clone();
            let keep = file.clone();
            let operation = handle.write(
                file.fd(),
                IoBuf::from_vec(buffer),
                offset + written as u64,
                move |result: loadngo_proactor::IoResult| {
                    let _keep = keep;
                    let mut state = done.lock().unwrap_or_else(|p| p.into_inner());
                    state.busy = false;
                    state.result = Some(result.map(|value| {
                        state.buffer = Some(value.buf.into_vec());
                        value.bytes_transferred as usize
                    }));
                },
            );
            let operation = match operation {
                Ok(id) => id,
                Err(error) => {
                    slot.lock().unwrap_or_else(|p| p.into_inner()).busy = false;
                    return Err(error);
                }
            };
            let n = loop {
                if let Some(result) = slot.lock().unwrap_or_else(|p| p.into_inner()).result.take() {
                    break result?;
                }
                if let Err(error) = proactor.run_once() {
                    // The completion retains its registered file/buffer. Keep
                    // the slot busy until it arrives, refusing any reuse.
                    let _ = handle.cancel_io(operation);
                    return Err(error);
                }
            };
            if n == 0 || n > bytes.len() - written {
                return Err(io::ErrorKind::WriteZero.into());
            }
            written += n;
        }
        Ok(())
    }

    /// One chunk's outcome, handed from the completion to the waiting thread.
    struct Completion {
        bytes: Vec<u8>,
        result: Option<io::Result<usize>>,
    }
    type Slot = Arc<Mutex<Completion>>;
    fn slot() -> Slot {
        Arc::new(Mutex::new(Completion {
            bytes: vec![0; CHUNK_BYTES],
            result: None,
        }))
    }

    fn read_with<P: IoPort>(
        proactor: &Proactor<P>,
        reader: &DataReader,
        slot: &Slot,
        path: &Path,
        length: usize,
        bytes: &mut Vec<u8>,
    ) -> io::Result<()> {
        if length == 0 {
            return Ok(());
        }
        let handle = proactor.handle();
        // registered for this file's reads, released when the last one is done
        let file = DataFile::register(&handle, open_data_file(path)?)?;
        read_chunks(proactor, reader, slot, &file, length, |chunk| {
            bytes.extend_from_slice(chunk)
        })?;
        Ok(())
    }

    fn read_chunks<P: IoPort>(
        proactor: &Proactor<P>,
        reader: &DataReader,
        slot: &Slot,
        file: &DataFile,
        length: usize,
        mut append: impl FnMut(&[u8]),
    ) -> io::Result<usize> {
        let handle = proactor.handle();
        let mut read = 0;
        while read < length {
            slot.lock().unwrap_or_else(|p| p.into_inner()).result = None;
            let wanted = (length - read).min(CHUNK_BYTES);
            let done = Arc::clone(slot);
            let operation =
                reader.read_registered(&handle, file.clone(), read as u64, move |result| {
                    let mut completion = done.lock().unwrap_or_else(|p| p.into_inner());
                    completion.result = Some(result.map(|chunk| {
                        let used = chunk.len().min(wanted);
                        completion.bytes[..used].copy_from_slice(&chunk[..used]);
                        used
                    }));
                })?;
            // drive the proactor on this thread until the read completes
            let used = loop {
                if let Some(result) = slot.lock().unwrap_or_else(|p| p.into_inner()).result.take() {
                    break result?;
                }
                if let Err(error) = proactor.run_once() {
                    // Completion owns its registered file/chunk; no caller
                    // buffer loan survives refusal. Keep reader busy until drain.
                    let _ = handle.cancel_io(operation);
                    return Err(error);
                }
            };
            if used == 0 {
                break; // the file shrank
            }
            append(&slot.lock().unwrap_or_else(|p| p.into_inner()).bytes[..used]);
            read += used;
        }
        Ok(read)
    }
}

#[cfg(test)]
pub(crate) fn writer_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_reader_reuses_output_checks_admission_and_reports_shrinkage() {
        let root = std::env::temp_dir().join(format!("espeak-exact-read-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("data");
        let input: Vec<u8> = (0..CHUNK_BYTES * 3 + 13).map(|i| (i * 23) as u8).collect();
        std::fs::write(&path, &input).unwrap();
        let mut output = vec![0; input.len()];
        let pointer = output.as_ptr();
        for _ in 0..16 {
            let reader = FileReader::open(&path, input.len()).unwrap();
            assert_eq!(reader.length(), input.len());
            reader.read_exact_into(&mut output).unwrap();
            assert_eq!(output, input);
            assert_eq!(output.as_ptr(), pointer);
        }
        assert_eq!(
            FileReader::open(&root, input.len()).err().unwrap().kind(),
            io::ErrorKind::IsADirectory
        );
        assert_eq!(
            FileReader::open(&path, input.len() - 1)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::InvalidData
        );
        let reader = FileReader::open(&path, input.len()).unwrap();
        assert_eq!(
            reader.read_exact_into(&mut output[..5]).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(7)
            .unwrap();
        assert_eq!(
            reader.read_exact_into(&mut output).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
        drop(reader);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn output_chunks_patch_headers_reuse_storage_and_release_registrations() {
        let _serial = writer_test_lock();
        let directory =
            std::env::temp_dir().join(format!("espeak-engine-write-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("data");
        let input: Vec<u8> = (0..CHUNK_BYTES * 3 + 41).map(|i| (i * 19) as u8).collect();
        let mut expected = input.clone();
        expected[4..8].copy_from_slice(b"HEAD");
        let mut actual = Vec::new();
        let mut address = None;
        for _ in 0..64 {
            let mut writer = Writer::create(&path).unwrap();
            if let Some(old) = address {
                assert_eq!(writer.bytes.as_ptr(), old);
            } else {
                address = Some(writer.bytes.as_ptr());
            }
            assert_eq!(
                Writer::create(&path).err().unwrap().kind(),
                io::ErrorKind::WouldBlock
            );
            writer.write(&input).unwrap();
            assert_eq!(writer.position(), input.len() as u64);
            writer.seek(4).unwrap();
            writer.write(b"HEAD").unwrap();
            writer.finish().unwrap();
            read_file_into(&path, input.len(), &mut actual).unwrap();
            assert_eq!(actual, expected);
        }
        let mut writer = Writer::create(&path).unwrap();
        assert_eq!(
            writer.seek(u64::MAX).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        writer.finish().unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
        assert_eq!(
            Writer::create(&directory.join("missing/data"))
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::NotFound
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reads_whole_files_in_chunks() {
        let directory =
            std::env::temp_dir().join(format!("espeak-engine-io-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("data");
        let bytes: Vec<u8> = (0..CHUNK_BYTES * 2 + 1234).map(|i| (i * 7) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(read_file(&path, bytes.len()).unwrap(), bytes);
        let mut reuse = Vec::new();
        read_file_into(&path, bytes.len(), &mut reuse).unwrap();
        let allocation = reuse.as_ptr();
        for _ in 0..3 {
            read_file_into(&path, bytes.len(), &mut reuse).unwrap();
            assert_eq!(reuse.as_ptr(), allocation);
            assert_eq!(reuse, bytes);
        }
        let empty = directory.join("empty");
        std::fs::write(&empty, b"").unwrap();
        assert_eq!(read_file(&empty, 0).unwrap(), b"");
        assert_eq!(
            read_file(&path, bytes.len() - 1).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            read_file(&directory, usize::MAX).unwrap_err().kind(),
            io::ErrorKind::IsADirectory
        );
        assert_eq!(
            read_file(&directory.join("missing"), 9).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        if cfg!(feature = "proactor") {
            assert_ne!(
                backend(),
                Backend::Blocking,
                "a proactor should be available here"
            );
        } else {
            assert_eq!(backend(), Backend::Blocking);
        }
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
