//! The engine's file reads: phoneme data, dictionaries, voices, sound icons
//! and MBROLA tables.
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

#[cfg(not(feature = "proactor"))]
mod imp {
    use super::Backend;
    use std::io::{self, Read};
    use std::path::Path;

    pub fn read(path: &Path, length: usize, bytes: &mut Vec<u8>) -> io::Result<()> {
        std::fs::File::open(path)?
            .take(length as u64)
            .read_to_end(bytes)?;
        Ok(())
    }

    pub fn backend() -> Backend {
        Backend::Blocking
    }
}

#[cfg(feature = "proactor")]
mod imp {
    use super::{Backend, CHUNK_BYTES};
    use crate::data_io::{open_data_file, DataFile, DataReader};
    use loadngo_proactor::{new_platform_proactor, IoPort, PlatformPort, Proactor};
    use std::io::{self, Read};
    use std::path::Path;
    use std::sync::{Arc, Mutex, OnceLock};

    enum Engine {
        Platform(Proactor<PlatformPort>, DataReader, Slot),
        #[cfg(target_os = "linux")]
        Epoll(Proactor<loadngo_proactor::EpollPort>, DataReader, Slot),
        Blocking,
    }

    fn engine() -> &'static Mutex<Engine> {
        static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();
        ENGINE.get_or_init(|| {
            let reader = DataReader::new(CHUNK_BYTES).ok();
            let engine = match (new_platform_proactor(), reader.clone()) {
                (Ok(proactor), Some(reader)) => Engine::Platform(proactor, reader, slot()),
                #[cfg(target_os = "linux")]
                (Err(_), Some(reader)) => match loadngo_proactor::EpollPort::new() {
                    Ok(port) => Engine::Epoll(Proactor::new(port), reader, slot()),
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
            Engine::Platform(proactor, reader, slot) => {
                read_with(proactor, reader, slot, path, length, bytes)
            }
            #[cfg(target_os = "linux")]
            Engine::Epoll(proactor, reader, slot) => {
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
        while bytes.len() < length {
            slot.lock().unwrap_or_else(|p| p.into_inner()).result = None;
            let wanted = (length - bytes.len()).min(CHUNK_BYTES);
            let done = Arc::clone(slot);
            reader.read_registered(&handle, file.clone(), bytes.len() as u64, move |result| {
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
                proactor.run_once()?;
            };
            if used == 0 {
                break; // the file shrank
            }
            bytes.extend_from_slice(&slot.lock().unwrap_or_else(|p| p.into_inner()).bytes[..used]);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
