//! Bounded registered assembler leases; foreign inputs copied before effects.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::compiler_program::{Host, Program, Token};
use std::{
    ffi::c_void,
    sync::{Mutex, OnceLock},
};
type Next = unsafe extern "C" fn(*mut c_void, i32, *mut Token) -> i32;
type Unget = unsafe extern "C" fn(*mut c_void);
type Error = unsafe extern "C" fn(*mut c_void, *const u8, usize);
type Lookup = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
type Load = unsafe extern "C" fn(*mut c_void, *const u8, usize, i32) -> i32;
type Position = unsafe extern "C" fn(*mut c_void) -> i64;
type Write = unsafe extern "C" fn(*mut c_void, i32, *const u8, usize) -> i32;
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Descriptor {
    owner: u64,
    context: *mut c_void,
    words: *const u16,
    length: usize,
    control: i32,
    kind: i32,
    next: Option<Next>,
    unget: Option<Unget>,
    error: Option<Error>,
    lookup: Option<Lookup>,
    load: Option<Load>,
    position: Option<Position>,
    write: Option<Write>,
}
#[repr(C)]
pub struct Outcome {
    count: i32,
    after_if: i32,
    level: i32,
    then_count: i32,
    returned: i32,
    value: i32,
    start: i32,
    end: i32,
    status: i32,
}
#[derive(Default)]
struct Slot {
    id: u64,
    program: Program,
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
pub extern "C" fn espeak_rs_compiler_program_begin() -> u64 {
    let Ok(mut pool) = pool().try_lock() else {
        return 0;
    };
    let Some(index) = pool.slots.iter().position(|s| s.id == 0) else {
        return 0;
    };
    let id = pool.next;
    let Some(next) = id.checked_add(1) else {
        return 0;
    };
    if pool.slots[index].program.reset().is_err() {
        return 0;
    }
    pool.slots[index].id = id;
    pool.next = next;
    id
}
#[no_mangle]
pub extern "C" fn espeak_rs_compiler_program_end(owner: u64) -> i32 {
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
    slot.program.clear_words();
    0
}
struct Effects {
    d: Descriptor,
}
impl Host for Effects {
    fn next(&mut self, kind: i32) -> Result<Token, i32> {
        let mut token = Token::default();
        // SAFETY: synchronous callback fills initialized bounded Rust scratch,
        // with admitted live context; caller cannot retain or unwind.
        let status = unsafe { (self.d.next.unwrap())(self.d.context, kind, &mut token) };
        if status != 0 {
            return Err(status);
        }
        if !token.text.contains(&0) {
            return Err(22);
        }
        Ok(token)
    }
    fn unget(&mut self) {
        // SAFETY: admitted synchronous primitive with live opaque context.
        unsafe {
            (self.d.unget.unwrap())(self.d.context);
        }
    }
    fn error(&mut self, bytes: &[u8]) {
        // SAFETY: bounded owned diagnostic loan, synchronous and not retained.
        unsafe {
            (self.d.error.unwrap())(self.d.context, bytes.as_ptr(), bytes.len());
        }
    }
    fn lookup(&mut self, bytes: &[u8]) -> i32 {
        // SAFETY: copied token bytes loaned synchronously to admitted lookup.
        unsafe { (self.d.lookup.unwrap())(self.d.context, bytes.as_ptr(), bytes.len()) }
    }
    fn load(&mut self, bytes: &[u8], control: i32) -> i32 {
        // SAFETY: copied bounded path, live context, no foreign output loans.
        unsafe { (self.d.load.unwrap())(self.d.context, bytes.as_ptr(), bytes.len(), control) }
    }
    fn position(&mut self) -> Result<i64, i32> {
        // SAFETY: admitted synchronous position primitive with live context.
        let value = unsafe { (self.d.position.unwrap())(self.d.context) };
        if value < 0 {
            Err(5)
        } else {
            Ok(value)
        }
    }
    fn write(&mut self, log: bool, bytes: &[u8]) -> Result<(), i32> {
        // SAFETY: retained Rust payload loaned synchronously; output owner
        // copies it before I/O. No callback retention or unwind permitted.
        let status = unsafe {
            (self.d.write.unwrap())(self.d.context, i32::from(log), bytes.as_ptr(), bytes.len())
        };
        if status == 0 {
            Ok(())
        } else {
            Err(status)
        }
    }
}
/// Serialized ABI: inputs readable for their declared extents, result writable,
/// callbacks/context live and synchronous. Input/output may overlap; result is
/// published only after all effects. No buffer view escapes the owner.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_program_call(
    descriptor: *const Descriptor,
    operation: u32,
    result: *mut Outcome,
) -> i32 {
    if descriptor.is_null() || result.is_null() || operation > 14 {
        return 22;
    }
    // SAFETY: caller provides an initialized descriptor; copied before effects.
    let d = unsafe { descriptor.read() };
    if d.error.is_none()
        || (matches!(operation, 2 | 3 | 5 | 8 | 9)
            && (d.next.is_none() || d.unget.is_none() || d.lookup.is_none() || d.load.is_none()))
        || (operation == 14 && (d.position.is_none() || d.write.is_none()))
    {
        return 22;
    }
    let mut input = [0u16; 256];
    if operation == 1 {
        if d.length > input.len() || (d.length != 0 && d.words.is_null()) {
            return 22;
        }
        if d.length != 0 {
            // SAFETY: admitted readable input extent; raw copy avoids a foreign
            // reference spanning callbacks or result publication.
            unsafe {
                std::ptr::copy_nonoverlapping(d.words, input.as_mut_ptr(), d.length);
            }
        }
    }
    let Ok(mut pool) = pool().try_lock() else {
        return 11;
    };
    let Some(slot) = pool
        .slots
        .iter_mut()
        .find(|s| s.id == d.owner && d.owner != 0)
    else {
        return 9;
    };
    let p = &mut slot.program;
    let mut h = Effects { d };
    let (mut value, mut start, mut end) = (0, 0, 0);
    let status = match operation {
        0 => p.reset(),
        1 => p.emit(&input[..d.length], &mut h),
        2 => p.transition(d.control, d.kind, &mut h),
        3 => p.condition(d.control != 0, &mut h).map(|v| value = v),
        4 => p.otherwise(&mut h),
        5 => p.elif(&mut h),
        6 => p.endif(&mut h),
        7 => p.switch(d.control, &mut h),
        8 => p.tone(&mut h).map(|(a, b)| {
            start = i32::from(a);
            end = i32::from(b);
        }),
        9 => p.sound(d.control, d.kind, &mut h),
        10 => {
            p.decrement();
            Ok(())
        }
        11 => {
            p.mark_returned();
            Ok(())
        }
        12 => {
            p.clear_words();
            Ok(())
        }
        13 => p.finish(&mut h),
        14 => p
            .publish(d.control != 0, &mut h)
            .map(|v| value = v.unwrap_or(-1)),
        _ => unreachable!(),
    }
    .err()
    .unwrap_or(0);
    let out = Outcome {
        count: p.words().len() as i32,
        after_if: i32::from(p.after_if()),
        level: p.level() as i32,
        then_count: p.then_count(),
        returned: i32::from(p.returned()),
        value,
        start,
        end,
        status,
    };
    // SAFETY: writable result supplied by serialized caller; foreign inputs have
    // been copied, callbacks completed, and no foreign references remain.
    unsafe {
        result.write(out);
    }
    status
}
/// Copies a current program for diagnostic/parity consumers; no retained view.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_program_copy(
    owner: u64,
    output: *mut u16,
    capacity: usize,
) -> i32 {
    let Ok(pool) = pool().try_lock() else {
        return -11;
    };
    let Some(slot) = pool.slots.iter().find(|s| s.id == owner && owner != 0) else {
        return -9;
    };
    let words = slot.program.words();
    if words.len() > capacity || (output.is_null() && !words.is_empty()) {
        return -22;
    }
    if !words.is_empty() {
        // SAFETY: caller provides writable declared extent, disjoint from the
        // private Rust owner. No callbacks or buffer views cross this copy.
        unsafe {
            std::ptr::copy_nonoverlapping(words.as_ptr(), output, words.len());
        }
    }
    words.len() as i32
}
