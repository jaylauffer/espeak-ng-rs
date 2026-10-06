//! Copied, serialized main clause compatibility bridge.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{clause_engine as engine, clause_input};
use std::{ffi::c_void, ptr};
#[derive(Clone, Copy)]
#[repr(C)]
struct Context {
    owner: *mut c_void,
    eof: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    read: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    peek: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    classify: Option<unsafe extern "C" fn(i32, u32) -> i32>,
    replace: Option<unsafe extern "C" fn(*mut c_void, *mut i32) -> i32>,
    effect: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *mut engine::State,
            *mut engine::Command,
            *mut u8,
            usize,
        ) -> i32,
    >,
}
struct RawOutput {
    bytes: *mut u8,
    indexes: *mut i16,
    capacity: usize,
    index_capacity: usize,
    initialized: usize,
}
impl engine::Output for RawOutput {
    fn capacity(&self) -> usize {
        self.capacity
    }
    fn initialized(&self) -> &[u8] {
        // SAFETY: frontier admits only a contiguous prefix actually initialized
        // by native sparse writes or a successful retained backend effect.
        unsafe { std::slice::from_raw_parts(self.bytes, self.initialized) }
    }
    fn write(&mut self, position: usize, bytes: &[u8]) -> Result<(), engine::Error> {
        if position > self.initialized {
            return Err(engine::Error::Uninitialized);
        }
        let end = position
            .checked_add(bytes.len())
            .filter(|end| *end <= self.capacity)
            .ok_or(engine::Error::Capacity)?;
        // SAFETY: admitted exclusive writable footprint, disjoint copied source;
        // unused foreign output bytes are neither borrowed nor read.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.bytes.add(position), bytes.len());
        }
        self.initialized = self.initialized.max(end);
        Ok(())
    }
    fn character_index(&mut self, position: usize, value: i16) -> Result<(), engine::Error> {
        if position >= self.index_capacity {
            return Err(engine::Error::Capacity);
        }
        // SAFETY: admitted exclusive writable index scalar, no tail read/borrow.
        unsafe {
            *self.indexes.add(position) = value;
        }
        Ok(())
    }
}
struct Host {
    context: Context,
    output: *mut u8,
    capacity: usize,
    frontier: usize,
}
impl engine::Host for Host {
    fn source_eof(&self) -> bool {
        // SAFETY: retained serialized source owner, pure initialized predicate.
        unsafe { self.context.eof.expect("admitted EOF")(self.context.owner) != 0 }
    }
    fn source_read(&mut self) -> u32 {
        // SAFETY: retained serialized decoder advances without retaining borrows.
        unsafe { self.context.read.expect("admitted read")(self.context.owner) }
    }
    fn source_peek(&mut self) -> u32 {
        // SAFETY: retained serialized decoder may select its AUTO fallback.
        unsafe { self.context.peek.expect("admitted peek")(self.context.owner) }
    }
    fn classify(&self, code: i32, class: engine::Class) -> bool {
        // SAFETY: pure stable platform/locale classifier, no owner/output borrow.
        unsafe { self.context.classify.expect("admitted classify")(code, class as u32) != 0 }
    }
    fn replace(&self, code: i32) -> Result<clause_input::Replacement, engine::Error> {
        let mut value = code;
        // SAFETY: pure immutable replacement table, exclusive copied scalar.
        let ignored = unsafe {
            self.context.replace.expect("admitted replacement")(self.context.owner, &mut value)
        };
        if !matches!(ignored, 0 | 1) {
            return Err(engine::Error::Backend);
        }
        Ok(clause_input::Replacement {
            code: value,
            ignore: ignored != 0,
        })
    }
    fn effect(
        &mut self,
        state: &mut engine::State,
        output: &mut dyn engine::Output,
        command: &mut engine::Command,
    ) -> Result<(), engine::Error> {
        self.frontier = output.initialized().len();
        // SAFETY: initialized exclusive local state/command copies; separate C
        // owner publishes scalar effects and refreshes after callbacks. Callback
        // cannot invalidate/reenter/retain state/command/output. Prefix through
        // the incoming index is initialized; unused writable capacity may not be.
        let status = unsafe {
            self.context.effect.expect("admitted effect")(
                self.context.owner,
                state,
                command,
                self.output,
                self.capacity,
            )
        };
        if status != 0 {
            return Err(engine::Error::Backend);
        }
        // This trait object's implementation is RawOutput in this bridge. The
        // callback admits and initializes its complete returned logical prefix.
        // Only that prefix is read; backend NUL or untouched tails are not guessed.
        let length = usize::try_from(command.index).map_err(|_| engine::Error::Backend)?;
        if length > self.capacity {
            return Err(engine::Error::Capacity);
        }
        if command.kind <= 2 {
            // Publish admitted initialized bytes through sparse output writes.
            // SAFETY: successful effect guarantees initialized disjoint copied
            // prefix [old_frontier,length); no unused byte is accessed.
            for index in self.frontier..length {
                // SAFETY: successful callback initialized this admitted byte.
                let byte = unsafe { *self.output.add(index) };
                output.write(index, &[byte])?;
            }
        }
        Ok(())
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_read_clause(
    context: *const Context,
    state: *mut engine::State,
    bytes: *mut u8,
    capacity: usize,
    indexes: *mut i16,
    index_capacity: usize,
) -> i32 {
    if context.is_null()
        || state.is_null()
        || bytes.is_null()
        || indexes.is_null()
        || capacity > i32::MAX as usize
        || index_capacity > isize::MAX as usize / 2
    {
        return -2;
    }
    // SAFETY: initialized disjoint context/state snapshots and admitted retained
    // serialized owner; output/index capacity is exclusive writable storage.
    let (context, mut snapshot) = unsafe { (*context, *state) };
    if context.eof.is_none()
        || context.read.is_none()
        || context.peek.is_none()
        || context.classify.is_none()
        || context.replace.is_none()
        || context.effect.is_none()
        || !snapshot.valid()
    {
        return -2;
    }
    let mut output = RawOutput {
        bytes,
        indexes,
        capacity,
        index_capacity,
        initialized: 0,
    };
    let mut host = Host {
        context,
        output: bytes,
        capacity,
        frontier: 0,
    };
    let result = engine::read_clause(&mut snapshot, &mut output, &mut host);
    // SAFETY: exclusive copied-state publication, no callback or foreign borrow.
    unsafe {
        *state = snapshot;
    }
    result.unwrap_or(-2)
}
