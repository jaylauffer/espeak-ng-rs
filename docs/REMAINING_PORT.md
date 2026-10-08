# Remaining port: the definitive list

This is everything in eSpeak NG that is still C (or C++, Java, JavaScript)
in the Rust-core build, as of 2026-10-08 (`dev`). It also says where loadngo's
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

The configuration measured was static Rust core, Klatt and speechPlayer on;
async, MBROLA, libsonic and pcaudio off. Async and MBROLA sources were
measured in a second configuration with both on. Test-only oracle copies of
retained C (`reference_*`, built into test targets) are left out: they go
when the C they check goes.

**Totals:** about 12,400 lines of C logic in the library and CLI, plus 1,855
in the async and MBROLA sources, 622 lines of C++ (speechPlayer) and the
platform front ends. The two largest groups are the text front end
(items 1 to 7, about 5,400 lines) and the data compilers (item 19, about
3,900 lines with the spectrum reader).

## A. Text front end

1. **`translate.c`** (1,648 lines; 26 C functions). Clause translation
   `TranslateClauseWithTerminator` (807), `TranslateWord2` (287) and
   `TranslateWordWithBounds`, plus character substitution and replacement
   (`SubstituteChar`, `TranslateChar`, `FindReplacementChars`,
   `UpperCaseInWord`). Also embedded commands in text (`EmbeddedCommand`,
   `Word_EmbeddedCmd`), the language-switch translators
   (`SetAlternateTranslator`, `SwitchLanguage`, `FreeAlternateTranslators`)
   and ideograph segmentation (`ShouldSplitIdeographs`, `SegmentReplacement`).
   The rest is small: `CombineFlag`, `InitText`, `CalcWordLength`,
   `CountSyllables`, `SetPlist2`, `lookupwchar`, `strchr_w`. No proactor role:
   this is CPU only.
2. **`translateword.c`** (996; 9 C). `TranslateWord3` (522), `TranslateLetter`
   (209), `LookupEmojiBaseSequence`, `Unpronouncable`/`Unpronouncable2`,
   `CheckDottedAbbrev`, `addPluralSuffixes`, `SpeakIndividualLetters`,
   `NonAsciiNumber`. CPU only.
3. **`numbers.c`** (1,194; all 15 functions). Number translation
   (`TranslateNumber_1` 365, `LookupNum2` 191, `LookupNum3` 176,
   `LookupThousands`, `M_Variant`, `CheckThousandsGroup`, `hu_number_e`),
   Roman numerals, ordinals, letter lookup (`LookupLetter`, `LookupLetter2`,
   `LookupAccentedLetter`, `IsSuperscript`) and `SetSpellingStress`. CPU only.
4. **`dictionary.c`** (949; 13 C, 7 mixed). The rule engine's C driver
   `TranslateRules` (219, mixed) and dictionary lookup `LookupDictList` (105,
   mixed) with `Lookup`/`LookupFlags`. Also phoneme string encoding and
   writing (`EncodePhonemes`, `WritePhMnemonic`,
   `WritePhMnemonicWithStress`, `GetTranslatedPhonemeString`),
   `RemoveEnding` (mixed), the environments the Rust matcher calls back into
   (`espeak_rs_match_rule`, `espeak_rs_lookup_dict`, `RustLetterConfig`,
   `RustPrefixFlags`, `RustMatchTrace`), and the dictionary owner glue
   `InitDictionary`/`LoadDictionary` (mixed). Dictionary bytes are already
   read through the proactor.
5. **`readclause.c`** (256; 15 C, 2 mixed). The clause reader's host
   callbacks (`ClauseSnapshot`, `ClausePublish`, `ClauseClassify`,
   `ClauseEffect`, `ClauseSource*`, `ClauseReplace`, `ClausePhonemeAlpha`),
   `ReadClause` (mixed), `CharacterQuery`, `SetVoiceStack`, `InitText2`,
   `PunctuationName`, `AddNameData` (mixed). The reader itself is Rust.
6. **`ssml.c`** (76; 8 C, 1 mixed). The SSML engine's host side:
   `ProcessSsmlTag` (mixed), voice selection (`SsmlSelectVoice`,
   `SsmlResolveVoiceName`), `SsmlUpdateRate`, character classes and the URI
   callback setter. The URI callback (for `<audio>`) is a candidate for
   proactor I/O once it is Rust.
7. **Phoneme list, lengths and intonation glue.** `phonemelist.c` (86):
   `MakePhonemeList` adapter, `ListSelect`, `ListInvalidInstruction`.
   `setlengths.c` (155): `SetParameter` (51), the `CalcLengths` adapter,
   `DoEmbedded2`, `LengthEmbedded`, `LengthToneEnvelope`, `SetLengthMods`.
   `intonation.c` (44): the `CalcPitches` adapter. CPU only.

## B. Synthesis back end

8. **`synthesize.c`** (352; 6 C, 5 mixed). `SpeakNextClause` (47), the
   `Generate` adapter and its effect dispatcher `GenerateEffect` (100), the
   command writers' host `CommandEffect` (80; spectrum lookup, smoothing,
   frames, speed changes, sound icons) with `CommandSettings`,
   `FormantTransitionWithCapacity`, `GenerateEnvelope`, `SynthesizeInit`,
   `WordToString` and `espeak_SetPhonemeCallback`. Spectrum smoothing and
   lookup run in Rust but over C-owned arguments.
9. **`synthdata.c`** (298; 10 C, 6 mixed). The phoneme program and spectrum
   adapters (`InterpretPhonemeWithLength`, `InterpretPhoneme2WithData`,
   `InterpretPhoneme2`, `LookupSpect`, `RustSpectrumTransition`,
   `RustPhonemeStorage`), table selection (`SelectPhonemeTable`,
   `SelectPhonemeTableName`), `TonePhoneme`, and the phoneme data owner glue
   (`LoadPhData`, `ReadPhFile`, `FreePhData`, mixed). The data is already
   read through the proactor.
10. **Klatt and speechPlayer.** `klatt.c` (579; 16 C, 2 mixed): the Klatt
    cascade/parallel synthesizer (`parwave`, `SetSynth_Klatt`,
    `Wavegen_Klatt`, the sources, resonators, `pitch_synch_par_reset`,
    `frame_init`, `KlattInit`/`Reset`/`Fini`). `sPlayer.c` (114): the
    speechPlayer adapter. `src/speechPlayer` (622 lines of C++): the
    speechPlayer synthesizer. CPU only. Klatt's and speechPlayer's queue
    look-ahead still read the Rust queue in place.
11. **`wavegen.c` remainder** (53; 4 C, 1 mixed). The generator's host
    dispatch `WavegenEffect` (Klatt, MBROLA, markers, hooks, sonic),
    `WavegenFill` (the sonic speed-up), `Write4Bytes`,
    `espeak_ng_SetOutputHooks`, and the C-owned sample rate and output hooks.
12. **MBROLA** (MBROLA builds; 908 lines). `synth_mbrola.c` (348; mostly
    bridges, plus `MbrolaGenerate`, `MbrolaReset` and voice probing) and
    `mbrowrap.c` (560). `mbrowrap.c` spawns the `mbrola` process and talks
    to it over pipes (`start_mbrola`, `send_to_mbrola`,
    `receive_from_mbrola`, `mbrola_has_errors`, `mbrola_died`), or loads the
    MBROLA DLL on Windows. **Proactor: the pipe reads and writes and the
    child's error stream belong on the proactor** (`IoPort` read/write and
    readiness on Unix), replacing blocking pipe I/O and `poll` waits.

## C. Engine, API and I/O

13. **`speech.c`** (391; 27 C, 4 mixed). Initialization
    (`espeak_ng_Initialize`, `espeak_ng_InitializeOutput`,
    `espeak_ng_InitializePath`, `check_data_path`), the synthesis loop
    `Synthesize` (mixed), the `sync_espeak_*` and `espeak_ng_*` entry points,
    audio dispatch (`dispatch_audio`, `create_events`), parameter
    get/set, `espeak_TextToPhonemes*`, `espeak_SetPhonemeTrace`,
    `espeak_ng_Cancel`/`Synchronize`/`Terminate`, `espeak_Info`. **Proactor:
    the synthesis loop should run as proactor work** (buffer fills as work
    items, completions delivering audio and events), so that cancellation
    and audio back-pressure are completions rather than blocking calls.
14. **`espeak_api.c`** (85) and **`error.c`** (113). The legacy API wrappers
    and status/error messages. Kept as the C ABI; their bodies become Rust.
15. **`voices.c` remainder** (267; 11 C, 2 mixed). `espeak_ng_SetVoiceByName`
    (43), `ByFile`, `ByProperties`, `LoadVoiceVariant`, `LoadVoice` and
    `SelectVoice` (mixed), the callbacks Rust calls (`RustActiveVoiceHost`
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
17. **Asynchronous API** (async builds; 947 lines). *The command queue is
    done: in proactor builds `fifo.c` forwards to `rust/async_queue.rs`;
    its legacy C stays as the oracle. `event.c` remains.* `fifo.c` (530): the
    command queue and the synthesis thread (`say_thread`,
    `sleep_until_start_request_or_inactivity`, `fifo_*`). `event.c` (417):
    the event thread that delivers events as audio plays (`polling_thread`,
    `event_*`, timing helpers). Both are pthread mutexes, condition
    variables and timed waits. **Proactor: this is the clearest fit.**
    Commands become `enqueue_work`, event delivery at play time becomes
    `defer_until` timers, and stop/cancel becomes `stop`/cancellation. That
    replaces both threads and their sleeps.
18. **Audio output and speed-up.** Playback uses pcaudio (external C
    library: `audio_object_open`/`write`/`drain`/`flush` in `speech.c`).
    Speed-up uses libsonic (external C library). **Proactor and audio:**
    loadngo-audio-io provides output streams (ALSA, PipeWire, CoreAudio,
    cpal; pull model, f32), but not on Android or iOS, and only its Windows
    backend runs on the proactor. Replacing pcaudio means feeding its
    pull callback from the synthesis work above through a bounded buffer,
    with resampling from the voice rate. libsonic needs a Rust port.

## D. Tools and data compilers

19. **Data compilers** (3,542 lines, plus `spect.c` 404). `compiledata.c`
    (2,113; 45 functions: phoneme and intonation compilers,
    `espeak_ng_CompilePhonemeDataPath`, `espeak_ng_CompileIntonationPath`,
    `CompilePhoneme` 342 and its tokenizer). `compiledict.c` (1,343; 17
    functions: `espeak_ng_CompileDictionary`, `compile_dictrules`,
    `compile_rule`, `copy_rule_string`, `compile_line`, `DecodeRule`).
    `compilembrola.c` (86, MBROLA builds). `spect.c` (404): the spectrum
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
26. Gates not yet run for any stage: cross-target builds (including aarch64
    and Windows), libsonic and pcaudio builds, real audio devices, MBROLA
    output, and Android, Windows and Emscripten builds.

## Where the proactor applies

| Area | Status |
| --- | --- |
| Engine data reads: phoneme data, dictionaries, voices, variants, sound-icon configuration and icons, MBROLA tables | **Done** (`rust/engine_io.rs`): one process-wide proactor, io_uring on Linux (epoll where io_uring is refused), chunked reads driven on the calling thread. `espeak_rs_engine_io_backend` and the `rust_engine_io` CTest check that a proactor build does not fall back to `std::fs`. |
| Asynchronous API command queue (`fifo.c`) | **Done** (`rust/async_queue.rs`): commands are proactor work on one worker, the inactivity wait is a proactor timer; no pthread mutexes, conditions or timed waits. Output matches the legacy queue exactly (`rust_async` CTest). |
| Playback event thread (`event.c`) | To do (item 17): delivering events as audio plays should become proactor timers. It runs only with audio output (pcaudio), which is not installed here, so it cannot be tested here yet. |
| Synthesis loop (`speech.c`) | **Done** (`rust/synthesis_loop.rs`): each pass (fill a buffer, deliver it with events, generate) is a work item on the calling thread's proactor. Still to do: cancellation as a posted completion, and audio back-pressure once output is proactor-driven (item 18). |
| MBROLA process pipes (`mbrowrap.c`) | To do (item 12): pipe I/O and readiness. |
| Audio output (pcaudio) | To do (item 18): loadngo-audio-io streams fed from proactor work. |
| Data compilers and CLI file I/O | To do (items 19 and 20). |
| `<audio>` URI callback, voice catalogue directory listing | The callback is the caller's; directory listing has no proactor operation. Their file contents are read through the proactor. |
| Text front end, synthesis math, Klatt | Not applicable: CPU only, no I/O. |

## Appendix: the generated table

`python3 tools/c_inventory.py build-rust --markdown`, library and CLI sources in
the measured configuration:

| File | C logic (lines) | Mixed: C around Rust calls (lines) | Bridges to Rust |
| --- | --- | --- | --- |
| `src/espeak-ng.c` | `DisplayVoices` 57, `Write4Bytes` 6, `OpenWavFile` 39, `CloseWavFile` 22, `SynthCallback` 33, `PrintVersion` 8, `main` 650 | - | 0 |
| `src/libespeak-ng/common.c` | `GetFileLength` 15, `utf8_in` 2, `CommonAlpha` 3, `CommonDigit` 3, `CommonSpace` 3, `espeak_ng_SetRandSeed` 3 | - | 19 |
| `src/libespeak-ng/compiledata.c` | `clean_context` 13, `error` 16, `error_from_status` 7, `ReadPhondataManifest` 65, `ReservePhCodes` 12, `LookupPhoneme` 33, `get_char` 6, `unget_char` 4, `CheckNextChar` 6, `NextItem` 107, `NextItemMax` 8, `NextItemBrackets` 12, `UngetItem` 5, `Range` 11, `CompileVowelTransition` 95, `LoadSpect` 148, `LoadWavefile` 78, `LoadEnvelope` 16, `Hash8` 11, `LoadEnvelope2` 50, `LoadDataFile` 85, `CompileToneSpec` 30, `CompileSound` 30, `CompileIf` 106, `FillThen` 23, `CompileElse` 25, `CompileElif` 8, `CompileEndif` 22, `CompileSwitch` 9, `FindPhonemeTable` 10, `FindPhoneme` 29, `ImportPhoneme` 26, `CallPhoneme` 28, `DecThenCount` 3, `CompilePhoneme` 342, `WritePhonemeTables` 37, `EndPhonemeTable` 14, `StartPhonemeTable` 38, `CompilePhonemeFiles` 62, `espeak_ng_CompilePhonemeData` 6, `espeak_ng_CompilePhonemeDataPath` 171, `LookupEnvelopeName` 2, `espeak_ng_CompileIntonation` 6, `espeak_ng_CompileIntonationPath` 294, `CalculateSample` 4 | - | 0 |
| `src/libespeak-ng/compiledict.c` | `clean_context` 10, `print_dictionary_flags` 23, `DecodeRule` 150, `compile_line` 285, `compile_dictlist_start` 17, `compile_dictlist_end` 15, `compile_dictlist_file` 66, `isHexDigit` 8, `copy_rule_string` 241, `compile_rule` 144, `string_sorter` 8, `rgroup_sorter` 7, `output_rule_group` 49, `compile_lettergroup` 56, `free_rules` 7, `compile_dictrules` 163, `espeak_ng_CompileDictionary` 94 | - | 0 |
| `src/libespeak-ng/dictionary.c` | `RustOffset` 6, `EncodePhonemes` 89, `PhonemeTextAlpha` 6, `WritePhMnemonic` 75, `WritePhMnemonicWithStress` 23, `GetTranslatedPhonemeString` 116, `RustLetterConfig` 3, `IsVowel` 2, `RustPrefixFlags` 4, `RustMatchTrace` 8, `utf8_nbytes` 9, `Lookup` 27, `LookupFlags` 11 | `InitDictionary` 28, `LoadDictionary` 48, `espeak_rs_match_rule` 50, `TranslateRules` 219, `espeak_rs_lookup_dict` 93, `LookupDictList` 105, `RemoveEnding` 27 | 8 |
| `src/libespeak-ng/error.c` | `create_file_error_context` 17, `create_version_mismatch_error_context` 17, `espeak_ng_ClearErrorContext` 8, `espeak_ng_GetStatusCodeMessage` 55, `espeak_ng_PrintStatusCodeMessage` 16 | - | 0 |
| `src/libespeak-ng/espeak_api.c` | `status_to_espeak_error` 11, `espeak_Initialize` 39, `espeak_Synth` 2, `espeak_Synth_Mark` 2, `espeak_Key` 2, `espeak_Char` 2, `espeak_SetParameter` 2, `espeak_SetPunctuationList` 2, `espeak_SetVoiceByName` 2, `espeak_SetVoiceByFile` 2, `espeak_SetVoiceByProperties` 2, `espeak_Cancel` 2, `espeak_Synchronize` 2, `espeak_Terminate` 2, `espeak_CompileDictionary` 11 | - | 0 |
| `src/libespeak-ng/intonation.c` | - | `CalcPitches` 44 | 0 |
| `src/libespeak-ng/klatt.c` | `resonator` 6, `antiresonator` 5, `flutter` 18, `sampled_source` 31, `KlattReset` 26, `KlattFini` 2, `frame_init` 39, `impulsive_source` 8, `natural_source` 11, `pitch_synch_par_reset` 65, `setabc` 9, `setzeroabc` 15, `gen_noise` 8, `DBtoLIN` 15, `Wavegen_Klatt` 69, `KlattInit` 37 | `parwave` 123, `SetSynth_Klatt` 92 | 0 |
| `src/libespeak-ng/langopts.c` | `RustLanguageEnvironment` 16, `LoadConfig` 2, `CheckTranslator` 7 | - | 1 |
| `src/libespeak-ng/numbers.c` | `LookupLetter2` 18, `LookupAccentedLetter` 55, `LookupLetter` 52, `IsSuperscript` 10, `SetSpellingStress` 40, `CheckDotOrdinal` 42, `hu_number_e` 9, `TranslateRoman` 95, `M_Variant` 51, `LookupThousands` 69, `LookupNum2` 191, `LookupNum3` 176, `CheckThousandsGroup` 15, `TranslateNumber_1` 365, `TranslateNumber` 6 | - | 0 |
| `src/libespeak-ng/phonemelist.c` | `ListSelect` 6, `ListInvalidInstruction` 3 | `MakePhonemeList` 77 | 0 |
| `src/libespeak-ng/readclause.c` | `UngetC` 2, `CharacterQuery` 49, `PunctuationName` 12, `PunctuationUnreadSecond` 2, `SetVoiceStack` 24, `ClauseSnapshot` 15, `ClausePublish` 11, `ClauseSourceEof` 1, `ClauseSourceRead` 1, `ClauseSourcePeek` 1, `ClauseClassify` 26, `ClauseReplace` 2, `ClauseEffect` 21, `InitText2` 38, `ClausePhonemeAlpha` 6 | `AddNameData` 21, `ReadClause` 24 | 11 |
| `src/libespeak-ng/sPlayer.c` | `MIN` 1, `needsMixWaveFile` 4, `mixWaveFile` 21, `fillSpeechPlayerFrame` 29, `KlattInitSP` 2, `KlattFiniSP` 6, `KlattResetSP` 3 | `Wavegen_KlattSP` 48 | 1 |
| `src/libespeak-ng/setlengths.c` | `SetParameter` 51, `DoEmbedded2` 9, `LengthEmbedded` 7, `LengthToneEnvelope` 12, `SetLengthMods` 5 | `CalcLengths` 71 | 1 |
| `src/libespeak-ng/soundicon.c` | - | - | 5 |
| `src/libespeak-ng/spect.c` | `read_double` 6, `polint` 29, `SpectFrameCreate` 33, `SpectFrameDestroy` 6, `LoadFrame` 123, `GetFrameRms` 19, `SpectSeqCreate` 22, `SpectSeqDestroy` 15, `GetFrameLength` 10, `LoadSpectSeq` 141 | - | 0 |
| `src/libespeak-ng/speech.c` | `cancel_audio` 1, `dispatch_audio` 24, `check_data_path` 13, `espeak_ng_InitializePath` 8, `espeak_ng_Initialize` 49, `espeak_ng_SetPhonemeEvents` 9, `espeak_ng_GetSampleRate` 2, `sync_espeak_Synth` 25, `sync_espeak_Synth_Mark` 14, `sync_espeak_Key` 11, `sync_espeak_Char` 8, `sync_espeak_SetPunctuationList` 12, `espeak_SetSynthCallback` 2, `espeak_ng_Synthesize` 11, `espeak_ng_SynthesizeMark` 11, `espeak_ng_SpeakKeyName` 4, `espeak_ng_SpeakCharacter` 2, `espeak_GetParameter` 4, `espeak_ng_SetParameter` 2, `espeak_ng_SetPunctuationList` 3, `espeak_SetPhonemeTrace` 9, `espeak_TextToPhonemesWithTerminator` 16, `espeak_TextToPhonemes` 4, `espeak_ng_Cancel` 5, `espeak_IsPlaying` 2, `espeak_ng_Synchronize` 4, `espeak_Info` 6 | `create_events` 16, `espeak_ng_InitializeOutput` 16, `Synthesize` 71, `espeak_ng_Terminate` 27 | 1 |
| `src/libespeak-ng/ssml.c` | `SsmlWideSpace` 3, `SsmlByteSpace` 5, `SsmlByteLower` 1, `SsmlDecimalPoint` 6, `SsmlResolveVoiceName` 14, `SsmlSelectVoice` 17, `SsmlUpdateRate` 4, `espeak_SetUriCallback` 2 | `ProcessSsmlTag` 24 | 1 |
| `src/libespeak-ng/synthdata.c` | `RustSpectrumTransition` 8, `SelectPhonemeTableName` 6, `InvalidInstn` 5, `RustPhonemeStorage` 46, `RustPhonemeDataLength` 2, `RustPhonemePrograms` 3, `RustInvalidInstruction` 2, `InterpretPhoneme2WithData` 22, `InterpretPhoneme2` 2, `TonePhoneme` 6 | `ReadPhFile` 45, `LoadPhData` 50, `FreePhData` 31, `LookupSpect` 19, `SelectPhonemeTable` 18, `InterpretPhonemeWithLength` 33 | 4 |
| `src/libespeak-ng/synthesize.c` | `WordToString` 6, `SynthesizeInit` 7, `FormantTransition2` 2, `GenerateEnvelope` 8, `SpeakNextClause` 47, `espeak_SetPhonemeCallback` 2 | `CommandSettings` 27, `CommandEffect` 80, `FormantTransitionWithCapacity` 25, `GenerateEffect` 100, `Generate` 48 | 15 |
| `src/libespeak-ng/tr_languages.c` | - | - | 3 |
| `src/libespeak-ng/translate.c` | `FreeAlternateTranslators` 11, `lookupwchar` 7, `strchr_w` 6, `ShouldSplitIdeographs` 23, `SegmentReplacement` 44, `TranslateWordWithBounds` 69, `TranslateWord` 2, `SetPlist2` 7, `CountSyllables` 8, `Word_EmbeddedCmd` 21, `SetAlternateTranslator` 29, `SetTranslator2` 2, `SetTranslator3` 2, `TranslateWord2` 287, `TranslateWord2WithContext` 9, `EmbeddedCommand` 46, `FindReplacementChars` 39, `SubstituteChar` 40, `TranslateChar` 49, `UpperCaseInWord` 18, `TranslateClauseWithTerminator` 807, `TranslateClause` 4, `CalcWordLength` 11, `CombineFlag` 67, `SwitchLanguage` 18, `InitText` 22 | - | 1 |
| `src/libespeak-ng/translateword.c` | `LookupEmojiBaseSequence` 93, `TranslateWord3` 522, `SpeakIndividualLetters` 26, `TranslateLetter` 209, `addPluralSuffixes` 26, `CheckDottedAbbrev` 44, `NonAsciiNumber` 10, `Unpronouncable` 50, `Unpronouncable2` 16 | - | 2 |
| `src/libespeak-ng/voices.c` | `RustVoiceLength` 2, `RustCatalogDiagnostic` 11, `RustVoiceWorkspace` 9, `RustVoiceDirectory` 9, `RustOrdinalEnvironment` 6, `RustActiveVoiceHost` 45, `LoadVoiceVariant` 15, `espeak_ng_SetVoiceByFile` 24, `espeak_ng_SetVoiceByName` 43, `espeak_ng_SetVoiceByProperties` 10, `espeak_GetCurrentVoice` 2 | `LoadVoice` 65, `SelectVoice` 26 | 9 |
| `src/libespeak-ng/wavegen.c` | `WavegenFini` 2, `Write4Bytes` 6, `WavegenFill` 4, `espeak_ng_SetOutputHooks` 3 | `WavegenEffect` 38 | 14 |
| `src/speechPlayer/src/frame.cpp` | `create` 1 | - | 0 |
| `src/speechPlayer/src/speechPlayer.cpp` | `speechPlayer_initialize` 7, `speechPlayer_queueFrame` 4, `speechPlayer_synthesize` 2, `speechPlayer_getLastIndex` 3, `speechPlayer_terminate` 5 | - | 0 |
| `src/speechPlayer/src/speechWaveGenerator.cpp` | `create` 1 | - | 0 |
