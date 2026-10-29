//! Serialized C views of retained Rust spectral compiler owners.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    spect_file::{self, Formant, LoadError, Peak, Sequence, UNSUPPORTED},
    wavegen::Wavegen,
};
use std::{
    ffi::{c_char, c_void},
    sync::{Mutex, OnceLock},
};
#[repr(C)]
pub struct Frame {
    keyframe: i32,
    amplitude: i16,
    length_adjust: f32,
    rms: f64,
    time: f32,
    pitch: f32,
    length: f32,
    dx: f32,
    nx: u16,
    markers: i16,
    max_y: i32,
    spect: *mut u16,
    klatt: [i16; 14],
    formants: [Formant; 9],
    peaks: [Peak; 9],
}
#[repr(C)]
pub struct Pitch {
    first: u16,
    last: u16,
    env: [u8; 128],
}
#[repr(C)]
pub struct View {
    count: i32,
    amplitude: i16,
    spare: i32,
    name: *mut c_char,
    frames: *mut *mut Frame,
    pitchenv: Pitch,
    first: i32,
    last: i32,
    duration: i32,
    grid: i32,
    bass: i32,
    max_x: i32,
    max_y: i16,
    format: i32,
}
impl Default for View {
    fn default() -> Self {
        Self {
            count: 0,
            amplitude: 0,
            spare: 0,
            name: std::ptr::null_mut(),
            frames: std::ptr::null_mut(),
            pitchenv: Pitch {
                first: 0,
                last: 0,
                env: [0; 128],
            },
            first: 0,
            last: 0,
            duration: 0,
            grid: 1,
            bass: 0,
            max_x: 3000,
            max_y: 1,
            format: 0,
        }
    }
}
#[derive(Default)]
struct Owner {
    view: View,
    sequence: Sequence,
    frames: Vec<Frame>,
    pointers: Vec<*mut Frame>,
    leased: bool,
}
// SAFETY: raw views point only into this stable boxed owner and retained arenas.
// Every API operation takes the pool lock; foreign access is serialized and
// exclusive to a lease, with no loans retained across a call or moved thread.
unsafe impl Send for Owner {}
impl Owner {
    fn clear(&mut self) {
        self.view = View::default();
        self.sequence.clear();
        self.frames.clear();
        self.pointers.clear();
    }
    fn publish(&mut self) -> Result<(), i32> {
        let count = self.sequence.frames().len();
        self.frames.clear();
        self.pointers.clear();
        self.frames.try_reserve_exact(count).map_err(|_| 12)?;
        self.pointers.try_reserve_exact(count).map_err(|_| 12)?;
        let bins = self.sequence.bins().as_ptr().cast_mut();
        for f in self.sequence.frames() {
            // SAFETY: range endpoints are checked by the owned parser; bins
            // never reallocates while C holds the published frame view.
            let spect = unsafe { bins.add(f.bins.start) };
            self.frames.push(Frame {
                keyframe: f.keyframe,
                amplitude: f.amplitude,
                length_adjust: f.length_adjust,
                rms: f.rms,
                time: f.time,
                pitch: f.pitch,
                length: f.length,
                dx: f.dx,
                nx: f.bins.len() as u16,
                markers: f.markers,
                max_y: f.max_y,
                spect,
                klatt: f.klatt,
                formants: f.formants,
                peaks: f.peaks,
            });
        }
        for frame in &mut self.frames {
            self.pointers.push(frame as *mut Frame);
        }
        self.view.count = count as i32;
        self.view.amplitude = self.sequence.amplitude;
        self.view.name = if self.sequence.name().is_empty() {
            std::ptr::null_mut()
        } else {
            self.sequence.name().as_ptr().cast_mut().cast()
        };
        self.view.frames = if count == 0 {
            std::ptr::null_mut()
        } else {
            self.pointers.as_mut_ptr()
        };
        self.view.max_x = self.sequence.max_x;
        self.view.max_y = self.sequence.max_y;
        self.view.format = self.sequence.format;
        self.view.duration = self.sequence.duration;
        self.view.first = i32::from(self.view.pitchenv.first);
        self.view.last = i32::from(self.view.pitchenv.last);
        Ok(())
    }
}
const OWNERS: usize = 4;
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Frame>() == 248 && std::mem::size_of::<View>() == 200);
fn pool() -> &'static Mutex<[Option<Box<Owner>>; OWNERS]> {
    static POOL: OnceLock<Mutex<[Option<Box<Owner>>; OWNERS]>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(std::array::from_fn(|_| None)))
}
/// C access is serialized; create/load/destroy delimit exclusive view leases.
#[no_mangle]
pub extern "C" fn espeak_rs_spect_create() -> *mut View {
    let Ok(mut slots) = pool().try_lock() else {
        return std::ptr::null_mut();
    };
    for slot in slots.iter_mut() {
        let owner = slot.get_or_insert_with(|| Box::new(Owner::default()));
        if !owner.leased {
            owner.clear();
            owner.leased = true;
            return &mut owner.view;
        }
    }
    std::ptr::null_mut()
}
#[no_mangle]
pub extern "C" fn espeak_rs_spect_destroy(view: *mut View) {
    let Ok(mut slots) = pool().try_lock() else {
        return;
    };
    for owner in slots.iter_mut().flatten() {
        if std::ptr::eq(&owner.view, view) && owner.leased {
            owner.clear();
            owner.leased = false;
            return;
        }
    }
}
type Log = unsafe extern "C" fn(*const u8, usize);
fn errno(e: &std::io::Error) -> i32 {
    #[cfg(unix)]
    if let Some(code) = e.raw_os_error() {
        return code;
    }
    match e.kind() {
        std::io::ErrorKind::NotFound => 2,
        std::io::ErrorKind::PermissionDenied => 13,
        std::io::ErrorKind::IsADirectory => 21,
        std::io::ErrorKind::NotADirectory => 20,
        std::io::ErrorKind::OutOfMemory => 12,
        std::io::ErrorKind::WouldBlock => 11,
        _ => 22,
    }
}
unsafe extern "C" fn standard_error_log(bytes: *const u8, length: usize) {
    // SAFETY: the loader's initialized message fragment, written to C's stderr.
    unsafe {
        crate::crt::write(
            crate::crt::standard_error(),
            std::slice::from_raw_parts(bytes, length),
        );
    }
}
/// `LoadSpectSeq`: as `espeak_rs_spect_load`, logging to C's `stderr`.
///
/// # Safety
/// As for `espeak_rs_spect_load`.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_spect_load_crt(view: *mut View, filename: *const c_char) -> i32 {
    // SAFETY: forwarded caller contract.
    unsafe { espeak_rs_spect_load(view, filename, Some(standard_error_log)) }
}
/// Initialized filename through NUL within 4096; callback is synchronous and
/// cannot retain loans or unwind. The registered view is leased exclusively.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_spect_load(
    view: *mut View,
    filename: *const c_char,
    log: Option<Log>,
) -> i32 {
    if filename.is_null() || view.is_null() || log.is_none() {
        return UNSUPPORTED;
    }
    let mut path = [0_u8; 4096];
    let mut end = None;
    for (i, b) in path.iter_mut().enumerate() {
        // SAFETY: caller initialized filename bytes through NUL or all 4096.
        *b = unsafe { filename.add(i).cast::<u8>().read() };
        if *b == 0 {
            end = Some(i);
            break;
        }
    }
    let Some(end) = end else { return UNSUPPORTED };
    let Ok(native) = crate::voice_storage::compat_path(&path[..end]) else {
        return UNSUPPORTED;
    };
    let Ok(mut slots) = pool().try_lock() else {
        return UNSUPPORTED;
    };
    let Some(owner) = slots
        .iter_mut()
        .flatten()
        .find(|o| o.leased && std::ptr::eq(&o.view, view))
    else {
        return UNSUPPORTED;
    };
    let result = owner.sequence.load(&native);
    if !matches!(result, Err(LoadError::Io(_))) {
        if let Err(code) = owner.publish() {
            owner.clear();
            return code;
        }
    }
    match result {
        Ok(()) => 0,
        Err(LoadError::Format(code)) => {
            if code == UNSUPPORTED {
                let bytes = b"Unsupported spectral file format.\n";
                // SAFETY: initialized static log bytes, synchronous projection.
                unsafe {
                    log.unwrap()(bytes.as_ptr(), bytes.len());
                }
            }
            code
        }
        Err(LoadError::Io(error)) => {
            let mut bytes = [0_u8; 4120];
            let prefix = b"Failed to open: '";
            bytes[..prefix.len()].copy_from_slice(prefix);
            bytes[prefix.len()..prefix.len() + end].copy_from_slice(&path[..end]);
            bytes[prefix.len() + end] = b'\'';
            // SAFETY: initialized owned log bytes; no filename loan survives I/O.
            unsafe {
                log.unwrap()(bytes.as_ptr(), prefix.len() + end + 1);
            }
            errno(&error)
        }
    }
}
/// Live initialized disjoint x/y arrays of n points; borrowed only during call.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_spect_polint(
    x: *const f32,
    y: *const f32,
    n: i32,
    at: f32,
) -> f32 {
    if x.is_null() || y.is_null() || !(1..=8).contains(&n) {
        return 0.0;
    }
    // SAFETY: admitted initialized n-element arrays; no callbacks or mutation.
    let (x, y) = unsafe {
        (
            std::slice::from_raw_parts(x, n as usize),
            std::slice::from_raw_parts(y, n as usize),
        )
    };
    spect_file::interpolate(x, y, at).unwrap_or(0.0)
}
/// Live Rust wavegen exclusive to caller; frame belongs to a current leased
/// owner. No foreign pointer is dereferenced before its registry admission.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_spect_rms(
    frame: *mut Frame,
    amplitude: i32,
    wavegen: *mut c_void,
    rate: i32,
) -> f64 {
    if frame.is_null() || wavegen.is_null() {
        return 0.0;
    }
    let Ok(mut slots) = pool().try_lock() else {
        return 0.0;
    };
    for owner in slots.iter_mut().flatten().filter(|o| o.leased) {
        let Some(offset) = (frame as usize).checked_sub(owner.frames.as_ptr() as usize) else {
            continue;
        };
        let size = std::mem::size_of::<Frame>();
        if offset % size != 0 {
            continue;
        }
        if let Some(f) = owner.frames.get_mut(offset / size) {
            // SAFETY: live serialized engine-owned Rust Wavegen, disjoint from
            // the pool; no callback or foreign loan in the native calculation.
            let generator = unsafe { &mut *wavegen.cast::<Wavegen>() };
            f.rms = spect_file::frame_rms(&f.peaks, amplitude, f.amplitude, generator, rate)
                .unwrap_or(0.0);
            return f.rms;
        }
    }
    0.0
}
