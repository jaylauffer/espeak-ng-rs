//! The phoneme and intonation compilers against the engine: the C API's
//! `espeak_ng_CompilePhonemeData[Path]` and `espeak_ng_CompileIntonation[Path]`
//! with the engine's sample rate, phoneme data, voice and wave generator, and
//! the C runtime's streams, in place of the C callbacks they used to receive.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    compiler_context_compat::Context,
    compiler_host_compat::{self, espeak_rs_compiler_host_classes},
    compiler_native_compat::{self, espeak_rs_compile_phonemes_native},
    intonation_native_compat::{self, espeak_rs_compile_intonation_native},
};
use crate::{compiler_driver::Op, crt};
use std::ffi::{c_char, c_int, c_long, c_void};

unsafe extern "C" {
    static path_home: c_char;
    static mut voice: *mut c_void;
    fn LoadPhData(rate: *mut c_int, context: *mut c_void) -> c_int;
    fn LoadVoice(name: *const c_char, control: c_int) -> *mut c_void;
    fn WavegenInit(rate: c_int, wavemult_fact: c_int);
    fn WavegenSetVoice(voice: *mut c_void);
    fn WavegenFini();
    fn create_file_error_context(context: *mut c_void, status: c_int, path: *const c_char)
        -> c_int;
}

/// `N_PATH_BUF` (speech.h): `_MAX_PATH` on Windows, else `PATH_MAX`.
#[cfg(windows)]
const PATH_BUFFER: usize = 260;
#[cfg(not(windows))]
const PATH_BUFFER: usize = libc::PATH_MAX as usize;
const VOICE_NOT_FOUND: c_int = 0x1000_06FF;

/// The engine's sample rate (`samplerate`).
fn rate() -> c_int {
    // SAFETY: serialized engine scalar.
    unsafe { super::wavegen_host_compat::samplerate }
}
unsafe extern "C" fn host_rate() -> c_int {
    rate()
}
unsafe extern "C" fn host_message(status: c_int, output: *mut c_char, capacity: usize) {
    // SAFETY: the compiler's bounded output.
    unsafe { super::status_compat::espeak_rs_error_message_crt(status as u32, output, capacity) }
}
unsafe extern "C" fn host_file_error(status: c_int) {
    crt::set_errno(status);
}
/// The table and file host for one compiler context.
fn host(context: *mut Context) -> compiler_host_compat::Descriptor {
    compiler_host_compat::Descriptor {
        context,
        width: PATH_BUFFER,
        resource_log: crt::standard_error().cast(),
        rate: Some(host_rate),
        generator: Some(super::wavegen_host_compat::espeak_rs_native_wavegen_state),
        message: Some(host_message),
        file_error: Some(host_file_error),
    }
}

/// The phoneme compiler's caller state.
struct PhonemeCall {
    rate: c_long,
    log: *mut libc::FILE,
    error: *mut c_void,
}
// The driver's operations (crate::compiler_driver::Op, RUST_COMPILER_*).
const RATE: c_int = Op::Rate as c_int;
const RELOAD: c_int = Op::Reload as c_int;
const VOICE: c_int = Op::Voice as c_int;
const WAVE_INIT: c_int = Op::WaveInit as c_int;
const OUTPUT_OPEN: c_int = Op::OutputOpen as c_int;
const BASE_TABLE: c_int = Op::BaseTable as c_int;
const COMPILE: c_int = Op::Compile as c_int;
const END_TABLE: c_int = Op::EndTable as c_int;
const WRITE_TABLES: c_int = Op::WriteTables as c_int;
const OUTPUT_ERROR: c_int = Op::OutputError as c_int;
const CLOSE_LOG: c_int = Op::CloseLog as c_int;
const WAVE_FINI: c_int = Op::WaveFini as c_int;
// RUST_TABLE_* (rust_compiler_tables.h).
const TABLE_START: u32 = 0;
const TABLE_FINISH: u32 = 4;
const TABLE_PACK: u32 = 5;

unsafe extern "C" fn phoneme_engine(
    opaque: *mut c_void,
    context: *mut Context,
    operation: c_int,
    value: c_int,
) -> c_int {
    // SAFETY: the compile call's state, live for its duration.
    let call = unsafe { &*opaque.cast::<PhonemeCall>() };
    // SAFETY: serialized engine; each primitive gets the compiler's live context.
    unsafe {
        match operation {
            RATE => super::wavegen_host_compat::samplerate = call.rate as c_int,
            RELOAD => {
                LoadPhData(std::ptr::null_mut(), std::ptr::null_mut());
            }
            VOICE => {
                return if LoadVoice(c"".as_ptr(), 8).is_null() {
                    VOICE_NOT_FOUND
                } else {
                    0
                }
            }
            WAVE_INIT => {
                WavegenInit(call.rate as c_int, 0);
                WavegenSetVoice(voice);
            }
            OUTPUT_OPEN => return rate(),
            BASE_TABLE => {
                compiler_host_compat::espeak_rs_compiler_host_tables(
                    &host(context),
                    TABLE_START,
                    c"base".as_ptr(),
                    0,
                );
            }
            COMPILE => {
                compiler_host_compat::espeak_rs_compiler_host_files(&host(context));
            }
            END_TABLE => {
                compiler_host_compat::espeak_rs_compiler_host_tables(
                    &host(context),
                    TABLE_FINISH,
                    std::ptr::null(),
                    0,
                );
            }
            WRITE_TABLES => {
                compiler_host_compat::espeak_rs_compiler_host_tables(
                    &host(context),
                    TABLE_PACK,
                    std::ptr::null(),
                    0,
                );
            }
            OUTPUT_ERROR => {
                let status = super::compiler_diagnostic_compat::Status {
                    context,
                    status: value,
                    name: c"compiler output".as_ptr(),
                    width: 4096,
                    message: Some(host_message),
                };
                super::compiler_diagnostic_compat::espeak_rs_compiler_diagnostic_status(&status);
            }
            CLOSE_LOG => {
                let log = (*context).f_errors.cast::<libc::FILE>();
                if log != crt::standard_error() && log != crt::standard_output() {
                    libc::fclose(log);
                }
            }
            WAVE_FINI => WavegenFini(),
            _ => return libc::EINVAL,
        }
    }
    0
}
unsafe extern "C" fn phoneme_log(opaque: *mut c_void, bytes: *const u8, length: usize) {
    // SAFETY: the compile call's state and the compiler's message fragment.
    unsafe {
        crt::write(
            (*opaque.cast::<PhonemeCall>()).log,
            std::slice::from_raw_parts(bytes, length),
        );
    }
}
unsafe extern "C" fn phoneme_error(
    opaque: *mut c_void,
    status: c_int,
    path: *const c_char,
) -> c_int {
    // SAFETY: the caller's error context and the compiler's path.
    unsafe { create_file_error_context((*opaque.cast::<PhonemeCall>()).error, status, path) }
}
/// Output that cannot reach its file goes to stderr: 0 when all of it does,
/// else errno (EIO when errno is not set).
unsafe extern "C" fn output_fallback(_: *mut c_void, bytes: *const u8, length: usize) -> c_int {
    // SAFETY: the compiler's initialized fragment.
    if unsafe {
        crt::write(
            crt::standard_error(),
            std::slice::from_raw_parts(bytes, length),
        )
    } {
        return 0;
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(code) if code != 0 => code,
        _ => libc::EIO,
    }
}

/// `espeak_ng_CompilePhonemeDataPath` (null paths for the data directory).
///
/// # Safety
/// Paths are null or terminated; `log` is null or a live C stream; `context`
/// is null or the caller's error context; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_compile_phoneme_data(
    rate: c_long,
    source: *const c_char,
    destination: *const c_char,
    log: *mut libc::FILE,
    context: *mut c_void,
) -> c_int {
    // The C runtime's narrow classes, as the original's isalpha/isspace.
    let mut alpha = [0u8; 256];
    let mut spaces = [0u8; 256];
    for i in 0..256 {
        // SAFETY: C classifiers of unsigned char values.
        unsafe {
            alpha[i] = u8::from(libc::isalpha(i as c_int) != 0);
            spaces[i] = u8::from(libc::isspace(i as c_int) != 0);
        }
    }
    // Whether this C library's %x leaves "x" after a bare "0x" prefix.
    let mut zero: u32 = 0;
    let mut remainder: c_char = 0;
    // SAFETY: matching conversions and writable scalars.
    unsafe {
        libc::sscanf(
            c"0x g".as_ptr(),
            c"%x %c".as_ptr(),
            &mut zero as *mut u32,
            &mut remainder as *mut c_char,
        );
    }
    let log = if log.is_null() {
        crt::standard_error()
    } else {
        log
    };
    let mut call = PhonemeCall {
        rate,
        log,
        error: context,
    };
    let descriptor = compiler_native_compat::Descriptor {
        context: std::ptr::addr_of_mut!(call).cast(),
        home: std::ptr::addr_of!(path_home),
        source,
        destination,
        alpha: alpha.as_ptr(),
        spaces: spaces.as_ptr(),
        width: PATH_BUFFER,
        windows: c_int::from(cfg!(windows)),
        keep_prefix_x: c_int::from(remainder == b'x' as c_char),
        version: super::synthdata_compat::version_phdata as u32,
        log_file: log.cast(),
        engine: Some(phoneme_engine),
        log: Some(phoneme_log),
        error: Some(phoneme_error),
        fallback: Some(output_fallback),
        classes: Some(espeak_rs_compiler_host_classes),
    };
    // SAFETY: the descriptor and its buffers outlive the call.
    unsafe { espeak_rs_compile_phonemes_native(&descriptor) }
}

unsafe extern "C" fn intonation_reload(_: *mut c_void) {
    // SAFETY: serialized engine.
    unsafe { LoadPhData(std::ptr::null_mut(), std::ptr::null_mut()) };
}
unsafe extern "C" fn intonation_error(
    context: *mut c_void,
    status: c_int,
    path: *const c_char,
) -> c_int {
    // SAFETY: the caller's error context and the compiler's path.
    unsafe { create_file_error_context(context, status, path) }
}
/// `espeak_ng_CompileIntonationPath` (null paths for the data directory).
///
/// # Safety
/// As for `espeak_rs_compile_phoneme_data`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_compile_intonation_data(
    source: *const c_char,
    destination: *const c_char,
    log: *mut libc::FILE,
    context: *mut c_void,
) -> c_int {
    let descriptor = intonation_native_compat::Descriptor {
        source,
        destination,
        home: std::ptr::addr_of!(path_home),
        log: log.cast(),
        fallback_log: crt::standard_error().cast(),
        error_context: context,
        width: PATH_BUFFER,
        reload: Some(intonation_reload),
        error: Some(intonation_error),
    };
    // SAFETY: the descriptor outlives the call.
    unsafe { espeak_rs_compile_intonation_native(&descriptor) }
}
