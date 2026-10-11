//! Registered retained compiler resource owners and copied C projections.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    compiler_resource::{Host, Progress, Request, Workspace},
    spect_file::UNSUPPORTED,
    wavegen::Wavegen,
};
use std::{
    ffi::{c_char, c_void},
    sync::{Mutex, OnceLock},
};
type Log = unsafe extern "C" fn(*mut c_void, *const u8, usize);
type Error = unsafe extern "C" fn(*mut c_void, i32, i32, i32, *const u8, usize);
type Position = unsafe extern "C" fn(*mut c_void) -> i64;
type Write = unsafe extern "C" fn(*mut c_void, i32, *const u8, usize) -> i32;
#[repr(C)]
pub struct Descriptor {
    owner: u64,
    context: *mut c_void,
    source: *const c_char,
    name: *const c_char,
    generator: *mut Wavegen,
    lower: *const i32,
    space: *const u8,
    width: usize,
    rate: i32,
    control: i32,
    mnemonic: i32,
    table: i32,
    decimal: i32,
    initial: Progress,
    log: Option<Log>,
    error: Option<Error>,
    position: Option<Position>,
    write: Option<Write>,
}
struct Slot {
    id: u64,
    workspace: Workspace,
}
struct Pool {
    slots: [Slot; 4],
    next: u64,
}
fn pool() -> &'static Mutex<Pool> {
    static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();
    POOL.get_or_init(|| {
        Mutex::new(Pool {
            slots: std::array::from_fn(|_| Slot {
                id: 0,
                workspace: Workspace::default(),
            }),
            next: 1,
        })
    })
}
#[no_mangle]
pub extern "C" fn espeak_rs_compiler_resources_begin() -> u64 {
    let Ok(mut pool) = pool().try_lock() else {
        return 0;
    };
    let Some(index) = pool.slots.iter().position(|slot| slot.id == 0) else {
        return 0;
    };
    let id = pool.next;
    let Some(next) = id.checked_add(1) else {
        return 0;
    };
    pool.next = next;
    pool.slots[index].workspace.clear();
    pool.slots[index].id = id;
    id
}
#[no_mangle]
pub extern "C" fn espeak_rs_compiler_resources_end(owner: u64) -> i32 {
    if owner == 0 {
        return 0;
    }
    let Ok(mut pool) = pool().try_lock() else {
        return UNSUPPORTED;
    };
    let Some(slot) = pool.slots.iter_mut().find(|slot| slot.id == owner) else {
        return UNSUPPORTED;
    };
    slot.workspace.clear();
    slot.id = 0;
    0
}
struct Effects {
    d: Descriptor,
}
impl Host for Effects {
    fn log(&mut self, bytes: &[u8]) {
        // SAFETY: initialized owned synchronous loan, no retention or unwind.
        unsafe {
            self.d.log.unwrap()(self.d.context, bytes.as_ptr(), bytes.len());
        }
    }
    fn error(&mut self, code: i32, a: i32, b: i32, name: &[u8]) {
        // SAFETY: bounded owned synchronous diagnostic projection.
        unsafe {
            self.d.error.unwrap()(self.d.context, code, a, b, name.as_ptr(), name.len());
        }
    }
    fn position(&mut self) -> Result<i64, i32> {
        // SAFETY: live caller stream, synchronous primitive.
        let n = unsafe { self.d.position.unwrap()(self.d.context) };
        if n < 0 {
            Err(5)
        } else {
            Ok(n)
        }
    }
    fn generator(&mut self) -> &mut Wavegen {
        // SAFETY: live exclusive initialized Rust generator; loan exists only
        // during synchronous packing after reads and before callbacks.
        unsafe { &mut *self.d.generator }
    }
    fn write(&mut self, destination: i32, bytes: &[u8]) -> Result<(), i32> {
        // SAFETY: initialized retained payload/manifest, no retained loan.
        let status = unsafe {
            self.d.write.unwrap()(self.d.context, destination, bytes.as_ptr(), bytes.len())
        };
        if status == 0 {
            Ok(())
        } else {
            Err(status)
        }
    }
}
unsafe fn snapshot(pointer: *const c_char) -> Option<[u8; 4096]> {
    if pointer.is_null() {
        return None;
    }
    let mut bytes = [0; 4096];
    for (i, c) in bytes.iter_mut().enumerate() {
        // SAFETY: initialized through NUL or all 4096 on rejection.
        *c = unsafe { *pointer.add(i) as u8 };
        if *c == 0 {
            return Some(bytes);
        }
    }
    None
}
fn text(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|c| *c == 0).unwrap_or(bytes.len())]
}
#[cfg(target_pointer_width = "64")]
const _: () =
    assert!(std::mem::size_of::<Descriptor>() == 136 && std::mem::size_of::<Progress>() == 16);
/// Initialized terminated strings, 256 initialized locale entries, live
/// serialized generator/effects and disjoint output. No foreign loan crosses
/// I/O or callbacks; output publication follows every effect.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_resource_load(
    descriptor: *const Descriptor,
    out: *mut Progress,
) -> i32 {
    if descriptor.is_null() || out.is_null() {
        return UNSUPPORTED;
    }
    // SAFETY: copy initialized descriptor before I/O or callbacks.
    let d = unsafe { descriptor.read() };
    if d.owner == 0
        || !(2..=4096).contains(&d.width)
        || d.generator.is_null()
        || d.lower.is_null()
        || d.space.is_null()
        || !(1..=255).contains(&d.decimal)
        || d.log.is_none()
        || d.error.is_none()
        || d.position.is_none()
        || d.write.is_none()
    {
        return UNSUPPORTED;
    }
    // SAFETY: copy initialized bounded paths/maps before any effect.
    let (Some(source), Some(name)) = (unsafe { snapshot(d.source) }, unsafe { snapshot(d.name) })
    else {
        return UNSUPPORTED;
    };
    // SAFETY: admitted aligned initialized 256-element map copied before effects.
    let lower = unsafe { *(d.lower.cast::<[i32; 256]>()) };
    // SAFETY: admitted initialized 256-byte map copied before effects.
    let space = unsafe { *(d.space.cast::<[u8; 256]>()) };
    let Ok(mut pool) = pool().try_lock() else {
        return UNSUPPORTED;
    };
    let Some(slot) = pool.slots.iter_mut().find(|slot| slot.id == d.owner) else {
        return UNSUPPORTED;
    };
    let request = Request {
        source: text(&source),
        name: text(&name),
        width: d.width,
        rate: d.rate,
        control: d.control,
        initial: d.initial,
        mnemonic: d.mnemonic,
        table: d.table,
        lower: &lower,
        space: &space,
        decimal: d.decimal as u8,
    };
    let (status, result) = slot.workspace.load(&mut Effects { d }, &request);
    // SAFETY: output is live/disjoint; terminal publication follows callbacks.
    unsafe {
        out.write(result);
    }
    status
}
