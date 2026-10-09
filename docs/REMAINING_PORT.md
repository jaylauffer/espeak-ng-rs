# Remaining port: the definitive list

This is everything in eSpeak NG that is still C (or C++, Java, JavaScript)
in the Rust-core build, as of 2026-10-09 (`dev`). It also says where loadngo's
proactor applies. It is derived, not recalled. `tools/c_inventory.py`
preprocesses every source file exactly as the build compiles it, so code
behind `#ifndef USE_RUST_CORE` drops out. It then lists every function that
survives:

    cmake -S . -B build-rust -DUSE_RUST_CORE=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    python3 tools/c_inventory.py build-rust --functions

Kinds of function:

- **C**: C logic with no call into Rust; this has to be ported.
- **mixed**: C logic around Rust calls (more than 15 lines); the C part has to
  be ported.
- **bridge**: a short C wrapper that forwards to Rust; it goes away when its
  callers are Rust.

The current appendix was measured on macOS: static Rust core, proactor and
Rust audio on, Klatt and speechPlayer on; async, MBROLA, libsonic and
pcaudio off. Optional async and MBROLA source counts below retain the earlier
Linux baseline unless explicitly marked as replaced. Test-only oracle copies of
retained C (`reference_*`, built into test targets) are left out: they go
when the C they check goes.

Line counts depend on platform, configuration and preprocessor formatting.
Differences from the earlier Linux baseline alone do not measure port progress;
the native replacements and their parity evidence do.

**Totals:** 9,555 lines of C and mixed logic in the current library/CLI
configuration. A separate current async/MBROLA-on macOS inventory
(`tools/c_inventory.py build-rust-async --functions`) counts 10,080 C/mixed
lines; it includes optional engine code and cannot be substituted for the
sync configuration above. `espeak_command.c` now contains 11 forwarding
bridges and no C/mixed logic in that native build. The earlier optional async/MBROLA inventory counted 1,855
lines in those sources; its queues have since moved to Rust in proactor
builds (item 17). The earlier C++ scan counted 622 lines in speechPlayer; class method bodies
are not included by the current top-level function scanner. Platform front
ends also remain. The two largest groups are the text front end
(items 1 to 7, about 4,600 lines) and the data compilers (item 19, about
3,300 lines with the spectrum reader).

## A. Text front end

1. **`translate.c`** (1,369 lines; 26 C functions). Clause translation
   `TranslateClauseWithTerminator` (639), `TranslateWord2` (276) and
   `TranslateWordWithBounds`, plus character substitution and replacement
   (`SubstituteChar`, `TranslateChar`, `FindReplacementChars`,
   `UpperCaseInWord`). Also embedded commands in text (`EmbeddedCommand`,
   `Word_EmbeddedCmd`), the language-switch translators
   (`SetAlternateTranslator`, `SwitchLanguage`, `FreeAlternateTranslators`)
   and ideograph segmentation (`ShouldSplitIdeographs`, `SegmentReplacement`).
   The rest is small: `CombineFlag`, `InitText`, `CalcWordLength`,
   `CountSyllables`, `SetPlist2`, `lookupwchar`, `strchr_w`. No proactor role:
   this is CPU only.
2. **`translateword.c`** (878; 9 C). `TranslateWord3` (466), `TranslateLetter`
   (209), `LookupEmojiBaseSequence`, `Unpronouncable`/`Unpronouncable2`,
   `CheckDottedAbbrev`, `addPluralSuffixes`, `SpeakIndividualLetters`,
   `NonAsciiNumber`. CPU only.
3. **`numbers.c`** (1,103; all 15 functions). Number translation
   (`TranslateNumber_1` 365, `LookupNum2` 191, `LookupNum3` 176,
   `LookupThousands`, `M_Variant`, `CheckThousandsGroup`, `hu_number_e`),
   Roman numerals, ordinals, letter lookup (`LookupLetter`, `LookupLetter2`,
   `LookupAccentedLetter`, `IsSuperscript`) and `SetSpellingStress`. CPU only.
4. **`dictionary.c`** (792; 13 C, 7 mixed). The rule engine's C driver
   `TranslateRules` (197, mixed) and dictionary lookup `LookupDictList` (105,
   mixed) with `Lookup`/`LookupFlags`. Also phoneme string encoding and
   writing (`EncodePhonemes`, `WritePhMnemonic`,
   `WritePhMnemonicWithStress`, `GetTranslatedPhonemeString`),
   `RemoveEnding` (mixed), the environments the Rust matcher calls back into
   (`espeak_rs_match_rule`, `espeak_rs_lookup_dict`, `RustLetterConfig`,
   `RustPrefixFlags`, `RustMatchTrace`), and the dictionary owner glue
   `InitDictionary`/`LoadDictionary` (mixed). Dictionary bytes are already
   read through the proactor.
5. **`readclause.c`** (139; 15 C plus bridges). The clause reader's host
   callbacks (`ClauseSnapshot`, `ClausePublish`, `ClauseClassify`,
   `ClauseEffect`, `ClauseSource*`, `ClauseReplace`, `ClausePhonemeAlpha`),
   `ReadClause` and `AddNameData` bridges, `CharacterQuery`, `SetVoiceStack`,
   `InitText2` and `PunctuationName`. The reader itself is Rust.
6. **`ssml.c`** (58; 8 C, 1 mixed). The SSML engine's host side:
   `ProcessSsmlTag` (mixed), voice selection (`SsmlSelectVoice`,
   `SsmlResolveVoiceName`), `SsmlUpdateRate`, character classes and the URI
   callback setter. The URI callback (for `<audio>`) is a candidate for
   proactor I/O once it is Rust.
7. **Phoneme list, lengths and intonation glue.** `phonemelist.c` (75):
   `MakePhonemeList` adapter, `ListSelect`, `ListInvalidInstruction`.
   `setlengths.c` (143): `SetParameter` (49), the `CalcLengths` adapter,
   `DoEmbedded2`, `LengthEmbedded`, `LengthToneEnvelope`, `SetLengthMods`.
   `intonation.c` (42): the `CalcPitches` adapter. CPU only.

## B. Synthesis back end

8. **`synthesize.c`** (296; 6 C, 5 mixed). `SpeakNextClause` (37), the
   `Generate` adapter and its effect dispatcher `GenerateEffect` (88), the
   command writers' host `CommandEffect` (80; spectrum lookup, smoothing,
   frames, speed changes, sound icons) with `CommandSettings`,
   `FormantTransitionWithCapacity`, `GenerateEnvelope`, `SynthesizeInit`,
   `WordToString` and `espeak_SetPhonemeCallback`. Spectrum smoothing and
   lookup run in Rust but over C-owned arguments.
9. **`synthdata.c`** (193; 10 C, 5 mixed). The phoneme program and spectrum
   adapters (`InterpretPhonemeWithLength`, `InterpretPhoneme2WithData`,
   `InterpretPhoneme2`, `LookupSpect`, `RustSpectrumTransition`,
   `RustPhonemeStorage`), table selection (`SelectPhonemeTable`,
   `SelectPhonemeTableName`), `TonePhoneme`, and the phoneme data owner glue
   (`LoadPhData`, `ReadPhFile`, `FreePhData`, mixed). The data is already
   read through the proactor.
10. **Klatt and speechPlayer.** Klatt's cascade/parallel synthesizer is now
    native Rust (`rust/klatt.rs`), with all five glottal sources, filters,
    frame interpolation, pitch/noise history, mixing, echo and fades.
    `klatt.c` keeps only owner callbacks and short Rust bridges, plus the
    speechPlayer delegation. The retained C is excluded from Rust-core builds
    and extracted solely for differential testing. Still to port: `sPlayer.c`
    (100 lines of adapter logic) and `src/speechPlayer` (622 lines of C++).
    These are CPU work within the proactor-driven synthesis step.
11. **`wavegen.c` remainder** (53; 4 C, 1 mixed). The generator's host
    dispatch `WavegenEffect` (Klatt, MBROLA, markers, hooks, sonic),
    `WavegenFill` (the sonic speed-up), `Write4Bytes`,
    `espeak_ng_SetOutputHooks`, and the C-owned sample rate and output hooks.
12. **MBROLA** (463 C/mixed lines measured in the macOS Rust-core,
    proactor/audio/async/MBROLA-on, speechPlayer-off configuration using
    `tools/c_inventory.py build-rust-async --functions`). `synth_mbrola.c`
    contributes 152, `mbrowrap.c` 311. `rust/mbrola_generate.rs` now owns
    command-generation decisions, embedded/word cursors and bounded pending
    text, retaining admitted byte offsets across partial writes.
    `rust/mbrola_fill.rs` owns output sample accounting and distinguishes
    pending I/O from end, with a bounded caller PCM destination. Its C adapter
    retains the blocking `read_MBR` and idle interpretation. The former short
    C cursor bridge was excluded from the logic total; the new four-line
    reader adapter explains the increase from 459 to 463. The C
    `MbrGenerateEffect` dispatch, one-clause snapshot and startup/output shell
    remain. Five short transport helpers classified
    as bridges by the scanner also retain C lifecycle/syscall/logging work;
    they still need integration. The earlier Linux baseline counted 908
    C/mixed lines; configuration changes alone are not a port metric.
    Rust-core Unix builds now
    use `rust/mbrola_transport.rs` for bounded FIFO command buffering,
    persistent stderr framing and WAV sample-rate parsing. The C shell still
    spawns the `mbrola` process and talks
    to it over pipes (`start_mbrola`, `send_to_mbrola`,
    `receive_from_mbrola`, `mbrola_has_errors`, `mbrola_died`), or loads the
    MBROLA DLL on Windows. **Proactor: the pipe reads and writes and the
    child's error stream belong on the proactor** (`IoPort` read/write and
    readiness on Unix), replacing blocking pipe I/O and `poll` waits. The
    transport checkpoint has not replaced these waits or the Windows DLL
    loader. `rust/mbrola_process.rs` now supplies a tested safe Unix driver
    using socketpair stdio and the caller's loadngo `send`/`recv` completions,
    with reusable command/audio/error loans and explicit whole-input EOF.
    This API is not yet connected to the C generation/output loop. Spawn and
    final kill/reap still run on an initialization/shutdown owner, outside
    callbacks; integrating host worker/process completion remains required.
    Ordinary flushes have no explicit completion acknowledgement in
    upstream's protocol; its stderr flush diagnostic is a reset-signal path.
    See the evidence and next-step constraints in `RUST_PORT.md`.

## C. Engine, API and I/O

13. **`speech.c`** (244; 23 C, 4 mixed). Native `rust/engine_lifecycle.rs`
    owns initialization/output setup, cancel/synchronize/terminate sequencing,
    atomic mode/rate/error state and admitted audio-handle cleanup. Checked
    output sizing preserves defined C rounding and rejects overflow. Teardown
    destroys audio retained across a switch to retrieval mode, and relinquishes
    audio/translator/decoder slots before destruction callbacks. Initialization
    primitives, parameter arrays and most engine resources still have C owners;
    lifecycle admission is serialized. The optional pthread-only backend keeps
    its existing C synchronization loop; no Rust polling fallback is added.
    Remaining control includes path lookup
    (`espeak_ng_InitializePath`, `check_data_path`), the synthesis step/loop
    `SynthesizeStep` and `Synthesize` (mixed), the `sync_espeak_*` and `espeak_ng_*` entry points,
    audio dispatch (`dispatch_audio`, `create_events`), parameter
    get/set, `espeak_TextToPhonemes*`, `espeak_SetPhonemeTrace`,
    `espeak_Info`. Native synthesis runs on the proactor and audio/event waits
    use completions. C steps still report locally-ready/done, with pending
    process I/O, owned engine instances and full resource integration remaining.
    Short C routines containing native state getters may be classified as
    bridges by the scanner even though their control still needs migration.
14. **`espeak_api.c`** (14 bridges) and **`error.c`** (8; 4 CRT callbacks,
    5 bridges). Native `rust/legacy_api.rs` and `rust/status.rs` now own legacy
    initialization/compiler control, status conversion, error contexts and
    byte-preserving diagnostics. Retained C checks control order and diagnostic
    output. C retains errno messages and stream locking/writes; underlying
    engine callbacks and lifecycle remain in item 13. No scheduler is added.
15. **`voices.c` remainder** (171; 11 C, 1 mixed). `espeak_ng_SetVoiceByName`
    (43), `ByFile`, `ByProperties`, `LoadVoiceVariant`, `LoadVoice` (mixed) and
    the `SelectVoice` bridge, the callbacks Rust calls (`RustActiveVoiceHost`
    45, `RustVoiceLength`, `RustVoiceDirectory`, `RustVoiceWorkspace`,
    `RustCatalogDiagnostic`, `RustOrdinalEnvironment`) and
    `espeak_GetCurrentVoice`. Voice files are already read through the
    proactor. Directory listing for the voice catalogue is synchronous: the
    proactor has no directory operation.
16. **Small helpers.** `common.c` (`GetFileLength`, `utf8_in`, `CommonAlpha`,
    `CommonDigit`, `CommonSpace`, `espeak_ng_SetRandSeed`), `langopts.c`
    (`RustLanguageEnvironment`, `LoadConfig`, `CheckTranslator`). The bridges
    in `common.c`, `soundicon.c`, `tr_languages.c` and elsewhere go with
    their callers.
17. **Asynchronous API** (earlier optional inventory: 947 lines). In
    proactor builds `fifo.c` forwards to `rust/async_queue.rs` and `event.c`
    forwards to `rust/event_delivery.rs`; their legacy C stays as the oracle.
    Native event admission and clear now wait on bounded caller-port
    completions, removing the C-engine 10 ms full-queue retries. Admission
    has 16 slots and clear has four separate slots, so saturation cannot
    prevent cleanup. Command cancellation and clear interrupt ordinary
    admission; mandatory message completions survive both to release caller
    user data. Delivery termination and failure release either kind. The C adapter releases its global owner lock
    before waiting or calling back; callbacks can replace their handler,
    clear, refuse self-waits and stop their delivery worker. The old wall-clock
    helpers compile only with the retained pthread FIFO; engine/API lifecycle
    integration and final oracle retirement remain.
    `espeak_command.c` now forwards construction, dispatch and deletion to
    `rust/async_command.rs` and its C adapter. Rust owns all nine command
    kinds and copied payloads; the C header/union is a prefix view. Pending
    terminated-message deletion changes state before notifying the host,
    preserving caller cleanup. The retained C independently checks 54,000
    command pairs across all states and processing/discard paths. Text storage
    keeps wide alignment and initialized terminators; four reusable buffers
    retain at most 8 MiB. Commands larger than the per-buffer cache limit
    remain valid but their storage is not retained. Engine callbacks and the
    surrounding API lifecycle remain to port.
    `fifo.c` (earlier 530): the
    command queue and the synthesis thread (`say_thread`,
    `sleep_until_start_request_or_inactivity`, `fifo_*`). `event.c` (417):
    the event thread that delivers events as audio plays (`polling_thread`,
    `event_*`, timing helpers). Both are pthread mutexes, condition
    variables and timed waits. **Proactor: this is the clearest fit.**
    Commands become `enqueue_work`, event delivery at play time becomes
    `defer_until` timers, and stop/cancel becomes `stop`/cancellation. That
    replaces the pthread polling/timed-wait loops with proactor workers.
18. **Audio output and speed-up.** *Playback is done where loadngo-audio-io
    has a backend:* with `USE_RUST_AUDIO` (on by default where ALSA is
    found, and on macOS) `speech.c` keeps its pcaudio calls, and
    `rust_audio.h` maps them onto `rust/audio_out.rs`. That module converts
    the voice rate to the device rate into a bounded queue, which the
    device's real-time callback drains. Writers and drains wait on the
    sink's proactor, and a cancel releases them. Native sink writes/drains
    now bind to the async command's cancellation scope, so queue stop and
    termination release back-pressure even when the device stalls; their
    registration is fenced before the sink is reused. Synchronous callers
    without that scope still use the explicit sink canceller. pcaudio remains the
    playback path where the Rust sink is off (Windows, Android, iOS:
    loadngo-audio-io has no output there, or only through cpal, which has
    not been tried). Still to do: the Windows sink (cpal), and the
    PipeWire desktop stream as an alternative to ALSA. Speed-up uses
    libsonic (external C library), which needs a Rust port.

## D. Tools and data compilers

19. **Data compilers** (3,001 lines, plus `spect.c` 272). `compiledata.c`
    (1,804; 45 functions: phoneme and intonation compilers,
    `espeak_ng_CompilePhonemeDataPath`, `espeak_ng_CompileIntonationPath`,
    `CompilePhoneme` 342 and its tokenizer). `compiledict.c` (1,197; 17
    functions: `espeak_ng_CompileDictionary`, `compile_dictrules`,
    `compile_rule`, `copy_rule_string`, `compile_line`, `DecodeRule`).
    `compilembrola.c` (86, MBROLA builds). `spect.c` (272): the spectrum
    file reader the compiler uses. **Proactor: their file reads and writes**
    (sources, `phondata`, `phonindex`, `phontab`, `*_dict`, the spectrum
    and WAV inputs).
20. **Command-line tools.** `src/espeak-ng.c` (815: `main`, WAV writing,
    voice listing) and `src/speak-ng.c`. **Proactor: the WAV output writes.**

## E. Platform front ends

21. **Android:** `android/jni/jni/eSpeakService.c` (394, JNI) and the Java
    service, settings and tests.
22. **Windows SAPI:** `src/windows/com/ttsengine.cpp` (282).
23. **Emscripten:** `emscripten/espeakng_glue.cpp` (123) and JavaScript glue.

These call the C API; they move to a Rust API once one exists.

## F. Build and retirement

24. The C ABI shell (`speak_lib.h`/`espeak_ng.h` entry points) stays as
    the compatibility surface; behind it, CMake still builds the C
    library and links the Rust core into it.
25. Retained C oracles: every `/* End legacy ... */` block, the C-only build
    and the `reference_*` test targets. They are removed only after their
    Rust replacements have passed parity on all target platforms.
26. Remaining final integration/runtime gates: libsonic and pcaudio builds,
    real time-paced audio devices, MBROLA output, and full Android, Windows
    and Emscripten front ends. Rust C-ABI compilation has passed for Linux,
    Windows, iOS and Android; CoreAudio linking passed on macOS in the Klatt
    stage, but its audio CTest skipped because no device opened. These checks
    do not establish full platform runtime coverage or thermal safety.

## Where the proactor applies

| Area | Status |
| --- | --- |
| Engine data reads: phoneme data, dictionaries, voices, variants, sound-icon configuration and icons, MBROLA tables | **Done** (`rust/engine_io.rs`): one process-wide proactor, io_uring on Linux (epoll where io_uring is refused), chunked reads driven on the calling thread. `espeak_rs_engine_io_backend` and the `rust_engine_io` CTest check that a proactor build does not fall back to `std::fs`. |
| Asynchronous API command queue (`fifo.c`) | Native `rust/async_queue.rs`: ordered commands on one proactor worker; inactivity is a proactor timer. Synchronize uses bounded caller-port completions, without its earlier 20 ms polling; stop/termination wake active synthesis, and worker exit releases waiters. Worker callbacks cannot synchronize on themselves. Atomic admission, stop/settings ownership and weak queued jobs are tested; the C async parity hash is preserved. |
| Playback events (`event.c`, `speech.c`) | Native delivery (`rust/event_delivery.rs`, item 17) preserves ordered playback timers. Full admission waits for capacity on the caller's port instead of retrying every 10 ms; 16 admission and four separate clear reservations are bounded. Ordinary waits are cancellable; mandatory message completions survive command cancellation/clear to release caller user data. Worker exit releases both. Weak jobs and fenced registrations protect reused owners; adapter/callback locks are released around owner calls. Self-waits are refused; old clock helpers compile only with the retained FIFO. Engine lifecycle, timer cancellation across repeated clears and real-device/thermal gates remain. |
| Synthesis loop (`speech.c`) | Native `run_on` (`rust/synthesis_loop.rs`) suspends Pending passes until host completions wake them, coalesces wakes and posts cancellation; stale jobs cannot access a finished callback. Nested calls share the caller's port, and failures return without direct replay. Registered file reads and native MBROLA sessions exercise this runner. The legacy C step still reports only locally-ready/done; async stop/termination interrupt both its active runner and registered native audio waits. C pending process/file I/O and synchronous cancellation integration remain. |
| MBROLA process pipes (`mbrowrap.c`) | Native Unix completion driver, owned generator retries and output cursor tested (item 12); C-engine pending/EOF and lifecycle integration and Windows DLL port remain. |
| Audio output | Native sink on Linux (ALSA) and macOS (`rust/audio_out.rs`, item 18): a bounded queue drained by the loadngo-audio-io device callback. Writes/drains wait on its proactor; device progress or cancellation posts their completion. Async command cancellation now binds the sink's owned capability while each operation is live, including before-wait races, nesting and unwind fencing. Stalled-sink stop/termination and reuse tests pass without an audio device; real-device/platform/thermal gates remain. |
| Data compilers and CLI file I/O | To do (items 19 and 20). |
| `<audio>` URI callback, voice catalogue directory listing | The callback is the caller's; directory listing has no proactor operation. Their file contents are read through the proactor. |
| Text front end, synthesis math, Klatt | Not applicable: CPU only, no I/O. |

## Appendix: the generated table

`python3 tools/c_inventory.py build-rust --markdown`, library and CLI sources in
the measured configuration:

| File | C logic (lines) | Mixed: C around Rust calls (lines) | Bridges to Rust |
| --- | --- | --- | --- |
| `src/espeak-ng.c` | `DisplayVoices` 49, `Write4Bytes` 6, `OpenWavFile` 25, `CloseWavFile` 12, `SynthCallback` 29, `PrintVersion` 6, `main` 429 | - | 0 |
| `src/libespeak-ng/common.c` | `GetFileLength` 7, `utf8_in` 2, `CommonAlpha` 1, `CommonDigit` 1, `CommonSpace` 1, `espeak_ng_SetRandSeed` 3 | - | 19 |
| `src/libespeak-ng/compiledata.c` | `clean_context` 13, `error` 8, `error_from_status` 7, `ReadPhondataManifest` 43, `ReservePhCodes` 10, `LookupPhoneme` 33, `get_char` 6, `unget_char` 4, `CheckNextChar` 6, `NextItem` 85, `NextItemMax` 8, `NextItemBrackets` 12, `UngetItem` 3, `Range` 11, `CompileVowelTransition` 95, `LoadSpect` 142, `LoadWavefile` 66, `LoadEnvelope` 10, `Hash8` 11, `LoadEnvelope2` 46, `LoadDataFile` 71, `CompileToneSpec` 30, `CompileSound` 30, `CompileIf` 86, `FillThen` 19, `CompileElse` 19, `CompileElif` 8, `CompileEndif` 20, `CompileSwitch` 9, `FindPhonemeTable` 8, `FindPhoneme` 21, `ImportPhoneme` 24, `CallPhoneme` 26, `DecThenCount` 3, `CompilePhoneme` 317, `WritePhonemeTables` 37, `EndPhonemeTable` 14, `StartPhonemeTable` 36, `CompilePhonemeFiles` 58, `espeak_ng_CompilePhonemeData` 2, `espeak_ng_CompilePhonemeDataPath` 112, `LookupEnvelopeName` 2, `espeak_ng_CompileIntonation` 2, `espeak_ng_CompileIntonationPath` 227, `CalculateSample` 4 | - | 0 |
| `src/libespeak-ng/compiledict.c` | `clean_context` 10, `print_dictionary_flags` 23, `DecodeRule` 140, `compile_line` 246, `compile_dictlist_start` 13, `compile_dictlist_end` 13, `compile_dictlist_file` 46, `isHexDigit` 8, `copy_rule_string` 223, `compile_rule` 133, `string_sorter` 8, `rgroup_sorter` 7, `output_rule_group` 47, `compile_lettergroup` 56, `free_rules` 5, `compile_dictrules` 151, `espeak_ng_CompileDictionary` 68 | - | 0 |
| `src/libespeak-ng/dictionary.c` | `RustOffset` 2, `EncodePhonemes` 71, `PhonemeTextAlpha` 2, `WritePhMnemonic` 62, `WritePhMnemonicWithStress` 23, `GetTranslatedPhonemeString` 100, `RustLetterConfig` 3, `IsVowel` 2, `RustPrefixFlags` 4, `RustMatchTrace` 6, `utf8_nbytes` 9, `Lookup` 21, `LookupFlags` 9 | `InitDictionary` 28, `LoadDictionary` 32, `espeak_rs_match_rule` 36, `TranslateRules` 197, `espeak_rs_lookup_dict` 66, `LookupDictList` 100, `RemoveEnding` 19 | 8 |
| `src/libespeak-ng/error.c` | `StatusErrno` 2, `StatusLock` 2, `StatusUnlock` 2, `StatusWrite` 2 | - | 5 |
| `src/libespeak-ng/espeak_api.c` | - | - | 14 |
| `src/libespeak-ng/intonation.c` | - | `CalcPitches` 42 | 0 |
| `src/libespeak-ng/klatt.c` | `KlattRandom` 1, `KlattSpeechPlayerReset` 2, `KlattFini` 2 | - | 4 |
| `src/libespeak-ng/langopts.c` | `RustLanguageEnvironment` 12, `LoadConfig` 2, `CheckTranslator` 5 | - | 1 |
| `src/libespeak-ng/numbers.c` | `LookupLetter2` 14, `LookupAccentedLetter` 55, `LookupLetter` 46, `IsSuperscript` 10, `SetSpellingStress` 40, `CheckDotOrdinal` 36, `hu_number_e` 9, `TranslateRoman` 93, `M_Variant` 38, `LookupThousands` 69, `LookupNum2` 191, `LookupNum3` 156, `CheckThousandsGroup` 9, `TranslateNumber_1` 331, `TranslateNumber` 6 | - | 0 |
| `src/libespeak-ng/phonemelist.c` | `ListSelect` 6, `ListInvalidInstruction` 3 | `MakePhonemeList` 66 | 0 |
| `src/libespeak-ng/readclause.c` | `UngetC` 2, `CharacterQuery` 33, `PunctuationName` 8, `PunctuationUnreadSecond` 2, `SetVoiceStack` 18, `ClauseSnapshot` 13, `ClausePublish` 11, `ClauseSourceEof` 1, `ClauseSourceRead` 1, `ClauseSourcePeek` 1, `ClauseClassify` 8, `ClauseReplace` 2, `ClauseEffect` 17, `InitText2` 20, `ClausePhonemeAlpha` 2 | - | 13 |
| `src/libespeak-ng/sPlayer.c` | `MIN` 1, `needsMixWaveFile` 2, `mixWaveFile` 21, `fillSpeechPlayerFrame` 29, `KlattInitSP` 2, `KlattFiniSP` 4, `KlattResetSP` 3 | `Wavegen_KlattSP` 38 | 1 |
| `src/libespeak-ng/setlengths.c` | `SetParameter` 49, `DoEmbedded2` 9, `LengthEmbedded` 7, `LengthToneEnvelope` 10, `SetLengthMods` 5 | `CalcLengths` 63 | 1 |
| `src/libespeak-ng/soundicon.c` | - | - | 5 |
| `src/libespeak-ng/spect.c` | `read_double` 6, `polint` 29, `SpectFrameCreate` 29, `SpectFrameDestroy` 4, `LoadFrame` 55, `GetFrameRms` 19, `SpectSeqCreate` 16, `SpectSeqDestroy` 11, `GetFrameLength` 10, `LoadSpectSeq` 93 | - | 0 |
| `src/libespeak-ng/speech.c` | `check_data_path` 9, `espeak_ng_InitializePath` 8, `RustEngineNoop` 1, `RustEngineCurrentVoiceClear` 1, `RustEngineStackReset` 1, `RustEngineVoiceReset` 1, `RustEngineClock` 1, `espeak_ng_SetPhonemeEvents` 9, `espeak_ng_GetSampleRate` 2, `sync_espeak_Synth_Mark` 10, `sync_espeak_Key` 9, `sync_espeak_Char` 6, `sync_espeak_SetPunctuationList` 8, `espeak_SetSynthCallback` 2, `espeak_ng_SpeakCharacter` 2, `espeak_GetParameter` 4, `espeak_ng_SetParameter` 2, `espeak_ng_SetPunctuationList` 3, `espeak_SetPhonemeTrace` 5, `espeak_TextToPhonemesWithTerminator` 8, `espeak_TextToPhonemes` 2, `espeak_IsPlaying` 2, `espeak_Info` 4 | `dispatch_audio` 48, `SynthesizeStep` 39, `Synthesize` 25, `sync_espeak_Synth` 32 | 15 |
| `src/libespeak-ng/ssml.c` | `SsmlWideSpace` 1, `SsmlByteSpace` 1, `SsmlByteLower` 1, `SsmlDecimalPoint` 6, `SsmlResolveVoiceName` 8, `SsmlSelectVoice` 15, `SsmlUpdateRate` 4, `espeak_SetUriCallback` 2 | `ProcessSsmlTag` 20 | 1 |
| `src/libespeak-ng/synthdata.c` | `RustSpectrumTransition` 6, `SelectPhonemeTableName` 6, `InvalidInstn` 3, `RustPhonemeStorage` 34, `RustPhonemeDataLength` 2, `RustPhonemePrograms` 3, `RustInvalidInstruction` 2, `InterpretPhoneme2WithData` 16, `InterpretPhoneme2` 2, `TonePhoneme` 4 | `ReadPhFile` 19, `LoadPhData` 44, `FreePhData` 17, `SelectPhonemeTable` 16, `InterpretPhonemeWithLength` 19 | 5 |
| `src/libespeak-ng/synthesize.c` | `WordToString` 6, `SynthesizeInit` 5, `FormantTransition2` 2, `GenerateEnvelope` 6, `SpeakNextClause` 37, `espeak_SetPhonemeCallback` 2 | `CommandSettings` 21, `CommandEffect` 66, `FormantTransitionWithCapacity` 17, `GenerateEffect` 88, `Generate` 46 | 15 |
| `src/libespeak-ng/tr_languages.c` | - | - | 3 |
| `src/libespeak-ng/translate.c` | `FreeAlternateTranslators` 7, `lookupwchar` 7, `strchr_w` 4, `ShouldSplitIdeographs` 13, `SegmentReplacement` 35, `TranslateWordWithBounds` 52, `TranslateWord` 2, `SetPlist2` 7, `CountSyllables` 8, `Word_EmbeddedCmd` 21, `SetAlternateTranslator` 21, `SetTranslator2` 2, `SetTranslator3` 2, `TranslateWord2` 276, `TranslateWord2WithContext` 9, `EmbeddedCommand` 44, `FindReplacementChars` 32, `SubstituteChar` 32, `TranslateChar` 47, `UpperCaseInWord` 16, `TranslateClauseWithTerminator` 639, `TranslateClause` 2, `CalcWordLength` 11, `CombineFlag` 44, `SwitchLanguage` 18, `InitText` 18 | - | 1 |
| `src/libespeak-ng/translateword.c` | `LookupEmojiBaseSequence` 83, `TranslateWord3` 466, `SpeakIndividualLetters` 19, `TranslateLetter` 183, `addPluralSuffixes` 15, `CheckDottedAbbrev` 44, `NonAsciiNumber` 10, `Unpronouncable` 46, `Unpronouncable2` 12 | - | 2 |
| `src/libespeak-ng/voices.c` | `RustVoiceLength` 2, `RustCatalogDiagnostic` 5, `RustVoiceWorkspace` 3, `RustVoiceDirectory` 5, `RustOrdinalEnvironment` 4, `RustActiveVoiceHost` 33, `LoadVoiceVariant` 11, `espeak_ng_SetVoiceByFile` 22, `espeak_ng_SetVoiceByName` 35, `espeak_ng_SetVoiceByProperties` 10, `espeak_GetCurrentVoice` 2 | `LoadVoice` 39 | 10 |
| `src/libespeak-ng/wavegen.c` | `WavegenFini` 2, `Write4Bytes` 6, `WavegenFill` 4, `espeak_ng_SetOutputHooks` 3 | `WavegenEffect` 38 | 14 |
| `src/speechPlayer/src/frame.cpp` | `create` 1 | - | 0 |
| `src/speechPlayer/src/speechPlayer.cpp` | `speechPlayer_initialize` 7, `speechPlayer_queueFrame` 4, `speechPlayer_synthesize` 2, `speechPlayer_getLastIndex` 3, `speechPlayer_terminate` 5 | - | 0 |
| `src/speechPlayer/src/speechWaveGenerator.cpp` | `create` 1 | - | 0 |
