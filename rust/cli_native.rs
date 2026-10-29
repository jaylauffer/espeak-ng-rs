//! Complete native command-line controller. The thin executable configuration
//! supplies the host's getopt and CRT streams; the public engine ABI remains
//! the integration surface while its remaining native hosts are migrated.
//! SPDX-License-Identifier: GPL-3.0-or-later
use crate::async_command::{Voice, Wide};
use crate::cli_io::{self, WaveOutput};
use crate::events::Event;
use std::ffi::{c_char, c_int, c_long, c_void, CStr};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

#[repr(C)]
struct LongOption {
    name: *const c_char,
    argument: c_int,
    flag: *mut c_int,
    value: c_int,
}
impl LongOption {
    fn new(name: &'static CStr, argument: i32, value: i32) -> Self {
        Self {
            name: name.as_ptr(),
            argument,
            flag: ptr::null_mut(),
            value,
        }
    }
}
/// The platform's `getopt_long` (src/compat/getopt.c with MSVC), which keeps
/// its permutation, diagnostics and optional-argument rules.
type GetoptLong = unsafe extern "C" fn(
    c_int,
    *mut *mut c_char,
    *const c_char,
    *const LongOption,
    *mut c_int,
) -> c_int;

#[repr(C)]
pub struct Config {
    playback_mode: c_int,
    mbrola: c_int,
    program_name: *const c_char,
    default_voice: *const c_char,
    input: *mut libc::FILE,
    output: *mut libc::FILE,
    error: *mut libc::FILE,
    getopt_long: GetoptLong,
    /// That getopt's `optarg` and `optind`.
    optarg: *mut *mut c_char,
    optind: *mut c_int,
}
const _: () = assert!(size_of::<Config>() == 8 + size_of::<[usize; 8]>());
const _: () = assert!(std::mem::offset_of!(Config, optind) == 8 + 7 * size_of::<usize>());
/// Standard output in binary mode on Windows.
unsafe extern "C" fn binary_output(file: *mut libc::FILE) {
    // SAFETY: the configuration's live standard output stream.
    unsafe { crate::crt::binary_output(file) }
}
const _: () = assert!(size_of::<LongOption>() == 4 * size_of::<usize>());
const _: () = assert!(std::mem::offset_of!(LongOption, value) == 3 * size_of::<usize>());
const _: () = assert!(size_of::<Event>() == 32 + size_of::<usize>());
const _: () = assert!(std::mem::offset_of!(Event, id) == 24 + size_of::<usize>());
const _: () = assert!(size_of::<Voice>() == 8 + 4 * size_of::<usize>());
const _: () = assert!(std::mem::offset_of!(Voice, spare) == 8 + 3 * size_of::<usize>());

extern "C" {
    fn espeak_Initialize(output: i32, length: i32, path: *const c_char, options: i32) -> i32;
    fn espeak_Info(path: *mut *const c_char) -> *const c_char;
    fn espeak_ListVoices(selector: *mut Voice) -> *const *const Voice;
    fn espeak_SetSynthCallback(
        callback: Option<unsafe extern "C" fn(*mut i16, i32, *mut Event) -> i32>,
    );
    fn espeak_SetParameter(parameter: i32, value: i32, relative: i32) -> i32;
    fn espeak_SetPunctuationList(punctuation: *const Wide) -> i32;
    fn espeak_SetPhonemeTrace(mode: i32, stream: *mut libc::FILE);
    fn espeak_Synth(
        text: *const c_void,
        length: usize,
        position: u32,
        position_type: i32,
        end: u32,
        flags: u32,
        identifier: *mut u32,
        user: *mut c_void,
    ) -> i32;
    fn espeak_ng_InitializePath(path: *const c_char);
    fn espeak_ng_Initialize(context: *mut *mut c_void) -> i32;
    fn espeak_ng_InitializeOutput(mode: i32, length: i32, device: *const c_char) -> i32;
    fn espeak_ng_GetSampleRate() -> i32;
    fn espeak_ng_SetRandSeed(seed: c_long) -> i32;
    fn espeak_ng_SetVoiceByName(name: *const c_char) -> i32;
    fn espeak_ng_SetVoiceByFile(path: *const c_char) -> i32;
    fn espeak_ng_SetVoiceByProperties(selector: *mut Voice) -> i32;
    fn espeak_ng_Synchronize() -> i32;
    fn espeak_ng_Terminate() -> i32;
    fn espeak_ng_PrintStatusCodeMessage(status: i32, output: *mut libc::FILE, context: *mut c_void);
    fn espeak_ng_ClearErrorContext(context: *mut *mut c_void);
    fn espeak_ng_CompileDictionary(
        source: *const c_char,
        name: *const c_char,
        output: *mut libc::FILE,
        flags: i32,
        context: *mut *mut c_void,
    ) -> i32;
    fn espeak_ng_CompileMbrolaVoice(
        path: *const c_char,
        output: *mut libc::FILE,
        context: *mut *mut c_void,
    ) -> i32;
    fn espeak_ng_CompileIntonation(output: *mut libc::FILE, context: *mut *mut c_void) -> i32;
    fn espeak_ng_CompilePhonemeData(
        rate: c_long,
        output: *mut libc::FILE,
        context: *mut *mut c_void,
    ) -> i32;
    fn espeak_ng_CompilePhonemeDataPath(
        rate: c_long,
        source: *const c_char,
        destination: *const c_char,
        output: *mut libc::FILE,
        context: *mut *mut c_void,
    ) -> i32;
}

fn wave() -> &'static Mutex<Option<WaveOutput>> {
    static WAVE: OnceLock<Mutex<Option<WaveOutput>>> = OnceLock::new();
    WAVE.get_or_init(|| Mutex::new(None))
}
static RUNNING: AtomicBool = AtomicBool::new(false);
struct Admission;
impl Drop for Admission {
    fn drop(&mut self) {
        *wave().lock().unwrap_or_else(|p| p.into_inner()) = None;
        RUNNING.store(false, Ordering::Release);
    }
}

unsafe extern "C" fn synth_callback(samples: *mut i16, count: i32, events: *mut Event) -> i32 {
    // The engine publishes a terminated event array and a buffer containing
    // count samples for the duration of this synchronous callback.
    if samples.is_null() {
        return 0;
    }
    let Ok(mut output) = wave().try_lock() else {
        return 1;
    };
    let Some(output) = output.as_mut() else {
        return 1;
    };
    if count < 0 || events.is_null() {
        return 1;
    }
    // SAFETY: engine callback contract above; the array terminator is part of
    // its initialized allocation. No engine call or foreign mutation occurs
    // while either borrowed view is retained.
    unsafe {
        let mut length = 0;
        while (*events.add(length)).kind != 0 {
            length += 1;
        }
        output.callback(
            std::slice::from_raw_parts(samples, count as usize),
            std::slice::from_raw_parts(events, length),
        )
    }
}

/// Borrow one native NUL-terminated string, without changing its byte encoding.
unsafe fn bytes<'a>(value: *const c_char) -> &'a [u8] {
    if value.is_null() {
        return &[];
    }
    // SAFETY: caller supplies a readable, NUL-terminated string for the loan.
    unsafe { CStr::from_ptr(value).to_bytes() }
}
unsafe fn put(output: *mut libc::FILE, text: &[u8]) {
    // SAFETY: the process configuration/trace owner retains this CRT stream;
    // fwrite consumes the initialized bytes synchronously without retaining.
    unsafe {
        libc::fwrite(text.as_ptr().cast(), 1, text.len(), output);
    }
}
fn clipped<const N: usize>(output: &mut [u8; N], input: &[u8]) {
    let length = input.len().min(N - 1);
    output[..length].copy_from_slice(&input[..length]);
    output[length] = 0;
}
fn text(buffer: &[u8]) -> &[u8] {
    &buffer[..buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len())]
}
fn selector(language: *const c_char) -> Voice {
    Voice {
        name: ptr::null(),
        languages: language,
        identifier: ptr::null(),
        gender: 0,
        age: 0,
        variant: 0,
        internal: 0,
        score: 0,
        spare: ptr::null_mut(),
    }
}
unsafe fn version(config: &Config) {
    // SAFETY: the public API accepts NULL path and supplies borrowed static
    // version/path strings. Config streams outlive this entrypoint.
    unsafe {
        espeak_Initialize(2, 0, ptr::null(), 0x8000);
        let mut path = ptr::null();
        let version = espeak_Info(&mut path);
        put(config.output, b"eSpeak NG text-to-speech: ");
        put(config.output, bytes(version));
        put(config.output, b"  Data at: ");
        put(config.output, bytes(path));
        put(config.output, b"\n");
    }
}

fn padded(output: &mut Vec<u8>, value: &[u8], width: usize) {
    output.extend_from_slice(value);
    output.resize(output.len() + width.saturating_sub(value.len()), b' ');
}
unsafe fn display_voices(config: &Config, language: *const c_char) {
    // SAFETY: the public API owns a terminated table of initialized voices,
    // strings, and priority/language byte pairs. It remains unchanged while
    // this display borrows it. CRT writes do not reenter the engine.
    unsafe {
        let mut select = selector(language);
        let voices = espeak_ListVoices(if bytes(language).is_empty() {
            ptr::null_mut()
        } else {
            &mut select
        });
        put(config.output, b"Pty Language       Age/Gender VoiceName          File                 Other Languages\n");
        if voices.is_null() {
            return;
        }
        let mut index = 0;
        let mut line = Vec::with_capacity(256);
        while !(*voices.add(index)).is_null() {
            let voice = &**voices.add(index);
            let mut languages = voice.languages;
            let mut first = true;
            line.clear();
            while !languages.is_null() && *languages != 0 {
                let priority = *languages as i32;
                let language = bytes(languages.add(1));
                if first {
                    line.extend_from_slice(format!("{priority:2}  ").as_bytes());
                    padded(&mut line, language, 15);
                    if voice.age == 0 {
                        line.extend_from_slice(b" --");
                    } else {
                        line.extend_from_slice(format!("{:3}", voice.age).as_bytes());
                    }
                    line.push(b'/');
                    line.push(*b"-MF-".get(voice.gender as usize).unwrap_or(&b'-'));
                    line.extend_from_slice(b"      ");
                    let name = bytes(voice.name);
                    let at = line.len();
                    line.extend(
                        name.iter()
                            .take(80)
                            .map(|&c| if c == b' ' { b'_' } else { c }),
                    );
                    let length = line.len() - at;
                    line.resize(line.len() + 18usize.saturating_sub(length), b' ');
                    line.push(b' ');
                    padded(&mut line, bytes(voice.identifier), 20);
                    line.push(b' ');
                } else {
                    line.push(b'(');
                    line.extend_from_slice(language);
                    line.extend_from_slice(format!(" {priority})").as_bytes());
                }
                languages = languages.add(language.len() + 2);
                first = false;
            }
            line.push(b'\n');
            put(config.output, &line);
            index += 1;
        }
    }
}

struct Trace {
    stream: *mut libc::FILE,
    standard: *mut libc::FILE,
}
impl Trace {
    fn close(&mut self) {
        if !self.stream.is_null() && self.stream != self.standard {
            // SAFETY: only successful fopen results are owned here. The
            // engine has synchronized before normal close; early exits have
            // not submitted speech. Closing NULL is explicitly refused.
            unsafe { libc::fclose(self.stream) };
        }
        self.stream = self.standard;
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        self.close();
    }
}

unsafe fn compiler_result(config: &Config, status: i32, context: &mut *mut c_void) -> i32 {
    if status == 0 {
        return 0;
    }
    // SAFETY: the compiler created this context; API consumes only its own
    // allocation and clears the caller's pointer. Error stream is borrowed.
    unsafe {
        espeak_ng_PrintStatusCodeMessage(status, config.error, *context);
        espeak_ng_ClearErrorContext(context);
    }
    1
}

fn options(mbrola: bool) -> Vec<LongOption> {
    let mut options = vec![
        LongOption::new(c"help", 0, b'h'.into()),
        LongOption::new(c"stdin", 0, 0x100),
        LongOption::new(c"compile-debug", 2, 0x101),
        LongOption::new(c"compile", 2, 0x102),
        LongOption::new(c"punct", 2, 0x103),
        LongOption::new(c"voices", 2, 0x104),
        LongOption::new(c"stdout", 0, 0x105),
        LongOption::new(c"split", 2, 0x106),
        LongOption::new(c"path", 1, 0x107),
        LongOption::new(c"phonout", 1, 0x108),
        LongOption::new(c"pho", 0, 0x109),
        LongOption::new(c"ipa", 2, 0x10a),
        LongOption::new(c"version", 0, 0x10b),
        LongOption::new(c"sep", 2, 0x10c),
        LongOption::new(c"tie", 2, 0x10d),
    ];
    if mbrola {
        options.push(LongOption::new(c"compile-mbrola", 2, 0x10e));
    }
    options.extend([
        LongOption::new(c"compile-intonations", 0, 0x10f),
        LongOption::new(c"compile-phonemes", 2, 0x110),
        LongOption::new(c"load", 0, 0x111),
        LongOption::new(c"ssml-break", 1, 0x112),
        LongOption {
            name: ptr::null(),
            argument: 0,
            flag: ptr::null_mut(),
            value: 0,
        },
    ]);
    options
}

/// The executable supplies the process argv and immutable platform descriptor.
///
/// # Safety
/// `argv` contains `argc` valid NUL-terminated arguments plus its NULL terminator
/// and may be permuted by getopt. Config strings, streams and callbacks remain
/// live for this entire call. The public engine is otherwise idle. The native
/// process calls this once; concurrent/reentrant admission is refused.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_cli_main(
    argc: c_int,
    argv: *mut *mut c_char,
    config: *const Config,
) -> c_int {
    if argc < 0 || argv.is_null() || config.is_null() {
        return 1;
    }
    if RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return 1;
    }
    let _admission = Admission;
    // SAFETY: the caller's descriptor and argv contract is checked at this
    // native boundary. run keeps all transient strings alive across API calls.
    unsafe {
        let config = &*config;
        if config.input.is_null() || config.output.is_null() || config.error.is_null() {
            return 1;
        }
        run(argc, argv, config)
    }
}

unsafe fn run(argc: i32, argv: *mut *mut c_char, config: &Config) -> i32 {
    let options = options(config.mbrola != 0);
    let mut option_index = 0;
    let mut position;
    let mut argument;
    let mut data_path = ptr::null();
    let mut filename = [0; 200];
    let mut voicename = [0; 40];
    let mut devicename = [0; 200];
    let mut wavefile = [0; 200];
    let mut punctuation = [0 as Wide; 100];
    let mut stdin_mode = false;
    let mut compile = 0;
    let mut load = false;
    let mut flags: i32 = 0x100 | 0x1000;
    let (mut volume, mut speed, mut pitch, mut range) = (-1, -1, -1, -1);
    let (mut wordgap, mut capitals, mut punct, mut ssml_break) = (-1, -1, -1, -1);
    let (mut separator, mut phonemes, mut linelength) = (0i32, 0i32, 0i32);
    let (mut waveout, mut quiet, mut deterministic) = (false, false, false);
    let mut split_seconds = 0u32;
    let mut trace = Trace {
        stream: config.output,
        standard: config.output,
    };

    // SAFETY: entrypoint retains argv and its strings while platform getopt
    // permutes only argv slots. Required arguments are supplied by getopt;
    // optional NULL arguments are checked before conversion. Every engine
    // call receives initialized ABI objects or live borrowed CRT streams.
    unsafe {
        loop {
            let option = (config.getopt_long)(
                argc,
                argv,
                c"a:b:Dd:f:g:hk:l:mp:P:qs:v:w:xXz".as_ptr(),
                options.as_ptr(),
                &mut option_index,
            );
            argument = *config.optarg;
            position = *config.optind;
            if option == -1 {
                break;
            }
            match option {
                98 => {
                    // -b keeps sscanf's numeric-prefix and <=4 behavior.
                    let mut value = 0i32;
                    if crate::crt::scan(argument, c"%d", &[std::ptr::addr_of_mut!(value).cast()])
                        == 1
                        && value <= 4
                    {
                        flags |= value;
                    } else {
                        flags |= 2;
                    }
                }
                100 => clipped(&mut devicename, bytes(argument)),
                68 => deterministic = true,
                104 => {
                    put(config.output, b"\n");
                    version(config);
                    help(config);
                    return 0;
                }
                107 => capitals = libc::atoi(argument),
                120 => phonemes |= 1,
                88 => phonemes |= 8,
                109 => flags |= 0x10,
                112 => pitch = libc::atoi(argument),
                80 => range = libc::atoi(argument),
                113 => quiet = true,
                102 => clipped(&mut filename, bytes(argument)),
                108 => linelength = libc::atoi(argument),
                97 => volume = libc::atoi(argument),
                115 => speed = libc::atoi(argument),
                103 => wordgap = libc::atoi(argument),
                118 => clipped(&mut voicename, bytes(argument)),
                119 => {
                    waveout = true;
                    clipped(&mut wavefile, bytes(argument));
                }
                122 => flags &= !0x1000,
                0x100 => stdin_mode = true,
                0x105 => {
                    waveout = true;
                    clipped(&mut wavefile, b"stdout");
                }
                0x101 | 0x102 => {
                    if bytes(argument).is_empty() {
                        put(
                            config.error,
                            if option == 0x101 {
                                b"Voice name to '--compile-debug' not specified.\n"
                            } else {
                                b"Voice name to '--compile' not specified.\n"
                            },
                        );
                        return 1;
                    }
                    clipped(&mut voicename, bytes(argument));
                    compile = option;
                    quiet = true;
                }
                0x103 => {
                    punct = 1;
                    if !argument.is_null() {
                        // Previous CLI widens native char bytes, not Unicode.
                        for (out, &byte) in punctuation.iter_mut().take(99).zip(bytes(argument)) {
                            *out = (byte as c_char) as Wide;
                        }
                        let end = bytes(argument).len().min(99);
                        punctuation[end] = 0;
                        punct = 2;
                    }
                }
                0x104 => {
                    espeak_Initialize(2, 0, data_path, 0);
                    display_voices(config, argument);
                    return 0;
                }
                0x106 => {
                    split_seconds = if argument.is_null() {
                        30 * 60
                    } else {
                        libc::atoi(argument).wrapping_mul(60) as u32
                    }
                }
                0x107 => data_path = argument,
                0x108 => {
                    trace.close();
                    trace.stream = libc::fopen(argument, c"w".as_ptr());
                    if trace.stream.is_null() {
                        put(config.error, b"Can't write to: ");
                        put(config.error, bytes(argument));
                        put(config.error, b"\n");
                    }
                }
                0x109 => phonemes |= 16,
                0x10a => {
                    phonemes |= 2;
                    if !argument.is_null() {
                        match libc::atoi(argument) {
                            1 => separator = b'_' as i32,
                            2 => {
                                separator = 0x361;
                                phonemes |= 128;
                            }
                            3 => {
                                separator = 0x200d;
                                phonemes |= 128;
                            }
                            _ => {}
                        }
                    }
                }
                0x10b => {
                    version(config);
                    return 0;
                }
                0x10c | 0x10d => {
                    phonemes |= 1;
                    let tie = option == 0x10d;
                    if tie {
                        phonemes |= 128;
                    }
                    separator = if argument.is_null() {
                        if tie {
                            0x361
                        } else {
                            b' ' as i32
                        }
                    } else {
                        crate::utf8::decode(CStr::from_ptr(argument).to_bytes_with_nul(), 0, false)
                            .map_or(0, |character| character.code as i32)
                    };
                    if separator == b'z' as i32 {
                        separator = if tie { 0x200d } else { 0x200c };
                    }
                }
                0x10e..=0x110 => {
                    espeak_ng_InitializePath(data_path);
                    let mut context = ptr::null_mut();
                    let status = match option {
                        0x10e => {
                            espeak_ng_CompileMbrolaVoice(argument, config.output, &mut context)
                        }
                        0x10f => espeak_ng_CompileIntonation(config.output, &mut context),
                        _ if argument.is_null() => {
                            espeak_ng_CompilePhonemeData(22050, config.output, &mut context)
                        }
                        _ => espeak_ng_CompilePhonemeDataPath(
                            22050,
                            argument,
                            ptr::null(),
                            config.output,
                            &mut context,
                        ),
                    };
                    return compiler_result(config, status, &mut context);
                }
                0x111 => load = true,
                0x112 => ssml_break = libc::atoi(argument),
                _ => return 0, // getopt already printed its platform diagnostic.
            }
        }

        espeak_ng_InitializePath(data_path);
        let mut context = ptr::null_mut();
        let status = espeak_ng_Initialize(&mut context);
        if status != 0 {
            return compiler_result(config, status, &mut context);
        }
        if deterministic {
            espeak_ng_SetRandSeed(1);
        }
        let device = if devicename[0] == 0 {
            ptr::null()
        } else {
            devicename.as_ptr().cast()
        };
        let status = espeak_ng_InitializeOutput(
            if waveout || quiet {
                1
            } else {
                config.playback_mode
            },
            0,
            device,
        );
        let rate = espeak_ng_GetSampleRate();
        if waveout || quiet {
            *wave().lock().unwrap_or_else(|p| p.into_inner()) = Some(WaveOutput::new(
                quiet,
                text(&wavefile).to_vec(),
                split_seconds,
                rate,
                config.output,
                config.error,
                binary_output,
            ));
            espeak_SetSynthCallback(Some(synth_callback));
        }
        if status != 0 {
            espeak_ng_PrintStatusCodeMessage(status, config.error, ptr::null_mut());
            return 1;
        }

        if voicename[0] == 0 {
            clipped(&mut voicename, bytes(config.default_voice));
        }
        let mut status = if load {
            espeak_ng_SetVoiceByFile(voicename.as_ptr().cast())
        } else {
            espeak_ng_SetVoiceByName(voicename.as_ptr().cast())
        };
        if status != 0 {
            let mut select = selector(voicename.as_ptr().cast());
            status = espeak_ng_SetVoiceByProperties(&mut select);
            if status != 0 {
                espeak_ng_PrintStatusCodeMessage(status, config.error, ptr::null_mut());
                return 1;
            }
        }
        if compile != 0 {
            let mut context = ptr::null_mut();
            let status = espeak_ng_CompileDictionary(
                c"".as_ptr(),
                ptr::null(),
                config.error,
                compile & 1,
                &mut context,
            );
            return compiler_result(config, status, &mut context);
        }
        for (parameter, value, minimum) in [
            (1, speed, 1),
            (2, volume, 0),
            (3, pitch, 0),
            (4, range, 0),
            (6, capitals, 0),
            (5, punct, 0),
            (7, wordgap, 0),
            (13, linelength, 1),
            (10, ssml_break, 1),
        ] {
            if value >= minimum {
                espeak_SetParameter(parameter, value, 0);
            }
        }
        if punct == 2 {
            espeak_SetPunctuationList(punctuation.as_ptr());
        }
        espeak_SetPhonemeTrace(phonemes | separator.wrapping_shl(8), trace.stream);
        let positional = if position >= 0 && position < argc {
            Some(bytes(*argv.add(position as usize)))
        } else {
            None
        };
        let result = cli_io::run_input(
            text(&filename),
            stdin_mode,
            positional,
            config.input,
            config.output,
            config.error,
            |input, length| {
                // Input owns initialized storage including its terminating
                // wide NUL tail through synchronous call/async admission copy.
                espeak_Synth(
                    input.as_ptr().cast(),
                    length,
                    0,
                    1,
                    0,
                    flags as u32,
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
            },
        );
        if let Err(status) = result {
            if status != -1 {
                espeak_ng_PrintStatusCodeMessage(status, config.error, ptr::null_mut());
            }
            return 1;
        }
        let status = espeak_ng_Synchronize();
        if status != 0 {
            espeak_ng_PrintStatusCodeMessage(status, config.error, ptr::null_mut());
            return 1;
        }
        trace.close();
        let output = wave().lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(mut output) = output {
            let _ = output.finish();
        }
        espeak_ng_Terminate();
        0
    }
}

unsafe fn help(config: &Config) {
    // SAFETY: immutable process config owns program name and borrowed stdout.
    unsafe {
        put(config.output, b"\n");
        put(config.output, bytes(config.program_name));
        put(config.output, HELP_BEFORE_MBROLA);
        if config.mbrola != 0 {
            put(config.output, HELP_MBROLA);
        }
        put(config.output, HELP_AFTER_MBROLA);
    }
}

const HELP_BEFORE_MBROLA: &[u8] = b" [options] [\"<words>\"]\n\n\
-f <text file>   Text file to speak\n\
--stdin    Read text input from stdin at once till to the end of a stream.\n\n\
If neither -f nor --stdin are provided, then <words> from arguments are spoken,\n\
or text is spoken from stdin, read separately one line by line at a time.\n\n\
-a <integer>\n\
\t   Amplitude, 0 to 200, default is 100\n\
-d <device>\n\
\t   Use the specified device to speak the audio on. If not specified, the\n\
\t   default audio device is used.\n\
-g <integer>\n\
\t   Word gap. Pause between words, units of 10mS at the default speed\n\
-k <integer>\n\
\t   Indicate capital letters with: 1=sound, 2=the word \"capitals\",\n\
\t   higher values indicate a pitch increase (try -k20).\n\
-l <integer>\n\
\t   Line length. If not zero (which is the default), consider\n\
\t   lines less than this length as end-of-clause\n\
-p <integer>\n\
\t   Pitch adjustment, 0 to 99, default is 50\n\
-P <integer>\n\
\t   Pitch range adjustment, 0 to 99, default is 50\n\
-s <integer>\n\
\t   Speed in approximate words per minute. The default is 175\n\
-v <voice name>\n\
\t   Use voice file of this name from espeak-ng-data/voices\n\
-w <wave file name>\n\
\t   Write speech to this WAV file, rather than speaking it directly\n\
-b\t   Input text encoding, 1=UTF8, 2=8 bit, 4=16 bit \n\
-m\t   Interpret SSML markup, and ignore other < > tags\n\
--ssml-break=<percentage>\n\
\t   Set SSML break time multiplier, default is 100\n\
-q\t   Quiet, don't produce any speech (may be useful with -x)\n\
-x\t   Write phoneme mnemonics to stdout\n\
-X\t   Write phonemes mnemonics and translation trace to stdout\n\
-z\t   No final sentence pause at the end of the text\n\
-D\t   Enable deterministic random mode\n\
--compile=<voice name>\n\
\t   Compile pronunciation rules and dictionary from the current\n\
\t   directory. <voice name> specifies the language\n\
--compile-debug=<voice name>\n\
\t   Compile pronunciation rules and dictionary from the current\n\
\t   directory, including line numbers for use with -X.\n\
\t   <voice name> specifies the language\n";
const HELP_MBROLA: &[u8] = b"--compile-mbrola=<voice name>\n\
\t   Compile an MBROLA voice\n";
const HELP_AFTER_MBROLA: &[u8] = b"--compile-intonations\n\
\t   Compile the intonation data\n\
--compile-phonemes=<phsource-dir>\n\
\t   Compile the phoneme data using <phsource-dir> or the default phsource directory\n\
--ipa      Write phonemes to stdout using International Phonetic Alphabet\n\
--path=\"<path>\"\n\
\t   Specifies the directory containing the espeak-ng-data directory\n\
--pho      Write mbrola phoneme data (.pho) to stdout or to the file in --phonout\n\
--phonout=\"<filename>\"\n\
\t   Write phoneme output from -x -X --ipa and --pho to this file\n\
--punct=\"<characters>\"\n\
\t   Speak the names of punctuation characters during speaking.  If\n\
\t   =<characters> is omitted, all punctuation is spoken.\n\
--sep=<character>\n\
\t   Separate phonemes (from -x --ipa) with <character>.\n\
\t   Default is space, z means ZWJN character.\n\
--split=<minutes>\n\
\t   Starts a new WAV file every <minutes>.  Used with -w\n\
--stdout   Write speech output to stdout\n\
--tie=<character>\n\
\t   Use a tie character within multi-letter phoneme names.\n\
\t   Default is U+361, z means ZWJ character.\n\
--version  Shows version number and date, and location of espeak-ng-data\n\
--voices=<language>\n\
\t   List the available voices for the specified language.\n\
\t   If <language> is omitted, then list all voices.\n\
--load     Load voice from a file in current directory by name.\n\
-h, --help Show this help.\n";
