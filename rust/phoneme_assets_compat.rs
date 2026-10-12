//! Serialized resident views; input/scratch copied before effects.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    core_storage::Slot,
    phoneme_assets::{Host, Paths, Workspace},
    phoneme_data::{MAX_TABLES, MISSING},
};
use std::{
    ffi::{c_char, c_int, c_void},
    sync::{Mutex, OnceLock},
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TableView {
    name: [u8; 32],
    records: *mut c_void,
    count: c_int,
    includes: c_int,
}
#[repr(C)]
pub struct Projection {
    buffers: [*mut u8; 4],
    lengths: [c_int; 4],
    wave: *mut u8,
    tables_count: c_int,
    active: c_int,
    number: c_int,
    current: c_int,
    tune_count: c_int,
    tables: [TableView; MAX_TABLES],
    selected: [*mut c_void; 256],
}
type Project = unsafe extern "C" fn(*mut c_void, *const Projection);
type FileError = unsafe extern "C" fn(*mut c_void, c_int, *const c_char) -> c_int;
type VersionError = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, c_int) -> c_int;
#[repr(C)]
pub struct Descriptor {
    context: *mut c_void,
    home: *const c_char,
    name: *const c_char,
    width: usize,
    separator: c_int,
    number: c_int,
    argument: c_int,
    operation: c_int,
    project: Option<Project>,
    file_error: Option<FileError>,
    version_error: Option<VersionError>,
}
#[repr(C)]
pub struct Outcome {
    rate: c_int,
    number: c_int,
}
struct Effects {
    descriptor: Descriptor,
    home: [u8; 4096],
}
impl Host for Effects {
    fn project(&mut self, w: &mut Workspace) {
        let slots = [
            Slot::Phontab,
            Slot::Phonindex,
            Slot::Phondata,
            Slot::Intonations,
        ];
        let lengths = slots.map(|s| w.storage.bytes(s).len() as i32);
        let buffers = slots.map(|s| w.storage.pointer(s));
        let mut out = Projection {
            buffers,
            lengths,
            wave: if w.wave_ready {
                buffers[2]
            } else {
                std::ptr::null_mut()
            },
            tables_count: 0,
            active: w.active,
            number: w.number,
            current: w.current,
            tune_count: w.tunes,
            tables: [TableView {
                name: [0; 32],
                records: std::ptr::null_mut(),
                count: 0,
                includes: 0,
            }; MAX_TABLES],
            selected: [std::ptr::null_mut(); 256],
        };
        if let Some(index) = &w.index {
            out.tables_count = index.tables().len() as i32;
            for (row, meta) in out.tables.iter_mut().zip(index.tables()) {
                *row = TableView {
                    name: meta.name,
                    records: buffers[0].wrapping_add(meta.records_offset).cast(),
                    count: meta.count as i32,
                    includes: meta.includes as i32,
                };
            }
        }
        for (pointer, &offset) in out.selected.iter_mut().zip(&w.selected) {
            if offset != MISSING {
                *pointer = buffers[0].wrapping_add(offset).cast();
            }
        }
        // SAFETY: initialized scratch copied synchronously into C. Resident
        // writable aliases derive from exclusive storage access and remain
        // live only until the next admitted mutation/release.
        // Callback cannot access/mutate their bytes until this control returns.
        unsafe { self.descriptor.project.unwrap()(self.descriptor.context, &out) }
    }
    fn file_error(&mut self, path: &[u8], error: std::io::Error) -> i32 {
        // SAFETY: initialized owned path and serialized live error allocator.
        unsafe {
            self.descriptor.file_error.unwrap()(
                self.descriptor.context,
                crate::compiler_driver::errno(&error),
                path.as_ptr().cast(),
            )
        }
    }
    fn version_error(&mut self, actual: u32) -> i32 {
        // SAFETY: owned terminated home; no callback retains the path loan.
        unsafe {
            self.descriptor.version_error.unwrap()(
                self.descriptor.context,
                self.home.as_ptr().cast(),
                actual as i32,
                crate::phoneme_data::PHONDATA_VERSION as i32,
            )
        }
    }
}
fn pool() -> &'static Mutex<Workspace> {
    static POOL: OnceLock<Mutex<Workspace>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(Workspace::default()))
}
unsafe fn text(pointer: *const c_char, width: usize) -> Result<[u8; 4096], i32> {
    if pointer.is_null() {
        return Err(22);
    }
    let mut out = [0; 4096];
    for (i, b) in out[..width].iter_mut().enumerate() {
        // SAFETY: initialized through NUL or width bytes on rejection.
        *b = unsafe { *pointer.add(i) as u8 };
        if *b == 0 {
            return Ok(out);
        }
    }
    Err(crate::compiler_driver::NAME_TOO_LONG)
}
/// Descriptor/text/out initialized/aligned; caller serializes the whole engine,
/// including all raw resident consumers. Outcome may overlap inputs (all copied
/// before effects) but not global projections or callback context. Callbacks
/// cannot unwind, retain scratch or touch resident bytes before return. Before
/// replacement/shutdown, C raw views are revoked, then recreated only from
/// initialized live owned storage. Immediate admission refuses reentry.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_phoneme_assets(
    d: *const Descriptor,
    out: *mut Outcome,
) -> c_int {
    if d.is_null() || out.is_null() {
        return 22;
    }
    // SAFETY: initialized aligned descriptor snapshot before callbacks.
    let d = unsafe { d.read() };
    if !(0..=4).contains(&d.operation)
        || !(2..=4096).contains(&d.width)
        || !(1..=255).contains(&d.separator)
        || d.project.is_none()
        || d.file_error.is_none()
        || d.version_error.is_none()
    {
        return 22;
    }
    let home = if d.operation == 0 {
        // SAFETY: admitted non-null bounded home text.
        match unsafe { text(d.home, d.width) } {
            Ok(s) => s,
            Err(e) => return e,
        }
    } else {
        [0; 4096]
    };
    let name = if d.operation == 3 || d.operation == 4 {
        // SAFETY: initialized name through NUL within 4096 on these operations.
        match unsafe { text(d.name, 4096) } {
            Ok(s) => s,
            Err(e) => return e,
        }
    } else {
        [0; 4096]
    };
    let operation = d.operation;
    let number = d.number;
    let argument = d.argument;
    let mut effects = Effects {
        descriptor: d,
        home,
    };
    let Ok(mut w) = pool().try_lock() else {
        return 11;
    };
    w.number = number;
    let mut result = Outcome {
        rate: 0,
        number: -1,
    };
    match operation {
        0 => {
            let paths = match Paths::new(
                crate::compiler_driver::string(&effects.home),
                effects.descriptor.width,
                effects.descriptor.separator as u8,
            ) {
                Ok(p) => p,
                Err(e) => return e,
            };
            match w.load(&paths, number, &mut effects) {
                Ok(rate) => result.rate = rate as i32,
                Err(status) => return status,
            }
        }
        1 => w.release(&mut effects),
        2 => {
            let changed = w.current != argument;
            let success = w.select(argument);
            if changed {
                effects.project(&mut w);
            }
            if !success {
                return 22;
            }
        }
        3 => {
            result.number = w.lookup(crate::compiler_driver::string(&name));
        }
        4 => {
            result.number = w.lookup(crate::compiler_driver::string(&name));
            if result.number >= 0 && w.current != result.number {
                w.select(result.number);
                effects.project(&mut w);
            }
        }
        _ => unreachable!(),
    }
    // SAFETY: admitted output written after all descriptor/text loans/effects.
    unsafe {
        out.write(result);
    }
    0
}
