//! Complete native MBROLA host. The engine serializes foreign owners; this
//! lease also rejects callback reentry without waiting or replaying effects.
//! The external process/DLL primitives still block and are not a completion host.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_snake_case, non_upper_case_globals)]
use super::clause_postprocess_compat::Entry;
use crate::{
    generate, mbrola, mbrola_fill, mbrola_generate as mbr, phoneme::Phoneme,
    phoneme_program::PhonemeData, speed, voice::Voice, wave_memory::WaveMemory,
};
use std::{
    ffi::{c_char, c_void},
    io, ptr,
    sync::{Mutex, MutexGuard, OnceLock},
};
type Void = unsafe extern "system" fn();
type Write = unsafe extern "system" fn(*mut c_char) -> i32;
#[derive(Clone, Copy)]
#[repr(C)]
struct Config {
    enabled: u32,
    posix: u32,
    width: usize,
    load: Option<unsafe extern "C" fn() -> i32>,
    init: *const Option<Write>,
    close: *const Option<Void>,
    reset: *const Option<Void>,
    read: *const Option<unsafe extern "system" fn(*mut i16, i32) -> i32>,
    write: *const Option<Write>,
    flush: *const Option<unsafe extern "system" fn() -> i32>,
    frequency: *const Option<unsafe extern "system" fn() -> i32>,
    volume: *const Option<unsafe extern "system" fn(f32)>,
    no_error: *const Option<unsafe extern "system" fn(i32)>,
    standard_error: unsafe extern "C" fn() -> *mut libc::FILE,
}
const _: () = assert!(std::mem::size_of::<Config>() == 8 + 12 * std::mem::size_of::<usize>());
const _: () =
    assert!(std::mem::offset_of!(Config, standard_error) == 8 + 11 * std::mem::size_of::<usize>());
extern "C" {
    static espeak_rs_mbrola_config: Config;
    static mut samplerate: i32;
    static mut speed: speed::Factors;
    static mut voice: *mut Voice;
    static mut phoneme_tab: [*mut Phoneme; 256];
    static envelope_data: [*const u8; 20];
    static mut clause_start_char: i32;
    static mut clause_start_word: i32;
    static mut count_sentences: i32;
    static mut option_phoneme_events: i32;
    static mut option_phonemes: i32;
    static mut f_trans: *mut libc::FILE;
    static mut path_home: c_char;
    static mut espeak_rs_wave_memory: WaveMemory;
    static mut espeak_rs_output: crate::output::Output;
    fn SetParameter(parameter: i32, value: i32, relative: i32) -> i32;
    fn GetFileLength(path: *const c_char) -> i32;
    fn WcmdqFree() -> i32;
    fn DoEmbedded(cursor: *mut i32, source: i32);
    fn DoMarker(kind: i32, position: i32, length: i32, value: i32);
    fn DoPhonemeMarker(kind: i32, position: i32, length: i32, name: *mut c_char);
    fn WritePhMnemonicBounded(
        output: *mut c_char,
        capacity: usize,
        ph: *mut Phoneme,
        entry: *mut Entry,
        ipa: i32,
        flags: *mut i32,
    ) -> *mut c_char;
    fn SetPitch2(voice: *mut Voice, first: i32, last: i32, base: *mut i32, range: *mut i32);
    fn InterpretPhonemeWithLength(
        tr: *mut c_void,
        control: i32,
        entry: *mut Entry,
        list: *mut Entry,
        data: *mut PhonemeData,
        word: *mut Entry,
        count: usize,
    );
    fn DoSample3(data: *mut PhonemeData, length: i32, amplitude: i32) -> i32;
    fn DoSpect2(
        ph: *mut Phoneme,
        which: i32,
        fmt: *mut generate::FmtParams,
        entry: *mut Entry,
        modulation: i32,
    ) -> i32;
    fn PauseLength(length: i32, control: i32) -> i32;
    fn espeak_rs_queue_mbrola(memory: *mut WaveMemory, length: i32);
}
#[no_mangle]
static mut mbrola_name: [u8; 20] = [0; 20];
#[no_mangle]
static mut mbrola_delay: i32 = 0;
#[derive(Default)]
struct State {
    table: mbrola::Table,
    source: Vec<u8>,
    generator: mbr::Generator,
    entries: Vec<generate::Entry>,
    fill: mbrola_fill::Fill,
    prefix: i32,
    list: usize,
    count: usize,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn lease() -> Option<MutexGuard<'static, State>> {
    STATE
        .get_or_init(|| Mutex::new(State::default()))
        .try_lock()
        .ok()
}
fn config() -> Config {
    // SAFETY: immutable platform descriptor has process lifetime.
    unsafe { espeak_rs_mbrola_config }
}
unsafe fn slot<T: Copy>(p: *const Option<T>) -> Option<T> {
    // SAFETY: configured pointer slots remain initialized for process lifetime;
    // engine serialization excludes concurrent DLL load/unload.
    unsafe {
        if p.is_null() {
            None
        } else {
            p.read()
        }
    }
}
unsafe fn bytes(p: *const c_char, limit: usize) -> Result<Vec<u8>, i32> {
    if p.is_null() {
        return Err(libc::EINVAL);
    }
    let mut out = Vec::new();
    for i in 0..limit {
        // SAFETY: caller supplies a terminated string; read only up to its NUL.
        let b = unsafe { p.add(i).cast::<u8>().read() };
        if b == 0 {
            return Ok(out);
        }
        out.push(b);
    }
    Err(libc::ENAMETOOLONG)
}
fn path(parts: &[&[u8]], width: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(width);
    for part in parts {
        let n = part.len().min(width - 1 - out.len());
        out.extend_from_slice(&part[..n]);
    }
    out.push(0);
    out
}
fn io_error(e: io::Error) -> i32 {
    #[cfg(unix)]
    if let Some(code) = e.raw_os_error() {
        return code;
    }
    match e.kind() {
        io::ErrorKind::NotFound => libc::ENOENT,
        io::ErrorKind::PermissionDenied => libc::EACCES,
        io::ErrorKind::IsADirectory => libc::EISDIR,
        io::ErrorKind::OutOfMemory | io::ErrorKind::WouldBlock => libc::ENOMEM,
        io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => libc::EINVAL,
        _ => libc::EIO,
    }
}
unsafe fn close(c: Config) {
    // SAFETY: fresh optional runtime slot; caller holds the host lease.
    unsafe {
        if let Some(f) = slot(c.close) {
            f()
        }
    }
}
#[no_mangle]
unsafe extern "C" fn LoadMbrolaTable(
    name: *const c_char,
    translation: *const c_char,
    rate: *mut i32,
) -> i32 {
    let c = config();
    if c.enabled == 0 {
        return 0x10000aff;
    }
    let Some(mut state) = lease() else {
        return libc::EAGAIN;
    };
    // SAFETY: admitted serialized engine call, owns these published scalars.
    unsafe {
        mbrola_name[0] = 0;
        mbrola_delay = 0;
    }
    state.prefix = 0;
    if name.is_null() {
        // SAFETY: ordinary parameter primitive under engine serialization.
        unsafe {
            SetParameter(14, 0, 0);
        }
        return 0;
    }
    if rate.is_null() || c.width < 2 || c.width > 65536 {
        return libc::EINVAL;
    }
    // SAFETY: caller strings and home array are terminated/live through call;
    // own all bytes before file I/O or external runtime callbacks.
    let (name, translation, home) = unsafe {
        (
            bytes(name, 20),
            bytes(translation, c.width),
            bytes(ptr::addr_of!(path_home), c.width),
        )
    };
    let (name, translation, home) = match (name, translation, home) {
        (Ok(a), Ok(b), Ok(d)) => (a, b, d),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
    };
    // SAFETY: descriptor load callback initializes subsequent DLL slots.
    if unsafe { c.load.is_none_or(|f| f() == 0) } {
        return 0x100007ff;
    }
    let mut voice_path = path(&[&home, b"/mbrola/", &name], c.width);
    if c.posix != 0 {
        // SAFETY: getenv result is copied before any callbacks; engine's
        // environment mutation follows the platform's process-wide contract.
        let dirs = unsafe {
            let p = libc::getenv(c"XDG_DATA_DIRS".as_ptr());
            if p.is_null() {
                Ok(b"/usr/local/share:/usr/share".to_vec())
            } else {
                bytes(p, 1024 * 1024)
            }
        };
        let dirs = match dirs {
            Ok(v) => v,
            Err(e) => return e,
        };
        // SAFETY: owned terminated path, primitive retains no pointer.
        let mut found = unsafe { GetFileLength(voice_path.as_ptr().cast()) } > 0;
        if !found {
            'search: for dir in dirs.split(|b| *b == b':').filter(|d| !d.is_empty()) {
                for parts in [
                    vec![dir, b"/mbrola/", &name],
                    vec![dir, b"/mbrola/", &name, b"/", &name],
                    vec![dir, b"/mbrola/voices/", &name],
                ] {
                    voice_path = path(&parts, c.width);
                    // SAFETY: owned terminated path, primitive retains no pointer.
                    if unsafe { GetFileLength(voice_path.as_ptr().cast()) } > 0 {
                        found = true;
                        break 'search;
                    }
                }
            }
        }
        if !found {
            let mut diagnostic = Vec::new();
            for p in [
                b"Cannot find MBROLA voice file '".as_slice(),
                &name,
                b"' in neither of paths:\n - $data_dir/mbrola/",
                &name,
                b"\n - $data_dir/mbrola/",
                &name,
                b"/",
                &name,
                b"\n - $data_dir/mbrola/voices/",
                &name,
                b"\nfor any data_dir in XDG_DATA_DIRS=",
                &dirs,
                b"\nPlease install necessary MBROLA voice!\n",
            ] {
                diagnostic.extend_from_slice(p);
            }
            // SAFETY: borrowed stderr and initialized diagnostic bytes; lease
            // refuses callback reentry for the full duration of publication.
            unsafe {
                libc::fwrite(
                    diagnostic.as_ptr().cast(),
                    1,
                    diagnostic.len(),
                    (c.standard_error)(),
                );
            }
            voice_path = path(&[&name], c.width);
        }
        // SAFETY: POSIX closes the previous backend before every initialization.
        unsafe { close(c) }
    }
    // SAFETY: all runtime slots are fresh; init may mutate its owned path only.
    unsafe {
        if slot(c.init).is_none_or(|f| f(voice_path.as_mut_ptr().cast()) != 0) {
            return 0x100008ff;
        }
        if let Some(f) = slot(c.no_error) {
            f(1)
        }
    }
    let table_path = path(&[&home, b"/mbrola_ph/", &translation], c.width);
    let result = crate::voice_storage::compat_path(&table_path[..table_path.len() - 1])
        .and_then(|p| crate::engine_io::read_file_into(&p, mbrola::MAX_BYTES, &mut state.source));
    let result = result.and_then(|()| {
        let State { table, source, .. } = &mut *state;
        table.replace(source)
    });
    if let Err(e) = result {
        // SAFETY: load failed after init; close backend, retain previous table.
        unsafe { close(c) }
        return io_error(e);
    }
    // SAFETY: serialized globals and exclusive caller rate, fresh runtime slots.
    unsafe {
        if let Some(f) = slot(c.volume) {
            f((state.table.control() & 255) as f32 / 16.0)
        }
        let frequency = slot(c.frequency).map_or(0, |f| f());
        samplerate = frequency;
        rate.write(frequency);
        SetParameter(14, i32::from(frequency != 22050), 0);
        ptr::copy_nonoverlapping(
            name.as_ptr(),
            ptr::addr_of_mut!(mbrola_name).cast(),
            name.len(),
        );
        mbrola_name[name.len()] = 0;
        mbrola_delay = 1000;
    }
    0
}
struct Host<'a> {
    list: *mut Entry,
    count: usize,
    file: *mut libc::FILE,
    table: &'a mbrola::Table,
    prefix: &'a mut i32,
    c: Config,
}
impl Host<'_> {
    fn entry(&self, ix: usize) -> Entry {
        // SAFETY: admitted initialized count+1 foreign rows retained by engine;
        // copy ends every foreign loan before callbacks.
        unsafe { self.list.add(ix).read() }
    }
    fn phoneme(p: *const Phoneme) -> Option<Phoneme> {
        // SAFETY: engine retains any nonnull phoneme row for this clause.
        unsafe { p.as_ref().copied() }
    }
}
impl mbr::Host for Host<'_> {
    fn free(&mut self) -> i32 {
        // SAFETY: serialized queue primitive.
        unsafe { WcmdqFree() }
    }
    fn settings(&mut self) -> mbr::Settings {
        // SAFETY: initialized engine globals; copy before effects.
        unsafe {
            mbr::Settings {
                pause_factor: speed.pause,
                wav_factor: speed.wave,
                sample_rate: samplerate,
                lengthen: Self::phoneme(phoneme_tab[12])
                    .map_or(0, |p| i32::from(p.standard_length)),
                clause_char: clause_start_char,
                clause_word: clause_start_word,
                sentences: count_sentences,
                phoneme_events: option_phoneme_events,
            }
        }
    }
    fn embedded(&mut self, cursor: &mut i32, source: i32) {
        // SAFETY: exclusive local cursor, primitive retains no pointer.
        unsafe { DoEmbedded(cursor, source) }
    }
    fn marker(&mut self, kind: i32, position: i32, length: i32, value: i32) {
        // SAFETY: serialized bounded queue effect.
        unsafe { DoMarker(kind, position, length, value) }
    }
    fn phoneme_marker(&mut self, ix: usize, ipa: bool, position: i32) {
        let entry = self.entry(ix);
        let mut name = [0; 64];
        // SAFETY: owned output and live foreign entry; no loan across callback.
        unsafe {
            if !WritePhMnemonicBounded(
                name.as_mut_ptr(),
                name.len(),
                entry.phoneme,
                self.list.add(ix),
                i32::from(ipa) * 2,
                ptr::null_mut(),
            )
            .is_null()
            {
                DoPhonemeMarker(7, position, 0, name.as_mut_ptr());
            }
        }
    }
    fn select(&mut self, ix: usize) -> mbrola::Selection {
        let p = self.entry(ix);
        let next = self.entry(ix + 1);
        let previous = self.entry(ix - 1);
        let Some(current) = Self::phoneme(p.phoneme) else {
            return mbrola::Selection::default();
        };
        // SAFETY: serialized resident pause-table pointer copied immediately.
        let pause = unsafe { Self::phoneme(phoneme_tab[0]) };
        let selected = mbrola::select(
            self.table.mappings(),
            &current,
            Self::phoneme(previous.phoneme).as_ref(),
            Self::phoneme(next.phoneme).as_ref(),
            pause.as_ref(),
            &mbrola::Context {
                word_start: p.new_word.into(),
                next_word_start: next.new_word.into(),
                synth_flags: p.flags.into(),
                stress: p.stress.into(),
                word_stress: p.word_stress.into(),
                prefix: *self.prefix,
            },
        );
        *self.prefix = selected.prefix;
        selected
    }
    fn pitch(&mut self, ix: usize, split: i32, final_only: bool) -> Result<mbr::Text, mbr::Error> {
        let p = self.entry(ix);
        let mut output = mbr::Text::default();
        let mut pitch = crate::synthesis_parameters::Pitch::default();
        // SAFETY: valid resident envelope copied before calling pitch primitive.
        let envelope = unsafe {
            if usize::from(p.env) >= 20 || envelope_data[usize::from(p.env)].is_null() {
                return Ok(output);
            }
            *(envelope_data[usize::from(p.env)] as *const [u8; 128])
        };
        // SAFETY: initialized voice and disjoint local scalar outputs.
        unsafe {
            SetPitch2(
                voice,
                p.pitch1.into(),
                p.pitch2.into(),
                &mut pitch.base,
                &mut pitch.range,
            )
        }
        // Previous native WritePitch published empty text on a bounded
        // arithmetic/split refusal and then continued generation.
        if let Ok(text) =
            crate::mbrola_output::pitch_text(&envelope, p.env.into(), pitch, split, final_only)
        {
            output.append(text.bytes())?;
        }
        Ok(output)
    }
    fn set_flags(&mut self, ix: usize, flags: u16) {
        // SAFETY: admitted mutable row; no overlapping foreign loan retained.
        unsafe {
            (*self.list.add(ix)).flags = flags;
        }
    }
    fn interpret(&mut self, ix: usize) -> PhonemeData {
        let mut data = PhonemeData::default();
        // SAFETY: admitted foreign list/extent and exclusive local output.
        unsafe {
            InterpretPhonemeWithLength(
                ptr::null_mut(),
                0,
                self.list.add(ix),
                self.list,
                &mut data,
                ptr::null_mut(),
                self.count,
            )
        }
        data
    }
    fn sample(&mut self, data: &mut PhonemeData, length: i32) -> i32 {
        // SAFETY: disjoint owned data, primitive retains no reference.
        unsafe { DoSample3(data, length, -1) }
    }
    fn spect(&mut self, ix: usize, fmt: &mut generate::FmtParams) -> i32 {
        let p = self.entry(ix);
        // SAFETY: copied pointer from live row and exclusive local format state.
        unsafe { DoSpect2(p.phoneme, 0, fmt, self.list.add(ix), -1) }
    }
    fn pause_length(&mut self, length: i32, control: i32) -> i32 {
        // SAFETY: serialized primitive reads current speed state.
        unsafe { PauseLength(length, control) }
    }
    fn submit(&mut self, text: &mbr::Text, offset: usize, file: bool) -> mbr::Admission {
        let bytes = &text.bytes()[offset..];
        // SAFETY: FILE is borrowed synchronously; DLL receives only a mutable
        // bounded copy and cannot overwrite the generator's retained command.
        let n = unsafe {
            if file {
                let n = libc::fwrite(bytes.as_ptr().cast(), 1, bytes.len(), self.file);
                if n == 0 {
                    -1
                } else {
                    n as i32
                }
            } else {
                let mut copy = [0; mbr::TEXT_CAPACITY];
                copy[..bytes.len()].copy_from_slice(bytes);
                slot(self.c.write).map_or(-1, |f| f(copy.as_mut_ptr().cast()))
            }
        };
        match n {
            n if n > 0 && n as usize == bytes.len() => mbr::Admission::Accepted,
            n if n > 0 => mbr::Admission::Progress(n as usize),
            0 => mbr::Admission::Pending,
            _ => mbr::Admission::Failed,
        }
    }
    fn queue_audio(&mut self, length: i32) {
        // SAFETY: serialized queue, admission was checked by generator.
        unsafe { espeak_rs_queue_mbrola(ptr::addr_of_mut!(espeak_rs_wave_memory), length) }
    }
    fn drain_audio(&mut self) {
        self.queue_audio(500)
    }
    fn flush(&mut self) -> mbr::Admission {
        // SAFETY: fresh runtime slots; Windows flush zero means error, Unix
        // command zero means backpressure. Neither acknowledges output EOF.
        let n = unsafe {
            if self.c.posix == 0 {
                slot(self.c.flush).map_or(-1, |f| if f() != 0 { 1 } else { -1 })
            } else {
                let mut text = *b"\n#\n\0";
                slot(self.c.write).map_or(-1, |f| f(text.as_mut_ptr().cast()))
            }
        };
        match n {
            n if n > 0 => mbr::Admission::Accepted,
            0 => mbr::Admission::Pending,
            _ => mbr::Admission::Failed,
        }
    }
}
unsafe fn translate(
    state: &mut State,
    list: *mut Entry,
    count: usize,
    resume: bool,
    file: *mut libc::FILE,
) -> i32 {
    if list.is_null() || count > 1000 {
        return 0;
    }
    if resume {
        if state.list != list as usize || state.count != count {
            return 0;
        }
    } else {
        if state.entries.capacity() < 1001
            && state
                .entries
                .try_reserve_exact(1001 - state.entries.len())
                .is_err()
        {
            return 0;
        }
        state.entries.clear();
        for ix in 0..=count {
            // SAFETY: caller provides initialized count+1 rows and live phonemes.
            let p = unsafe { list.add(ix).read() };
            let ph = Host::phoneme(p.phoneme);
            state.entries.push(generate::Entry {
                phoneme: ph.unwrap_or_default(),
                length: p.length,
                synthflags: p.flags,
                source: p.source,
                kind: p.kind,
                new_word: p.new_word,
                prepause: p.prepause,
                env: p.env,
                pitch1: p.pitch1,
                pitch2: p.pitch2,
                present: u8::from(ph.is_some()),
                ..generate::Entry::default()
            });
        }
        state.list = list as usize;
        state.count = count;
    }
    let State {
        generator,
        entries,
        table,
        prefix,
        ..
    } = state;
    let mut host = Host {
        list,
        count,
        file,
        table,
        prefix,
        c: config(),
    };
    i32::from(generator.translate(entries, count, resume, !file.is_null(), &mut host) == Ok(true))
}
#[no_mangle]
unsafe extern "C" fn MbrolaTranslate(
    list: *mut Entry,
    count: i32,
    resume: bool,
    file: *mut libc::FILE,
) -> i32 {
    if config().enabled == 0 {
        return 0;
    }
    let Some(mut state) = lease() else { return 0 };
    // SAFETY: serialized caller retains complete foreign list for this call.
    unsafe { translate(&mut state, list, count as usize, resume, file) }
}
#[no_mangle]
unsafe extern "C" fn MbrolaGenerate(list: *mut Entry, count: *mut i32, resume: bool) -> i32 {
    if config().enabled == 0 || count.is_null() {
        return 0;
    }
    let Some(mut state) = lease() else { return 0 };
    // SAFETY: exclusive initialized count; list/FILE remain live through call.
    unsafe {
        if *count == 0 {
            return 0;
        }
        let file = if option_phonemes & 16 != 0 {
            f_trans
        } else {
            ptr::null_mut()
        };
        let result = translate(&mut state, list, *count as usize, resume, file);
        if result == 0 {
            *count = 0
        }
        result
    }
}
#[no_mangle]
unsafe extern "C" fn MbrolaFill(length: i32, resume: bool, amplitude: i32) -> i32 {
    let c = config();
    if c.enabled == 0 {
        return 0;
    }
    let Some(mut state) = lease() else { return 0 };
    // SAFETY: serialized initialized output descriptor, disjoint engine buffer.
    let (output, end, rate) = unsafe { (espeak_rs_output.ptr, espeak_rs_output.end, samplerate) };
    let Some(size) = (end as usize).checked_sub(output as usize) else {
        return 0;
    };
    if output.is_null() || size > i32::MAX as usize || size % 2 != 0 || output as usize % 2 != 0 {
        return 0;
    }
    // SAFETY: live initialized exclusive buffer; external reader is synchronous,
    // bounded by the requested count and may not retain the destination.
    let outcome = unsafe {
        state.fill.fill(
            std::slice::from_raw_parts_mut(output, size),
            rate,
            length,
            resume,
            amplitude,
            |bytes| match slot(c.read).map_or(-1, |f| {
                f(bytes.as_mut_ptr().cast(), (bytes.len() / 2) as i32)
            }) {
                n if n > 0 => mbrola_fill::Read::Samples(n as usize),
                0 => mbrola_fill::Read::End,
                _ => mbrola_fill::Read::Failed,
            },
        )
    };
    let Ok(outcome) = outcome else { return 0 };
    // SAFETY: outcome bytes are within the original output extent.
    unsafe { espeak_rs_output.ptr = output.add(outcome.bytes) }
    i32::from(matches!(
        outcome.status,
        mbrola_fill::Status::More | mbrola_fill::Status::Pending
    ))
}
#[no_mangle]
unsafe extern "C" fn MbrolaReset() {
    let c = config();
    if c.enabled == 0 {
        return;
    }
    let Some(_state) = lease() else { return };
    // SAFETY: guarded fresh optional primitive; no local cursor reset in old host.
    unsafe {
        if let Some(f) = slot(c.reset) {
            f()
        }
    }
}
#[no_mangle]
unsafe extern "C" fn FreeMbrolaTable() {
    let c = config();
    if c.enabled == 0 {
        return;
    }
    let Some(mut state) = lease() else { return };
    *state = State::default();
    // SAFETY: all host loans drained, published metadata cleared before close.
    unsafe {
        mbrola_name[0] = 0;
        mbrola_delay = 0;
        close(c)
    }
}
