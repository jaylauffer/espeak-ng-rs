//! Fresh file reads with bounded resident dictionary snapshots and shared storage.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    core_storage::{Buffer, MAX_BYTES},
    dictionary::{Dictionary, InvalidDictionary, BUCKETS},
    rules::RuleIndex,
};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Weak},
};
pub const MAX_DICTIONARIES: usize = 128;
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(InvalidDictionary),
    Empty,
}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<InvalidDictionary> for Error {
    fn from(error: InvalidDictionary) -> Self {
        Self::Invalid(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Invalid(error) => error.fmt(formatter),
            Self::Empty => formatter.write_str("empty dictionary file"),
        }
    }
}
impl std::error::Error for Error {}
/// Immutable, aligned bytes and parsed indices stay live independently of cache
/// eviction. Translation borrows these snapshots; it never reparses file indices.
#[derive(Debug)]
pub struct Snapshot {
    buffer: Buffer,
    buckets: [usize; BUCKETS],
    rules_offset: usize,
    rules: RuleIndex,
}

impl Snapshot {
    pub fn bytes(&self) -> &[u8] {
        self.buffer.bytes()
    }
    pub fn buckets(&self) -> &[usize; BUCKETS] {
        &self.buckets
    }
    pub fn rules_offset(&self) -> usize {
        self.rules_offset
    }
    pub fn rules(&self) -> &RuleIndex {
        &self.rules
    }
}
struct Entry {
    path: PathBuf,
    data: Arc<Snapshot>,
    used: u64,
}
/// Setup/worker-only file cache. Every load rereads the file into reusable scratch
/// before comparing with resident bytes, preserving changes even when timestamps
/// and lengths are unchanged. Unchanged dictionaries reuse bytes and indices.
/// Limits count retained/retired pinned snapshots and scratch capacity, with
/// bounded admission/backpressure. Views must drain before engine teardown.
pub struct Cache {
    entries: Vec<Entry>,
    retired: Vec<Weak<Snapshot>>,
    scratch: Buffer,
    limit: usize,
    tick: u64,
}
impl Cache {
    pub fn new(limit: usize) -> io::Result<Self> {
        if limit == 0 || limit > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "dictionary storage limit must be 1..=134217728",
            ));
        }
        let mut entries = Vec::new();
        let mut retired = Vec::new();
        entries
            .try_reserve_exact(MAX_DICTIONARIES)
            .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        retired
            .try_reserve_exact(MAX_DICTIONARIES)
            .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        Ok(Self {
            entries,
            retired,
            scratch: Buffer::default(),
            limit,
            tick: 0,
        })
    }
    pub fn reserved_bytes(&self) -> usize {
        self.scratch.capacity()
            + self
                .entries
                .iter()
                .map(|entry| entry.data.buffer.capacity())
                .sum::<usize>()
            + self
                .retired
                .iter()
                .filter_map(Weak::upgrade)
                .map(|data| data.buffer.capacity())
                .sum::<usize>()
    }
    fn evict(&mut self) -> io::Result<()> {
        let candidate = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| Arc::strong_count(&entry.data) == 1)
            .min_by_key(|(_, entry)| entry.used)
            .map(|(index, _)| index);
        let Some(index) = candidate else {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "all cached dictionaries are pinned",
            ));
        };
        let Entry { path, data, used } = self.entries.swap_remove(index);
        match Arc::try_unwrap(data) {
            Ok(snapshot) => {
                if snapshot.buffer.capacity() > self.scratch.capacity() {
                    self.scratch = snapshot.buffer;
                }
                Ok(())
            }
            Err(data) => {
                self.entries.push(Entry { path, data, used });
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "dictionary became pinned during eviction",
                ))
            }
        }
    }
    pub fn load(&mut self, path: &Path) -> Result<Arc<Snapshot>, Error> {
        if path.as_os_str().as_encoded_bytes().len() > 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "dictionary path exceeds bound",
            )
            .into());
        }
        let metadata = std::fs::metadata(path)?;
        if metadata.is_dir() {
            return Err(io::Error::from(io::ErrorKind::IsADirectory).into());
        }
        if metadata.len() == 0 {
            return Err(Error::Empty);
        }
        let length = usize::try_from(metadata.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "dictionary size exceeds address space",
            )
        })?;
        let mut file = File::open(path)?;
        self.read(path, &mut file, length)
    }
    /// Adopt a snapshot from host-proactor resident bytes on initialization/a
    /// worker. This path performs no filesystem operation; supplied bytes are the
    /// authoritative version and use the same bounds/cache/validation contract.
    pub fn resident(&mut self, path: &Path, bytes: &[u8]) -> Result<Arc<Snapshot>, Error> {
        self.read(path, &mut io::Cursor::new(bytes), bytes.len())
    }
    fn read(
        &mut self,
        path: &Path,
        source: &mut impl io::Read,
        length: usize,
    ) -> Result<Arc<Snapshot>, Error> {
        if path.as_os_str().as_encoded_bytes().len() > 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "dictionary path exceeds bound",
            )
            .into());
        }
        if length == 0 {
            return Err(Error::Empty);
        }
        self.retired.retain(|data| data.strong_count() > 0);
        if self.entries.len() == MAX_DICTIONARIES
            && !self.entries.iter().any(|entry| entry.path == path)
        {
            self.evict()?;
        }
        let required = length
            .checked_add(7)
            .map(|length| length / 8 * 8)
            .filter(|length| *length <= self.limit)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "dictionary exceeds storage bound",
                )
            })?;
        for attempt in 0..=MAX_DICTIONARIES {
            if self
                .reserved_bytes()
                .checked_add(required.saturating_sub(self.scratch.capacity()))
                .is_some_and(|total| total <= self.limit)
            {
                break;
            }
            if attempt == MAX_DICTIONARIES {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "dictionary storage admission did not converge",
                )
                .into());
            }
            self.evict()?;
        }
        self.scratch.read(source, length)?;
        self.tick = self.tick.wrapping_add(1);
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.path == path && entry.data.bytes() == self.scratch.bytes())
        {
            entry.used = self.tick;
            return Ok(Arc::clone(&entry.data));
        }
        let view = Dictionary::parse(self.scratch.bytes())?;
        let buckets = *view.bucket_offsets();
        let rules_offset = view.rules_offset();
        let rules = view.rule_index()?;
        let previous = self.entries.iter().position(|entry| entry.path == path);
        if previous.is_some_and(|index| Arc::strong_count(&self.entries[index].data) > 1)
            && self.retired.len() == MAX_DICTIONARIES
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "too many pinned retired dictionaries",
            )
            .into());
        }
        let data = Arc::new(Snapshot {
            buffer: std::mem::take(&mut self.scratch),
            buckets,
            rules_offset,
            rules,
        });
        if let Some(index) = previous {
            let old = std::mem::replace(&mut self.entries[index].data, Arc::clone(&data));
            self.entries[index].used = self.tick;
            match Arc::try_unwrap(old) {
                Ok(snapshot) => self.scratch = snapshot.buffer,
                Err(old) => self.retired.push(Arc::downgrade(&old)),
            }
        } else {
            self.entries.push(Entry {
                path: path.to_path_buf(),
                data: Arc::clone(&data),
                used: self.tick,
            });
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dictionary(version: u8) -> Vec<u8> {
        let mut bytes = vec![0; 8 + BUCKETS];
        bytes[..4].copy_from_slice(&(BUCKETS as u32).to_le_bytes());
        bytes[4..8].copy_from_slice(&((8 + BUCKETS) as u32).to_le_bytes());
        bytes.extend_from_slice(&[7, version]);
        bytes
    }
    #[test]
    fn unchanged_snapshots_reuse_storage_and_pinned_versions_have_backpressure() {
        let mut cache = Cache::new(3120).unwrap();
        let path = Path::new("en_dict");
        let first = cache.resident(path, &dictionary(0)).unwrap();
        let same = cache.resident(path, &dictionary(0)).unwrap();
        assert!(Arc::ptr_eq(&first, &same));
        drop(same);
        assert_eq!(cache.reserved_bytes(), 2080);
        let second = cache.resident(path, &dictionary(1)).unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
        let third = cache.resident(path, &dictionary(2)).unwrap();
        assert_eq!(cache.reserved_bytes(), 3120);
        assert!(
            matches!(cache.resident(path,&dictionary(3)),Err(Error::Io(error)) if error.kind()==io::ErrorKind::WouldBlock)
        );
        assert_eq!(first.bytes(), dictionary(0));
        assert_eq!(second.bytes(), dictionary(1));
        assert_eq!(third.bytes(), dictionary(2));
        drop(first);
        drop(second);
        let same = cache.resident(path, &dictionary(2)).unwrap();
        assert!(Arc::ptr_eq(&third, &same));
        assert_eq!(cache.reserved_bytes(), 2080);
        assert!(matches!(
            cache.resident(path, b"bad"),
            Err(Error::Invalid(_))
        ));
        assert_eq!(third.bytes(), dictionary(2));
        assert!(matches!(cache.resident(path, b""), Err(Error::Empty)));
    }
    #[test]
    fn eviction_overwrites_unpinned_storage_and_entry_count_stays_bounded() {
        let mut cache = Cache::new(2080).unwrap();
        let first = cache.resident(Path::new("a"), &dictionary(0)).unwrap();
        let address = first.bytes().as_ptr();
        drop(first);
        let second = cache.resident(Path::new("b"), &dictionary(1)).unwrap();
        drop(second);
        let third = cache.resident(Path::new("c"), &dictionary(2)).unwrap();
        assert_eq!(third.bytes().as_ptr(), address);
        assert_eq!(cache.entries.len(), 2);
        let mut larger = Cache::new(MAX_BYTES).unwrap();
        for index in 0..=MAX_DICTIONARIES {
            larger
                .resident(Path::new(&format!("{index}")), &dictionary(0))
                .unwrap();
        }
        assert_eq!(larger.entries.len(), MAX_DICTIONARIES);
        assert!(larger.retired.is_empty());
        assert!(larger.reserved_bytes() < MAX_BYTES);
    }
    #[test]
    fn file_reads_detect_changes_with_unchanged_length_and_modified_time() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("espeak-dictionary-{}-{stamp}", std::process::id()));
        std::fs::write(&path, dictionary(0)).unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let mut cache = Cache::new(3120).unwrap();
        let first = cache.load(&path).unwrap();
        std::fs::write(&path, dictionary(1)).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        let second = cache.load(&path).unwrap();
        assert_eq!(second.bytes(), dictionary(1));
        assert_eq!(first.bytes(), dictionary(0));
        std::fs::remove_file(&path).unwrap();
        assert!(
            matches!(cache.load(&path),Err(Error::Io(error)) if error.kind()==io::ErrorKind::NotFound)
        );
    }
}
