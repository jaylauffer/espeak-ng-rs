//! Resident engine assets, loaded with bounded admission on a caller's proactor.
//!
//! Open/allocate during initialization or on an I/O worker. Completion work is
//! one bounded chunk copy. Indexing is a separate initialization/CPU-worker step,
//! never disguised as proactor offload. Asset storage is retained across speech.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::data_io::{open_data_file, DataFile, DataReader};
use crate::dictionary::OwnedDictionary;
use crate::phoneme_data::{self, TableIndex};
use loadngo_proactor::{IoPort, ProactorHandle};
use std::collections::VecDeque;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub const MAX_RESIDENT_BYTES: usize = crate::core_storage::MAX_BYTES;

#[derive(Debug)]
enum Asset {
    Phontab,
    Phonindex,
    Phondata,
    Intonations,
    Dictionary(String),
}
struct PendingFile {
    asset: Asset,
    file: Arc<File>,
    /// `file` registered with the loading proactor when its first read is
    /// submitted; released when this entry is dropped.
    registered: Option<DataFile>,
    length: usize,
    bytes: Vec<u8>,
}

/// Prepared at initialization/on an I/O worker, not in paint/input callbacks.
pub struct PreparedAssets {
    files: VecDeque<PendingFile>,
    total_bytes: usize,
}
impl PreparedAssets {
    /// `root` is the directory containing phontab and language `_dict` files.
    /// The complete plan is admitted before any read. Each final allocation is
    /// reserved exactly once, and combined storage cannot exceed `byte_limit`.
    pub fn open(root: &Path, dictionaries: &[&str], byte_limit: usize) -> io::Result<Self> {
        if byte_limit == 0 || byte_limit > MAX_RESIDENT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resident byte limit must be 1..=134217728",
            ));
        }
        if dictionaries.len() > 128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "at most 128 resident dictionaries are supported",
            ));
        }
        let mut plan = Self {
            files: VecDeque::new(),
            total_bytes: 0,
        };
        for (name, asset) in [
            ("phontab", Asset::Phontab),
            ("phonindex", Asset::Phonindex),
            ("phondata", Asset::Phondata),
            ("intonations", Asset::Intonations),
        ] {
            plan.add(root, name, asset, byte_limit)?;
        }
        for (number, &name) in dictionaries.iter().enumerate() {
            if name.is_empty()
                || name.len() > 39
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                || dictionaries[..number].contains(&name)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "dictionary names must be unique ASCII language identifiers",
                ));
            }
            plan.add(
                root,
                &format!("{name}_dict"),
                Asset::Dictionary(name.into()),
                byte_limit,
            )?;
        }
        Ok(plan)
    }
    fn add(&mut self, root: &Path, name: &str, asset: Asset, limit: usize) -> io::Result<()> {
        let file = open_data_file(&root.join(name))?;
        let length = usize::try_from(file.metadata()?.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "asset length exceeds address space",
            )
        })?;
        let total = self
            .total_bytes
            .checked_add(length)
            .filter(|sum| *sum <= limit)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "speech assets exceed resident byte limit",
                )
            })?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|_| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "cannot reserve resident speech asset",
            )
        })?;
        self.files.push_back(PendingFile {
            asset,
            file,
            registered: None,
            length,
            bytes,
        });
        self.total_bytes = total;
        Ok(())
    }
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

/// Unindexed completed reads; native indexing can be substantial CPU work.
/// Pass this to the host's bounded worker or index during initialization.
pub struct ResidentBytes {
    files: Vec<(Asset, Vec<u8>)>,
}
impl ResidentBytes {
    pub fn index(self) -> io::Result<ResidentAssets> {
        let mut result = ResidentAssets {
            phontab: Vec::new(),
            phonindex: Vec::new(),
            phondata: Vec::new(),
            intonations: Vec::new(),
            tables: None,
            dictionaries: Vec::new(),
            sample_rate: 0,
        };
        for (asset, bytes) in self.files {
            match asset {
                Asset::Phontab => {
                    result.tables = Some(TableIndex::parse(&bytes).map_err(invalid_data)?);
                    result.phontab = bytes;
                }
                Asset::Phonindex => {
                    if bytes.len() % 2 != 0 {
                        return Err(invalid_data("phonindex has an incomplete 16-bit word"));
                    }
                    result.phonindex = bytes;
                }
                Asset::Phondata => {
                    result.sample_rate = phoneme_data::sample_rate(&bytes).map_err(invalid_data)?;
                    result.phondata = bytes;
                }
                Asset::Intonations => result.intonations = bytes,
                Asset::Dictionary(name) => result
                    .dictionaries
                    .push((name, OwnedDictionary::parse(bytes).map_err(invalid_data)?)),
            }
        }
        Ok(result)
    }
}
fn invalid_data(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

/// Immutable, explicitly owned native engine data. No process-global pointers.
pub struct ResidentAssets {
    phontab: Vec<u8>,
    phonindex: Vec<u8>,
    phondata: Vec<u8>,
    intonations: Vec<u8>,
    tables: Option<TableIndex>,
    dictionaries: Vec<(String, OwnedDictionary)>,
    sample_rate: u32,
}
impl ResidentAssets {
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn phontab(&self) -> &[u8] {
        &self.phontab
    }
    pub fn phonindex(&self) -> &[u8] {
        &self.phonindex
    }
    /// Borrow the already resident instructions; execution allocates nothing.
    pub fn phoneme_programs(&self) -> crate::phoneme_program::Program<'_> {
        crate::phoneme_program::Program::new(&self.phonindex)
            .expect("resident index validated complete instruction words")
    }
    pub fn phondata(&self) -> &[u8] {
        &self.phondata
    }
    /// Borrow resident spectra and envelopes without allocation or I/O.
    pub fn spectra(&self) -> crate::spectrum::SpectrumData<'_> {
        crate::spectrum::SpectrumData::new(&self.phondata)
    }
    pub fn intonations(&self) -> &[u8] {
        &self.intonations
    }
    pub fn tables(&self) -> &TableIndex {
        self.tables
            .as_ref()
            .expect("prepared plan always includes phontab")
    }
    pub fn dictionary(&self, name: &str) -> Option<&OwnedDictionary> {
        self.dictionaries
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, dict)| dict)
    }
    pub fn dictionaries(&self) -> impl Iterator<Item = (&str, &OwnedDictionary)> {
        self.dictionaries
            .iter()
            .map(|(name, dict)| (name.as_str(), dict))
    }
}

/// Cooperative cancellation: finish the outstanding chunk, then deliver
/// Interrupted before submitting another read. Caller must continue/drain the
/// host loop. No polling or timer is introduced; completion wakes the host.
#[derive(Clone)]
pub struct LoadCancellation(Arc<AtomicBool>);
impl LoadCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// One assembled plan at a time, shared by clones, with one reusable I/O buffer.
#[derive(Clone)]
pub struct ResidentLoader {
    reader: DataReader,
    active: Arc<AtomicBool>,
}
impl ResidentLoader {
    pub fn new(chunk_bytes: usize) -> io::Result<Self> {
        Ok(Self {
            reader: DataReader::new(chunk_bytes)?,
            active: Arc::new(AtomicBool::new(false)),
        })
    }
    pub fn is_busy(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    pub fn load<P: IoPort>(
        &self,
        handle: &ProactorHandle<P>,
        plan: PreparedAssets,
        done: impl FnOnce(io::Result<ResidentBytes>) + Send + 'static,
    ) -> io::Result<LoadCancellation> {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "resident asset load already in flight",
                )
            })?;
        let cancel = LoadCancellation(Arc::new(AtomicBool::new(false)));
        let job = Job {
            pending: plan.files,
            files: Vec::new(),
            loader: self.clone(),
            cancel: cancel.clone(),
            done: Some(Box::new(done)),
        };
        let posted_handle = handle.clone();
        handle.enqueue_work(move |_| job.next(posted_handle))?;
        Ok(cancel)
    }
}
type Done = Box<dyn FnOnce(io::Result<ResidentBytes>) + Send>;
struct Job {
    pending: VecDeque<PendingFile>,
    files: Vec<(Asset, Vec<u8>)>,
    loader: ResidentLoader,
    cancel: LoadCancellation,
    done: Option<Done>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.loader.active.store(false, Ordering::Release);
    }
}
impl Job {
    fn finish(mut self, result: io::Result<ResidentBytes>) {
        // Publish admission before calling user code, permitting the next plan.
        // The old job must not clear the new job's admission in Drop.
        let done = self.done.take().expect("one terminal completion");
        drop(self);
        done(result);
    }
    fn next<P: IoPort>(mut self, handle: ProactorHandle<P>) {
        if self.cancel.0.load(Ordering::Acquire) {
            self.finish(Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "resident asset load cancelled",
            )));
            return;
        }
        while self
            .pending
            .front()
            .is_some_and(|file| file.bytes.len() == file.length)
        {
            let file = self.pending.pop_front().expect("present file");
            self.files.push((file.asset, file.bytes));
        }
        let Some(file) = self.pending.front_mut() else {
            let files = std::mem::take(&mut self.files);
            self.finish(Ok(ResidentBytes { files }));
            return;
        };
        let reader = self.loader.reader.clone();
        let offset = file.bytes.len() as u64;
        let registered = match &file.registered {
            Some(registered) => registered.clone(),
            None => match DataFile::register(&handle, Arc::clone(&file.file)) {
                Ok(registered) => file.registered.insert(registered).clone(),
                Err(error) => {
                    self.finish(Err(error));
                    return;
                }
            },
        };
        // Keep recoverable job ownership if the port refuses the submission.
        let shared_job = Arc::new(std::sync::Mutex::new(Some(self)));
        let completion_job = Arc::clone(&shared_job);
        let completion_handle = handle.clone();
        let submit = reader.read_registered(&handle, registered, offset, move |result| {
            let mut job = completion_job
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take()
                .expect("pending read job");
            let file = job.pending.front_mut().expect("pending file");
            let error = match result {
                Err(error) => Some(error),
                Ok([]) => Some(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "resident asset shortened during load",
                )),
                Ok(bytes) => {
                    let remaining = file.length - file.bytes.len();
                    file.bytes
                        .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
                    None
                }
            };
            // Post after the callback loan has returned. Never retry inline.
            *completion_job.lock().unwrap_or_else(|p| p.into_inner()) = Some(job);
            let posted_job = Arc::clone(&completion_job);
            let next_handle = completion_handle.clone();
            let posted = completion_handle.enqueue_work(move |_| {
                let job = posted_job
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                    .expect("posted read job");
                if let Some(error) = error {
                    job.finish(Err(error));
                } else {
                    job.next(next_handle);
                }
            });
            if let Err(error) = posted {
                let job = completion_job
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take();
                if let Some(job) = job {
                    job.finish(Err(error));
                }
            }
        });
        if let Err(error) = submit {
            if let Some(job) = shared_job.lock().unwrap_or_else(|p| p.into_inner()).take() {
                job.finish(Err(error));
            }
        }
    }
}
