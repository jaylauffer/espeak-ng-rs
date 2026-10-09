//! Engine lifecycle control and shared atomic runtime state.
// SPDX-License-Identifier: GPL-3.0-or-later
use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};

#[repr(usize)]
#[derive(Clone, Copy)]
pub enum Field {
    Mode,
    OutputRate,
    VoiceRate,
    Error,
}
impl Field {
    pub fn from_index(index: u32) -> Option<Self> {
        Some(match index {
            0 => Self::Mode,
            1 => Self::OutputRate,
            2 => Self::VoiceRate,
            3 => Self::Error,
            _ => return None,
        })
    }
}
pub struct State {
    values: [AtomicI32; 4],
    audio: AtomicPtr<c_void>,
}
impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}
impl State {
    pub const fn new() -> Self {
        Self {
            values: [
                AtomicI32::new(1),
                AtomicI32::new(0),
                AtomicI32::new(22050),
                AtomicI32::new(0),
            ],
            audio: AtomicPtr::new(std::ptr::null_mut()),
        }
    }
    pub fn get(&self, field: Field) -> i32 {
        self.values[field as usize].load(Ordering::Acquire)
    }
    pub fn set(&self, field: Field, value: i32) {
        self.values[field as usize].store(value, Ordering::Release);
    }
    pub fn audio(&self) -> *mut c_void {
        self.audio.load(Ordering::Acquire)
    }
    /// Lifecycle admission is serialized by the engine API owner. The handle
    /// is opaque here and reclaimed through that owner's host callbacks.
    pub fn admit_audio(&self, pointer: *mut c_void) {
        self.audio.store(pointer, Ordering::Release);
    }
    fn take_audio(&self) -> *mut c_void {
        self.audio.swap(std::ptr::null_mut(), Ordering::AcqRel)
    }
}
#[derive(Clone, Copy, Default)]
pub struct Capabilities {
    pub asynchronous: bool,
    pub audio: bool,
    pub mbrola: bool,
    pub proactor: bool,
}
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Config,
    SynthesisInit,
    NamesInit,
    QueueInit,
    QueueStop,
    QueueTerminate,
    EventClear,
    EventTerminate,
    PhonemeRelease,
    VoiceListRelease,
    CurrentVoiceRelease,
    AlternateRelease,
    DictionaryRelease,
    WaveFinish,
    NamesRelease,
    IconsRelease,
    MbrolaRelease,
    CurrentVoiceClear,
    StackReset,
    VoiceReset,
    EventsRelease,
    OutputRelease,
}
pub trait Host {
    fn capabilities(&self) -> Capabilities;
    fn action(&mut self, action: Action);
    fn locale(&mut self, name: &'static [u8]) -> bool;
    fn load_phonemes(&mut self, rate: &mut i32) -> u32;
    fn wave_init(&mut self, rate: i32);
    fn defaults(&mut self);
    fn option(&self, index: usize) -> i32;
    fn parameter(&mut self, index: usize, value: i32);
    fn saved(&self, index: usize) -> i32;
    fn phoneme_flags(&mut self, value: i32);
    fn echo_reset(&mut self);
    fn seed(&mut self);
    fn create_audio(&mut self) -> *mut c_void;
    fn close_audio(&mut self, pointer: *mut c_void);
    fn destroy_audio(&mut self, pointer: *mut c_void);
    fn flush_audio(&mut self, pointer: *mut c_void);
    fn output_reserve(&mut self, bytes: usize) -> bool;
    fn events_reserve(&mut self, count: i32) -> bool;
    fn synchronize(&mut self) -> u32;
    fn translator_release(&mut self);
    fn decoder_release(&mut self);
}
pub fn initialize(host: &mut impl Host) -> u32 {
    for name in [b"C.UTF-8\0".as_slice(), b"UTF-8\0", b"en_US.UTF-8\0", b"\0"] {
        if host.locale(name) {
            break;
        }
    }
    let mut rate = 22050;
    let status = host.load_phonemes(&mut rate);
    if status != 0 {
        return status;
    }
    host.wave_init(rate);
    for action in [
        Action::Config,
        Action::CurrentVoiceClear,
        Action::StackReset,
        Action::SynthesisInit,
        Action::NamesInit,
        Action::VoiceReset,
    ] {
        host.action(action);
    }
    host.defaults();
    host.parameter(1, 175);
    host.parameter(2, 100);
    host.parameter(6, host.option(0));
    host.parameter(5, host.option(1));
    host.parameter(7, 0);
    host.phoneme_flags(0);
    host.seed();
    0
}
/// Preserve C's defined arithmetic domain, rejecting overflow before allocating.
/// C rounds an exact millisecond boundary up by one sample too.
pub fn output_plan(length: i32, rate: i32) -> Option<(usize, i32)> {
    let length = length.max(60);
    if rate < 0 {
        return None;
    }
    let millisamples = length.checked_mul(rate)?;
    let bytes = millisamples
        .checked_add(1000)?
        .checked_sub(millisamples % 1000)?
        / 500;
    let events = length
        .checked_mul(200)?
        .checked_div(1000)?
        .checked_add(20)?;
    Some((usize::try_from(bytes).ok()?, events))
}
pub fn output(state: &State, host: &mut impl Host, mode: i32, length: i32, rate: i32) -> u32 {
    state.set(Field::Mode, mode);
    state.set(Field::OutputRate, 0);
    let cap = host.capabilities();
    if cap.audio && mode & 2 != 0 && state.audio().is_null() {
        state.admit_audio(host.create_audio());
    }
    if cap.asynchronous && mode & 1 == 0 {
        host.action(Action::QueueInit);
    }
    let Some((bytes, events)) = output_plan(length, rate) else {
        return 12;
    };
    if !host.output_reserve(bytes) || !host.events_reserve(events) {
        return 12;
    }
    0
}
pub fn cancel(state: &State, host: &mut impl Host) -> u32 {
    let cap = host.capabilities();
    if cap.asynchronous {
        host.action(Action::QueueStop);
        host.action(Action::EventClear);
    }
    if cap.audio && state.get(Field::Mode) & 2 != 0 {
        host.flush_audio(state.audio());
    }
    host.echo_reset();
    for index in 0..15 {
        host.parameter(index, host.saved(index));
    }
    0
}
pub fn synchronize(state: &State, host: &mut impl Host) -> u32 {
    let status = state.get(Field::Error) as u32;
    let cap = host.capabilities();
    // The optional retained pthread backend still uses its C oracle. Never
    // introduce a Rust polling fallback when no completion contract exists.
    if cap.asynchronous && (!cap.proactor || host.synchronize() != 0) {
        return 22;
    }
    state.set(Field::Error, 0);
    status
}
pub fn terminate(state: &State, host: &mut impl Host) -> u32 {
    let cap = host.capabilities();
    if cap.asynchronous {
        for action in [
            Action::QueueStop,
            Action::QueueTerminate,
            Action::EventTerminate,
        ] {
            host.action(action);
        }
    }
    // An admitted handle remains owned after changing to retrieval mode.
    // Relinquish it before destruction callbacks so it cannot be freed twice.
    let pointer = state.take_audio();
    if !pointer.is_null() {
        host.close_audio(pointer);
        host.destroy_audio(pointer);
    }
    if !pointer.is_null() || state.get(Field::Mode) & 2 != 0 {
        state.set(Field::OutputRate, 0);
    }
    for action in [
        Action::EventsRelease,
        Action::OutputRelease,
        Action::PhonemeRelease,
        Action::VoiceListRelease,
        Action::CurrentVoiceRelease,
    ] {
        host.action(action);
    }
    host.translator_release();
    host.action(Action::AlternateRelease);
    host.action(Action::DictionaryRelease);
    host.decoder_release();
    for action in [
        Action::WaveFinish,
        Action::NamesRelease,
        Action::IconsRelease,
    ] {
        host.action(action);
    }
    if cap.mbrola {
        host.action(Action::MbrolaRelease);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Spy {
        cap: Capabilities,
        calls: Vec<String>,
        load: u32,
        locale_fail: usize,
        allocation_fail: usize,
        sync: u32,
        saved: [i32; 15],
    }
    impl Host for Spy {
        fn capabilities(&self) -> Capabilities {
            self.cap
        }
        fn action(&mut self, action: Action) {
            self.calls.push(format!("{action:?}"));
        }
        fn locale(&mut self, _: &'static [u8]) -> bool {
            self.calls.push("locale".into());
            if self.locale_fail != 0 {
                self.locale_fail -= 1;
                false
            } else {
                true
            }
        }
        fn load_phonemes(&mut self, rate: &mut i32) -> u32 {
            self.calls.push(format!("load:{rate}"));
            *rate = 16000;
            self.load
        }
        fn wave_init(&mut self, rate: i32) {
            self.calls.push(format!("wave:{rate}"));
        }
        fn defaults(&mut self) {
            self.calls.push("defaults".into());
        }
        fn option(&self, index: usize) -> i32 {
            10 + index as i32
        }
        fn parameter(&mut self, index: usize, value: i32) {
            self.calls.push(format!("parameter:{index}:{value}"));
            if index + 1 < 15 {
                self.saved[index + 1] += 1;
            }
        }
        fn saved(&self, index: usize) -> i32 {
            self.saved[index]
        }
        fn phoneme_flags(&mut self, _: i32) {
            self.calls.push("flags".into());
        }
        fn echo_reset(&mut self) {
            self.calls.push("echo".into());
        }
        fn seed(&mut self) {
            self.calls.push("seed".into());
        }
        fn create_audio(&mut self) -> *mut c_void {
            self.calls.push("create".into());
            std::ptr::dangling_mut::<c_void>()
        }
        fn close_audio(&mut self, _: *mut c_void) {
            self.calls.push("close".into());
        }
        fn destroy_audio(&mut self, _: *mut c_void) {
            self.calls.push("destroy".into());
        }
        fn flush_audio(&mut self, _: *mut c_void) {
            self.calls.push("flush".into());
        }
        fn output_reserve(&mut self, bytes: usize) -> bool {
            self.calls.push(format!("output:{bytes}"));
            self.allocation_fail != 1
        }
        fn events_reserve(&mut self, events: i32) -> bool {
            self.calls.push(format!("events:{events}"));
            self.allocation_fail != 2
        }
        fn synchronize(&mut self) -> u32 {
            self.calls.push("synchronize".into());
            self.sync
        }
        fn translator_release(&mut self) {
            self.calls.push("translator".into());
        }
        fn decoder_release(&mut self) {
            self.calls.push("decoder".into());
        }
    }
    #[test]
    fn initialization_fallback_and_failed_load_fence_later_effects() {
        let mut host = Spy {
            locale_fail: 4,
            load: 0x100002ff,
            ..Spy::default()
        };
        assert_eq!(initialize(&mut host), host.load);
        assert_eq!(
            host.calls,
            ["locale", "locale", "locale", "locale", "load:22050"]
        );
        host.calls.clear();
        host.load = 0;
        assert_eq!(initialize(&mut host), 0);
        assert_eq!(&host.calls[0..3], ["locale", "load:22050", "wave:16000"]);
        assert_eq!(
            &host.calls[10..],
            [
                "parameter:1:175",
                "parameter:2:100",
                "parameter:6:10",
                "parameter:5:11",
                "parameter:7:0",
                "flags",
                "seed"
            ]
        );
    }
    #[test]
    fn output_checks_arithmetic_and_keeps_failure_order_without_allocating() {
        assert_eq!(output_plan(60, 22050), Some((2648, 32)));
        assert_eq!(output_plan(100, 16000), Some((3202, 40)));
        assert_eq!(output_plan(0, 0), Some((2, 32)));
        assert_eq!(output_plan(i32::MAX, 22050), None);
        assert_eq!(output_plan(i32::MAX, 0), None);
        assert_eq!(output_plan(60, -1), None);
        let state = State::new();
        let mut host = Spy {
            cap: Capabilities {
                audio: true,
                asynchronous: true,
                ..Capabilities::default()
            },
            allocation_fail: 1,
            ..Spy::default()
        };
        assert_eq!(output(&state, &mut host, 2, 60, 22050), 12);
        assert_eq!(host.calls, ["create", "QueueInit", "output:2648"]);
        host.calls.clear();
        host.allocation_fail = 2;
        assert_eq!(output(&state, &mut host, 2, 60, 22050), 12);
        assert_eq!(host.calls, ["QueueInit", "output:2648", "events:32"]);
    }
    #[test]
    fn retrieval_switch_retains_handle_until_exactly_once_teardown() {
        let state = State::new();
        let mut host = Spy {
            cap: Capabilities {
                audio: true,
                ..Capabilities::default()
            },
            ..Spy::default()
        };
        assert_eq!(output(&state, &mut host, 2, 60, 22050), 0);
        assert_eq!(output(&state, &mut host, 0, 60, 22050), 0);
        host.calls.clear();
        assert_eq!(terminate(&state, &mut host), 0);
        assert!(state.audio().is_null());
        assert_eq!(&host.calls[..2], ["close", "destroy"]);
        host.calls.clear();
        terminate(&state, &mut host);
        assert!(!host.calls.iter().any(|s| s == "destroy" || s == "close"));
    }
    #[test]
    fn cancel_reads_fresh_saved_values_and_failed_wait_preserves_error() {
        let state = State::new();
        let mut host = Spy::default();
        cancel(&state, &mut host);
        assert_eq!(&host.calls[..3], ["echo", "parameter:0:0", "parameter:1:1"]);
        state.set(Field::Error, 0x100005ff);
        host.cap.asynchronous = true;
        host.cap.proactor = true;
        host.sync = 22;
        assert_eq!(synchronize(&state, &mut host), 22);
        assert_eq!(state.get(Field::Error), 0x100005ff);
        host.sync = 0;
        assert_eq!(synchronize(&state, &mut host), 0x100005ff);
        assert_eq!(state.get(Field::Error), 0);
    }
}
