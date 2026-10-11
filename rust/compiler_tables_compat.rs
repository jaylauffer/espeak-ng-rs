//! Retained registered table arenas with quiescent, separate C row views.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    compiler_tables::{text, Host, Workspace, ROWS, TABLES},
    phoneme::Phoneme,
};
use std::{
    ffi::{c_char, c_void},
    sync::{Mutex, OnceLock},
};
type Base = unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> i32;
type Error = unsafe extern "C" fn(*mut c_void, i32, i32, *const u8, usize);
type Write = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
#[repr(C)]
pub struct Descriptor {
    owner: u64,
    context: *mut c_void,
    name: *const c_char,
    target: *mut Phoneme,
    line: i32,
    control: i32,
    base: Option<Base>,
    error: Option<Error>,
    write: Option<Write>,
}
#[repr(C)]
pub struct Outcome {
    current: *mut Phoneme,
    found: *mut Phoneme,
    tables: i32,
    count: i32,
    value: i32,
    status: i32,
}
#[derive(Default)]
struct Slot {
    id: u64,
    workspace: Workspace,
    views: Vec<Phoneme>,
}
struct Pool {
    slots: [Slot; 4],
    next: u64,
}
fn pool() -> &'static Mutex<Pool> {
    static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();
    POOL.get_or_init(|| {
        Mutex::new(Pool {
            slots: std::array::from_fn(|_| Slot::default()),
            next: 1,
        })
    })
}
#[no_mangle]
pub extern "C" fn espeak_rs_compiler_tables_begin() -> u64 {
    let Ok(mut pool) = pool().try_lock() else {
        return 0;
    };
    let Some(i) = pool.slots.iter().position(|s| s.id == 0) else {
        return 0;
    };
    let id = pool.next;
    let Some(next) = id.checked_add(1) else {
        return 0;
    };
    let slot = &mut pool.slots[i];
    slot.workspace.clear();
    if slot
        .views
        .try_reserve_exact(TABLES * ROWS - slot.views.len())
        .is_err()
    {
        return 0;
    }
    slot.views.resize(TABLES * ROWS, Phoneme::default());
    slot.id = id;
    pool.next = next;
    id
}
#[no_mangle]
pub extern "C" fn espeak_rs_compiler_tables_end(owner: u64) -> i32 {
    if owner == 0 {
        return 0;
    }
    let Ok(mut pool) = pool().try_lock() else {
        return 11;
    };
    let Some(slot) = pool.slots.iter_mut().find(|s| s.id == owner) else {
        return 9;
    };
    slot.id = 0;
    slot.workspace.clear();
    0
}
struct Effects {
    d: Descriptor,
}
impl Host for Effects {
    fn base(&mut self) -> Result<[u8; 256], i32> {
        let mut out = [0; 256];
        // SAFETY: bounded initialized Rust output scratch, synchronous callback
        // with live context; no other input/output foreign loan spans effects.
        let code = unsafe { (self.d.base.unwrap())(self.d.context, out.as_mut_ptr(), out.len()) };
        if code != 0 {
            return Err(code);
        }
        if !out.contains(&0) {
            return Err(22);
        }
        Ok(out)
    }
    fn error(&mut self, code: i32, a: i32, _b: i32, name: &[u8]) {
        // SAFETY: synchronous initialized owned diagnostic bytes; no retention
        // or unwind, with the admitted live callback context.
        unsafe {
            (self.d.error.unwrap())(self.d.context, code, a, name.as_ptr(), name.len());
        }
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), i32> {
        // SAFETY: synchronous initialized owned packed payload, bounded before
        // callback; callback snapshots before its own I/O and cannot retain it.
        let code = unsafe { (self.d.write.unwrap())(self.d.context, bytes.as_ptr(), bytes.len()) };
        if code == 0 {
            Ok(())
        } else {
            Err(code)
        }
    }
}
unsafe fn snapshot(pointer: *const c_char) -> Option<[u8; 256]> {
    if pointer.is_null() {
        return None;
    }
    let mut out = [0; 256];
    for (i, b) in out.iter_mut().enumerate() {
        // SAFETY: input is initialized through NUL, or all 256 on rejection.
        *b = unsafe { *pointer.add(i) as u8 };
        if *b == 0 {
            return Some(out);
        }
    }
    None
}
#[cfg(target_pointer_width = "64")]
const _: () =
    assert!(std::mem::size_of::<Descriptor>() == 64 && std::mem::size_of::<Outcome>() == 32);
/// Serialized live exclusive lease. Only current rows can be modified between
/// calls; older returned rows are read-only. All views are quiescent during a
/// call, including callbacks, and expire at end. C views and safe Rust rows are
/// separate arenas; snapshot current before effects, commit after effects.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_tables_call(
    d: *const Descriptor,
    operation: u32,
    out: *mut Outcome,
) -> i32 {
    if d.is_null() || out.is_null() || operation > 6 {
        return 22;
    }
    // SAFETY: initialized aligned descriptor, copied before callbacks.
    let d = unsafe { d.read() };
    if d.owner == 0
        || d.error.is_none()
        || (operation == 0 && d.base.is_none())
        || (operation == 5 && d.write.is_none())
        || (operation == 3 && d.target.is_null())
    {
        return 22;
    }
    let mut name = [0; 256];
    if operation <= 3 {
        // SAFETY: admitted bounded terminated input copied before effects.
        let Some(copy) = (unsafe { snapshot(d.name) }) else {
            return 22;
        };
        name = copy;
    }
    let target = if operation == 3 {
        // SAFETY: live aligned initialized 16-byte target; copied before effects.
        unsafe { d.target.read() }
    } else {
        Phoneme::default()
    };
    let Ok(mut pool) = pool().try_lock() else {
        return 11;
    };
    let Some(slot) = pool.slots.iter_mut().find(|s| s.id == d.owner) else {
        return 9;
    };
    if let Some(i) = slot.workspace.tables().len().checked_sub(1) {
        let src = slot.views.as_ptr();
        let rows = slot.workspace.rows_mut(i).unwrap();
        // SAFETY: retained disjoint initialized view arena, current 256 rows
        // quiescent under the exclusive lease; snapshot ends before effects.
        unsafe {
            std::ptr::copy_nonoverlapping(src.add(i * ROWS), rows.as_mut_ptr(), ROWS);
        }
    }
    let mut effects = Effects { d };
    let mut found = None;
    let mut value = 0;
    let mut imported = None;
    let line = effects.d.line;
    let control = effects.d.control;
    let result = match operation {
        0 => slot.workspace.start(&mut effects, text(&name)),
        1 => slot
            .workspace
            .lookup(&mut effects, text(&name), control, line)
            .map(|n| {
                value = n;
            }),
        2 => slot
            .workspace
            .find(&mut effects, text(&name), line)
            .map(|n| {
                found = n;
            }),
        3 => slot
            .workspace
            .import(&mut effects, text(&name), target, line)
            .map(|row| {
                imported = Some(row);
            }),
        4 => slot.workspace.finish_table(&mut effects),
        5 => slot.workspace.pack(&mut effects),
        6 => slot.workspace.terminate_unused(),
        _ => unreachable!(),
    };
    let status = result.err().unwrap_or(0);
    let tables = slot.workspace.tables().len();
    let first = if operation == 5 {
        0
    } else {
        tables.saturating_sub(1)
    };
    for i in first..tables {
        let rows = slot.workspace.rows(i).unwrap();
        let dest = slot.views.as_mut_ptr();
        // SAFETY: disjoint initialized retained arenas; all callbacks finished,
        // no foreign view loan is live. Publish only checked table extents.
        unsafe {
            std::ptr::copy_nonoverlapping(rows.as_ptr(), dest.add(i * ROWS), ROWS);
        }
    }
    let current = if tables == 0 {
        std::ptr::null_mut()
    } else {
        // SAFETY: checked initialized current table in stable retained view arena.
        unsafe { slot.views.as_mut_ptr().add((tables - 1) * ROWS) }
    };
    let found = found.map_or(std::ptr::null_mut(), |(table, code)| {
        // SAFETY: owned find returned checked table and row within its arena.
        unsafe { slot.views.as_mut_ptr().add(table * ROWS + code) }
    });
    if let Some(row) = imported {
        // SAFETY: admitted live target, publication after every callback. Exact
        // source/target alias works because the source was copied before effects.
        unsafe {
            effects.d.target.write(row);
        }
    }
    // SAFETY: disjoint live aligned caller result, all effects complete.
    unsafe {
        out.write(Outcome {
            current,
            found,
            tables: tables as i32,
            count: slot.workspace.count() as i32,
            value,
            status,
        });
    }
    status
}
