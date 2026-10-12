//! Snapshotted public voice requests and scoped serialized engine effects.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::ForeignVoice;
use crate::{
    voice::Voice,
    voice_control::{self, Host, Name},
};
use std::{
    ffi::{c_char, c_int, c_void},
    sync::atomic::{AtomicBool, Ordering},
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    context: *mut c_void,
    separator: c_int,
    classes: Option<unsafe extern "C" fn(*mut c_void, *mut u8)>,
    load: Option<unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> *mut c_void>,
    empty: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    catalog: Option<unsafe extern "C" fn(*mut c_void)>,
    by_name: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *const c_char>,
    select:
        Option<unsafe extern "C" fn(*mut c_void, *const ForeignVoice, *mut c_int) -> *const c_char>,
    change: Option<unsafe extern "C" fn(*mut c_void)>,
    current: Option<unsafe extern "C" fn(*mut c_void) -> *const Voice>,
    stack: Option<unsafe extern "C" fn(*mut c_void, *const ForeignVoice, *const c_char)>,
}
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Descriptor>() == 88);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(std::mem::size_of::<Descriptor>() == 44);
unsafe fn string<const N: usize>(pointer: *const c_char) -> Option<[u8; N]> {
    let mut bytes = [0; N];
    for (i, b) in bytes.iter_mut().enumerate() {
        // SAFETY: initialized through NUL, or the whole admitted bound on refusal.
        *b = unsafe { pointer.add(i).read() as u8 };
        if *b == 0 {
            return Some(bytes);
        }
    }
    None
}
unsafe fn prefix(pointer: *const c_char) -> [u8; 60] {
    // SAFETY: request supplies bytes through NUL or its consumed59-byte prefix.
    unsafe { property_prefix::<60>(pointer) }
}
unsafe fn property_prefix<const N: usize>(pointer: *const c_char) -> [u8; N] {
    let mut bytes = [0; N];
    for (i, b) in bytes[..N - 1].iter_mut().enumerate() {
        // SAFETY: through NUL or the initialized consumed prefix; unused tails
        // are never scanned, and the final byte remains our owned terminator.
        *b = unsafe { pointer.add(i).read() as u8 };
        if *b == 0 {
            break;
        }
    }
    bytes
}
struct Effects {
    d: Descriptor,
    fault: bool,
    original_name: *const c_char,
    original_selector: *const ForeignVoice,
}
impl Effects {
    unsafe fn name(&mut self, p: *const c_char) -> Option<Name> {
        if p.is_null() {
            return None;
        }
        // SAFETY: primitive retains its initialized result until immediate copying.
        match unsafe { string::<4096>(p) } {
            Some(bytes) => Some(Name(bytes)),
            None => {
                self.fault = true;
                None
            }
        }
    }
}
impl Host for Effects {
    type Loaded = *mut c_void;
    type Selector = ForeignVoice;
    fn classes(&mut self) -> [u8; 256] {
        let mut map = [0; 256];
        // SAFETY: copied callback receives initialized exclusive fixed scratch.
        unsafe { self.d.classes.unwrap()(self.d.context, map.as_mut_ptr()) };
        if map[0] != 0 {
            self.fault = true;
        }
        map
    }
    fn load(&mut self, name: &[u8], control: i32) -> Option<*mut c_void> {
        // SAFETY: owned terminated load name, no retained loan or callback unwind.
        let p = unsafe { self.d.load.unwrap()(self.d.context, name.as_ptr().cast(), control) };
        (!p.is_null()).then_some(p)
    }
    fn empty_catalog(&mut self) -> bool {
        // SAFETY: fresh serialized engine scalar after a refused initial load.
        unsafe { self.d.empty.unwrap()(self.d.context) != 0 }
    }
    fn catalog(&mut self) {
        // SAFETY: synchronous serialized catalogue admission, no input loans.
        unsafe { self.d.catalog.unwrap()(self.d.context) };
    }
    fn by_name(&mut self, name: &[u8]) -> Option<Name> {
        // SAFETY: owned terminated base; result stays live through its immediate copy.
        let p = unsafe { self.d.by_name.unwrap()(self.d.context, name.as_ptr().cast()) };
        // SAFETY: primitive's bounded result loan is snapshotted before more effects.
        unsafe { self.name(p) }
    }
    fn select(&mut self, selector: &ForeignVoice) -> (bool, Option<Name>) {
        let mut found = 0;
        // SAFETY: snapshotted selector/strings and disjoint initialized scalar.
        let p = unsafe { self.d.select.unwrap()(self.d.context, selector, &mut found) };
        if found == 0 {
            return (false, None);
        }
        if p.is_null() {
            self.fault = true;
            return (true, None);
        }
        // SAFETY: bounded primitive result, copied before load/catalogue mutation.
        (true, unsafe { self.name(p) })
    }
    fn change(&mut self) {
        // SAFETY: synchronous primitive reads the fresh current voice, not saved load output.
        unsafe { self.d.change.unwrap()(self.d.context) };
    }
    fn language(&mut self) -> Option<[u8; 20]> {
        // SAFETY: serialized current voice stays live for the immediate field copy.
        let p = unsafe { self.d.current.unwrap()(self.d.context) };
        if p.is_null() {
            self.fault = true;
            return None;
        }
        // SAFETY: initialized native Voice; raw field read before any further effect.
        let language = unsafe { std::ptr::addr_of!((*p).language).read() };
        if !language.contains(&0) {
            self.fault = true;
            return None;
        }
        Some(language)
    }
    fn stack_name(&mut self, _: &[u8], language: &[u8; 20], variant: &[u8; 40]) {
        let mut copied = [0; 40];
        for (i, byte) in copied[..39].iter_mut().enumerate() {
            // SAFETY: original input stays live; reread its actual stack prefix
            // after engine effects, preserving aliases of current-voice metadata.
            *byte = unsafe { self.original_name.add(i).read() as u8 };
            if *byte == 0 {
                break;
            }
        }
        // SAFETY: pointers are raw nullable fields, remaining fields are integers.
        let mut selector: ForeignVoice = unsafe { std::mem::zeroed() };
        selector.name = copied.as_ptr().cast();
        selector.languages = language.as_ptr().cast();
        // SAFETY: initialized selector/terminated owned strings; native stack copies its prefixes.
        unsafe { self.d.stack.unwrap()(self.d.context, &selector, variant.as_ptr().cast()) };
    }
    fn stack_properties(&mut self, _: &ForeignVoice) {
        // SAFETY: original selector remains live; no reference spans engine effects.
        let mut selector = unsafe { self.original_selector.read() };
        let mut name = [0; 40];
        if !selector.name.is_null() {
            for (i, byte) in name[..39].iter_mut().enumerate() {
                // SAFETY: fresh live name prefix through NUL or39 initialized bytes.
                *byte = unsafe { selector.name.add(i).read() as u8 };
                if *byte == 0 {
                    break;
                }
            }
            selector.name = name.as_ptr().cast();
        }
        let language = if selector.languages.is_null() {
            None
        } else {
            // SAFETY: fresh live language through NUL or20 readable bytes on refusal.
            match unsafe { string::<20>(selector.languages) } {
                Some(s) => Some(s),
                None => {
                    self.fault = true;
                    return;
                }
            }
        };
        selector.languages = language
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ptr().cast());
        // SAFETY: fresh selected stack fields/owned strings, immutable empty variant.
        unsafe { self.d.stack.unwrap()(self.d.context, &selector, c"".as_ptr()) };
    }
}
static BUSY: AtomicBool = AtomicBool::new(false);
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
/// Selected snapshots precede synchronous effects; callbacks retain no owned loans.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_voice_control_call(
    descriptor: *const Descriptor,
    mode: u32,
    request: *const c_char,
    selector: *const ForeignVoice,
    number: c_int,
    output: *mut *mut c_void,
) -> c_int {
    if descriptor.is_null() || mode > 3 {
        return libc::EINVAL;
    }
    // SAFETY: fully initialized descriptor copied before any callback/publication.
    let d = unsafe { descriptor.read() };
    if !matches!(d.separator, 47 | 92)
        || d.load.is_none()
        || (mode < 2 && (d.classes.is_none() || d.current.is_none()))
        || (mode < 3 && (d.change.is_none() || d.stack.is_none()))
        || (mode == 0 && (d.empty.is_none() || d.catalog.is_none() || d.by_name.is_none()))
        || (mode == 2 && d.select.is_none())
        || (mode == 3 && output.is_null())
    {
        return libc::EINVAL;
    }
    // Nonwaiting refusal precedes foreign input reads or engine effects.
    if BUSY
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return libc::EAGAIN;
    }
    let _guard = Guard;
    let mut effects = Effects {
        d,
        fault: false,
        original_name: request,
        original_selector: selector,
    };
    let status = if mode == 2 {
        if selector.is_null() {
            return libc::EINVAL;
        }
        // SAFETY: initialized selector copied before its selected string snapshots.
        let mut selected = unsafe { selector.read() };
        // SAFETY: each nonnull selected property string has its documented readable bound.
        let name = if selected.name.is_null() {
            None
        } else {
            // SAFETY: initialized name through NUL or4095 readable prefix bytes.
            Some(unsafe { property_prefix::<4096>(selected.name) })
        };
        // Selection may reject a long language before ever updating the stack.
        // Preserve that defined outcome; enforce the20-byte stack bound only
        // when its fresh post-load language is actually consumed.
        let language = if selected.languages.is_null() {
            None
        } else {
            // SAFETY: initialized language through NUL or4095 readable prefix bytes.
            Some(unsafe { property_prefix::<4096>(selected.languages) })
        };
        // SAFETY: identifier may drive selection when name/language is absent; snapshot before effects.
        let id = if selected.identifier.is_null() {
            None
        } else {
            // SAFETY: initialized identifier through NUL or4095 readable prefix bytes.
            Some(unsafe { property_prefix::<4096>(selected.identifier) })
        };
        selected.name = name
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ptr().cast());
        selected.languages = language
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ptr().cast());
        selected.identifier = id.as_ref().map_or(std::ptr::null(), |s| s.as_ptr().cast());
        voice_control::properties(&mut effects, &selected, d.separator as u8)
    } else {
        if request.is_null() {
            return libc::EINVAL;
        }
        // SAFETY: first59 bytes or through NUL; independent of every subsequent effect.
        let name = unsafe { prefix(request) };
        let n = name.iter().position(|b| *b == 0).unwrap();
        if mode == 3 {
            match voice_control::variant(&mut effects, &name[..n], number, d.separator as u8) {
                Some(loaded) => {
                    // SAFETY: admitted writable pointer slot; all overlapping inputs already copied.
                    unsafe { output.write(loaded) };
                    0
                }
                None => voice_control::NOT_FOUND,
            }
        } else {
            voice_control::named(&mut effects, &name[..n], mode == 1, d.separator as u8)
        }
    };
    if effects.fault {
        libc::EINVAL
    } else {
        status
    }
}
