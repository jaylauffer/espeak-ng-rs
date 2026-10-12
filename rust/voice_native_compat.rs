//! Native active-voice lifetime, fresh raw projections and bounded input reuse.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::ForeignVoice;
use crate::voice_load::Host as _;
use crate::{
    voice::Voice,
    voice_backend::Mbrola,
    voice_current::Current,
    voice_directive::{Action, Features},
    voice_load,
    voice_native::{self, Config, Host},
    voice_reader::{Reader, TextMode},
    voice_setup::{Effect, Setup},
};
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    io::{self, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Environment {
    voice: *mut Voice,
    translator: *mut c_void,
    fast: *mut i32,
    translator_index: *mut i32,
    selected: *mut ForeignVoice,
    dictionary_ready: c_int,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    context: *mut c_void,
    root: *const c_char,
    default: *const c_char,
    current: *mut *mut Current,
    separator: c_int,
    width: usize,
    features: u32,
    environment: Option<unsafe extern "C" fn(*mut c_void, *mut Environment)>,
    length: Option<unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i64>,
    bind: Option<unsafe extern "C" fn(*mut c_void, *const Current)>,
    reset: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
    delete: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
    translator: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
    table: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int>,
    options: Option<unsafe extern "C" fn(*mut c_void, u32, *const c_char)>,
    speed: Option<unsafe extern "C" fn(*mut c_void)>,
    replacement: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
    dictionary: Option<unsafe extern "C" fn(*mut c_void, *const c_char, u32)>,
    mbrola: Option<unsafe extern "C" fn(*mut c_void, *const Mbrola, *mut i32) -> u32>,
    backend_error: Option<unsafe extern "C" fn(*mut c_void, u32)>,
    diagnostic: Option<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char)>,
}
#[cfg(target_pointer_width = "64")]
const _: () =
    assert!(std::mem::size_of::<Descriptor>() == 168 && std::mem::size_of::<Environment>() == 48);
#[cfg(target_pointer_width = "32")]
const _: () =
    assert!(std::mem::size_of::<Descriptor>() == 84 && std::mem::size_of::<Environment>() == 24);

static ACTIVE: AtomicBool = AtomicBool::new(false);
pub(super) fn admitted() -> bool {
    ACTIVE.load(Ordering::Acquire)
}
struct Guard;
impl Guard {
    fn enter() -> Option<Self> {
        ACTIVE
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Self)
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}

// Only an admitted load accesses this private pool. The mutex is held solely
// for swapping storage; no I/O or native effect runs with a pool lock retained.
static INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
struct Source {
    bytes: Vec<u8>,
    position: usize,
}
impl Source {
    fn take() -> Self {
        let mut pool = INPUT.lock().unwrap_or_else(|e| e.into_inner());
        Self {
            bytes: std::mem::take(&mut *pool),
            position: 0,
        }
    }
}
impl Read for Source {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = out.len().min(self.bytes.len() - self.position);
        out[..n].copy_from_slice(&self.bytes[self.position..self.position + n]);
        self.position += n;
        Ok(n)
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        let mut pool = INPUT.lock().unwrap_or_else(|e| e.into_inner());
        *pool = std::mem::take(&mut self.bytes);
    }
}
unsafe fn prefix<const N: usize>(pointer: *const c_char) -> [u8; N] {
    let mut out = [0; N];
    if !pointer.is_null() {
        for (i, b) in out[..N - 1].iter_mut().enumerate() {
            // SAFETY: selected initialized prefix through NUL or N-1 readable bytes.
            *b = unsafe { pointer.add(i).read() as u8 };
            if *b == 0 {
                break;
            }
        }
    }
    out
}
unsafe fn string<const N: usize>(pointer: *const c_char, bound: usize) -> Option<[u8; N]> {
    let mut out = [0; N];
    for (i, b) in out[..bound].iter_mut().enumerate() {
        // SAFETY: selected initialized string through NUL or its refusal bound.
        *b = unsafe { pointer.add(i).read() as u8 };
        if *b == 0 {
            return Some(out);
        }
    }
    None
}
fn bytes(input: &[u8]) -> &[u8] {
    &input[..input.iter().position(|b| *b == 0).unwrap_or(input.len())]
}
struct Effects {
    d: Descriptor,
    original: *const c_char,
    fault: bool,
}
impl Effects {
    fn environment(&mut self) -> Option<Environment> {
        if self.fault {
            return None;
        }
        // SAFETY: all fields are integers/raw nullable pointers; callback initializes projections.
        let mut e: Environment = unsafe { std::mem::zeroed() };
        // SAFETY: copied synchronous callback borrows only disjoint initialized scratch.
        unsafe { self.d.environment.unwrap()(self.d.context, &mut e) };
        if e.voice.is_null() || e.fast.is_null() || e.selected.is_null() {
            self.fault = true;
            None
        } else {
            Some(e)
        }
    }
    fn current(&self) -> *mut Current {
        // SAFETY: unique live aligned engine slot, retained without replacement through admission.
        unsafe { self.d.current.read() }
    }
    fn delete(&mut self, clear: bool) {
        if self.fault {
            return;
        }
        // SAFETY: primitive consumes fresh serialized translator; clearing differs by historical phase.
        unsafe { self.d.delete.unwrap()(self.d.context, c_int::from(clear)) };
    }
    fn translator(&mut self, name: &[u8; 40]) {
        if self.fault {
            return;
        }
        // SAFETY: initialized owned terminated setup name; no retained loan.
        unsafe { self.d.translator.unwrap()(self.d.context, name.as_ptr().cast()) };
    }
    fn diagnostic(&mut self, format: &CStr, key: &[u8]) {
        if self.fault {
            return;
        }
        let mut copied = [0; 4096];
        if key.len() >= copied.len() {
            self.fault = true;
            return;
        }
        copied[..key.len()].copy_from_slice(key);
        // Preserve printf's stream effect sequence. Flattening prefix/key/newline
        // into fwrite changes unbuffered funopen callbacks and fresh engine state.
        // SAFETY: static owned format and initialized owned terminated key; native
        // stdio borrows them synchronously, ignores write errors and retains none.
        unsafe {
            self.d.diagnostic.unwrap()(self.d.context, format.as_ptr(), copied.as_ptr().cast())
        };
    }
    fn metadata(&mut self, setup: &Setup, effect: Effect, replacement: Option<&[u8]>) -> bool {
        let current = self.current();
        if current.is_null() {
            self.fault = true;
            return false;
        }
        // SAFETY: live unique metadata view, with no reference retained across native effects.
        unsafe {
            std::ptr::addr_of_mut!((*current).name).write(setup.name);
            std::ptr::addr_of_mut!((*current).languages).write(setup.languages);
        }
        let Some(e) = self.environment() else {
            return false;
        };
        // SAFETY: immediate writable selected scalar projections, disjoint from owned setup.
        unsafe {
            std::ptr::addr_of_mut!((*e.selected).gender).write(setup.gender);
            std::ptr::addr_of_mut!((*e.selected).age).write(setup.age);
        }
        if effect == Effect::SelectLanguage {
            self.select_table(bytes(&setup.phonemes));
            self.translator(&setup.translator);
            let Some(e) = self.environment() else {
                return false;
            };
            // SAFETY: fresh initialized acoustic view; raw field publication ends before more effects.
            unsafe { std::ptr::addr_of_mut!((*e.voice).language).write(setup.language) };
        }
        if let Some(value) = replacement {
            if effect == Effect::SelectPhonemes {
                self.select_table(bytes(&setup.phonemes));
            }
            // SAFETY: terminated borrowed native directive line remains live during primitive.
            unsafe { self.d.replacement.unwrap()(self.d.context, value.as_ptr().cast()) };
        }
        !self.fault
    }
}
impl voice_load::Host for Effects {
    fn directive(
        &mut self,
        snapshot: &mut Voice,
        fast: &mut i32,
        setup: &Setup,
        key: &[u8],
        value: &[u8],
        action: Action,
    ) -> bool {
        let Some(e) = self.environment() else {
            return false;
        };
        // SAFETY: writable initialized acoustic/fast views are disjoint from owned snapshots; no loans span effects.
        unsafe {
            e.voice.write(*snapshot);
            e.fast.write(*fast);
        }
        match action {
            Action::LanguageOption(key) => {
                // SAFETY: owned terminated directive value; primitive consumes serialized translator.
                unsafe { self.d.options.unwrap()(self.d.context, key, value.as_ptr().cast()) };
            }
            Action::Acoustics { update_speed } => {
                if update_speed {
                    // SAFETY: synchronous serialized speed effect; no source/snapshot loan retained.
                    unsafe { self.d.speed.unwrap()(self.d.context) };
                }
            }
            Action::Metadata(effect) => {
                if !self.metadata(setup, effect, None) {
                    return false;
                }
            }
            Action::Replacement(effect) => {
                if !self.metadata(setup, effect, Some(value)) {
                    return false;
                }
            }
            Action::Mbrola(request) => {
                let mut rate = request.sample_rate;
                // SAFETY: owned initialized backend request and disjoint exclusive rate output.
                let status = unsafe { self.d.mbrola.unwrap()(self.d.context, &request, &mut rate) };
                if status != 0 {
                    // SAFETY: synchronous engine status diagnostic, historical external status catalogue.
                    unsafe { self.d.backend_error.unwrap()(self.d.context, status) };
                    return false;
                }
                let Some(e) = self.environment() else {
                    return false;
                };
                // SAFETY: fresh writable initialized acoustic rate field after backend effects.
                unsafe { std::ptr::addr_of_mut!((*e.voice).sample_rate).write(rate) };
            }
            Action::UnsupportedMbrola => {
                self.diagnostic(c"espeak-ng was built without mbrola support\n", b"")
            }
            Action::UnsupportedKlatt => {
                self.diagnostic(c"espeak-ng was built without klatt support\n", b"")
            }
            Action::Unknown => self.diagnostic(c"Bad voice attribute: %s\n", key),
        }
        let Some(e) = self.environment() else {
            return false;
        };
        // SAFETY: immediate raw snapshot of fresh initialized engine views after all preceding effects.
        unsafe {
            *snapshot = e.voice.read();
            *fast = e.fast.read();
        }
        !self.fault
    }
    fn invalid(&mut self, key: &[u8]) {
        self.diagnostic(c"Invalid voice attribute: %s\n", key);
    }
    fn ensure_translator(&mut self, setup: &Setup) {
        if self.environment().is_some_and(|e| e.translator.is_null()) {
            self.translator(&setup.translator);
        }
    }
    fn select_table(&mut self, name: &[u8]) -> i32 {
        if self.fault {
            return -1;
        }
        // SAFETY: native request/setup slice has its retained owned NUL sentinel.
        unsafe { self.d.table.unwrap()(self.d.context, name.as_ptr().cast()) }
    }
    fn unknown_table(&mut self, name: &[u8]) {
        self.diagnostic(c"Unknown phoneme table: '%s'\n", name);
    }
    fn phoneme_index(&mut self, index: i32) {
        let Some(e) = self.environment() else {
            return;
        };
        if e.translator_index.is_null() {
            self.fault = true;
            return;
        }
        // SAFETY: fresh initialized writable acoustic and translator index fields, immediate raw writes.
        unsafe {
            std::ptr::addr_of_mut!((*e.voice).phoneme_table).write(index);
            e.translator_index.write(index);
        }
    }
    fn dictionary(&mut self, name: &[u8], quiet: bool) -> bool {
        if self.fault {
            return false;
        }
        // SAFETY: retained terminated native setup key; primitive consumes the fresh translator.
        unsafe {
            self.d.dictionary.unwrap()(
                self.d.context,
                name.as_ptr().cast(),
                if quiet { 4 } else { 0 },
            )
        };
        if self.environment().is_none_or(|e| e.dictionary_ready == 0) {
            self.delete(false);
            false
        } else {
            true
        }
    }
}
impl Host for Effects {
    type Source = Source;
    type Loaded = *mut Voice;
    fn length(&mut self, path: &[u8]) -> i64 {
        // SAFETY: initialized owned native path and sentinel, borrowed only for the immediate probe.
        unsafe { self.d.length.unwrap()(self.d.context, path.as_ptr(), path.len()) }
    }
    fn open(&mut self, path: &[u8], width: usize) -> Option<Reader<Source>> {
        let path = crate::voice_storage::compat_path(path).ok()?;
        let mut source = Source::take();
        if crate::engine_io::read_file_into(&path, super::MAX_TEXT_FILE, &mut source.bytes).is_err()
        {
            return None;
        }
        Reader::new(source, width, TextMode::platform()).ok()
    }
    fn prepare(&mut self, fallback: &[u8; 40], tone: bool) -> Option<Setup> {
        let mut current = self.current();
        if current.is_null() {
            current = super::espeak_rs_current_voice_create();
            // SAFETY: admitted live unique slot receives one retained Rust metadata owner.
            unsafe { self.d.current.write(current) };
        }
        let e = self.environment()?;
        // SAFETY: initialized selected scalars; raw copies precede the fresh original name/language reads.
        let (gender, age) = unsafe {
            (
                std::ptr::addr_of!((*e.selected).gender).read(),
                std::ptr::addr_of!((*e.selected).age).read(),
            )
        };
        // SAFETY: live original input, through NUL or its consumed41-byte identifier prefix; nullable for compilation.
        let request = unsafe { prefix::<42>(self.original) };
        // SAFETY: fresh initialized acoustic language after input copying; no native effect intervenes.
        let language = unsafe { std::ptr::addr_of!((*e.voice).language).read() };
        // SAFETY: unique live initialized metadata owner; all possibly aliased inputs have already copied.
        match unsafe {
            (&mut *current).prepare(
                bytes(&request),
                bytes(fallback),
                tone,
                gender,
                age,
                &language,
            )
        } {
            Ok(setup) => Some(setup),
            Err(_) => {
                self.fault = true;
                None
            }
        }
    }
    fn clear_translator(&mut self) {
        if self.environment().is_some_and(|e| !e.translator.is_null()) {
            self.delete(true);
        }
    }
    fn bind_current(&mut self) {
        if self.fault {
            return;
        }
        if self.current().is_null() {
            self.fault = true;
            return;
        }
        // SAFETY: retained metadata owner, field addresses remain stable until serialized clear after views drain.
        unsafe { self.d.bind.unwrap()(self.d.context, self.current()) };
    }
    fn reset(&mut self, tone: bool) {
        if self.fault {
            return;
        }
        // SAFETY: serialized engine/backend reset, with current preparation and optional binding completed.
        unsafe { self.d.reset.unwrap()(self.d.context, if tone { 2 } else { 0 }) };
    }
    fn snapshot(&mut self) -> Option<(Voice, i32)> {
        let e = self.environment()?;
        // SAFETY: initialized live acoustic/fast values, copied with no engine reference across later callbacks.
        Some(unsafe { (e.voice.read(), e.fast.read()) })
    }
    fn close(&mut self, reader: Option<Reader<Source>>) {
        drop(reader);
    }
    fn publish_languages(&mut self, setup: &Setup) {
        let current = self.current();
        if current.is_null() {
            self.fault = true;
            return;
        }
        // SAFETY: live unique metadata field; preserve fresh name and all historical partial effects.
        unsafe { std::ptr::addr_of_mut!((*current).languages).write(setup.languages) };
    }
    fn result(&mut self) -> Option<*mut Voice> {
        self.environment().map(|e| e.voice)
    }
}

#[no_mangle]
pub unsafe extern "C" fn espeak_rs_voice_native_load(
    descriptor: *const Descriptor,
    request: *const c_char,
    control: u32,
    output: *mut *mut c_void,
) -> c_int {
    if descriptor.is_null() || output.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: initialized retained descriptor, copied before inputs/effects/publication.
    let d = unsafe { descriptor.read() };
    if d.root.is_null()
        || d.default.is_null()
        || d.current.is_null()
        || !(2..=4096).contains(&d.width)
        || !matches!(d.separator, 47 | 92)
        || d.features & !3 != 0
        || d.environment.is_none()
        || d.length.is_none()
        || d.reset.is_none()
        || d.translator.is_none()
        || d.table.is_none()
        || d.options.is_none()
        || d.speed.is_none()
        || d.replacement.is_none()
        || d.diagnostic.is_none()
        || (control & 2 == 0 && (d.bind.is_none() || d.delete.is_none()))
        || (control & 8 == 0 && (d.dictionary.is_none() || d.delete.is_none()))
        || (d.features & 2 != 0 && (d.mbrola.is_none() || d.backend_error.is_none()))
    {
        return libc::EINVAL;
    }
    let Some(_guard) = Guard::enter() else {
        return libc::EAGAIN;
    };
    // SAFETY: root/default are initialized through NUL or their declared refusal bounds.
    let (Some(root), Some(default)) = (unsafe { string::<4096>(d.root, 4096) }, unsafe {
        string::<40>(d.default, 40)
    }) else {
        return libc::EINVAL;
    };
    let input = if request.is_null() {
        None
    } else if control & 0x10 != 0 {
        // SAFETY: explicit path has initialized bytes through NUL or the path-capacity refusal bound.
        let Some(input) = (unsafe { string::<4096>(request, d.width) }) else {
            return libc::EINVAL;
        };
        Some(input)
    } else {
        // SAFETY: ordinary request consumes only39 initial path-name bytes; unused tails are not scanned.
        let name = unsafe { prefix::<40>(request) };
        let mut input = [0; 4096];
        input[..40].copy_from_slice(&name);
        Some(input)
    };
    let config = Config {
        root: bytes(&root),
        default: bytes(&default),
        separator: d.separator as u8,
        width: d.width,
        features: Features {
            klatt: d.features & 1 != 0,
            mbrola: d.features & 2 != 0,
        },
    };
    let mut effects = Effects {
        d,
        original: request,
        fault: false,
    };
    let result = voice_native::load(
        &mut effects,
        &config,
        input.as_ref().map(|s| bytes(s)),
        control,
    );
    if effects.fault {
        return libc::EINVAL;
    }
    match result {
        Ok(Some(voice)) => {
            // SAFETY: admitted aligned writable pointer output is disjoint from retained engine/owner state.
            unsafe { output.write(voice.cast()) };
            0
        }
        Ok(None) => crate::voice_control::NOT_FOUND,
        Err(_) => libc::EINVAL,
    }
}

#[no_mangle]
pub unsafe extern "C" fn espeak_rs_voice_native_clear(slot: *mut *mut Current) -> c_int {
    if slot.is_null() {
        return libc::EINVAL;
    }
    let Some(_guard) = Guard::enter() else {
        return libc::EAGAIN;
    };
    if super::voice_control_compat::admitted() {
        return libc::EAGAIN;
    }
    // SAFETY: serialized live unique slot transfers its owner after all native/public views drain.
    unsafe {
        super::espeak_rs_current_voice_destroy(slot.read());
        slot.write(std::ptr::null_mut());
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_real_opens_reuse_storage_and_reset_input_position() {
        let _guard = Guard::enter().unwrap();
        let directory =
            std::env::temp_dir().join(format!("espeak-native-voice-pool-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let large = directory.join("large");
        let small = directory.join("small");
        std::fs::write(&large, vec![b'A'; 512 * 1024]).unwrap();
        std::fs::write(&small, b"name Reused\nlanguage en\n").unwrap();
        // SAFETY: private test calls only open, which consumes no descriptor fields.
        let d: Descriptor = unsafe { std::mem::zeroed() };
        let mut host = Effects {
            d,
            original: std::ptr::null(),
            fault: false,
        };
        let mut reader = host.open(large.to_str().unwrap().as_bytes(), 4096).unwrap();
        let (key, _) = reader.next_directive().unwrap().unwrap();
        assert_eq!(key.len(), 4095);
        drop(reader);
        let (pointer, capacity) = {
            let pool = INPUT.lock().unwrap();
            (pool.as_ptr() as usize, pool.capacity())
        };
        assert!(capacity >= 512 * 1024);
        for _ in 0..32 {
            let mut reader = host.open(small.to_str().unwrap().as_bytes(), 4096).unwrap();
            assert_eq!(
                reader.next_directive().unwrap(),
                Some((b"name".as_slice(), b"Reused".as_slice()))
            );
            assert_eq!(
                reader.next_directive().unwrap(),
                Some((b"language".as_slice(), b"en".as_slice()))
            );
            assert!(reader.next_directive().unwrap().is_none());
            drop(reader);
            let pool = INPUT.lock().unwrap();
            assert_eq!(pool.as_ptr() as usize, pointer);
            assert_eq!(pool.capacity(), capacity);
        }
        assert!(host
            .open(directory.to_str().unwrap().as_bytes(), 4096)
            .is_none());
        assert_eq!(INPUT.lock().unwrap().as_ptr() as usize, pointer);
        std::fs::remove_file(large).unwrap();
        std::fs::remove_file(small).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
