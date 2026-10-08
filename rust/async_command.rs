//! Owned asynchronous engine commands (`espeak_command.c`).
//!
//! The C queue sees only the prefix view. Payload buffers belong to this
//! owner until processing/discard finishes. State changes precede host calls,
//! including mandatory completion of a discarded pending message.
// SPDX-License-Identifier: GPL-3.0-or-later
use std::alloc::{alloc, Layout};
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

#[cfg(windows)]
pub type Wide = u16;
#[cfg(not(windows))]
pub type Wide = i32;
pub const UNDEFINED: i32 = 0;
pub const PENDING: i32 = 1;
pub const PROCESSED: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Text {
    pub id: u32,
    pub text: *mut c_void,
    pub position: u32,
    pub position_type: i32,
    pub end: u32,
    pub flags: u32,
    pub user: *mut c_void,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Mark {
    pub id: u32,
    pub text: *mut c_void,
    pub mark: *const c_char,
    pub end: u32,
    pub flags: u32,
    pub user: *mut c_void,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Key {
    pub id: u32,
    pub user: *mut c_void,
    pub name: *const c_char,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Character {
    pub id: u32,
    pub user: *mut c_void,
    pub character: Wide,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Parameter {
    pub parameter: i32,
    pub value: i32,
    pub relative: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Terminated {
    pub id: u32,
    pub user: *mut c_void,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Voice {
    pub name: *const c_char,
    pub languages: *const c_char,
    pub identifier: *const c_char,
    pub gender: u8,
    pub age: u8,
    pub variant: u8,
    pub internal: u8,
    pub score: i32,
    pub spare: *mut c_void,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union ViewData {
    pub text: Text,
    pub mark: Mark,
    pub key: Key,
    pub character: Character,
    pub parameter: Parameter,
    pub punctuation: *const Wide,
    pub name: *const c_char,
    pub voice: Voice,
    pub terminated: Terminated,
}
#[repr(C)]
pub struct View {
    pub kind: i32,
    pub state: i32,
    pub data: ViewData,
}

/// Text can be UTF-8, UTF-16 or wchar_t. Its allocation must retain the
/// alignment malloc supplied to the C decoder, with an initialized wide NUL.
pub struct TextBuffer(Vec<Wide>);
const TEXT_CACHE_SLOTS: usize = 4;
const MAX_CACHED_TEXT_BYTES: usize = 2 * 1024 * 1024;
struct TextPool(Mutex<[Option<Vec<Wide>>; TEXT_CACHE_SLOTS]>);
static TEXT_POOL: TextPool = TextPool(Mutex::new([const { None }; TEXT_CACHE_SLOTS]));
impl TextPool {
    fn take(&self, words: usize) -> Vec<Wide> {
        // Oversized captures must not evict/grow the reusable normal pool.
        if words > MAX_CACHED_TEXT_BYTES / size_of::<Wide>() {
            return Vec::new();
        }
        let mut slots = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let adequate = slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| {
                slot.as_ref()
                    .filter(|v| v.capacity() >= words)
                    .map(|v| (i, v.capacity()))
            })
            .min_by_key(|&(_, capacity)| capacity);
        let largest = || {
            slots
                .iter()
                .enumerate()
                .filter_map(|(i, slot)| slot.as_ref().map(|v| (i, v.capacity())))
                .max_by_key(|&(_, capacity)| capacity)
        };
        adequate
            .or_else(largest)
            .and_then(|(i, _)| slots[i].take())
            .unwrap_or_default()
    }
    fn put(&self, storage: Vec<Wide>) {
        if storage.capacity() > MAX_CACHED_TEXT_BYTES / size_of::<Wide>() {
            return;
        }
        let old = {
            let mut slots = self.0.lock().unwrap_or_else(|p| p.into_inner());
            let slot = slots.iter().position(Option::is_none).or_else(|| {
                slots
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.as_ref().unwrap().capacity())
                    .filter(|(_, v)| v.as_ref().unwrap().capacity() < storage.capacity())
                    .map(|(i, _)| i)
            });
            let Some(slot) = slot else {
                return;
            };
            slots[slot].replace(storage)
        };
        drop(old); // deallocation never occurs while holding the pool lock
    }
}
impl Drop for TextBuffer {
    fn drop(&mut self) {
        TEXT_POOL.put(std::mem::take(&mut self.0));
    }
}
impl TextBuffer {
    pub fn copy(input: &[u8]) -> Option<Self> {
        let words = input.len().checked_add(size_of::<Wide>() - 1)? / size_of::<Wide>();
        let words = words.checked_add(1)?;
        let mut storage = TEXT_POOL.take(words);
        if words > storage.len() {
            storage.try_reserve_exact(words - storage.len()).ok()?;
        }
        storage.resize(words, 0);
        // SAFETY: source and freshly allocated destination are disjoint;
        // destination has enough initialized wide units for all input bytes
        // and a whole NUL. Every bit pattern is valid for Wide (i32/u16).
        unsafe {
            std::ptr::copy_nonoverlapping(input.as_ptr(), storage.as_mut_ptr().cast(), input.len());
            std::ptr::write_bytes(
                storage.as_mut_ptr().cast::<u8>().add(input.len()),
                0,
                words * size_of::<Wide>() - input.len(),
            );
        }
        Some(Self(storage))
    }
    fn as_mut_ptr(&mut self) -> *mut c_void {
        self.0.as_mut_ptr().cast()
    }
}

/// Owned buffers include their terminators. Text receives an extra whole
/// code unit so the supplied byte extent need not already end in NUL.
pub enum Data {
    Text(Text, TextBuffer),
    Mark(Mark, TextBuffer, Vec<u8>),
    Key(Key, Vec<u8>),
    Character(Character),
    Parameter(Parameter),
    Punctuation(Vec<Wide>),
    VoiceName(Vec<u8>),
    Voice(Voice, [Option<Vec<u8>>; 3]),
    Terminated(Terminated),
}
pub trait Host {
    fn process(&mut self, data: &Data);
    fn terminated(&mut self, completion: Terminated);
}
#[repr(C)]
pub struct Command {
    pub view: View,
    data: Data,
}

impl Command {
    /// Fallible owner allocation. Partial payload ownership is released on
    /// failure; the C prefix and its buffer addresses remain stable in a Box.
    pub fn new(mut data: Data) -> Option<Box<Self>> {
        let (kind, raw) = match &mut data {
            Data::Text(args, bytes) => {
                args.text = bytes.as_mut_ptr();
                (0, ViewData { text: *args })
            }
            Data::Mark(args, bytes, mark) => {
                args.text = bytes.as_mut_ptr();
                args.mark = mark.as_ptr().cast();
                (1, ViewData { mark: *args })
            }
            Data::Key(args, name) => {
                args.name = name.as_ptr().cast();
                (2, ViewData { key: *args })
            }
            Data::Character(args) => (3, ViewData { character: *args }),
            Data::Parameter(args) => (4, ViewData { parameter: *args }),
            Data::Punctuation(list) => (
                5,
                ViewData {
                    punctuation: list.as_ptr(),
                },
            ),
            Data::VoiceName(name) => (
                6,
                ViewData {
                    name: name.as_ptr().cast(),
                },
            ),
            Data::Voice(args, strings) => {
                let pointers = strings
                    .each_ref()
                    .map(|s| s.as_ref().map_or(std::ptr::null(), |s| s.as_ptr().cast()));
                [args.name, args.languages, args.identifier] = pointers;
                (7, ViewData { voice: *args })
            }
            Data::Terminated(args) => (8, ViewData { terminated: *args }),
        };
        // SAFETY: checked nonzero allocation, initialized once and transferred
        // to Box with the identical global allocator and layout.
        unsafe {
            let pointer = alloc(Layout::new::<Self>()).cast::<Self>();
            if pointer.is_null() {
                return None;
            }
            pointer.write(Self {
                view: View {
                    kind,
                    state: UNDEFINED,
                    data: raw,
                },
                data,
            });
            Some(Box::from_raw(pointer))
        }
    }
    pub fn process(&mut self, host: &mut impl Host) {
        host.process(self.prepare_process());
    }
    /// Finish mutation before lending only the payload to a host. In the C
    /// adapter the exclusive prefix borrow ends before its callback runs.
    pub fn prepare_process(&mut self) -> &Data {
        self.view.state = PROCESSED;
        &self.data
    }
    /// Assign only after all allocations succeed, as the retained C does.
    pub fn assign_id(&mut self, counter: &AtomicU32) {
        match &mut self.data {
            Data::Text(args, _) => {
                args.id = next_id(counter);
                self.view.data = ViewData { text: *args };
            }
            Data::Mark(args, _, _) => {
                args.id = next_id(counter);
                self.view.data = ViewData { mark: *args };
            }
            Data::Key(args, _) => {
                args.id = next_id(counter);
                self.view.data = ViewData { key: *args };
            }
            Data::Character(args) => {
                args.id = next_id(counter);
                self.view.data = ViewData { character: *args };
            }
            _ => {}
        }
    }
    /// Pending completions run even when stop discards synthesis. State is
    /// changed first so later disposal cannot notify the completion twice.
    pub fn discard(&mut self, host: &mut impl Host) {
        if let Some(completion) = self.take_completion() {
            host.terminated(completion);
        }
    }
    /// Copy the cleanup notification out before entering owner callbacks.
    pub fn take_completion(&mut self) -> Option<Terminated> {
        if self.view.state == PENDING {
            if let Data::Terminated(completion) = self.data {
                self.view.state = PROCESSED;
                return Some(completion);
            }
        }
        None
    }
}
/// Upstream wraps without resetting the identifier. Atomic admission also
/// permits simultaneous API producers without racing the counter store.
pub fn next_id(counter: &AtomicU32) -> u32 {
    counter.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Recorder(Vec<u32>);
    impl Host for Recorder {
        fn process(&mut self, data: &Data) {
            if let Data::Terminated(args) = data {
                self.terminated(*args);
            }
        }
        fn terminated(&mut self, args: Terminated) {
            self.0.push(args.id);
        }
    }
    #[test]
    fn pending_discard_notifies_once_and_processed_discard_does_not_repeat() {
        let mut host = Recorder::default();
        for state in [UNDEFINED, PENDING, PROCESSED] {
            let mut command = Command::new(Data::Terminated(Terminated {
                id: state as u32,
                user: std::ptr::null_mut(),
            }))
            .unwrap();
            command.view.state = state;
            command.discard(&mut host);
            command.discard(&mut host);
        }
        assert_eq!(host.0, [PENDING as u32]);
        let mut command = Command::new(Data::Terminated(Terminated {
            id: 10,
            user: std::ptr::null_mut(),
        }))
        .unwrap();
        command.view.state = PENDING;
        command.process(&mut host);
        command.discard(&mut host);
        assert_eq!(host.0, [1, 10]);
        assert_eq!(command.view.state, PROCESSED);
    }
    #[test]
    fn identifiers_wrap_and_concurrent_producers_do_not_repeat() {
        let counter = AtomicU32::new(u32::MAX - 1);
        assert_eq!(
            (next_id(&counter), next_id(&counter), next_id(&counter)),
            (u32::MAX, 0, 1)
        );
        let counter = AtomicU32::new(0);
        let mut ids = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| (0..500).map(|_| next_id(&counter)).collect::<Vec<_>>()))
                .collect();
            workers
                .into_iter()
                .flat_map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        ids.sort_unstable();
        assert_eq!(ids, (1..=4000).collect::<Vec<_>>());
    }
    #[test]
    fn text_copies_have_wide_alignment_and_initialized_termination() {
        for length in 1..17 {
            let input = vec![0xff; length];
            let mut buffer = TextBuffer::copy(&input).unwrap();
            let pointer = buffer.as_mut_ptr();
            assert_eq!(pointer as usize % align_of::<Wide>(), 0);
            // SAFETY: copy owns words * sizeof(Wide) initialized bytes;
            // this temporary read does not outlive or mutate the allocation.
            let bytes = unsafe {
                std::slice::from_raw_parts(pointer.cast::<u8>(), buffer.0.len() * size_of::<Wide>())
            };
            assert_eq!(&bytes[..length], input);
            assert!(bytes[length..].iter().all(|byte| *byte == 0));
        }
    }
    #[test]
    fn text_pool_reuses_capacity_and_bounds_retained_storage() {
        let pool = TextPool(Mutex::new([const { None }; TEXT_CACHE_SLOTS]));
        let storage = vec![1; 1024];
        let pointer = storage.as_ptr();
        pool.put(storage);
        let storage = pool.take(512);
        assert_eq!(storage.as_ptr(), pointer);
        assert_eq!(storage.len(), 1024);
        pool.put(storage);
        assert!(pool
            .take(MAX_CACHED_TEXT_BYTES / size_of::<Wide>() + 1)
            .is_empty());
        assert_eq!(pool.0.lock().unwrap().iter().flatten().count(), 1);
        pool.put(vec![0; MAX_CACHED_TEXT_BYTES / size_of::<Wide>() + 1]);
        assert_eq!(pool.0.lock().unwrap().iter().flatten().count(), 1);
        for _ in 0..TEXT_CACHE_SLOTS + 2 {
            pool.put(vec![0; 2048]);
        }
        assert_eq!(
            pool.0.lock().unwrap().iter().flatten().count(),
            TEXT_CACHE_SLOTS
        );
    }
}
