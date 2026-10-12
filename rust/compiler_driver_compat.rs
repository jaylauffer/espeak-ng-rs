//! Copied public compiler admission; no foreign path survives an effect.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    compiler_driver::{self, Counts, Host, Op, Paths},
    compiler_manifest,
    voice_reader::TextMode,
};
use std::{
    ffi::{c_char, c_int, c_void},
    sync::{Mutex, OnceLock},
};
type Effect =
    unsafe extern "C" fn(*mut c_void, c_int, c_int, *const c_char, *const c_char, *mut u8) -> c_int;
type Count = unsafe extern "C" fn(*mut c_void, *mut Counts);
type Log = unsafe extern "C" fn(*mut c_void, *const u8, usize);
type Error = unsafe extern "C" fn(*mut c_void, c_int, *const c_char) -> c_int;
#[repr(C)]
pub struct Descriptor {
    pub(super) context: *mut c_void,
    pub(super) home: *const c_char,
    pub(super) source: *const c_char,
    pub(super) destination: *const c_char,
    pub(super) alpha: *const u8,
    pub(super) spaces: *const u8,
    pub(super) width: usize,
    pub(super) windows: c_int,
    pub(super) keep_prefix_x: c_int,
    pub(super) effect: Option<Effect>,
    pub(super) counts: Option<Count>,
    pub(super) log: Option<Log>,
    pub(super) error: Option<Error>,
}
struct Effects(Descriptor);
impl Host for Effects {
    fn effect(
        &mut self,
        op: Op,
        value: i32,
        source: &[u8],
        destination: &[u8],
        out: &mut [u8; 4096],
    ) -> i32 {
        // SAFETY: initialized owned terminated paths/scratch, synchronous live context.
        unsafe {
            self.0.effect.unwrap()(
                self.0.context,
                op as i32,
                value,
                source.as_ptr().cast(),
                destination.as_ptr().cast(),
                out.as_mut_ptr(),
            )
        }
    }
    fn counts(&mut self) -> Counts {
        let mut c = Counts::default();
        // SAFETY: initialized disjoint scalar scratch; no retained reference.
        unsafe { self.0.counts.unwrap()(self.0.context, &mut c) }
        c
    }
    fn log(&mut self, bytes: &[u8]) {
        // SAFETY: owned bytes are loaned only for this synchronous effect.
        unsafe { self.0.log.unwrap()(self.0.context, bytes.as_ptr(), bytes.len()) }
    }
    fn file_error(&mut self, status: i32, path: &[u8]) -> i32 {
        // SAFETY: owned terminated path; callback cannot retain or unwind.
        unsafe { self.0.error.unwrap()(self.0.context, status, path.as_ptr().cast()) }
    }
}
fn pool() -> &'static Mutex<compiler_manifest::Workspace> {
    static POOL: OnceLock<Mutex<compiler_manifest::Workspace>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(compiler_manifest::Workspace::default()))
}
#[repr(C)]
pub struct ManifestDescriptor {
    bytes: *const u8,
    length: usize,
    width: usize,
    windows: c_int,
    keep_prefix_x: c_int,
    alpha: *const u8,
    spaces: *const u8,
}
/// Initialized input/maps and aligned output capacity/used. Input may overlap
/// output; admission and parsing complete before publication. Refusal leaves
/// outputs untouched. Rows contain fixed initialized names and no raw loans.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_manifest_parse(
    descriptor: *const ManifestDescriptor,
    rows: *mut compiler_manifest::Row,
    capacity: usize,
    used: *mut usize,
) -> c_int {
    if descriptor.is_null()
        || used.is_null()
        || capacity > compiler_manifest::MAX_LINES
        || (capacity != 0 && rows.is_null())
    {
        return 22;
    }
    // SAFETY: initialized aligned descriptor copied before output.
    let d = unsafe { descriptor.read() };
    if !(2..=4096).contains(&d.width)
        || d.length > compiler_manifest::MAX_BYTES
        || (d.length != 0 && d.bytes.is_null())
        || d.alpha.is_null()
        || d.spaces.is_null()
    {
        return 22;
    }
    let mut alpha = [0; 256];
    let mut spaces = [0; 256];
    // SAFETY: initialized maps copied before parsing/publication.
    unsafe {
        std::ptr::copy_nonoverlapping(d.alpha, alpha.as_mut_ptr(), 256);
        std::ptr::copy_nonoverlapping(d.spaces, spaces.as_mut_ptr(), 256);
    }
    // SAFETY: initialized input loan only used before publishing any output.
    let bytes = if d.length == 0 {
        &[]
    } else {
        // SAFETY: non-null initialized length was admitted above.
        unsafe { std::slice::from_raw_parts(d.bytes, d.length) }
    };
    let Ok(mut workspace) = pool().try_lock() else {
        return 11;
    };
    let mode = if d.windows != 0 {
        TextMode::Windows
    } else {
        TextMode::Binary
    };
    workspace.prefix_policy(d.keep_prefix_x != 0);
    let status = match workspace.parse_bytes(bytes, d.width, mode, &alpha, &spaces) {
        Ok(s) => s,
        Err(e) => return compiler_driver::errno(&e),
    };
    if workspace.rows().len() > capacity {
        return 28;
    }
    // SAFETY: caller's admitted capacity covers initialized owned rows, disjoint
    // from retained storage. The input loan is dead before either output write.
    unsafe {
        if !workspace.rows().is_empty() {
            std::ptr::copy_nonoverlapping(workspace.rows().as_ptr(), rows, workspace.rows().len());
        }
        used.write(workspace.rows().len());
    }
    status
}
unsafe fn text(pointer: *const c_char, width: usize) -> Result<[u8; 4096], i32> {
    let mut out = [0; 4096];
    for (i, b) in out[..width].iter_mut().enumerate() {
        // SAFETY: caller initializes through NUL or the full width on rejection.
        *b = unsafe { *pointer.add(i) as u8 };
        if *b == 0 {
            return Ok(out);
        }
    }
    Err(compiler_driver::NAME_TOO_LONG)
}
/// Caller serializes the engine; descriptor/maps/strings are initialized and
/// snapshotted before effects. Optional paths are NULL. Callbacks cannot retain
/// loans, unwind, close compiler-owned resources or mutate the live context
/// except their documented projection. Admission reentry refuses immediately.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_phonemes(descriptor: *const Descriptor) -> c_int {
    if descriptor.is_null() {
        return 22;
    }
    // SAFETY: initialized aligned descriptor copied before any callback.
    let d = unsafe { descriptor.read() };
    if !(2..=4096).contains(&d.width)
        || d.home.is_null()
        || d.alpha.is_null()
        || d.spaces.is_null()
        || d.effect.is_none()
        || d.counts.is_none()
        || d.log.is_none()
        || d.error.is_none()
    {
        return 22;
    }
    // SAFETY: every non-null path satisfies the bounded text contract.
    let home = match unsafe { text(d.home, d.width) } {
        Ok(s) => s,
        Err(e) => return e,
    };
    // SAFETY: optional readable path copied before effects.
    let source = if d.source.is_null() {
        None
    } else {
        // SAFETY: non-null path initialized through NUL or full width.
        match unsafe { text(d.source, d.width) } {
            Ok(s) => Some(s),
            Err(e) => return e,
        }
    };
    // SAFETY: optional readable path copied before effects.
    let destination = if d.destination.is_null() {
        None
    } else {
        // SAFETY: non-null path initialized through NUL or full width.
        match unsafe { text(d.destination, d.width) } {
            Ok(s) => Some(s),
            Err(e) => return e,
        }
    };
    let mut alpha = [0; 256];
    let mut spaces = [0; 256];
    // SAFETY: initialized classifiers copied before any side effect.
    unsafe {
        std::ptr::copy_nonoverlapping(d.alpha, alpha.as_mut_ptr(), 256);
        std::ptr::copy_nonoverlapping(d.spaces, spaces.as_mut_ptr(), 256);
    }
    let paths = match Paths::new(
        compiler_driver::string(&home),
        source.as_ref().map(|s| compiler_driver::string(s)),
        destination.as_ref().map(|s| compiler_driver::string(s)),
        d.width,
        d.windows != 0,
    ) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let mode = if d.windows != 0 {
        TextMode::Windows
    } else {
        TextMode::Binary
    };
    let Ok(mut manifest) = pool().try_lock() else {
        return 11;
    };
    manifest.prefix_policy(d.keep_prefix_x != 0);
    compiler_driver::run(
        &mut Effects(d),
        &paths,
        &mut manifest,
        &alpha,
        &spaces,
        mode,
    )
}
