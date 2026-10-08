//! Audio playback (`speech.c`'s pcaudio output) with its waits on the
//! loadngo proactor.
//!
//! Synthesis writes 16-bit mono samples at the voice's rate. [`Sink`]
//! converts them to the device's rate and channel count (linear
//! interpolation) into a bounded queue, which the device's real-time fill
//! callback ([`Sink::fill`]) drains, emitting silence when the queue is
//! empty or busy. A writer that finds the queue full, and a drain that waits
//! for it to empty, block on a completion of the sink's own proactor: the
//! fill callback posts one when it frees room and a writer is waiting, and a
//! cancel posts one to release the writer at once. There is no sleep or
//! polling.
//!
//! The device itself comes from loadngo-audio-io (the `audio` feature);
//! without it a sink is driven by whoever calls [`Sink::fill`], as the tests
//! do.
// SPDX-License-Identifier: GPL-3.0-or-later
use loadngo_proactor::{new_platform_proactor, PlatformPort, Proactor, ProactorHandle};
use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A write or drain was cut short by [`Sink::cancel`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cancelled;

/// Converts 16-bit mono at one rate to `f32` mono at another by linear
/// interpolation, carrying its position across calls.
#[derive(Clone, Debug)]
pub struct Resampler {
    step: f64,
    position: f64,
    previous: f32,
}

impl Resampler {
    pub fn new(from_hz: u32, to_hz: u32) -> Self {
        Self {
            step: f64::from(from_hz.max(1)) / f64::from(to_hz.max(1)),
            position: 0.0,
            previous: 0.0,
        }
    }

    /// Appends the converted `samples` to `out`. `position` indexes
    /// `samples`; the previous call's last sample sits at -1.
    pub fn convert(&mut self, samples: &[i16], out: &mut Vec<f32>) {
        let value = |i: isize| -> f32 {
            if i < 0 {
                self.previous
            } else {
                f32::from(samples[i as usize]) / 32768.0
            }
        };
        let last = samples.len() as f64 - 1.0;
        while self.position <= last {
            let base = self.position.floor();
            let fraction = (self.position - base) as f32;
            let index = base as isize;
            let a = value(index);
            if fraction > 0.0 {
                out.push(a + (value(index + 1) - a) * fraction);
            } else {
                out.push(a);
            }
            self.position += self.step;
        }
        if let Some(&sample) = samples.last() {
            self.previous = f32::from(sample) / 32768.0;
            self.position -= samples.len() as f64;
        }
    }

    /// Forgets the carried sample (after a cancel).
    pub fn reset(&mut self) {
        self.position = 0.0;
        self.previous = 0.0;
    }
}

struct Shared {
    queue: Mutex<VecDeque<f32>>,
    capacity: usize,
    /// A writer or drain is blocked and wants a completion.
    waiting: AtomicBool,
    /// Bumped by each cancel.
    generation: AtomicU64,
    wake: ProactorHandle<PlatformPort>,
}

impl Shared {
    fn wake(&self) {
        // an empty work item: its completion returns the waiter's run_once
        let _ = self.wake.enqueue_work(|_| {});
    }
}

/// The real-time side: what the device's fill callback holds.
#[derive(Clone)]
pub struct Filler(Arc<Shared>);

impl Filler {
    /// Writes every sample of `buffer` (interleaved, `channels` wide):
    /// queued mono frames copied to each channel, then silence. Never
    /// blocks: when the writer holds the queue, the buffer is silence.
    pub fn fill(&self, buffer: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let mut written = 0;
        if let Ok(mut queue) = self.0.queue.try_lock() {
            for frame in buffer.chunks_mut(channels) {
                let Some(sample) = queue.pop_front() else {
                    break;
                };
                frame.fill(sample);
                written += frame.len();
            }
        }
        buffer[written..].fill(0.0);
        if written > 0 && self.0.waiting.swap(false, Ordering::AcqRel) {
            self.0.wake();
        }
    }
}

/// A playback queue whose writers wait on a proactor.
pub struct Sink {
    shared: Arc<Shared>,
    proactor: Proactor<PlatformPort>,
    resampler: Resampler,
    converted: Vec<f32>,
    /// The cancel generation the resampler's carried sample belongs to.
    generation: u64,
}

impl Sink {
    /// A sink converting from `voice_hz` to `device_hz`, holding up to
    /// `capacity` device frames.
    pub fn new(voice_hz: u32, device_hz: u32, capacity: usize) -> io::Result<Self> {
        let proactor = new_platform_proactor()?;
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::with_capacity(capacity.max(1))),
            capacity: capacity.max(1),
            waiting: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            wake: proactor.handle(),
        });
        Ok(Self {
            shared,
            proactor,
            resampler: Resampler::new(voice_hz, device_hz),
            converted: Vec::new(),
            generation: 0,
        })
    }

    /// Converts from `voice_hz` to `device_hz` from now on.
    pub fn set_rates(&mut self, voice_hz: u32, device_hz: u32) {
        self.resampler = Resampler::new(voice_hz, device_hz);
    }

    /// The handle for the device's fill callback.
    pub fn filler(&self) -> Filler {
        Filler(Arc::clone(&self.shared))
    }

    /// The handle a cancel from another thread uses.
    pub fn canceller(&self) -> Canceller {
        Canceller(Arc::clone(&self.shared))
    }

    /// Frames queued for the device.
    pub fn queued(&self) -> usize {
        self.lock().len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<f32>> {
        self.shared.queue.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Blocks on the proactor until `ready` holds for the queue, or a
    /// cancel arrives.
    fn wait_until(
        &self,
        generation: u64,
        ready: impl Fn(&VecDeque<f32>) -> bool,
    ) -> Result<(), Cancelled> {
        loop {
            if self.shared.generation.load(Ordering::Acquire) != generation {
                return Err(Cancelled);
            }
            // announce the wait before checking, so a fill in between wakes us
            self.shared.waiting.store(true, Ordering::Release);
            if ready(&self.lock()) {
                self.shared.waiting.store(false, Ordering::Release);
                return Ok(());
            }
            if self.shared.generation.load(Ordering::Acquire) != generation {
                return Err(Cancelled);
            }
            if self.proactor.run_once().is_err() {
                return Err(Cancelled);
            }
        }
    }

    /// Queues `samples` (voice rate, mono), waiting for room as the device
    /// plays. A cancel during the write drops the rest.
    pub fn write(&mut self, samples: &[i16]) -> Result<(), Cancelled> {
        let generation = self.shared.generation.load(Ordering::Acquire);
        if generation != self.generation {
            // cancelled since the last write: drop the carried sample
            self.resampler.reset();
            self.generation = generation;
        }
        self.converted.clear();
        self.resampler.convert(samples, &mut self.converted);
        let capacity = self.shared.capacity;
        let mut done = 0;
        while done < self.converted.len() {
            self.wait_until(generation, |queue| queue.len() < capacity)?;
            let mut queue = self.lock();
            if self.shared.generation.load(Ordering::Acquire) != generation {
                return Err(Cancelled);
            }
            let room = capacity - queue.len();
            let take = room.min(self.converted.len() - done);
            queue.extend(&self.converted[done..done + take]);
            done += take;
        }
        Ok(())
    }

    /// Waits until the device has taken everything queued.
    pub fn drain(&self) -> Result<(), Cancelled> {
        let generation = self.shared.generation.load(Ordering::Acquire);
        self.wait_until(generation, VecDeque::is_empty)
    }

    /// Drops what is queued (from the writing thread).
    pub fn cancel(&self) {
        self.canceller().cancel();
    }
}

/// Cancels a sink's queued audio from any thread.
#[derive(Clone)]
pub struct Canceller(Arc<Shared>);

impl Canceller {
    /// Drops what is queued and releases a blocked writer or drain.
    pub fn cancel(&self) {
        self.0.generation.fetch_add(1, Ordering::AcqRel);
        self.0
            .queue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        self.0.waiting.store(false, Ordering::Release);
        self.0.wake();
    }
}

/// A sink playing through the system's output device (pcaudio's
/// `audio_object`). The device is opened on first use and kept across
/// voice rate changes, which only change the conversion.
#[cfg(feature = "audio")]
pub struct Device {
    name: Option<String>,
    sink: Sink,
    stream: Option<loadngo_audio_io::OutputStream>,
}

/// Device frames queued at most: a quarter second at 48 kHz.
#[cfg(feature = "audio")]
pub const DEVICE_QUEUE_FRAMES: usize = 12_000;

#[cfg(feature = "audio")]
impl Device {
    /// A device named `name` (the default when `None`), not yet opened.
    pub fn new(name: Option<String>) -> io::Result<Self> {
        Ok(Self {
            name,
            sink: Sink::new(22050, 22050, DEVICE_QUEUE_FRAMES)?,
            stream: None,
        })
    }

    /// Opens the device if needed and converts from `voice_hz`.
    pub fn open(&mut self, voice_hz: u32) -> Result<(), String> {
        if self.stream.is_none() {
            let filler = self.sink.filler();
            let stream = loadngo_audio_io::open_output_stream(
                self.name.as_deref(),
                None,
                move |buffer, channels| filler.fill(buffer, channels),
            )
            .map_err(|error| error.to_string())?;
            self.stream = Some(stream);
        }
        let device_hz = self
            .stream
            .as_ref()
            .map_or(voice_hz, |s| s.sample_rate_hz());
        self.sink.set_rates(voice_hz, device_hz);
        Ok(())
    }

    /// Stops the device, dropping what is queued.
    pub fn close(&mut self) {
        self.sink.cancel();
        self.stream = None;
    }

    pub fn sink(&mut self) -> &mut Sink {
        &mut self.sink
    }

    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    /// Milliseconds of audio queued for the device (0 when closed).
    pub fn latency_ms(&self) -> u32 {
        let rate = self.stream.as_ref().map_or(0, |s| s.sample_rate_hz());
        if rate == 0 {
            return 0;
        }
        (self.sink.queued() as u64 * 1000 / u64::from(rate)) as u32
    }
}

/// The C API, shaped like pcaudio's `audio_object` so `speech.c` keeps its
/// calls. Errors: 0 ok, 1 the device failed to open, 2 not open, 3 the
/// write or drain was cancelled.
#[cfg(all(feature = "audio", feature = "c-abi"))]
mod c_api {
    use super::{Canceller, Device};
    use std::ffi::{c_char, c_int, CStr};

    /// The writer's device, with a canceller another thread may use while
    /// a write is blocked.
    pub struct RustAudio {
        device: std::cell::UnsafeCell<Device>,
        canceller: Canceller,
        error: std::cell::UnsafeCell<std::ffi::CString>,
    }

    impl RustAudio {
        /// # Safety
        /// Only the thread that writes may call this (pcaudio's contract).
        #[allow(clippy::mut_from_ref)]
        unsafe fn device(&self) -> &mut Device {
            // SAFETY: caller contract; the canceller never touches it.
            unsafe { &mut *self.device.get() }
        }
    }

    #[no_mangle]
    extern "C" fn espeak_rs_audio_create(name: *const c_char) -> *mut RustAudio {
        let name = (!name.is_null()).then(|| {
            // SAFETY: a C string from the caller.
            unsafe { CStr::from_ptr(name) }
                .to_string_lossy()
                .into_owned()
        });
        match Device::new(name) {
            Ok(device) => {
                let canceller = device.sink.canceller();
                Box::into_raw(Box::new(RustAudio {
                    device: device.into(),
                    canceller,
                    error: std::ffi::CString::default().into(),
                }))
            }
            Err(_) => std::ptr::null_mut(),
        }
    }

    /// # Safety
    /// `audio` is null or from `espeak_rs_audio_create`, not yet destroyed.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_audio_destroy(audio: *mut RustAudio) {
        if !audio.is_null() {
            // SAFETY: caller contract.
            drop(unsafe { Box::from_raw(audio) });
        }
    }

    /// # Safety
    /// As for destroy; called on the writing thread.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_audio_open(audio: *mut RustAudio, rate: c_int) -> c_int {
        // SAFETY: caller contract.
        let Some(audio) = (unsafe { audio.as_ref() }) else {
            return 2;
        };
        // SAFETY: the writing thread.
        match unsafe { audio.device() }.open(rate.max(1) as u32) {
            Ok(()) => 0,
            Err(message) => {
                let message =
                    std::ffi::CString::new(message.replace('\0', " ")).unwrap_or_default();
                // SAFETY: the error is only touched on the writing thread.
                unsafe { *audio.error.get() = message };
                1
            }
        }
    }

    /// # Safety
    /// As for open.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_audio_close(audio: *mut RustAudio) {
        // SAFETY: caller contract.
        if let Some(audio) = unsafe { audio.as_ref() } {
            // SAFETY: the writing thread.
            unsafe { audio.device() }.close();
        }
    }

    /// Queues `bytes` of 16-bit samples, waiting on the proactor for room.
    ///
    /// # Safety
    /// As for open; `samples` holds `bytes` bytes.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_audio_write(
        audio: *mut RustAudio,
        samples: *const i16,
        bytes: usize,
    ) -> c_int {
        // SAFETY: caller contract.
        let Some(audio) = (unsafe { audio.as_ref() }) else {
            return 2;
        };
        // SAFETY: the writing thread.
        let device = unsafe { audio.device() };
        if !device.is_open() {
            return 2;
        }
        let samples = if samples.is_null() || bytes < 2 {
            &[][..]
        } else {
            // SAFETY: caller contract.
            unsafe { std::slice::from_raw_parts(samples, bytes / 2) }
        };
        device.sink().write(samples).map_or(3, |()| 0)
    }

    /// Waits on the proactor until the device has played what is queued.
    ///
    /// # Safety
    /// As for open.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_audio_drain(audio: *mut RustAudio) -> c_int {
        // SAFETY: caller contract.
        let Some(audio) = (unsafe { audio.as_ref() }) else {
            return 2;
        };
        // SAFETY: the writing thread.
        let device = unsafe { audio.device() };
        if !device.is_open() {
            return 0;
        }
        device.sink().drain().map_or(3, |()| 0)
    }

    /// Milliseconds queued for the device, for timing events.
    ///
    /// # Safety
    /// As for open.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_audio_latency_ms(audio: *mut RustAudio) -> c_int {
        // SAFETY: caller contract.
        let Some(audio) = (unsafe { audio.as_ref() }) else {
            return 0;
        };
        // SAFETY: the writing thread.
        c_int::try_from(unsafe { audio.device() }.latency_ms()).unwrap_or(c_int::MAX)
    }

    /// Drops what is queued and releases a blocked write; any thread.
    ///
    /// # Safety
    /// `audio` is null or live.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_audio_flush(audio: *mut RustAudio) -> c_int {
        // SAFETY: caller contract.
        if let Some(audio) = unsafe { audio.as_ref() } {
            audio.canceller.cancel();
        }
        0
    }

    /// # Safety
    /// As for open.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_audio_strerror(
        audio: *mut RustAudio,
        error: c_int,
    ) -> *const c_char {
        let fixed: &'static CStr = match error {
            0 => c"no error",
            2 => c"audio device not open",
            3 => c"audio cancelled",
            _ => c"audio device error",
        };
        // SAFETY: caller contract.
        match unsafe { audio.as_ref() } {
            // SAFETY: the writing thread.
            Some(audio) if error == 1 => unsafe { (*audio.error.get()).as_ptr() },
            _ => fixed.as_ptr(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn resampling_interpolates_across_calls() {
        let mut same = Resampler::new(22050, 22050);
        let mut out = Vec::new();
        same.convert(&[0, 16384, -32768], &mut out);
        same.convert(&[8192], &mut out);
        assert_eq!(out, [0.0, 0.5, -1.0, 0.25]);

        // doubling the rate puts a midpoint between each pair, including
        // across the call boundary
        let mut up = Resampler::new(11025, 22050);
        let mut out = Vec::new();
        up.convert(&[0, 16384], &mut out);
        up.convert(&[-16384], &mut out);
        assert_eq!(out, [0.0, 0.25, 0.5, 0.0, -0.5]);

        let mut down = Resampler::new(44100, 22050);
        let mut out = Vec::new();
        down.convert(&[0, 1, 16384, 3, -16384], &mut out);
        assert_eq!(out, [0.0, 0.5, -0.5]);
        down.reset();
        out.clear();
        down.convert(&[16384], &mut out);
        assert_eq!(out, [0.5]);
    }

    #[test]
    fn fill_copies_to_channels_then_silence() {
        let mut sink = Sink::new(8000, 8000, 16).unwrap();
        sink.write(&[16384, -16384]).unwrap();
        let mut buffer = [9.0f32; 6];
        sink.filler().fill(&mut buffer, 2);
        assert_eq!(buffer, [0.5, 0.5, -0.5, -0.5, 0.0, 0.0]);
        assert_eq!(sink.queued(), 0);
    }

    #[test]
    fn a_full_queue_waits_for_the_device() {
        let mut sink = Sink::new(8000, 8000, 4).unwrap();
        let filler = sink.filler();
        let (played, heard) = mpsc::channel();
        let device = std::thread::spawn(move || {
            let mut total = Vec::new();
            while total.len() < 12 {
                std::thread::sleep(Duration::from_millis(2));
                let mut buffer = [0.0f32; 3];
                filler.fill(&mut buffer, 1);
                total.extend(buffer.iter().copied().filter(|&s| s != 0.0));
            }
            played.send(total).unwrap();
        });
        let samples: Vec<i16> = (1..=12).map(|i| i * 1024).collect();
        sink.write(&samples).unwrap();
        sink.drain().unwrap();
        assert_eq!(sink.queued(), 0);
        let total = heard.recv().unwrap();
        device.join().unwrap();
        let expected: Vec<f32> = samples.iter().map(|&s| f32::from(s) / 32768.0).collect();
        assert_eq!(total, expected);
    }

    #[test]
    fn cancel_releases_a_blocked_writer() {
        let mut sink = Sink::new(8000, 8000, 2).unwrap();
        let canceller = sink.canceller();
        let cancel = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            canceller.cancel();
        });
        // no device: the write blocks until the cancel
        assert_eq!(sink.write(&[1, 2, 3, 4, 5]), Err(Cancelled));
        cancel.join().unwrap();
        assert_eq!(sink.queued(), 0);
        // the sink is usable again
        sink.write(&[7]).unwrap();
        assert_eq!(sink.queued(), 1);
        sink.cancel();
        assert_eq!((sink.queued(), sink.drain()), (0, Ok(())));
        // the cancel dropped the carried sample: no midpoint before 0.5
        let mut up = Sink::new(4000, 8000, 8).unwrap();
        up.write(&[16384]).unwrap();
        up.cancel();
        up.write(&[16384]).unwrap();
        let mut buffer = [9.0f32; 2];
        up.filler().fill(&mut buffer, 1);
        assert_eq!(buffer, [0.5, 0.0]);
    }
}
