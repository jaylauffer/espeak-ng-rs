//! Thread-confined registered compiler outputs and copied compatibility loans.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::compiler_output::{Host, Request, Workspace, MAX_WRITE};
use std::{
    cell::RefCell,
    ffi::{c_char, c_void},
    sync::atomic::{AtomicU64, Ordering},
};
type Fallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
#[repr(C)]
pub struct Descriptor {
    source: *const c_char,
    destination: *const c_char,
    width: usize,
    version: u32,
    rate: i32,
    context: *mut c_void,
    fallback: Option<Fallback>,
}
#[repr(C)]
pub struct Outcome {
    owner: u64,
    mask: u32,
    status: i32,
    filename: [u8; 4096],
}
struct Slot {
    id: u64,
    workspace: Workspace,
    context: *mut c_void,
    fallback: Option<Fallback>,
}
struct Pool {
    slots: [Slot; 4],
}
thread_local! {
    static POOL:RefCell<Pool>=RefCell::new(Pool{slots:std::array::from_fn(|_|Slot{id:0,workspace:Workspace::default(),context:std::ptr::null_mut(),fallback:None})});
}
struct Effects {
    context: *mut c_void,
    fallback: Fallback,
}
impl Host for Effects {
    fn stderr(&mut self, bytes: &[u8]) -> Result<(), i32> {
        // SAFETY: synchronous initialized owned loan; owner retains the live
        // callback context through end, and effects cannot retain or unwind.
        let code = unsafe { (self.fallback)(self.context, bytes.as_ptr(), bytes.len()) };
        if code == 0 {
            Ok(())
        } else {
            Err(code)
        }
    }
}
unsafe fn snapshot(pointer: *const c_char) -> Option<[u8; 4096]> {
    if pointer.is_null() {
        return None;
    }
    let mut out = [0; 4096];
    for (i, c) in out.iter_mut().enumerate() {
        // SAFETY: input initialized through NUL or all 4096 on rejection.
        *c = unsafe { *pointer.add(i) as u8 };
        if *c == 0 {
            return Some(out);
        }
    }
    None
}
fn text(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|c| *c == 0).unwrap_or(bytes.len())]
}
#[cfg(target_pointer_width = "64")]
const _: () =
    assert!(std::mem::size_of::<Descriptor>() == 48 && std::mem::size_of::<Outcome>() == 4112);
/// Initialized bounded strings/descriptor, disjoint live output and a fallback
/// context live until end on this thread. Copy admission before effects; publish
/// result after creation/header effects, including initialized error filenames.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_output_begin(
    descriptor: *const Descriptor,
    out: *mut Outcome,
) -> i32 {
    if descriptor.is_null() || out.is_null() {
        return 22;
    }
    // SAFETY: initialized descriptor copied before effects.
    let d = unsafe { descriptor.read() };
    if !(2..=4096).contains(&d.width) || d.fallback.is_none() {
        return 22;
    }
    // SAFETY: bounded terminated path inputs copied before effects.
    let (Some(source), Some(destination)) = (unsafe { snapshot(d.source) }, unsafe {
        snapshot(d.destination)
    }) else {
        return 22;
    };
    POOL.with(|pool| {
        let Ok(mut pool) = pool.try_borrow_mut() else {
            return 11;
        };
        let Some(slot) = pool.slots.iter_mut().find(|slot| slot.id == 0) else {
            return 11;
        };
        static NEXT: AtomicU64 = AtomicU64::new(1);
        // The replacement try_update requires Rust 1.95; retain the 1.85 MSRV.
        #[allow(deprecated)]
        let Ok(owner) =
            NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        else {
            return 12;
        };
        let request = Request {
            source: text(&source),
            destination: text(&destination),
            width: d.width,
            version: d.version,
            rate: d.rate,
            windows_text: cfg!(windows),
        };
        let status = slot
            .workspace
            .begin(
                &mut Effects {
                    context: d.context,
                    fallback: d.fallback.unwrap(),
                },
                &request,
            )
            .err()
            .unwrap_or(0);
        let mut result = Outcome {
            owner: 0,
            mask: 0,
            status,
            filename: [0; 4096],
        };
        let name = slot.workspace.error_name();
        result.filename[..name.len()].copy_from_slice(name);
        if status == 0 {
            slot.id = owner;
            slot.context = d.context;
            slot.fallback = d.fallback;
            result.owner = owner;
            result.mask = slot.workspace.mask();
        }
        // SAFETY: disjoint live caller output; no reference crosses effects.
        unsafe {
            out.write(result);
        }
        status
    })
}
/// Initialized disjoint bytes are snapshotted into retained Rust storage before
/// any proactor wait or callback. No borrowed foreign slice spans those effects.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_output_write(
    owner: u64,
    file: u32,
    bytes: *const u8,
    length: usize,
) -> i32 {
    if owner == 0 || file >= 5 || length > MAX_WRITE || (length != 0 && bytes.is_null()) {
        return 22;
    }
    POOL.with(|pool| {
        let Ok(mut pool) = pool.try_borrow_mut() else {
            return 11;
        };
        let Some(slot) = pool.slots.iter_mut().find(|slot| slot.id == owner) else {
            return 9;
        };
        slot.workspace.input.clear();
        if slot.workspace.input.try_reserve_exact(length).is_err() {
            return slot.workspace.remember(12, file as usize);
        }
        if length != 0 {
            // SAFETY: admitted initialized source range; copy ends before effects
            // into initialized disjoint retained storage with checked capacity.
            let input = unsafe { std::slice::from_raw_parts(bytes, length) };
            slot.workspace.input.extend_from_slice(input);
        }
        let mut effects = Effects {
            context: slot.context,
            fallback: slot.fallback.unwrap(),
        };
        slot.workspace
            .write_snapshot(&mut effects, file as usize)
            .err()
            .unwrap_or(0)
    })
}
#[no_mangle]
pub extern "C" fn espeak_rs_compiler_output_position(owner: u64, file: u32) -> i64 {
    POOL.with(|pool| {
        let Ok(pool) = pool.try_borrow() else {
            return -1;
        };
        let Some(slot) = pool
            .slots
            .iter()
            .find(|slot| owner != 0 && slot.id == owner)
        else {
            return -1;
        };
        slot.workspace
            .position(file as usize)
            .map_or(-1, |n| n as i64)
    })
}
#[no_mangle]
pub extern "C" fn espeak_rs_compiler_output_flush(owner: u64, file: u32) -> i32 {
    POOL.with(|pool| {
        let Ok(mut pool) = pool.try_borrow_mut() else {
            return 11;
        };
        let Some(slot) = pool
            .slots
            .iter_mut()
            .find(|slot| owner != 0 && slot.id == owner)
        else {
            return 9;
        };
        slot.workspace.flush(file as usize).err().unwrap_or(0)
    })
}
/// Complete all writes before removing the owner; repeated/stale end rejects.
#[no_mangle]
pub extern "C" fn espeak_rs_compiler_output_end(owner: u64) -> i32 {
    if owner == 0 {
        return 0;
    }
    POOL.with(|pool| {
        let Ok(mut pool) = pool.try_borrow_mut() else {
            return 11;
        };
        let Some(slot) = pool.slots.iter_mut().find(|slot| slot.id == owner) else {
            return 9;
        };
        let status = slot.workspace.finish();
        slot.id = 0;
        slot.context = std::ptr::null_mut();
        slot.fallback = None;
        status
    })
}
