//! Owned contexts and serialized CRT adapters for the native diagnostics.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::status::{self, Context, Message};
use std::alloc::{alloc, Layout};
use std::ffi::{c_char, c_void, CStr};
use std::ptr;

#[repr(C)]
pub struct View {
    kind: i32,
    name: *mut c_char,
    version: i32,
    expected: i32,
}
#[repr(C)]
struct Owned {
    view: View,
    name: Vec<u8>,
}
type Errno = unsafe extern "C" fn(u32, *mut c_char, usize);
#[repr(C)]
pub struct Io {
    lock: unsafe extern "C" fn(*mut c_void),
    write: unsafe extern "C" fn(*mut c_void, *const u8, usize),
    unlock: unsafe extern "C" fn(*mut c_void),
    errno: Errno,
}
struct Locked<'a> {
    io: &'a Io,
    stream: *mut c_void,
}
impl Drop for Locked<'_> {
    fn drop(&mut self) {
        // SAFETY: matching live CRT stream/table, locked before this guard.
        unsafe {
            (self.io.unlock)(self.stream);
        }
    }
}
unsafe fn set(
    slot: *mut *mut View,
    code: u32,
    name: *const c_char,
    version: i32,
    expected: i32,
    kind: i32,
) -> u32 {
    if slot.is_null() {
        return code;
    }
    if name.is_null() {
        return 22;
    }
    // SAFETY: admitted live name through NUL. Copy before borrowing/changing
    // the context, including when name aliases its current owned name.
    let bytes = unsafe { CStr::from_ptr(name) }.to_bytes_with_nul();
    let mut owned = Vec::new();
    if owned.try_reserve_exact(bytes.len()).is_err() {
        return 12;
    }
    owned.extend_from_slice(bytes);
    // SAFETY: live unique context slot; null or an allocation created here.
    let current = unsafe { *slot }.cast::<Owned>();
    let current = if current.is_null() {
        // SAFETY: nonzero layout, checked global allocation; initialization
        // precedes publishing its prefix to C.
        let pointer = unsafe { alloc(Layout::new::<Owned>()) }.cast::<Owned>();
        if pointer.is_null() {
            return 12;
        }
        // SAFETY: newly allocated exclusive storage of the admitted layout.
        unsafe {
            pointer.write(Owned {
                view: View {
                    kind,
                    name: ptr::null_mut(),
                    version,
                    expected,
                },
                name: owned,
            });
        }
        pointer
    } else {
        // SAFETY: exclusive live owned context; input already copied and any
        // aliases are no longer borrowed. Atomic failure preserves old state.
        unsafe {
            (*current).name = owned;
            (*current).view = View {
                kind,
                name: ptr::null_mut(),
                version,
                expected,
            };
        }
        current
    };
    // SAFETY: fully initialized owner and stable heap name, published once.
    unsafe {
        (*current).view.name = (*current).name.as_mut_ptr().cast();
        *slot = current.cast();
    }
    code
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_error_file(
    slot: *mut *mut View,
    code: u32,
    name: *const c_char,
) -> u32 {
    // SAFETY: same unique slot and live name contract as set.
    unsafe { set(slot, code, name, 0, 0, 0) }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_error_version(
    slot: *mut *mut View,
    name: *const c_char,
    version: i32,
    expected: i32,
) -> u32 {
    // SAFETY: same unique slot and live name contract as set.
    unsafe { set(slot, status::VERSION_MISMATCH, name, version, expected, 1) }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_error_clear(slot: *mut *mut View) {
    if slot.is_null() {
        return;
    }
    // SAFETY: unique live slot, relinquished exactly once. Clear prefix before
    // reclaiming ownership; name/header are no longer borrowed by callers.
    unsafe {
        let owner = ptr::replace(slot, ptr::null_mut()).cast::<Owned>();
        if !owner.is_null() {
            drop(Box::from_raw(owner));
        }
    }
}
unsafe fn copy(output: *mut u8, capacity: usize, input: &[u8], pad: bool) {
    if capacity == 0 {
        return;
    }
    let count = input.len().min(capacity - 1);
    // SAFETY: admitted exclusive initialized-or-uninitialized output extent;
    // disjoint initialized input. Do not borrow or read undefined tails.
    unsafe {
        ptr::copy_nonoverlapping(input.as_ptr(), output, count);
        ptr::write_bytes(output.add(count), 0, if pad { capacity - count } else { 1 });
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_error_message(
    code: u32,
    output: *mut c_char,
    capacity: usize,
    errno: Option<Errno>,
) {
    if output.is_null() || capacity > isize::MAX as usize {
        return;
    }
    match status::message(code) {
        Message::Builtin(bytes) => {
            // SAFETY: caller's output extent, static initialized source.
            unsafe {
                copy(output.cast(), capacity, bytes, true);
            }
        }
        Message::Other(code) => {
            let mut bytes = [0u8; 32];
            let mut length = 0;
            status::other(code, |part| {
                bytes[length..length + part.len()].copy_from_slice(part);
                length += part.len();
            });
            // SAFETY: live caller output and disjoint bounded stack message.
            unsafe {
                copy(output.cast(), capacity, &bytes[..length], false);
            }
        }
        Message::Errno(code) => {
            if let Some(errno) = errno {
                // SAFETY: caller supplies the CRT callback and admitted buffer.
                // Match that CRT's strerror_r/s behavior, including zero length.
                unsafe {
                    errno(code, output, capacity);
                }
            }
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_error_print(
    code: u32,
    stream: *mut c_void,
    context: *const View,
    io: *const Io,
) {
    if stream.is_null() || io.is_null() {
        return;
    }
    // SAFETY: immutable admitted function table, live for the complete call.
    let io = unsafe { &*io };
    let mut message = [0u8; 512];
    // SAFETY: initialized bounded local buffer and admitted CRT errno adapter.
    unsafe {
        espeak_rs_error_message(
            code,
            message.as_mut_ptr().cast(),
            message.len(),
            Some(io.errno),
        );
    }
    let end = message
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(message.len());
    let context = if context.is_null() {
        None
    } else {
        // SAFETY: live C prefix with name through NUL. Printing only borrows;
        // caller keeps this context/name unchanged until the stream returns.
        let context = unsafe { &*context };
        let name = if context.name.is_null() {
            &[][..]
        } else {
            // SAFETY: name is a live terminated byte string for this call.
            unsafe { CStr::from_ptr(context.name) }.to_bytes()
        };
        Some(match context.kind {
            0 => Context::File(name),
            1 => Context::Version {
                name,
                expected: context.expected,
                actual: context.version,
            },
            _ => Context::Unknown,
        })
    };
    // SAFETY: caller's live CRT stream; serialized for every emitted fragment.
    unsafe {
        (io.lock)(stream);
    }
    let _locked = Locked { io, stream };
    status::diagnostic(context, &message[..end], |part| {
        // SAFETY: admitted locked stream; initialized fragment is borrowed
        // only for the callback. No context or formatting state is mutable.
        unsafe {
            (io.write)(stream, part.as_ptr(), part.len());
        }
    });
}
#[no_mangle]
extern "C" fn espeak_rs_legacy_status(code: u32) -> i32 {
    status::legacy(code)
}
