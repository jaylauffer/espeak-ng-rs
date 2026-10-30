//! The MBROLA wrapper (from mbrowrap.c): the ten `*_MBR` entry points the
//! engine calls, bound by `load_MBR` either to mbrola.dll (Windows) or to an
//! `mbrola` child process talking over three pipes (elsewhere). The entry
//! points are Rust statics, so they exist in every build.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_upper_case_globals, non_snake_case)]
use std::ffi::{c_char, c_float, c_int, c_short};

type Init = unsafe extern "system" fn(*mut c_char) -> c_int;
type Plain = unsafe extern "system" fn();
type Read = unsafe extern "system" fn(*mut c_short, c_int) -> c_int;
type Write = unsafe extern "system" fn(*mut c_char) -> c_int;
type Query = unsafe extern "system" fn() -> c_int;
type Volume = unsafe extern "system" fn(c_float);
type LastError = unsafe extern "system" fn(*mut c_char, c_int) -> *mut c_char;
type NoError = unsafe extern "system" fn(c_int);

#[no_mangle]
static mut init_MBR: Option<Init> = None;
#[no_mangle]
static mut close_MBR: Option<Plain> = None;
#[no_mangle]
static mut reset_MBR: Option<Plain> = None;
#[no_mangle]
static mut read_MBR: Option<Read> = None;
#[no_mangle]
static mut write_MBR: Option<Write> = None;
#[no_mangle]
static mut flush_MBR: Option<Query> = None;
#[no_mangle]
static mut getFreq_MBR: Option<Query> = None;
#[no_mangle]
static mut setVolumeRatio_MBR: Option<Volume> = None;
#[no_mangle]
static mut lastErrorStr_MBR: Option<LastError> = None;
#[no_mangle]
static mut setNoError_MBR: Option<NoError> = None;

/// `load_MBR`: 1 when the entry points are bound.
///
/// # Safety
/// Serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_load_mbr() -> c_int {
    // SAFETY: caller contract.
    unsafe { platform::load() }
}
/// `unload_MBR`.
///
/// # Safety
/// No MBROLA call is in progress; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_unload_mbr() {
    // SAFETY: caller contract.
    unsafe { platform::unload() }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{ffi::c_void, ptr};

    unsafe extern "system" {
        fn LoadLibraryA(name: *const c_char) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> c_int;
    }
    static mut LIBRARY: *mut c_void = ptr::null_mut();

    /// An exported function of the loaded DLL, or None.
    ///
    /// # Safety
    /// `T` is the function's pointer type, as mbrola.dll exports it.
    unsafe fn entry<T>(name: &std::ffi::CStr) -> Option<T> {
        // SAFETY: the loaded library; a null result is None.
        unsafe {
            let address = GetProcAddress(LIBRARY, name.as_ptr());
            (!address.is_null()).then(|| std::mem::transmute_copy(&address))
        }
    }
    pub(super) unsafe fn load() -> c_int {
        // SAFETY: serialized engine state; the DLL's exports have the
        // signatures mbrowrap.h declares.
        unsafe {
            if !LIBRARY.is_null() {
                return 1;
            }
            LIBRARY = LoadLibraryA(c"mbrola.dll".as_ptr());
            if LIBRARY.is_null() {
                return 0;
            }
            init_MBR = entry(c"init_MBR");
            write_MBR = entry(c"write_MBR");
            flush_MBR = entry(c"flush_MBR");
            getFreq_MBR = entry(c"getFreq_MBR");
            read_MBR = entry(c"read_MBR");
            close_MBR = entry(c"close_MBR");
            reset_MBR = entry(c"reset_MBR");
            lastErrorStr_MBR = entry(c"lastErrorStr_MBR");
            setNoError_MBR = entry(c"setNoError_MBR");
            setVolumeRatio_MBR = entry(c"setVolumeRatio_MBR");
        }
        1
    }
    pub(super) unsafe fn unload() {
        // SAFETY: serialized engine state.
        unsafe {
            if !LIBRARY.is_null() {
                FreeLibrary(LIBRARY);
                LIBRARY = ptr::null_mut();
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{
        super::engine_c as c, close_MBR, flush_MBR, getFreq_MBR, init_MBR, lastErrorStr_MBR,
        read_MBR, reset_MBR, setNoError_MBR, setVolumeRatio_MBR, write_MBR,
    };
    use crate::mbrola_transport::{self, Transport};
    use std::{
        ffi::{c_char, c_float, c_int, c_short, CStr, CString},
        ptr::{self, addr_of_mut},
    };

    // enum mbr_state.
    const INACTIVE: u8 = 0;
    const IDLE: u8 = 1;
    const NEW_DATA: u8 = 2;
    const AUDIO: u8 = 3;
    const WEDGED: u8 = 4;

    static mut STATE: u8 = INACTIVE;
    static mut VOICE_PATH: Option<CString> = None;
    static mut COMMAND_FD: c_int = 0;
    static mut AUDIO_FD: c_int = 0;
    static mut ERROR_FD: c_int = 0;
    static mut PROC_STAT: c_int = 0;
    static mut PID: libc::pid_t = 0;
    static mut SAMPLE_RATE: c_int = 0;
    static mut VOLUME: c_float = 1.0;
    static mut ERRORS: [u8; 160] = [0; 160];
    static mut TRANSPORT: Option<Transport> = None;

    /// The command and stderr buffers of the running child.
    fn transport() -> Option<&'static mut Transport> {
        // SAFETY: serialized engine state.
        unsafe { (*addr_of_mut!(TRANSPORT)).as_mut() }
    }
    /// The transport's front bytes, as `espeak_rs_mbr_front` gave them.
    fn front() -> Option<(*const u8, usize)> {
        let front = transport()?.front();
        (!front.is_empty()).then_some((front.as_ptr(), front.len()))
    }

    fn errors() -> &'static mut [u8; 160] {
        // SAFETY: serialized engine state.
        unsafe { &mut *addr_of_mut!(ERRORS) }
    }
    fn errors_text() -> &'static [u8] {
        let errors = errors();
        &errors[..errors.iter().position(|&b| b == 0).unwrap_or(errors.len())]
    }
    /// `snprintf(errors + at, sizeof - at, "%s", text)`.
    fn set_errors(at: usize, text: &[u8]) {
        let errors = errors();
        let length = text.len().min(errors.len() - 1 - at);
        errors[at..at + length].copy_from_slice(&text[..length]);
        errors[at + length] = 0;
    }
    fn errno() -> c_int {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
    }
    fn strerror(code: c_int) -> Vec<u8> {
        // SAFETY: the C runtime's text for the code, copied at once.
        unsafe { CStr::from_ptr(libc::strerror(code)) }
            .to_bytes()
            .to_vec()
    }
    /// `err`: record the message and print it.
    fn error(message: &[u8]) {
        set_errors(0, message);
        c::error(&[b"mbrowrap error: ", errors_text(), b"\n"]);
    }
    fn error_code(prefix: &[u8], code: c_int) {
        error(&[prefix, &strerror(code)].concat());
    }

    unsafe fn create_pipes(p1: &mut [c_int; 2], p2: &mut [c_int; 2], p3: &mut [c_int; 2]) -> c_int {
        // SAFETY: pipe and close of the descriptors made here.
        unsafe {
            let code;
            if libc::pipe(p1.as_mut_ptr()) != -1 {
                if libc::pipe(p2.as_mut_ptr()) != -1 {
                    if libc::pipe(p3.as_mut_ptr()) != -1 {
                        return 0;
                    }
                    code = errno();
                    libc::close(p2[0]);
                    libc::close(p2[1]);
                } else {
                    code = errno();
                }
                libc::close(p1[0]);
                libc::close(p1[1]);
            } else {
                code = errno();
            }
            error_code(b"pipe(): ", code);
        }
        -1
    }
    unsafe fn close_pipes(p1: &[c_int; 2], p2: &[c_int; 2], p3: &[c_int; 2]) {
        for fd in [p1[0], p1[1], p2[0], p2[1], p3[0], p3[1]] {
            // SAFETY: descriptors made by create_pipes.
            unsafe { libc::close(fd) };
        }
    }
    /// The child's side of `start_mbrola`: only async-signal-safe calls.
    unsafe fn child(
        stdin: &[c_int; 2],
        stdout: &[c_int; 2],
        stderr: &[c_int; 2],
        volume: *const c_char,
        voice: *const c_char,
    ) -> ! {
        // SAFETY: the child's own descriptors and terminated arguments.
        unsafe {
            if libc::dup2(stdin[0], 0) == -1
                || libc::dup2(stdout[1], 1) == -1
                || libc::dup2(stderr[1], 2) == -1
            {
                let text = libc::strerror(errno());
                libc::write(stderr[1], c"dup2(): ".as_ptr().cast(), 8);
                libc::write(stderr[1], text.cast(), libc::strlen(text));
                libc::write(stderr[1], c"\n".as_ptr().cast(), 1);
                libc::_exit(1);
            }
            let mut fd = stderr[1];
            while fd > 2 {
                libc::close(fd);
                fd -= 1;
            }
            for signal in [libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM] {
                libc::signal(signal, libc::SIG_IGN);
            }
            libc::execlp(
                c"mbrola".as_ptr(),
                c"mbrola".as_ptr(),
                c"-e".as_ptr(),
                c"-v".as_ptr(),
                volume,
                voice,
                c"-".as_ptr(),
                c"-.wav".as_ptr(),
                ptr::null::<c_char>(),
            );
            // The exec failed.
            let text = libc::strerror(errno());
            libc::write(2, c"mbrola: ".as_ptr().cast(), 8);
            libc::write(2, text.cast(), libc::strlen(text));
            libc::write(2, c"\n".as_ptr().cast(), 1);
            libc::_exit(1);
        }
    }
    unsafe fn start(voice: *const c_char) -> c_int {
        // SAFETY: serialized engine state; the caller's terminated path.
        unsafe {
            if STATE != INACTIVE {
                error(b"mbrola init request when already initialized");
                return -1;
            }
            if transport().is_none() {
                TRANSPORT = Some(
                    Transport::new(mbrola_transport::COMMAND_CAPACITY).expect("fixed capacity"),
                );
            }
            if let Some(owner) = transport() {
                owner.clear_commands();
                owner.clear_stderr();
            }
            let (mut stdin, mut stdout, mut stderr) = ([0; 2], [0; 2], [0; 2]);
            if create_pipes(&mut stdin, &mut stdout, &mut stderr) != 0 {
                return -1;
            }
            // Everything the child needs is made before the fork.
            let mut volume = [0 as c_char; 20];
            libc::snprintf(
                volume.as_mut_ptr(),
                volume.len(),
                c"%g".as_ptr(),
                f64::from(VOLUME),
            );
            PID = libc::fork();
            if PID == -1 {
                let code = errno();
                close_pipes(&stdin, &stdout, &stderr);
                error_code(b"fork(): ", code);
                return -1;
            }
            if PID == 0 {
                child(&stdin, &stdout, &stderr, volume.as_ptr(), voice);
            }
            let pid = PID;
            let path = CString::new(format!("/proc/{pid}/stat")).expect("no NUL");
            PROC_STAT = libc::open(path.as_ptr(), libc::O_RDONLY);
            if PROC_STAT == -1 {
                let code = errno();
                close_pipes(&stdin, &stdout, &stderr);
                libc::waitpid(PID, ptr::null_mut(), 0);
                PID = 0;
                error_code(b"/proc is unaccessible: ", code);
                return -1;
            }
            libc::signal(libc::SIGPIPE, libc::SIG_IGN);
            if libc::fcntl(stdin[1], libc::F_SETFL, libc::O_NONBLOCK) == -1
                || libc::fcntl(stdout[0], libc::F_SETFL, libc::O_NONBLOCK) == -1
                || libc::fcntl(stderr[0], libc::F_SETFL, libc::O_NONBLOCK) == -1
            {
                let code = errno();
                close_pipes(&stdin, &stdout, &stderr);
                libc::waitpid(PID, ptr::null_mut(), 0);
                PID = 0;
                error_code(b"fcntl(): ", code);
                return -1;
            }
            COMMAND_FD = stdin[1];
            AUDIO_FD = stdout[0];
            ERROR_FD = stderr[0];
            libc::close(stdin[0]);
            libc::close(stdout[1]);
            libc::close(stderr[1]);
            STATE = IDLE;
        }
        0
    }
    unsafe fn stop() {
        // SAFETY: serialized engine state and its descriptors.
        unsafe {
            if STATE == INACTIVE {
                return;
            }
            libc::close(PROC_STAT);
            libc::close(COMMAND_FD);
            libc::close(AUDIO_FD);
            libc::close(ERROR_FD);
            if PID != 0 {
                libc::kill(PID, libc::SIGTERM);
                libc::waitpid(PID, ptr::null_mut(), 0);
                PID = 0;
            }
            STATE = INACTIVE;
        }
    }
    unsafe fn died() -> c_int {
        let mut status = 0;
        // SAFETY: serialized engine state.
        let pid = unsafe { libc::waitpid(PID, &mut status, libc::WNOHANG) };
        let message: Vec<u8> = if pid == 0 {
            b"mbrola closed stderr and did not exit".to_vec()
        // SAFETY: as above.
        } else if pid != unsafe { PID } {
            b"waitpid() is confused".to_vec()
        } else {
            // SAFETY: as above.
            unsafe { PID = 0 };
            if libc::WIFSIGNALED(status) {
                format!("mbrola died by signal {}", libc::WTERMSIG(status)).into_bytes()
            } else if libc::WIFEXITED(status) {
                format!("mbrola exited with status {}", libc::WEXITSTATUS(status)).into_bytes()
            } else {
                b"mbrola died and wait status is weird".to_vec()
            }
        };
        let message = &message[..message.len().min(79)]; // msgbuf[80]
        c::error(&[b"mbrowrap error: ", message, b"\n"]);
        let length = errors_text().len();
        if length == 0 {
            set_errors(0, message);
        } else if length < 159 {
            set_errors(length, &[b", (", message, b")"].concat());
        }
        -1
    }
    unsafe fn has_errors() -> c_int {
        let mut buffer = [0u8; 256];
        // SAFETY: serialized engine state; one bounded read.
        unsafe {
            let result = libc::read(ERROR_FD, buffer.as_mut_ptr().cast(), buffer.len());
            if result < 0 {
                let code = errno();
                if code == libc::EAGAIN || code == libc::EINTR {
                    return 0;
                }
                error_code(b"read(error): ", code);
                return -1;
            }
            let mut messages = 0;
            if let Some(owner) = transport() {
                messages = owner.stderr(&buffer[..result as usize], result == 0);
                if messages != 0 {
                    set_errors(0, owner.error());
                }
            }
            if messages > 0 {
                c::error(&[b"mbrola: ", errors_text(), b"\n"]);
            }
            if result == 0 {
                died()
            } else {
                0
            }
        }
    }
    /// At most one nonblocking write; the transport keeps an unwritten suffix.
    unsafe fn drain() -> c_int {
        let Some((bytes, length)) = front() else {
            return 0;
        };
        // SAFETY: serialized engine state; the transport's front bytes.
        let result = unsafe { libc::write(COMMAND_FD, bytes.cast(), length) };
        if result < 0 {
            let code = errno();
            if code == libc::EAGAIN || code == libc::EINTR {
                return 0;
            }
            // SAFETY: serialized engine state.
            if code == libc::EPIPE && unsafe { has_errors() } != 0 {
                return -1;
            }
            error_code(b"write(): ", code);
            return -1;
        }
        match transport().map(|owner| owner.consume(result as usize)) {
            Some(Ok(())) => 0,
            _ => -1,
        }
    }
    unsafe fn send(command: &[u8]) -> c_int {
        // SAFETY: serialized engine state.
        unsafe {
            if PID == 0 || transport().is_none() {
                return -1;
            }
            if command.len() > c_int::MAX as usize {
                error(b"MBROLA command is too large");
                return -1;
            }
            // Queue before writing: newer bytes never bypass an older suffix.
            if transport().is_some_and(|owner| owner.queue(command).is_err()) {
                return 0;
            }
            if drain() < 0 {
                return -1;
            }
        }
        command.len() as c_int
    }
    /// Whether /proc shows mbrola sleeping ("12345 (mbrola) S").
    unsafe fn is_idle() -> bool {
        let mut buffer = [0u8; 20];
        // SAFETY: serialized engine state; one bounded read.
        unsafe {
            if libc::lseek(PROC_STAT, 0, libc::SEEK_SET) != 0 {
                return false;
            }
            if libc::read(PROC_STAT, buffer.as_mut_ptr().cast(), buffer.len())
                != buffer.len() as isize
            {
                return false;
            }
        }
        match buffer.iter().position(|&b| b == b')') {
            Some(p) if p < buffer.len() - 2 => buffer[p + 1] == b' ' && buffer[p + 2] == b'S',
            _ => false,
        }
    }
    unsafe fn receive(buffer: *mut u8, size: usize) -> isize {
        let mut wait = 1;
        let mut filled = 0usize;
        // SAFETY: serialized engine state; the caller's `size` bytes.
        unsafe {
            if PID == 0 {
                return -1;
            }
            loop {
                let mut fds = [
                    libc::pollfd {
                        fd: AUDIO_FD,
                        events: libc::POLLIN,
                        revents: 0,
                    },
                    libc::pollfd {
                        fd: ERROR_FD,
                        events: libc::POLLIN,
                        revents: 0,
                    },
                    libc::pollfd {
                        fd: COMMAND_FD,
                        events: libc::POLLOUT,
                        revents: 0,
                    },
                ];
                let count = if front().is_some() { 3 } else { 2 };
                // A sleeping child may still need commands the transport keeps.
                let idle = is_idle() && count != 3;
                let result = libc::poll(fds.as_mut_ptr(), count, if idle { 0 } else { wait });
                if result == -1 {
                    error_code(b"poll(): ", errno());
                    return -1;
                }
                if result == 0 {
                    if idle {
                        STATE = IDLE;
                        break;
                    }
                    if wait >= 5000 * (4 - 1) / 4 {
                        STATE = WEDGED;
                        error(b"mbrola process is stalled");
                        break;
                    }
                    wait *= 4;
                    continue;
                }
                wait = 1;
                if fds[1].revents != 0 && has_errors() != 0 {
                    return -1;
                }
                if count == 3 && fds[2].revents != 0 && drain() < 0 {
                    return -1;
                }
                if fds[0].revents != 0 {
                    let obtained = libc::read(AUDIO_FD, buffer.add(filled).cast(), size - filled);
                    if obtained == -1 {
                        error_code(b"read(): ", errno());
                        return -1;
                    }
                    filled += obtained as usize;
                    STATE = AUDIO;
                }
                if filled >= size {
                    break;
                }
            }
        }
        filled as isize
    }

    unsafe extern "system" fn init(voice: *mut c_char) -> c_int {
        let mut header = [0u8; 45];
        // SAFETY: serialized engine state; the caller's terminated path.
        unsafe {
            if start(voice) != 0 {
                return -1;
            }
            // Give mbrola time to start under the Windows Subsystem for Linux.
            libc::usleep(100);
            if send(b"#\n") != 2 {
                stop();
                return -1;
            }
            let result = receive(header.as_mut_ptr(), 45);
            if result != 44 {
                if result >= 0 {
                    error(b"unable to get .wav header from mbrola");
                }
                stop();
                return -1;
            }
            SAMPLE_RATE = mbrola_transport::sample_rate(&header[..44]).unwrap_or(-1);
            if SAMPLE_RATE < 0 {
                error(b"mbrola did not return a .wav header with a valid sample rate");
                stop();
                return -1;
            }
            // Remember the voice path for setVolumeRatio_MBR.
            let same = (*addr_of_mut!(VOICE_PATH))
                .as_ref()
                .is_some_and(|path| path.as_ptr() == voice.cast_const());
            if !same {
                VOICE_PATH = Some(CStr::from_ptr(voice).to_owned());
            }
        }
        0
    }
    unsafe extern "system" fn close() {
        // SAFETY: serialized engine state.
        unsafe {
            stop();
            TRANSPORT = None;
            VOICE_PATH = None;
            VOLUME = 1.0;
        }
    }
    unsafe extern "system" fn reset() {
        let mut ok = true;
        let mut scratch = [0u8; 4096];
        // SAFETY: serialized engine state and its descriptors.
        unsafe {
            if STATE == IDLE || PID == 0 {
                return;
            }
            if libc::kill(PID, libc::SIGUSR1) == -1 {
                ok = false;
            }
            if let Some(owner) = transport() {
                owner.clear_commands();
            }
            if send(b"\n#\n") != 3 {
                ok = false;
            }
            let mut result;
            loop {
                result = libc::read(AUDIO_FD, scratch.as_mut_ptr().cast(), scratch.len());
                if result <= 0 {
                    break;
                }
            }
            if result != -1 || errno() != libc::EAGAIN {
                ok = false;
            }
            if has_errors() == 0 && ok {
                STATE = IDLE;
            }
        }
    }
    unsafe extern "system" fn read(buffer: *mut c_short, samples: c_int) -> c_int {
        // SAFETY: the caller's `samples` samples.
        let result = unsafe { receive(buffer.cast(), samples as usize * 2) } as c_int;
        if result > 0 {
            result / 2
        } else {
            result
        }
    }
    unsafe extern "system" fn write(data: *mut c_char) -> c_int {
        // SAFETY: serialized engine state; the caller's terminated text.
        unsafe {
            STATE = NEW_DATA;
            send(CStr::from_ptr(data).to_bytes())
        }
    }
    unsafe extern "system" fn flush() -> c_int {
        // SAFETY: serialized engine state.
        c_int::from(unsafe { send(b"\n#\n") } == 3)
    }
    unsafe extern "system" fn frequency() -> c_int {
        // SAFETY: serialized engine scalar.
        unsafe { SAMPLE_RATE }
    }
    unsafe extern "system" fn volume(value: c_float) {
        // SAFETY: serialized engine state.
        unsafe {
            if value == VOLUME {
                return;
            }
            VOLUME = value;
            if STATE != IDLE {
                return;
            }
            // mbrola takes the volume as an argument: restart it.
            stop();
            let path = (*addr_of_mut!(VOICE_PATH))
                .as_ref()
                .map_or(ptr::null_mut(), |path| path.as_ptr().cast_mut());
            if let Some(init) = init_MBR {
                init(path);
            }
        }
    }
    unsafe extern "system" fn last_error(buffer: *mut c_char, size: c_int) -> *mut c_char {
        // SAFETY: serialized engine state; the caller's `size` bytes.
        unsafe {
            if PID != 0 {
                has_errors();
            }
            if size > 0 {
                let text = errors_text();
                let length = text.len().min(size as usize - 1);
                ptr::copy_nonoverlapping(text.as_ptr(), buffer.cast(), length);
                *buffer.add(length) = 0;
            }
        }
        buffer
    }
    unsafe extern "system" fn no_error(_: c_int) {}

    pub(super) unsafe fn load() -> c_int {
        // SAFETY: serialized engine state.
        unsafe {
            init_MBR = Some(init);
            close_MBR = Some(close);
            reset_MBR = Some(reset);
            read_MBR = Some(read);
            write_MBR = Some(write);
            flush_MBR = Some(flush);
            getFreq_MBR = Some(frequency);
            setVolumeRatio_MBR = Some(volume);
            lastErrorStr_MBR = Some(last_error);
            setNoError_MBR = Some(no_error);
        }
        1
    }
    pub(super) unsafe fn unload() {}
}
