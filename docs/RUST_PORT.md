# Rust migration

Jay requested a native Rust port of this fork on 2026-10-04, with loadngo's
proactor where applicable and NPU use where useful. This is an incremental
replacement of the engine. It is not yet a complete Rust synthesizer.
The retained C implementation at the starting commit `ba90c8e9` is the
behavior oracle, including this fork's language data and Unicode version.

## Implemented

| Original module | Native Rust implementation | Engine status |
|---|---|---|
| `encoding.c` | `rust/encoding.rs`, fixed encoding tables | Replaces C with `USE_RUST_CORE=ON`; all encodings, aliases, AUTO, wide strings and cursor API |
| `mnemonics.c` | `rust/mnemonics.rs` plus byte-exact C adapter | Replaces C; first match and sentinel defaults |
| `ieee80.c` | `rust/ieee80.rs` | Replaces C; AIFF sample-rate decoding |
| All six `ucd-tools/src/*.c` modules | `rust/unicode.rs`, fixed tables | Replaces C; categories, scripts, properties, case conversion and character classifiers |
| `phoneme.c` | `rust/phoneme.rs` | Replaces C; 16-byte phoneme records, feature names and articulatory-feature mutations |
| Compiled dictionary storage and indices | `rust/dictionary.rs`, `rust/rules.rs` | Replaces C bucket/rule indexing and `HashDictionary`; native resident owner caches indices |
| Letter-to-phoneme template VM and string groups | `rust/rule_match.rs` | Replaces `MatchRule`, `$list`/`$p_alt` scoring and `IsLetterGroup`; prefix lookup frontend and trace formatting supplied through an explicit environment |
| Rule translation orchestration | `rust/translate_rules.rs`, `rust/translate_rules_compat.rs` | Replaces complete `TranslateRules` policy, group scoring/order, digit and letter fallbacks, accent retries, language switches, endings, deletion and source restoration; serialized C source/state and matcher projections remain |
| Main word translation | `rust/translate_word.rs`, `rust/translate_word_compat.rs` | Replaces complete `TranslateWord3` policy, dictionary/key/textmode admission, numbers/spelling, prefix/suffix retries, stress/grammar, joins and restoration; bounded numeric sources and owned pronunciation, with remaining serialized C owners and primitive adapters |
| Word-to-phoneme-list control | `rust/translate_word2.rs`, `rust/translate_word2_compat.rs` | Replaces complete word dispatch, retries, stress/tone/list assembly and restoration; scoped owned replacement and bounded live projections retain serialized C owners |
| Replacement segmentation and expansion | `rust/replacement.rs`, `rust/replacement_compat.rs` | Replaces ideograph splitting, segmentation and recursive text expansion; bounded owned scratch, checked joins/skips and live primitive effects |
| Clause translation control | `rust/translate_clause.rs`, `rust/translate_clause_compat.rs` | Replaces complete tokenization, word/number/spelling dispatch and final publication; scoped rule/clause context and initialized owned scratch, with serialized primitive/resource owners still C |
| Character conversion | `rust/character_conversion.rs`, `rust/character_conversion_compat.rs` | Ordered replacement matching/substitution, Hangul, Afrikaans/Dutch apostrophes and Irish uppercase policy; checked numeric dictionary/source reads, with serialized owners still C |
| Connected text control | `rust/text_control.rs`, `rust/text_control_compat.rs` | Embedded parsing/application, combination/syllables, language switch and reset policy; live effects/publications with scoped primitive context and retained C owners |
| Alternate-language lifecycle | `rust/alternate_translator.rs`, `rust/alternate_translator_compat.rs` | Replaces admission, cache comparison/publication, resource replacement/recovery and release order; copied bounded names and live checked slots; alternate slots/cache names and global admission still C |
| Scalar language letter predicates | `rust/letters.rs`, generated accent table | Replaces `IsLetter` used by rule matching, vowels and stress; borrows prepared native language configuration |
| Suffix removal and UTF-8 output | `rust/suffix.rs` | Replaces `RemoveEnding` and `utf8_out`; bounded edit planning, spelling repairs and explicit grammatical/history effects; long words supported |
| Word-stress extraction and assignment | `rust/word_stress.rs` | Replaces `GetVowelStress` and `SetWordStress`; sparse selected tables, all language stress-position rules and explicit previous-stress effects; clause/intonation stress remains C |
| Translation word transforms | `rust/word_stress.rs`, `rust/phoneme_word.rs` | Replaces `ChangeWordStress`, `AppendPhonemes` and `ApplySpecialAttribute2`; admitted writes, checked vowel/stress counters and explicit compatibility byte signedness |
| MBROLA mapping storage and names | `rust/mbrola.rs` | Replaces table reads/storage and `GetMbrName`; reusable transactional owners, validated little-endian records and explicit prefix effects; backend process/audio remains C |
| MBROLA command generation | `rust/mbrola_generate.rs`, `rust/mbrola_generate_compat.rs` | Replaces `MbrolaTranslate` decisions and resume cursors; bounded pending commands retain partial-write progress; acoustic/marker effects and process admission use the compatibility callback |
| MBROLA output sample cursor | `rust/mbrola_fill.rs`, `rust/mbrola_fill_compat.rs` | Replaces `MbrolaFill` accounting and resume state; bounded caller PCM, partial reads and explicit pending/end outcomes; the C adapter retains the blocking backend reader |
| Contextual dictionary exception lookup | `rust/lookup.rs` | Replaces `LookupDict2`; explicit grammatical context, conditions, stress/word flags, multiword matches, precedence and legacy output side effects |
| Dictionary-list and symbol lookup | `rust/lookup_list.rs`, `rust/lookup_symbol.rs`, `rust/list_host_compat.rs` | Replaces list/symbol/prefix policy and connected typed source/repeat/replacement/global projections; nested source restoration and pronunciation copying use owned bounded scratch; primitive lookup/accent/word translation/trace effects and engine integration remain |
| Dictionary alphabet compression | `rust/word_key.rs` | Replaces `TransposeAlphabet`; language maps, frequent pairs, six-bit packing and byte-exact legacy hash tails |
| Compiled phoneme tables and header | `rust/phoneme_data.rs` | Replaces C table parsing, inheritance overlays, name lookup and phondata header decoding; data compiler still C |
| Compiled phoneme-program VM | `rust/phoneme_program.rs` | Replaces `InterpretPhoneme` bytecode execution, instruction widths and vowel-switch decoding; uses native bounded context or an explicit owner environment |
| Phoneme condition/stress evaluation | `rust/phoneme_context.rs` | Replaces `InterpretCondition`, `StressCondition` and vowel-position counting; explicit initialized list bounds, table resolution and isolated previous-vowel snapshot; phoneme-list construction and stress assignment still C |
| Spectrum lookup and envelopes | `rust/spectrum.rs` | Replaces `LookupSpect` selection/scaling and `GetEnvelope` addressing; bounded ordinary/Klatt record views, vowel split, secondary append and duration adjustment |
| Formant transitions and frame copies | `rust/formant.rs` | Replaces `FormantTransition2`, formant/RMS adjustments, coloring and `CopyFrame` math; native admitted pool plus compatibility queue-owned storage |
| Spectrum smoothing | `rust/smoothing.rs` | Replaces `SmoothSpect` with bounded backward/forward ring traversal, frequency-rate limiting and shared frame-link repair; reusable planning workspace and actual-copy admission before mutations |
| Acoustic voice configuration | `rust/voice.rs` | Replaces acoustic `VoiceReset`, formant/pitch/tone/breath/Klatt and related attribute parsing, `Read8Numbers` and `ReadTonePoints`; backend resets and speed recomputation still C |
| Voice metadata and matching | `rust/voice_selection.rs` | Replaces metadata parsing, `ScoreVoice`, `SelectVoiceByName` matching and variant suffix extraction; bounded native metadata and borrowed matching; backend setup remains hybrid |
| Voice ordering, candidate ranking and property selection | `rust/voice_catalog.rs` | Replaces catalogue ordering, `SetVoiceScores`, visibility filtering and `SelectVoice` algorithms; caller-owned bounded workspace, cached score effects, fallback and variant cycling |
| Catalogue discovery and ownership | `rust/voice_storage.rs`, native ABI owner | Replaces catalogue directory walking, metadata file reads, record/result-array/workspace allocation and release; stable records and incremental parsing for host-loaded chunks; serialized compatibility loading is synchronous initialization work |
| Ordered active-voice metadata | `rust/voice_setup.rs` | Replaces language/name/gender/dictionary/phoneme directives in `LoadVoice`; bounded setup snapshot and explicit first-language effect; C owner still performs file and backend operations |
| Active voice request planning | `rust/voice_request.rs` | Replaces path/name resolution, fallback controls and current variant identifiers; bounded snapshots and explicit owner probes; file opening and directive orchestration remain owner work |
| Phoneme names and backend directives | `rust/phoneme.rs`, `rust/voice_backend.rs` | Replaces `PhonemeCode`, `LookupPhonemeString`, phoneme replacement rules and MBROLA request parsing; sparse table lookup and bounded replacement state; backend startup/output still C |
| Mutable language options | `rust/language_options.rs` | Replaces `LoadLanguageOptions`, `ReadNumbers` and separator processing; instance-owned stress arrays, tune selection, number flags and language parameters; configuration I/O still C |
| Static translator presets and alphabet classification | `rust/language.rs`, generated native tables | Replaces `SelectTranslator` configuration and `AlphabetFromChar` classification; shared immutable tables, native instance options, bounded dictionary names and prepared letter/compression views; typed translator allocation now Rust |
| Typed translator storage and dictionary bindings | `rust/translator.rs`, `rust/translator_compat.rs` | Rust owns allocation, initialization, pinned dictionary handles, checked index publication, replacement and destruction; C retains serialized globals, slots, path/diagnostic/cache admission and projections |
| Data I/O and resident assets | `rust/data_io.rs`, `rust/resident.rs`, optional `proactor` feature | Native library loads and indexes complete resident asset sets; caller-owned loadngo proactor, reusable bounded buffer, one plan/read in flight; legacy C byte loader still uses stdio |
| Accelerator capability | `rust/acceleration.rs`, optional `npu` feature | Core ML device discovery on macOS; portable CPU fallback; no NPU speech computation enabled |
| Common copy, stream-word and random primitives | `rust/common_primitives.rs`, `rust/common_primitives_compat.rs` | Replaces `strncpy0`, `Read4Bytes`, `espeak_rand` and `espeak_srand`; owned native random state, atomic compatibility state, bounded copy/padding and host CRT stream reads |
| `intonation.c` pitch calculation | `rust/intonation.rs`, `rust/intonation_compat.rs` | Replaces `CalcPitches` and `CalcPitches_Tone`; copied phoneme-list snapshot, borrowed compiled tunes and phoneme table, native head/nucleus tables, atomic rejection of inputs C reads out of bounds |
| `setlengths.c` `CalcLengths`, `intonation.c` envelope tables | `rust/lengths.rs`, `rust/lengths_compat.rs`, `rust/envelope.rs` | Replaces `CalcLengths`; copied list snapshot including the entries C reads past the clause, explicit cross-clause syllable state, ordered host callbacks for embedded speed and tone envelopes; `envelope_data` and `env_fall` are exported from Rust |
| `phonemelist.c` | `rust/phoneme_list.rs`, `rust/phoneme_list_compat.rs` | Replaces `MakePhonemeList`, `SubstitutePhonemes`, `SetRegressiveVoicing` and `ReInterpretPhoneme`; owned working list with native phoneme-program storage, host table selection, outputs as (table, slot) references resolved to the legacy record pointers |
| `synthesize.c` `Generate` | `rust/generate.rs`, `rust/generate_compat.rs` | Replaces the clause driver's decisions and resumable state; queue writers (`DoPause`, `DoPitch`, `DoAmplitude`, `DoSpect2`, `DoSample3`, markers, embedded commands), phoneme programs and frames stay C behind one ordered effect callback; MBROLA keeps its own generator |
| `synthesize.c` command writers | `rust/commands.rs`, `rust/commands_compat.rs` | Replaces `DoSpect2`, `DoSample2`/`DoSample3`, `DoPause`/`PauseLength`, `DoPitch`, `DoAmplitude`, `EndPitch`, `EndAmplitude` and `StartSyllable`; Rust owns their shared state; the queue, spectrum lookup, smoothing and frame copies are host operations, and formant-transition pauses are returned from the lookup rather than issued re-entrantly |
| `wavegen.c` | `rust/wavegen.rs`, `rust/wavegen_compat.rs` | Replaces the formant wave generator and queue consumer: `WavegenFill2`, `Wavegen`, `SetSynth`, `PeaksToHarmspect`, `AdvanceParameters`, breath resonators, `PlaySilence`, `PlayWave`, `SetPitch`, `SetAmplitude`, `SetEmbedded`, echo setup, `WavegenSetVoice`, `GetAmplitude`, `InitBreath` and `WavegenInit`; Rust owns the generator state and voice copy; the embedded values and sample rate stay shared C memory read in place; Klatt, MBROLA, sonic, markers, output hooks and the random generator are host operations |
| `klatt.c` | `rust/klatt.rs`, `rust/klatt_compat.rs`, `rust/klatt_data.rs` | Replaces the complete cascade/parallel Klatt synthesizer, all five glottal sources, filters, flutter, interpolation, mixing and fades; an owned native instance keeps every source/filter history; the C shell supplies resident queue/sample access, PCM/echo output, the random stream and speechPlayer delegation |
| `wavegen.c` queue and echo ring | `rust/wave_memory.rs`, `rust/wavegen_compat.rs` | Rust owns `wcmdq` with its head/tail and the echo ring (`espeak_rs_wave_memory`) and replaces `WcmdqFree`, `WcmdqUsed`, `WcmdqInc`, `WcmdqIncHead`, the queue reset in `WcmdqStop`, and Klatt's echo reads and writes; the queue writers below run in Rust |
| `synthesize.c`/`synth_mbrola.c` queue writers | `rust/wave_memory.rs`, `rust/commands.rs`, `rust/wave_memory_compat.rs` | Replaces `DoMarker`, `DoPhonemeMarker`, `DoPhonemeAlignment`, `DoSonicSpeed`, `DoVoiceChange`'s queue entry, `DoEmbedded` (speed changes, sound icons, marks, audio and generator commands) and the MBROLA output entries; the command writers write the Rust queue directly; smoothing and Klatt's and speechPlayer's look-ahead still read it in place |
| `speech.c` output buffer, `synthesize.c` frame pool | `rust/output.rs`, `rust/wave_memory_compat.rs` | Rust allocates, resizes and frees the PCM output buffer and owns its cursor (`espeak_rs_output`; C's `out_ptr`/`out_end` are macros over it), and owns the round-robin pool of modified frames (`espeak_rs_frame_pool`) with the storage callback the frame copy, transition and smoothing calls use; Klatt, speechPlayer, MBROLA, sonic and events still advance or read the cursor in place |
| `speech.c` event list, `wavegen.c` embedded values | `rust/events.rs`, `rust/wave_memory_compat.rs` | Rust allocates, resizes and frees the event list (`espeak_rs_events`) and replaces `MarkerEvent`, `RescaleEventSamples`, list termination and the message terminator; the embedded values and their defaults are Rust statics under C's names, which the remaining C readers and writers address in place |
| Engine file I/O | `rust/engine_io.rs`, `rust/data_io.rs` | Every engine file read (phoneme data, dictionaries, voices, variants, sound icons and their configuration, MBROLA tables, the voice catalogue's files) goes through loadngo's proactor (io_uring; epoll, kqueue or IOCP elsewhere); `std::fs` only without the `proactor` feature or proactor |
| Dictionary compiler lifetime and output | `rust/dictionary_compile.rs`, `rust/dictionary_compile_compat.rs`, `rust/engine_io.rs` | Owns rule-path admission, output creation, list/rule ordering, header patching and registered pooled proactor writes; C retains serialized translator/classifier/diagnostic and reload projections |
| Dictionary flag and compiled-rule trace rendering | `rust/dictionary_trace.rs`, `rust/dictionary_trace_compat.rs` | Bounded pure formatting with owned mnemonic tables and fixed stack scratch; native rule traces carry their admitted dictionary extent, preserving defined legacy display bytes and safely handling metadata that underflowed C scratch |
| Compiler source ownership and tokens | `rust/compiler_source.rs`, `rust/compiler_source_compat.rs` | Owned phoneme/include/intonation snapshots, EOF/line accounting, bounded tokens/numbers and replay over retained proactor source buffers |
| Complete intonation compiler | `rust/intonation_compile.rs`, `rust/intonation_compile_compat.rs` | Two-pass discovery, grammar/defaults/envelopes, diagnostics and explicit records; owned source/output lifetime with engine proactor writes |
| Spectral compiler sources and packing | `rust/spect_file.rs`, `rust/spect_compile.rs` | All three source formats, owned frame/bin/source arenas, interpolation/RMS and complete ordinary/Klatt phondata packing; compatibility views remain for legacy callers |
| Phoneme compiler resources | `rust/compiler_resource.rs`, `rust/compiler_resource_compat.rs` | Complete dispatch, proactor reads, WAV/envelope encoding and owned reference cache/payload/manifest publication; parent compiler control remains C |
| Phoneme compiler output set | `rust/compiler_output.rs`, `rust/compiler_output_compat.rs`, `rust/engine_io.rs` | Owns all five file lifetimes, creation/fallback policy, initial words, bounded retained staging, completed proactor writes and failure cleanup; C retains property/bytecode production, diagnostics and manifest/reload control |
| Phoneme compiler tables | `rust/compiler_tables.rs`, `rust/compiler_tables_compat.rs` | Complete reserved-code, lookup/find/import, inheritance, undeclared-reference and packing policy in retained row arenas; separate C views temporarily serve the remaining property/bytecode producer |
| MBROLA child stdio | `rust/mbrola_process.rs` | Safe persistent Unix session with caller-owned loadngo send/recv, bounded reusable loans, streaming WAV decoding and explicit whole-input EOF; C-engine generation/output and final process lifecycle integration remain |
| Synthesis scheduling | `rust/synthesis_loop.rs` | Caller-owned completion port, bounded locally-ready passes, pending/wake/cancel and callback lifetime fencing; nested runs share the port; async cancellation reaches registered native audio waits; C pending process/file I/O and synchronous cancellation integration remain |
| Synthesis request control | `rust/engine_request.rs`, `rust/engine_request_compat.rs` | Native synchronous text/mark preparation and public text, mark, key, character, parameter and punctuation admission; typed failure returns preserve ownership until queue success, with atomic text/notification pairs; parameter/skip resources and key/character/translation primitives still use the serialized C engine |
| Synthesis startup and passes | `rust/engine_driver.rs`, `rust/engine_driver_compat.rs` | Native proactor-path startup, buffer delivery, status propagation and clause/end control over serialized projections; checked cursor/count accounting; existing completion runner with no replay on refusal; proactor-off retains its C driver, while pending process I/O and owned engine/resource integration remain |
| Audio dispatch and event timing | `rust/engine_audio.rs`, `rust/engine_audio_compat.rs` | Proactor-path rate/reopen control, single PCM delivery, bounded fresh event-prefix traversal and event admission; widened/clamped sample timing over existing device and completion waits; serialized resources and proactor-off C control remain |
| Number/spelling primitives | `rust/number_primitives.rs`, `rust/number_primitives_compat.rs` | Native superscript mapping, in-place spelling stress/pause policy, thousands variants/group boundaries, Hungarian suffix forms and Roman recognition/cursor; checked output capacity and initialized-prefix adapters |
| Thousands-name dictionary control | `rust/number_lookup.rs`, `rust/number_lookup_compat.rs` | Native exact/ordinal/e/x/variant lookup order, lower-power probes, missing-name fallback state and output concatenation; initialized stack scratch and actual caller capacities; dictionary/state primitives remain C |
| Tens/units and hundred controllers | `rust/number_digits.rs`, `rust/number_digits_compat.rs` | Native cached/dictionary/feminine/ordinal/year/vigesimal forms, joins, vowel elision and stress; direct native nested two-digit/thousand calls, bounded output and fresh prefix/state projections; translator/text/phoneme owners remain C |
| Dot ordinal, main number and Roman controllers | `rust/number_ordinal.rs`, `rust/number_frontend.rs`, `rust/number_roman.rs` | Native context/form decisions, parsing, grouping, fractions, numeric/ordinal source assembly, dictionary reset and final joins; C source/word/state/dictionary projections remain |
| Letter/symbol and diacritic lookup | `rust/letter_lookup.rs`, `rust/letter_lookup_data.rs` | Native normal/spelling/space lookup ordering, accent/ligature tables and assembly, English fallback and stress admission; actual caller capacities reach accent fallback; C dictionary/voice/stress projections remain |
| Isolated-letter translation | `rust/translate_letter.rs`, `rust/translate_letter_compat.rs` | Replaces complete `TranslateLetter` policy, capitals, superscripts, script names, secondary/Hangul fallback, hexadecimal/braille and final joins; owned bounded scratch and fresh output publication over serialized engine primitives |

The safe library has no runtime dependency on the C engine. The `c-abi`
feature adds compatibility exports; the algorithms ported here execute in Rust.
The rule matcher's C adapter borrows language letter configuration and supplies
prefix dictionary lookup and trace formatting through synchronous callbacks.
Scalar letter checks execute directly in Rust without a C callback.
Normal Cargo builds use committed native tables. Regeneration alone uses
the original C files as a data oracle, via `tools/generate_rust_tables.py`.
The port retains GPL-3.0-or-later and original notices; see `COPYING` and
`COPYING.UCD` for engine/data licensing.

## Build and use

From the repository root, with Rust/Cargo and CMake installed:

```sh
cargo test --locked --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --check
python3 tools/generate_rust_tables.py --check
python3 tools/generate_rust_languages.py --check

cmake -S . -B build-rust -DUSE_RUST_CORE=ON -DUSE_ASYNC=OFF \
  -DUSE_LIBPCAUDIO=OFF -DUSE_LIBSONIC=OFF -DUSE_MBROLA=OFF
cmake --build build-rust --parallel 3
ctest --test-dir build-rust --parallel 3 --output-on-failure

ESPEAK_DATA_PATH="$PWD/build-rust" build-rust/src/espeak-ng -xq -v en "Hello world"
ESPEAK_DATA_PATH="$PWD/build-rust" build-rust/src/espeak-ng -w /tmp/hello.wav -v en "Hello world"

cargo run --locked --features npu --example hardware
cargo run --locked --features proactor --example resident_data -- \
  --data-dir build-rust/espeak-ng-data --dictionary en --dictionary si
```

The original C build remains available with `USE_RUST_CORE=OFF` (the
default during migration). Each CMake build has its own Cargo target
directory. Cross builds must specify `ESPEAK_RUST_TARGET` to match the C
target and install that Rust target/toolchain. A host Rust archive must
never be linked into a different C architecture. Sanitizer builds currently
instrument the remaining C code, not the Rust archive.

## Proactor contract

Enable `proactor` and pass the host's `ProactorHandle<P>` to
`DataReader::read`. Open files during initialization or on the host's
existing offload path using `open_data_file`; file opening is synchronous.
Windows opens use `FILE_FLAG_OVERLAPPED` for IOCP. Positioned reads then use
loadngo's platform file I/O path (including its worker offload on macOS).
No independent proactor, polling loop, timer thread or per-read worker
is constructed by the library.

Each reader allocates one 1-byte to 1-MiB chunk buffer, and clones share it.
Busy admission returns `WouldBlock`; successful completions reuse the same
allocation, including after short reads and EOF. A callback borrows the
chunk and must return promptly. Submit the next chunk after returning to
the host. Exceptional I/O failures replenish a buffer because the port's
error API does not return its buffer. Keep assembled dictionary/phoneme
data resident; do not reload it per utterance or audio callback.

The reader retains the file until completion. Cancellation uses the
returned `IoOpId` and `handle.cancel_io`; cancelling does not immediately
return the buffer. Shutdown must stop and drain the host's proactor.
CPU-heavy translation/synthesis must eventually run on a bounded host
offload path, with completion delivery through the proactor.
`enqueue_work` alone is completion delivery, not CPU offload.

`PreparedAssets::open` prepares phontab, phonindex, phondata, intonations and
the selected dictionaries during initialization/on an I/O worker. It opens
and reserves each asset allocation once. The caller supplies a combined byte
limit of at most 128 MiB, with at most 128 unique dictionary identifiers.
Assets must remain stable while loading; this is not a filesystem snapshot.
`ResidentLoader::load` accepts that plan and a host handle. Clones share
admission, and a second concurrent plan returns `WouldBlock`. Successful
chunks reuse the reader buffer and append within the pre-reserved allocation.
No large buffer is reallocated per chunk. Posted work only advances the
read chain after the previous loan returns; no CPU parser runs there.

Completion returns `ResidentBytes`. Call `index()` during initialization or
on the host's bounded CPU worker to produce immutable `ResidentAssets`.
It keeps the original allocations and caches phoneme/dictionary indices.
Raw phonindex, phondata and intonation payloads are retained for the later
interpreter ports. Phonindex word alignment is validated; full spectrum and
intonation schemas are not validated yet. The `resident_data` example is a
standalone host, loading real assets
through loadngo before indexing them outside the completion loop.

`OwnedDictionary::lookup` uses those cached indices and an immutable
`lookup::Context`; it borrows the selected phonemes and does no allocation or
I/O. Prepare keys with `word_key::Alphabet` for languages using compression.
Keep the returned descriptor and the whole buffer: embedded NUL bytes count
toward matching, while the unchanged tail can still affect the dictionary hash.
The compatibility C frontend snapshots its grammatical state for this same
Rust matcher. Native `lookup_list` owns abbreviations, text replacement,
repetition and ending handling; serialized source/state projections and the
translator's replacement storage remain C.

`OwnedDictionary::match_group` executes a cached rule-group offset using
`rule_match::Context` and an `Environment` supplied by the engine owner. It
returns scores, phoneme offsets, ending flags, an optional deletion offset and
the number of consumed bytes. The VM and string-list matcher use bounded slices,
borrow resident storage and allocate nothing. Prefix checks use one fixed
160-byte stack buffer. Environment callbacks must remain bounded and synchronous;
an utterance belongs on the host's bounded CPU offload path, with a completion
posted through its proactor. No executor, polling loop or timer is introduced.

`letters::LetterSet` borrows a 256-byte bitfield, an alphabet offset and up to
eight wide-character lists. It preserves the original mask-valued results,
accent mapping, wide-list precedence and NUL matches. `WideLetters` models
16-bit Windows and 32-bit macOS/Linux `wchar_t` units without UTF-16 decoding.
The C compatibility layer caches static wide-list lengths once during
`SelectTranslator`, then borrows them during matching and stress checks.
No allocation or string-length scan occurs per predicate; configuration must
remain immutable for each synchronous match, including its callbacks. The
resident asset integration test combines host-proactor loading, cached native
rule matching and this native language predicate with no C dependency.

`ResidentAssets::phoneme_programs` borrows little-endian phonindex instructions
without copying them. `Program::interpret` returns fixed-size `PhonemeData`:
length/change/append parameters, selected sound addresses, signed vowel
adjustments, pitch/amplitude envelope addresses, vowel transitions and IPA.
It handles nested calls with the fork's ten-entry return stack, conditional
chains and jumps, six-way vowel switches and implicit sound returns. It checks
instruction accesses and stops after 65,536 execution steps, including
condition-chain steps. No heap allocation occurs during execution.

`phoneme_context::Context` implements `phoneme_program::Environment` with native
condition/stress evaluation and neighbouring vowel types. Owners borrow reusable
entry/table storage through `SliceStorage`, or implement `Storage` to read and
resolve bounded resident records. `Settings` supplies initialized list length,
current index, reduction policy and backend flags. Clauses are limited to 1,001
entries; scans stop at their bounds or word boundaries. The previous vowel is
an isolated snapshot with no adjacent entries. No heap allocation occurs during
evaluation. The resident integration test executes both stress branches from
host-proactor-loaded instructions without C.

Every C engine caller now passes its initialized list span, including explicit
pause sentinels. Its adapter supplies scalar snapshots and table-code resolution;
the condition algorithms execute in Rust. It updates existing phoneme/word state
after successful interpretation and records phonindex length during loading.
Neither the interpreter nor its storage callbacks perform I/O. Keep these
synchronous CPU calls on initialization or the host's bounded worker path,
outside proactor completion handlers. Full clause ownership and stress assignment
remain to be ported.

The last compiled sound record can end at EOF. The original C interpreter
reads one word past its allocation for sound lookahead; native execution
treats exactly that terminal boundary as Return. Other truncated reads and
invalid/cyclic control flow return an error. The C adapter keeps default
phoneme parameters on failure. Bounded storage access prevents native condition
scans from crossing clause ends or scanning past the previous-vowel snapshot.
The serialized C adapter still requires valid live pointers and initialized
spans; these guards do not establish safety of the remaining C engine.

`ResidentAssets::spectra` borrows resident phondata through `SpectrumData`.
Sequences validate their complete 44-byte ordinary or 64-byte Klatt records;
frame views decode little-endian flags/frequencies and expose borrowed bytes.
Lookup reuses 25 owner-supplied `FrameRef` slots, preserves the reserved transition
slot, splits at the last vowel-center marker and appends secondary spectra. The
first secondary frame supplies only the previous terminal frame's duration.
Native integer scaling preserves C truncation while rejecting arithmetic overflow.
`envelope` checks all 128 bytes; the C adapter uses its default envelope on invalid
addresses. Empty, truncated, unaligned and oversized combined sequences fail,
as do inconsistent Klatt layout flags or resident writable-copy markers.

`spectrum::Environment` resolves frame handles into resident storage and supplies
consonant blending when requested. `Offsets` permits ordinary selection without
copying frames; requesting blending without an implementation returns an error.
`formant::ResidentPool` supplies native blending with decoded writable frames,
voice snapshots and explicit queue effects. It applies the fork's RMS table,
frequency adjustment, vowel coloring, Klatt amplitude changes and glottal
modulation. Copies read only the flagged 44/64 bytes; ordinary extensions are
zeroed. `Pool::new` allocates once at engine initialization for 1–170 frames.
Slots stay admitted until the owner calls `release` after consumption; generation
checks reject stale handles. Capacity is reserved before transition mutations,
and a full pool returns an error so the host can drain or apply backpressure.
Freed slots are overwritten in place; no allocation occurs during transitions.

`ResidentPool::lookup` resets pending effects and selects/blends into reusable
references. The caller submits its returned pause/modulation intent on the host
and retains all pooled handles until output is consumed. Direct use through
`spectrum::Environment` must likewise clear old effects before each lookup.
Effects do not themselves create a worker, polling loop or scheduler. The
proactor-resident integration test loads bytes through loadngo, performs native
selection/blending, consumes queue effects, releases a frame and verifies reuse.

The C adapter supplies allocation and pool-membership callbacks and snapshots
voice fields. All transition/copy algorithms execute in Rust. The adapter applies
queue effects through existing C commands and passes the available reference
capacity explicitly. It retains the compatibility engine's serialized 170-frame
cyclic pool and queue lifetime contract; it does not yet use the native admitted
pool. Queue construction and waveform generation remain C.
Owner callbacks must preserve resident bytes and retain modified frame storage.
Selection/scaling/blending perform no allocation, I/O or private scheduling. Run
them on the host's bounded CPU path, outside proactor completion handlers. These
scalar operations do not supply an NPU compute partition.

`smoothing::smooth` processes an explicit ring of at most 170 four-word commands
and a bounded syllable start/end/center snapshot. It preserves pause/wave stops,
discontinuous frame chains, low-frequency breaks, rate flags, ordinary/Klatt
frames and adjacent shared handles. A caller-owned `Workspace` plans all changes
against value snapshots without mutating queued frames. Only the actual new
copies are reserved; an already full pool can still admit a pass needing no
new copies. Invalid bounds, arithmetic overflow or insufficient native pool
capacity leave frames, ring and syllable start intact. After successful
admission, owner storage must fulfill writes/allocations; an unexpected owner
failure requires discarding/draining the partially committed syllable.

The native API reuses its workspace across syllables. The C adapter uses bounded
stack scratch for the same plan and the compatibility cyclic frame pool; no
heap allocation or private scheduler is introduced. Resident integration loads
assets with the host proactor, smooths on the caller's CPU path, then releases
the consumed handles. The six-frequency sequential rate limiter is scalar CPU
work; this stage does not introduce an NPU computation or placement claim.

`voice::Voice` owns an acoustic snapshot. `reset` returns the sample-rate-adjusted
formant limits and fast default; it preserves name/language/table metadata.
`apply` consumes one borrowed directive and returns whether its owner should
recompute speed. The native `Directives` iterator borrows complete configuration
bytes and reproduces fgets chunk widths, comments and whitespace, including
vertical tabs. `formant_settings` supplies a voice snapshot to native blending.
Configuration parsing runs during setup or on the host CPU path, after proactor
I/O completes. No parsing is disguised as proactor worker offload.

Tone curves are bounded to 1,000 bins at 8 Hz intervals. Native integer parsing,
interpolation, sample rates and arithmetic are checked before committing a
directive or reset. Rejection leaves native voice/fast/points state unchanged.
Defined legacy partial assignments, missing formant fields, short truncation,
zero/EOF scanner counts, negative-value preservation and speed intent are
retained. The C ABI reports rejected acoustic attributes; legacy numeric helper
exports return initialized fallback arrays for out-of-range inputs. This
defines behavior where the original scanner/math could overflow or write
outside its tone table.

The compatibility layer still opens voice files through stdio, selects language
and dictionaries, applies translator options and phoneme replacements, and
resets backend breath/MBROLA state. Actual speed recalculation and waveform
generation remain C. Native applications can feed proactor-read configuration
bytes directly to `Directives`/`Voice`; a regression verifies this path outside
completion handlers. Acoustic setup is scalar CPU work, with no NPU operation.

The compatibility frontend borrows explicit clause/number windows for the
duration of a word translation and restores the previous window afterward.
This preserves PRE rules that inspect earlier words or digits. Standalone
inputs retain their accessible preceding byte without probing farther backward.
The adapter records the resident dictionary allocation bound on successful
loading and passes the platform's plain-char signedness for ending bytes.
`LookupFlags` frontend behavior remains a C callback; `IsLetter` scalar language
classification and `IsLetterGroup` string matching are native Rust.

`LoadCancellation::cancel` is cooperative: let the outstanding chunk complete,
then report `Interrupted` before another read. No timer or polling is needed.
Cancel outstanding plans and wait for their terminal completion before
stopping/draining the host. Dropping a cancellation token does not cancel a
load. This API does not create a private runtime inside the compatibility C
library. The C engine now uses Rust indices but still opens/reads its own
data through stdio; adopting a caller-owned asset store there remains work.

## NPU boundary

The hardware example queries public Core ML device capability through
loadngo, once at initialization. On this Mac on 2026-10-04 it reported
`[Npu, Cpu]`. That is available hardware, not a trace of NPU speech execution.
The feature also exposes loadngo's `PreparedCompute`, `ComputePolicy` and
`DeviceKind` contracts for later eligible partitions. Other platforms
currently report only CPU; no platform NPU implementation is claimed.

The current work consists of byte decoding, dictionary and Unicode table
lookups. These do not map usefully to dense neural execution. The original
formant/ Klatt synthesizers use small harmonic loops and recurrent filters;
their accelerator feasibility remains unmeasured. Do not introduce a
different neural voice or artificial projection benchmark as evidence
that this engine was accelerated. Before enabling an actual partition,
measure representative shapes, submission/copy latency, output parity,
end-to-end utterance latency, and thermal behavior. A `CpuAndNpu` policy
allows CPU fallback and does not prove Neural Engine execution.

## Validation evidence

All results below are local to this checkout and Mac; Linux/Windows
runtime execution awaits CI. The new `.github/workflows/rust.yml` runs
Cargo checks/tests on Linux, macOS and Windows, plus both static and
shared speech parity on Linux/macOS. No push or CI run has been performed.

### Acoustic voice configuration stage, 2026-10-05

- Retained C agrees on 282,640 acoustic directive applications and 20,606
  default resets, including six sample rates and randomized initial snapshots.
  The file corpus covers all 351 source voice/language files and 255 built
  files; comparisons cover acoustic fields rather than backend execution.
  Twelve scanner cases include empty/partial input, signs, vertical tabs,
  signed integer boundaries and ignored trailing fields. Output is
  `/private/tmp/espeak-stage11-voice-parity.log`.
- Cargo all-feature tests pass: 45 unit, five reader and five resident tests.
  Regressions check rejected tone bounds/interpolation/arithmetic, unchanged
  snapshots on failure, partial assignments, EOF counts, disabled Klatt, line
  chunks/comments and native formant-setting snapshots. The additional reader
  test loads a voice file through the platform proactor and performs acoustic
  parsing after completion, with explicit speed-change intent.
  No-default-feature tests pass.
- All 28 CTests pass in static, shared and legacy async Rust-core builds;
  all 19 retained-C tests pass. Logs are
  `/private/tmp/espeak-stage11-{core,shared,async,reference}-tests.log`.
  The new `rust_voice` oracle runs alongside existing language, audio and
  spectrum/transition comparisons.
- Strict macOS all-target/all-feature Clippy, formatting and generated tables
  pass. All-feature library Clippy passes for Linux ARM64 and Windows MSVC;
  library checks pass for iOS and Android. The MBROLA-on/Klatt-off Rust-core
  library compiles. Runtime coverage remains macOS; no target runtime CI,
  push, NPU speech execution, speedup or thermal measurement is claimed.

### Spectrum smoothing stage, 2026-10-05

- All 27 CTests pass in static, shared and legacy async Rust-core builds;
  all 19 retained-C tests pass. Logs are
  `/private/tmp/espeak-stage10-{core,shared,async,reference}-tests.log`.
- The independent retained C smoother agrees on 60,000 passes, including
  repeated passes, ordinary/Klatt commands, signed frequencies, up to 169
  commands in a wrapped 170-entry ring, variable sample lengths/rates,
  rate/low-frequency/break flags, pause/wave stops and discontinuous chains.
  Defined frame contents, command payloads, adjacent frame-handle identity and
  final syllable start match. Existing 200,000 transition comparisons,
  49,080 real spectrum selections and all 16 envelopes still pass. Output is
  `/private/tmp/espeak-stage10-spectrum-parity.log`.
- Cargo all-feature tests pass: 42 unit, four reader and five resident tests;
  CTest additionally runs the two real-data tests. Native regressions verify
  reserve-before-mutation with existing queued writable frames, unchanged
  outputs on overflow/invalid center, full-pool reuse with no new copies,
  shared links and release after consumption. The proactor-resident test
  performs native smoothing over loaded assets without C or a private runtime.
  No-default-feature tests pass.
- Strict macOS all-target/all-feature Clippy, formatting and generated tables
  pass. All-feature library Clippy passes for Linux ARM64 and Windows MSVC;
  library checks pass for iOS and Android. The MBROLA-on/Klatt-off Rust-core
  library compiles. Runtime evidence remains macOS; no push, target runtime
  CI, NPU speech execution, performance speedup or thermal result is claimed.

### Formant transition stage, 2026-10-05

- All 27 CTests pass in static, shared and legacy async Rust-core builds;
  all 19 retained-C tests pass. Logs are
  `/private/tmp/espeak-stage9-{core,shared,async,reference}-tests.log`.
- The retained C transition routines agree on 200,000 native transitions.
  Two successive passes exercise copied-frame reuse, all transition-word
  bits, signed frequency limits/truncation, zero RMS, Klatt state, glottal
  neighbors, frame extension, coloring and reference flags. Frame contents,
  counts, adjustments, return lengths, modulation and pause intent match.
  Ordinary records compare their defined 44 bytes; their absent extension is
  independently checked as zero in native copies. Klatt records compare all
  64 bytes. Existing 49,080 real spectrum selections across 818 records and
  all 16 envelopes also match; the spectrum oracle now calls the retained C
  transition, independently of the native engine. Output is
  `/private/tmp/espeak-stage9-spectrum-parity.log`.
- Cargo all-feature tests pass: 40 unit, four reader and five resident tests;
  CTest also runs the two real-data tests. Regressions cover complete record
  decoding, RMS policy, pool saturation, reserve-before-mutation, stale handles,
  arithmetic overflow, malformed counts and forged copied-frame pointers.
  The proactor-resident test performs native blending and effect handling with
  frame release/reuse; no C dependency or private scheduler is involved.
  No-default-feature tests pass.
- Formatting, generated-table provenance and strict macOS Clippy pass.
  All-feature library cross-target Clippy passes for Linux ARM64 and Windows
  MSVC; library checks pass for iOS and Android. The MBROLA-on/Klatt-off
  Rust-core library compiles. Runtime coverage remains local macOS;
  no push, CI run, NPU speech computation or thermal measurement is claimed.

### Spectrum selection stage, 2026-10-05

- All 27 CTests pass in static, shared and legacy async Rust-core builds;
  all 19 retained-C tests pass. Logs are
  `/private/tmp/espeak-stage8-{core,shared,async,reference}-tests.log`.
- Native selection and live C blending match the retained lookup on 49,080
  comparisons across all 818 compiled spectrum records. Tests vary vowel
  position, lengths, suffixes, front control, lengthening and blending
  parameters, including Klatt voice state. Frame lengths/flags, resident
  offsets, ordinary/Klatt frame contents and final adjustment agree. An exact
  EOF ordinary frame verifies zero-padded pool copying; resident copied flags
  are rejected before invoking the mutation callback. All 16
  compiled envelopes match; invalid addresses fall back safely. Parity output
  is `/private/tmp/espeak-stage8-spectrum-parity.log`.
- Cargo all-feature tests pass: 37 unit, four reader and five resident tests;
  CTest also runs the two real-data tests. Native regressions cover mixed
  frame formats, vowel splits, suffix terminal-length semantics, the reserved
  transition slot, truncation, buffer capacity, envelope bounds and arithmetic
  overflow. The proactor-resident test exercises native lookup and envelope
  access without C. No-default-feature tests pass.
- Formatting, generated-table provenance and strict macOS Clippy pass.
  All-feature library cross-target Clippy passes for Linux ARM64 and Windows
  MSVC; library checks pass for iOS and Android. The MBROLA-on/Klatt-off
  Rust-core library compiles. Runtime coverage remains local macOS;
  no push, CI run, NPU speech computation or thermal measurement is claimed.

### Phoneme condition/stress stage, 2026-10-05

- All 26 CTests pass in static, shared and legacy async Rust-core builds;
  all 19 retained-C tests pass. Logs are
  `/private/tmp/espeak-stage7-{core,shared,async,reference}-tests.log`.
- Native conditions agree with retained C on 563,000 comparisons spanning all
  selectors 0–10 and all 256 identity/property bytes. Trials vary boundaries,
  stress/reduction, dictionary flags, deleted/missing phonemes and refresh
  control; list and previous-vowel resolution side effects also match. Two
  properties that make the C oracle scan beyond its previous-vowel snapshot
  are excluded there and covered by native bounded-context regressions.
- The existing 224,616 real executions across 141 phoneme tables and 500
  synthetic programs still match C, including output and state updates.
  Detailed parity output is `/private/tmp/espeak-stage7-program-parity.log`.
- Cargo all-feature tests pass: 34 unit, four reader and five resident tests;
  CTest also runs the two real-data tests. New regressions cover clause ends,
  stress policy, isolated snapshots and vowel counts beyond 255 entries.
  Resident tests use the native context with reused owner storage and no C
  condition callbacks. No-default-feature tests pass.
- Formatting, generated tables and strict macOS Clippy pass. All-feature
  library cross-target Clippy passes for Linux ARM64 and Windows MSVC; library
  checks pass for iOS and Android. A Rust-core C library with MBROLA enabled
  and Klatt disabled compiles; this is compile coverage, not MBROLA runtime
  validation. Runtime execution remains local macOS; no push or CI run was
  performed. No NPU speech computation or thermal result is claimed.

### Phoneme-program VM stage, 2026-10-05

- All 26 CTests pass in static, shared and legacy async Rust-core builds;
  all 19 retained-C tests pass. Logs are
  `/private/tmp/espeak-stage6-{core,shared,async,reference}-tests.log`.
- The unchanged C VM agrees on 224,616 real phoneme executions across the
  141 compiled tables, including full output bytes, neighbouring resolution
  side effects, phoneme lengths/sound fields and previous-vowel state updates.
  Trials vary language presence, reduction policy, prevoicing/change passes,
  stress, word boundaries, dictionary flags and missing neighbouring records.
  The oracle receives one explicit Return word after EOF to make its terminal
  lookahead defined; the native VM borrows the exact unpadded resident file.
- Another 500 synthetic programs compare calls, signed length changes, IPA,
  envelope addresses, transitions, vowel switches, conditional jumps, sound
  signedness, continuation and implicit FMT/WAV/addWav returns.
- Cargo all-feature tests pass: 30 unit, four reader and five resident tests;
  CTest runs the two real-data tests. The resident test now executes a native
  phoneme program from host-proactor-loaded data. Native regression tests cover
  EOF sounds, missing vowel records, stack saturation, malformed instructions
  and bounded cycles. No-default-feature tests pass.
- Formatting, generated tables and strict macOS Clippy pass. All-feature
  library cross-target Clippy passes for Linux ARM64 and Windows MSVC; library
  checks pass for iOS and Android. Runtime execution remains local macOS;
  no push or CI run was performed. These table/bytecode operations execute on
  CPU; no NPU speech acceleration or thermal result is claimed.

### Scalar language letter stage, 2026-10-05

- All 25 CTests pass in static, shared and legacy async Rust-core builds;
  the retained C configuration passes all 19 tests. Logs are
  `/private/tmp/espeak-stage5-{core,shared,async,reference}-tests.log`.
- The retained `IsLetter` oracle agrees on 886,890 exact mask comparisons
  across all 123 compiled dictionary language configurations: accent ranges,
  each positive alphabet offset and its boundaries, actual wide-list entries,
  NUL and randomized Unicode characters. The rule oracle still agrees on
  46,812 real and 59,400 synthetic executions, including trace output.
- Cargo all-feature tests pass: 27 unit, four reader and five resident tests;
  the two real-data tests are run by CTest. No-default-feature tests pass.
  The resident test exercises native language-vowel rule selection and its
  fallback using assets loaded through the host's proactor.
- Formatting, generated-table provenance and strict all-target/all-feature
  macOS Clippy pass. All-feature library cross-target Clippy passes for
  `aarch64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`; library checks pass
  for iOS and Android. These are compilation checks, not platform execution.
- Hardware discovery reports `[Npu, Cpu]` on this Mac; no NPU speech partition,
  performance improvement or thermal claim is established by these results.
  No Linux runtime session, push or CI run was performed.

### Rule matcher stage, 2026-10-05

- All 25 CTests pass in static, shared and legacy async Rust-core builds;
  the retained C configuration passes its 19 tests. Logs are
  `/private/tmp/espeak-stage4-{tests,shared-tests,async-tests,reference-tests}.log`.
  The final oracle fixture also passes in shared/async builds after restricting
  its inputs to character boundaries.
- `rust_rulematch` compiles the unchanged C matcher, letter predicates and
  prefix helper as separate oracles. Rust matches 46,812 real executions across
  11 languages, including indexed Unicode-offset groups, and 59,400 synthetic
  executions covering every contextual opcode in PRE, POST and consume modes.
  Comparisons cover scores, input advancement, phonemes, ending flags, deletion
  positions and byte-for-byte trace output for selected trials. The final
  standalone oracle passes twice consecutively, as well as through CTest.
- Tests preserve common phonemes, later-rule ties, explicit word-start bonuses,
  condition limits, prefix alternatives, syllables/stress, Devanagari digits,
  hyphens, skip scans, suffix restrictions and signed/unsigned ending bytes.
  Explicit borrowed context fixes reproduced Greek cross-word pronunciation
  and SSML time/colon regressions. Existing waveforms and replacement traces pass.
- The original PRE no-vowels loop can scan before storage when called from an
  artificial continuation-byte cursor. C differential inputs use character
  boundaries. A native regression exercises that cursor safely: NUL boundaries
  reject the candidate, and malformed instructions/windows return errors.
- Cargo all-feature tests pass: 25 unit tests, four reader tests and five
  resident-loader tests. Two real-data cases run through CTest. Proactor-loaded
  native dictionaries exercise both contextual lookup and cached rule execution.
  No-default-feature tests, formatting and strict all-target/all-feature Clippy
  pass. Linux AArch64 and Windows x86-64 MSVC Clippy, plus iOS/Android AArch64
  library checks, pass; target runtime execution still awaits CI.
- A small command-level comparison used `-xq -v <voice> -f <input>` for repeated
  English (10,900 bytes), Greek (6,600 bytes) and Russian (10,000 bytes) text.
  Output bytes match C in all three cases. Three-run median Rust/C elapsed
  ratios were 1.000, 1.005 and 1.034; raw samples and inputs are under
  `/private/tmp/espeak-stage4-benchmark*`. Builds overlapped part of this sample,
  and these timings include the rest of the hybrid engine. They establish
  neither an isolated VM speedup nor thermal safety.
- The hardware example again reports `[Npu, Cpu]` through loadngo. NPU
  integration remains capability discovery; no eligible speech compute
  partition was added. This stage executes bounded bytecode matching on CPU.

### Contextual lookup and compression stage, 2026-10-04 (`c778e239`)

- All 24 CTest tests pass in static, shared and legacy async Rust-core builds.
  The retained C configuration passes its 19 tests. Logs are
  `/private/tmp/espeak-stage3-{tests,shared-tests,async-tests,reference-tests}.log`.
  Existing pronunciation, replacement traces, SSML and waveform checks pass.
- `rust_lookup` builds the original C `LookupDict2` and `TransposeAlphabet`
  as separate oracles. It compares 50,000 alphabet transpositions including
  maps, frequent pairs, mixed scripts and the entire unchanged buffer tail;
  20,000 synthetic lookups covering flag byte values 0 through 163 and randomized
  grammar; and 20,800 real dictionary lookups across eight languages including
  compressed keys, capitalization and symbols. Return pointers, both flag
  words, phoneme copies and skip counts match. Selected trials also compare
  trace output byte for byte. An additional case covers a next-word pointer
  immediately after the terminal NUL.
- Matching uses bounded slices and stack state. The C adapter examines at most
  255 next-word bytes, the maximum compiled-record length, and at most 20 word
  metadata entries, stopping at the word-table sentinel. It never reads a
  next-word C string from a one-past pointer. Rust rejects invalid condition
  shifts and truncated multiword windows rather than reading outside storage.
- Cargo all-feature tests pass: 20 unit tests, four reader tests and five
  resident-loader tests, with two data-dependent cases run by CTest. Resident
  assets loaded through the actual host proactor now exercise contextual
  lookup through the native owned dictionary API. No-default-feature tests
  also pass.
- Formatting and strict all-target/all-feature Clippy pass on macOS, Linux
  AArch64 and Windows x86-64 MSVC targets; all-feature library checks pass for
  iOS AArch64 and Android AArch64. Target compile checks do not establish
  Linux/Windows runtime behavior; CI execution still awaits publication.
- The hardware example again reports `[Npu, Cpu]` through loadngo. No NPU speech
  partition is enabled: contextual byte matching belongs on the CPU.
  Existing host-owned proactor loading and Core ML capability integration
  remain available. Translation and waveform synthesis are still hybrid C/Rust.

### Resident-data and index stage (`daf121eb`)

- All 23 CTest tests pass in static, shared and legacy async Rust-core builds.
  The retained C configuration passes its 19 tests. Logs for this local run
  are `/private/tmp/espeak-stage2-{tests,shared-tests,async-tests,reference-tests}.log`.
- `rust_data` compiles the original C `InitGroups` and `SetUpPhonemeTable`
  routines as separate oracles. Rust matches all indices for 123 dictionaries
  and all 141 phoneme tables, with forward/reverse table switching and
  duplicate table names preserving first-match lookup. Missing/malformed
  dictionary replacement retains the previous valid data and indices;
  invalid table selection clears selected pointers safely.
- `rust-resident-assets` loads all 30,324,062 bytes through the real platform
  proactor: four core assets plus 123 dictionaries. Every resident byte matches
  the CMake-generated files; native indexing validates all 141 tables/chains.
- Cargo all-feature tests pass: 15 unit tests, four reader tests, five resident
  loader tests. Two data-dependent tests are ignored by Cargo alone and run
  by CTest. Resident tests cover assembly/native lookup, admission, cancellation
  and reuse, shortened files, malformed data, byte/name limits and starting the
  next plan from a completion on the same host.
- Test deadlines release their host references after stop so pending watchdogs
  do not retain the proactor/file workers. Parallel fixtures use an atomic
  sequence as well as a timestamp, avoiding a reproduced directory collision.
- Strict all-target/all-feature Clippy and formatting pass on macOS. Linux
  AArch64 and Windows x86-64 all-target Clippy also pass; all-feature library
  checks pass for those targets, iOS AArch64 and Android AArch64. These are
  compile checks; Linux/Windows runtime validation still awaits CI.
- `resident_data --data-dir build-rust-core/espeak-ng-data --dictionary en
  --dictionary si` loaded 1,013,456 bytes, 141 tables and two dictionaries at
  22,050 Hz. Core ML capability still reports `[Npu, Cpu]`; no eligible NPU
  speech partition or acceleration is claimed for this stage.

### Text/Unicode foundation stage (`0ac7b3bb`)

- The unchanged C baseline configured with async/audio-device/sonic/MBROLA
  disabled: all 19 CTest tests passed.
- Rust core linked into both static and shared engine builds with the same
  configuration: all 21 CTest tests passed. The original 19 tests include
  pronunciation, numbers, SSML, voices, crash vectors and waveform hashes.
  A separate Rust-core build with the legacy asynchronous runtime enabled
  also passed all 21 tests.
- `rust-parity` compared every one of 1,114,112 Unicode codepoints: category,
  group, script, three case conversions, twelve classifiers and all 31
  property-category inputs. Also 1,001 invalid/random codepoints.
- Decoder differential coverage: 20 encodings, 1,000 random buffers each,
  supported AUTO codepage fallbacks, cursor offsets, peek state and EOF.
- IEEE80 differential coverage: 196,608 values, every exponent and sign;
  bit-exact against C except unspecified NaN payloads.
- Phoneme-feature differential coverage: all 17,576 lowercase triples applied
  to 175,760 randomized phoneme records, checking statuses and every record byte.
- The Rust compiled-dictionary parser validated every dictionary in the
  full CMake data build: 123 dictionaries (`rust-dictionaries`).
- Rust unit tests cover legacy UTF-8 boundaries, aliases, sentinel behavior,
  Unicode table invariants, dictionary records and malformed/truncated data.
- Real loadngo proactor tests cover byte content, short reads, EOF, buffer
  address reuse, busy rejection, file retention, cancellation and recovery
  after I/O failure. Cargo all-feature tests: 10 unit and 4 proactor tests pass;
  the real dictionary test is explicitly ignored by Cargo alone and run by CTest.
- All-feature cross checks compiled the Rust library for Linux AArch64,
  Windows x86-64, iOS AArch64 and Android AArch64. Cross Clippy also passed
  for Linux and Windows. These checks do not establish target runtime behavior.

Two deliberate safety improvements diverge from undefined C behavior:
reading after EOF returns zero, and invalid encoding discriminants are
rejected before indexing. AUTO with a non-codepage fallback replaces invalid
bytes as ASCII instead of dereferencing a null codepage. Valid/defined
legacy behavior is preserved, including case-sensitive aliases, the UTF-8
trailing-byte rule, U+FFFD becoming U+001A in a decoded three-byte sequence,
and permanent AUTO fallback even when first selected by `peek`.

### Mutable language-option stage

Native language options apply to an instance-owned snapshot, with borrowed
tune names supplied by its owner. Compatibility adapters commit a successful
snapshot to the legacy translator and tone flags. Number ordinals, partial
numeric assignments, stress-array conversions, tune `NULL` entries and unknown
tune diagnostics retain defined C behavior. Negative or punctuation-only
ordinal tokens that stall the legacy cursor, overflowing integers and
overlong tune names fail without modifying the snapshot.

- The independent retained-C oracle passes 310,576 language-option comparisons,
  covering all 18 indexed parameters and 606 built/source configuration files.
  The existing acoustic oracle also passes 282,640 applications and 20,606 resets.
- Static, shared and legacy-async Rust-core builds pass all 28 CTest tests;
  the retained C build passes its 19 tests, including waveform hashes.
- Cargo all-feature tests pass 47 unit, five reader and five resident tests.
  The resident fixture applies language options after loading through the host
  proactor, outside its completion callback.
- Formatting, generated-table provenance, minimal-feature tests, strict
  all-feature Clippy on macOS, Linux AArch64 and Windows x86-64, and iOS/Android
  library checks pass. Cross compilation does not establish target runtime parity.

### Static language-preset stage

Language selection now borrows committed native stress, letter, number,
punctuation, alphabet and vowel-length tables. Each `Language` owns its mutable
options and a bounded dictionary identifier. Selecting a preset, constructing
letter/compression views and applying directives perform no allocation or I/O.
Regeneration uses the retained C configuration as a development-time data
oracle; Cargo uses immutable native constants. A provenance check covers all
switch selectors and the default branch.

- The independent C setup matches 18,385 native configurations: all lowercase
  names of up to three letters, every longer switch name, rolling four-byte
  aliases and a 39-byte identifier. All initialized scalar fields and every
  referenced immutable table are compared by value.
- Alphabet classification matches C for 1,114,469 signed/Unicode inputs.
- Xextan's one-unit punctuation table now has a terminator; the oracle compares
  its declared unit instead of reading beyond the legacy array. Overlong names
  are rejected before committing output. C compatibility allocation is zeroed.
- 59 Rust tests and 29 static/shared/legacy-async CTests pass; the C-only baseline
  passes 19. Existing pronunciation, SSML, number, voice and waveform hashes pass.
  Native language presets also configure proactor-loaded voice directives outside
  completion callbacks.
- Strict all-feature Clippy, minimal-feature tests, formatting and both table
  provenance checks pass. Linux AArch64/Windows x86-64 strict library Clippy and
  iOS/Android library checks pass; MBROLA-on/Klatt-off compiles. These do not
  establish target runtime parity, NPU speech execution or thermal behavior.

### Voice metadata and matching stage, 2026-10-06

Voice metadata now parses into bounded native state, preserving the legacy
119-byte input chunks, comment/whitespace handling, partial gender/age
assignments, language admission limits and priority-byte layout. The native
matcher retains dialect scoring, age/gender/name preferences, filename/name
precedence and numeric/named variants. Compatibility adapters keep file I/O,
catalogue ownership and returned-string lifetime in the serialized C layer.

- 10,606 metadata records match C, including all 606 built/source voice files;
  200,000 generated voice scores, 50,005 name selections and 50,000 variants
  also match. Existing acoustic, language-option and language-preset oracles pass.
- Bare `+` variant suffixes retain the directory prefix when requested. The
  native filename matcher checks short identifiers without reading before their
  allocation; the retained oracle uses deliberately padded identifiers for those
  comparisons. Overlong metadata tokens/variants and integer overflow are rejected.
- Metadata parsing, native language setup and voice directives are verified on
  bytes loaded through the caller-owned host proactor, outside completions.
- 62 Rust tests and all 29 static/shared/async CTests pass. The C-only baseline
  passes 19 tests. Strict Clippy, minimal-feature tests, formatting, both table
  provenance checks and Linux/Windows/iOS/Android library cross gates pass.
  MBROLA-on/Klatt-off compiles; its backend is not executed by these tests.

### Voice candidate selection stage, 2026-10-06

Native property selection now ranks an immutable roster and cycles the selected
gender/age variants in a caller-owned workspace. It reserves space for at most
499 voices and 12 variants during initialization; repeated selection clears and
reuses those allocations. Sorting candidates allocates nothing and preserves
input order for equal scores/names. The C adapter creates one serialized
workspace for its catalogue and destroys it with that catalogue.

Directory discovery remains an explicit owner input/callback, invoked only for
the normalized one-part MBROLA selector. Catalogue file reads and catalogue
ordering (including stable-sort scratch) are initialization/offload work.
Selection plans score effects before the compatibility owner commits them; an
`all` query retains cached scores as in C. Oversized language selectors and
combined compatibility identifiers fail instead of overflowing C buffers.

- 20,000 full catalogue selections and 20,000 candidate rankings match the
  independent retained C algorithms, including ordering, score updates, default
  fallback, named variants, gender/age choices and variant cycling.
- Existing metadata, score, name, variant, language/acoustic and speech oracles
  pass. The host-proactor fixture performs 50 selections on loaded metadata;
  unit tests verify workspace buffer addresses remain unchanged across requests.
- 64 Rust tests and all 29 CTests pass in static/shared/legacy-async builds;
  the C-only baseline passes 19. Strict Clippy, minimal-feature tests, formatting,
  both provenance checks and Linux/Windows/iOS/Android library cross gates pass.
  MBROLA-on/Klatt-off compiles. Runtime/thermal/NPU speech claims remain unmeasured.

### Ordered active-voice setup stage, 2026-10-06

The active voice has separate native setup state from catalogue metadata.
Only the first language directive chooses the translator; later languages
extend its priority list, while dictionary and phoneme directives override
their names in file order. Tone-only variants retain the current metadata.
The native parser reports language selection as an owner effect and rejects
malformed or oversized input before changing state.

- 181,785 ordered setup snapshots match the retained C cases, including all
  606 built/source files, partial age assignments, repeated directives and
  tone-only variants. Prior catalogue, language and acoustic comparisons pass.
- 66 Rust tests and all 29 static/shared/legacy-async CTests pass; the C-only
  baseline passes 19, including pronunciation and waveform hashes. The host
  proactor fixture applies native setup to loaded bytes outside completions.
- Strict Clippy, minimal-feature tests, formatting and both provenance checks
  pass. Linux/Windows strict library Clippy, iOS/Android library checks and
  MBROLA-on/Klatt-off compilation pass. Logs use the
  `/private/tmp/espeak-stage16-*` prefix. Target runtime parity, thermal behavior
  and NPU speech execution remain unmeasured.

### Phoneme names and backend directives stage, 2026-10-06

Native replacement parsing preserves partial integer conversion, first-match
mnemonic lookup, optional `NULL` replacement, flag-byte conversion and the
60-entry admission limit. Active setup reports the first replacement's table
selection in file order. MBROLA directives produce bounded startup requests;
the compatibility owner still performs startup and backend file operations.
Oversized tokens, invalid counts and integer overflow fail before changing
replacement or request outputs.

- Independent C comparisons cover randomized sparse/duplicate phoneme tables,
  every initialized real table, 260,000 generated replacement snapshots and
  20,002 generated MBROLA requests, plus directives in all 606 voice files.
  The ordered setup oracle also covers repeated replacement/table directives.
- 68 Rust tests, all 29 static/shared/legacy-async CTests and the C-only
  baseline's 19 tests pass. Pronunciation and waveform hashes remain unchanged.
  Proactor-loaded bytes configure replacements and backend requests outside
  completions; no backend startup occurs in a completion callback.
- Strict Clippy, minimal-feature tests, formatting, provenance checks and
  Linux/Windows/iOS/Android library cross gates pass. MBROLA-on/Klatt-off compiles;
  MBROLA backend runtime, thermal behavior and NPU speech execution remain
  unmeasured. Logs use `/private/tmp/espeak-stage17-*`.

### Catalogue discovery and ownership stage, 2026-10-06

Rust owns catalogue metadata and identifiers, including every compatibility
voice/string pointer until `FreeVoiceList`. Discovery retains voices-before-lang
ordering, hidden/empty/unreadable-file handling and the original 498-record
compatibility limit. Native callers can admit up to 499 records, reuse their
selection workspace and retain score state independently.

`MetadataChunks` accepts borrowed host-proactor buffers without allocation;
it retains only the current 119-byte fgets chunk and a metadata snapshot. The
serialized compatibility loader uses one 8-KiB scratch buffer for all files.
Windows keeps the active-code-page path representation, CRLF conversion and
text EOF behavior. Discovery bounds initialization to 8,192 entries, depth 64
and 16 MiB read; oversized identifiers and malformed metadata are rejected.
Directory/file work belongs to initialization or the caller's worker path.
The compatibility API supplies no host handle and uses synchronous native I/O;
the native API accepts bytes loaded through the caller-owned proactor.

- Independent C discovery/admission comparisons match 1,104 records: all
  606 shipped built/source records and 498 admitted from a 510-file fixture,
  including a configuration larger than 64 KiB. Record fields and ordering match.
  Prior setup, acoustic, language, mnemonic and selection comparisons pass.
- 72 Rust tests and all 29 static/shared/legacy-async CTests pass; the C-only
  baseline passes 19. Chunk widths 1 through 129 preserve metadata, NUL handling
  and storage addresses. The proactor fixture selects from owned loaded metadata.
- Strict Clippy, minimal-feature tests, formatting, provenance checks and
  Linux/Windows/iOS/Android library cross gates pass. Minimal Windows strict
  Clippy and Windows test compilation pass; Windows runtime is not exercised.
  MBROLA-on/Klatt-off compiles. Logs use `/private/tmp/espeak-stage18-*`.

### Catalogue result storage and listing stage, 2026-10-06

The catalogue owner now retains its sorted pointer roster, public result buffer
and reusable selection workspace. Compatibility callers borrow these until
catalogue destruction; repeated native list queries reuse the same initialized
result buffer, preserve cached score behavior and allocate nothing. `FreeVoiceList`
releases the complete owner rather than freeing records or workspaces separately.
Visibility filtering and property-list orchestration execute in Rust; directory
discovery remains an explicit synchronous owner callback.

- 753 full native list queries match C on shipped data and the capacity fixture,
  including default visibility, MBROLA/directory filters, age/gender, persistent
  scores and buffer-address reuse. All 1,104 owned record comparisons still pass.
- The full source catalogue exposes equal-name/equal-score Pashto records with
  different settings. C `qsort` does not specify their relative order; Rust
  preserves input order. Comparisons require identical membership within tied
  groups and exact preference order between distinguishable candidates.
- 73 Rust tests, all 29 static/shared/legacy-async CTests and the C-only
  baseline's 19 tests pass. Strict Clippy, minimal features, provenance/fmt and
  Linux/Windows/iOS/Android cross gates pass, including minimal Windows Clippy
  and Windows test compilation. MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage19-*`; target runtimes and NPU speech remain unmeasured.

### Active voice request planning stage, 2026-10-06

Native request planning retains the voices-before-lang path probe, 39-byte name
truncation, platform path-buffer truncation, explicit-file checks, compilation
mode and no-default/tone-only fallback controls. Identifier replacement snapshots
aliased requests and the previous variant before committing bounded output.
Truncated variant prefixes and overflowing explicit paths fail safely.
Request probes and opening belong to initialization or the caller's worker path;
the compatibility owner still opens the active file and dispatches its directives.

- Independent C cases compare 20,000 request paths/probe sequences, generated
  open/table/fallback combinations and 60,000 current identifiers, including
  aliased requests. Prior catalogue, setup and speech comparisons pass.
- 75 Rust tests and all 29 static/shared/legacy-async CTests pass; the C-only
  baseline passes 19. The host-proactor fixture plans the explicit request and
  opens its file before submitting positioned reads, then configures native state.
- Strict Clippy, minimal-feature tests, formatting/provenance and
  Linux/Windows/iOS/Android cross gates pass. Minimal Windows Clippy and Windows
  test compilation pass; MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage20-*`. Runtime/NPU/thermal boundaries are unchanged.

### Ordered active voice directive stage, 2026-10-06

The active file loop now dispatches through one native ordered parser. Language
options, acoustics, metadata, phoneme replacements and backend requests retain
their legacy precedence. Rust reports explicit owner actions for translator/table
selection, speed updates and MBROLA startup; parsing performs no I/O or callbacks.
Rejected values preserve snapshots, and unavailable backends skip their parsers.
File opening/reading and final load orchestration still belong to the C owner.

- 380,000 independent C comparisons match ordered actions and complete snapshot
  effects across all four backend feature combinations and ordinary/tone-only
  loads. Prior request, metadata, catalogue and speech comparisons pass.
- 77 Rust tests and all 29 static/shared/legacy-async CTests pass; the C-only
  baseline passes 19. The proactor fixture configures loaded native snapshots
  and handles actions outside completion delivery.
- Strict Clippy, minimal features, formatting/provenance and Linux/Windows/iOS/
  Android cross gates pass, including minimal Windows Clippy and Windows test
  compilation. MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage21-*`; runtime/NPU/thermal boundaries are unchanged.

### Active voice stream stage, 2026-10-06

Active voice file opening, buffered reading and directive splitting now execute
in Rust. One owner reuses an 8 KiB read buffer and a bounded 4 KiB line buffer;
borrowed terminated directives remain live until the next read or destruction.
The reader preserves legacy fgets widths, embedded NULs, trailing whitespace,
comments and final unterminated lines, including Windows CRLF/CTRL-Z conversion
and ANSI path decoding. No directive allocates. Native callers can instead read
from resident bytes loaded by the caller-owned host proactor; the compatibility
API has no host handle and performs synchronous setup/worker I/O.

- Independent C stream comparisons cover all 606 shipped built/source files
  plus a 100,011-byte binary fixture at six line widths, including 2 and 4,096.
  The same storage is reused, EOF preserves outputs, and invalid widths/paths
  fail before publishing an owner. Prior snapshot and speech comparisons pass.
- 79 Rust tests, all 29 static/shared/legacy-async CTests and the C-only
  baseline's 19 tests pass. The proactor fixture feeds resident bytes through
  the native stream reader after completion delivery.
- Strict Clippy, minimal features, formatting/provenance and Linux/Windows/iOS/
  Android cross gates pass, including minimal Windows Clippy and Windows test
  compilation. MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage22-*`; target runtime/NPU/thermal claims remain absent.

### Active voice load driver stage, 2026-10-06

The native driver now owns the directive loop and final translator/table/
dictionary ordering. It applies snapshot parsers, reports rejected attributes,
executes explicit backend actions, preserves backend failure prefixes and skips
final language work for tone-only/compilation controls. Accepted partial files
retain legacy behavior on read errors; the safe API reports that condition.
The compatibility adapter copies snapshots around short backend operations,
so no Rust exclusive borrow aliases a mutable C global during callbacks.
Initial voice selection/reset/current metadata storage and actual translator,
dictionary and synthesis backend resources remain in the C owner.

- 4,096 load sequences match C scanner/action references, ordered callbacks and
  snapshots across backend feature/control combinations, table fallback and
  backend/dictionary failures. Native tests also cover rejected setup with no
  effects and read errors after an accepted prefix. Prior comparisons pass.
- 81 Rust tests and all 29 static/shared/legacy-async CTests pass; the C-only
  baseline passes 19. The proactor fixture executes the complete native driver
  on resident bytes after completion delivery, including final owner operations.
- Strict Clippy, minimal features, formatting/provenance and Linux/Windows/iOS/
  Android cross gates pass, including minimal Windows Clippy and Windows test
  compilation. MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage23-*`; target runtime/NPU/thermal boundaries remain.

### Current voice owner stage, 2026-10-06

Current identifier/name/language storage now belongs to a native per-instance
owner. Initial setup and variant preparation snapshot aliased requests before
mutation, clear ordinary-load visible metadata and preserve variant metadata.
Compatibility API strings remain stable across catalogue rebuilding and native
calls; engine termination releases the owner after legacy async workers drain.
The independent native API commits metadata explicitly after load configuration.
Backend reset and actual resource operations still use compatibility adapters.

- 20,000 current voice preparations match retained C identifier behavior and
  initial setup, including aliased requests and metadata-address reuse. Two
  owners stay independent, malformed inputs preserve outputs, and an API check
  verifies catalogue rebuilding retains the current strings.
- 82 Rust tests and all 29 static/shared/legacy-async CTests pass; the C-only
  baseline passes 19. The proactor fixture prepares and commits the native
  current owner around the native load driver.
- Strict Clippy, minimal features, formatting/provenance and Linux/Windows/iOS/
  Android cross gates pass, including minimal Windows Clippy and Windows test
  compilation. MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage24-*`; target runtime/NPU/thermal boundaries remain.

### Core phoneme asset storage stage, 2026-10-06

Compatibility phoneme file reads now use a native core owner instead of C
malloc/fread/free. Four buffers retain u64-aligned bytes for remaining C table,
instruction, spectrum and tune consumers. Warm reloads reuse capacity; the
combined reserved capacity, including retained peaks, is bounded to 128 MiB.
Missing/open/admission errors retain earlier bytes; started failed reads clear
their slot without releasing capacity. Native callers can populate the same
storage from caller-proactor resident bytes during initialization/worker work.
The compatibility API has no host handle and uses synchronous native setup I/O.
Index/view lifetimes drain before replacement or destruction; termination clears
the remaining C pointers. At this stage dictionary ownership still used C
allocation; the following stage replaces it.

- All 715,600 core bytes match C across 40 file loads, with alignment and stable
  addresses verified. Empty/missing/directory/oversized files exercise ownership
  and admission behavior. Native tests verify retained peak bounds, short-read
  invalidation and worker-side alignment of proactor-resident bytes.
- 84 Rust tests and all 29 static/shared/legacy-async CTests pass; the C-only
  baseline passes 19. Existing 123-dictionary/141-table and speech checks pass.
- Strict Clippy, minimal features, formatting/provenance and Linux/Windows/iOS/
  Android cross gates pass, including minimal Windows Clippy and Windows test
  compilation. MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage25-*`; target runtime/NPU/thermal boundaries remain.

### Dictionary snapshot ownership stage, 2026-10-06

Compatibility dictionary loading now uses a Rust cache and per-translator
snapshot handles. Immutable, u64-aligned bytes and parsed bucket/rule indices
are shared across translators and unchanged reloads. Every load rereads the file
into reusable scratch before comparing bytes, so same-size changes with unchanged
timestamps are observed. At most 128 cached and 128 pinned retired versions are
admitted; reserved byte storage, including scratch and live retired buffers, is
bounded to 128 MiB. Eviction overwrites unpinned storage; pinned admission returns
backpressure. Small fixed index/path/handle overhead is separate from this byte
limit. Failed loads preserve translator bindings. Cache release leaves pinned
views live; termination releases alternate translators and then the cache.

The synchronous compatibility API performs setup I/O on its serialized owner.
The native cache also accepts caller-proactor resident bytes without filesystem
operations, with conversion/parsing performed after completion on the owner or
a worker. This is not a complete owned engine or an NPU execution path.

- All 123 dictionary indices match the independent C oracle, including three
  unchanged reloads and a second translator sharing each dictionary. Alignment,
  failed replacement and cache release/recreation with pinned views pass. The
  added opaque handle has a consistent internal Translator layout in library,
  tool and test consumers; a conditional-field mismatch found by the oracle was
  corrected before validation.
- Native tests cover LRU storage reuse, entry/byte bounds, pinned-version
  backpressure, fresh reads with unchanged timestamps, and 50 resident cache
  reuses after host-proactor loading. All 87 Rust tests and 29 static/shared/
  legacy-async CTests pass; C-only passes 19. Existing core/table and speech
  regressions remain green.
- Strict Clippy, minimal features, formatting/provenance and Linux/Windows/iOS/
  Android cross gates pass, including minimal Windows Clippy and Windows test
  compilation. MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage26-*`; target runtime/NPU/thermal boundaries remain.

### Speech-rate computation stage, 2026-10-06

SetSpeed now delegates calibration and translation/synthesis speed snapshots to
native Rust. Lookup tables, voice percentage/factors, high-rate pause/wave/length
adjustments and optional Sonic acceleration settings replace the C calculations.
Native State owns the factors and three syllable lengths, computes without
allocation/I/O/globals/callbacks, and returns a fixed two-element ordered Sonic
effect prefix. Compatibility code commits the snapshot then submits those effects
to the existing synthesis queue. Unsupported controls or arithmetic overflow
preserve all outputs; retained C overflow cases are undefined and excluded from
the oracle. Sonic remains an optional external audio backend, not NPU execution.

- 104,256 snapshots and Sonic effect sequences match independently compiled
  retained C with Sonic both enabled and disabled. Tests cover all rates -128 to
  1500, primary/secondary selection, controls 0..3, percentage settings and signed
  custom syllable factors. Rejected controls/flags/arithmetic retain snapshots.
- All 90 Rust tests and 30 static/shared/legacy-async CTests pass; C-only passes
  19. The proactor voice fixture computes the native rate after asset completion.
- Strict Clippy, minimal features, formatting/provenance and Linux/Windows/iOS/
  Android cross gates pass, including minimal Windows Clippy and Windows test
  compilation. MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage27-*`. No libsonic installation was found on this
  host: both branches are oracle-tested, but actual Sonic runtime audio remains
  a platform gate. Target runtime/NPU/thermal boundaries remain unchanged.

### Configuration and sound-icon ownership stage, 2026-10-06

Native Rust now parses global configuration and owns up to 80 sound-icon names
and reusable aligned WAV buffers. The shared streaming reader exposes raw
fgets-width chunks, preserving first-column prefixes, text-mode conversion,
tone settings, character signedness and ordered definitions. Filename/table
bounds replace C's overflowing writes. Dynamic filename lookup selects the
first matching entry; punctuation lookup retains its selected entry even when
other characters name the same file. Compatibility filename inputs are copied
before exclusive owner access, supporting aliases of published names.

Warm nonempty audio retains PCM addresses and skips I/O/allocation. Empty PCM
can reread while retaining capacity. Combined reserved WAV capacity is bounded
to 128 MiB; fixed entry/name overhead is separate. File work runs on the
serialized initialization/synthesis owner; already-resident host-proactor bytes
can be installed after completion without filesystem work. Termination drains
the existing workers/audio and releases owned icons after waveform teardown.
Published C views are immutable borrows, not malloc-owned bytes.

WAV compatibility retains the C fixed 44-byte header and mono/PCM/rate/byte-rate
checks, with an additional declared-data-length bound. The original conversion
command is disabled; other formats formerly opened an empty temporary file and
could read beyond it. Native unsupported/short/oversized data returns failure
without publishing playable PCM. This stage does not add format conversion or
claim NPU/audio speedup.

- 4,000 configuration entries and 100 WAV loads (12,750 PCM bytes) match retained
  C. Tests cover duplicate filenames, stable warm and empty PCM storage,
  absolute/relative names, warm lookup after deletion, compatibility name
  aliases, cache teardown/recreation and table admission. Native bounds/header
  tests and a real host-proactor load verify owner-side PCM installation.
- All 94 Rust tests and 31 static/shared/legacy-async CTests pass; C-only passes
  19. Earlier voice stream, rate, dictionary, table and speech oracles pass.
- Strict Clippy, minimal features, formatting/provenance and Linux/Windows/iOS/
  Android cross gates pass, including minimal Windows Clippy and Windows test
  compilation. MBROLA-on/Klatt-off compiles. Logs use
  `/private/tmp/espeak-stage28-*`; target runtime/NPU/thermal boundaries remain.

### Suffix and character encoding stage, 2026-10-06

Native Rust removes UTF-8 suffixes, restores spelling, applies English/Dutch
repairs and returns grammatical, preceding-byte and trace effects for the
serialized owner to commit. Immutable language letters are borrowed directly;
no callback, I/O or allocation occurs. Small fixed edit plans preserve the
original-word copy's 160-byte compatibility bound while accepting long words.
Invalid suffix bounds or repair capacity leave the input and effects unchanged.
UTF-8 output preserves the C surrogate and out-of-range behavior and writes only
the encoded bytes, retaining the caller's trailing storage.

An initial whole-word bound rejected a long suffix chain without making
progress, causing repeated translation in the existing crash regression. The
bound now applies only to the original copy. A 440-byte native suffix-chain
test and 800-byte retained-C cases verify progress; all final crash tests pass.
The three owned processes from the failed run were stopped before rerunning.

- 68,283 suffix repairs match the independently extracted C implementation,
  including all suffix counts/flags, nine languages, context/history repairs,
  multibyte words, clipped copies, untouched tails and trace output. All
  1,114,129 character encodings from 0 through 0x110010 match C byte for byte.
- All 98 Rust tests and 32 static/shared/legacy-async CTests pass; C-only passes
  19. A caller-owned proactor fixture applies native suffix repair after asset
  completion, on the owner rather than the completion thread.
- Strict Clippy, minimal features, formatting/provenance and Linux/Windows/iOS/
  Android cross gates pass, including minimal Windows Clippy and Windows test
  compilation. MBROLA-on/Klatt-off compiles. Final logs use
  `/private/tmp/espeak-stage29-fixed-*` and `/private/tmp/espeak-stage29-cargo.log`;
  target runtime/NPU/thermal boundaries remain unchanged.

### Word stress stage, 2026-10-06

Native Rust strips stress markers and assigns complete word-stress patterns
using borrowed selected phoneme tables and explicit language options. It retains
priority/previous stress, forced unstressed vowels, syllabic consonants, heavy
and long syllables, all 13 supported stress-position rules, secondary/diminished
stress flags, dictionary positions, tonic overrides, initial vowel pauses and
lengthen removal. Sparse/out-of-range input codes become schwa for assignment,
as in C. The previous-stress effect is captured before local diminished-stress
changes, retaining the original mutation order.

Planning uses fixed stack arrays and publishes completed prefixes only. The
existing 98-syllable extraction bound and 197-byte output-loop admission are
retained. Unused caller tails need not be initialized or borrowed. Unterminated
inputs, missing required records and unencodable stresses fail without partial
word/effect publication. Native options construct settings directly; a host
proactor fixture performs assignment after voice bytes arrive on the owner.
This is bounded scalar worker/owner work, with no I/O, callbacks or NPU compute.

- 322,388 extractions and 322,388 complete assignments match independently
  extracted C, including compiled phoneme tables, 17 language configurations,
  rule/flag combinations, sparse gaps, previous state, lengthening, syllabic
  consonants, caller tails and long-word truncation. ABI rejection tests verify
  transactional output/effect behavior.
- All 101 Rust tests and 33 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. Logs use `/private/tmp/espeak-stage30-*`.
- MBROLA-on/Klatt-off library compilation passes. An additional full build of
  that compile-only directory aborted while generating dictionaries, reporting
  absent `af_dict`, `ab_dict` and `am_dict`; backend data/runtime validation is
  not claimed. Actual Sonic/runtime/NPU/thermal gates remain open.

### Translation word transforms stage, 2026-10-06

Native Rust now promotes/reduces word stress, appends rule/suffix phonemes and
applies alternate pronunciations. Stress changes retain the C vowel-only
emission index even when extraction counted syllabic consonants. Unlike the
unchecked C write loop, a result exceeding the 200-byte word capacity is
rejected before publication. Append planning admits the complete tail and
checked vowel/stress count effects before any write; full destinations are
ordinary no-ops. Sparse/out-of-range records are skipped for counting, while
the original phoneme bytes remain in the appended string.

Alternate pronunciation changes only the byte immediately after the first
primary marker, with explicit signed/unsigned compatibility-byte behavior and
the original code-zero substitution for absent names. The extracted C oracle
caught and corrected an initial transform that continued through later vowels.
Native records and counter snapshots remain disjoint from output; no callbacks,
I/O or allocations occur. A caller-owned proactor fixture appends and changes
stress on the owner after voice asset completion.

- 336,480 stress changes, 336,532 appends and 336,532 attribute transforms match
  independently extracted C across compiled tables, language options, sparse
  gaps, admission limits and untouched caller tails. The expanded stress oracle
  also matches 336,532 extractions and assignments each. Native tests cover
  counter overflow, rejected expansion and both byte signedness modes.
- All 104 Rust tests and 33 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes. Logs use
  `/private/tmp/espeak-stage31-*`; backend runtime/data, NPU speech execution
  and thermal measurements remain open.

### MBROLA mapping ownership stage, 2026-10-06

Native Rust owns and decodes MBROLA mapping tables and performs contextual name
selection. Two reusable typed buffers preserve the active table/control if a
read, format check or admission fails. Combined reserved mapping capacity is
bounded to 128 MiB; reads use a fixed 3 KiB chunk and no per-record allocation.
Lengths must contain complete 24-byte records and a terminating name. Published
immutable views expire on successful replacement or destruction.

Name selection retains first-match precedence, previous/next phonemes, word
boundaries, lengthening, stressed syllables, split percentages, secondary names
and prefix chaining. Prefix changes are explicit owner effects, with defined
integer wrapping. Selection has no callbacks, allocations or I/O. File loading
is serialized initialization/worker work; the native API also installs host
proactor-resident bytes on the owner. Compatibility termination drains existing
workers, finishes waveform work, drops mapping storage and closes an initialized
backend through the existing C close routine. Backend discovery/startup,
protocol generation and PCM output remain C.

- All 52 mapping sources compile with the retained compiler into 4,146 records
  that load identically in Rust. 163,760 contextual selections match the
  independently extracted C lookup. Tests include repeated fresh reads, both
  reusable buffers, preserved views/control after missing/malformed files,
  owner recreation and the caller-owned proactor load/selection path.
- All 107 Rust tests and 34 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes. The mapping
  fixture was also rerun with a portable `/tmp` root after the full suites.
- Logs use `/private/tmp/espeak-stage32-*`. These are mapping/compile results;
  no external MBROLA process/audio, actual Sonic runtime, NPU speech execution
  or thermal result is claimed. The backend data/runtime gate remains open.

### Shared synthesis parameters and MBROLA output stage, 2026-10-06

Native Rust computes shared pitch calibration, pitch-dependent formants,
emphasis/general amplitude and voiced amplitude effects. Generated pitch and
emphasis tables retain the exact C values. Checked arithmetic and index checks
preserve the previous snapshot on rejected input; short-field conversions retain
the defined compatibility wrapping. The C waveform callers publish explicit
native effects while the waveform generator and command queue remain C.

MBROLA pitch contours use fixed owned storage with the original endpoint,
split, intermediate-point and final formatting. The ABI admits the complete
text and terminator before writing. PCM scaling works in place with no scratch
allocation: normal amplitudes have a proven signed-product bound; unusual
amplitudes preflight every sample before mutation. Backend reads remain C and
deliver an initialized sample span to Rust. No callbacks, I/O or NPU computation
occur in these scalar helpers. Caller-owned proactor fixtures exercise native
pitch/amplitude after voice completion and scaling after sound-icon completion.

- 200,000 pitch, 200,000 formant and 200,000 amplitude snapshots, 6,000 pitch
  contours and 851,968 PCM samples match independently extracted C. PCM coverage
  includes every signed 16-bit value at 13 positive/negative/zero amplitudes.
  Rejection tests verify arithmetic, indices, capacity, alignment and unchanged
  caller tails; both native and C oracles preserve the original operation order.
- All 111 Rust tests and 35 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage33-*`. External MBROLA process/audio,
  actual Sonic runtime, backend data/runtime, NPU speech execution and thermal
  measurements remain open. The full engine still requires C.

### SSML attribute and reference stage, 2026-10-06

Native Rust scans SSML attributes, matches mnemonic values, reads integer/time
values, plans UTF-8 copies, replaces key names and parses character/entity
references. Borrowed wide spans retain host code-unit width, including isolated
Windows surrogate units. Copies plan the complete admitted prefix before writing
and preserve unused output. References retain partial-number conversion counts,
hexadecimal signs/prefixes, decimal signs and entity effects; arithmetic outside
the defined destination range is rejected without publication. Integer overflow
returns the caller's default instead of overflowing the C accumulator.

Compatibility adapters supply synchronous pure locale whitespace classifiers;
they do not own engine state or perform work offload. Unquoted high code units
avoid the original out-of-domain `isspace` calls. Empty slash-delimited values
have an explicit representation, preserving quoted values starting with `/` and
avoiding the old empty-string backward read. A terminal bare attribute name has
defined bounded handling instead of advancing through both string terminators.
The retained-C oracle excludes that undefined terminal-name read. The SSML
controller, floating prosody parser and parameter/voice stacks remain C.

- In C and available UTF-8 locales, 800,000 mnemonic comparisons and 200,000
  lookups, numbers, copies, attribute scans, references and key replacements each
  match independently extracted C. Coverage includes quote/escape/truncation
  behavior, partial attribute-name matches, all reference entities, signed
  numeric boundaries and unchanged tails. Native tests cover checked bounds,
  overflow and wide-16 units. A real caller-owned proactor fixture parses and
  copies a loaded SSML tag on the owner after completion with fixed buffers.
- All 114 Rust tests and 36 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. Windows uses the installed x86-64 MSVC target; the initial
  ARM64 MSVC invocation could not compile because its target was absent.
  MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage34-*`. These are helper/parity and compile
  results; actual backend process/audio, Sonic runtime, NPU speech execution,
  real-platform runtime and thermal measurements remain open.

### SSML parameter-stack stage, 2026-10-06

Native Rust resolves nested parameter frames, emits ordered embedded commands,
pushes frames and unwinds closing tags. The compatibility final-slot saturation
behavior is retained: the last spare frame is reset/returned without increasing
the active frame count. Closing tags select the last matching non-base frame;
missing/base-only matches retain the active count. Negative values inherit from
earlier frames, and punctuation/capital settings remain explicit effects.

Plans use fixed 80-byte command storage with no allocation, I/O or callbacks.
The complete prefix and terminator are admitted before effects publish. The C
owner commits command bytes, parameter/options snapshots and the prospective
pop count after admission; failure preserves them. Unchanged plans require no
output capacity and do not write a terminator. Remaining SSML controller writes
and floating prosody/voice-stack work still run in C. The native controller
boundary also rejects an invalid parameter count before dereferencing a push.

- 100,000 full parameter selections, 100,000 pop plans and 95,263 valid pushes
  match independently extracted C, including all 15 parameters, nested/missing/
  duplicate tags, integer boundaries, negative inheritance, final-slot resets,
  caller tails and zero-frame snapshots. Rejected plans preserve their output
  effects. Native tests cover undersized command destinations and saturation;
  the real proactor SSML fixture plans/opens/closes a nested rate frame on the
  owner after completion. All earlier SSML helper comparisons still pass.
- All 116 Rust tests and 36 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage35-*`. The engine remains hybrid; no actual
  external backend/audio, Sonic runtime, NPU speech execution, real-platform
  runtime or thermal result is claimed.

### SSML voice-stack composition stage, 2026-10-06

Native Rust composes voice-stack selection properties in order, retaining known
name resolution, language overrides, exact base-language aliases, inherited
gender/age/variant values and compatibility unsigned-byte narrowing. Unresolved
names and absent languages retain the previous identifier as in the old static
storage. Resolved names copy their identifier before the next lookup. Frame
strings, packed base languages and prior identifier are admitted before lookup.

Resolution is a synchronous owner callback into existing catalogue selection;
no exclusive Rust engine/catalogue borrow crosses a callback. Input snapshots
must remain immutable/alive. Fixed effects publish after all callbacks finish,
then the C owner invokes final voice selection. Native base-variant planning
retains gender/selected-variant precedence and the original 39-byte clipped
identifier. Invalid/count/capacity/resolver cases preserve effect storage. These
helpers allocate nothing and perform no I/O or accelerator work; catalogue
resolution/final selection and the SSML controller remain compatibility paths.

- 100,000 complete choices match independently extracted C, including ordered
  resolver calls, unknown names, multilingual aliases, empty base lists, stale
  identifiers, byte narrowing, inherited/reset properties, missing selections
  and variant clipping. Admission tests reject malformed frames before lookup.
  All earlier SSML helper/parameter oracles pass. The caller-owned proactor tag
  fixture resolves/composes voice properties and a base variant on the owner
  after completion, with copied identifiers and fixed buffers.
- All 118 Rust tests and 36 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage36-*`. Full-engine C dependencies remain;
  external backend/audio, actual Sonic runtime, NPU speech execution,
  real-platform runtime and thermal measurements remain open.

### SSML native floating prosody stage, 2026-10-06

Native Rust parses bounded decimal and hexadecimal binary64 values and computes
prosody percentages, semitones, rate multipliers, absolute/relative values and
named parameters. The original sign-consumption and operation order are retained,
including bare semitones producing 100 percent and relative rate truncation
before adding 100. Hexadecimal conversion uses guard/sticky bits and ties-to-even
rounding, including subnormals, signed zero and the normal boundary. Decimal
conversion uses Rust's native binary64 parser with fixed 512-byte scratch.

The host supplies the locale decimal character and a synchronous pure whitespace
classifier; parsing and math no longer use `wcstod` in the engine. The XML source
limit is 500 wide units; native numeric spans admit up to 513 initialized units.
Non-finite/out-of-range integer conversions and overflowing products/additions
are rejected before updating the parameter. The oracle excludes undefined C
conversions/overflow. Initial `infinity`/NaN-payload end-position differences were
found by the oracle, corrected and covered by native tests. These are bounded
owner/worker helpers with no allocation, I/O or accelerator work. The real
proactor tag fixture computes a semitone parameter after completion.

- 400,000 binary64 conversions match C bit for bit under normal rounding,
  including arbitrary long hexadecimal significands, exponents, malformed
  prefixes/exponents, non-finite tokens and their consumed positions. 191,915
  defined prosody values and parameter updates each match extracted C. Additional
  locale-decimal cases run when a comma-decimal locale is available; the final
  oracle log records the locale and exact totals. All earlier SSML oracles pass.
- All 120 Rust tests and 36 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage37-*`. The SSML controller/frame dispatch
  and full engine still depend on C. External backend/audio, actual Sonic
  runtime, NPU speech execution, real-platform runtime and thermal gates remain
  open. The conversion result does not establish those runtime gates.

### SSML voice-frame dispatch stage, 2026-10-06

Native Rust creates SSML voice/language frames and plans closing-frame selection.
Names/languages retain quote, UTF-8 truncation and empty-attribute behavior;
gender matching requires the original closing quote, numeric attributes retain
defaults and variants zero/one remain equivalent. Frames publish initialized
owned storage before ordered voice selection. Count admission prevents the old
out-of-bounds add at slot 20; closing counts cannot discard the base frame.

The compatibility controller passes the count into its voice-attribute helper
by value. Native effects preserve that local-count behavior; this stage does
not change the host's stack-count semantics. Partial terminal attribute names
retain the bounded helper's absence result. Identifier changes admit the whole
new string before writing only its prefix/terminator, preserving caller tails
and accepting a self-source comparison. Oversized selected identifiers leave
the previous identifier unchanged. No exclusive Rust engine owner crosses
catalogue selection. Native frame parsing has no I/O or allocation and only
synchronous pure locale classifiers; the main tag controller remains C.

- 100,000 complete frame dispatches and 100,000 identifier changes match
  independently extracted C, including actual frame metadata, resolver order,
  selection properties, voice-change flags, inherited/quoted/unquoted values,
  closing frames, base variants and untouched caller tails. The C oracle uses
  initialized quoted empty values instead of the old static empty-string
  backward read. Native tests cover full-stack rejection and malformed input.
  The proactor fixture builds a voice frame from the loaded tag, then composes
  selection on the owner after completion. All earlier SSML oracles pass.
- All 121 Rust tests and 36 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage38-*`. The full C-to-Rust port remains
  active; backend process/audio, actual Sonic runtime, NPU speech computation,
  real-platform runtime and thermal validation remain open.

### SSML tag and parameter directive stage, 2026-10-06

Native Rust decodes bounded tag names using the host's pure locale classifiers
and a generated table of all 32 aliases. It preserves closing/self-closing,
separator, 39-unit name truncation and defined byte-narrowing behavior. The
compatibility owner applies the admitted slash replacement and separator.
The oracle excludes the old unsafe narrow-literal/wide-comment comparison;
ordinary ASCII declaration names retain their actual unknown-tag behavior.

Style, prosody and emphasis now produce complete native parameter frames before
the owner pushes them and applies native stack effects. Unknown emphasis levels
are rejected before frame mutation, avoiding the old unchecked array index.
Tone-language volume/range tables and ordered prosody updates retain C behavior.
The caller-owned proactor fixture decodes its loaded tag and plans a parameter
directive on the owner after completion. These scalar/string helpers allocate
nothing and have no I/O or NPU execution.

- 200,000 tag decisions and 200,000 full directive/frame/command comparisons
  match extracted C, including mutated tag/output tails, saturated parameter
  stacks, locale classifiers, narrowing, tone languages and named/numeric
  parameters. Earlier SSML oracles pass, including 420,000 binary64 parses and
  211,915 prosody values/updates with an available comma-decimal locale.
- All 123 Rust tests and 36 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage39-*`. Remaining SSML controller cases,
  translation, synthesis, tooling and owned engine integration still depend
  on C. Full port, external backend/audio, actual Sonic runtime, NPU speech
  computation, real-platform runtime and thermal validation remain open.

### SSML text directive stage, 2026-10-06

Native Rust plans phoneme wrappers, say-as mode/detail/format commands, key-name
closing, substitutions and ignore-text transitions. Borrowed attribute copy
plans stream into admitted output without whole-tag allocation. A completed
plan retains only the tag source, allowing the initialized output-prefix borrow
to end before sparse raw writes. Uninitialized unused output storage is never
borrowed as a Rust slice. State publishes after output admission and emission.

Compatibility details include no final NUL after phoneme brackets or say-as
closing, quoted-only mode matching, digits/detail precedence and the old-end
NUL left behind when key names shorten the logical output. Overflowing detail
arithmetic, invalid key starts and insufficient capacities preserve state/output.
The C oracle exposed a multibyte phoneme wrapper overrun beyond its declared
capacity; native dispatch rejects it before writes. The controller's preceding
separator remains a separate admitted effect. Proactor-loaded substitutions
are planned on the owner after completion, with reusable fixed output storage.
These scalar/string paths perform no I/O, heap allocation or accelerator work.

- 399,901 complete output/state/tail comparisons match retained C, with another
  99 legacy multibyte wrapper capacity overruns rejected without mutation.
  Earlier SSML oracles pass, including all tag, parameter/voice and prosody
  comparisons. Native regression tests cover key tails and output admission.
- All 125 Rust tests and 36 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage40-*`; initial oracle disagreements are
  retained in `ssml-parity-first`/`ssml-parity-second` logs. Break timing,
  clause/voice orchestration, marker/audio resources and the remaining engine
  still require porting. Real backend/audio, Sonic, NPU speech computation,
  platform runtime and thermal validation remain open.

### SSML pause and clause/voice transition stage, 2026-10-06

Native Rust plans break strength, embedded prepause commands, numeric time and
checked multiplier arithmetic. The compatibility owner admits/emits the command
and NUL before requesting the original rate update, then snapshots the resulting
pause factors for native timing. Sonic compensation retains ordered binary64
math and truncation; short-pause recalculation and long-pause scaling preserve C
behavior. Bad products, divisors and non-finite/out-of-range conversions reject
instead of overflowing. A failed finish can follow an already published command
and rate update, matching the explicitly separate effects; whole-controller
transactionality is not claimed.

Voice/clause plans admit active frame kinds only, unwind closing speak/voice
frames and emit ordered selection requests for sentences and paragraphs.
Compatibility frame-helper counts remain passed by value. The adapter reads
only initialized tag-kind fields, avoiding undefined property/string tails in
unused compatibility records. Pure plans retain no engine borrow across ordered
host calls. Clause terminators and accumulated voice-change flags finish in
Rust. XML-base name storage, marker/audio resource orchestration and the remaining
engine controller remain compatibility paths. Proactor-loaded tag time and voice
transitions are planned on the owner after completion without new scheduling.

- 200,000 break timing/output/rate-order comparisons match independently
  extracted C across both Sonic branches; 200,000 voice/clause transitions match
  original request order, count unwinding and terminators with deterministic
  selection effects. Earlier actual frame/voice composition and SSML oracles
  pass. This validates Sonic timing math, not an external Sonic runtime.
- All 127 Rust tests and 36 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage41-*`. Full engine/tooling/platform port,
  external backend/audio, actual Sonic runtime, NPU speech computation,
  real-platform runtime and thermal validation remain active/open.

### Native marker/URI name storage stage, 2026-10-06

Native Rust owns the marker, URI and compatibility wide-name byte arena.
Terminated narrow and two/four-byte wide sequences retain byte order, complete
prefix bytes and original offsets. Wide entries are opaque bytes: mixed narrow/
wide offsets retain C behavior and can be unaligned, so native consumers never
cast them to aligned wide references. Malformed sequences and exhaustion reject
before modifying existing entries. The reservation budget is at most 128 MiB
and rounds down to eight-byte storage words; growth uses bounded geometric
reservation and reset reuses initialized storage without per-utterance freeing.

Compatibility consumers serialize access and drain views before reset/growing
append/destruction. Input append sources are disjoint from the owner/backing
bytes. The old wide capacity bookkeeping is not reproduced: its visible byte
offsets/contents are retained, while allocation accounting is owned natively.
Termination releases the arena after async workers/events and waveform work
drain; initialization/reset preserves the public null empty view. Proactor-
loaded name bytes enter the native arena on the owner after completion.

- 100,000 mixed narrow/wide appends match extracted C offsets and complete byte
  prefixes. 1,000 warmed resets retain pointer/capacity. Native tests cover
  unaligned opaque entries, malformed unit widths/termination and exhaustion.
- Actual synthesis marker callbacks preserve ordered ASCII/UTF-8 names across
  eight utterances and two initialize/shutdown cycles, in each native build and
  the retained C-only API suite. This is callback retrieval, not speaker output.
- All 129 Rust tests and 37 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage42-*`. Marker/audio request orchestration,
  external resource/output execution, translation/synthesis/tooling/platform
  port and owned engine integration remain active. NPU speech execution and
  real-platform/audio/Sonic/thermal validation remain open.

### SSML marker/audio resource planning stage, 2026-10-06

Native Rust prepares bounded owned marker, audio-source and XML-base records,
decides awaited-marker clearing, admits complete sound-file paths, formats
embedded marker/sound/URI commands and plans audio push/pop/text effects.
Null/empty base and absolute-path precedence, quote/truncation, failed indices,
self-closing audio preservation and ordered parameter merging/pop retain C
behavior. The original local XML-base assignment remains local to this call.
Oversized paths reject before file operations instead of overflowing C storage.

The compatibility owner executes name appends, synchronous sound loading and
URI callbacks after native planning borrows finish. URI callbacks receive the
owned copied request name; growing the name arena cannot invalidate that text.
Base strings must remain live across host calls. Backend side effects remain
separate from output-capacity admission: this stage does not claim a whole
controller transaction or caller-proactor offload of sound loading. The native
proactor fixture prepares a loaded marker request and emits its native command
on the owner after completion. These planning helpers have no I/O/allocation/
engine callbacks or eligible NPU computation.

- 200,000 resource requests and 100,000 full marker/audio cases match extracted
  C, including exact output/tails, stack/current values, options, skip/audio
  flags and name/file/URI call order under deterministic backend responses.
  Capacity/discriminant rejection preserves initialized effect outputs.
- Actual retrieval synthesis verifies that a URI callback can append a 4 KiB
  name and grow the arena while retaining its request text; the subsequent PLAY
  event still reports the original URI. Earlier marker and helper oracles pass.
  Backend oracle responses do not establish external file/audio playback.
- All 131 Rust tests and 37 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage43-*`. Consolidated owned SSML/engine
  orchestration, remaining translation/synthesis/tooling/platform port, backend
  output/offload and real-platform/audio/Sonic/NPU/thermal execution remain open.

### Owned native SSML controller stage, 2026-10-06

The standalone native controller combines tag decoding, parameter/text plans,
voice selection, break timing and marker/audio requests into initialized owned
state. Independent instances retain separate stacks, identifiers, options and
flags. The host owns separate name/catalogue/backend/rate resources and supplies
pure locale classifiers and copied results. Explicit publication/refresh hooks
allow a future serialized compatibility bridge to synchronize external effects
without borrowing a process-global Rust engine across a C/user callback.

Output uses an initialized-prefix/writable-capacity contract with sparse admitted
writes and a checked native buffer adapter. Sixteen/thirty-two-bit tag code units
retain platform compatibility encoding. The controller allocates no per-tag
storage, preserves local voice-frame counts and the existing separate-effect
failure behavior. Resource methods execute on the caller owner/worker; the
controller does not introduce threads, polling or an app-local scheduler.

- Four native integration tests cover instance isolation, nested parameters,
  actual dispatcher key separators and old-end NUL, copied URI lifetime across
  name growth, audio push/merge/pop/text order, rate/voice requests, capacity and
  emphasis rejection, and Windows surrogate code-unit output. An initial test
  expectation supplied an extra key separator; it was corrected to include the
  dispatcher's own separator. The actual proactor-loaded voice fixture now runs
  through the consolidated controller on the owner after completion.
- All 135 Rust tests and the existing 37 static/shared/legacy-async CTests pass;
  C-only passes 19. Strict Clippy, minimal features, formatting/provenance and
  Linux/Windows/iOS/Android cross gates pass, including minimal Windows Clippy
  and Windows test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage44-*`. The C compatibility dispatcher still
  uses the individually validated native plans: wiring the consolidated owner
  and verifying its complete controller against C remain next work. This stage
  does not establish a native-only speech pipeline or final engine ownership.
  Remaining translation/synthesis/tooling/platform, external backend/output,
  actual Sonic/NPU speech and real-platform/thermal gates remain open.

### Consolidated SSML compatibility stage, 2026-10-06

The C SSML entry point now snapshots initialized active fields into the owned
native controller. Its thin adapter supplies locale classification, name/file/
URI, voice and rate callbacks; the previous intermediate dispatch branches are
retired. Only initialized terminated string prefixes are read from legacy
records. Sparse raw output writes avoid borrowing undefined capacity bytes;
unchanged inactive records and string tails remain untouched. State is published
before host effects and refreshed after them without retaining a foreign Rust
engine borrow across callbacks. Original XML base strings must remain live and
disjoint from the growable name arena. Rejection preserves earlier admitted
effects; it does not imply a whole-tag transaction.

- An independently extracted full retained-C controller matches 240,000
  combined transitions across nested parameter/voice stacks, spare stack slots,
  selection failures and base variants, aliases, markers, audio/file/URI effects,
  callback parameter changes, timed breaks and text directives. Comparisons
  cover output including tails, initialized state, XML mutation and callback
  order. Undefined unused voice-string tails and the legacy narrow-literal
  wchar comment check are excluded from the oracle's parity claim.
- Wiring review corrected missing voice selection to return the unmodified
  `default` identifier rather than adding the base variant; a regression test
  covers this. Invalid refreshed host counts/flags stop further native work.
  Actual API and URI-growth/name-event regressions pass through the new bridge.
- All 136 Rust tests and 38 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, provenance/formatting and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage45-*`. Host resource work still executes on
  its caller owner/worker; this stage adds no scheduler or accelerator workload.
  The complete engine remains hybrid. Remaining clause/translation/synthesis,
  tooling/platform, backend/output and real-platform/audio/Sonic/NPU/thermal
  execution gates remain open; the C build dependency is retained.

### Clause input and permissive UTF-8 stage, 2026-10-06

Clause character replay/count handling, Unicode speech-property classification,
Roman-letter checks, language-mnemonic unpacking, ignore/replace tables and
phoneme-input mode updates now run in Rust. The native input instance owns its
cursor and decoder position while borrowing immutable caller text. Compatibility
calls snapshot the serialized legacy fields; replay never increments the source
count, zero remains the empty replay slot, and checked count admission precedes
decoder advancement. The larger clause parsing loop and translator callbacks
remain C orchestration.

The common UTF-8 character reader now runs in Rust with its permissive legacy
semantics: directional continuation skipping, non-continuation tail acceptance,
truncated-at-NUL values, overlong/surrogate/out-of-range numeric codes, and width
excluding skipped bytes. Its safe API rejects absent readable storage. The
extent-free legacy ABI retains the caller's directional/head/tail storage
contract and reads only needed initialized bytes. Language-word output writes
only prefix plus NUL; unused caller tails are preserved. These scalar paths have
no I/O, allocation, scheduling or eligible NPU computation.

- Independently extracted C matches all 1,114,112 Unicode codepoint clause
  classes, all 4,096 speech-property combinations, 200,000 preprocessing cases,
  500,000 permissive directional UTF-8 scans and 240,000 cursor transitions.
  Native tests cover instance isolation, AUTO peek/fallback, replay at EOF,
  count overflow without decoder advancement, malformed table and slice bounds.
- All 140 Rust tests and 39 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- The language-table provenance extractor now stops at the retained UTF-8 body
  marker, excluding its new compatibility branch; generated presets still
  match. Logs use `/private/tmp/espeak-stage46-*`. Full clause/translation,
  synthesis, tooling/platform and external backend/output work remain open.
  Real-platform/audio/Sonic/NPU/thermal execution and C dependency removal
  remain final full-port gates.

### Native punctuation announcement stage, 2026-10-06

The clause parser now delegates punctuation announcement to a native controller.
It retains sound-icon and period/character dictionary lookup order, repeated
punctuation consumption, short-run speed commands, counted long runs, deferred
punctuation replay and exact pause selection. Name/source callbacks return
copied initialized results and keep their separate serialized owners; dynamic
announcement flags and speed are read after callback effects. No foreign Rust
engine/input/output borrow crosses those callbacks.

Native plans admit the complete output including its NUL and the existing
200-byte scratch bound. Backend/input effects can precede capacity rejection;
output remains unchanged when its plan rejects. The private C adapter now
receives the caller's output capacity. It keeps the retained C algorithm as an
independent oracle, adding only that unused capacity argument to its signature.
Period/character name lookup and phoneme-text formatting still use C callbacks.

- 200,000 independently extracted retained-C cases match exact output/tails,
  pause results, source position/count/pushback and backend order, including
  name callbacks that change flags and speed. Native tests cover repeated-name
  commands, deferred semicolon behavior and capacity/scratch rejection with
  earlier source effects retained. Defined legacy names stay within scratch
  bounds; legacy scratch overruns are excluded from the parity claim.
- All 142 Rust tests and 40 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage47-*`. These bounded scalar/string paths
  execute on the caller owner/worker without new scheduling or NPU computation.
  Full clause/translation/synthesis/tooling/platform, backend/output and final
  real-platform/audio/Sonic/NPU/thermal gates remain open. C remains required.

### Native phoneme text and clause wrappers stage, 2026-10-06

Internal phoneme mnemonic decoding now runs in an allocation-free native plan.
It retains missing/255-code skipping, stress characters, packed mnemonic order,
language-switch alphabet consumption and the initial `* ` write's observable
unused output bytes. Table/locale reads are pure and stable across planning and
emission. Signed high-byte `isalpha(char)` undefined inputs reject before the
classifier; absent NUL and output-capacity failures reject before publication.
The legacy extent-free adapter retains its caller-owned writable-footprint
contract; native and known-capacity callers use explicit output admission.

Clause phoneme wrappers also use native formatting, including fallback/default
voice markers and zero-byte removal in packed language names. Private special
lookup/formatting calls now carry the actual destination capacity; word-stress
effects execute before bounded decoding/formatting without a foreign Rust engine
borrow across that host work. Defined-input text and tails remain exact. On
rejected legacy formatting the void adapter supplies an empty string after its
earlier stress effects. Character-name/dictionary/rules fallback orchestration
still uses C and remains next work.

- 200,000 independently extracted retained-C decoder cases and 200,000 clause
  wrapper cases match exact output/tails and stress/flag effects. Tests cover
  initialized pointer-table holes, stress/program combinations, language-switch
  text, missing terminators, signed classifier domain and complete capacity
  admission. Native tests explicitly cover short initial-write tails and packed
  language bytes. Earlier punctuation and actual API/language/WAV gates pass.
- All 144 Rust tests and 41 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage48-*`. No I/O/scheduler/accelerator work is
  added. Full clause/translation/synthesis/tooling/platform and external backend/
  output port remain open, as do final real-platform/audio/Sonic/NPU/thermal
  execution and removal of the C build dependency.

### Native character and special-name lookup stage, 2026-10-06

Character/special-name lookup now uses owned initialized native word, phoneme,
flag and text buffers with copied backend commands. Prefixed/unprefixed lookup,
rules, default-voice fallback, formatting and table restoration retain their
order, including live language metadata after rules. Only-name misses remain
empty; ordinary exhausted lookups retain the original placeholder. Special
names are copied before backend callbacks and retain their55-byte legacy
phoneme bound. Dictionary/rules/default-translator setup and formatting execute
through their separate serialized compatibility owner without a foreign Rust
engine/storage borrow across callbacks.

The adapter publishes the initial character-name NUL before lookup and copied
formatted text before fallback-table restoration. A whole-path oracle caught
the latter publication-order difference and verified its repair. Fallback-table
restoration also runs after guarded backend/format failures. Backend effects
can precede later rejection; complete output prefix+NUL admission remains
separate and no whole-operation rollback is claimed.

- 200,000 independently extracted retained-C character-name cases and 200,000
  special-name cases match output/tails, backend/flag order, live metadata and
  fallback-table state, including output observed inside callbacks. Native tests
  cover complete fallback order, missing only/special names and restoration
  after formatting failure. Capacity/discriminant rejection and earlier API,
  pronunciation and WAV regressions pass.
- All 146 Rust tests and 42 static/shared/legacy-async CTests pass; C-only passes
  19. Strict Clippy, minimal features, formatting/provenance and Linux/Windows/
  iOS/Android cross gates pass, including minimal Windows Clippy and Windows
  test compilation. MBROLA-on/Klatt-off library compilation passes.
- Logs use `/private/tmp/espeak-stage49-*`; `names-first.log` retains the
  publication-order mismatch. Host work runs on its caller owner/worker; no
  scheduler or eligible NPU operation is added. Main clause/translation,
  synthesis/tooling/platform, backend/output and final real-platform/audio/
  Sonic/NPU/thermal gates remain open. C remains required.

### Native main clause controller stage, 2026-10-06

The entire main `ReadClause` loop now runs in Rust. Its instance owns replay,
counts, options, tone/voice results and copied parameter state, with sparse
character-index writes to the caller's buffer. Entity replay, tags, embedded
commands, paragraph/line boundaries,
capital names, phoneme mode, Armenian emphasis, ellipsis and repeated terminal
punctuation retain their order. Hungarian ordinals, acronym genitives, delayed
post-tag sentence decisions and Malayalam/Sinhala handling remain intact.
Native hosts supply separate source/classifier/replacement resources and copied
backend effects; compatibility calls use the already native SSML, punctuation
and name controllers against their serialized engine owner.

The output adapter never borrows unused foreign byte/index tails. It tracks
only the contiguous initialized prefix, including retained initialized bytes
after a tag shortens the logical result. Copied state publishes before backend
callbacks and refreshes afterwards. Capacity/arithmetic errors preserve earlier
admitted effects; the compatibility adapter supplies an empty EOF result after
rejection. The extent-bearing Rust interface reports the error directly.

The controller oracle exposed a repeating EOF punctuation-deferral case under
its deterministic name backend. Rust bounds consecutive internal replay steps
without new source admission (four times the 24-byte replay storage); it adds
no bound on ordinary input/word lengths. The original fixture sequence exceeded
100,000 source-predicate visits, while native execution returns a guarded error.
The real retained-C backend completed the shortest `>..` reproducer, so this is
controller evidence rather than a claim that every backend hangs. A separate
native regression rejects the legacy four-byte-character terminator overrun.

- The final oracle compares 234,679 terminating clauses across 100,000 episodes,
  including 96–512-byte output capacities, and guards one retained-C deferral
  loop. Output/tails, sparse indexes, source positions/replay/counts, flags,
  voice/tone state and callback-visible state/order match. Earlier runs before
  boundary expansion compared 242,775 clauses and exposed four loop episodes.
  Backend fixtures are deterministic; actual API, language/pronunciation,
  marker/URI, voice and WAV gates pass separately.
- All 152 Rust tests (139 unit, eight proactor and five resident) and 43 static,
  shared and legacy-async CTests pass; C-only passes 19. Strict all-target Clippy,
  minimal features, formatting/provenance and Linux/Windows/iOS/Android cross
  gates pass, including Windows test compilation. MBROLA-on/Klatt-off library
  compilation passes; external backend process/output remains unvalidated.
- Logs use `/private/tmp/espeak-stage50-*`; `oracle-first.log` was interrupted
  to investigate the loop, and subsequent diagnostic logs preserve its source
  and controller state. The caller owns execution; no scheduler or eligible
  NPU operation is introduced. Common predicates, number/translation frontend,
  synthesis/tooling/platform, owned full engine integration, backend/output and
  final hardware/audio/Sonic/NPU/thermal gates remain open. C remains required.

### Native common text helpers stage, 2026-10-06

The shared character predicates now run in Rust. Host-wide classification
retains locale/platform behavior before Indic, Hebrew/Arabic vowel marks,
combining accents, Tibetan, jamo, braille and Chinese/Japanese word extensions.
Emoji, regional indicators, skin-tone modifiers and tag ranges retain the
fork's token rules. Bracket membership preserves its one-based list index;
space fallback preserves the raw host result rather than normalizing it to 1.
ASCII/extended digits and the restricted byte-space predicate remain exact.

Null scanning and four-byte word packing use initialized byte reads, including
unaligned input and early nonzero/NUL admission. The bridge never borrows unused
tails; a single readable nonzero byte suffices for the legacy null-scan early
return, and word packing needs no terminator after four readable bytes.
Turkish dotless-I conversion and generated Unicode lowercase also use Rust.
The safe packer defines all high-byte bit patterns; the C oracle restricts the
most significant byte to avoid its signed-left-shift undefined domain.

- The extracted retained-C oracle matches 3,342,336 full-range codepoint/locale
  cases across `C`, `en_US.UTF-8` and `C.UTF-8`, plus out-of-range/WEOF values,
  and 200,000 word-packing and 200,000 null-scan cases. It compares raw integer
  return values, both lowercase policies and early admission. Native tests cover
  high-bit packing, special ranges, raw bracket/space values and empty spans.
- All 154 Rust tests and 44 static/shared/legacy-async CTests pass; C-only passes
  19. Actual API, language/pronunciation, emoji/SSML and WAV gates pass, including
  the main-clause oracle. Strict/minimal/cross/provenance gates and MBROLA-on/
  Klatt-off library compilation pass. Logs use `/private/tmp/espeak-stage51-*`.
- No I/O, scheduler or eligible NPU work is introduced. Byte-copy/file primitives
  and random state still use C, as do remaining number/translation, synthesis,
  tooling/platform and full-engine resource/output integration. Final hardware,
  audio/Sonic/NPU/thermal execution and removal of the C build remain open.

### Common primitives checkpoint, 2026-10-07

Recovered and validated the uncommitted common-primitives slice left by the
interrupted session. Bounded copies preserve truncation, forced termination
and zero padding without reading beyond the required source prefix. Stream
words retain exactly four host CRT reads, including EOF, and little-endian
packing. The `c-abi` feature uses optional `libc` for the platform's `FILE`
type and `fgetc`; the safe API accepts a caller-supplied byte reader.

Native random instances retain the original recurrence, seed flush and
remainder-minus-min expression, including negative ranges. The C adapter uses
atomic compatibility state and checks arithmetic in the target's `long` width.
Undefined overflow and zero-divisor inputs return zero without advancing state.
Stream reads remain synchronous caller-owned work; no worker, scheduler or
NPU operation is introduced.

- Retained-C parity: 2,000,000 random outputs, 200,000 bounded copies and
  20,000 stream words, plus partial/full EOF and invalid-input guards.
- On macOS, all 156 enabled all-feature Rust tests pass; the two asset-dependent
  tests deferred by Cargo are exercised by CTest. Minimal-feature tests, strict
  all-target/all-feature Clippy, formatting and both generated-table checks pass.
- All 45 CTests pass in each static, shared and legacy-async Rust-core build;
  the C-only reference passes all 19. These include API, language/pronunciation,
  SSML/emoji, WAV and retained-C oracle checks. Build and test logs use
  `/private/tmp/espeak-stabilize-*`.
- `c-abi` library cross-target Clippy passes for Linux aarch64 and Windows
  x86-64; compile checks pass for iOS and Android aarch64. These are compilation
  checks, not runtime validation on those platforms.

Jay requested stabilization followed by stopping. The native Rust migration
remains incomplete; the full engine still requires C. Further porting and
hardware/audio/backend/thermal validation are deferred.

### Clause intonation stage, 2026-10-07

`CalcPitches` and `CalcPitches_Tone` now run in Rust. `intonation::calc_pitches`
takes a copied 14-byte entry per phoneme-list item, the current phoneme table,
borrowed compiled `intonations` bytes (68-byte `TUNE` records, layout asserted
in C) and copied translator options. The fixed head/nucleus tables are native;
the envelope tables moved to Rust in the lengths stage below. Work is stack-only,
with no allocation, I/O or scheduling. Only stress, tone, envelope and pitches
are written back.

Legacy behavior is kept where it is observable: consonants take the following
syllable's stress, the last list entry is excluded from the syllable scan, an
emphasis split overwrites the end-of-clause flag, the unstressed count includes
its end index, `head_extend` reads past eight entries into following tune bytes,
and the zero tone-language gradient leaves tone levels unchanged. Mandarin,
Hakka and Vietnamese sandhi follow C's order, including the stale tone phoneme
used after a tone-5 change. A tone group that starts after the first syllable
and has no primary stress writes its pre-head from `start + end` rather than
`end`; Rust keeps those writes in a zeroed table twice the list capacity, where
C overruns its uninitialized 1,000-entry stack table.

Inputs that C reads out of bounds are rejected with the list unchanged: more
than 1,001 entries, syllable stress above 7, clause types past six, negative
groups, tunes outside the compiled or 13 fixed tables, and tone records missing
where C dereferences them. One such case is reachable from configuration: with
a nonzero intonation group, the remainder after an emphasis split takes
`langopts.tunes` (compiled tune numbers) as a fixed-table index. Phoneme table
slots past `n_phoneme_tab`, which C may still read as stale pointers, are absent.

- The extracted retained-C oracle (`rust_intonation`) matches 135,360 random
  clauses across every compiled phoneme table and 16 translators, including
  45,016 tone-language runs, emphasis splits, clause pauses, penultimate
  emphasis, both tune systems and lists up to the full capacity. It zeroes and
  doubles C's syllable table (configuration fails if that line changes).
  9,024 rejected inputs leave the list unchanged. Three injected Rust faults
  (drop clamp, tone level, unstressed count) were each detected.
- 260 WAVs (27 languages plus Mandarin, Cantonese, Hakka, Vietnamese, voice
  variants and Klatt; statements, commas, questions, exclamations, SSML
  emphasis and abbreviations) are byte-identical between C-only and Rust-core.
- On Linux x86-64: 160 all-feature Rust tests, minimal-feature tests, strict
  all-target Clippy, formatting and both generated-table checks pass. All 46
  CTests pass in static and shared Rust-core builds; C-only passes all 19.
  `non-executable-files-with-executable-bit` fails if a root `cargo test` has
  left `target/` in the tree; it passes once that is removed.
- Cross-target, legacy-async, MBROLA and real-platform audio gates were not
  run for this stage. Phoneme lists, lengths, synthesis queues and waveform
  generation remain C.

### Clause lengths stage, 2026-10-07

`CalcLengths` now runs in Rust. `lengths::calc_lengths` sets pre-pauses, lengths,
amplitudes and pre-vocalic pitch over a copied 28-byte entry per list item, which
carries the fields of the entry's phoneme record and resolved tone phoneme.
Translator stress tables and both 100-entry length-modifier tables are copied
into the settings. The function's `static more_syllables`, which carries a
stop's pre-pause context into the next clause, is explicit caller state that
changes only on success.

The legacy loop reads up to four entries past a vowel and scans forward to the
next word start. Real clauses end with the end-of-clause and short pauses, but
nothing bounds those reads to the clause, so C can read stale entries left by
an earlier, longer clause. The adapter copies entries through the first one
without a phoneme (`phoneme_list[N_PHONEME_LIST]` is a null sentinel); a read
past that span is an error. Length-modifier indices past 100 are also errors.
Unsigned length, `unsigned char` prepause/amp/pitch truncation and short-circuit
read order follow C; the `int` length arithmetic wraps rather than overflowing.

Two engine effects stay with the host and run in C's order: embedded commands
at a flagged entry (`DoEmbedded2`, which can change speed and so the length
factors mid-clause) and the first pitch-envelope byte of a tone phoneme's
program. On error the snapshot is discarded and the list unchanged, but speed
commands already applied stay applied, as they would in C. Bad envelope numbers
are counted and reported by the adapter with C's message.

The 20 pitch envelopes are now Rust data (`rust/envelope.rs`). The Rust-core
build exports `envelope_data` and `env_fall` under their C names for synthesis,
MBROLA and `GetEnvelope`; the C arrays compile only in the C-only build.

- The extracted retained-C oracle (`rust_lengths`) matches 90,240 random
  clause-shaped lists across every compiled phoneme table and 16 translators:
  2,057,636 syllables, 342,841 with tone phonemes, 361,205 embedded command
  groups (absolute and relative speed changes, compared through the resulting
  speed state too), varied word gaps, stress flags, tonic lengthening, length
  limits and end-of-clause amplitude, with stale entries after each clause.
  2,256 forced scans past the span leave the list unchanged. The 20 Rust
  envelope tables equal the extracted C arrays byte for byte. Four injected
  Rust faults (nasal adjustment, pre-vocalic amplitude cap, voiced fricative
  length, sequence-continue flag) were each detected.
- 316 WAVs are byte-identical between C-only and Rust-core builds, now
  including SSML rate changes mid-sentence (embedded speed commands).
- On Linux x86-64: 162 all-feature Rust tests, minimal-feature tests, strict
  Clippy, formatting and both generated-table checks pass. All 47 CTests pass
  in static and shared Rust-core builds; C-only passes all 19.
- Cross-target, legacy-async, MBROLA and real-platform audio gates were not
  run for this stage. Phoneme-list construction, synthesis queues and waveform
  generation remain C.

### Clause phoneme list stage, 2026-10-07

`MakePhonemeList` and its helpers now run in Rust. `phoneme_list::make_phoneme_list`
takes the first-stage list (`PHONEME_LIST2`, same layout) and updates it in place
as C does: last-word stress promotion, removal of switches to the current table
or before another switch, regressive voicing and voice phoneme replacements.
It then builds the synthesis list. The working list (`ph_list3`) is owned in Rust,
and each phoneme program runs on the native VM through a `phoneme_context::Storage`
over that list, with the word's previous-vowel snapshot and table refreshes
following the C adapter. Change, insert, append and next-phoneme replacement,
unstressed-syllable reduction, consonant doubling (the legacy `strchr` also
matches pauses), word-boundary and word-gap pauses and the two terminating
pauses keep C's order.

The current phoneme table changes inside the clause, and C stores record
pointers from whichever table was current. The host's only job is `select`, which
makes a table current and copies its 256 slots; `SelectPhonemeTable` clears
unused slots, so the copy is exact. Each output names its phoneme, and tone data
for switched-language words, as a slot in a table; the adapter resolves the same
pointers C held. The terminating pauses set only the fields C sets, and the
fields C leaves stale in every entry (`std_length`, sound address/parameter)
stay untouched. The leading pause is resolved in the table left by substitution,
as in C.

Errors cover inputs that make C dereference a missing phoneme or read past its
lists: an empty clause, a switch looking past the readable entries, an unstressed
scan without a closing pause, a missing changed/inserted/next phoneme, and an
output past the list capacity. The adapter then produces an empty list for the
clause. No compiled phoneme uses `ChangeNextPhoneme`, so that path is covered by
a Rust unit test with a synthetic program rather than the oracle.

- The extracted retained-C oracle (`rust_phonemelist`) matches 7,200 random
  first-stage clauses across 18 translators (579,835 synthesis entries, 25,179
  table switches including redundant runs, 5,873 voice replacements and 5,413
  phonemes absent from the current table), with varied regressive voicing,
  stress reduction flags, vowel and word-gap pauses, program reduction and the
  global word gap. Both the synthesis list (including fields neither side should
  touch) and the updated first-stage list and count must match. Phoneme programs
  ran change 23,161, append 2,085 and insert 265 times. Seven of nine injected
  Rust faults were detected; of the others, the next-phoneme replacement is
  caught by its unit test, and the deleted-word-start guard can never be false.
- 321 WAVs are byte-identical between C-only and Rust-core builds, now including
  mixed-script text that switches phoneme tables mid-clause.
- On Linux x86-64: 165 all-feature Rust tests, minimal-feature tests, strict
  Clippy, formatting and both generated-table checks pass. All 48 CTests pass in
  static and shared Rust-core builds; C-only passes all 19.
- Cross-target, legacy-async, MBROLA and real-platform audio gates were not run
  for this stage. Translation, synthesis queues and waveform generation remain C.

### Clause synthesis driver stage, 2026-10-07

`Generate` now decides in Rust. `generate::generate` walks a copied clause
(each entry with its phoneme record, plus the two neighbours past the clause)
and issues, per phoneme type, the same host effects in the same order as C.
These cover pauses and pre-pauses, pitch and amplitude envelopes (an
`envelope_data` table or a phoneme-data address), syllable marks, spectrum
sequences with their format parameters, samples, word/sentence/phoneme/end
markers, embedded commands, frame breaks and the two list writes (the stop's
next-pause flag, a pause's standard length). Vowel starts and ends come from the
vowel's program or its neighbours'. Tone phonemes supply pitch and amplitude
envelopes. Generation suspends when the queue is short and resumes from
explicit `State` (the legacy statics, including the text position C carries
across clauses).

The compatibility adapter maps one effect callback onto the existing queue
writers, so the wavegen queue, frame pool, `SmoothSpect` state and sample
handling are unchanged C for now. `pitch_started` reads the queue layer's
`last_pitch_cmd`. MBROLA voices still take `MbrolaGenerate`. Errors cover lists C
would read out of bounds or dereference as NULL (a neighbour past the copied
span, a missing phoneme, an envelope number past the tables, a tone without a
phoneme); the adapter then ends the clause.

- The extracted retained-C oracle (`rust_generate`) runs the legacy driver with
  every queue writer, phoneme program, marker, embedded command and frame/reset
  state write redirected to a recorder, and the Rust driver through a recording
  host. 12,000 random clauses over eight phoneme tables issue 6,692,951 effects
  identically, in order and with identical arguments (format parameters,
  phoneme data and resolved envelope pointers), across 8,300 queue-space
  suspensions, with phoneme events, IPA names, output hooks and word merging
  varied. The list writes must match too. Ten injected Rust faults were each
  detected. A Rust unit test covers a suspended vowel clause end to end.
- 321 WAVs are byte-identical between C-only and Rust-core builds.
- On Linux x86-64: 166 all-feature Rust tests, minimal-feature tests, strict
  Clippy, formatting and both generated-table checks pass. All 49 CTests pass in
  static and shared Rust-core builds; C-only passes all 19.
- Cross-target, legacy-async, MBROLA and real-platform audio gates were not run
  for this stage. The command queue writers, frame pool, `SmoothSpect` state,
  wavegen/Klatt and translation remain C.

### Synthesis command writer stage, 2026-10-07

The command writers now run in Rust: `DoSpect2`, `DoSample2`/`DoSample3`,
`DoPause` with `PauseLength`, `DoPitch`, `DoAmplitude`, `EndPitch`, `EndAmplitude`
and `StartSyllable`. `commands::State` owns what they share and what the legacy
code kept in file statics: the pending pitch/amplitude commands and lengths, the
last frame and command, the syllable marks, the format amplitude and
`DoSpect2`'s wave flag. In the Rust-core build those names are macros over the
one state, so `SynthesizeInit`, the `Generate` effects and the formant
transition code are unchanged.

The queue stays C for now (wavegen consumes it). Writers push only the words C
wrote, leaving the rest of an entry as it was, and patch lengths and frames in
place. Frame, envelope and sample addresses are carried as the queue's own
words. Sample addresses are computed from the borrowed phoneme sound data, and
headers are read with bounds checks. Spectrum lookup, smoothing, frame
length/flags and the high-formant frame copy are host operations. A spectrum
lookup can run formant transitions, which in C called `DoPause` from inside it.
The adapter now records those pauses during the lookup, and the Rust writer issues
them right after it returns, which is the same queue order without re-entering
the state. Smoothing takes and returns the syllable start by value.

Where C reads a sample header past the sound data, never finishes splitting a
sample shorter than four units (an infinite loop), or gets more frames or
transition pauses than a lookup holds, the writer returns -1 (the adapter
reports length 0). Commands already queued stay queued.

- The extracted retained-C oracle (`rust_commands`) runs the legacy writers with
  the queue, spectrum lookup, smoothing, frame copies and `seq_len_adjust`
  redirected to fixtures, and the Rust writers through the same fixtures over
  their own queue. 60,000 random scripts (1,226,884 operations; 4,818,851 queue
  writes) from identical starting state leave identical queues (including words
  left untouched), state, copied frames and host-call sequences. They cover
  queue wrap, unset pitches, long pauses past the overflow guard, a sample rate
  that separates the two pause conversions, 8/16-bit samples with and without
  mixing, vowel starts/ends, Klatt, wave cancelling, high-peak frame copies,
  transition pauses and modulation, and length-only (MBROLA) calls. The two
  inputs C mishandles are checked to return -1. Thirteen injected Rust faults
  were each detected; Rust unit tests cover a lookup with a transition pause.
- 321 WAVs are byte-identical between C-only and Rust-core builds.
- On Linux x86-64: 168 all-feature Rust tests, minimal-feature tests, strict
  Clippy, formatting and both generated-table checks pass. All 50 CTests pass in
  static and shared Rust-core builds; C-only passes all 19. The Rust-core
  library also compiles with MBROLA enabled; MBROLA output was not run.
- Cross-target, legacy-async and real-platform audio gates were not run for this
  stage. The wavegen queue and its consumer, frame pool, spectrum lookup and
  smoothing adapters, markers, embedded commands and translation remain C.

### Wave generator stage, 2026-10-07

`wavegen.c`'s formant generator and queue consumer now run in Rust.
`wavegen::Wavegen` owns everything the legacy file kept private: the copied
voice, formant peaks and their increments, the two harmonic spectra and
low-harmonic increments, flutter, gain control, glottal and roughness
modulation, breath resonators, cycle/segment counters, the amplitude envelope,
echo length, the peak-shape and HF window tables, and the function statics
(the resume flag, echo completion, the silence and wave sample counters). The
C adapter holds one instance for the process.

The memory other synthesizers share stays C and is read and written in place
through pointers: the command queue and its head/tail, the echo ring, the
output pointers, the embedded values and the sample rate. `WcmdqStop`,
`WcmdqFree`/`Used`/`Inc`, `Write4Bytes`, the sonic speed-up wrapper
(`WavegenFill`) and the output-hook API stay C. Klatt (and speechPlayer behind
it) gets pointers to the generator's `WGEN_DATA` and voice copy, as before.
Klatt, MBROLA, markers, phoneme alignment, the sample-rate event, sonic speed,
voice frees and `espeak_rand` are host operations. Queue words that hold
addresses (frames, envelopes, sample data, voices) are dereferenced as C did.
Frames are read only up to `fright`, so a short frame at the end of its data
is never read past.

The arithmetic follows C on the compiled targets. Signed overflow wraps. A
double that does not fit an int converts to `INT_MIN` on x86 (as
`cvttsd2si`) and saturates elsewhere (as `fcvtzs`). C's two harmonic tables
are contiguous, so writes past the first one land in the second; the port
does the same. Some paths only trap or corrupt memory in C, and the port takes
a defined action instead:

- writes past the second table are dropped (in C they overwrite other
  statics);
- a zero divisor gives 0 (C traps);
- a non-positive harmonic pitch gives no harmonics (C loops through memory);
- a zero-width peak skips its shape;
- the bass ramp stops when its step is 0;
- out-of-table modulation or tone indices read 0;
- an echo head past the ring (a delay longer than the ring at a high rate) is
  not written for one sample (C wrote past the ring once);
- a null queued voice or frame is ignored or reads as zero.

- The extracted retained-C oracle (`rust_wavegen`) compiles the whole legacy
  generator against its own copy of the shared memory, with Klatt, MBROLA,
  markers, output hooks and the random generator as recording fixtures. The
  Rust generator runs over a second copy through the compatibility FFI. 400
  random programs (222,312 queue commands, 93,849 buffer fills of random
  sizes, 143,720,820 samples) leave identical output bytes, echo rings, queue
  heads, embedded values, sample rates and host-call sequences. Host calls
  include every Klatt call's `WGEN_DATA` snapshot. The programs cover all
  queue commands, sample rates from 8 to 40 kHz, voice changes, breath, echo,
  roughness, glottal stops, tone tables, mixed waves, constant F0 and direct
  `SetEmbedded`/`GetAmplitude`/`PeaksToHarmspect`/`WavegenSetVoice` calls. A
  one-sample-buffer mode (`WAVEGEN_ORACLE_SINGLE`) matched 1,500 programs
  (149,800,000 fills). Inputs where C overwrites other statics (segments
  shorter than a cycle between distinct frames) are excluded. 22 injected Rust
  faults were each detected. Rust unit tests cover the guarded C traps,
  embedded clamping, resumption and a spectrum segment.
- 321 WAVs are byte-identical between C-only and Rust-core builds. A second
  corpus of every voice variant (`en+<variant>`, including Klatt, whisper and
  echo variants) at default, high and low pitch/amplitude/rate settings gives
  315 identical WAVs in both the static and shared Rust-core builds.
- On Linux x86-64: 175 all-feature Rust tests, minimal-feature tests, strict
  Clippy, formatting and both generated-table checks pass. All 51 CTests pass in
  static and shared Rust-core builds; C-only passes all 19. The Rust-core
  library also compiles with MBROLA enabled; MBROLA output was not run.
- Cross-target (including the aarch64 double conversion), legacy-async, sonic
  and real-platform audio gates were not run for this stage.

### Wave queue and echo ring stage, 2026-10-07

The synthesis command queue and the echo ring now belong to Rust.
`wave_memory::WaveMemory` holds the queue entries, head and tail, and the echo
ring with its head, tail and amplitude. Its methods are the queue counts and
increments (`WcmdqFree`, `WcmdqUsed`, `WcmdqInc`, `WcmdqIncHead` and the queue
part of `WcmdqStop`) and the ring's take, put and reset. The Rust-core build
exports the process's instance as `espeak_rs_wave_memory`. C no longer
defines `wcmdq`, `wcmdq_head`, `wcmdq_tail`, `echo_buf`, `echo_head`,
`echo_tail` or `echo_amp`.

C writers that are not yet ported still use the queue in place:
`synthesize.c`'s markers, embedded commands, voice changes, sound icons and
phoneme alignment, MBROLA, smoothing, and Klatt's and speechPlayer's
look-ahead. `synthesize.h` maps `wcmdq`, `wcmdq_head` and `wcmdq_tail` to the
exported fields, so their code is unchanged. Klatt's echo now calls the Rust
ring instead of indexing it. `WcmdqStop` keeps its sonic and MBROLA resets in
C.

The wave generator takes its queue and ring as a `RustWaveMemory` pointer
next to the output pointers, embedded values and sample rate. Advancing the
head is no longer a host callback. A borrow of the memory never spans a host
callback, because Klatt, markers and alignment read the queue through C.

The behaviour is C's, apart from two cases where C left memory:

- a queue index outside the queue reads zeros;
- an echo head past the ring (a delay longer than the ring at a high sample
  rate) is not written, and wraps. Klatt now shares the generator's
  handling of this case.

- The `rust_wavegen` oracle now also extracts the legacy queue helpers. Its
  reference side runs on one `RustWaveMemory` through them; the native side
  runs on another through the Rust functions. Queue writes on both sides
  go through each side's `WcmdqInc`. After every call the oracle compares the
  heads, tails, free and used counts, and the whole echo ring with its head,
  tail and amplitude. Queue words are not compared, because they hold
  per-side allocations. 400 programs (222,312 commands, 93,849 fills,
  143,720,820 samples) match, as do the queue counts after every fill.
  Ten injected faults in the queue counts, increments, echo scaling, echo
  storage and echo reset were each detected, by the oracle or by Klatt echo
  WAV parity. Rust unit tests cover queue wrap, the ring's delay, wrap and
  out-of-ring head.
- 321 WAVs are byte-identical between C-only and Rust-core builds. The
  every-variant corpus gives 315 identical WAVs, including the Klatt voices
  with echo (announcer, robosoft, UniRobot). Variants with breath noise depend
  on `espeak_rand`, which `speech.c` seeds from the clock, so two runs started
  in different seconds differ even within the C build. The corpus script now
  retries a difference once.
- On Linux x86-64: 177 all-feature Rust tests, minimal-feature tests, strict
  Clippy, formatting and both generated-table checks pass. All 51 CTests pass in
  static and shared Rust-core builds; C-only passes all 19. The Rust-core
  library also compiles with MBROLA enabled; MBROLA output was not run.
- Cross-target, legacy-async, sonic and real-platform audio gates were not run
  for this stage.

### Queue writers stage, 2026-10-08

No C code writes the command queue any more. `WaveMemory` gained the
writers `synthesize.c` and `synth_mbrola.c` kept: event and phoneme markers
(queued only with more than five entries free), phoneme alignment, sonic
speed, voice changes, MBROLA output and sound icons. Each writes exactly the
words C wrote and leaves the rest of the entry as it was. A voice change
writes the command and the copy's address, not the second word. A phoneme
marker copies 8 name bytes from the third word on (on 32-bit targets, into
the fourth as well). Marker positions are computed as a C int. The voice copy
itself is still allocated in C, because the generator frees it with `free`.

`DoEmbedded` is now `Commands::embedded`. It runs speed changes, sound icons,
named marks, audio markers and generator commands with the Rust pause
writer. The host keeps `SetEmbedded`/`SetSpeed` and the sound-icon table. A
speed change returns the refreshed command settings, because C's later
pauses in the same list use the new pause factors. A first version that kept
the call's settings failed the `<prosody>` CTest. The fix is checked by the
oracle (a reverted refresh is detected). The list's length
(`N_EMBEDDED_LIST`, now in `synthesize.h`) is passed in. Where C would read
past the list, the writer stops with -1 after the commands before it ran.

The command writers now push, read and patch the Rust queue directly; the
settings carry its pointer. Only spectrum lookup, smoothing, frames, speed
changes, sound icons and the length adjustment remain host operations.
Smoothing and Klatt's and speechPlayer's look-ahead still read the queue in
place.

- The `rust_commands` oracle now also extracts the legacy queue writers and
  `DoPhonemeAlignment`. Its native side writes a `RustWaveMemory`. Random
  scripts add markers, phoneme markers, alignment, voice changes (per-side
  copies compared by order and content) and embedded command lists. The lists
  include speed changes that alter the pause factors, sound icons in and past
  the table, empty icons, marks, audio and signed generator commands, with
  random queue heads that cross the five-free-entries limit. 60,000 scripts
  (1,233,130 operations; 3,598,554 queue writes) leave identical queues,
  heads, tails, state and host calls. A list without an end returns -1 at its
  end. 14 injected faults were each detected, including the stale settings.
  Rust unit tests cover each writer's words and the embedded commands.
- 321 WAVs are byte-identical between C-only and Rust-core builds, and the
  every-variant corpus gives 315 identical WAVs in both Rust-core builds.
- On Linux x86-64: 179 all-feature Rust tests, minimal-feature tests, strict
  Clippy, formatting and both generated-table checks pass. All 51 CTests pass
  in static and shared Rust-core builds, including the SSML prosody and audio
  checks; C-only passes all 19. The Rust-core library compiles with MBROLA
  enabled; MBROLA output was not run.
- libsonic is not installed here, so the sonic speed writer was only unit
  tested. Cross-target, legacy-async and real-platform audio gates were not
  run.

### Output buffer and frame pool stage, 2026-10-08

The PCM output buffer and the frame pool now belong to Rust.

`output::Output` keeps C's cursor layout (`start`, `ptr`, `end`) next to the
buffer it owns. `espeak_ng_Initialize` sizes the buffer through
`espeak_rs_output_reserve`, as `realloc` did; a failed allocation still
returns `ENOMEM` and keeps the old buffer. Each fill starts with
`espeak_rs_output_begin`, and termination frees the buffer. In the Rust-core
build `out_ptr`/`out_end` are macros over the exported `espeak_rs_output`,
so Klatt, speechPlayer, MBROLA, the sonic speed-up and `WavegenFill` advance
it in place. `speech.c` reads the buffer through it. `MarkerEvent` measures
event positions from the buffer start. Its pointer parameter was renamed so
the macro cannot reach it, as were two `speech.c` parameters named `outbuf`.
The wave generator takes an output cursor next to its queue and ring, and
writes samples through `Output::write`.

`output::FramePool` is the round-robin pool of modified spectrum frames, the
size of the queue. It allocates as C did (advancing before use, so the first
frame is 1). The process's pool is `espeak_rs_frame_pool`. Its storage
callback, `espeak_rs_frame_pool_storage` (take the next frame, or confirm
that a frame is the pool's), replaces the C pool and callback that the frame
copy, formant transition and smoothing calls used.

- The `rust_wavegen` oracle's native side writes through a Rust output cursor
  over its own buffer. Its 400 programs (143,720,820 samples) still match.
- 321 WAVs are byte-identical between C-only and Rust-core builds, and the
  every-variant corpus gives 315 identical WAVs in both Rust-core builds.
- Event streams, through the public API with a synthesis callback, are
  identical between the C-only and shared Rust-core libraries at 20, 60 and
  200 ms buffers. That is 97 events per run (words, sentences, marks, ends)
  over plain text, SSML marks, breaks and prosody, with ordinary, German and
  Klatt voices.
- Five injected faults (sample byte order, fill size, room, pool membership,
  pool cursor) were each detected by the oracle or WAV parity. Rust unit tests
  cover the buffer's reserve, fill, room, failed allocation and release, and
  the pool's round robin and membership.
- On Linux x86-64: 181 all-feature Rust tests, minimal-feature tests, strict
  Clippy, formatting and both generated-table checks pass. All 51 CTests pass
  in static and shared Rust-core builds; C-only passes all 19.
- The Rust-core library compiles with MBROLA enabled; MBROLA output was not
  run. Cross-target, legacy-async, sonic and real-platform audio gates were
  not run.

### Event list and embedded values stage, 2026-10-08

The event list and the embedded command values now belong to Rust.

`events::EventList` owns the `espeak_EVENT` array, with its count and
capacity, in C's layout. `espeak_ng_Initialize` resizes it as `realloc` did,
keeping what fits; termination frees it. In the Rust-core build
`event_list`, `event_list_ix` and `n_event_list` are macros over the exported
`espeak_rs_events`, so the dispatch to callbacks and the audio event queue
read it in place. `create_events`' parameter of that name was renamed.

`MarkerEvent` now builds settings (message, user data, samples so far, MBROLA
delay, sample rate, names) and calls `EventList::marker`. The marker records
an event unless only the two terminator entries are left. It sums the sample
position in 64 bits as C did (`long` plus a pointer difference), then
narrows it to the event's int. It converts the time as compiled C does. It
writes only the union bytes the event's kind uses: 4 for a number, a pointer
for a mark or audio name, 8 for a phoneme's two values. The rest keep what an
earlier event left. The end-of-list terminators and the async message
terminator are `EventList::terminate` and `terminated_message`.
`RescaleEventSamples` (libsonic) is `EventList::rescale`.

`embedded_value` and `embedded_default` are now Rust statics exported under
their C names. The wave generator already set and read the values in Rust.
The C readers and writers that remain (speed setup, clause punctuation, the
announcement reset, the translator's defaults) address them in place.

The `api` CTest checks the event list's lifecycle. In Rust-core builds it
now reads the Rust list.

- The extracted retained-C oracle (`rust_events`) runs the legacy
  `MarkerEvent` and `RescaleEventSamples` over a C list and the Rust list
  over a copy filled with the same random bytes. 20,000 scripts (458,084
  markers, 76,507 rescales, terminations and resets) leave byte-identical
  lists, stale union bytes included. The scripts mix every event kind, offsets
  at the buffer start, sample counts past 32 bits, MBROLA delays, a full list
  and changing sample rates. Ten injected faults were each detected; one
  needed scripts that keep the same base across operations, which now cover
  it. Rust unit tests cover the markers, termination, the message terminator,
  growing and rescaling.
- Event streams through the public API are identical between the C-only and
  shared Rust-core libraries at 20, 60 and 200 ms buffers (97 events).
- 321 WAVs are byte-identical between C-only and Rust-core builds, and the
  every-variant corpus gives 315 identical WAVs in both Rust-core builds.
- On Linux x86-64: 183 all-feature Rust tests, minimal-feature tests, strict
  Clippy, formatting and both generated-table checks pass. All 52 CTests pass
  in static and shared Rust-core builds; C-only passes all 19. A Rust-core
  build with the asynchronous API and MBROLA enabled passes all of its 53
  CTests. This is the first async run of the port; MBROLA output itself was
  not exercised.
- libsonic is not installed here, so rescaling was checked by the oracle
  only. Cross-target and real-platform audio gates were not run.

### Proactor engine I/O stage, 2026-10-08

The engine now reads its files through loadngo's proactor. Until this stage
the proactor was only an optional feature for the `resident` loader and its
example; the library the C engine links was built without it.

`engine_io::read_file` is now the engine's only file read. With the
`proactor` feature it keeps one process-wide proactor: io_uring on Linux,
or epoll (`EpollPort`) where io_uring cannot be set up, as under some
seccomp filters; kqueue on Apple and BSD; IOCP on Windows. Each file is read
in 256 KiB chunks through `data_io::DataReader`, and the calling thread
drives the proactor (`run_once`) until its chunk completes. That is a
blocking wait on the completion, with no sleep or polling thread. Without
the feature, or if no proactor can be created, it reads with `std::fs`.
`espeak_rs_engine_io_backend` reports which. Bounds keep the owners'
behaviour: directories are refused, and a file longer than the owner's byte
limit fails with `InvalidData` before it is read, the same kind the owners'
admission checks return.

Every engine load point now goes through it:

- phoneme data (`phontab`, `phonindex`, `phondata`, `intonations`) and
  dictionaries;
- MBROLA phoneme translation tables;
- sound icons and the sound-icon configuration;
- voice and variant files, now parsed from memory;
- each file the voice catalogue scan reads. Directory listing and metadata
  stay synchronous, because the proactor has none.

`cmake/rust.cmake` builds the Rust core with `c-abi,proactor` by default.
The new `USE_PROACTOR` option (default ON) turns the proactor off.

- `strace` of a German synthesis with the CMake build shows one
  `io_uring_setup` and 789 `io_uring_enter` calls. The only remaining `read`
  and `pread64` calls are the dynamic loader's and libc's locale table; there
  are no engine reads outside the proactor.
- The new `rust_engine_io` CTest initializes the engine, loads a voice and
  dictionary, synthesizes, and asserts the backend: a proactor in proactor
  builds, `std::fs` with `USE_PROACTOR=OFF`. A unit test reads multi-chunk,
  empty, over-bound, directory and missing files.
- `tools/c_inventory.py` and `docs/REMAINING_PORT.md` give the definitive
  remaining list (see above).
- 321 WAVs are byte-identical between C-only and Rust-core builds, and the
  every-variant corpus gives 315 identical WAVs in both Rust-core builds.
  API event streams are identical at 20 and 200 ms buffers.
- On Linux x86-64: 184 all-feature Rust tests pass, and 171 each without
  default features and with `c-abi` alone (the `std::fs` path). Strict
  Clippy passes for all three feature sets; formatting and both
  generated-table checks pass. All 53 CTests pass in static and shared
  Rust-core builds and in a `USE_PROACTOR=OFF` build. A build with the
  asynchronous API and MBROLA passes all 54. C-only passes all 19.
- The epoll fallback, kqueue and IOCP were not exercised here; io_uring
  worked in this environment.

### Asynchronous command queue on the proactor stage, 2026-10-08

In proactor builds the asynchronous API's command queue (`fifo.c`) runs in
Rust on loadngo's proactor (`async_queue.rs`). One worker thread runs the
proactor loop. Queued commands post a drain job to it, which runs them in
order through the owner's callbacks. C's inactivity wait (three timed
condition waits of 50 ms) is a proactor timer. Stop is a flag the drain
honours, then acknowledges; on a stop, parameter and voice commands still
run and the rest are deleted, as before. Terminate stops the proactor,
joins the worker and deletes what was not run. Adding waits until the
worker has taken the command, by sequence number, so a command that starts
and finishes quickly cannot be missed. A command queued by a running
command (SSML changing a parameter calls `espeak_SetParameter`) is added
from the worker itself without waiting; waiting there deadlocked a first
version. `fifo.c`'s pthread code stays as the legacy oracle.

- The new `rust_async` CTest (async builds) runs three texts, with SSML,
  marks and breaks, through the asynchronous API. It checks the output
  against the synchronous API: the asynchronous API ends a text one sample
  earlier with either queue. Its hash is identical with the proactor queue
  and the legacy C queue, over repeated runs. A cancel of a long text is
  acknowledged and synthesis works afterwards. In retrieval mode neither
  queue interrupts a running text: both check for a stop only between
  commands or while playing audio.
- Rust unit tests cover order, stop with settings kept, a full queue,
  terminate, and a command queued from a running command.
- An async+MBROLA build passes all 55 CTests with the proactor queue and
  with the legacy queue (`USE_PROACTOR=OFF`). The static Rust-core build
  passes its 53.
- `event.c`, which delivers events as audio plays, is still pthreads. It
  needs audio output (pcaudio), which is not installed here.

### Synthesis loop on the proactor stage, 2026-10-08

`Synthesize` now runs its loop as proactor work. The loop body is
`SynthesizeStep`: fill a buffer, deliver it with its events, generate. In
proactor builds `synthesis_loop.rs` runs each pass as a work item on the
calling thread's loadngo proactor, each pass posting the next, and the
thread drives the proactor until the last completes. The calling thread is
the API caller for the synchronous API and the queue's worker for the
asynchronous one. Each thread has its own proactor, so no lock is held
while the user's callbacks run. A nested synthesis from a callback runs its
steps in a plain loop. Other builds loop over the same step, so the output
cannot differ.

- 321 WAVs and the 315-WAV variant corpus are identical to C-only. API events
  are identical at 20, 60 and 200 ms buffers. The asynchronous queue's
  output hash is unchanged.
- Rust unit tests cover the order of steps and a nested loop.
  `rust_engine_io` now also asserts that the steps run on a proactor.
- All CTests pass: 53 in static and shared Rust-core builds, 55 in an
  async+MBROLA build, 19 in C-only.
- Cancellation is still the callback's return value and the queue's stop
  flag. It is not yet a posted completion.

### Audio output on the proactor stage, 2026-10-08

Playback no longer needs pcaudio. With `USE_RUST_AUDIO`, which is on by
default where ALSA is found and on macOS, the Rust core is built with its
`audio` feature. `speech.c` keeps its pcaudio-shaped calls, and
`rust_audio.h` maps them onto `rust/audio_out.rs`:

- 16-bit mono at the voice rate is converted by linear interpolation to the
  device's rate into a bounded queue (a quarter second at 48 kHz).
- loadngo-audio-io's real-time fill callback drains the queue to every
  channel. It emits silence when the queue is empty or locked, so it never
  blocks.
- A writer that finds the queue full, and a drain, block on a completion of
  the sink's own proactor. The fill callback posts that completion when it
  frees room for a waiting writer. A cancel (`espeak_ng_Cancel` on any
  thread) clears the queue and posts one to release the writer at once.
  There are no sleeps.
- A voice rate change only changes the conversion, so the device stays
  open.

- Rust unit tests cover the conversion across calls, channel fill, waiting
  on a full queue, and a cancel releasing a blocked writer. Removing the
  callback's wake hangs the waiting test.
- `rust_audio` plays through ALSA's null device, routed by a test
  `.asoundrc`: the C API directly (20 writes past capacity, drain, flush,
  a rate change, close), synchronous speech, and queued speech with a
  cancel. It skips where no device opens.
- strace shows playback waiting in `io_uring_enter` only.
- All CTests pass: 54 in static and shared Rust-core builds, 56 in an
  async+MBROLA build, 19 in C-only. File output is unchanged, so WAV parity
  is unaffected.
- Not tested: real devices, a time-paced device, CoreAudio linking, and
  Windows, where the sink stays off and pcaudio remains.

### Playback events on proactor timers stage, 2026-10-08

In playback modes, events are delivered as their audio plays. That was
`event.c`'s pthread thread, which delivered each declared event as soon as
it could. In proactor builds `event.c` forwards to
`rust/event_delivery.rs`, and its legacy C stays behind `#ifndef
USE_PROACTOR`.

- Each declared event is copied, including a mark or sound-icon name, and
  given a proactor timer. One thread runs the proactor and calls the owner's
  callback, so callbacks never run on the synthesis thread.
- `speech.c` sets the delay from the Rust sink: the audio still queued for
  the device, less what follows the event in the buffer just written. A
  message-terminated event waits for all queued audio. Without the Rust
  sink the delay is 0, which is the old behaviour.
- Order is kept: an event is never due before the one declared before it,
  and a timer delivers only the due events at the front of the queue. The
  first version delivered everything up to its own event, which let a short
  timer deliver an earlier event too soon. The unit test caught it.
- `event.c`'s rules are kept:
  - A message that starts with another kind of event is preceded by a
    sentence event.
  - Sound-icon events are not delivered.
  - The 1000-event bound returns `ENS_EVENT_BUFFER_FULL`.
  - A clear still reports the message-terminated events it drops. It runs
    on the delivery thread, so it returns only after any callback in
    progress has finished.
  - Delivery starts on the first declare, as `event.c` tolerated.
- Rust unit tests cover order and timing on the delivery thread, sentence
  insertion, name copies, clearing, the bound, and stopping with timers
  outstanding.
- `rust_audio` checks that queued playback delivers the events retrieval
  reports (SSML with a mark), in order, from another thread, followed by
  the message's end.
- All CTests pass: 54 static and shared, 56 async+MBROLA, 19 C-only. A
  non-proactor Rust-core build still compiles the legacy thread.
- Not tested: timing against a device that plays in real time. The null
  device takes audio as fast as it is written.

### Owned native Klatt stage, 2026-10-08

`rust/klatt.rs` replaces every Klatt DSP and frame-generation function in
`klatt.c`: initialization/reset, frame setup and interpolation, resonator
and antiresonator coefficients/history, impulse/natural/two sampled/sawtooth
sources, pitch-synchronous updates, flutter, filtered random noise, PCM
mixing and fades. Each native `Klatt` owns its fixed arrays and histories;
independent instances do not share them. The compatibility shell retains one
instance across reinitialization to preserve the C engine's source history.
speechPlayer (voice source 6) remains a separate C/C++ backend.

The DSP executes inside the existing proactor-driven synthesis buffer step.
It fills only the admitted output capacity, preserves C's parameter
advancement on resume, and adds no scheduler, thread, wait or per-sample
allocation. Queue lookahead is limited to the 170-entry ring. The adapter
reads ordinary frame records through their 44 initialized bytes, and extended
records through 64, avoiding the old unconditional 64-byte frame copy.
Unused next-frame coefficient calculations are omitted: the C kernel never
consumed those increments. No speedup or thermal benefit is claimed.

Evidence on this Mac mini (`dev`, stable Rust; builds use one compiler job):

- `rust_klatt` extracts the whole retained C implementation and compares
  3,000 commands across all five sources, 263,360 fill/resume calls and
  2,875,205 samples in C's defined conversion domain. PCM, full-buffer returns, pitch/mixing cursors, random
  consumption and echo samples/cursors match. Cases include one-sample
  buffers, ordinary/extended frames, queue wrap and lookahead, discontinuous
  formants, 8/16-bit mixed samples, fades and all reset controls. Native unit
  tests cover independent instance histories and zero-capacity admission.
  A protected-page CTest places a 44-byte ordinary frame against inaccessible
  memory and checks that synthesis succeeds without reading its extension.
- CI `37776987887` passed Rust checks on Linux/macOS and both macOS speech
  builds, but exposed undefined C float-to-int conversion on Linux. Repeated
  one-sample resumes advance the C parameters by 64 samples on every call;
  moving endpoints can drive bandwidths negative and make filters unstable.
  UBSan reproduced an out-of-range `-2.45156e+09` conversion. The initial
  synthetic stress corpus therefore did not prove portable defined-C parity.
  The defined-domain corpus now uses stationary endpoints for those repeated
  one-sample cases and asserts that no conversion guard is needed; ordinary
  buffer cases still cover interpolation. `rust_klatt_overshoot` retains the
  entire original moving-endpoint stress corpus: 3,000 commands, 260,480 calls
  and 2,872,325 samples, with 2,853,902 undefined C conversions normalized to
  Rust's specified saturation/NaN-to-zero and wrapping echo addition. The
  extracted oracle keeps every defined C truncation unchanged. Both corpora
  pass UBSan (`undefined,float-cast-overflow`) and the guard-page check.
- All five existing Klatt WAV hashes pass.
- The existing wavegen oracle exposed comparisons of unspecified ABI padding
  (Event bytes 52 and 100 on macOS). `test_wgen_data.h` compares every defined
  `WGEN_DATA` field, preserving all waveform/queue/echo/random assertions;
  the repaired oracle passes. The repair is commit `be961d3c`.
- 200 enabled all-feature Rust tests pass (187 unit, 8 host I/O, 5 resident);
  173 minimal-feature tests pass. Strict all-target/all-feature Clippy,
  formatting, and both generated-table checks pass.
- Rust C-ABI compilation passes for `aarch64-unknown-linux-gnu`,
  `x86_64-pc-windows-msvc`, `aarch64-apple-ios` and
  `aarch64-linux-android`. These are compilation checks, not device runs.
- Before the added overshoot lane, static and shared Rust-core builds pass
  54 runnable CTests each; the
  asynchronous + MBROLA build with speechPlayer disabled passes 56.
  `rust_audio` skips in all three because no audio device opens. CoreAudio
  linking succeeds. The new guard-page test also passes in all three builds.
  The retained C-only build passes all 19 CTests.
  The updated static suite passes all 55 runnable CTests; both Klatt CTests
  are also verified in shared and asynchronous builds. Those suites now
  contain one additional runnable test.
- OS observations before/during/after the build intervals report no recorded
  thermal or performance warning (`pmset -g therm`). That command supplies
  neither a current temperature nor a speech idle/active measurement; the
  final hardware/audio/thermal evidence gates remain open.

The current preprocessed C inventory is refreshed in `REMAINING_PORT.md`.
Klatt now contributes five lines of C callbacks plus four Rust bridges in
that configuration. Translation/numbers, engine ownership and glue,
speechPlayer, MBROLA process I/O, sonic, compilers/CLI and platform front ends
still require porting or integration. Proactor consolidation, cancellation
and real audio/backpressure/thermal behavior also remain completion gates.

## MBROLA transport checkpoint (2026-10-08)

`rust/mbrola_transport.rs` owns the Unix compatibility wrapper's pending
commands and stderr framing. A fixed 256 KiB command ring is allocated once
per owner, reused after partial writes and resets, and freed on close. Whole
command admission precedes I/O: full admission returns zero without sending
a prefix, and the synthesis caller can retry after draining older work. Every
submission or writable dispatch attempts at most one nonblocking write;
`EAGAIN` and `EINTR` retain the complete unwritten suffix. This fixes a C
ordering defect: new direct writes previously bypassed an older queued suffix.
A sleeping child is no longer treated as idle while commands remain queued.

Stderr state preserves incomplete lines across reads, bounds long lines while
discarding their excess through newline, suppresses the existing reset
diagnostics, and retains the latest warning. EOF finishes a partial line.
The old stack parser could lose a fragment at `EAGAIN` and issue a zero-length
read when its line buffer filled. The Rust-core shell now reads at most 256
bytes per dispatch and uses this persistent parser. Fixed WAV header parsing
also runs in Rust, preserving the C signature check and explicitly rejecting
zero or signed-API-unrepresentable sample rates.

This is a transport ownership checkpoint, not a completed process port.
The C process lifecycle, `/proc` idle detection, `poll` backoff, WSL startup
sleep and Windows DLL loading remain. No live MBROLA speech/latency result is
claimed: the binary and voice database are not installed here. C-only builds
retain their original implementation for differential work.

Before implementing the completion driver, the official MBROLA source was
inspected at `274dead162f2826dc38c208fba92efeddb724c33`:

- [The documented flush protocol](https://github.com/numediart/MBROLA/blob/274dead162f2826dc38c208fba92efeddb724c33/README.md#flush-the-output-stream)
  defines `#` and reset-signal behavior but no completion acknowledgement.
- [Standalone processing](https://github.com/numediart/MBROLA/blob/274dead162f2826dc38c208fba92efeddb724c33/Standalone/synth.c)
  flushes stdout after `Synthesis` returns. [The engine](https://github.com/numediart/MBROLA/blob/274dead162f2826dc38c208fba92efeddb724c33/Engine/mbrola.c)
  emits `Input Flush Signal` only when handling `must_flush`, so that message
  cannot establish an ordinary flush boundary or order the separate stdout
  stream. A deadline can bound failure, but must not masquerade as completion.

At this checkpoint the next work was explicit completion/end-of-input and
resume ownership around that actual protocol, bounded command/audio/error
operations with
loadngo, and validate against a real upstream binary/voice. The pinned
proactor's positioned file operations must not be used on anonymous Unix
pipes (`pread`/`pwrite` fail with `ESPIPE`); socketpair-backed stdio can use
its `send`/`recv` completion operations. Windows needs its own DLL lifecycle
or equivalent supported path. Consolidation with the engine's host proactor
and posted cancellation remain required.

Checkpoint evidence (Mac mini, one build/test job):

- Four native transport tests include a 200,000-operation comparison with
  an independent FIFO and assert the original storage address is retained.
  The CTest extracts the production submission functions and verifies
  partial writes, `EAGAIN`, `EINTR`, full admission with zero syscalls, a real
  full nonblocking pipe, fragmented stderr and 100,000 independently decoded
  sample rates. A standalone reproduction extracted from `07c2a656` produces
  `newold` after `old` encounters `EAGAIN` and `new` is accepted before draining;
  the new production path produces `oldnew`. No external MBROLA installation
  is needed for these tests.
- 204 enabled all-feature Rust tests (191 unit, eight host-I/O, five resident)
  and 177 minimal-feature tests pass; strict Clippy, formatting and generated
  table checks pass. The async/MBROLA-on CMake suite passes 58 runnable
  CTests; the shared/MBROLA-on build passes 57. `rust_audio` skips in both
  because no audio device opens. The `mbrola` shell
  test returns success without running speech when its binary/voices are
  absent; it is not live MBROLA evidence.
  The retained C-only/MBROLA-on build also compiles and passes all 20 CTests.
- C-ABI compilation passes for Linux (`aarch64-unknown-linux-gnu`), Windows
  (`x86_64-pc-windows-msvc`), iOS and Android; these are compilation checks
  only. The native transport itself has no OS or proactor dependency.
- Prior checkpoint CI `37778601088` passed all six Linux/macOS jobs. Windows
  was cancelled after three file-reading unit tests exceeded 60 seconds
  (`dictionary_storage`, `engine_io`, `voice_storage`); its runtime coverage
  remains open. This predates the transport changes. The pinned IOCP driver
  caches associations by raw handle value without a close notification;
  handle reuse is a concrete code concern, but its role in that hang still
  requires a Windows reproduction. Do not infer Windows success from a
  cross-compilation or cancellation.
- Coarse OS samples between serialized build intervals report no recorded
  thermal/performance warnings and no CPU power status. This is not a
  temperature reading or proof of idle/active speech thermal safety.

## Native MBROLA completion driver (2026-10-08)

`rust/mbrola_process.rs` provides a safe persistent Unix child session on
the caller's `ProactorHandle<P: IoPort>`. Three socketpairs back its stdio,
using `send`/`recv` rather than positioned file I/O. Each direction has one
reusable loan: 16 KiB commands, 16 KiB audio plus one split-sample byte, and
4 KiB stderr. The existing 256 KiB FIFO admits commands atomically and
consumes only acknowledged write prefixes. Full or busy admission yields
`WouldBlock`; no automatic retry, scheduler, timer thread or polling loop is
created by the session. The pinned loadngo backends use per-send SIGPIPE
suppression, so a closed child reports an I/O error without changing the
embedding process's signal policy.

Audio completions assemble the 44-byte header, strip it in place, retain an
odd sample byte and expose initialized little-endian pairs for the duration
of the callback. A header-only read is progress with an empty PCM slice.
Only an actual zero-byte stdout completion is EOF; invalid headers and
truncated streams terminate decoding with an error. Loan guards recover on
callback unwind. Command loans return before their callback so another send
can be submitted there; audio/error loans return after their callback so
borrowed slices cannot alias a new kernel operation. All three directions
must be driven concurrently to drain stderr and audio backpressure.

`flush()` queues the ordinary `#` protocol command while keeping the same
process/voice. `finish_input()` explicitly shuts down the input after all
queued and in-flight sends finish. It ends the whole input stream; it does
not label a reusable clause flush as EOF. Cancellation uses the host's
operation IDs. Internal callbacks retain socket/buffer state without owning
the child, so dropping the session can terminate it while outstanding reads
still complete. Spawn and final kill/wait are initialization/shutdown work
on the owner; they must run outside paint and completion callbacks, or be
submitted through the host's bounded worker path. Child lifecycle completion
and C-engine resume/output integration remain required. The C wrapper's
`/proc`, `poll`, sleep and Windows DLL implementation are still present.

Evidence on the Mac mini, with one build/test job:

- Four decoder tests cover every two-way split of a 1,070-byte WAV stream,
  repeated read sizes 1 through 103, a maximum-size read following an odd
  carry, all truncated-header lengths, odd tails, invalid signatures and
  invalid rates. The buffer address remains fixed.
- Seven live-child tests cover a full 256 KiB FIFO, atomic rejection, a stderr
  flood over 1 MiB before stdin consumption, persistent ordinary flushes,
  terminal invalid/truncated output, early child exit, cancellation, owner
  drop, callback unwind, and output EOF while input and the process remain
  live. The fixture has blocking child stdio, with no
  timer, sleep or helper threads. Tests block on the host with a single
  ten-second failure deadline. Linux tests require epoll and additionally
  exercise io_uring when the kernel permits it; macOS uses kqueue.
- Both opt-in upstream tests were executed with official MBROLA source
  `274dead162f2826dc38c208fba92efeddb724c33`, built with `make -j1`, and the
  official French `fr4` voice. Native completion-driven PCM matches direct
  file synthesis byte for byte, including two clauses with ordinary flushes
  on one child/voice lifetime. The two-clause oracle independently feeds a
  file containing the same commands to the official binary. The driver
  submits the second clause after real output progress, without treating
  that progress as an acknowledgement. Voice SHA-256:
  `0c0a916fc32382a8b1f252fdc5c269a2c8dcb8b440971b9bd1960c02b7cb0c93`;
  reference WAV SHA-256:
  `231ea156a48b654733464e7849af88f1aa77251f58d2e1c53b69421dc17de0dd`.
  The voice/binary stay in `/private/tmp`; the test fetches no assets and is
  ignored by default. This tests the native stdio API, not C-engine speech,
  real-time device output or end-of-clause acknowledgement.
- 215 enabled Rust tests (195 unit, seven process, eight host-I/O and five
  resident), 177 minimal tests, strict Clippy, formatting and table provenance
  checks pass. The async/MBROLA-on CMake build passes 58 runnable CTests;
  `rust_audio` skips because no device opens. Its `mbrola` shell test still
  skips actual synthesis when system binary/voices are absent; the opt-in
  upstream native test above supplies separate live evidence.
  C-ABI plus proactor library compilation passes for
  Linux, Windows, iOS and Android; mobile runtime remains untested, and the
  Unix child API is excluded on Windows.
- Loadngo's HANDLE-reuse fix and high-resolution IOCP deadline fix are pinned
  at `843ae1de`. Espeak `data_io::DataFile` registers a file once and retains
  registration through outstanding reads. Published espeak `2a1c068c` CI
  `37796614051` passed all seven jobs, including Windows; that resolves the
  earlier file-read hang recorded above. It does not validate a Windows
  MBROLA process implementation.
- Driver checkpoint `efc0b592` CI `37804956383` exposed a race in the
  early-child-exit fixture on Linux: stdout/stderr EOF could precede Python
  closing stdin, and a send could still succeed. The fixture now closes
  stdin before diagnostic EOF; the independent output-EOF regression proves
  a child can remain alive and accept input after closing both outputs.
  Do not use stream EOF as a process-exit notification or substitute a sleep
  in this test. Linux epoll passed the other five process tests on that run;
  Windows Rust tests and both Linux C parity jobs also passed.
- Coarse `pmset -g therm` samples between serialized gates report no recorded
  thermal/performance warnings or CPU power status. Idle/active process CPU,
  wakeups, memory, pacing and OS thermal evidence are still open hardware
  gates; these samples alone establish no thermal-safety result.

Reproduce the opt-in PCM test with existing local assets:

```sh
ESPEAK_MBROLA_PROGRAM=/path/to/mbrola \
ESPEAK_MBROLA_VOICE=/path/to/fr4 \
ESPEAK_MBROLA_PHO=/path/to/bonjour.pho \
ESPEAK_MBROLA_WAV=/path/to/direct-file-reference.wav \
CARGO_BUILD_JOBS=1 cargo test --locked --features proactor -j1 \
  --test mbrola_process upstream_mbrola_pcm_matches_direct_file_synthesis \
  -- --ignored --test-threads=1
```

To run the two-clause reference test, use the same program, voice and phoneme
variables with `upstream_flushes_preserve_one_childs_pcm_history`. That test
creates and removes its own temporary oracle input/output files.

## Native MBROLA generation checkpoint (2026-10-09)

`rust/mbrola_generate.rs` now owns the MBROLA clause generator's decisions,
embedded/word/phoneme cursors and pending output. One reusable owner retains
the command in a fixed 384-byte buffer, its admitted byte offset, duration
and next phoneme index. Neither queue backpressure nor a partial write replays
embedded commands, markers, mapping-prefix effects or acoustic programs.
Only complete admission queues the duration and commits the next index.
The C bridge snapshots the phoneme list once per clause, rather than on
every resume, and destroys the owner with the MBROLA table.

The native driver preserves vowel splits, consonant duration decisions,
the two calls for lengthened fricatives, final pitch newlines and appended
pauses. Pitch remains a resident host operation; its native compatibility
buffer is enlarged from 50 to 128 bytes. Missing neighbouring phoneme
records are accepted where the retained generator does not dereference them.
Overflow, capacity, invalid byte counts and host errors terminate a run;
restart clears pending state without rolling back already issued effects.

The compatibility adapter honours `write_MBR`'s actual byte-count return,
including the Windows DLL's partial writes. A bounded stack copy protects
the retained Rust bytes from that DLL's mutable `char *` ABI. Flush admission
has its own pending phase; it does not acknowledge audio completion. The
legacy 500 ms tail entry remains solely in the C drain callback, where it
still needs replacement during native process/output integration.

Validation for this checkpoint:

- Five Rust regressions cover blocked and partial writes, queue capacity,
  skipped indices, exactly-once effects, flush retries, independent owners,
  file mode, invalid resumes and terminal errors.
- `rust_mbrola_generate` compares 3,000 clauses against the extracted,
  unchanged C generator: **926,798 ordered effects and 628,946 file bytes
  match**. The corpus includes 1,000-entry clauses, embedded speed changes,
  all phoneme types, split mappings, pauses and optional neighbours.
  Separate tests reproduce C's rejected control-1 mapping resuming at the
  wrong phoneme, and verify lossless partial admissions through the C ABI.
- 220 enabled all-feature Rust tests and 182 minimal tests pass. Strict
  Clippy, formatting and generated table checks pass. Linux, Windows MSVC,
  iOS and Android C-ABI/proactor library checks pass; these are compilation
  checks rather than target runtime evidence.
- MBROLA-enabled CMake suites pass 59 runnable async CTests and 58 runnable
  shared CTests. `rust_audio` skips in both because no device opens. The
  unchanged C-only build passes all 20 CTests. Logs use
  `/private/tmp/espeak-mbr-gen-*`.
- A real `mb-fr4 --pho` CLI attempt with local official MBROLA/fr4 assets
  fails identically in C-only and Rust-core builds: the retained wrapper
  reports `/proc is unaccessible` on this Mac. No live engine PHO/PCM parity
  is claimed here. The previous native-session upstream PCM tests remain
  separate evidence; they do not exercise this C startup/output loop.
- The optional async/MBROLA preprocessed inventory now counts 148 C/mixed
  logic lines in `synth_mbrola.c`, down from 250 in the same configuration.
  `mbrowrap.c` remains at 311. The dispatch bridge and process lifecycle,
  Unix waits, Windows DLL, output drain and engine scheduling still need work.
- Builds/tests were serialized. Coarse `pmset -g therm` samples report no
  recorded thermal/performance warnings or CPU power status; representative
  idle/active CPU, wakeup, memory, pacing and thermal measurements remain open.

## Native MBROLA output state checkpoint (2026-10-09)

`rust/mbrola_fill.rs` replaces the C `MbrolaFill` sample counter and output
accounting. One reusable owner computes the duration's sample target, bounds
each read to the caller's initialized PCM storage, scales only acknowledged
little-endian sample pairs and retains the remaining target across resumes.
There is no per-fill allocation, internal read loop, thread or timer.

The native interface distinguishes an unfinished entry, pending I/O, complete
target and end of output. Pending neither consumes samples nor ends the
entry; hosts must resume on a fresh completion. Zero output capacity retains
the entry without calling the reader. Bounds/rate/resume rejection preserves
state; malformed counts, read and scaling errors make a run terminal until
restart. Separate owners have separate cursors.

The C adapter still calls `read_MBR` synchronously. Its zero result retains
the legacy end/idle interpretation. All negative DLL errors are normalized
to failure, because the DLL can return arbitrary codes, including the native
adapter's reserved pending value. Thus the existing wave queue cannot mistake
an old DLL error for an unfinished read. The owner is freed with the MBROLA
table; the reader's blocking calls and idle inference still need replacement.
Passing native pending outcomes into the existing immediate synthesis repost
loop would spin; completion-driven integration must suspend that loop and
resume it from I/O instead.

Validation for this checkpoint:

- Three native regressions check bounded/short reads, exact scaling and guard
  bytes, pending versus end, zero capacity/target, resume rejection, terminal
  failures and independent owners.
- `rust_mbrola_fill` compares 12,000 output entries against the extracted
  retained C function: **159,981 calls and 2,704,789 scaled samples match**,
  including sample request sizes, byte cursors and untouched output tails.
  Separate C-ABI checks reproduce C ending a zero-capacity entry and confirm
  native preservation, pending resume, explicit end and DLL error normalization.
- Both opt-in official MBROLA tests now route each decoded completion through
  the native output state using one reusable 256-byte destination. Single-input
  and two-clause PCM still exactly match their independent file references;
  two clauses retain one child. The test-only upper sample budget comes from
  the reference length, not a runtime prediction. Real stdout EOF alone ends
  the state; an ordinary flush or a lack of audio does not. This is native
  completion/consumer evidence, not live C-engine process integration.
- 223 enabled all-feature Rust tests, 185 minimal tests, strict Clippy,
  formatting and generated table checks pass. C-ABI/proactor library
  compilation passes for Linux, Windows MSVC, iOS and Android.
- All local CTests pass: 60 runnable async/MBROLA-on, 59 runnable shared/
  MBROLA-on, and 20 C-only. `rust_audio` skips in both Rust-core suites because
  no audio device opens. Logs use `/private/tmp/espeak-mbr-fill-*`.
- The optional async/MBROLA preprocessed inventory counts 152 C/mixed logic
  lines in `synth_mbrola.c` and 311 in `mbrowrap.c`. This scanner excludes the
  former cursor's short bridge from its logic total; the new four-line reader
  adapter raises that count despite moving the cursor itself into Rust.
- Builds/tests remained serialized. Coarse macOS samples report no recorded
  thermal/performance warnings or CPU power status; representative runtime
  CPU, wakeup, memory, pacing and thermal measurements remain open.

Native process/output integration, ordinary-flush sequencing, lifecycle and
Windows backend work remain. The live C CLI still depends on the retained
wrapper, whose `/proc` requirement failed on this Mac in the preceding slice.

## Native synthesis suspension checkpoint (2026-10-09)

`rust/synthesis_loop.rs` now exposes `run_on` for a caller-owned loadngo
completion port. A bounded pass returns Continue only when local work can
progress, Pending when it needs a host completion, or Done. Pending posts
no synthesis work. The host publishes its result and calls the run's `Wake`;
early and duplicate wakes coalesce. Cancellation fences further passes and
posts a wake-only completion, without stopping the shared port or cancelling
unrelated host I/O. No thread, timer or idle polling drives this scheduler.

Queue admission requires the observed phase and pass ticket. A publisher
delayed after observing Waiting cannot replace a running pass or wake a
later wait. Queued work and wake capabilities retain weak control references;
normal return, cancellation, post/poll failure and callback unwind remove
the callback before its synchronous owner can release the context. The host
must still own, cancel and drain I/O that outlives a run.

The thread-local compatibility runner shares its port with nested calls.
It no longer falls back to a plain callback loop after a failed/stopped port.
The C bridge returns -1 on failure and `Synthesize` stops the clause and
returns `ENS_SPEECH_STOPPED`. The legacy C step remains locally-ready/done;
its pending process reads and cancellation flag are not yet bound to the
native wake interface. This supersedes the nested-loop/fallback behavior
described in the October 8 synthesis-loop checkpoint.

Validation for this checkpoint:

- Nine scheduler tests cover ordered/nested passes, early/duplicate wakes,
  delayed wake admission during a running pass and a later wait, pending
  cancellation, stopped ports, failed posts/polls and callback unwind.
  Fault injection deliberately leaves queued jobs behind; later runs prove
  that those jobs cannot replay a released callback/context.
- An actual registered-file test suspends and resumes three sequential
  opens on one platform port, retaining one four-byte read buffer and the
  caller thread. The pinned loadngo `dev` revision `843ae1de` includes
  `e421f5ea`'s tagged Windows registrations and reused-HANDLE repair.
- Native Unix MBROLA fixture and official upstream tests now drive the
  same Pending/wake runner instead of their separate completion loop.
  Persistent completion state binds to each run's current waiter, so I/O
  submitted before an ordinary flush can wake a subsequent run. Both
  official PCM comparisons pass: single input and two clauses on one child,
  through the reusable native output state. Their ten-second watchdog is
  test-only; it does not infer EOF or flush completion. These remain native
  session tests, not live C-engine process integration.
- 231 enabled all-feature Rust tests, 185 minimal tests, strict Clippy,
  formatting and generated-table checks pass. C-ABI/proactor library checks
  pass for Linux, Windows MSVC, iOS and Android; runtime coverage comes from
  CI and actual target execution, not these cross checks.
- Local CTests pass: 60 runnable async/MBROLA-on, 59 runnable shared/
  MBROLA-on and 20 C-only, including retained-C waveform/API parity.
  `rust_audio` skips in both Rust-core suites because no audio device opens.
  Final logs use `/private/tmp/espeak-synthesis-wake-final-*`.
- The refreshed main inventory counts 331 C/mixed logic lines in `speech.c`
  and 9,787 overall. Its three-line increase is the explicit failure branch.
- Builds/tests remained serialized. Coarse macOS samples report no recorded
  thermal/performance warnings or CPU power status; representative runtime
  CPU, wakeup, memory, pacing and thermal measurements remain open.

Native C-engine process/output integration, ordinary-flush sequencing,
lifecycle, Windows backend and the remaining full-engine port stay open.

## Async synthesis cancellation checkpoint (2026-10-09)

The native command queue now binds each running command to a reusable
cancellation scope. `fifo_stop` and queue termination request cancellation
on the command's active synthesis port, rather than only setting a flag or
stopping the outer queue port. A Pending pass wakes and returns Interrupted;
a running bounded pass is fenced before the next one. The C synthesis
bridge converts that result to `ENS_SPEECH_STOPPED` and stops the clause.
Stopping threads never invoke the C synthesis callback or touch its context.

The innermost nested runner registers its weak wake capability; dropping its
binding restores and cancels the outer runner if a stop was requested.
Scope registration and thread-local activation are restored on return or
unwind. Stop acknowledgement resets the scope before admitting later speech;
termination remains requested even if it releases a concurrent stop waiter
before the command registers its synthesis capability.
Outstanding native I/O still belongs to its host and requires cancellation
and draining; the C process reader remains blocking.

Review also found that the native queue's admission wait had changed the
asynchronous API: it waited for the last command of a transaction to be
taken. For text plus terminated-message transactions, that meant waiting
until the text had finished, preventing the submitting caller from cancelling
it. The retained `fifo.c` waits only for an idle worker to start. Native
admission now follows that behavior, retaining atomic pair admission and
ordered execution. The C async regression now asserts that cancelling a
long text produces fewer samples than the short parity corpus; its previous
assertions allowed the complete long text to finish.

Validation includes 235 enabled all-feature Rust tests, 185 minimal tests,
strict Clippy and formatting. A completion signals the new queue test only
after its first pass enters Pending; stop and terminate both release it
without its test-only watchdog firing, the paired command is deleted, and
the next command after stop completes normally. Separate tests fence a
previously requested first pass, propagate cancellation through nested
runs, and order stop/termination before the first wake registration.

The fresh C async test records 148,200 synchronous and 148,199 asynchronous
samples (hash `311b5b6a8edf234e`, 36 events), preserving the earlier output.
Immediate cancellation of its long text now stops before the first audio
buffer (zero samples in this run), and subsequent speech succeeds. The
full async suite passes 60 runnable CTests plus the audio-device skip; after
the termination-order fix, the async regression was rebuilt and rerun.
C-ABI/proactor library checks pass for Linux, Windows MSVC, iOS and Android;
these are compilation checks, not full platform runtime coverage. Logs use
`/private/tmp/espeak-cancel-binding-*`.
The final shared/MBROLA-on suite passes 59 runnable CTests plus its
audio-device skip; all 20 retained C-only tests pass. Builds and tests remained serialized; coarse macOS
thermal samples report no recorded warnings, without establishing runtime
thermal safety.

The full engine port remains incomplete. C pending-process integration,
ordinary-flush acknowledgement, process lifecycle and platform backends
remain, along with the C API's polling Synchronize and other inventory items.

## Completion-driven synchronization checkpoint (2026-10-09)

In proactor builds, `espeak_ng_Synchronize` now calls the native queue's
completion wait instead of checking `espeak_IsPlaying` every 20 ms.
`Queue::synchronize_on` accepts a caller-owned port; the C bridge uses its
calling thread's cached port. Idle calls return without posting work. Busy
calls reserve one of 16 fixed waiter slots, return Pending and receive a
wake when the queue becomes idle. There is no synchronization timer, new
thread or polling loop. A resumed pass rechecks state under the queue mutex,
so commands admitted before it resumes cannot be skipped.

Each waiter retains a weak runner capability, allocated once, and releases
its reservation after callback fencing on return, post/poll failure or
unwind. Queue termination and worker exit interrupt waiters; a callback
cannot synchronize on its own running queue and receives WouldBlock
(`EINVAL` through the C API) promptly. The earlier C error snapshot/reset
behavior is preserved on successful synchronization. The retained C-only
and non-proactor modes keep their reference wait for differential testing.

Worker exit now wakes admission/stop waiters and marks the queue terminal,
including unexpected port stop or command unwind. Commands are individually
guarded for deletion; failed admission leaves its commands caller-owned.
Queued drain jobs and inactivity notices capture weak queue references,
avoiding a port retaining its own context through queued work. Stop includes
commands admitted by kept settings before publishing idle. A new unwind
regression first reproduced a drained batch losing its unstarted commands;
popping one command at a time preserves ownership and removes that batch
allocation. These changes are required for synchronization to report actual
quiescence and to release failed waits.

Validation for this checkpoint:

- 242 enabled all-feature Rust tests and 185 minimal tests pass, with strict
  Clippy, formatting and generated-table checks. The new tests cover early
  idle notification followed by new admission, post/poll failure and slot
  reuse, stop, explicit termination, worker-port stop, command/settings
  unwind, all 16 concurrent waiters and rejection of a seventeenth,
  immediate idle return, and release of the queue context after shutdown.
- The C async regression rejects synchronization from its running callback,
  retains the `311b5b6a8edf234e` parity hash (148,199 asynchronous samples,
  36 events), stops its long cancellation text before the first buffer in
  the recorded run, and synthesizes again afterwards.
- Full static async/MBROLA-on and shared async/MBROLA-on suites each pass
  60 runnable CTests plus the audio-device skip; all 20 C-only tests pass.
  After the final stop/settings ownership repair, both async builds were
  rebuilt and their C async regressions rerun. Logs use
  `/private/tmp/espeak-sync-*`, including the red ownership regression.
- C-ABI/proactor library checks pass for Linux, Windows MSVC, iOS and Android.
  The existing shared CI lanes now enable the C async API on Linux and macOS;
  static CI lanes retain async off. Cross checks alone are compilation
  evidence; target runtime evidence comes from those CI jobs and actual runs.
- The refreshed main preprocessed inventory is unchanged at 9,787 C/mixed
  library/CLI logic lines. The queue/bridge changes are in its optional async
  configuration. Build/test execution remained serialized; coarse macOS
  thermal samples report no recorded warnings, without establishing runtime
  thermal safety.

The full port remains incomplete: native C-engine pending I/O, outstanding
I/O cancellation/drain, MBROLA ordinary-flush sequencing and process lifecycle,
the remaining text/tooling/platform code and real-platform runtime gates stay
open.

## Audio cancellation integration checkpoint (2026-10-09)

Async command stop and termination now interrupt native audio writes/drains
waiting on the sink's separate proactor. Previously cancelling the synthesis
runner only posted to the runner's port: a full sink could keep its callback
blocked, preventing stop acknowledgement and engine shutdown.

Each write/drain binds the sink's existing Arc owner to the command's
cancellation scope, without a new allocation, thread or timer. Capturing its
cancel generation before registration, refusing already requested scopes
and checking generation while admitting samples prevents cancellation just
before a wait from becoming a new successful write. Interruption and guard
removal are serialized, so a late stop cannot flush a later operation using
the same sink. Nested guards restore their outer owner and propagate a
pending stop; unwind removes the registration. No borrowed device/engine
context reaches the stopping thread. Callers outside an async command scope
retain the explicit sink-canceller contract.

Tests exercise actual command queues and nested synthesis runners with a
full sink and no device callback: stop and termination release both writes
and drains, and a command admitted after stop acknowledgement writes again.
Drains also run after the synthesis runner has finished, matching the C
engine's final drain with no runner wake left to cancel.
Other regressions cover pre-registration cancellation, cancellation between
registration and the first write, empty operations, nested host waits,
unwind, and owner release. Temporarily removing the write/drain bindings
reproduced a requested cancellation accepting audio (`Ok` rather than
`Cancelled`) and a stalled writer losing stop until the regression's emergency
deadline interrupted it; the restored implementation passes. That test-only
deadline bounds a broken wait and fails the test if used. Logs use
`/private/tmp/espeak-audio-cancel-*`.

Validation: 245 enabled Rust tests (224 unit, 7 Unix process, 9 host-I/O,
5 resident-I/O) and 185 minimal tests pass, along with strict Clippy,
formatting and generated-table checks. Static/shared async/MBROLA-on
builds each pass 60 runnable CTests plus the unavailable-audio-device
skip; all 20 C-only tests pass. The C async hash remains
`311b5b6a8edf234e` (148,199 asynchronous samples, 36 events). Library
C-ABI/proactor checks pass for Linux, Windows MSVC, iOS and Android;
these checks prove compilation only. Final Rust tests include the bounded
regression and the post-runner drain variant; the C suites cover the same
production implementation. Builds/tests were serialized. Coarse macOS
thermal samples reported no recorded warnings; this is not real-device
thermal-safety evidence.

The full port remains incomplete. This binds native audio back-pressure;
pending C-engine process/file I/O, event queue back-pressure, MBROLA ordinary
flush sequencing, process lifecycle, synchronous cancellation, remaining C
text/tooling/platform code and real-device/thermal validation remain open.

## Completion-driven event admission checkpoint (2026-10-09)

The C proactor path now declares playback/terminated-message events through
native capacity waits, removing both 10 ms `speech.c` admission retry loops
from that configuration. Non-proactor C builds retain the reference behavior.
`Delivery::declare_on` accepts a caller-owned port; busy calls retain one
owned event/name and suspend until capacity changes. A wake rechecks the
queue, including another producer taking the newly freed space. Clearing
invalidates older ordinary admission; async command cancellation interrupts
those waits. Mandatory message-terminated admissions are cleanup obligations:
command cancellation does not discard them, and clear frees their capacity
so the completion that releases caller user data still arrives. Delivery
termination/worker failure release either kind with an error. Sixteen admission reservations and
four independent clear reservations keep cleanup available under saturation.
Fast admission allocates no wait context; each Pending wait allocates once.

Native clear is a callback barrier on the caller's proactor. A worker-exit
guard releases accepted clear jobs that never run. Post/poll failure and
unwind release reservations after callback fencing; stale jobs cannot clear
reused owner state. The event deque retains its preallocated capacity when
cleared. Each delivery pass is bounded to the event capacity, and queued
delivery/clear jobs capture weak owners to avoid their own port retaining
its context.

The C adapter clones its owned delivery handle while holding the global
mutex, then releases the mutex before admission, clear or callback work.
Callback handlers are taken/restored by generation, with no handler lock
held during owner code. A callback can replace its handler, clear pending
events or terminate its worker without joining itself; full admission from
that worker returns WouldBlock (`EINVAL` at the C boundary). The old C
wall-clock helpers compile only with the retained pthread FIFO. Normal delivery
preserves event ordering, synthetic sentence notifications and copied names.
Clear still reports dropped terminated messages; reentrant clear retains
its earlier rule of dropping pending notifications without recursive calls.

Deterministic regressions exercise repeated capacity wakes, owned marks,
clear/command cancellation and reuse, mandatory completion delivery across
both, caller unwind, failed posts/polls with stale jobs,
worker unwind with an accepted clear, all 16 actual pending callers,
seventeenth-call rejection, cleanup at saturation, and callback reentry,
self-wait refusal and owner release. Existing event tests now wait on
notification channels instead of yielding in a loop. Restoring the old
global-lock-held adapter reproduces the new C reentry regression failure;
the test remains bounded even with that defect. Logs use
`/private/tmp/espeak-event-wait-*`.

Validation: 255 enabled Rust tests (234 unit, 7 Unix process, 9 host-I/O,
5 resident-I/O) and 185 minimal tests pass, along with strict Clippy,
formatting and generated-table checks. Static/shared async/MBROLA-on builds
each pass 60 runnable CTests plus the unavailable-audio-device skip. All
20 retained-C core tests and 19 retained-C async/MBROLA-off tests pass.
Linux, Windows MSVC, iOS and Android C-ABI/proactor library checks pass;
these establish compilation only. C async parity remains
`311b5b6a8edf234e` (148,199 asynchronous samples, 36 events).
Native async `event.c` objects contain only the five forwarding functions,
without the legacy clock helpers; the native `speech.c` object references
the waiting admission adapter without `usleep`. The retained-C async object
still contains its clock helpers. These symbol checks cover those objects
and configurations, rather than every engine path. Temporarily binding
mandatory message admission to command cancellation reproduces a lost
completion; the restored shielding passes its regression. Builds/tests were
serialized. Coarse macOS thermal samples reported no recorded warnings;
real-device thermal-safety validation remains open.

The full port remains incomplete: C-engine pending process/file I/O, ordinary
MBROLA flush sequencing, native process/output and engine lifecycle, timing
oracle retirement, remaining text/tooling/platform code and real-device/
thermal validation remain open. Cancelling/decreasing outstanding playback
timers across repeated clears also needs an owned timer-cancellation contract;
the current proactor defer API does not return a cancellation ticket.
Command deletion is another lifecycle boundary: `delete_espeak_command`
still emits pending terminated-message callbacks, while the native queue's
deletion guard runs outside its processing cancellation scope. Cleanup
notification ownership and cancellation need integration without losing the
completion notification that releases a caller's user data.

## Owned asynchronous commands (2026-10-09)

Production Rust-core async builds now use `rust/async_command.rs` and its
C adapter for all nine command kinds: text, mark, key, character, parameter,
punctuation, voice name/specification and message completion. The C queue
keeps a prefix view for kind/state/identifier access. Rust owns payload
copies, dispatches through borrowed engine callbacks and releases commands.
Pending completion deletion marks the command processed before calling
the host, preserving caller user-data cleanup during stop/discard. Identifier
admission is atomic and retains unsigned wraparound; unsuccessful construction
does not consume an identifier. Engine callbacks and the surrounding API
lifecycle still need migration.

Text captures use wide-aligned storage with initialized termination beyond
the supplied byte extent, including mark synthesis and non-terminated input.
A four-slot pool reuses buffers outside its lock and retains at most 8 MiB;
individual captures above 2 MiB remain valid but are not cached. Names,
punctuation and voice selectors are copied before caller storage expires.
No command lock is held during callbacks; they may construct/dispose an
independent command. Command admission adds no production worker or timer.

The retained C independently compares 54,000 pairs across all nine kinds,
all three states, processing/discard, changed caller buffers, optional voice
strings, scalar arguments, callback reentry and pending cleanup. Native-only
boundary checks cover invalid/null input, oversized byte extents, initialized
byte/wide terminators and null process/delete. Rust regressions cover pending
completion exactly once, identifier wrap/concurrent producers, alignment/
termination and bounded buffer reuse. Preprocessing `8bbd05e2` with the current
async build flags counts 265 C command-logic lines; the new build has 11
forwarding bridges and no C/mixed command logic. Its command object references
the native create/process/delete adapters without malloc/free/strdup. The
complete async/MBROLA-on inventory now counts 10,354 C/mixed lines in that
configuration; the full text/tooling/platform port remains open.

Validation: 259 enabled Rust tests (238 unit, 7 Unix process, 9 host-I/O,
5 resident-I/O), 189 minimal tests, strict Clippy, formatting and generated
tables pass. Both native static/shared async suites pass 61 runnable CTests
plus the unavailable-audio-device skip. After the final oversized-pool guard,
their command and end-to-end async tests were rebuilt and rerun; the parity
hash remains `311b5b6a8edf234e`. The command oracle also passes in a fresh
Rust-core/proactor-off build. Rebuilt retained-C core/async builds pass 20/19
tests. Linux, Windows MSVC, iOS and Android library C-ABI/proactor checks
pass (compilation only). Logs use `/private/tmp/espeak-command-*`.
All local builds/tests were serialized; coarse macOS thermal samples reported
no recorded warnings. Real-device/runtime/thermal gates remain open.

## Native legacy API and diagnostics (2026-10-09)

Rust now owns the complete legacy initialization/compiler control and status
conversion (`rust/legacy_api.rs`), status selection and diagnostic formatting
(`rust/status.rs`), and error-context allocation/replacement/reclamation through
their C adapters. The legacy initialization retains output-mode mapping,
ignored output-initialization errors, event flags, error cleanup order and
`DONT_EXIT`; the production C callback still performs `exit(1)` when required.
Underlying engine initialization/compiler callbacks and lifecycle remain C.
This CPU control adds no worker, timer or polling path.

An owned context keeps its stable C prefix and a fallibly copied, terminated
byte name. Copying precedes replacement, including an input alias of the old
name; allocation failure preserves existing state. Clear relinquishes its
slot before reclaiming the owner and is idempotent. Filename bytes need not
be UTF-8. Diagnostics emit bounded stack fragments without allocating a
formatted string; CRT adapters hold the stream lock across the entire record
and supply platform errno messages. No Rust/global context lock is held across
callbacks. Printing borrows the context/name, which the caller must keep alive
and unchanged until it returns.

The original C branches are extracted and compiled independently under renamed
symbols. The oracle compares 280 initialization/compiler cases, 124,300 message
buffers including truncation and trailing sentinels, and 300 context replacements
with exact file/version diagnostic bytes. Native-only checks cover old-name
aliasing, null context slots, rejected null names, repeated clear and callback
reentry. A deliberately returning test exit callback is fenced after failure;
the retained-C comparison uses `DONT_EXIT` for failing initialization. Checks
and their calls execute even with `NDEBUG` defined. The status oracle retains
the already-ported shared bounded-copy helper and the same CRT errno adapter.

Identical preprocessing flags reduce the two modules' C/mixed logic from
176 lines to eight CRT adapter lines, with 14 legacy API and five error bridges.
The current async/MBROLA-on scan counts 10,186 C/mixed lines; the sync appendix
counts 9,642. The prior sync headline of 9,787 was stale: its appendix actually
summed to 9,810, and only these two rows changed in this checkpoint.

Validation: 263 enabled Rust tests (242 unit, 7 Unix process, 9 host-I/O,
5 resident-I/O), 193 minimal tests, strict Clippy, formatting and generated
tables pass. Native static/shared async builds pass 62 runnable CTests each;
the synchronous build passes 59. Each skips the unavailable audio device.
Final rebuilt `NDEBUG` oracles pass in all three, with async parity hash
`311b5b6a8edf234e` unchanged. The oracle and production API tests also pass
with the proactor disabled. Rebuilt retained-C core/async suites pass 20/19.
Linux, Windows MSVC, iOS and Android library C-ABI/proactor checks pass
(compilation only); the native CRT/engine adapter has local macOS runtime
coverage, with Linux/macOS CI lanes configured and no Windows engine lane. Logs use
`/private/tmp/espeak-api-*`. Local builds/tests were serialized, and coarse
macOS thermal samples reported no recorded warnings. Full engine/process
integration, remaining text/tooling/platform code and real-device/thermal
validation remain open.

## Native engine lifecycle control (2026-10-09)

`rust/engine_lifecycle.rs` now controls engine initialization, output setup,
cancellation, completion-based synchronization and teardown. Mode, output/voice
rates and error state use native atomic scalars; C synthesis accesses them
through typed getters/stores. The admitted audio handle also belongs to this
lifecycle state. Initialization/output/lifecycle admission remains serialized;
atomics do not turn the remaining C engine resources into independent thread-safe
instances. Callbacks and their resource slots must remain valid for their
admitted operations and use the same host for the handle's lifetime.

The initializer preserves locale fallback, phoneme-load failure fencing,
initial sample rate, voice/synthesis/names resets, parameter defaults and
overrides, phoneme flags and the platform seed. Output sizing preserves minimum
60 ms and the extra sample at an exact millisecond boundary. Checked arithmetic
rejects the old undefined overflow domain before either allocation; allocation
failure still retains earlier admitted resources, with output reservation
preceding event reservation. Cancel restores each saved parameter freshly
after queue stop/event clear, without retaining array borrows across callbacks.
Synchronization preserves the prior error snapshot, resets it after a successful
completion wait and preserves it on wait refusal/failure. The optional
pthread-only configuration keeps its existing C synchronization loop; no Rust
poll/sleep fallback is introduced.

Teardown fences queue/event workers before releasing resources, takes audio
before close/destroy callbacks and relinquishes translator/decoder slots before
their destructors. An audio handle admitted in playback remains owned after
switching to retrieval. The original controller skips that handle at termination;
the native controller destroys it once regardless of the final mode. Primitive
engine calls and most resources still have C owners; this is lifecycle control
and audio admission cleanup, not the full owned-engine/process integration.

The retained production controller bodies are independently extracted, with
native resource-release branches and deterministic host primitives. Under
`NDEBUG`, the oracle compares 20 locale/load/order/state initializations,
3,564 mode/rate/buffer/allocation cases, four cancellation/teardown cases,
and successful/failed synchronization. It separately confirms the original
mode-switch leak and native cleanup, repeated termination, detached destruction
slots, arbitrary device bytes and overflow refusal. Native Rust tests cover
the same lifecycle boundaries and fresh saved reads across parameter callbacks.
Identical async preprocessing flags count 89 lines in the five former controller
bodies; production now contains five forwarding bridges. The async scanner
counts 10,080 C/mixed lines and the synchronous appendix 9,555. Short remaining
C control that calls a native state getter may be classified as a bridge; that
classification alone does not establish a fully native controller.

Validation: 267 enabled Rust tests (246 unit, 7 Unix process, 9 host-I/O,
5 resident-I/O), 197 minimal tests, strict Clippy, formatting and generated
tables pass. Both native static/shared async suites pass 63 runnable CTests,
and synchronous passes 60, each with an unavailable-audio-device skip.
Retained-C core/async suites pass 20/19. Linux, Windows MSVC, iOS and Android
library C-ABI/proactor checks pass (compilation only). The proactor-off API and
lifecycle oracle pass. Its extra `rust_async` cancellation check fails: all
25,309,908 long-text samples complete before cancellation. Compiling the
previous `c2b816db` speech controller with identical flags and linking it with
the same other objects/library reproduces exactly that failure. This isolates
the new controller change; it is not a full prior-checkout validation. Default
proactor async cancellation passes and its hash remains `311b5b6a8edf234e`.
The legacy pthread backend still needs retirement/integration in the full port.

Logs use `/private/tmp/espeak-engine-*`. Local gates were serialized; coarse
macOS thermal samples reported no recorded warnings. No real audio device
or full platform/idle-active thermal result is established. Path discovery,
pending C synthesis/process I/O, remaining engine/resource ownership, text,
tooling and platform migration remain open.

## Native synthesis request control (2026-10-09)

`rust/engine_request.rs` replaces eight request controllers in `speech.c`:
`sync_espeak_Synth`, `sync_espeak_Synth_Mark`, `espeak_ng_Synthesize`,
`espeak_ng_SynthesizeMark`, `espeak_ng_SpeakKeyName`, `espeak_ng_SpeakCharacter`,
`espeak_ng_SetParameter` and `espeak_ng_SetPunctuationList`. C now supplies
argument records and typed primitives. Async inputs go straight to the existing
native owned command factory, with its bounded text-buffer reuse; queue and audio
waits retain their proactor implementation. This adds no thread, timer or poll.

Text/mark admission initializes and publishes the caller's identifier before
callbacks, captures the input and mandatory message, and submits them together.
Queue success transfers ownership; failures return owned commands for exactly
one cleanup. A failed input capture still attempts message creation, preserving
the API's allocation/error flow. Single-command admission preserves the queue's
null-command rejection. The controller does not rewrite an identifier modified
by a queue callback after transfer.

Synchronous text initializes input before copying current parameters, establishes
identity, uses the post-initialization skip slots, and sets the end position.
It flushes stopped playback and drains other playback statuses, reading the mode
after synthesis. Audio errors remain diagnostic and do not replace synthesis
status. Mark preparation preserves its different behavior: bounded 50-byte
zero-padded marker storage, SSML enabled for synthesis, no parameter snapshot
or added drain. Raw slot copies finish before host calls; no mutable Rust borrow
of shared engine state crosses a callback. The remaining resources still need
serialized engine admission and full owned-engine integration.

The independent CTest extracts the original eight production bodies and compiles
them in all four async/audio capability combinations. With checks executing under
`NDEBUG`, it passes 3,456 request admission and 11,520 synchronous preparation
comparisons: allocation and queue failures, null identifiers and punctuation,
unknown modes/positions, unsigned position limits, marker truncation/padding,
callback observation of published identifiers, queue-side disposal and caller
slot mutation, and synthesis/audio callbacks changing the mode/handle. Four native
Rust regressions cover ownership return, synchronous bypass, post-init skips,
stopped-text flush and mark-specific behavior.

Validation: 271 enabled Rust tests (250 unit, 7 Unix process, 9 host-I/O,
5 resident), 201 minimal tests, strict all-target/all-feature Clippy, format and
generated tables/languages. Native static/shared async suites pass 64 runnable
CTests each, synchronous 61, with unavailable-audio-device skips. Retained-C
core/async suites pass 20/19. The proactor-off API/request/lifecycle/owned-command
tests pass; its previously documented pthread cancellation defect remains open.
Linux, Windows MSVC, iOS and Android library C-ABI/proactor checks pass; these are
compilation checks, not full target-engine runtime evidence.

Identical preprocessing flags for `fe97b9ca` and the new source reduce `speech.c`
from 301 to 212 C/mixed lines in async/MBROLA builds and 244 to 197 in sync builds.
Full current inventories retain 9,991 and 9,508 C/mixed lines respectively.
Several former request bodies were already classified as bridges because of
native state getters; the counts do not describe all remaining C control or C++
class methods. Logs use `/private/tmp/espeak-request-*`. Local builds/tests were
serialized and coarse thermal samples reported no recorded warnings. Real-device
audio, full platform and idle/active thermal validation remain open, along with
pending C process I/O, engine/resource ownership, remaining frontend/tooling
control and legacy backend retirement. The full port goal remains active.

## Native synthesis startup and passes (2026-10-09)

`rust/engine_driver.rs` replaces the proactor-path `Synthesize` and
`SynthesizeStep` controllers. The C shell now projects configured default voice,
translator encoding and current generation through three primitives, and forwards
startup to the native driver. The Rust adapter admits an immutable callback table
and serialized output/event, flag, identity, translator and decoder slots.

Startup preserves missing-buffer rejection before flag/count writes, conditional
voice/decoder setup, decode error propagation, initial clause setup and the current
completion runner. Its per-run stack context remains live until that runner fences
all queued callbacks. Refusal/cancellation stops the clause and returns stopped
speech without any local replay loop. No production thread, timer, sleep or poll
is introduced. The proactor-off native build keeps its original C driver/loop;
identical preprocessing flags confirm all of its function inventories are
unchanged. Retiring that backend remains part of the full port.

A pass starts and fills the reused output buffer, accounts samples and terminates
the generated event prefix, delivers playback or retrieval, and waits for both
generation and the wave queue to finish before advancing a clause. End delivery
keeps its different null-buffer/event convention. Playback errors, positive stop
results, retrieval aborts and end-dispatch results preserve their distinct status
and cleanup behavior, including the legacy ignored positive end-playback result
and ignored `WavegenFill` result. Mode, callback and identity slots are read
freshly after owner calls; no mutable Rust resource/context borrow survives a
callback. Cursor lengths preserve defined odd-byte truncation, and checked cursor
bounds/native-long addition reject invalid overflow before sample publication,
event termination or delivery. This is control over hybrid resources, not full
owned-engine/process integration. Passes still report locally-ready/done; native
pending process I/O remains to be connected.

The CTest extracts the original two production bodies and executes its checks
under `NDEBUG`. It passes 10,368 synthesis-pass and 4,608 startup comparisons:
missing buffers/resources, allocation and voice/decode failures, flags, event
counts, odd/even lengths, delivery/queue/generation/clause combinations, runner
refusal before/after work, and callbacks changing output start, mode, callback
slot and identity. Separate native checks reject a sample-counter overflow,
out-of-buffer cursor and missing runner without delivery or replay. Three safe
Rust regressions exercise error fencing, clause boundaries, status distinctions
and runner refusal.

Validation: 274 enabled Rust tests (253 unit, 7 Unix process, 9 host-I/O,
5 resident), 204 minimal tests, strict all-target/all-feature Clippy, formatting
and generated table/language checks. Native static/shared async suites pass 65
runnable CTests each, synchronous 62, with unavailable-audio-device skips.
Retained-C core/async suites pass 20/19; selected proactor-off API/driver/request/
lifecycle/owned-command checks pass. Library C-ABI/proactor compilation checks
pass for Linux, Windows MSVC, iOS and Android; Windows-target strict library
Clippy also passes. These cross checks do not prove full target-engine runtime.
The default proactor async hash remains `311b5b6a8edf234e` and cancellation emits
zero samples. The previously documented legacy pthread cancellation defect
remains open; this slice preserves its C driver.

Identical preprocessing flags for `8b0f2a52` and current sources reduce `speech.c`
from 212 to 151 C/mixed lines in async/MBROLA builds and 197 to 136 in sync builds:
two controller bodies contributed 64 lines, replaced by a bridge and three short
primitives. Current full inventories retain 9,930 and 9,447 C/mixed lines, with
the prior getter/bridge and C++ scanner limits. Logs use `/private/tmp/espeak-driver-*`.
Local builds/tests were serialized; coarse thermal samples report no recorded
warnings. Pending C process I/O, ordinary MBROLA flush sequencing, owned engine/
resources, legacy backend retirement, text/tooling/platform migration and real
audio/idle-active thermal gates remain. The full port goal stays active.

## Native audio dispatch and event timing (2026-10-09)

`rust/engine_audio.rs` owns the proactor-path `dispatch_audio`, `create_events`
and `declare_event` controllers. An immutable typed table supplies atomic engine
state, device primitives, initialized event/PCM projections and existing proactor
event admission. The C shell keeps device-error formatting and forwarding wrappers.
Device open/write and event-capacity waits retain their completion implementation;
no worker, timer, sleep, polling or retry scheduler is added.

The controller preserves asynchronous command-enable checks, exact mode dispatch,
sample-rate event reopening, close/reset/open ordering, fresh device/rate/mode
reads after owner calls, event initialization, and distinct open/write policies.
Open/reopen errors publish audio failure and stop synthesis; write diagnostics
retain the earlier status and continue. Mode-zero callback returns remain ignored.
Empty word events are filtered without suppressing their PCM buffer. Event
admission publishes its returned status once through the existing capacity wait.

PCM is written once while the live event prefix is dispatched. Each event pointer
is checked against current count/capacity, and the initially admitted capacity
bounds callback-driven count changes. Invalid/negative counts, null nonempty
prefixes and negative PCM lengths stop before reading an unfilled tail or writing
invalid samples. Resource storage stays serialized and live through callbacks;
no mutable Rust resource borrow crosses them. Numeric sample-rate events read
only the defined four-byte union prefix.

Event delay uses widened sample-count/MBROLA-delay/position arithmetic before
millisecond scaling. Nonpositive rates, expired deadlines and extreme sample
positions clamp to a valid nonnegative host delay. Terminated-message events
retain the whole queued latency; other events subtract the trailing PCM time.
This prevents overflowing native-long arithmetic or narrowed millisecond
subtraction from producing an invalid timer. Cancellation ownership of those
timers remains with the existing event subsystem and still needs full integration.

The extracted, unchanged retained-C bodies run under `NDEBUG`: 155,520 dispatch
comparisons and 32,724 event-delay comparisons pass. They cover modes, missing
PCM/callback/device, rate transitions, device and admission results, zero/multiple
events, and callbacks changing mode, rate, device identity, count and event fields.
Native-only guards cover invalid extents/tables and extreme count/delay arithmetic.
Three Rust regressions check one-time PCM delivery and write-status policy,
optional capability paths, bounded count growth, and extreme/invalid timing.

Validation: 277 enabled Rust tests (256 unit, 7 Unix process, 9 host-I/O,
5 resident), 207 minimal tests, strict all-target/all-feature Clippy, formatting
and generated table/language checks. Native static/shared async suites each pass
66 runnable CTests; sync passes 63, with one unavailable-audio-device skip in
each suite. Retained-C core/async pass 20/19; six selected proactor-off API/audio/
driver/request/lifecycle/command checks pass. Library C-ABI/proactor compilation
checks pass on Linux, Windows MSVC, iOS and Android, plus Windows-target strict
library Clippy. These are compilation checks rather than full target-engine
runtime proof. Default proactor async PCM keeps hash `311b5b6a8edf234e` and
cancellation emits zero samples. The known legacy pthread cancellation defect
remains open; its original dispatcher and driver were preserved.

Identical preprocessing flags reduce `speech.c` C/mixed logic from 151 to 90
lines in async/MBROLA builds and 136 to 88 in sync builds. Current full measured
inventories retain 9,869/9,399 C/mixed lines respectively. The scanner classifies
short getter/diagnostic wrappers as bridges and omits C++ class members; moving
the short C event loop and timing controller is additional progress beyond the
reported non-bridge line decrease. Logs use `/private/tmp/espeak-audio-*`. Local
gates were serialized; coarse thermal samples report no recorded warnings.

Initial CI for `ea51f854` exposed an audio-disabled Linux build error: the
capability initializer used undefined `USE_RUST_AUDIO` in a C expression.
It now selects that capability with `#if USE_RUST_AUDIO`, matching the existing
configuration convention. An explicit proactor/async/MBROLA shared build with
Rust audio off passes all 66 CTests; rebuilt default API/audio-driver/synthesis/
async checks pass. The other five initial CI lanes passed. No Rust algorithm
or enabled-audio preprocessed controller body changed in this correction.

The proactor-off configuration retains the original C dispatch/loop. Identical
preprocessing flags for `f02b5d18` and current sources confirm every function in
its `speech.c` inventory is unchanged. Pending C process I/O, ordinary MBROLA
flush sequencing, full engine/resource ownership, legacy backend retirement,
remaining text/tooling/platform migration and real audio/idle-active thermal
validation remain open. The full port goal remains active.

## Native number and spelling primitives (2026-10-09)

`rust/number_primitives.rs` replaces six CPU algorithms from `numbers.c`:
`IsSuperscript`, `SetSpellingStress`, `M_Variant`, `hu_number_e`,
`CheckThousandsGroup`, and the recognition/value/cursor phase of `TranslateRoman`.
The 62-entry derived-letter map is now native static data; binary search preserves
its exact superscript/subscript return values. The C definitions remain available
as independent retained behavior oracles and in C-only builds.

Spelling preserves the original two-pass stress count and language-switch escape
policy, initial-letter versus grouped stress, marker deletion and short/long
pauses. It plans the final extent before writing and leaves the input unchanged
if the terminator or capacity is invalid. Its C adapter copies only the initialized
input prefix into a fixed 200-byte stack buffer, then writes only the validated
output and terminator. It never borrows an uninitialized foreign output tail or
allocates a buffer per character/request. The C wrapper retains the existing
terminated-string/200-byte owner contract; rejected native extents do not establish
safety for arbitrary malformed pointers passed to that public void C wrapper.

Thousands variants preserve signed C remainder behavior and all five configured
language forms. Group checks retain preceding/following digit exclusions,
including zero-length groups, and reject negative/oversized native requests.
Hungarian suffix handling retains the space, z, doubled-t and thousand/l cases.
The adapters stop at short-string terminators or the first rejected digit;
they do not construct full foreign views over an unused word tail.

Roman recognition preserves lowercase engine grammar, capital/dot policy,
neighboring-digit rejection, repeat/subtract transitions, configured min/max,
and the cursor after the consumed space/NUL. It deliberately keeps engine
behavior such as rejecting repeated `m`, rather than adopting a different
canonical Roman grammar. Native spans are bounded to the engine's 160-byte word
limit and include one initialized following boundary. Failed recognition leaves
value/cursor outputs and source unchanged. The C controller still owns `_roman`
dictionary lookup, dot-ordinal and Hungarian context, suffix text, number
translation and phoneme output; those are not claimed migrated here.

These are allocation-free bounded CPU operations on the existing engine path.
They add no worker, timer, sleep, polling or I/O scheduling. The same replacements
apply with proactor disabled; its legacy synthesis/async backend and known
pthread cancellation defect remain unchanged and open.

The extracted C algorithms execute under `NDEBUG`: 1,114,119 superscript cases,
200,000 spelling/output-tail cases, 160,008 signed thousands variants, 100,000
Hungarian forms, 100,000 grouping cases and 254,812 Roman cases pass (1,928,939
comparisons). Roman coverage includes 1,024 option/flag/neighbor combinations,
canonical inputs through 4,999, malformed/repeated/mixed strings and random words.
Three Rust regressions cover transactional capacity rejection, switch/stress
interaction, engine Roman policy and neighbor/bounds rejection, and number-form
and group boundaries. Native ABI checks cover null/unterminated/full-capacity
requests, short strings, negative grouping and bounded failed recognition.

Validation: 280 enabled Rust tests (259 unit, 7 Unix process, 9 host-I/O,
5 resident), 210 minimal tests, strict all-target/all-feature Clippy, formatting
and generated table/language checks. Native static/shared async each pass 67
runnable CTests; sync passes 64, with one unavailable-audio-device skip per suite.
Retained-C core/async pass 20/19; seven selected proactor-off API/primitive/driver/
request/lifecycle/command checks pass. Explicit audio-off shared async/MBROLA
passes all 67 CTests. C-ABI/proactor library compilation checks pass for Linux,
Windows MSVC, iOS and Android; Windows-target strict library Clippy passes.
Cross compilation does not establish full target-engine runtime.

Current retained-C and native CLI phoneme output matches in 48 comparisons across
English, Hungarian, Russian, Lithuanian, Polish, Czech, Slovak, Serbian, Croatian,
Bosnian, German and French. The four inputs cover Roman forms/malformed neighbors,
grouped/ordinal/Hungarian numbers, SSML spelling and derived characters. Exact
commands, output lengths and SHA-256 receipts are in `/private/tmp/espeak-number-cli.py`
and `espeak-number-cli-receipts.json`. Default native async PCM retains hash
`311b5b6a8edf234e`, with cancellation before its first sample.

Identical preprocessing flags for `aaf06759` and current sources reduce
`numbers.c` C/mixed logic from 1,103 to 943 lines in both sync and async builds.
The Roman controller's local declarations were also grouped when extracting its
recognizer, so line reduction alone is not an algorithm coverage metric. Current
full measured inventories retain 9,239 sync/9,709 async C/mixed lines, with prior
getter/bridge and C++ scanner limits. Logs use `/private/tmp/espeak-number-*`.
Builds/tests were serialized; coarse thermal samples report no recorded warnings.

Full number/letter dictionary control, remaining text frontend and tooling,
owned engine/resource and process/output integration, legacy backend retirement
and real audio/platform/idle-active thermal validation still remain. The full
port goal stays active.

## Native thousands-name dictionary control (2026-10-09)

`rust/number_lookup.rs` replaces the production `LookupThousands` controller.
It owns dictionary key construction, exact-value/ordinal/e/x lookup order,
fresh global language variants, lower-power probes, repeated-thousand fallback
and missing-name state, omit-one return policy and final concatenation. The
probe for the next lower power preserves the original behavior: its phonemes
are discarded, while its success suppresses the millions fallback. A generic
variant match still returns zero; exact-value and repeated-thousand matches
return their original flags. These policies are independent of output presence.

Typed serialized dictionary and state primitives remain in `numbers.c`.
Language options, number control and global variant options are read freshly
across lookups, including when the local and global translators differ. This
is bounded CPU control on the existing caller; no heap allocation, thread,
sleep, timer, polling loop or independent I/O scheduler is added.

The ABI uses fully initialized 200-byte dictionary scratch and copies only the
validated terminated output prefix into foreign storage. The two C callers
admit their actual 50-byte and 160-byte arrays. The controller enforces the
original 12-byte `_0of` and 160-byte name extents and checks the concatenated
capacity before publishing any output. The ABI leaves output and result
untouched on failure; the C shell publishes an empty pronunciation/no match.
Dictionary operations and already applied missing-state stores are not undone
or replayed on failure. The dictionary primitive still has its existing
200-byte terminated-output contract; this does not validate arbitrary C pointers
or bound an incorrectly implemented primitive's raw writes.

The original C body remains under `!USE_RUST_CORE` and is extracted into the
independent `rust_number_lookup` oracle. Its additional capacity parameter is
ignored; the old decision/concatenation body is retained. 355,648 defined-C
comparisons match exact lookup traces, all 200 output/tail bytes, result flags,
missing-name state and local/global control/options. Fixed sweeps cover
single-hit/missing outcomes, powers, values and all variant masks; randomized
cases include lookup operations that change options/control and zero flags
with nonempty output. ABI regressions cover null/missing admissions,
unterminated and overlong phonemes, insufficient 50/160-byte spans and successful
171-byte concatenation. Native regressions pin the discarded lower probe,
fallback return policy, exact-fit capacity, malformed phonemes and widened
signed dictionary keys.

Local gates pass: 282 enabled Rust tests (261 unit, seven Unix process, nine
host-I/O and five resident-I/O), 212 minimal tests, strict all-target/all-feature
Clippy, formatting and both generated-data checks. Four library cross-target
checks cover Linux aarch64, Windows MSVC x86_64, iOS and Android aarch64; strict
Windows-target library Clippy also passes. These are compilation checks.
Native static/shared async suites each pass 68 runnable CTests plus an audio
skip; synchronous passes 65 plus an audio skip. Retained-C core/async pass
20/19, selected proactor-off eight and explicit audio-off shared 68. The known
proactor-off pthread cancellation defect remains open and was not rerun.

After removing the now-unused native C `M_Variant` wrapper, affected paths were
rebuilt and focused API/primitive/oracle/phoneme/number checks passed on five
native configurations (six or seven checks each; native async also exercised
its synthesis/cancellation snapshot). 72 fresh CLI phoneme comparisons across
12 voices match retained C for grouped/ungrouped thousands and higher powers,
ordinals and locale separators. Texts/output lengths/SHA-256 receipts are in
`/private/tmp/espeak-thousands-cli.py` and `espeak-thousands-cli-receipts.json`.
Local builds/tests were serialized. Coarse thermal samples report no recorded
warnings; physical audio and representative idle/active thermal validation are
still absent.

Same preprocessing flags compare the published `0bf43fc9` source with this
slice: `numbers.c` C/mixed logic falls 943 to 883, including nine remaining
dictionary/state primitive lines and the five-line Rust bridge. Full inventories
are 9,179 sync and 9,649 async/MBROLA lines, with the prior getter/bridge and
C++ scanner limits. Inventory receipts and local logs use
`/private/tmp/espeak-thousands-*`. Remaining two/three-digit and main number
translation, Roman pronunciation, letter/dictionary/frontend/tooling, owned
engine/resource and process/output integration, legacy retirement and real
audio/platform/idle-active thermal validation keep the full goal active.

## Native tens/units and hundred pronunciation (2026-10-09)

`rust/number_digits.rs` replaces `LookupNum2` and `LookupNum3`. It owns the
cached final-digit shortcut; exact/feminine/e/a/o/q/ordinal dictionary order;
alternate and multiple ordinal suffixes; leading zeros and vigesimal tens;
units/tens order and conjunctions; vowel elision and primary-stress reduction;
hundred/year forms and omit-one policies; nested thousand joins and single-and
suppression; and short-pause removal before a final ordinal suffix. The hundred
path calls native `two` and `number_lookup::thousands` directly. Scalar flags,
cached text, suffixes and local/global options are freshly projected across
dictionary operations; C still owns the translator, text slots, phoneme table
and the main `TranslateNumber_1` parsing/decimal/context controller.

Fixed initialized buffers retain the original internal 12/20/50/100/160/211-byte
limits and check every concatenation. The C bridges admit actual 50/100/200-byte
output spans. Dictionary primitives receive initialized 200-byte scratch;
cached/ordinal adapters read only the initialized prefix through its terminator
within 50/12-byte extents. Final output and scalar publication occur only after
successful validation; unused foreign tails stay untouched. The C shells return
empty/no conjunction on failure. Already executed dictionary operations and
missing-name stores are not undone or replayed. The existing C dictionary still
has its 200-byte terminated-output contract; this does not bound a broken raw
primitive or validate arbitrary foreign pointers.

Native phoneme projections use unsigned byte codes and reject missing slots.
The legacy single-vowel path indexes using signed `char` converted to unsigned
int; negative codes can index far beyond the table. Those undefined legacy
cases have native-only regressions for a valid high-byte vowel and a missing
slot, plus C ABI coverage. Thousand-feminine shifts check the 0..3 range before
shifting, and the next power increments with checked arithmetic. Negative
numeric input/negative hundred context and overflowing increments fail without
output publication; legacy undefined shifts/overflows are not an oracle.

Retained C bodies remain under `!USE_RUST_CORE`, with ignored capacity arguments;
the old decision bodies are extracted into the `rust_number_digits` oracle.
366,400 tens/units and 293,184 hundred/nested-thousand comparisons (659,584 total)
match exact dictionary traces, all 200 output/tail bytes, return flags and
cache/ordinal/local/global control state. Fixed sweeps cover each number option,
whole/cached/absent lookup forms, all tens/units, boundary values through
`INT_MAX`, suffix forms, years and multiple powers; randomized cases combine
flags and lookup mutations of cache, ordinal, language/options and control.
Only defined C buffer/phoneme/shift cases enter parity comparisons. Native
regressions pin cached lookup suppression and first-stress precedence, unsigned
vowel/missing-slot handling, exact-fit output and short-pause suffix placement,
and bounded/checked power arithmetic. ABI guards cover incomplete tables,
null/invalid spans, unterminated source/dictionary text and missing phonemes.

Local validation passes 286 enabled Rust tests (265 unit, seven Unix process,
nine host-I/O and five resident-I/O) and 216 minimal tests; strict
all-target/all-feature Clippy, formatting and both generated-data checks pass.
Four library cross-target checks cover Linux aarch64, Windows MSVC x86_64,
iOS and Android aarch64, plus strict Windows-target library Clippy; these are
compilation checks. Native static/shared async pass 69 runnable CTests each
plus an audio skip; synchronous passes 66 plus an audio skip. Retained-C
core/async pass 20/19, selected proactor-off nine and explicit audio-off shared
69. The known proactor-off pthread cancellation defect remains open and was
not rerun. Local gates were serialized; coarse thermal samples report no
recorded warnings. Physical audio and representative idle/active thermal
evidence remain absent.

108 fresh CLI phoneme comparisons across 18 voices match retained C for
tens/units, hundreds, years, higher powers, ordinals, decimal/feminine forms
and locale separators, including Italian, Portuguese, Tamil, Malayalam,
Vietnamese and Mandarin alongside the previous 12 voices. Texts/output lengths/
SHA-256 receipts are in `/private/tmp/espeak-digits-cli.py` and
`espeak-digits-cli-receipts.json`. Native async retains 148,200 synchronous/
148,199 asynchronous samples, hash `311b5b6a8edf234e`, 36 events and zero
cancellation samples. These comparisons do not establish all-language or
physical-device coverage.

Same preprocessing flags compare published `bf8bb1b4` with this slice:
`numbers.c` C/mixed logic falls 883 to 550 lines; two controller bodies
(191/156 lines) become five/four-line Rust bridges, with 14 additional remaining
primitive lines. Full inventories retain 8,846 sync and 9,316 async/MBROLA C/mixed
lines, with previous getter/bridge and C++ scanner limits. Evidence/logs use
`/private/tmp/espeak-digits-*`. No production heap allocation, thread, timer,
sleep, polling or independent scheduler is added. Main number/Roman/letter and
full text/frontend/tooling/platform, owned resources and process/output,
legacy retirement and physical audio/idle-active thermal validation remain;
the full port goal stays active.

## Native dot-ordinal context (2026-10-09)

`rust/number_ordinal.rs` now owns `CheckDotOrdinal`: explicit/inferred dot
recognition, next-word spacing and uppercase guards, Roman exceptions, source
dot-to-space mutation, and Hungarian month/range decisions. The month lookup
retains `TranslateWord`'s changes to previous dictionary flags and current word
flags, including the final alternate-ordinal override after a hyphenated range.
It uses the native permissive UTF-8 reader, including initial continuation-byte
skipping, and the existing engine/Unicode classification primitives.

The C bridge admits the original initialized string through its terminator,
its two predecessor bytes, live word rows and a cursor within that string. The
native ABI validates its maximum 800-byte clause extent and complete callback
table before running. Source projections return virtual NUL beyond the admitted
extent and bound every mutation/translation offset. Each primitive projects a
scalar; Rust holds no foreign source/state slice across reentrant translation.
The immutable callback table and its stack context remain live through return.
These contracts rely on the existing C owners and trusted primitives, rather
than validating arbitrary raw pointers. The main number parser and Roman
pronunciation still own their remaining decisions in C.

The retained C body is extracted into `rust_number_ordinal`. 160,000 comparisons
match return values, all source bytes, word/dictionary state, classification
order and month-translation traces. Cases combine inferred/explicit dots,
uppercase/no-space guards, Roman/ordinary paths, Hungarian/other languages,
month flags, predecessor hyphens, callback state mutations, ASCII/Unicode and
continuation/truncated UTF-8. Initialized zero padding bounds the C oracle;
inaccessible/nonzero trailing storage has a separate native-only admission
test. Three Rust regressions cover callback-updated ordinal precedence,
Roman uppercase/month bypass and continuation/terminator lookahead. Invalid
tables, missing callbacks and invalid lengths/cursors return before mutation.

Local gates pass 289 enabled Rust tests (268 unit, seven Unix process, nine
host-I/O and five resident-I/O) and 219 minimal tests; strict all-target/all-feature
Clippy, formatting and both generated-data checks pass. Library compilation
checks cover Linux aarch64, Windows MSVC x86_64, iOS and Android aarch64, plus
strict Windows-target library Clippy. Native static/shared async pass 70 runnable
CTests each and sync passes 67, with audio-device skips. Retained-C core/async
pass 20/19, selected proactor-off ten and explicit audio-off shared 70. The known
legacy pthread cancellation defect remains open and was not rerun.

144 fresh CLI phoneme comparisons across 18 voices match retained C, including
Hungarian month/range phrases and Roman ordinals. Native async retains 148,200
sync/148,199 async samples, hash `311b5b6a8edf234e`, 36 events and zero cancellation
samples. Local workloads were serialized; coarse OS thermal samples report no
recorded thermal/performance warnings. Cross-target checks are compilation
evidence; physical audio and representative idle/active thermal evidence remain
absent. CLI text/output-length/hash receipts are in
`/private/tmp/espeak-ordinal-cli-receipts.json`.

Same preprocessing flags compare `47b03978` with this slice: `numbers.c`
C/mixed logic falls 550 to 535 lines. Its 36-line controller becomes a five-line
bridge plus 21 lines of source/state/classification primitives. Full sync and
async/MBROLA inventories retain 8,831 and 9,301 C/mixed lines, with the existing
getter/bridge and C++ scanner limitations. Evidence uses
`/private/tmp/espeak-ordinal-*`. No heap allocation, thread, timer, sleep,
polling or scheduler is added. Main number/decimal/Roman/letter/frontend and
tooling, owned resources and process/output integration, legacy retirement and
physical audio/platform/idle-active thermal evidence remain; the full port
goal is active.

## Native main number and decimal controller (2026-10-09)

`rust/number_frontend.rs` now owns `TranslateNumber_1` and the outer
`TranslateNumber` decision flow. It handles digit modes and long-number
declines; previous/grouped thousands and myriads; leading zeros and time
exceptions; dot and dictionary ordinal suffixes; Hungarian form control;
isolated-number names and percent prefixes; zero suppression and missing-name
fallback; all eight decimal policies, including feminine fraction suffixes and
alternative decimal digits; and final pause/skip-word/flag completion. It calls
native ordinal, digit and thousands controllers directly. Its final-digit cache,
count and form control live in each Rust invocation. The C stack-cache pointer
and its global count/control no longer compile in Rust-core builds. Nested
number lookup therefore retains the parent's live cache. Persistent primary/
alternate ordinal text and missing-thousand state remain C-owned and freshly
projected; owned engine/resources and full frontend migration remain open.

The source admission is the owner's complete initialized extent, rather than
`strlen(word)`. Slovak `12345` is split into `12\0 345`: its NUL thousands
separator precedes initialized following digits. A first-terminator bound lost
the thousand name and failed the cardinal suite. `TranslateWord2WithContext`
now scopes the actual initialized clause/split-number extent and restores the
previous context after translation. The bridge projects the local or primary
translator's containing context, with a CString fallback for separate synthetic
owners. Split buffers admit their explicitly initialized suffix padding;
uninitialized tails are excluded. Spelling fallback admits its fully initialized
150-byte buffer. Roman and replacement buffers now admit
three initialized predecessors. Clause extents are at most 800 bytes and word
rows at most 300; scalar reads provide virtual NUL outside the admitted extent.
Rust holds no foreign source/state slice across dictionary/month callbacks.

Fixed initialized buffers retain the original key, suffix, cache, ordinal and
intermediate phoneme limits. The ABI admits actual output capacity (at most
200) and two initialized flags, publishing only the validated terminated prefix.
Mode guards preserve output/flags; long initial numbers preserve output and
reset only the first flag as before. Isolated-name success keeps its original
early-return flag/missing-counter exception. Deliberate late decimal capacity
stops retain the admitted partial pronunciation and footer; a full nonexact
group footer overwrites the last byte to reserve its pause. Unbounded legacy
initial/intermediate concatenations, unterminated dictionary/cache text and
undefined fractional integer overflow instead reject without publishing
output/flags. Already executed source, dictionary and persistent-state effects
are not rolled back or replayed. Raw owner/primitive contracts still apply.

The retained C main and child controllers are independently extracted. An
observation after the original initial join records its extent; reduced-output
comparisons admit that join before testing legacy decimal stops. No C decision
is replaced or repaired. 244,096 cases, including 4,096 embedded-NUL cases and
196,803 reduced-capacity comparisons, match dictionary/list/classification/month
traces, both flags, all initialized source bytes, persistent state and final
pronunciation bytes. Native output tails remain untouched; incidental legacy
intermediate writes beyond its final terminator are not pronunciation evidence.
Cases combine all decimal modes, zero/time forms, groups/myriads, percent and
isolated names, ordinal/alternate suffixes, mutated options/word/month state,
Unicode/truncated lookahead, raw separator bytes/compiler char signedness and
initial integer boundaries. Six native regressions cover mode/overflow
declines, isolated-name early return, partial decimal stops, footer reservation,
cache ownership across nested numbers and embedded-NUL group lookahead. ABI
guards cover invalid tables/lengths/rows/output/flags, missing callbacks,
unterminated lookup/cache output, too-small initial output and undefined
fractional overflow.

Local validation passes 295 enabled Rust tests (274 unit, seven Unix process,
nine host-I/O and five resident-I/O) and 225 minimal tests; strict
all-target/all-feature Clippy, formatting and both generated-data checks pass.
Library cross-checks cover Linux aarch64, Windows MSVC x86_64, iOS and Android
aarch64, plus strict Windows-target library Clippy; these prove compilation.
Native static/shared async pass 71 runnable CTests each, synchronous 68, with
audio-device skips. Retained-C core/async pass 20/19, selected proactor-off eleven
and explicit audio-off shared 71. The known legacy pthread cancellation defect
remains open and was not rerun. The initial cardinal failure was corrected by
the owner-extent change; the final full suites pass.

Final source-extent review was followed by nine owner/rule/number/language checks
on each of five native configurations and a fresh repeat of the CLI comparisons;
all pass.

180 fresh CLI phoneme comparisons across 18 voices match retained C for whole,
grouped, ordinal, Roman, time/zero, long and repeated-decimal numbers, including
Hungarian month/range contexts. Receipts retain text/output lengths/hashes in
`/private/tmp/espeak-main-cli-receipts.json`. Native async still produces 148,200
synchronous/148,199 asynchronous samples, hash `311b5b6a8edf234e`, 36 events and
zero cancellation samples. Gates were serialized; coarse OS samples report no
recorded thermal/performance warning. Physical audio and representative
idle/active thermal evidence remain absent.

Same preprocessing flags compare published `368d3ff1` with this slice:
`numbers.c` C/mixed logic falls 535 to 241 lines; `translate.c` adds one
predecessor-initialization line. Full sync/async inventories retain 8,538/9,008
C/mixed lines with the existing getter/bridge and C++ scanner limits. Evidence
uses `/private/tmp/espeak-main-*`. No heap allocation, thread, timer, sleep,
polling or scheduler is added. Roman/letter/dictionary/translation frontend,
tooling/platform, owned engine/resources and process/output integration,
legacy retirement and physical audio/idle-active thermal evidence remain.
The full port goal stays active.

## Native Roman pronunciation controller (2026-10-09)

`rust/number_roman.rs` now owns the full `TranslateRoman` decision flow:
engine Roman recognition, optional dictionary name before/after the number,
configured numeric/ordinal suffix text, unchanged-dot decline, dot-derived
word ordinals, Hungarian hyphen/e-form admission, previous-dictionary reset
and final joins. It calls native ordinal/recognition primitives directly;
the compatibility adapter calls `rust/number_frontend.rs` directly for the
owned numeric source. No C Roman or number controller runs in Rust-core builds.
Obsolete C recognition/Hungarian/dot wrappers and unused ordinal projections
are removed from that configuration; retained bodies remain independent oracles.

The generated numeric text owns 160 initialized bytes, with three predecessors
and a checked terminated prefix. Dictionary/list/classification/state primitives
still use serialized C owners; source list/translation callbacks borrow the
owned text only through derived raw pointers and retain nothing after return.
Synthetic month translation scopes and restores its rule/source extent.
Name scratch admits the original 30-byte limit; foreign lookup output is
initialized 200-byte scratch. Output admission uses the actual capacity at most
200, reserving after-name space when the live initial policy requires it.
The final name policy and word flags are read freshly after callbacks. Defined
declines still publish the prefix (or empty output), even after recognition.
Malformed names/suffixes and source/output overflow instead reject without
publishing output; applied state/source/dictionary effects are not rolled back.
Already executed native number effects are not replayed on rejection.

The oracle extracts the original C Roman, ordinal, main-number and child-number
controllers independently. Only the existing main initial-join observation is
used to admit its unbounded writes; no decisions are repaired or replaced.
240,000 comparisons, including 238,761 reduced-capacity cases, match final
pronunciation, all initialized original source bytes, primitive traces and
persistent/word/previous-dictionary state. No generated cases require undefined
join exclusion. Unused native output tails stay untouched; incidental legacy
intermediate tail writes are not pronunciation evidence. The corpus combines
Roman grammar/ranges/capital flags, dot and uppercase contexts, Hungarian
hyphen/suffix forms, seven configured ordinal suffixes, live name-placement
changes and native main/digit/thousands variants. Four Rust regressions cover
post-recognition prefix declines, Hungarian admission/owned text, callback policy
and ordinal flags, and malformed/capacity rejection. ABI guards check invalid
tables, callbacks, lengths/rows/output, dictionary/cache text and suffix bounds.

Local gates pass 299 enabled Rust tests (278 unit, seven Unix process, nine
host-I/O and five resident-I/O) and 229 minimal tests; strict all-target/
all-feature Clippy, formatting and both generated-data checks pass. Static/
shared async pass 72 runnable CTests each, synchronous 69, with audio-device
skips. Retained-C core/async pass 20/19, selected proactor-off twelve and explicit
audio-off shared 72. The known legacy pthread cancellation defect remains open
and was not rerun. Library compilation passes Linux aarch64, Windows MSVC
x86_64, iOS and Android aarch64, plus strict Windows-target library Clippy.
The final uppercase-context fixture was strengthened to require dot-derived
ordinal flags without the unconditional Roman-ordinal option; full/minimal
Rust and strict checks were repeated afterward and pass.

480 fresh CLI phoneme comparisons across 24 voices match retained C for whole/
fractional/grouped numbers, upper/lowercase Roman forms, malformed numerals,
dot/uppercase/month contexts, Hungarian suffixes and configured ordinal forms.
Aragonese and Catalan join the earlier voice corpus. Text/output-length/hash
receipts are in `/private/tmp/espeak-roman-cli-receipts.json`. Native async retains
148,200 synchronous/148,199 asynchronous samples, hash `311b5b6a8edf234e`, 36 events
and zero cancellation samples. Gates were serialized; coarse OS samples report
no recorded thermal/performance warning. Physical audio and representative
idle/active thermal evidence remain absent; cross-target checks prove compilation.

Same preprocessing flags compare published `9a4ef58a` with this slice:
`numbers.c` C/mixed logic falls 241 to 207 lines. Full sync/async inventories retain
8,504/8,974 C/mixed lines with the existing getter/bridge and C++ scanner limits.
Evidence uses `/private/tmp/espeak-roman-*` and the current build CTest logs.
No heap allocation, thread, timer, sleep, polling or scheduler is added; Roman
pronunciation is bounded CPU work. Letter/diacritic lookup, full dictionary and
translation frontend, tooling/platform, owned engine/resources and process/output
integration, legacy retirement and real-device/thermal evidence remain open.
The full port goal stays active.

### Letter/symbol and diacritic lookup stage, 2026-10-09

`rust/letter_lookup.rs` now owns the complete `LookupLetter2`,
`LookupAccentedLetter` and `LookupLetter` controllers: dictionary ordering,
whitespace names, normal-text suppression and English fallback, spelling
context, rule fallback, decomposed accent/ligature assembly and stress
admission. `rust/letter_lookup_data.rs` owns the Latin and IPA packed tables,
non-ASCII base letters and modifier codes; the native controller owns the
accent-name keys. Original tables and controller bodies compile only in the
retained-C configuration and extracted test oracle.

The serialized C primitives supply dictionary lookup, rule translation,
locale whitespace classification, secondary translator setup, voice-table
selection and the existing native word-stress primitive through its C adapter.
Rule translation scopes/restores its initialized
source context. Secondary lookup restores the fresh live voice table before
validating its output, including malformed callback output. The native
controller preserves the original primary-miss pronunciation when an underscore
name suppresses normal text, and the original second-modifier behavior: a
before-marked modifier prefixes either form; an after-marked modifier trails
a ligature but is omitted from the non-ligature form. Latin U+017F remains
excluded by the original exclusive bound.

All controller text/source and callback scratch is owned and initialized;
no foreign output loan crosses a callback. Intermediate limits remain 30 bytes
for accent/base names, 20 for basic-letter rules and 160 for spelling rules.
Final publication admits the caller's actual extent, including the 80-byte
letter buffers, 77-byte secondary tails and decreasing hexadecimal tails.
`LookupBounded`/`LookupDictListBounded` carry actual dictionary caller extents
through to native accent fallback. These internal entry points do not make the
remaining C dictionary driver, raw lookup or text-mode writes bounded; that
driver and the outer spelling/translation joins remain migration work. The
public speech API is unchanged. Native rejection leaves caller output intact;
already applied primitive/source/state effects remain and are never replayed.

The independent retained-C oracle passes 1,234,112 comparisons: every code point
through U+10FFFF in accent mode and 120,000 varied accent/normal/spelling cases.
It checks keys, initialized predecessors, rule controls, live accent/language/
voice mutation, lookup misses with nonempty output, unsigned phoneme bytes,
callback order/state and output prefixes. 1,227,363 comparisons use reduced
capacities; mock callback outputs and original joins fit the admitted extents.
Malformed lookup/name/rule/stress output, invalid tables/arguments, too-small
output, untouched output tails and secondary-table restoration pass separate
guards. Six native regressions pin exact capacity, unsigned phonemes,
secondary-modifier ordering, ligature tails, normal suppression, restoration
and stress growth.

The user-requested loadngo `dev` refresh confirms `e421f5ea`'s Windows
reused-HANDLE repair is already included in this repository's `843ae1de` pin.
Engine/resident files use `DataFile` registrations, whose lifetime covers all
outstanding reads and ends before closing the file. The existing integration
regression reopens and reads three files on one platform proactor, suspending
synthesis until completion and retaining one buffer and the caller thread.
No dependency update is needed for this repair.

Local gates pass 305 enabled Rust tests (284 unit, seven Unix process, nine
host-I/O and five resident-I/O) and 235 minimal tests, strict all-target/
all-feature Clippy, formatting and both generated-data checks. Native static/
shared async each pass 73 runnable CTests and synchronous 70, with audio-device
skips. Retained-C core/async pass 20/19, selected proactor-off thirteen and
explicit audio-off shared all 73. The known proactor-off pthread cancellation
defect remains open and was not rerun. Library compilation checks pass Linux
aarch64, Windows MSVC x86_64, iOS and Android aarch64, plus strict Windows-target
library Clippy; these checks alone do not prove runtime behavior.

**Evidence correction, 2026-10-10:** the expanded CLI corpus at `e5d5f608`
matched full C in 853 of 864 cases; all earlier 480 number/Roman cases matched.
The claimed substituted-controller and published-HEAD comparisons were invalid:
the CLI linked `libespeak-ng.a`, and the probe's relink command did not replace
its archive members. Those comparisons ran the unchanged native archive and
cannot establish that the eleven differences predated the letter slice.
Correct archive-member substitution finds nine regressions introduced by this
slice and two earlier Tamil phrase-boundary differences. The repair and actual
substitution evidence are recorded in the next stage. The original CLI receipts
and isolated-source files remain under `/private/tmp/espeak-letter-*` as the
superseded record; do not reuse the original relink procedure.

Native async retains 148,200 synchronous/148,199 asynchronous samples, hash
`311b5b6a8edf234e`, 36 events and zero cancellation samples. Same preprocessing
flags reduce `numbers.c` C/mixed logic from 207 to 122 lines; current full sync/
async inventories retain 8,419/8,889 with existing bridge/getter and C++ scanner
limits. Evidence uses `/private/tmp/espeak-letter-*` and current CTest logs.
Local gates were serialized; coarse OS samples report no recorded thermal or
performance warning. This bounded CPU slice adds no heap allocation, thread,
timer, sleep, polling or scheduler. Physical audio and representative idle/
active thermal evidence, full dictionary/translation frontend and its identified
parity gaps, tooling/platform, owned engine/resources and process/output
integration and legacy retirement remain open. The full port goal stays active.

### Preserve letter pronunciation and replacement rule context, 2026-10-10

Rule fallback is an in/out operation: `TranslateRules` may append to the current
pronunciation or leave it unchanged. A dictionary lookup may return zero flags
while still writing pronunciation, notably when translating an emoji's text
replacement. The initial native letter controller copied that pronunciation
into its buffer, then passed empty scratch to rules. This dropped the prefix
and could trigger repeated replacement and an English fallback during spelling.

Both basic-letter and general spelling paths now copy the validated dictionary
prefix into their owned rule scratch before invoking the primitive. They keep
the original rule capacities, source scopes, bounded final publication and
callback effects. Two Rust regressions check append and no-op rules in spelling
and accent-base lookup. The retained-C oracle now traces the incoming rule
pronunciation and varies append/no-op/overwrite behavior, rather than always
overwriting it. That strengthened test fails against the original controller
at U+41CED with a pronunciation-bearing dictionary miss and passes after repair.
Two shell regressions require retained-C pronunciation for Polish and Malayalam
character-mode DNA emoji; the original native controller fails both.

The two older Tamil discrepancies came from missing backward source context in
multi-word text replacements. `TranslateWordWithBounds` now scopes the native
rule matcher to its whole initialized, owned replacement phrase, including the
three-byte prefix and NUL, and restores the caller's window after nested
translation. A third shell regression pins slightly smiling face and copyright
sign names with Tamil sandhi. The saved pre-repair executable splits those names;
the repaired executable and full C join them identically. The replacement driver
itself remains C and still needs migration.

Corrected probes copy the actual native engine archive, replace its `numbers.c.o`
member with the retained C letter controllers (`ar r`, then `ranlib`), and link
the CLI against that archive. The published-HEAD probe similarly replaces all
four affected C objects, compiled with their `f64db19d` headers and otherwise
identical native algorithms/data. The old procedure, which changed object
arguments absent from the CLI's static-library link command, is not evidence.
The prefix-only repair passes all 864 retained-letter CLI comparisons across
24 voices and matches full C in 862; its two Tamil differences reproduce with
the correctly linked `f64db19d` probe. After the source-window repair, all 864
cases match full C and the retained-letter probe rebuilt from the same repaired
engine archive. These are finite corpus comparisons, not universal language
parity. Final receipts are
`/private/tmp/espeak-letter-repair-cli-{receipts,baseline-mismatches}.json`;
the final substituted archive/executable is
`/private/tmp/espeak-letter-repair-retained-letter/`. Intermediate prefix-only
receipts and correctly linked old-source probes remain under
`/private/tmp/espeak-letter-prefix-cli-*` and `/private/tmp/espeak-correct-probe-*`.

Rust gates pass 307 enabled tests (286 unit, seven Unix process, nine host-I/O
and five resident-I/O) and 237 minimal tests, strict all-target/all-feature
Clippy, formatting and both generated-data checks. The strengthened oracle
passes 1,234,112 comparisons, including 1,227,363 reduced capacities. After both
repairs, native static/shared async each pass 73 runnable CTests and synchronous
70, with audio-device skips. The async API still produces 148,200 synchronous
and 148,199 asynchronous samples, hash `311b5b6a8edf234e`, with 36 events.
Retained-C core/async pass 20/19 CTests, the selected proactor-off set passes
thirteen, and the explicit audio-off suite passes all 73. The known proactor-off
pthread cancellation defect remains open and was not rerun.
Library compilation checks pass Linux aarch64, Windows MSVC x86_64, iOS and
Android aarch64, plus strict Windows library Clippy. These are compilation
checks; hosted CI supplies runtime platform evidence. Current same-configuration
sync/async C inventories are 8,425/8,895; the replacement scope adds six counted
C lines while the outer driver remains C. Logs use
`/private/tmp/espeak-letter-{prefix,repair}-*`.

The loadngo `dev` refresh to `2516ab2b` confirms no proactor changes after the
existing `843ae1de` pin; the Windows reused-HANDLE repair `e421f5ea` remains
included. Local builds/tests were serialized. Coarse macOS thermal samples
report no recorded thermal/performance warning, but do not establish thermal
safety. No allocation, thread, timer, sleep or scheduler was added. Physical
audio, representative active/idle thermal measurement, full frontend migration,
owned engine/resource integration, process/output, tooling/platform adapters and
legacy retirement remain open. The full port goal stays active.

### Native dictionary-list controller, 2026-10-10

`rust/lookup_list.rs` now owns the complete `LookupDictList` policy: dotted
abbreviation folding and its byte-based skip count, numeric-dot word admission,
three-repeat suppression and zero-padded repeat cache, accent fallback, added-e
and doubled-stem retries, live default-textmode inversion, original-word-only
replacement publication and final phoneme/flag publication. The old controller
and UTF-8 byte-count helper compile only in retained-C/oracle builds.

Serialized C projections supply bounded source bytes, contextual native
dictionary matches, current repeat/textmode state, accent pronunciation and
trace formatting. The controller owns initialized 160-byte keys/replacements,
200-byte pronunciation scratch and 20-byte repeat copies. No foreign output tail
is read, no foreign source/state loan survives nested translation, and errors
preserve the ABI caller's output/flags while retaining already-executed primitive
effects. The C engine adapter clears output on admission failure. Contextual
bucket projection now rejects pronunciation beyond the 200-byte scratch before
copying it. Replacement bytes are copied to the serialized translator's storage
rather than a global shared static buffer; engine/translator ownership still
needs migration.

Eight Rust regressions cover abbreviation bypass/cursors, terminal NUL and
numeric dots, repeat suppression/truncation, accent no-op pronunciation,
suffix retry ordering, fresh textmode state and replacement admission, atomic
output/flag failures, replacement capacity and repeat overflow. The initial
200,000-case retained-C oracle found a missing repeat-cache zero-padding effect;
after correction it passes callback key/next/flags/output order, repeat/skip and
textmode state, replacement text, trace output, reduced publication capacities
and untouched output tails. Undefined C synthetic-accent pointer subtraction
is excluded from traced cases; ordinary replacement traces are compared.
Null table/flag/output, missing byte callback, zero/oversized capacity,
unterminated primitive output and insufficient publication extents have separate
ABI guards; the final guard set also passes focused reruns in all five local
native/legacy/audio-off configurations.

Local gates pass 315 enabled Rust tests (294 unit, seven Unix process, nine
host-I/O and five resident-I/O) and 245 minimal tests, strict all-target/all-feature
Clippy, formatting and both generated checks. Native async static/shared each
pass 74 runnable CTests and synchronous 71, with audio-device skips; retained C
core/async pass 20/19, selected proactor-off fourteen and explicit audio-off all
74. The known proactor-off pthread cancellation defect remains open and was not
rerun. The deterministic async API still produces 148,200/148,199 samples,
hash `311b5b6a8edf234e`, and 36 events. Library compilation checks pass Linux
aarch64, Windows MSVC x86_64, iOS and Android aarch64, plus strict Windows library
Clippy; these are compilation evidence, not runtime platform validation.

All 864 phoneme-output cases across 24 voices match full C and a correctly
linked retained-list probe. Another 120 real CLI trace cases match that probe.
It recompiles the original list controller with current Translator headers,
copies the actual native archive, replaces `dictionary.c.o` using `ar r`/`ranlib`,
and links against the substituted archive; all other native algorithms and data
stay identical. Receipts/probe are `/private/tmp/espeak-lookup-list-{cli,trace-cli}-receipts.json`
and `/private/tmp/espeak-lookup-list-retained/`; gate logs use
`/private/tmp/espeak-lookup-list-*`. These finite comparisons do not establish
universal language parity.

Same-configuration preprocessing reduces dictionary C/mixed logic from 792 to
709 lines, with the scanner's existing small-bridge exclusions. Full sync/async
counts are now 8,342/8,812. The controller adds no heap allocation, thread,
timer, sleep, polling or scheduler. Dictionary bytes already arrive through
the platform proactor. Local builds/tests were serialized and coarse macOS
samples report no recorded thermal/performance warning; representative idle/
active measurements and physical audio remain unproven. `LookupBounded`,
`TranslateRules`, word/clause translation, engine/resource ownership, process/
output integration, tooling/platform adapters and legacy retirement remain
open. The full port goal stays active.

### Native symbol lookup, translated names and prefix flags, 2026-10-10

`rust/lookup_symbol.rs` replaces the complete `Lookup`/`LookupBounded` controller.
It invokes native dictionary-list policy directly, initializes symbol flags,
preserves signed dictionary/translation flag bits and pronunciation with zero
flags, prepares the original 80-byte padded/truncated name source, clears say-as
for translation, restores its fresh prior value after child return and validates
the final pronunciation before bounded publication. The retained C wrapper
compiles only in C/oracle builds; its raw final `strcpy` no longer runs in the
native engine. The ABI status and signed flag result are separate. The same
module replaces `LookupFlags` with an ordinary-context native list query and
owned 100-byte scratch, retaining both attributes even when the list returns
false for text replacement or a miss. Its static C flag buffer is retired.

Serialized projections provide the current original/replacement bytes and
say-as state. The translation primitive scopes/restores the native rule source
to the whole initialized owned 80-byte buffer, calls the remaining word driver
and copies only its terminated global pronunciation prefix into initialized
200-byte scratch. The safe controller holds no foreign source/state loan across
nested translation. On failure ABI output/result remain untouched while prior
dictionary/translation effects remain live; the C adapter clears output.

Eight Rust regressions cover signed direct flags/exact output, nonempty
pronunciation with zero child flags, fresh say-as restoration, source padding/
truncation, child/short-output failure, no replay and missing replacement source
before say-as changes, ordinary prefix attributes and failure without replay.
The retained-C oracle passes 200,000 symbol policy comparisons (100,093 nested
translations, 198,995 reduced capacities) and 200,000 prefix flag comparisons.
It compares the unchanged C parent policies over the same native list engine,
including query key/next/flag/output order, pronunciation, repeat/textmode/skip
state, replacement source, signed results and say-as restoration. Source,
unterminated child output, short publication and malformed ABI arguments have
separate guards that preserve output/results and already-executed effects.
The dictionary-list policy retains its independent 200,000-case C oracle.
Local gates pass 323 enabled Rust tests (302 unit, seven Unix process, nine
host-I/O and five resident-I/O) and 253 minimal tests, strict all-target/all-feature
Clippy, formatting and both generated checks. Native async static/shared each
pass 75 runnable CTests and synchronous 72, with audio-device skips; retained C
core/async pass 20/19, selected proactor-off fifteen and explicit audio-off all
75. The known proactor-off pthread cancellation defect remains open and was not
rerun. Deterministic async output retains 148,200/148,199 samples, hash
`311b5b6a8edf234e`, and 36 events. Linux aarch64, Windows MSVC x86_64, iOS and
Android aarch64 library compilation checks and strict Windows library Clippy
pass; these checks do not establish runtime platform behavior.

All 864 phoneme cases across 24 voices match full C and the retained symbol/
prefix probe; another 120 real CLI trace cases match that probe. It compiles
both original C controllers with current headers, replaces `dictionary.c.o` in
the actual native archive with `ar r`/`ranlib` and links the CLI against that
substituted archive. All other native policies and data remain identical.
Receipts/probe are `/private/tmp/espeak-lookup-symbol-{cli,trace-cli}-receipts.json`
and `/private/tmp/espeak-lookup-symbol-retained/`; gate logs use
`/private/tmp/espeak-lookup-symbol-*`. These finite comparisons do not establish
universal language parity.

Same-configuration preprocessing reduces dictionary C/mixed logic from 709 to
698 lines, with existing small-bridge exclusions; full sync/async counts are
8,331/8,801. `LookupBounded` and `LookupFlags` are thirteen-/seven-line admission
bridges, with source/say-as/translation projections still C. The controllers add
no heap allocation, thread, timer, sleep, polling or scheduler. Dictionary data
already arrives through the platform proactor. Latest loadngo dev `1cb30c82`
contains no proactor changes after the existing `843ae1de` dependency pin, which
includes Windows HANDLE reuse repair `e421f5ea`; no repinning was necessary.
Local gates were serialized, and coarse macOS samples report no recorded thermal/
performance warning. Representative idle/active and physical audio measurements
remain unproven. Translator/engine ownership, word/rule/clause translation,
integration, tooling/platform adapters and legacy retirement remain open. The
full native Rust port goal stays active.

### Rule translation orchestration, 2026-10-10

`rust/translate_rules.rs` replaces the complete `TranslateRules` controller:
relative-alphabet, paired and single/default group selection and score ties,
digit/symbol pronunciation and paired-digit pauses, spelling and letter fallback,
English/alphabet language switches, accent and diaeresis retries, ending/prefix
decisions, forward deletions, trace decisions and source restoration. Native
letter/accent resources replace the two remaining C tables in Rust-core builds.
The original driver and resources remain available to pure-C builds and the
independent test oracle.

Each invocation owns its original-word and match-pronunciation snapshots.
Source cursors, group identifiers and deletions cross the controller ABI as
checked numeric offsets. No controller borrow of foreign source, output,
translator state or resident pronunciation survives a nested matcher, symbol or
letter call. Output publication and the native phoneme append primitive operate
on the owner's fresh output: nested lookup can overwrite the shared word output,
and preserving that effect is required for parity. Ordinary and ending returns
restore the original non-NUL source prefix; early unpronounceable and language
switch returns retain the original C side effects. Rejection preserves already
executed effects and never replays the C controller. This ownership statement
applies to the new controller; the existing VM adapter and serialized C engine
owners still need migration.

Eleven native regressions cover ordered group probes/ties, relative alphabet
groups, paired digits and nested shared output, prefix lengths/restoration,
ignored-prefix deletion, early unpronounceable/language returns, accent restart,
diaeresis continuation, bracket pause/letter repeat state and malformed matches.
The retained-C oracle passes 200,000 complete driver comparisons, including
64,432 symbol calls, 6,580 letter calls and 122,863 ending/unpronounceable returns.
It compares the entire source/output/ending storage, flags, vowel/stress/repeat
and pause state, callback order and trace effects. ABI guards cover missing
callbacks/context/source, invalid groups, nonprogress, unterminated pronunciation
and publication refusal; malformed calls preserve the signed result and do not
replay effects. Missing-space accent input is rejected boundedly by Rust, and
is excluded from the original C oracle because its loop reads out of bounds.

Final current-source CLI evidence passes 864 phoneme cases across 24 voices
against both full C and the native build with the original rule driver, plus
120 matching original-driver traces. The probe compiles the retained driver
with current headers, substitutes the actual `dictionary.c.o` member using
`ar r`/`ranlib`, and links against that archive. Receipts/probe are
`/private/tmp/espeak-translate-rules-{cli,trace-cli}-receipts.json` and
`/private/tmp/espeak-translate-rules-retained/`. This finite corpus does not
establish universal language parity.

Serialized local gates pass: 334 enabled Rust tests (313 unit, seven process,
nine host and five resident integration), 264 minimal tests, strict Clippy,
formatting and both generated-data checks. Native async/shared configurations
each pass 76 runnable CTests, and sync passes 73; the device-dependent audio
test is skipped in each. Pure-C sync/async suites pass 20/19, selected
proactor-off checks pass 16 and explicit audio-off passes all 76. The known
legacy proactor-off cancellation defect is excluded and remains open.
Library checks pass for Linux aarch64, Windows MSVC x64, iOS aarch64 and Android
aarch64, with strict Windows Clippy; these checks establish compilation, not
target runtime validation. Gate logs use `/private/tmp/espeak-translate-rules-*`.

Fresh same-configuration inventories count 630 dictionary C/mixed lines
(previously 698), and 8,263/8,733 full sync/async lines. The original 197-line
C controller is replaced by native policy and smaller serialized projections;
the new seventeen-line adapter still counts as mixed. Scanner bridge exclusions
and C++ member limitations remain documented in `REMAINING_PORT.md`.

The controller adds no heap allocation, I/O, thread, timer, sleep, polling or
scheduler. It performs bounded CPU work over data already loaded through the
platform proactor. Coarse macOS samples report no recorded thermal/performance
warning; representative idle/active and physical audio measurements remain
unproven. Word/clause translation, translator/engine ownership,
process/output integration, tooling/platform adapters, legacy retirement and
representative hardware/thermal measurements remain open. The full native
Rust port goal stays active.

### Isolated-letter translation, 2026-10-10

`rust/translate_letter.rs` owns complete `TranslateLetter` orchestration:
private-character decoding and capital/dotless-I policy, superscript/subscript
modifiers, non-ASCII digit fallback, script names, original/local/secondary voice
selection, Hangul syllable decomposition, nested language requests, unknown
symbol names and hexadecimal/braille pronunciation. Native resources replace
the hex-name/modifier/range tables and `NonAsciiNumber` in Rust-core builds.
The original driver and resources remain compiled in pure-C builds and in the
independent oracle.

Every invocation owns initialized pronunciation, modifier, script-name and
synthetic Hangul buffers. Actual child capacities remain 30/27, 80/77 and the
remaining hexadecimal tail. Footer and final joins admit their complete prefix
before writing; undefined original C scratch overflows reject boundedly, without
replaying prior effects. Final owner publication reads fresh shared output after
nested lookup, preserving replacement effects and the original skip when an
otherwise valid append would fill the 200-byte owner. Early language switches
replace that output and return zero consumed bytes. The existing native UTF-8
primitive supplies character/consumed/following-byte scalars; native alphabet
identity crosses as its unique range start rather than a foreign pointer.
Source/state/translator storage are not borrowed by the Rust controller across
callbacks. C engine ownership and the existing leaf adapters remain; this is
not an ownership claim for the whole hybrid engine.

The Hangul primitive scopes/restores the whole padded 12-byte source extent
around native rule translation, then uses fresh secondary-translator stress
state. Table-zero terminators, script return tables, nested secondary setup,
ordered repeated letter probes and capitalization order preserve retained-C
behavior. Eleven Rust regressions cover these paths, private-character/dotless-I,
digit mapping, braille blanks/dots, English hex literals, absent secondary
translation, malformed scratch and capacity refusal. The retained-C oracle
passes 200,000 complete driver comparisons, including 129,119 named lookups,
306,327 letter lookups, 38,274 secondary setups, 1,916 Hangul calls and 8,111
early switches. Full owner output (including untouched tail), translator state,
callback order and trace effects agree. Malformed ABI guards preserve the
signed result and never replay effects. Undefined scratch-overflow inputs are
covered only by Rust rejection regressions, not executed by the C oracle.

Final CLI evidence passes 1,152 phoneme cases across 24 voices against both
full C and the native build with the original isolated-letter driver, plus 216
matching original-driver traces. The expanded corpus includes superscripts,
derived letters, non-ASCII digit ranges, Hangul, braille, private/upper/dotless
characters and mixed scripts. The probe compiles the retained driver with
current headers, substitutes the actual `translateword.c.o` archive member with
`ar r`/`ranlib`, and links against that archive. Receipts/probe are
`/private/tmp/espeak-translate-letter-{cli,trace-cli}-receipts.json` and
`/private/tmp/espeak-translate-letter-retained/`. These finite comparisons do
not establish universal language parity.

Serialized local gates pass: 345 enabled Rust tests (324 unit, seven process,
nine host and five resident integration), 275 minimal tests, strict Clippy,
formatting and both generated-data checks. Native async/shared configurations
each pass 77 runnable CTests, sync passes 74, with device-dependent audio skips.
Pure-C sync/async pass 20/19, selected proactor-off checks pass 17 and explicit
audio-off passes all 77. The known proactor-off cancellation defect is excluded
and remains open. Library checks pass for Linux aarch64, Windows MSVC x64, iOS
aarch64 and Android aarch64, with strict Windows Clippy; these prove compilation,
not target runtime validation. Gate logs use `/private/tmp/espeak-translate-letter-*`.
Fresh same-configuration inventories count 742 translation-word C/mixed lines
(previously 878), and 8,127/8,597 full sync/async lines; the original 209-line
driver and digit helper are replaced by native policy and smaller projections,
with the nine-line adapter excluded as a bridge. Scanner/C++ member limits
remain documented in `REMAINING_PORT.md`.

The controller adds no heap allocation, thread, timer, sleep, polling or
scheduler. Secondary voice setup retains the existing proactor-backed file I/O
and serialized C ownership; fixed phoneme encoding and remaining voice/stress
primitives are still owner callbacks. No separate I/O runner is added. Full
word/clause translation, engine/resource ownership, integration, tooling/platform
adapters, legacy retirement and physical audio/thermal validation remain open.
Coarse macOS samples report no recorded thermal/performance warning; they do
not establish representative idle/active thermal safety.
The full native Rust port goal stays active.

### Main word translation, 2026-10-10

`rust/translate_word.rs` replaces complete `TranslateWord3` policy: dictionary
and key/textmode admission, dots/grouped words, number/Roman/spelling paths,
unpronounceable prefixes, prefix confirmation/removal, stacked suffixes, stress,
grammatical expectations, bounded pronunciation publication and source restoration.
The original 466-line C policy remains only in non-Rust builds and the oracle.

The controller owns initialized 200-byte pronunciation/ending scratch and
160-byte suffix/original copies. Source identities are slot/checked-offset pairs
over admitted live owner windows, including initialized predecessors/lookahead.
The owner copies synthetic prefix text into its retained 65-byte storage before
the synchronous callback returns. No Rust source/state/word-row/output loan
crosses a primitive call. The serialized C owners and projections remain; this
does not establish ownership or memory safety for the entire hybrid engine.
The named number-language probe, spelling switches, plural operation and shared
stress act on fresh actual word output, preserving nested output mutations.
Normal return restores exactly the original admitted prefix, capped at 159
bytes; early returns preserve the original path-specific effects and ordering.
The historical end-unstress test uses pointer presence, including a false bool.

Prefix retries retain the 50-pass limit. Suffix joins truncate in place;
positive removals that make no source, predecessor, grammatical or ending-flag
progress reject without replay. Zero-character suffixes may advance grammatical
context without shortening the stem and retain a finite retry budget. Missing
source terminators, invalid offsets, malformed pronunciation or failed callbacks
preserve executed effects and refuse completion. ABI failure preserves the
result flag slot; the adapter never replays the retained controller.

Twelve Rust regressions cover unloaded owners, keys/textmode, fresh numeric
admission, spelling switches, prefix stress, stacked/zero-character suffixes,
pointer-presence/grammar behavior, malformed buffers, nonprogress and bounded
offsets/joins. The independent retained-C driver passes 200,000 episodes over
31 scenarios, including 755,228 prefix rule calls, 90,474 suffix removals and
6,452 letter calls. Whole source, output/text tails, translator state, flags,
callback order and traces agree. The scenarios include the full prefix limit,
multibyte prefixes and zero-character grammatical suffixes. ABI guards cover
absent callbacks/owners, extreme offsets and effect retention after failure.
The invalid/nonterminating cases are Rust rejection tests, not executed by the
C oracle. Primitive mocks isolate the controller; real speech is checked below.

Fresh CLI evidence passes 1,509 full-C phoneme comparisons across 27 voices and
459 original-word-driver traces. The current-header probe substitutes the actual
`translateword.c.o` archive member with `ar r`/`ranlib`; only the main word policy
is retained, with the other native controllers unchanged. Three additional
Finnish braille cases match the native and retained-word-driver builds, while
full C traps with signal 5. The macOS crash report confirms `__sprintf_chk`
overflow in `TranslateLetter`'s 80-byte formatting buffer. Those three cases
are separately recorded, excluded from successful full-C comparisons, and
exercise the existing bounded letter admission rather than this word change.
These finite corpora do not establish universal language parity. Receipts are
`/private/tmp/espeak-translate-word-{cli,trace-cli}-receipts.json`,
`/private/tmp/espeak-translate-word-finnish-braille-{baselines.json,crash-stack.txt}`
and `/private/tmp/espeak-translate-word-retained/`.

Serialized local gates pass: 357 enabled Rust tests (336 unit, seven process,
nine host and five resident integration), 287 minimal tests, strict Clippy,
formatting and both generated-data checks. Native async/shared each pass 78
runnable CTests, sync passes 75, with device-dependent audio skips. Pure-C
sync/async pass 20/19, selected proactor-off checks pass 18 and explicit audio-off
passes all 78. The known proactor-off cancellation defect remains excluded and
open. Library checks pass for Linux aarch64, Windows MSVC x64, iOS aarch64 and
Android aarch64, plus strict Windows Clippy; these prove compilation, not target
runtime coverage. Logs use `/private/tmp/espeak-translate-word-*`.
Fresh same-configuration inventories count 507 translation-word C/mixed lines
(previously 742), and 7,892/8,362 full sync/async lines. The 26-line C admission
function is still mixed glue; numeric source/state and primitive projections,
emoji/unpronounceable/dotted/spelling/plural helpers remain. Scanner and C++
member limitations remain documented in `REMAINING_PORT.md`.

The controller adds no heap allocation, thread, timer, sleep, polling or
scheduler. Existing voice/engine file reads keep their process-wide proactor.
The Loadngo pin already includes the Windows repeated-HANDLE registration fix;
latest dev has no subsequent proactor changes, so no repin is needed. Coarse
macOS samples report no recorded thermal/performance warning; representative
physical audio/idle/active thermal validation remains open. Word/clause glue,
engine/resource ownership, pending process I/O, tooling/platform integration and
legacy retirement also remain. The full native Rust port goal stays active.

### Word admission helpers, 2026-10-10

`rust/word_admission.rs` now owns complete `CheckDottedAbbrev` and
`Unpronouncable`/`Unpronouncable2` policy. Checked numeric sources and owned
80-byte abbreviation scratch preserve compaction/padding, possessive byte-copy
behavior, capacity refusal and dictionary skip count. Zero/one-letter results
and capacity refusal leave the original source and skip count unchanged.
Alphabet admission, raw locale classification, vowel position, apostrophe
context and fresh language options preserve the retained helper ordering.
The rule probe owns initialized 200-byte scratch and restores its saved
predecessor even when the primitive fails; other nested source/output/state
effects remain. Invalid source, overflowing scalar limits and failed callbacks
reject without replay; ABI failure preserves the caller's result slot.

The C adapter shares the main word's checked live source projections. Admission
first uses a matching translator rule window, including secondary owners;
the internal fallback still depends on the caller's initialized predecessor,
NUL terminator and lookahead contract. Serialized translator/source ownership
remains C. These bounds and scoped patches do not prove memory safety for the
whole hybrid engine. The complete original helper bodies remain in non-Rust
builds and as independently compiled oracle references.

Nine Rust regressions cover compaction, multibyte/possessive byte copies,
capacity refusal, alphabet/disabled modes, apostrophe/vowel context, fresh
language options, failed probe restoration and invalid source/scalar bounds.
The retained-C comparisons pass 200,000 dotted and 200,000 unpronounceable
inputs: 76,743 compactions, 39,600 capacity refusals, 79,291 vowel calls and
4,930 rule probes. Whole source/output tails, translator state, skip count,
callback order and traces agree, including nested option and source changes.
ABI guards reject absent callbacks and source spans. The existing 200,000
main-word comparisons also pass after sharing source-window admission.

A fresh current-header probe replaces the actual `translateword.c.o` archive
member with `ar r`/`ranlib`, retaining only the two C helper policies. All
1,509 phoneme cases across 27 voices match full C and that probe, and all 459
native/probe traces agree. The three previously recorded Finnish braille
full-C overflow cases are excluded from successful full-C counts; fresh native
and C-helper probe outputs agree with each other and the prior native outputs.
The full-C crashes were not rerun in this slice. Their crash stack and prior
receipts remain `/private/tmp/espeak-translate-word-finnish-braille-*`.
Current receipts and logs use `/private/tmp/espeak-word-admission-*`.
These finite corpora establish measured agreement, not universal parity.

Serialized offline local gates pass: 366 enabled Rust tests (345 unit, seven
process, nine host and five resident integration), 296 minimal tests, strict
Clippy, formatting and both generated-data checks. Native async/shared each
pass 79 runnable CTests, sync passes 76, with device-dependent audio skips.
Pure-C sync/async pass 20/19, selected proactor-off checks pass 19 and explicit
audio-off passes all 79. The known proactor-off cancellation defect remains
excluded and open. Linux aarch64, Windows MSVC x64, iOS aarch64 and Android
aarch64 library checks and strict Windows Clippy pass; these are compilation
checks, not execution on those targets.

Fresh same-configuration inventories count 420 translation-word C logic lines
(previously 507), 48 C functions, zero mixed functions and six bridges.
Full sync/async counts are 7,805/8,275. Main word admission is now a short
forwarding bridge; checked source/state/output projections, emoji fallback,
plural/spelling helpers and broader word/clause glue still require migration.
Scanner and C++ member limitations remain in `REMAINING_PORT.md`.

This controller introduces no heap allocation, thread, timer, sleep, polling or
scheduler. Existing file reads continue through the process-wide proactor;
the current Loadngo pin already includes the Windows repeated-HANDLE fix.
Physical audio/idle/active thermal validation remains open. The published prior
checkpoint's remote CI was last verified at six of seven jobs passed; final
status and this slice's publication await restored automatic approval-review
access after its usage-limit failure. The full native Rust port goal is active.

### Individual-letter spelling and plural helpers, 2026-10-10

`rust/word_helpers.rs` now owns complete `SpeakIndividualLetters` and
`addPluralSuffixes` policies. Spelling uses checked numeric source progress,
capital/noninitial controls and fresh live pronunciation after every letter
primitive. A language switch copies the actual current output and bypasses
final spelling stress. Zero-byte consumption is valid for a language switch;
the switch is handled before refusing zero progress that would continue the
loop. Empty input still applies zero-character spelling stress. Failed
primitives, missing source, overflowing offsets and nonprogress retain executed
effects and refuse without replay; ABI failure preserves result slots.

Plural admission and `ss`/`zz`/`iz` selection are native. Its initialized owned
five-byte source includes the original NUL/space predecessor and terminator.
The primitive scopes and restores the translator's rule-source context, while
keeping all other nested state/output effects. Letter/stress/rule primitives
use fresh actual retained output; no foreign Rust source/state/output loan
crosses them. C source/translator/output ownership and primitive projections
remain, so this does not establish safety of the whole hybrid engine. The
controller adds no heap allocation, thread, timer, sleep, polling or scheduler;
existing file I/O continues through the process-wide proactor.

Seven Rust regressions cover multibyte spelling and modes, empty input, zero
and positive-byte language switches, invalid/nonprogressing sources, all 256
plural byte values and failed primitive effects. Retained-C comparisons pass
200,000 spelling and 200,000 plural cases, including 383,335 letter calls,
21,204 zero-byte switches, 178,796 stresses and 133,333 rule calls. Complete
source/output tails, scalar state, callback order and traces agree, with
ordinary output aliases and scoped rule-context restoration. The existing
200,000 main-word and 400,000 admission-helper comparisons also pass.

The original C spelling helper traps on macOS when a language switch uses
aliased output buffers: `__strcpy_chk` reports source/destination overlap.
Those invalid C calls are excluded from the oracle; the native path separately
verifies its bounded `memmove` switch copy, including zero-byte consumption.
The retained-C crash stack is
`/private/tmp/espeak-word-helpers-retained-alias-crash-stack.txt`. ABI guards
also verify missing callbacks/source, zero nonswitch progress, unterminated
switch output and preserved result/effect slots. Invalid/nonterminating inputs
are native rejection checks, not executions of the retained C loop.

Fresh current-header probes substitute the actual `translateword.c.o` archive
member, retaining only these two helper policies. All 1,509 phoneme cases
across 27 voices match full C and the probe; all 459 native/probe traces agree.
The real Russian Roman-numeral/English fallback phrase exposed the zero-byte
switch ordering requirement and is now a permanent pronunciation regression,
passing all native and retained-C speech lanes. Three previously recorded
Finnish braille full-C overflow cases remain outside successful full-C counts;
fresh native and C-helper outputs agree with each other and prior native output.
Full-C crashes were not rerun here. Finite corpus agreement does not establish
universal language parity. Receipts/logs use `/private/tmp/espeak-word-helpers-*`.

Serialized offline gates pass on the corrected code: 373 enabled Rust tests
(352 unit, seven process, nine host and five resident integration), 303 minimal
tests, strict Clippy, formatting and both generated-data checks. Native
async/shared each pass 80 runnable CTests, sync passes 77, with device-dependent
audio skips. Pure-C sync/async pass 20/19, selected proactor-off checks pass 20
and explicit audio-off passes all 80. The known proactor-off cancellation
defect remains excluded and open. Linux aarch64, Windows MSVC x64, iOS aarch64
and Android aarch64 library checks and strict Windows Clippy pass; these prove
compilation, not target runtime execution.

Fresh same-configuration inventories count 422 translation-word C logic lines,
53 C functions, zero mixed functions and eight bridges; full sync/async counts
are 7,807/8,277. The former 34-line C policies are now native, but their fresh
owner/primitive projections add 36 C lines, so the raw total increases by two.
Emoji fallback, those projections, broader word/clause glue, engine/resources,
pending process I/O, tooling/platform and legacy retirement remain. Physical
audio and representative idle/active thermal validation also remain open.
Publication and final prior-head remote CI verification still await restored
automatic approval review after its usage-limit failure. The full port goal
remains active.

### Emoji base/modifier/tag fallback, 2026-10-10

`rust/emoji_fallback.rs` now owns complete `LookupEmojiBaseSequence` policy:
initial emoji admission, modifier/tag removal, strict whole-character stripped
capacity, base lookup, tag-only retry, up to eight modifier names, bounded
joins, fresh tracing and source/flag publication. A too-wide character is
skipped intact while later narrower characters can still fit, preserving the
original byte behavior. Missing modifier base names refuse; unlisted tag-only
sequences reuse the stripped base. Absent modifier names skip, and a join that
would reach byte 159 stops with the existing replacement.

Lookup input/selected text and pronunciation are initialized owned 162/200-byte
scratch, with two local flags reset before each lookup. Selected dictionary
text is copied from an admitted complete terminated owner before return, so
later lookups cannot overwrite a borrowed result. Replacement writes occur at
the original offsets and times: before modifier calls, then after each admitted
join. This preserves nested live replacement mutations, untouched tails and
the original tracked end offset. Trace uses fresh source/replacement state and
the latest trace option; caller flags are read fresh only after source
publication. Missing source, malformed selected text, invalid ranges or failed
callbacks retain executed effects and refuse without replay.

The former global replacement is now a 160-byte translator field, initialized
by the native translator's existing `calloc`. Its lifetime follows that owner;
another translator's fallback cannot overwrite it. No per-request heap
allocation, thread, timer, sleep, polling or scheduler is introduced. Byte,
list, checked write, trace/snapshot and pointer publication still project
serialized C source/translator owners. No foreign Rust source/state/output
loan crosses a nested lookup, but whole-engine ownership and memory safety
remain separate work. Existing file reads retain their process-wide proactor
and the pin containing the Windows repeated-HANDLE registration fix.

Eleven Rust regressions cover plain/empty-base admission, modifier and tag-only
fallbacks, strict multibyte stripping, eight-modifier and join limits, missing
names, fresh nested source/replacement effects, failed lookups after publication,
missing source and unterminated selected text. An unchanged retained-C helper
passes 200,000 episodes: 672,376 list calls, 76,632 traces, 125,249 replacements
and 74,751 refusals. Complete source, translator/replacement tails, flags,
callback order, trace option and traces agree. Its static owner's location is
normalized into the native field; its actual initialized contents and lifetime
between episodes are preserved. Native checks separately prove two-translator
isolation and retained partial writes/result slots after failure. These pass
on async/shared/sync, selected proactor-off and audio-off lanes. Existing one
million main-word/admission/spelling/plural comparisons also remain green.

The fresh current-header probe substitutes the actual `translateword.c.o`
archive member, retaining only the original emoji policy. All 1,671 phoneme
cases across 27 voices match full C and that probe; all 621 native/probe traces
agree. Six added cases per voice exercise modifier chains, tagged flags,
missing base names and ordinary text after replacements. Three recorded
Finnish braille full-C overflow cases remain outside successful full-C counts;
current native and C-helper outputs match each other and prior native output.
Full-C crashes were not rerun. Finite corpus agreement does not prove universal
parity. Evidence uses `/private/tmp/espeak-emoji-fallback-*`.

Serialized offline gates pass: 384 enabled Rust tests (363 unit, seven process,
nine host and five resident integration), 314 minimal tests, strict Clippy,
formatting and both generated-data checks. Native async/shared each pass 81
runnable CTests, sync passes 78, with device-dependent audio skips. Pure-C
sync/async pass 20/19, selected proactor-off checks pass 21 and explicit audio-off
passes all 81. The known proactor-off cancellation defect remains excluded and
open. Four library checks pass for Linux aarch64, Windows MSVC x64, iOS aarch64
and Android aarch64, plus strict Windows Clippy; these prove compilation, not
target runtime execution. Only the final isolation test was added after the
full suites; its complete comparison/ABI target was rebuilt and passed on all
five native/compatibility lanes afterward.

Fresh inventories count 380 translation-word C logic lines (previously 422),
59 C functions, zero mixed functions and nine bridges. Whole sync/async counts
are 7,765/8,235. The complete word helper policies are now native; checked
source/state/output and primitive projections remain. Larger `TranslateWord2`,
replacement/clause controllers, engine/resources, pending process I/O,
tooling/platform and retirement still require work. Physical audio and
representative idle/active thermal validation remain open. Publication and
final prior-head CI verification await restored automatic approval review
after its usage-limit failure. The full native port goal remains active.

### Complete word-to-first-stage-list controller, 2026-10-10

`rust/translate_word2.rs` owns complete `TranslateWord2` policy: embedded
command admission, deleted-word command carriers, reserved list capacity,
prepause timeout and say-as pauses, capital pitch commands, phoneme mnemonic
input, spelling restoration, combination admission, up to two alternate
language retries, dictionary/table restoration, dictionary pauses and emphasis,
capital announcements, initial table-switch reuse, hyphen pause removal,
stress/tone/syllabic/lengthening/boundary actions, first-consonant doubling,
source indices, end-vowel state, comma pauses, pitch reset and stress promotion.
The native build no longer compiles the original controller or its `SetPlist2`
initializer; retained C builds and tests keep them as an oracle.

Pronunciation bytes, phoneme classifications and scalar options are projected
fresh from serialized C owners. Captured original word flags/source index,
first-entry index and local stress/cursor state retain the original timing;
nested translator and table-selection effects remain live. Only initialized
owned entry copies cross read/write callbacks; untouched entry fields and
array tails survive. Numeric source, list and command indices are checked;
missing bytes, invalid predecessors or failed callbacks refuse without replay
and retain executed effects. Row/translator/global owner lifetime and broader
engine memory safety remain separate work.

Saved word/replacement scratch is bounded and initialized. The replacement
uses 163 bytes: two predecessor bytes plus the main primitive's complete
161-byte text output. This fixes the old local allocation's mismatch with its
`+2` output offset. The alternate replacement primitive scopes the full owned
source window and restores its caller's rule context after nested effects.
Names and saved dictionary state use initialized owned 200/40-byte scratch;
no foreign state/output loan crosses a nested translation or resource call.
There is no per-word heap allocation, thread, timer, sleep or scheduler in the
new production controller. Alternate resources retain the existing shared
proactor-backed reads and pin containing the Windows HANDLE-reuse fix.

Ten Rust regressions and 200,000 unchanged-C controller comparisons pass.
The latter exercise 338,110 translations, 198,049 alternate attempts, 20,306
combinations, 11,419,279 accumulated list entries and 20,561 invalid-code
diagnostics. Complete translator/row/source/output/list/command storage,
options, dictionary, counters, callback order and diagnostics agree. Table
selection mutates subsequent classifications, testing fresh reads rather than
a cached table. Native guards separately cover invalid ABI tables, retained
failure effects/result slots, complete wide replacement output, checked
indices and restoration of replacement rule context after nested mutation.

All serial offline gates pass: 394 enabled Rust tests (373 unit, seven process,
nine host and five resident integration), 324 minimal, strict all-target Clippy,
formatting and both generated-data checks. Native async/shared each pass 82
runnable CTests and sync passes 79, with device-dependent audio skips. Pure-C
sync/async pass 20/19; selected proactor-off checks pass 22 and explicit audio-off
passes all 82. The known proactor-off cancellation defect remains excluded and
open. Linux aarch64, Windows MSVC x64, iOS aarch64 and Android aarch64 library
checks and strict Windows Clippy pass; these are compilation checks, not target
runtime execution. No physical audio or thermal-safety claim is made.

A fresh current-header archive probe forces only original `TranslateWord2`
and `SetPlist2`, substitutes the actual `translate.c.o` member and links against
the current native core. All 2,211 phoneme cases across 27 voices match both
full C and the probe; all 945 native/probe traces agree. New inputs exercise
embedded emphasis/pauses, mnemonic input, longer clauses and CLI capital modes
1/3/7. Another 25 deterministic offline WAV outputs across five voices match
all three executables, including capital announcements, pitch, emphasis and
pauses. Three known full-C Finnish braille overflow cases remain excluded from
successful full-C counts; current native/probe and prior native outputs agree.
Full-C crashes were not rerun. Finite corpus agreement does not prove universal
parity. Evidence uses `/private/tmp/espeak-word2-*`.

Fresh inventory counts 1,166 C logic lines in `translate.c` (previously 1,376),
43 C functions, zero mixed and two bridges. The 276-line controller and its
initializer are native; checked owner/primitive projections remain. Whole
sync/async totals are 7,555/8,025, subject to the documented scanner and C++
limits. `TranslateWordWithBounds`, replacement segmentation, larger clause
control, full engine/resource ownership, pending process I/O, tooling/platform
retirement and physical audio/thermal validation remain open. Publication and
final prior-head CI verification await restored automatic approval review after
its usage-limit failure. The full port goal remains active.

### Replacement segmentation and recursive expansion, 2026-10-10

`rust/replacement.rs` now owns complete `ShouldSplitIdeographs`,
`SegmentReplacement` and `TranslateWordWithBounds` policy. Ideograph splitting
uses native alphabet resources and fresh translator options. Segmentation
preserves whole-character capacity checks, its two-byte reservation, alphabet
and punctuation decisions, hyphen dropping between letters and untouched output
tails. Expansion preserves original flags, same-width case conversion, nested
replacement depth, dictionary skip save/restore, word-boundary separators and
the empty-result rule. Main translation still calls the existing native word
controller through a serialized C projection.

Phrase/replacement/pronunciation scratch is initialized owned 323/161/200-byte
storage. Nested word calls scope the actual initialized phrase prefix and
restore the caller's rule context. Source mutations, row flags, grammar,
pronunciation and dictionary skip effects remain live. Initial text writes go
directly to the retained caller output; only a terminated textmode prefix is
copied into owned scratch. Uninitialized caller output tails are neither read
nor overwritten. No foreign source/state/output view crosses a callback.
Retained translator/row/global lifetimes and whole-engine memory safety remain
separate work.

The join tracks `snprintf`'s would-have-written count numerically while copying
only the admitted bounded prefix and NUL. It never constructs the old
out-of-array pointer after truncation. Skipping stops at initialized phrase NUL,
including missing final delimiters and excess skip counts. Negative depth,
nonprogressing invalid skip counts, missing source, unterminated pronunciation
and failed primitives refuse without replay. Skip scope is restored even on
failure; other executed effects and result slots are retained. Production
scratch is fixed-size and recursion/word scans are bounded, with no per-word
heap allocation, thread, sleep, polling or scheduler.

Eleven Rust regressions and 200,000 retained-C cases for each of segmentation
and expansion pass: 3,278,084 segmented bytes, 96,667 ideograph splits,
1,087,988 word calls and 490,806 depth-limit calls. The unchanged original
policies agree on complete source/translator/row/text/output/skip state and
callback sequence, including nested phrase mutations, case handling and
untouched output tails. Native-only guards cover invalid tables/result slots,
partial failure, scoped context restoration, missing termination and checked
extents; Rust tests separately cover truncation, invalid depth/skips and missing
final delimiters outside the supported C oracle domain.

The first full speech suite exposed two adapter regressions that the original
fixture domain missed. Symbol lookup calls `TranslateWord` with null rows and
null replacement output; rejecting those rows dropped valid pronunciation with
zero dictionary flags, breaking Polish/Malayalam DNA character spelling. The
wrapper now admits that primitive-only path, verified separately against C and
the existing permanent pronunciation regressions. On macOS, unsigned byte
classification treated a Georgian UTF-8 continuation byte (`0xa0`) as whitespace,
adding an extra final-letter pronunciation. Byte skipping now applies locale
whitespace only to ASCII bytes, preserving UTF-8 word boundaries. Georgian
replacement phrases were added to the C oracle and expanded archive corpus;
the existing complete `translate` suite passes after both corrections.

Incremental builds also revealed stale extracted projection includes:
`file(READ)` did not track its source as a configure dependency. CMake now tracks
`translate.c` and `translateword.c`, so policy/projection test copies regenerate
when either changes. The corrected production/projection target was freshly
configured, rebuilt and passed with the complete 400,000 comparisons and guard
checks. Failure and correction logs use the same stage evidence prefix.

Corrected-code serial gates pass: 405 enabled Rust tests (384 unit and 21
integration), 335 minimal tests, strict all-target/all-feature Clippy, formatting
and both generated-resource checks. Native static/shared async suites each pass
83 runnable CTests; synchronous passes 80, with physical audio skipped in those
three configurations. Explicit audio-off passes 83; full-C sync/async pass
20/19, and 23 selected proactor-off checks pass. The known proactor-off
cancellation defect remains outside those selected checks. Linux aarch64,
Windows MSVC x64, iOS and Android aarch64 library cross-checks and strict Windows
Clippy pass; these establish compilation, not target runtime validation.

The current-header archive probe compiles unchanged C segmentation/expansion,
substitutes the actual `translate.c.o` member and retains all other native
controllers. It matches 2,931 successful full-C phoneme-output cases across 33
voices, including capital modes and mixed ideograph/Georgian/emoji replacement
inputs. Japanese symbol-only cases may have no phonemes; equality is checked
for the complete successful output rather than requiring nonempty speech.

The larger corpus exposed three additional full-C Arabic braille traps, on the
same inputs already recorded for Finnish. All three OS crash reports show
signal 5 through `__chk_fail_overflow`, `__sprintf_chk` and `TranslateLetter`.
These are excluded from the 2,931 successful full-C comparisons; current
native/probe output matches on all three. Receipts retain subprocess status,
PID, exact input, output and audited crash-report path. Previously verified
Finnish C crashes are not rerun. Evidence uses
`/private/tmp/espeak-replacement-*`, including
`new-braille-audit.json` and the original failure/correction logs.

Native and retained replacement-controller probe match 1,386 real CLI traces
across 33 voices. Twenty-five deterministic offline WAV cases across five
voices match full C, native and the probe. The three Finnish braille cases also
match current native/probe and previous native output. No physical playback or
new representative thermal measurement is claimed.

Fresh inventory counts 1,098 C logic lines in `translate.c` (previously 1,166),
48 C functions, no mixed functions and four bridges. New checked projections
are counted as C even though policy has moved; complete sync/async totals are
7,487/7,957, subject to the documented scanner/C++ limits. Clause control,
full engine/resources, pending process I/O, tooling/platform retirement and
physical audio/thermal validation remain open. Publication and final prior-head
CI verification still await restored automatic approval-review access after
its usage-limit failure; Jay's authorization persists. The full port goal stays
active.

### Complete clause translation controller, 2026-10-10

`rust/translate_clause.rs` now owns complete `TranslateClauseWithTerminator`
policy: admission/reset, tokenization and source positions, script/emoji/case
boundaries, embedded commands, number regrouping, spelling retries, skip and
abbreviation effects, and final pause/phoneme/voice publication. The original
controller is retained only for C builds and test oracles. The old word-length
helper and unused C ideograph bridge also leave the production native build;
test-only extraction/probe guards retain them where the old controller needs
them. Character substitution, uppercase-word matching, embedded command
primitives and translator/engine/resource ownership remain separate work.

Fixed owned scratch contains an 800-byte clause, 804 position entries,
initialized 301-slot row tables with explicit padding and a predecessor slot,
150-byte number/spelling scratch, and a 40-byte voice name. Number-row storage
is reused through dispatch. Serialized callbacks supply fresh scalar, source,
classification, entry and command state. Each word call scopes/restores rule
and clause-end contexts; C retains no pointer into Rust scratch after return.
Voice reading updates stable retained C storage, preserving previous returned
name pointers even when current voice output slots are absent. Only its
initialized terminated prefix enters Rust scratch. A C regression compares
that prior-pointer behavior against the original controller. Source, grammar,
row/skip/pause and output mutations remain live; errors retain executed effects
and completed public output writes without replaying C.

The UTF-8 reader has the physical 840-byte retained-source limit even if a
callback incorrectly keeps returning bytes. Tokenization/insertion work is
bounded; byte/row/phoneme/command extents, skip subtraction and signed scalar
arithmetic are checked. Number grouping admits only bit positions 0..31,
avoiding the old undefined signed shifts for negative padding positions.
Spelling stops at initialized NUL rather than scanning beyond a missing space.
Production adds no heap allocation, thread, sleep, polling or scheduler. This
CPU policy stays in the existing engine path; it does not offload work by
misusing proactor completion delivery. Blocking-resource/engine ownership work
and physical thermal validation remain in the full goal.

The unchanged-C controller and actual native projections pass 200,000 complete
episodes: 2,870,339 word calls, 83,334 embedded commands, 321,912 number parts
and 197,620 spelling calls. The oracle compares primitive order, initialized
source/number prefixes, meaningful row fields and complete retained fixture
state; it does not claim agreement on undefined C stack padding. Source
mutations, fresh tone/emphasis and say-as effects, source positions, context
restoration, grammar, skips, phoneme tails and publication agree. ABI guards
cover null/missing callbacks, failed word execution with retained effects and
initial output publication, altered primitive context restoration, invalid
source/number extents and invalid row bounds.

Fifteen Rust regressions cover emoji sequences/flags, number suffix context,
fresh digit mode, partial failure, missing spelling delimiters, cyclic
insertion, infinite foreign whitespace/continuation bytes, extreme foreign
scalars, phoneme capacity, delete-word effects, empty-clause sentence carry,
abbreviation tone and terminated voice publication. An unusually dense grouping
mask can consume the suffix reservation; the complete 17-byte reservation is
checked before copying, avoiding a panic in the exported controller. End-offset
validation also avoids unsigned overflow before pointer arithmetic. The first numeric test
expectation incorrectly joined an ordinal suffix to the digit token; C splits
it into the following token while retaining it in number context. The test now
checks that actual behavior. Initial strict lint failures and the corrected
test-only macro collision are retained in stage logs.

Serial validation passes with 420 enabled Rust tests (399 unit and 21
integration), 350 minimal tests, strict all-target/all-feature Clippy, formatting
and generated-table checks. Linux, Windows, iOS and Android library checks and
strict Windows library Clippy pass. After the voice-alias fix and helper
retirement, native async/static, shared and synchronous suites pass 84, 84 and
81 runnable CTests, each with physical audio skipped; audio-off passes 84,
retained-C configurations pass 20/19 and selected legacy checks pass 24. After
the last dense-number and extreme-offset guards, the complete native async
suite passes again (84 runnable), alongside the final Rust and cross-target
gates. The known proactor-off cancellation defect remains outside the selected
legacy checks.

A current-header archive probe substitutes the actual `translate.c.o` member
with the unchanged C clause controller, retaining other native controllers.
Current native/probe outputs match all 2,931 successful full-C CLI cases across
33 voices; 1,386 native/probe CLI traces match across those voices, and 25
offline WAV cases across five voices match full C/native/probe byte for byte.
Three known Finnish and three audited Arabic full-C braille overflow cases
remain excluded from successful full-C counts. Their current native/probe
outputs match previous native receipts; the full-C crashes are not rerun.

Fresh inventories count 586 C logic lines in `translate.c`, down from 1,098;
the measured synchronous/async configurations contain 6,975/7,445 C/mixed
lines. These counts include the new serialized projections and retain the
scanner/platform limits described in `REMAINING_PORT.md`. Evidence uses
`/private/tmp/espeak-clause-*`, including final `post-*` gate logs and renewed
CLI/trace/WAV receipts. Validation processes exited successfully. This is a
verified controller migration; engine/resource ownership, remaining C helpers,
process I/O, compilers, platform/tooling and retirement work remain in the full
goal. Offline synthesis does not establish physical playback or thermal safety.
Publication and final remote CI verification await restored automatic
approval-review access following its usage-limit failure; Jay's push approval
is retained.

### Complete character conversion policy, 2026-10-10

`rust/character_conversion.rs` owns complete `FindReplacementChars`,
`SubstituteChar`, `TranslateChar` and `UpperCaseInWord` policy. Ordered
replacement matching preserves the original candidate traversal, language
lowercasing, consumption of matched following characters, two-character output,
case rules and exact optional replacement traces. Hangul syllable and
compatibility-jamo paths bypass substitution state as before. Afrikaans/Dutch
apostrophe handling uses locale alphabet/space primitives; the Afrikaans
single-letter deletion remains a live source write. Irish prefixes use three
initialized owned predecessor bytes and fresh vowel classification.

Dictionary replacement bytes are projected numerically from the validated
retained dictionary allocation; missing, misplaced and truncated spans are
refused before dereference. Every resource/source scan advances within the
dictionary/840-byte clause bound. No foreign slice or scratch pointer spans a
callback. Case conversion uses the existing Rust Unicode tables with serialized
language/locale classifiers. Conversion adds no heap allocation, thread, sleep
or scheduler. Dictionary/translator and pending-consumption state still retain
serialized C owners; full engine ownership is separate remaining work. Failed
primitives retain executed effects and leave public result slots unchanged,
without replaying C.

Seven Rust regressions pass, covering ordered/mismatched multi-character rules,
case and insert handling, consumption across later calls, complete Hangul
syllable-range arithmetic, apostrophe source mutation, Irish prefixes, retained
effects on trace refusal and malformed/truncated or infinite continuation-byte
input. A 200,000-sequence unchanged-C fixture compares results, insert/flags,
full source mutations, subsequent consumed-character calls, exact trace bytes
and Irish uppercase decisions. An initial C test build lacked the dictionary
header. A coverage assertion then exposed correlated random low-bit choices;
using higher bits verifies nonzero coverage of the required branches rather
than weakening the assertion. Corrected comparisons pass with 51,816
replacements, 12,771 consumed characters, 86,027 nonzero inserts and 7,697
Irish uppercase matches. Additional null/missing callback, misplaced dictionary,
extreme source offset and partial-failure ABI guards pass in the serial gates.
All four retained policy bodies match the previous commit byte for byte, and
the configured oracle include matches current source.

Final serial gates pass 427 enabled Rust tests (406 unit and 21 integration),
357 minimal tests, strict all-target/all-feature Clippy, formatting/generated
checks, Linux/Windows/iOS/Android library checks and strict Windows library
Clippy. Native async/static, shared and synchronous suites pass 85/85/82
runnable CTests with physical audio skipped; audio-off passes 85, retained-C
configurations 20/19 and selected legacy checks 25. The known proactor-off
cancellation defect remains outside the selected legacy checks.

A current-header probe restores just the unchanged C character policy in the
actual `translate.c.o` archive member; the clause and other controllers stay
native. All 3,141 successful CLI outputs match full C/native/probe across 35
voices. The earlier 33-voice corpus is expanded with six character-specific
cases per voice; the added Afrikaans and Irish voices run those six focused
cases. Native/probe traces match 1,596 cases across this voice set. Offline WAV
outputs match full C/native/probe in 42 cases across seven voices, including
Afrikaans and Irish. Three known Finnish and three audited Arabic full-C
braille overflow cases remain excluded from successful full-C counts; current
native/probe outputs match their previous native receipts. The full-C crashes
are not rerun. An inherited CLI log summary still says clause driver; the
probe recipe and compilation verify that it restores character policy, and
the script label is corrected for future runs.

Fresh inventories count 485 C logic lines in `translate.c`, down from 586;
synchronous/async C/mixed totals are 6,874/7,344 with the existing scanner and
platform limits. Receipts, final gate logs, corrected fixtures and inventories
are under `/private/tmp/espeak-character-*`. Validation processes exited
successfully. Remaining text control, owned engine/resources, pending process
I/O, compilers, tooling/platform retirement and physical audio/thermal gates
stay in the full goal; offline synthesis does not prove physical playback or
thermal safety. Publication/final remote CI verification remain blocked by the
automatic approval-review service usage limit; Jay's push approval persists.

### Complete connected text control policies, 2026-10-10

`rust/text_control.rs` owns complete embedded command parsing/application,
word combination and live syllable counting, phoneme-language switching and
text initialization policy. Ordered primitive calls preserve partial source,
row, pronunciation, command and flag effects. Combination backs up only the
initialized pronunciation prefix and restores that prefix on rejection;
exceeding the syllable limit reverts the separator and translates again, retaining row
and grammar effects. Completed main-translation flags publish before later
checks. Nested rule contexts are scoped/restored around the retained primitive.

The command parser keeps case-sensitive lookup, sign/default handling, say-as
digit reset and emphasis effects, including the command-table NUL entry.
Word-command application preserves the original mandatory first read, signed
value extraction, live pause/say-as/emphasis effects and fresh command extent.
Source/row/list scans and foreign progress are bounded; unsupported decimal
overflow, missing terminators, overlong language names and extreme pause
arithmetic are refused with executed effects retained and no C replay.
Initialization preserves callback order, the namedata retention flag, marker
tails and effects written by the initialization primitives. The policy adds
no heap allocation, thread, sleep or scheduler; source/translator/resource and
global-state ownership remain serialized C work in the full goal.

Eight Rust regressions and 1,200,000 unchanged-C episodes pass (200,000 each
for six connected policy operations). They cover 164,674 embedded admissions,
including 16,468 NUL commands, 390,257 applied word commands, 184,591 nested
translations, 133,333 table selections and 300,000 reset primitives. The oracle
compares retained fixture state, initialized pronunciation/source mutations,
primitive order and meaningful name prefixes; it does not read undefined C
name-buffer tails. All six retained function bodies match the previous
commit byte for byte. Null/missing callbacks, invalid source/row bounds,
decimal overflow, partial flag publication and changed-context restoration
guards pass. The first C fixture build needed the retained pronunciation global
declaration. Checked decimal parsing and unsigned packing avoid undefined C
overflow; the oracle counts exclude such undefined input domains.

Final serial gates pass 435 enabled Rust tests (414 unit and 21 integration),
365 minimal tests, strict all-target/all-feature Clippy, formatting/generated
checks, four-target library checks and strict Windows library Clippy. Native
async/static, shared and synchronous suites pass 86/86/83 runnable CTests with
physical audio skipped; audio-off passes 86, retained C 20/19, selected legacy
26. The known proactor-off cancellation defect remains outside those selected
legacy checks.

A current-header archive probe restores all six unchanged C text-control
functions in the actual `translate.c.o` member, retaining other native
controllers. All 3,351 successful CLI outputs match full C/native/probe across
35 voices. Six additional text-control cases per voice cover prosody/digit mode,
embedded commands, phoneme-language switching and Czech/Hungarian combination
phrases. Added Afrikaans/Irish voices now run 12 focused character/text-control
cases each; the earlier 33 voices retain the broader corpus. Native/probe
traces match 1,806 cases, and full C/native/probe offline WAV outputs match in
49 cases across seven voices. Six recorded Finnish/Arabic full-C braille
overflow cases remain excluded from successful full-C counts; their current
native/probe outputs match previous native receipts, without rerunning crashes.

Fresh inventory counts 387 C logic lines in `translate.c`, down from 485;
synchronous/async C/mixed totals are 6,776/7,246 with the established scanner
and platform limits. Evidence is under `/private/tmp/espeak-text-control-*`.
Validation processes exited successfully. Alternate translator lifecycle,
owned engine/resources, remaining front-end projections, process I/O,
compilers, tooling/platform retirement and physical audio/thermal gates remain
in the full goal. Offline synthesis does not prove physical playback or thermal
safety. Publication/final remote CI await restored automatic approval-review
access after its usage-limit failure; Jay's push approval persists.

### Complete alternate-translator lifecycle policy, 2026-10-10

`rust/alternate_translator.rs` now owns `SetAlternateTranslator`,
`SetTranslator2`, `SetTranslator3` and `FreeAlternateTranslators` policy.
It admits the phoneme table before comparing the cached language, deletes and
clears a mismatched resource before creation, publishes only the terminated
name prefix before dictionary loading, and preserves initialized cache tails.
A failed load keeps the new translator, restores the freshly read main voice
table, clears only the first cache byte and assigns table -1. Existing matching
resources keep their table field; both admitted and unavailable languages reset
the repeat prefix of a live translator. Release preserves delete/clear/cache
order for each slot, including calls to the null-tolerant deleter.

The ABI copies a bounded language name before callbacks; no foreign name or
resource loan crosses effects. Checked slot/cache projections preserve live
primitive mutations. Overlong or embedded-NUL names, invalid operations and
missing callbacks fail before effects. Allocation failure stops before load
or dereference; failure preserves the public result and completed effects.
The unchanged C oracle excludes its undefined null-allocation/overflow input
domains. Production policy introduces no heap allocation, thread, scheduler or
sleep. Translator allocation, dictionary-cache/handle admission, cached language
storage and the serialized engine globals remain C integration work. Resident
dictionary bytes remain owned by the existing Rust cache; loading continues
through its proactor-backed reads.

Six Rust regressions and 200,000 retained-C episodes, each with 12 operations,
pass. The 2,400,000 operations include 1,600,224 table lookups, 1,293,970 resource
creations and dictionary loads, 2,162,586 deleter calls (852,914 with no resource)
and 258,727 recovery table selections. The test compiles the actual projections
and unchanged retained lifecycle bodies, comparing results, both retained
translator objects, initialized cache bytes, voice/table state and ordered
primitive observations after every operation. Pointer identity is compared as
fixture object indexes; no uninitialized padding or undefined scratch is read.
The four retained bodies match the previous commit byte for byte. Null/missing
ABI callbacks, invalid name/slot/cache bounds and allocation-failure partial
effects are checked separately. The first fixture build used `assert` without
its header; it was corrected to the repository's always-enabled `TEST_ASSERT`.

This checkpoint also corrects the previous text-control prose count from seven
to six functions; that stage's six operations and test receipts are unchanged.

Serial offline gates pass 441 enabled Rust tests (420 unit and 21 integration),
371 minimal tests, strict all-target/all-feature Clippy, format/generated-table
checks, four-target library checks and strict Windows library Clippy. Native
async/static, shared and synchronous suites pass 87/87/84 runnable CTests;
physical audio is skipped. Audio-off passes 87, retained C 20/19 and selected
legacy 27. The known proactor-off cancellation defect remains open and excluded
from those selected legacy gates. Cross-target checks establish compilation,
not target execution.

A current-header archive probe restores the four unchanged lifecycle functions
in the actual `translate.c.o` member, leaving other controllers native. All
3,561 successful CLI outputs match full C/native/probe across 35 voices.
Six new cases per voice exercise repeated phoneme-language switches,
mixed-script spelling, Unicode digits and repeated SSML voice changes.
The earlier 33 voices run 107 cases each before the six recorded crash
exclusions; Afrikaans/Irish run 18 focused cases each. Native/probe traces match
2,016 cases. Full C/native/probe match 70 offline WAV cases across seven voices,
including mixed-script spelling and voice changes. The six recorded
Finnish/Arabic full-C braille overflow cases remain excluded; current
native/probe outputs match previous native receipts, without rerunning crashes.
The Finnish helper's completed log retains its previous controller label;
executed binary paths and JSON labels identify the current lifecycle probe,
and the helper's future status label is corrected. The probe's two unused
test-only C helpers (`CalcWordLength`, `ShouldSplitIdeographs`) produce warnings,
not production build failures.

Fresh inventories count 394 C logic lines in `translate.c` (92 C, 0 mixed,
13 bridges), with synchronous/async totals 6,783/7,253. The seven-line net
increase reflects checked resource projections replacing the old policy;
line counts alone do not measure migration progress. All 29 current appendix
rows match the fresh synchronous inventory. Existing platform/configuration
and C++ method-scanner limits remain. Evidence is under
`/private/tmp/espeak-alternate-*`; the serial gate process exited with status 0.

macOS `pmset -g therm` returned unavailable warning/performance/CPU power
status (`0xe00002bc`). Builds and tests used one job/thread; this is not proof
of thermal safety. No fresh physical playback or representative idle/active
hardware measurement is claimed. Full owned engine/translator integration,
remaining projections/control, process I/O and MBROLA flush, path discovery,
playback cancellation, compilers, tooling/platform retirement and hardware
gates remain in the goal. Publication/final remote CI still await restored
automatic approval review after its usage-limit block; Jay's push approval
persists.

### Typed translator storage and dictionary ownership, 2026-10-10

`rust/translator.rs` owns the complete typed translator record: fallible
allocation, initialization from native language profiles, dictionary views and
index publication, unique pinned snapshot handles, replacement and destruction.
The native `SelectTranslator` and `DeleteTranslator` paths now use that owner.
`LoadDictionary` admits file/cache work through its existing proactor-backed
reader, then calls Rust to bind the completed snapshot. Its C index-copy and
handle-release implementation (`InitDictionary`, `RustOffset`) is removed.
Every published pointer is checked against initialized pinned bytes; replacement
publishes views before releasing the previous handle. Failure preserves owner
views and its meaningful dictionary-name prefix. Name input may alias the
owner's field: the ABI copies it before I/O and acquires no Rust translator
borrow while the proactor runs.

The record is a typed `repr(C)` Rust allocation, not an opaque byte buffer.
Static profile tables and dictionary pointers are non-owning views; the
snapshot handle pins the immutable bytes. Zeroed allocation initializes every
runtime array and avoids a large temporary stack copy. Large dictionary storage
continues through the existing bounded reusable cache. The two small allocations
(record and snapshot handle) check allocation failure before publication; actual
allocator exhaustion has not been injected. No thread, timer, sleep or spin
loop is added. Internal native records must come from the Rust constructor;
stack or C-allocated oracle records cannot be passed to its destructor. C-only
builds keep their original allocation/destruction path.

Five new Rust regressions pass for construction/limits, shared pins across cache
teardown, replacement/release, preserved grammar and rejected rebinding. The C
test checks all 54 translator and 39 language-option field offsets, both sizes
and alignments, null/invalid ABI inputs, aliased names, shared bytes/distinct
handles, failed replacement and survivor views after cache/peer destruction.
The data fixture now creates native Rust records; its unchanged C index oracle
still uses separate C storage. All 123 dictionary index sets match. The language
fixture additionally compares the complete initialized Rust record with the
previous C setup projection for all 18,385 presets; existing meaningful-field
checks against retained C and 1,114,469 alphabet classifications also pass.
Whole-record comparison covers explicitly initialized native storage/padding,
not undefined padding or unused tails of the old malloc-based C oracle.

Initial strict lint checks found an unsafe-comment placement and a layout helper
following the test module; both were corrected before final gates. C still
retains alternate slots/cached language names, global dictionary-cache admission,
path construction/diagnostics, engine globals and source/state projections.
Typed allocation does not complete owned engine instances or retire the C build.

Final serial offline gates pass 446 enabled Rust tests (425 unit and 21
integration), 376 minimal tests, strict all-target/all-feature Clippy,
format/generated checks, four-target library checks and strict Windows library
Clippy. Native async/static, shared and synchronous suites pass 88/88/85
runnable CTests with physical audio skipped; audio-off passes 88, retained C
20/19 and selected legacy 30. The known proactor-off cancellation defect remains
excluded from the selected legacy gates. The gate process exited with status 0.
Layout/runtime evidence is from macOS; cross-target library checks establish
compilation, not C-layout verification or execution on other platforms.

The current-header archive probe restores previous C allocation/setup,
dictionary index/handle rebinding and destruction from `8de2fd57` in all three
actual members (`tr_languages.c.o`, `dictionary.c.o`, `translate.c.o`), retaining
other native controllers. All 3,561 successful CLI outputs match full
C/native/previous-owner probe across 35 voices; af/ga retain 18 focused cases
each. Native/probe traces match 2,016 cases, and full C/native/probe match 70
offline WAV cases across seven voices. Six recorded Finnish/Arabic full-C
braille traps remain excluded; native/probe match previous native receipts,
without rerunning crashes. The probe exposes two unchanged unused test helpers;
their warnings are separate from production gates.

Fresh dictionary C/mixed logic falls from 630 to 587 lines (34 C, 6 mixed,
12 bridges); `translate.c` remains 394 C logic lines. Native constructor and
destructor forwarding bodies shrink, but individual bridge lengths are omitted
from the markdown appendix. All 29 rows match the fresh inventory, with only the
dictionary row changed. Current synchronous/async C/mixed totals are 6,740/7,210,
and text-front-end items 1–7 total 1,940. Platform/configuration and C++ method
scanner limits remain. The first inventory-update helper expected bridge-length
changes in markdown and stopped before writing; its assertion was corrected
against actual fresh output. Evidence is under `/private/tmp/espeak-translator-*`.

No fresh physical playback or representative idle/active thermal proof is
claimed. Builds/tests ran one job/thread; the earlier macOS thermal-status
query was unavailable. Full owned engine integration, source/control projections,
process I/O/MBROLA flush, path discovery, playback cancellation, compilers,
tooling/platform retirement and hardware gates remain. Publication/final remote
CI await restored automatic approval review after its usage-limit block;
Jay's push approval persists.

## Connected native rule-host projections, 2026-10-10

`rust/translate_rules_host_compat.rs` now supplies the existing complete rule
controller directly from typed translator storage. Rust owns source admission
and current/main-owner scoping, checked signed pointer/byte access, fresh
translator/global/flag projections, bounded dictionary groups, ending presence
and pronunciation/ending publication. `Translator` keeps its resource fields
private and exposes only the short scalar/group operations needed by this host.
Eight C helpers are removed; the native primitive table shrinks from thirteen
callbacks to six. The generic thirteen-callback ABI and its independent
retained-C controller oracle remain supported.

Admission preserves the initialized predecessor, NUL-terminated source contract,
strict parent membership and current-owner-before-main-owner order. The matcher
keeps the original string extent even when source access admits a larger parent
extent. Signed offset arithmetic rejects overflow before deriving a pointer
with the original source provenance. Output publication validates initialized
owned scratch and copies only its meaningful prefix plus NUL. Scalar/resource
reads are fresh after primitive effects; no translator/source/output reference
survives a nested call. The host metadata is owned on the Rust stack and borrowed
read-only by remaining C primitives. No allocation, thread, timer, sleep or spin
loop is added.

Remaining effects are CRT classification, matching, symbol/letter lookup,
phoneme append and trace output. Match pronunciation/deletion conversion, append
admission, the VM environment, engine globals and other front-end projections
still require integration work. This is a connected host replacement, not a
complete owned engine instance or retirement of the C dependency.

Three new Rust regressions cover signed source bounds/predecessor access,
publication limits/prefix-only writes and current translator/group projections,
including invalid indexes, dictionary end/before-base pointers and target C-char
signedness. The new C fixture compares actual previous production host code
from `244ef3b6` with the current native host over the same primitive effects.
Its frozen oracle matches the previous source byte for byte after its provenance
comment. All 200,000 episodes match source/output/state and primitive order,
including 64,432 symbol calls, 6,580 letter calls and 122,863 ending returns.
Separate guards verify current/main source selection, unchanged matcher extent,
extreme offsets and invalid metadata/table/result admission. A missing fixture
forward declaration stopped the first broad C build; it was corrected before
the successful remaining gates, with the failed log preserved.

Serial offline gates pass 449 enabled Rust tests (428 unit and 21 integration),
376 minimal tests, strict all-target/all-feature Clippy, format/generated
checks, four-target library checks and strict Windows library Clippy. Native
async/static, shared and synchronous suites pass 89/89/86 runnable CTests;
physical audio is skipped. Explicit audio-off passes 89, retained C 20/19 and
selected proactor-off 31. The known proactor-off cancellation defect stays open
and excluded from that selected set. The resumed gate process exited with
status 0. Cross-target checks establish compilation, not target runtime or
C-layout validation outside macOS.

The archive probe restores previous `dictionary.c` from `244ef3b6` in the actual
dictionary object member, using current headers and Rust archive. Only its local
host typedef is renamed to avoid the new header type; previous function bodies
remain unchanged. All 3,561 successful full-C/native/previous-host CLI cases
match across 35 voices, including 18 focused cases each for af/ga. Native/probe
traces match 2,016 cases; full C/native/probe offline WAV output matches 70 cases
across seven voices. Six recorded Finnish/Arabic full-C braille traps remain
excluded; native/probe output matches prior receipts without rerunning crashes.

Fresh dictionary C/mixed logic falls from 587 to 504 lines (26 C, 5 mixed,
13 bridges). Synchronous/async totals are 6,657/7,127 and text-front-end items
1–7 total 1,857. All 29 appendix rows match fresh inventory; only dictionary
changes. The first update helper counted an unrelated table row and stopped
before writing; scoping it to the appendix fixed that validation. Scanner,
platform and configuration limits remain. Evidence and serial
recipes are under `/private/tmp/espeak-rule-host-*`. Local loadngo `dev` history
still has no proactor change after the existing `843ae1de` pin, which includes
both reused-HANDLE repair `e421f5ea` and IOCP deadline repair `84b2b6a8`; no
network refresh or dependency change was performed in this slice.

Physical playback and representative idle/active thermal proof remain open;
single-job/thread execution is discipline, not thermal-safety evidence. Full
engine/global/source integration, remaining control/projections, process I/O
and MBROLA flush, path discovery, cancellation, compilers, tooling/platform
retirement and hardware gates remain. Publication and final remote CI await
restored automatic approval review after its usage-limit block. Jay's push
approval is retained; no unchanged blocked request was repeated.

## Typed native rule-matcher admission and environment, 2026-10-10

`rust/rule_match_host_compat.rs` replaces the native C matcher adapter's
dictionary admission, current/main source scoping, context/letter projection,
string-letter-group predicate and result/pointer publication. The existing
bounded Rust rule VM and generic matcher ABI remain supported. Translator
resource fields stay private; four short typed methods supply dictionary,
pattern, context and letter metadata. The C `RustMatchPredicate` is removed
and `espeak_rs_match_rule` becomes a forwarding bridge.

Missing groups preserve all meaningful record fields except points and advance
one byte. Checked invalid groups publish the default record and advance one;
VM failure retains default output, legacy progress and executed effects without
replay. Negative widths retain their failed-VM one-byte progress. Native output
uses field stores without reading undefined record padding or uninitialized
missing-group fields. Source scoping retains current-before-main order, strict
membership, the predecessor and the exact matcher end extent, rather than the
parent's unused capacity. Checked numeric offsets are converted back to pointers
with their original source/dictionary provenance.

Prefix flags and diagnostic trace remain C effects. The native matcher owns
read-only host metadata and scalar context; no whole-translator reference spans
effects. The VM retains immutable resident dictionary, source and language
views through those calls. Effects may change unrelated counters but must not
rebind/mutate the admitted resources or retain/mutate metadata or input loans.
The current prefix effect is ordinary list lookup, without symbol translation.
Native typed ownership remains serialized by the engine integration contract;
this does not finish owned engine instances or remove the C build. No heap
allocation, worker, timer, sleep or spin loop is added.

Three new Rust regressions verify absent-group partial publication, numeric
source/cursor bounds, dictionary pattern bounds and typed scalar/configuration
projections, including bit preservation and platform plain-char/wchar_t widths.
The expanded C fixture passes 886,890 scalar letter comparisons across 123
profiles, 46,812 real matcher executions across 11 languages and 59,400 synthetic
opcode executions against unchanged retained C. Trace comparisons are retained.
New guards cover null metadata/slots/effects, missing effects/translator/source,
zero extent, absent/invalid groups and negative widths. Ten additional retained-C
comparisons use a distinct main translator's clause extent for cross-word PRE
access. The first strict lint pass required the safety comment immediately next
to a test's unsafe call; the call was moved to a named result before final gates.

Serial offline gates pass 452 enabled Rust tests (431 unit and 21 integration),
376 minimal tests, strict all-target/all-feature Clippy, format/generated checks,
four-target library checks and strict Windows library Clippy. Native async/static,
shared and synchronous suites pass 89/89/86 runnable CTests; physical audio is
skipped. Explicit audio-off passes 89, retained C 20/19 and selected proactor-off
32, now including the matcher fixture. The known proactor-off cancellation
defect stays open and excluded. The gate process exited with status 0. Target
checks establish compilation; target runtime and C-layout validation outside
macOS remain open.

The current-header archive probe restores previous `dictionary.c` from
`39958b8b` in the actual dictionary object member, retaining current Rust VM
and other controllers. Its local matcher-host typedef alone is renamed to
avoid the new header type; previous function bodies remain unchanged. All
3,561 successful full-C/native/previous-host CLI cases match across 35 voices,
including 18 focused cases each for af/ga. Native/probe traces match 2,016 cases;
full C/native/probe offline WAV output matches 70 cases across seven voices.
Six recorded Finnish/Arabic full-C braille traps remain excluded; native/probe
output matches prior receipts without rerunning crashes.

Fresh dictionary C/mixed logic falls from 504 to 468 lines (26 C, 4 mixed,
13 bridges); sync/async totals are 6,621/7,091 and text-front-end items 1–7 total
1,821. All 29 appendix rows match fresh inventory; only dictionary changes.
Scanner/platform/configuration limits remain. Evidence and serial recipes are
under `/private/tmp/espeak-match-host-*`.

Physical playback and representative idle/active thermal proof remain open;
serial execution is discipline, not thermal-safety evidence. Full engine/global/
source integration, remaining control/projections, process I/O/MBROLA flush,
path discovery, cancellation, compilers, tooling/platform retirement and hardware
gates remain. Publication and final remote CI still await restored automatic
approval review after its usage-limit block. Jay's push approval is retained;
no unchanged blocked network request was repeated.

## Native match-result conversion (2026-10-10)

`rust/rule_conversion_compat.rs` replaces the remaining C `RulesMatched`
conversion policy. Rust admits width, group and initialized numeric input
positions, invokes one raw matcher primitive, checks its returned cursor and
converts deletion pointers to signed source offsets. Nonempty dictionary
pronunciation is copied into the existing owned 200-byte scratch only after
bounded termination checks. Null/empty and same-scratch results preserve their
previous behavior. Cursor, score and ending publish before deletion and
pronunciation validation; failed later checks retain this partial publication.
A failed primitive keeps its executed scratch/source/state effects without
publishing converted scalars or replaying the call. No translator, source or
foreign dictionary reference survives the nested primitive.

The six-slot native effect table changes its matched slot to the raw cursor/
record signature. The thirteen-slot generic controller ABI stays unchanged.
The C matched adapter only forwards to the native matcher; no allocation,
thread, timer, sleep or spin loop is added. The helper's C/Rust contract records
readable first-byte requirements even for foreign empty pronunciation.

The actual previous C host fixture passes 200,000 comparisons, including
callback order and state effects. Thirteen focused conversion cases cover
outside and before-source cursors, invalid deletion pointers, positive/negative
deletions, foreign nonempty and empty pronunciation, dictionary termination
failure, null pronunciation, same-scratch copying and unterminated same-scratch
results. Separate null/table/input guards verify admission has no effects;
a failed raw callback retains one execution and scratch writes. The oracle
remains the production host frozen from `244ef3b6`; match field comparisons
exclude C struct padding.

Serial offline gates pass 452 enabled Rust tests (431 unit and 21 integration),
376 minimal tests, strict all-target/all-feature Clippy, format/generated checks,
four-target library compilation and strict Windows library Clippy. Native
async/static, shared and synchronous suites pass 89/89/86 runnable CTests;
physical audio is skipped. Explicit audio-off passes 89, retained C 20/19 and
selected proactor-off 32. The known proactor-off cancellation defect stays open
and excluded. The complete gate process exited 0; the final async host fixture
was rebuilt and passed after removing struct-padding comparisons. Target
runtime and C-layout validation outside macOS remain open.

The archive probe restores `92c9a8d0` dictionary source with the actual native
C rule-host block from `244ef3b6`, using the unchanged generic thirteen-slot
controller ABI. Its host typedef alone is renamed for the current header;
function bodies are unchanged. This restores both previous C projections and
conversion, while retaining the current Rust matcher and other controllers.
The frozen fixture was verified byte-for-byte against that original block.
All 3,561 successful full-C/native/probe CLI cases match across 35 voices;
2,016 native/probe traces and 70 full-C/native/probe offline WAV cases across
seven voices match. Six recorded Finnish/Arabic full-C braille traps remain
excluded; current native/probe matches prior receipts without rerunning crashes.

Fresh dictionary C/mixed logic falls from 468 to 437 lines (26 C, 3 mixed,
14 bridges). Sync/async totals are 6,590/7,060; text-front-end items 1–7 total
1,790. All 29 appendix rows match fresh inventory; only dictionary changes.
Scanner/platform/configuration limits remain. Evidence and serial recipes use
`/private/tmp/espeak-rule-conversion-*`.

Physical playback and representative idle/active thermal proof remain open.
`pmset -g therm` could not obtain thermal/performance/CPU status (0xe00002bc);
serial execution does not establish thermal safety. Full engine/global/source
integration, remaining control/projections, process I/O/MBROLA flush, path
and cancellation, compilers, tooling/platform retirement and hardware gates
remain. Publication and final remote CI await restored automatic approval
review after its usage-limit block; Jay's push approval remains recorded.
No unchanged blocked network request was repeated.

## Native dictionary lookup environment (2026-10-10)

`rust/lookup_host_compat.rs` now prepares native alphabet keys, snapshots bounded
word records and grammatical/main-translator context, checks resident bucket
bounds, reads the at-most-255-byte next-word window and publishes attempted
pronunciation, skip counts and selected flags in the previous order. A NUL
predecessor admits a one-past next-word pointer without reading it. Missing
buckets clear only the first flag; malformed buckets preserve prior outputs;
rejected records retain their attempted pronunciation/skip effects. Typed
translator methods keep resource/state fields private. Alphabet/pair projection
and bounded sentinel admission also move to Rust; the C transposer forwards.

Diagnostic admission and the owned bounded multiword snippet are Rust. Locale
classification and flag/phoneme formatting remain primitive callbacks. No
foreign translator/source/dictionary reference survives either callback. Owned
161-byte scratch is reused for key preparation and trace; transposed input copy
stops at 159 bytes instead of scanning a longer key. No allocation, thread,
timer, sleep or spin loop is added. Bucket and transpose generic ABIs remain.

The fixture compares previous native host function bodies frozen verbatim from
`442aecb4`, retained C and the current Rust path: 20,000 randomized grammar/flag
lookups, 20,800 real dictionary lookups and 50,000 map/pair/hash-tail transpositions.
Trace bytes, flags, pronunciation, returned pointers and skip counts match.
C WORD_TAB size/field offsets are checked against Rust's typed view. Ten focused
previous-native cases cover absent/invalid/truncated buckets, attempted-copy
side effects, a 255-byte window, one-past next source, negative/clamped word
counts and absent main translator. Null/table/output and missing-pair-sentinel
guards pass. Two Rust regressions cover grammar bits/clause bounds and absent
bucket/early-admission output preservation. Initial C classifier signedness,
Rust cast parsing and strict lint findings were corrected before final gates.

The known-capacity diagnostic adapter now calls the bounded phoneme decoder
with its actual 200-byte capacity. A 159-byte valid pronunciation whose mnemonic
expansion exceeds that capacity still publishes speech and flags, and safely
emits an empty diagnostic mnemonic. This guard is native-only: the unbounded
previous C trace is not executed on that unsupported footprint. Normal trace
bytes remain subject to the three-way and archive-probe comparisons.

Final serial offline gates pass 454 enabled Rust tests (433 unit and 21
integration), 376 minimal tests, strict all-target/all-feature Clippy, format/
generated checks, four-target library compilation and strict Windows library
Clippy. Native async/static, shared and synchronous pass 89/89/86 runnable
CTests; physical audio is skipped. Explicit audio-off passes 89, retained C
20/19 and selected proactor-off 33, now including contextual lookup. The known
proactor-off cancellation defect stays open and excluded. The final gate process
exited 0. Target runtime/C-layout coverage outside macOS remains open.

The archive probe restores `442aecb4` dictionary.c unmodified in the actual
object member using current headers and Rust matcher/controllers. All 3,561
successful full-C/native/probe CLI cases match across 35 voices, 2,016
native/probe traces match, and 70 full-C/native/probe offline WAVs match across
seven voices. Recorded Finnish/Arabic full-C braille traps remain excluded;
native/probe matches their prior receipts without rerunning crashes. Final
receipts use fresh bounded-build paths, superseding the initial gate run before
the known-capacity diagnostic fix.

Fresh dictionary C/mixed inventory falls from 437 to 370 lines (26 C, 2 mixed,
16 bridges); sync/async totals are 6,523/6,993, frontend items 1–7 total 1,723.
All 29 appendix rows match fresh inventory; only dictionary changes. The short
C trace formatter calls the bounded Rust decoder and is classified as a bridge
by the scanner; diagnostic formatting itself remains C. Counts exclude bridge
logic and retain scanner/platform/configuration limits. Evidence and serial
recipes use `/private/tmp/espeak-lookup-host-bounded-*`.

Full engine/global/source integration, remaining control/projections, process
I/O/MBROLA flush, path/cancellation, compilers, tooling/platform retirement and
physical playback/representative thermal validation remain. Serial execution
does not prove thermal safety. Publication and final remote CI await restored
automatic approval review after its usage-limit block. Jay's approval is retained;
no unchanged blocked network action was repeated.

## Native dictionary-list, symbol and prefix host environment

`rust/list_host_compat.rs` now supplies the native host directly to the existing
Rust list/symbol controllers. Private typed translator methods project repeat
storage/count, text mode, replacement storage and source metadata. Rust owns
registered-source admission (including its starting address), checked bytes and
returned match pointers, skip/say-as globals, replacement cursor publication,
trace admission/snippet copying and nested translation source restoration. Rule
prefix queries preserve their previous terminated-source admission independently
of the registered rule extent. Native wrappers retain only four primitive
effects: raw dictionary lookup, accent lookup, word translation and trace
printing. Generic nine-/thirteen-slot controller ABIs stay supported.

Nested translation receives initialized, padded 80-byte owned text. A guard
restores the previous rule source after the child and pronunciation copy, even
when that child changed the metadata. No foreign owner/source/output reference
spans an effect. Pronunciation stays in initialized 200-byte owned scratch until
validated publication; callers can use the engine's global child pronunciation
buffer as output without lending it through the child. Executed effects survive
failure, with the native adapters' first-byte/two-flag clearing behavior and no
replay. These bounded CPU projections introduce no allocation, thread, timer,
sleep or polling scheduler.

The new native host fixture freezes the actual three previous adapter blocks
from `6541d604` and extracts current production effect/wrapper bodies during
CMake configuration. All 600,000 previous/current episodes pass (200,000 each
list, symbol and prefix), comparing full output/tails, flags, repeat/replacement
storage, cursor, state, effect order/hash, trace contents and source restoration.
Eighteen focused malformed cases, early native guards and global-output aliasing
also pass. Four new Rust regressions cover registered-versus-prefix admission,
child restoration and pronunciation prefix copying, full typed storage/cursor
bounds and early/missing-effect publication. Frozen-body provenance is checked
against the previous commit. Initial safety-comment/header findings and an alias
test that reused mutated state were corrected before the final serial gates.

Serial offline gates pass 458 enabled Rust tests (437 unit and 21 integration),
376 minimal tests, strict all-target/all-feature Clippy, format/generated checks,
four-target library compilation and strict Windows library Clippy. Native
async/static, shared and synchronous pass 90/90/87 runnable CTests; physical
audio is skipped. Explicit audio-off passes 90, retained C 20/19 and selected
proactor-off 34, including the new host fixture. The known proactor-off
cancellation defect stays open and excluded. Target runtime/C-layout validation
outside macOS remains open.

The archive probe restores `6541d604` dictionary.c in the actual object member
with only its local `RustListHost` type renamed to avoid the new header's type;
old function logic remains unchanged. Current headers and Rust controllers
are retained. All 3,561 successful full-C/native/probe CLI cases match across 35
voices, 2,016 native/probe traces match and 70 full-C/native/probe offline WAVs
match across seven voices. Recorded Finnish/Arabic full-C braille traps remain
excluded; native/probe results match their prior receipts without rerunning
crashes. The complete serial gate process exited 0.

Fresh dictionary C/mixed inventory falls from 370 to 335 lines (18 C, 2 mixed,
16 bridges); sync/async totals are 6,488/6,958, frontend items 1–7 total 1,688.
All 29 appendix rows match fresh inventory; only dictionary changes. The
remaining C trace formatter/global-reference factory and four primitive effects
are still present. Counts exclude bridge logic and retain scanner/platform/
configuration limits. Receipts, recipes and final logs use
`/private/tmp/espeak-list-host-*`.

The full engine migration and platform/physical-audio/representative thermal
gates remain open. A coarse `pmset -g therm` query returned unavailable/error
results (`0xe00002bc`); it provides no current thermal reading. Serial execution
does not establish thermal safety. Publication/final remote CI await restored
automatic approval review after its usage-limit block; Jay's approval is retained.

## Native letter lookup host environment

`rust/letter_host_compat.rs` now connects the letter/accent controllers directly
to serialized typed translator and acoustic voice state. Rust selects the fresh
secondary translator, reads language/accent policy between effects, scopes and
restores the 10-byte owned rule source, and restores the current voice table
after secondary lookup. The global secondary/voice pointer slots are read at
the point of use, including when a child replaces them. A missing secondary
clears only the scratch head and avoids lookup; a missing voice during restoration
fails atomic output publication while preserving completed effects. No foreign
owner/output reference spans a callback. Mutable owned source pointers are
derived once for metadata and child access; named keys remain readonly.

The native wrappers retain six effects: raw symbol lookup, platform whitespace
classification, raw rules, default-secondary setup, phoneme-table selection and
word stress. Private translator fields remain encapsulated, and the generic
seven-callback controller ABI remains supported. The source guard is shared
with the native list/symbol host, retaining nested child metadata restoration.
This bounded CPU handling adds no allocation, thread, timer, sleep or scheduler.

The new fixture freezes the actual native block from `81ab4ecf` numbers.c and
extracts current production wrappers/effects at configuration time. It compares
200,000 previous/current episodes and 500 malformed-output episodes with full
pronunciation/tails, typed translator/voice state, effect order/hash, changed
secondary/voice slots, owned-source mutation and rule metadata restoration.
These pass, as do early null/capacity guards and a native-only missing-voice
publication guard; the old adapter's undefined null dereference is not executed.
Three Rust regressions cover fresh/absent secondary, prefix-preserving child
rules/source restoration and fresh/missing voice state. Frozen-body provenance
matches the prior commit.

Complete serial offline gates pass 461 enabled Rust tests (440 unit and 21
integration), 376 minimal tests, strict all-target/all-feature Clippy, format/
generated checks, Linux/Windows/iOS/Android library compilation and strict
Windows library Clippy. Native async/static, shared and synchronous pass
91/91/88 runnable CTests; physical audio is skipped. Explicit audio-off passes
91, retained C 20/19 and selected proactor-off 35, including the new fixture.
The known proactor-off cancellation defect remains open and excluded from that
selected set. Target runtime/C-layout evidence outside macOS remains open.

The archive probe restores `81ab4ecf` numbers.c unmodified in the actual
numbers.c.o object member, retaining current headers and Rust controllers.
All 3,561 successful full-C/native/probe CLI cases match across 35 voices,
2,016 native/probe traces match and 70 full-C/native/probe offline WAVs match
across seven voices. Recorded Finnish/Arabic full-C braille traps remain
excluded; current native/probe matches previous receipts without rerunning
crashes. The complete gate process exited 0.

Fresh numbers C/mixed inventory falls from 122 to 106 lines (27 C, 6 bridges);
sync/async totals are 6,472/6,942, measured sync frontend items 1–7 total 1,672.
All 29 current appendix rows match fresh inventory; only numbers changes.
Counts exclude bridge logic and retain scanner/platform/configuration limits.
Evidence, final logs, recipes and provenance use `/private/tmp/espeak-letter-host-*`.

Full engine integration, remaining projections/control, process I/O/MBROLA
flush, path/cancellation, compilers, tooling/platform retirement and real-device/
representative thermal gates remain. The coarse thermal query again returned
unavailable/error results (`0xe00002bc`), not a current reading. Publication and
final remote CI await restored automatic approval review after its usage-limit
block; Jay's push approval remains held.

## Native phoneme mnemonic encoding

`rust/phoneme_encode.rs` owns longest mnemonic matching, first-entry ties,
sparse/invalid table admission, single/double bar handling, language switch
lowercasing and default-language suppression. The allocation-free immutable
plan preserves the one-byte error cursor and the complete written footprint,
including language bytes left beyond an early NUL. Platform byte classification
is snapshotted before foreign strings are borrowed; table records are copied.
No callback occurs while planning or publishing, and output tails are not read.

`EncodePhonemesBounded` rejects short buffers before publishing output or the
optional bad-phoneme integer. Compiler list/rule buffers and native runtime
word/spelling buffers forward their actual capacities and propagate rejection.
The extent-free legacy wrapper still requires the caller to own the complete
written footprint. Inputs/outputs are disjoint; detected string/output/integer
aliasing is rejected. Retained C is preserved for C-only builds, and the new
fixture freezes the actual `98f1dc3c` encoder with unchanged body and verified
provenance. 251,021 C/native/bounded comparisons pass with full output tails,
bad integers and returned cursors, plus 251,021 atomic short-buffer guards.
Cases cover sparse/invalid/zero mnemonics, first ties, all unsigned input bytes,
four mnemonic widths, permissive/truncated UTF-8, language switches, separators
and long names. Native-only null/count/callback/string and output/integer alias
guards pass; these rejected cases are not run through undefined C operations.
Four Rust regressions cover matching, default-language tails, error cursors and
initial ASCII/platform space distinctions.

Complete serial offline gates pass 465 enabled Rust tests (444 unit and 21
integration), 380 minimal tests, strict all-target/all-feature Clippy, format/
generated checks, four target library compilations and strict Windows library
Clippy. Native async/static, shared and synchronous pass 92/92/89 runnable CTests;
physical audio is skipped. Explicit audio-off passes 92, retained C 20/19 and
selected proactor-off 36. The known proactor-off cancellation defect remains
open/excluded; target runtime and C-layout evidence outside macOS remain open.

The archive probe restores `98f1dc3c` dictionary.c in the actual dictionary.c.o
member with its encoder body unchanged, plus an extent-forwarding compatibility
shim routing the new bounded call sites to that old encoder. The shim is used
only for valid fitting corpus/compiler inputs, not malformed-capacity checks.
All 3,561 full-C/native/probe CLI cases across 35 voices, 2,016 native/probe
traces and 70 full-C/native/probe offline WAVs across seven voices match.
Recorded full-C braille traps remain excluded; current native/probe matches
previous receipts without rerunning crashes. Actual compiler output also matches
all three lanes for normal/bar/language-switch/near-boundary list input and
79-byte phoneme rule input; all 123 built dictionary payloads match full C.
Complete gate session 29078 and the subsequent compiler verification exited 0.

Dictionary C/mixed logic falls from 335 to 268 lines. Compiler admission adds
eight C lines and native word admission one: net measured reduction is 58,
with sync/async totals 6,414/6,884 and sync frontend items 1–7 at 1,606.
All 29 generated appendix rows match fresh inventory; only dictionary, compiler
and word-admission counts change. Counts retain
bridge/scanner/platform/configuration limits. Evidence, provenance, recipes and
final logs use `/private/tmp/espeak-phoneme-encode-*`. The local loadngo dev
proactor has no diff after the existing `843ae1de` dependency pin, which includes
the Windows HANDLE reuse repair `e421f5ea`; no remote refresh was possible here.

Full engine integration, remaining projections/control, process I/O/MBROLA
flush, path/cancellation, compilers, tooling/platform retirement and real-device/
representative thermal gates remain. The coarse thermal query returned unavailable
errors (`0xe00002bc`), not a current reading. Publication/final remote CI remain
blocked by the automatic approval review usage limit; Jay's push approval is held.

## Native phoneme text rendering

`rust/phoneme_render.rs` owns mnemonic/IPA/stress writing and complete translated
clause text assembly. The selected C interpreter and alphabetic classifier are
primitive effects. Rust preserves first/variant/digit/pause/vowel spelling,
stress levels, language table names, word/phoneme separators, tie insertion,
length/syllabic/tone suffixes and the existing reset of IPA flags before ties.
List metadata and table slots are reread after interpretation effects; resolved
tone records take precedence over the current selected table.

The native list bridge reads only initialized needed metadata, retaining standalone
stress/switch nodes with unrelated fields uninitialized. Owned 64-byte spelling
and 384-byte node scratch support the complete bounded resource domain. One
reusable 384,385-byte clause buffer replaces incremental malloc/realloc growth;
its pointer remains valid until the next serialized render. Nested rendering is
refused via try_lock without blocking/spinning or changing the outer buffer.
Phoneme records must remain fully initialized, immutable and alive during effects.
Native alignment/marker buffers are 64 bytes and forward their actual capacities.
Rejected writer publication preserves output/flags and retains executed effects;
the extent-free compatibility functions require the complete caller footprint.
Nonterminating signed mnemonic shifts and unterminated resource names are refused.
No allocation, thread, timer, sleep or scheduler is introduced by rendering.

The fixture freezes the actual `392b5575` C renderer and extracts current production
effects/wrappers. 161,020 writer and 20,000 clause comparisons pass, including
effect order, changed list fields/table slots, complete writer tails and fresh
clause bounds. Every unsigned mnemonic byte and the full 96-entry IPA map are
covered. Native-only unterminated IPA/table names, signed nontermination, short
capacity, long table names, partial standalone nodes, reentry and maximum 1001-node
guards pass. Six Rust regressions include all stress levels, fresh metadata, tie
flags and suffix assembly. An added actual-C comparison seeds both output buffers
then verifies permissive UTF-8 continuation tails preserve previously written
bytes beyond the first NUL; the copied prefix and advanced cursor differ in C.

The complete serial offline recipe passes native async/static, shared and
synchronous 93/93/90 runnable CTests (physical audio skipped), audio-off 93,
retained C 20/19 and selected proactor-off 37. The known proactor-off cancellation
defect remains excluded/open. Rust gates pass 471 enabled tests (450 unit and 21
integration), 386 minimal, strict all-target/all-feature Clippy, format/generated
checks, four target library compilations and strict Windows library Clippy.
The added tail regression is run again in all five native/compatibility lanes,
with final full/minimal Rust and strict Clippy checks. No production behavior
changed after the complete recipe; only this extra test was added. Cross-target
compilation does not establish target runtime/C-layout validation outside macOS.

The archive probe restores `392b5575` dictionary.c in the actual dictionary.c.o
member. It retains rendering policy while enlarging its two 30-byte clause scratch
arrays to 384 and recording any output outside the original scratch domain; two
forwarding shims connect valid fitting new writer call sites to the old writers.
The committed differential fixture retains the original C body unchanged, with
verified provenance. All 3,561 full-C/native/probe CLI cases over 35 voices, 2,016
native/probe traces and 70 full-C/native/probe offline WAVs over seven voices match.
Prior full-C braille traps remain excluded; native/probe matches previous receipts
without rerunning crashes. Actual custom compiler output and all 123 built
dictionary payloads match full C. A separate 1,224-case IPA/separator/tie corpus
over 17 voices matches all three lanes; none exceeds original scratch capacity.
The complete gate session 23250 and IPA session 17322 exited 0. Final expanded
regression session 80775 exited 0; its five fixture lanes and Rust/strict checks
are recorded separately in the edge logs.

Fresh dictionary C/mixed logic falls 268 to 96 lines (21 C, 2 mixed, 20 bridges),
with sync/async totals 6,242/6,712 and sync frontend items 1–7 at 1,434. The generated
appendix is refreshed against all 29 inventory rows. Counts retain bridge/scanner/
platform/configuration limits. Evidence and recipes use `/private/tmp/espeak-phoneme-render-*`.

Full engine integration, remaining projections/control, process I/O/MBROLA flush,
path/cancellation, compilers, tooling/platform retirement and real-device/runtime/
representative thermal gates remain. The coarse thermal query returns unavailable
errors (`0xe00002bc`), not a current reading. Publication/final remote CI remain
blocked by the automatic approval review usage limit; Jay's push approval is held.

## Native individual dictionary rule compilation

`rust/rule_compile.rs` now owns the complete individual-rule parser and bytecode
assembly previously in `compiledict.c`'s `isHexDigit`, `copy_rule_string` and
`compile_rule`. It preserves section transitions, conditions, group aliases,
prefix reversal, debug line bytes, permissive hexadecimal/octal escapes,
letter groups, ordered dollar commands, suffix flags and numeric group matching.
Diagnostic order and text remain compatible on the defined retained-C domain.
Invalid letter-group arithmetic explicitly follows the target's plain-char
signedness; a separate safe Rust regression checks both target conventions.

The native descriptor copies the initialized input, group name, letter-group
definitions and byte-space classifications before primitive effects. Owned
80-byte part/encoding scratch and a 512-byte record replace unchecked array
access; the source line admits 499 bytes, matching the compiler's 500-byte fgets
buffer. Completed output is published only after effects and capacity admission.
Rejection preserves the destination while retaining effects already performed.
Oversized token expansion, signed numeric overflow, incomplete letter-group
escapes and unterminated input now report an error rather than accessing beyond
the original buffers. Rust introduces no heap allocation, thread, timer, sleep
or scheduler here. At this checkpoint, the C adapter retained diagnostics and
malloc/free ownership for the group compiler; rule-file control/source reads
are migrated in the next section. List/top-level compiler I/O remains C.

The fixture freezes the actual `df14a1fc` mnemonic table and complete C compiler
without policy edits; provenance is verified byte for byte. It extracts the
current production adapter at CMake configure time, with a configure dependency
on `compiledict.c` so edits cannot leave a stale generated adapter. 24,350
record comparisons pass with exact allocation lengths, complete initialized
record bytes, diagnostic text and counts. Cases include randomized grammar,
all literal nonzero byte values, token lengths through 79, conditions, debug
NUL bytes, numeric Unicode groups, switches and both valid/invalid letter groups.
Native-only malformed/overflow, short-output and unterminated-input guards pass.
Seven safe Rust regressions cover ordering, escapes, bytecode, rejection and
target char signedness. The enabled/minimal Rust suites pass 478/393 tests,
and strict all-target/all-feature Clippy passes.

The complete serial offline recipe (session 10462, terminal exit 0) passes
native async/shared/synchronous 94/94/91 runnable CTests, with physical audio
skipped, audio-disabled 94, retained C 20/19 and selected proactor-disabled 38.
The known proactor-disabled cancellation defect remains excluded/open.
Formatting/generated-table checks, four target-library compilation checks and
strict Windows library Clippy pass. Cross-target compilation does not establish
runtime or C-layout validation on those target devices.

The frozen parser and native parser share current encoding/classification
primitives. Separately, the actual native and full-C CLI compilers emit identical
1,291-byte custom dictionary payloads (SHA-256
`144c8fb2a269120b5e71e6d00e612e8627cb43f94b78919eba25558c8bb355ef`)
and exact diagnostics for lists, language switches, 79-byte rules, pre/post
contexts, conditions, letter groups, dollar commands, suffix flags and numeric
Unicode matches. All 123 complete generated dictionaries match full C byte for
byte. Fresh `compiledict.c` C logic drops from 1,205 to 863 lines; sync/async
C/mixed totals become 5,900/6,370, a net reduction of 342. The generated appendix
is refreshed against all 29 inventory rows; bridge/scanner/platform/configuration
limits remain. Group/list compilers and source/output I/O are still C.

Evidence and serial offline recipes use `/private/tmp/espeak-rule-compile-*`.
The dependency already includes Windows HANDLE reuse repair `e421f5ea` and IOCP
deadline repair `84b2b6a8` through the `843ae1de` pin; local loadngo dev has no
subsequent proactor diff. A fresh remote query remains unavailable. The coarse
thermal query returns unavailable errors (`0xe00002bc`), not a current reading.
Full engine integration, remaining projections/control, process I/O/MBROLA,
compiler drivers, tooling/platform retirement and real-device/runtime/thermal
gates remain. Jay's push approval persists; publication/final remote CI remain
blocked by the recorded automatic approval-review service usage limit.

## Native dictionary rule-file compilation and pooled source reads

`rust/rule_file.rs` owns complete rule-file control, letter-group definitions,
replacement records, group-name conversion, indexed/ordinary group ordering,
phoneme sorting/common compression, group packing and final bytecode framing.
The parser retains 499-byte fgets chunks, source-platform CRLF/CTRL-Z behavior,
EOF line counting, unknown dot-directive skipping, duplicate indexed-group
headers, target plain-char item-length truncation and exact diagnostic order.
Initialized input tails remain observable: the legacy letter-item scanner can
advance past the NUL inserted at a comment marker, or the NUL of a shorter final
line, into initialized comment/prior-line bytes. Rust retains and mutates one
bounded input scratch prefix to reproduce those shipped bytes without reading
uninitialized storage. Explicit actual-C and safe Rust regressions cover this.

The caller-owned `Workspace` retains bounded record/order/group/packed arenas,
overwriting previously allocated storage across compilations. Rule/group counts
admit the legacy 3,000/120 domains; the source/output arenas have a 200 MiB
ceiling and fallible growth. Writer failures, group/resource limits and nested
native compilation are refused. Streaming writes and prior diagnostics remain
visible on failure. No per-rule malloc, per-item allocation or repeated packed
realloc remains in the native controller. Existing grouping mistakes are
preserved as measured semantics, rather than silently changing dictionary data.

The native bridge reads rule source contents with `engine_io::read_file_into`,
using the process's existing platform proactor and pooled source storage.
`engine_io` also reuses one fixed completion scratch buffer instead of allocating
and freeing a Vec for each chunk. Caller output storage and completion storage
are distinct; the caller copies only after delivery, without retaining a buffer
loan or invoking compiler policy from the completion callback. An added real-I/O
regression repeatedly reads a file larger than two chunks and checks unchanged
output allocation addresses. Without a proactor, the documented std::fs fallback
remains. No new thread, timer, sleep, spin or compiler scheduler is introduced.

The C dictionary entry point still selects/opens source paths and owns the final
dictionary stream; the adapter retains fwrite/ftell and diagnostic formatting.
List/phoneme compilation and top-level compiler source/output I/O ownership still
need migration. Source content is read by path after that C driver's path checks;
concurrent source replacement during compilation is not validated here.

The fixture freezes the actual `6bc208e6` complete C rule-file/group functions
without policy edits, and extracts current native effects/control from source
at CMake configuration. It shares the unchanged original individual-rule oracle
and current primitive encoder/classifier. It compares complete bytecode, exact
diagnostics/counts, final line/group/index state and letter-group definitions,
including ordinary/indexed/duplicate/numeric/UTF-8 groups, replacements at four
initial writer alignments, long item lengths, comments and initialized tails.
Eight safe Rust regressions cover packing, repeated indexed headers, alignment,
signed/unsigned length conventions, Windows text, initialized tails, source EOF,
resource/writer failures, the once-per-group rule limit and allocation reuse.

The complete serial offline gate (session 21043, terminal exit 0) passes
486/401 enabled/minimal Rust tests, native async/shared/synchronous 95/95/92
runnable CTests (physical audio skipped), audio-disabled 95, retained C 20/19,
selected proactor-disabled 39, strict/generated checks, four target-library
compilation checks and strict Windows library Clippy. The known proactor-disabled
cancellation defect remains excluded/open. The compiler oracle passes 1,296
complete bytecode/state/exact diagnostic comparisons in all five native lanes;
the prior individual-rule oracle still passes 24,350 cases. Cross-target checks
establish compilation, not runtime/C-layout evidence on target devices.

Fresh `compiledict.c` C/mixed logic drops 863 to 632 lines (14 C functions,
one mixed adapter); sync/async totals become 5,669/6,139, a net reduction of 231.
All 29 generated appendix rows are verified; only compiledict changes. The
inventory retains bridge/scanner/platform/configuration limits. The coarse
thermal query remains unavailable (`0xe00002bc`), not a current reading.

Actual native/full-C CLI custom dictionary bytes and diagnostics match; all 123
rebuilt dictionary payloads also match full C. This comparison found the Catalan
comment-tail case and confirmed its repair before the complete validation recipe.
Evidence and offline serial recipes use `/private/tmp/espeak-rule-file-*`.
Full engine/resource integration, remaining projections/control, MBROLA/process
I/O, path/cancellation, compiler drivers, tooling/platform retirement and real
device/audio/thermal proof remain open. Jay's push approval remains held;
publication/final remote CI remain blocked by the recorded automatic-review
service quota.

## Native dictionary-list compilation and proactor sources

`rust/dict_list.rs` now owns the complete dictionary-entry state machine,
conditional/keyword flags and persistent text-mode control, multiword metadata,
replacement and phoneme admission, lowercase/all-capitals control, Unicode
character keys and variation-selector removal, record assembly, hash-chain
precedence and final bucket framing. Alphabet compression still uses the
existing native Rust implementation through a serialized translator projection;
hashing uses the safe Rust dictionary helper directly. Special-word translation,
locale uppercase/lowercase classification and the encoder remain synchronous
primitive effects on owned scratch. The C compiler's historical context-owned
special-word pronunciation behavior is retained.
Legacy in-place normalization is retained even when Turkish `I` widens and
overwrites its delimiter: the resulting initialized multiword tail and permissive
UTF-8 bytes are copied back before metadata packing. Actual-C comparisons and a
safe Rust regression cover that behavior separately.

`rust/dict_list_compat.rs` controls roots, list/listx, emoji and extra in the
original precedence order, tries `.txt` before the extensionless source, and reads
each admitted source through `engine_io::read_file_into`. A serialized pool
retains source, record bytes and linked-node index arenas across compilations;
there is no allocation/free per dictionary entry. Source/record storage and node
storage each have a 200 MiB ceiling, fallible geometric growth and backpressure.
Reentrant/native concurrent use is refused with `try_lock`, without waiting or
spinning. Filename scratch is reused and its prefix is bounded and copied before
effects. Parser/compiler policy runs on the caller after I/O completion; no new
thread, timer, sleep, spin or compiler scheduler is introduced.

The file reader preserves 199-byte fgets admission, initialized prior-line
parser tails, CRLF/CTRL-Z behavior and long-line remainder discard. In particular,
the keyword scanner may advance past a shorter final line's NUL into initialized
prior-line bytes; keyword lookup still terminates at the first NUL. It refuses
any scan outside the initialized extent. C/Rust fixtures cover those tails and
ensure discarded comments cannot become spurious dictionary keys. Completed
records admit 255 bytes and 100 flags; malformed strings/records are refused
without C replay. Primitive diagnostics and parser state already published
remain visible, while an individual rejected record leaves output/hash untouched.
Optional missing/inaccessible paths use the legacy fallback and are skipped when
both candidates fail to open. Other source/resource/write failures abort
the compiler and report explicitly, rather than publishing an incomplete list.

The original `11367a76` parser, mnemonic table, linked-chain and file-driver
bodies are frozen without policy edits. CMake extracts the actual current native
primitive bridge and controller, avoiding a separate model of the integration.
The comparisons cover exact records/hashes, text/retry state, diagnostics/counts,
all keyword definitions, real locale case handling, compressed Russian keys,
duplicate precedence across all five files, `.txt` shadow selection, long lines,
EOF and initialized tails. A malformed empty special-word text entry makes the
old C compiler write into its static empty string and traps; that input is
excluded from the C oracle and is a passing owned-input Rust regression instead.
Native ABI, capacity/unchanged-output, descriptor, source-prefix, optional-file
and writer-failure guards are checked separately.

The native top-level entry point still checks rule paths and owns final output,
header/offset patching, fclose, LoadDictionary and cleanup. Encoder/translator/
locale/logging/writer projections still use C owners; dictionary tracing,
phoneme/intonation/spectrum compilers and the rest of the full engine/resource,
platform/backend/audio migration remain open. Target compilation does not prove
target runtime, C layout, physical audio or thermal behavior. The Loadngo pin
already includes reused-HANDLE registration/release and IOCP deadline fixes.

Final serial offline validation uses recipes/sessions 65928 and 24476, both
terminal exit 0; session 38609 rechecks the strengthened literal primitive-trace
fixture in async/shared/audio-disabled builds after the full suites. All five
native configurations pass 20,072 retained-C record/hash/state/diagnostic/actual
primitive-call comparisons, 104 complete list-file/bucket comparisons and seven
native boundary/ABI guards. Primitive traces compare literal framed call inputs,
counts and order, rather than relying only on a fingerprint. Eight new safe Rust
regressions pass. All 123 rebuilt dictionary payloads and the actual custom CLI
payload/exact diagnostics match the full C compiler.

The final enabled/minimal Rust totals are 494/409 (473 unit plus 21 integration
tests with features enabled). Native async/shared/synchronous speech suites pass
96/96/93 runnable CTests, audio-disabled 96, retained C 20/19 and selected
proactor-disabled compatibility 40. Strict all-target/all-feature Clippy, fmt,
generated-table checks, Linux/Windows/iOS/Android library compilation and strict
Windows library Clippy pass. The physical-audio test is skipped; the known
proactor-disabled cancellation defect remains excluded/open. These checks do
not establish target runtime/C-layout evidence or physical playback/thermal
safety. The coarse thermal query is still unavailable (`0xe00002bc`).

Fresh `compiledict.c` C/mixed logic falls 632 to 337 lines: 17 C functions, one
mixed adapter and one forwarding bridge. Whole sync/async inventories become
5,374/5,844, a net reduction of 295; all 29 generated rows are verified, with
only compiledict changing. The scanner's bridge/platform/C++ limitations still
apply. Evidence, provenance receipts and serial recipes use
`/private/tmp/espeak-dict-list-*`. The full port remains active. Jay's push
approval is retained; publication/final remote CI remain pending the recorded
automatic approval service quota.

## Native dictionary compiler driver and proactor output

`rust/dictionary_compile.rs` now owns the complete top-level dictionary driver:
rule-file admission with `.txt` precedence and fallback, source/output lifetimes,
list/rule ordering, the eight-byte header and rules-offset patch, partial-failure
cleanup, and final reload/error decisions. The C ABI adapter copies bounded
source/name/home/table strings before filesystem or callback effects. Native C
retains synchronous translator/classifier/diagnostic and dictionary-reload
projections; its diagnostic FILE remains caller-owned. Output FILE ownership,
fwrite/ftell/fseek/fclose and heap compiler-context cleanup leave the native
top-level driver. The unchanged C-only compiler remains available.

`engine_io::Writer` opens read/write/truncate output, matching the prior `wb+`
mode. Windows proactor files use overlapped handles and the pinned Loadngo
registration API; registration is released before the file closes. Successful
completions return a reusable loan buffer. One retained 256 KiB staging buffer
and one completion buffer bound output storage, with one write in flight and
no allocation/free per compiler record. The logical position includes staged
bytes; seeking flushes pending bytes before the header patch. Positive short
writes advance the offset and submit only the remaining suffix. Open/close and
path admission remain OS primitives; content writes drive the engine's existing
proactor on the caller, without adding a worker, timer, sleep or spin loop.
Proactor-disabled/unavailable configurations retain the blocking fallback.

A failed write poisons its writer instead of replaying buffered output. A poll
failure requests cancellation, while the completion retains its file/buffer and
the slot refuses reuse until that completion arrives. Writer admission checks
the slot before truncating another file. Drop starts no I/O; explicit finish
flushes before normal release. Resource/write failures are reported, rather than
claiming successful output as the old unchecked final stdio calls could.
Source-probe lifetime and close ordering match the previous driver: list failure
closes the source before flushing partial output; rule completion closes it
before patching the header. The initial rules probe and subsequent pooled source
read remain distinct opens, as in the previous native compiler.

The frozen `645abcb2` native driver body is verified byte for byte against its
recorded SHA-256. It calls the extracted current list/rule projections and uses
a real C FILE sink; the comparison calls the actual current library entry point
with Rust-owned output. Cases compare full output existence/bytes, status,
file-error-context type/name and exact diagnostics. They cover randomized full
compilations, source extensions, debug/list ordering, duplicate rules, Unicode,
semantic errors, NULL source/name, `.txt` precedence, a directory source and
output-directory failure. Separate ABI/admission guards reject invalid inputs
before effects. Safe Rust regressions check header/effect order, partial list and
rule failure, path truncation, missing/output errors and semantic-error reload.
A real-file writer regression compares multi-chunk output plus header patching
through 64 reopen cycles, verifies stable staging storage and refuses concurrent
writers before truncation. These are macOS runtime checks, with target
compilation recorded separately; injected short-write/poll/cancellation failures
and Windows/Linux/device runtime still need evidence.

The full port remains active. Dictionary tracing and native projections, other
compiler source/output I/O, full engine/resource and MBROLA lifecycle integration,
tooling/platform retirement, physical audio and active/idle thermal proof remain.
The Loadngo pin already includes the reused-HANDLE fix (`e421f5ea`) and IOCP
deadline fix (`84b2b6a8`); local Loadngo dev has no newer proactor diff from that
pin. No fresh remote or target-runtime claim is made. The coarse thermal query
returned `0xe00002bc`; validation runs serially without a thermal-safety claim.
Jay's push approval is retained; publication/final remote CI remain pending the
recorded automatic approval service quota.

Final serial offline gate session 67351 exited 0. Enabled/minimal Rust totals
are 497/410 (476 enabled unit tests plus 21 integration tests). Native
async/shared/synchronous suites pass 97/97/94 runnable CTests; audio-disabled
passes 97, retained C configurations pass 20/19, and selected proactor-disabled
compatibility passes 41. Strict all-target/all-feature Clippy, formatting,
generated tables, Linux/Windows/iOS/Android library compilation and strict Windows
library Clippy pass. Session 1100 exited 0 after checking the strengthened driver
fixture in all five native configurations: 245 complete-driver comparisons and
seven admission guards pass in each. Only fixture cases changed after the full
suites; production code was unchanged. The physical-audio test remains skipped,
and the known legacy proactor-disabled cancellation defect stays excluded/open.

Actual native/full-C custom compiler payloads and exact diagnostics match:
1,291 bytes, SHA-256
`144c8fb2a269120b5e71e6d00e612e8627cb43f94b78919eba25558c8bb355ef`.
All 123 rebuilt dictionary payloads also match full C. Fresh `compiledict.c`
C/mixed logic falls 337 to 279 lines (18 C functions, one mixed and five
forwarding bridges), reducing measured sync/async totals to 5,316/5,786 by 58
lines. All 29 generated appendix rows match, with only compiledict changed;
the scanner's bridge/C++/platform limitations still apply. Recipes, provenance,
inventories, compiler receipts and fixture logs use
`/private/tmp/espeak-dictionary-compile-*`. These local gates do not establish
target runtime, full C layout, physical playback or thermal safety.

## Native dictionary flag and rule trace rendering

`rust/dictionary_trace.rs` replaces complete `print_dictionary_flags` and
`DecodeRule` policy. Ordered Rust mnemonic tables preserve stress-first names,
first synonyms, unknown-bit spaces and cumulative flag budgets, including names
skipped for capacity. Unsigned masks handle all 64 flag positions. Rule rendering
preserves target plain-char condition conversion, line numbers, dollar controls,
suffix/prefix flags, letter-group formatting, symbols, bytewise pre-context
reversal, truncation and minimum padding. The C adapter retains the historical
`sizeof(char *)` byte budget to preserve defined existing trace output; typed
callers can supply their actual byte budget.

Native `RustMatchHost` now carries the admitted dictionary remainder length.
`RustMatchTrace` passes explicit source/group/output extents to Rust instead of
walking an unbounded rule pointer in C. Formatting uses fixed 200/512-byte stack
scratch, consumes at most 512 encoded bytes, makes no callbacks or allocations,
and publishes only initialized output plus NUL after complete success. Malformed
parameters, overlong scratch and short destinations reject without publication.
These are CPU transforms; proactor I/O remains in the engine/compiler paths.
Native C retains diagnostic FILE writes and serialized resource/projection
owners. Legacy decoding and mnemonic tables compile only in the retained C
configuration or test-only frozen oracle.

The old decoder's pointer-size budget has an additional memory-safety defect:
line-number plus condition prefixes can put `p` beyond `p_end`, making
`buf[p_end-p] = 0` index before the array. The exact frozen `62985518` body,
with initialized bytes `{9,2,1,5,1,3}`, produces an AddressSanitizer-confirmed
one-byte write three bytes before its 200-byte `buf`. The probe exits 134; its
external symbolizer is unavailable, but ASan identifies `ReferenceRule`, the
frame and `buf` underflow. This undefined case is excluded from C equality
coverage and tested directly in Rust/native regression coverage. Rust preserves
the metadata prefix and emits no match suffix when the byte budget is exhausted,
without negative indexing. Invalid letter groups and truncated payloads are
likewise guard cases, rather than unsafe C oracle inputs.

The unchanged C tables/functions are frozen from `62985518`; provenance records
both table and body SHA-256. The fixture compares complete initialized output
and untouched tails for 200,000 flag cases and 100,000 rule cases. Rule cases
separate line/condition/pre-context modes to avoid the known C underflow, admit
only valid letter-group indices and keep scratch below 200 bytes. It checks
the extended host layout on the executing ABI, ten admission guards and the
unsafe-prefix regression. Four safe Rust regressions check bounded publication,
plain-char variants, suffix/dollar/letter-group output and preserved legacy
truncation. The old matcher oracle links a test-only frozen decoder; production
native code no longer uses that unbounded implementation.

The full port remains active: native diagnostic/resource owners, remaining
compiler projections, phoneme/intonation/spectrum compilers, complete engine and
MBROLA lifecycle integration, tooling/platform retirement and real target/audio/
active-idle thermal proof remain. Known proactor-disabled cancellation is still
excluded/open. Validation runs serially; the recorded thermal-provider error
`0xe00002bc` supplies no thermal-safety evidence. Jay's push approval is retained;
publication/final remote CI remain pending the recorded approval-service quota.

Final serial offline gate session 20104 exits 0: 501/414 enabled/minimal Rust
tests (480 enabled unit tests plus 21 integration tests), strict all-target/
all-feature Clippy, formatting/generated tables, four-target library compilation
and strict Windows library Clippy pass. Native async/shared/synchronous suites
pass 98/98/95 runnable CTests, audio-disabled 98, retained C 20/19 and selected
proactor-disabled compatibility 42. The physical-audio test remains skipped.
The executing native configurations pass the 300,000 formatting comparisons;
the existing matcher checks pass 886,890 scalar masks, 46,812 real rule executions
and 59,400 synthetic executions, plus its native admission guards.

Session 91445 exits 0 with the frozen C fixture compiled using AddressSanitizer:
all 300,000 selected comparisons, the native unsafe-prefix regression and ten
guards pass without an ASan report. Rust and other library objects in that
fixture are not instrumented. The separate failing original-prefix probe is
retained as defect evidence, not counted as a passing C comparison. Only source
copyright comments changed after the main gate started; formatting behavior and
ABI code were unchanged. Frozen table/body SHA-256 and all 29 generated appendix
rows are reverified against the actual sources.

The complete current CLI matches the unchanged `62985518` native executable for
2,016 exact traces across 35 voices, including spelling, Unicode, language
switches, number and alternate-language cases. Its executable was copied before
the rebuild. Actual custom compiler payload/diagnostics and all 123 rebuilt
dictionaries still match full C. Fresh compiler/dictionary C/mixed logic falls
279/96 to 116/90; compiler has 16 C functions, one mixed and six bridges, while
dictionary has 20 C, two mixed and 21 bridges. Whole sync/async totals fall by
169 to 5,147/5,617. Only those two appendix rows change; bridge/C++/platform scan
limits still apply. Recipes, original/frozen provenance, ASan probes, fixture/
CLI/compiler receipts and inventories use `/private/tmp/espeak-dictionary-trace-*`.
These are local host/runtime and target-compilation checks, with no new real
Linux/Windows/mobile, physical-audio or thermal-safety claim.

## Owned phoneme and intonation compiler sources

`rust/compiler_source.rs` owns source contents, byte pushback, EOF, line
accounting, token bookmarks/replay, comments, escaping, equals and bracket/comma
boundaries, bounded numeric parsing and fgets-width chunks. Native
`compiledata.c` no longer reads or seeks phoneme/include/intonation source FILEs.
It uses a serialized Rust source session; diagnostic FILEs, keyword and phoneme
lookup, include-dispatch policy, phoneme/program generation, compiler outputs
and sound/spectrum/manifest inputs remain C projections or logic.

Thirteen stable include slots cover the existing twelve-level include stack
and its top-level source. Each holds retained byte storage, overwritten on
subsequent opens. Sources admit at most 16 MiB per file and 64 MiB retained
capacity across the pool. Metadata admission precedes buffer growth; failed
reads leave the slot unavailable, and ending the session clears logical bytes
without releasing the retained allocation. A compatibility session stays on
its creating thread. Nested session acquisition fails
without waiting. Intonation's first pass and rewind share the admitted snapshot.
Windows text mode translates CRLF in place, preserves lone CR and stops at
CTRL-Z; phoneme sources stay binary. Legacy native paths retain the platform's
path-byte/code-page conversion.

Content reads use `engine_io::read_file_into`, its existing 256 KiB retained
completion storage and one engine proactor, driven synchronously by the
compiler's caller. Open/metadata/close remain OS primitives. Token scanning
has fixed 256-byte scratch, no per-byte callback or allocation, and no custom
thread, sleep or timer. C receives initialized token records and copies only
the token prefix plus NUL; EOF/opening-bracket results leave its previous token
and terminator untouched. The source session releases every include slot on
cleanup, including early exits.

The old C scanner writes beyond its 256-byte item array for oversized tokens
and uses unchecked signed decimal accumulation. Native scanning rejects tokens
longer than 255 bytes after consuming their boundary, and reports numeric
overflow without performing overflowing arithmetic. These undefined C cases
are tested as Rust admission/progress regressions, not C equality cases.
Locale classifiers are copied before source reads. Frozen C numeric comparisons
exclude negative plain-char arguments to `isdigit`; non-ASCII byte/token
comparisons use string/keyword/bracket modes. Slash-at-EOF retains the old
narrowed 255-byte pushback behavior, and escaped EOF, embedded NUL, comments,
replay, line counts, untouched tails and chunk EOF behavior have direct coverage.

The test-only scanner and replay helpers are byte-identical to `82f273a3`;
their SHA-256 values are
`ad6d33a74a2f2a49ffe81706b339c499695a769bde35a5a959c3c7737b944028`
and `efcd30811a0e9e01c6d439c9649ff8bf04676d9ecb2108a3081493cf45771772`.
Safe tests cover multichunk real reads, repeated include-slot buffer reuse,
failed/oversized opens, parent preservation, token bounds, numeric admission,
replay and Windows text conversion. Native fixture coverage includes a second
serialized session, unavailable slots, invalid discriminants/output widths and
no publication on rejected records.

Final serial offline gate session 47700 exits 0. Enabled/minimal Rust suites
pass 505/418 tests (484 enabled unit tests plus 21 integrations). Native
async/shared/synchronous suites pass 99/99/96 runnable CTests, explicit
audio-disabled 99, retained C 20/19 and selected proactor-disabled compatibility
43. Physical audio remains skipped and known legacy cancellation excluded/open.
Strict all-target/all-feature Clippy, formatting/generated tables, Linux
aarch64, Windows x64, iOS aarch64 and Android aarch64 library checks and strict
Windows library Clippy pass. The native fixture passes 278,481 actual frozen-C
token/check/replay/fgets-chunk comparisons and source/output guards. The same
fixture's C object is ASan-instrumented and passes; Rust and other library
objects are not instrumented, so this is selected C-fixture coverage.

Complete compiler probe session 85975 exits 0 and checks fifteen native/full-C
phoneme/intonation scenarios match exact status, diagnostics and output bytes:
twelve-level include restoration, depth-thirteen refusal, missing includes,
escaped names, EOF slash/comments, signed/unsigned numeric errors and suffixes,
maximum clamping, bare/text-name intonation lookup, CRLF and two-pass replay.
Minimal phoneme fixtures use current `vwl` and `vcd alv stp` feature names;
each case asserts its intended success/error status, including successful
twelve-level include compilation.
Two native-only probes reject 10,000-byte tokens and overflowing numbers with
the expected diagnostics and exit 1; undefined C counterparts are not invoked.
All four regenerated master binary phoneme/intonation payloads and the manifest match
the retained C build byte-for-byte. The pinned Loadngo proactor contains reused
Windows HANDLE fix `e421f5ea`; local `dev` at `6b2489d1` has no proactor changes
relative to that pin. This checks local source contents, not remote freshness
or Windows execution.

Fresh core/async inventories count 1,745 C/mixed lines in `compiledata.c`
(36 C functions, five mixed, three short wrappers), down from 1,804. Whole
sync/async totals fall by 59 to 5,088/5,558. All 29 appendix rows are verified;
only `compiledata.c` changes. The scan classifies 14-line `clean_context` as
a bridge, although its C reference/manifest teardown still remains. Bridge,
C++ inline-member and platform scanning limits apply. Recipes, frozen-body
provenance, selected ASan, compiler receipts and inventories are retained
under `/private/tmp/espeak-compiler-source-*`.

The full migration remains active. Keyword/phoneme ownership, compiler control
and output/resource I/O, spectrum compilation, engine/MBROLA lifecycle and
platform retirement remain. No new real Linux/Windows/mobile execution,
physical-audio or thermal-safety proof is established. Checks are serial and
offline; the recorded thermal-provider error remains unavailable telemetry.
Jay's push approval is held, with publication/fresh remote CI pending the
recorded approval-service quota.

## Complete owned intonation compiler

`rust/intonation_compile.rs` replaces the full `espeak_ng_CompileIntonationPath`
controller, keyword/envelope/default tables and tune assembly. Safe Rust callers
can bind resident source bytes or compile source paths without C. Compatibility
inputs copy paths and locale maps before I/O or callbacks; C retains only the
public API/default-path projection, diagnostic FILE writes/closure, error-context
allocation and resource reload. Native builds exclude the old intonation
compiler and its tables. Phoneme/spectrum compilation remains unfinished.

The compiler owns two-pass name discovery, four preset names, up to 100 tune
records, duplicate/unknown-name and missing-preset diagnostics, all eleven
statements and ten envelopes, parameter conversion, split defaults and target
references. Each record is an explicit 68-byte array, including initialized
reserved bytes. Names/tunes and diagnostic scratch use fixed arenas; source
bytes stay in retained slots. There is no per-token/tune allocation or callback.
The safe source pool now supports bounded resident-byte binding as well as
proactor reads. Windows text conversion remains explicit.

Legacy details are deliberate: onset/head-last flags persist across subsequent
tunes, duplicate declarations overwrite the earlier record after reporting an
error, byte fields retain signed/unsigned narrowing, missing/invalid tunes still
produce their legacy output records, and the old bad-name diagnostic spelling
and EOF token behavior remain. Numeric accumulation, token bounds and percentage
scaling reject operations that would overflow or overwrite C storage; those
undefined C cases have native-only safety regressions rather than equality
claims. Source admission bounds are 16 MiB per file and 64 MiB retained source
capacity; the output contains at most 6,800 bytes.

The complete driver selects `intonation.txt` then the bare fallback, discovers
names before output truncation, opens the output before semantic diagnostics,
and writes through `engine_io::Writer`. It uses the existing engine proactor,
retained 256 KiB staging/completion storage and one write in flight. Output
completion and registration/handle release precede summary/reload. Input/output
open failures close the diagnostic log before allocating the file-error context,
as in C. Unlike the old unchecked `fwrite`/`fclose`, a detected write/completion
error propagates without success summary/reload. No custom thread, sleep, timer
or scheduler is introduced; synchronous compilation runs on its caller, outside
proactor completion delivery. Reentrant compatibility calls reject before I/O.

Five safe regressions cover all default/field bytes, split and cross-tune state,
duplicate/bad-name/envelope/numeric diagnostics, the 100-tune bound, percentage
and token overflow, repeated resident bindings, Windows text, source fallback,
real output bytes and completion/log/reload/error order. The native fixture uses
actual frozen C name/keyword/default tables and the complete compiler body from
`82f273a3`, plus its unchanged scanner. It compares complete output, status,
diagnostics, log closure, reload counts and file-error paths, with descriptor,
callback reentry and caller-path mutation guards. C integer multiplication is
kept in its defined range; unsafe numeric/token cases are direct Rust checks.
Frozen constants/function/keyword SHA-256 values are respectively
`ec3670fa3e9656a17d8f9fee405108c0d7fa0a2965813779cfbc08781fbc959d`,
`ffad82bed52589b02d610b2d4f291be438a74da1ed722d6546ab8dab936825d6`,
and `255263cb433dc003ebbd5f8960dc7ba46fd4afb6107dbe52c848a8f92b49ef25`.

Serial offline gate session 23105 exits 0. Enabled/minimal Rust suites pass
510/423 tests (489 enabled units plus 21 integrations). Static async/shared/core
native suites pass 100/100/97 runnable CTests, explicit audio-disabled 100,
retained C core/async 20/19, and selected proactor-disabled compatibility 44.
Physical audio is skipped and known legacy cancellation remains excluded/open.
Strict all-target/all-feature Clippy, formatting/generated tables, Linux
aarch64, Windows x64, iOS aarch64 and Android aarch64 library checks and strict
Windows library Clippy pass. The native fixture passes 515 full-C intonation
comparisons and descriptor/reentry guards. Its selected C object also passes
AddressSanitizer; Rust and the other library objects are not instrumented.

Complete CLI compiler session 15215 exits 0: fifteen native/full-C phoneme and
intonation scenarios match exact status, diagnostics and payloads, with intended
success/error statuses asserted. Bare/text-name lookup, CRLF, bad signed input
and missing intonation sources exercise the new driver; the earlier include,
EOF, escape and numeric cases remain. Two native-only phoneme token/numeric
overflow cases reject with expected diagnostics. All four regenerated master
binary phoneme/intonation files and the manifest match retained C byte-for-byte.
Frozen constants/compiler/keywords are confirmed byte-identical fragments of
`82f273a3`; trailing blank lines at fixture EOF are omitted. The
existing Loadngo `843ae1de` pin includes HANDLE reuse repair `e421f5ea`; local
Loadngo `dev` at `6b2489d1` has no later proactor diff. This is local source
verification, not remote freshness or Windows execution.

Fresh core/async inventories count 1,544 C/mixed lines in `compiledata.c`
(39 C functions, five mixed, two short wrappers), down 201 from 1,745.
The intonation entry point is now a 16-line projection; the remaining phoneme
compiler and primitive intonation log/error/reload callbacks still count as C.
Whole sync/async totals fall to 4,887/5,357. All 29 appendix rows are verified;
only `compiledata.c` changes. Bridge, C++ inline-member and platform scanner
limits remain, including the C teardown inside 14-line `clean_context`.
Recipes, logs, compiler receipts, selected ASan, provenance and inventories
are retained under `/private/tmp/espeak-intonation-compile-*`.

The full port remains active: phoneme/spectrum/output-resource owners, engine
and MBROLA lifecycle, remaining native projections and platform retirement,
real target/audio/active-idle thermal proof remain. Known proactor-disabled
cancellation stays excluded/open. Validation is serial and offline; the
recorded thermal-provider failure is no thermal-safety evidence. Jay's push
approval is retained, with publication/fresh remote CI pending the recorded
approval-service quota.

## Owned spectral compiler sources, frames and calculations

`rust/spect_file.rs` replaces the complete spectral-file reader in `spect.c`:
all three eSpeak/Klatt file identifiers, little-endian fields, IEEE80 floats,
name bytes, default/formant/peak/Klatt fields, harmonic bins, keyframe detection,
signed maximum-height selection, zero-relative times, display scales, duration
and keyframe length adjustment. The polynomial interpolator and frame RMS
calculation also run in safe Rust. RMS borrows the existing Rust wave generator
and preserves its live voice, sample rate and harmonic side effects; it does
not introduce a second generator or call C harmonic policy.

Frames and bins use contiguous retained arenas rather than one allocation per
frame. Safe callers can bind resident bytes or load paths through
`engine_io::read_file_into`, the existing process engine proactor, retained
256 KiB completion storage and one read flight. Input is bounded to 16 MiB;
positive signed-short frame counts and bin counts are bounded to 32,767.
Metadata/open/close remain OS primitives. Parsing/copying runs on the compiler's
caller, outside completion delivery, with no custom worker, sleep or timer.
Two retained input buffers preserve the previous successful result on failed
reads; malformed parses expose only initialized complete-frame prefixes and
must not be consumed as successful results.

`rust/spect_file_compat.rs` owns four stable boxed sequence leases, retained
frame views/pointer arrays and all exposed storage. Create/load/destroy and
RMS lookup use a nonblocking registry lock; reentry and excess leases reject.
Frame lookup checks owner range/alignment in bounded time before any foreign
frame pointer could be dereferenced. C still sees its original struct layouts
while the unfinished phoneme compiler consumes/mutates those views. Native
`spect.c` contains only API and stderr projections; the C body and tables are
excluded. The existing `RustWavegenState` owner accessor is available to this
projection. Full engine/foreign-view concurrency and lifecycle retirement
remain separate work.

Names are opaque declared-length bytes: some corpus files have no terminating
NUL. Comparisons do not use `strcmp` on those C allocations. The original
constructor leaves pitch-envelope and several unused fields uninitialized;
native constructors initialize every field. The fixture explicitly initializes
the old pitch envelope before loading and excludes unused padding/uninitialized
fields from equality. Numeric products used to scale RMS peaks are checked;
the engine's established wrapping harmonic/squared-bin arithmetic remains.
Negative counts, signed-short cursor overflow, nonfinite floats and invalid
duration/scale conversions reject rather than invoking undefined C operations.
Old unsafe cases are native-only checks, not C equality claims.

Five safe tests cover all formats/fields/defaults, signed bins, cross-keyframe
length adjustment, every truncated prefix, storage reuse, real repeated reads,
failed-open preservation, numeric/name/count admission, interpolation bounds
and RMS product overflow. The original complete `spect.c` at `ce4759b7` is
frozen test-only, with only final blank lines omitted; its SHA-256 is
`a4a0fd016113745ac36dd282ed456b256e3c0bdfa526b5ead2ca79a4bd539570`.
That fixture retains its C reader/interpolation/RMS arithmetic and calls the
current IEEE80 and harmonic primitives. A test-only harmonic projection copies
the four defined peak scalars into a fully initialized record before calling
Rust; unused C stack fields never become borrowed Rust values. It passes
1,925 source/status/field
comparisons: 300 generated sources across all three formats, every prefix of
three single-frame sources, zero-frame/unsupported inputs and all 927 spectrum
files found in the configured checkout's `phsource` paths. It also passes
10,658 RMS and 100,000 polynomial comparisons, with lease/null/invalid-path/
path-width and callback-reentry guards. The current corpus has 411 eSpeak
and 516 extended-Klatt files; its maximum size/count are 70,709 bytes/71 frames.
The non-extended Klatt format has generated coverage. This corpus claim is
limited to the configured source checkout, not every external spectrum file.

Serial offline gate session 84999 exits 0. Enabled/minimal Rust suites pass
515/428 tests (494 enabled units plus 21 integrations). Native async/shared/core
suites pass 101/101/98 runnable CTests, explicit audio-disabled 101, retained C
core/async 20/19 and selected proactor-disabled compatibility 45. Physical audio
is skipped and known legacy cancellation remains excluded/open. Strict
all-target/all-feature Clippy, formatting/generated tables, Linux aarch64,
Windows x64, iOS aarch64 and Android aarch64 library checks and strict Windows
library Clippy pass. The selected C fixture object also passes AddressSanitizer;
Rust and other library objects are not instrumented. These are local validation
and cross-compilation results, not fresh Linux/Windows/mobile execution.

Follow-up session 11825 exits 0. After the test-only unused harmonic-field
initialization was added during the broader run, static/shared fixtures were
rebuilt and rerun against the final source. Both pass the same 1,925 source/
status/field, 10,658 RMS and 100,000 interpolation cases and guards; the later
core/compatibility/audio-disabled and ASan checks already use that projection.
No production code changed during these gates. Fifteen complete native/full-C
phoneme/intonation CLI scenarios match exact diagnostics, intended statuses and
payloads. Two native-only phoneme token/numeric overflow checks reject with the
expected diagnostics. All four regenerated master binary phoneme/intonation
files and the manifest match the full-C build byte-for-byte, including the
production spectral integration through the unfinished phoneme compiler.

Fresh core/async inventories count only two C/mixed lines in `spect.c`, down
from 272: one diagnostic projection and five short wrappers. Whole sync/async
totals fall by 270 to 4,617/5,087; all 29 appendix rows are verified and only
`spect.c` changes. Existing bridge, C++ inline-member and platform scanning
limits remain. Logs, gate/follow-up recipes, frozen provenance, selected ASan,
compiler receipts and inventories remain under `/private/tmp/espeak-spect-file-*`.

The full migration remains active: phoneme compilation/output resources,
engine and MBROLA lifecycle, native projections/platform retirement and real
target/audio/idle-active thermal proof remain. Validation is serial/offline;
the recorded thermal-provider failure establishes no thermal-safety evidence.
Jay's push approval is retained; publication/fresh remote CI remain pending
the recorded automatic approval-service quota.

## Owned spectral-to-phondata compiler controller

`rust/spect_compile.rs` replaces complete `LoadSpect` policy in `compiledata.c`:
source loading, Klatt extension detection across every input frame, keyframe
selection/counting, preceding-frame length totals, existing or second-keyframe
vowel markers, live-wavegen RMS, rounding/clipping/narrowing, bandwidth defaults,
all ordinary/extended record fields, final zero length, explicit little-endian
header/records, frame-counter progress and output publication. Records occupy
44 or 64 bytes; an initialized retained 1,604-byte arena admits at most 25
keyframes. No C struct aliasing or uninitialized padding is serialized.

The compiler reads directly into owned `spect_file::Sequence` arenas through
the existing engine proactor and retained completion/input buffers. It no
longer creates or consumes the C spectral-view leases for normal phoneme
compilation. Source/default/field state is safe Rust. The compatibility bridge
copies source/reference paths and the descriptor before I/O; path construction
preserves the old bounded `snprintf` prefix/slash/reference behavior. It borrows
the live Rust wave generator only for synchronous packing after reads complete
and before any output callback. Registered workspace admission is nonblocking;
reentry rejects with no result publication. Caller outputs are published after
all callbacks finish, and initialized payload loans exist only during writes.

C retains primitive stderr/diagnostic projections, `ftell`, a checked single
`fwrite`, and the mirror of compiler address/frame-count scalars. The parent
phoneme compiler still owns its output FILE and resource-dispatch probe; final
output-stream ownership/proactor writes and resource dispatch remain open.
This stage does not describe the temporary stream primitive as proactor I/O.
There is no per-frame callback/allocation, custom thread, sleep or timer.

Legacy details are retained: non-keyframes can require the Klatt extension;
each keyframe adds the immediate previous input frame's length; a missing
vowel-center marker affects the second keyframe or frame zero when none exists;
the last output frame has zero length; short/byte narrowing and signed division
retain their defined behavior; and the default Kopen makes old non-extended
sources emit extended records. Float-to-integer range, scaled products,
frame-counter overflow and more than 25 keyframes reject before output instead
of invoking old undefined conversion/stack writes. Zero keyframes preserve the
old four-byte output header. Detected write failures propagate with admitted
address/count progress, rather than the old unchecked success return.

Four safe regressions cover every record field/layout, marker/previous-length
rules, signed narrowing, bandwidth defaults, exactly 25 and rejected 26
keyframes, numeric bounds, actual source reads, source-error ordering,
position/generator/write ordering, final result publication and injected write
failure. The original complete controller from `bfde095b` is frozen test-only;
the unchanged controller fragment's SHA-256 is
`a5819547403c06d6f551bf0e021851e1a583fe4ea607f27eec7827eff1631d10`.
The fixture including its license banner has SHA-256
`d45e37cc41654565e7dc3f827250254920344723f32b1d5c560950b8bdc35818`.
It uses the previous complete frozen C spectral reader/RMS, initialized unused
pitch-envelope/harmonic fields and current IEEE80/harmonic primitives.
Comparisons check full payload bytes, status, address, frame counts, source and
vowel diagnostics, output prefix/position and the presence of native writes.
The configured corpus includes all 927 current `phsource` spectrum files with
both controls. Generated cases cover all three source formats, every truncated
prefix and zero-frame/unsupported/missing sources. Admission, descriptor/result
guards, callback reentry and changed caller buffers test owned boundaries.
Four native-only full-driver probes reject keyframe/numeric/counter overflow
or propagate an injected write error; undefined old C counterparts are omitted.

Serial validation gate `92176` exited zero: 519 enabled Rust tests (498 unit
and 21 integration), 432 minimal tests, native async/shared/core suites with
102/102/99 runnable CTests (one physical-audio test skipped in each), 102
explicit audio-disabled tests, retained-C core/async 20/19 and 46 selected
proactor-disabled compatibility checks. Strict all-target/all-feature Clippy,
format/generated checks, Linux/Windows/iOS/Android library cross-checks and
Windows library Clippy pass. Cross-checks establish compilation, not target
runtime proof; known legacy cancellation remains excluded and open.

The final fixture passes 3,157 complete C comparisons including ordinary and
Klatt 25-keyframe records. Only these additional test cases changed during the
main gate; production code was unchanged. Follow-up gate `93584` exited zero
after rebuilding/rerunning final static/shared fixtures, checking fifteen
actual full-C/native CLI phoneme/intonation scenarios with their intended
statuses, two native-only overflow diagnostics and all five regenerated master
payloads. The selected fixture object, including frozen C reader/controller,
also passes AddressSanitizer; Rust and other library objects were not
instrumented. Write failure is injected, not a real disk-full/hardware test.

Evidence is in `/private/tmp/espeak-spect-compile-final-*`, corrected static/
shared fixture logs, ASan logs and compiler receipts. Fresh core/async C/mixed
inventories total 4,487/4,957 lines; `compiledata.c` falls from 1,544 to 1,414.
All 29 current appendix rows were compared with generated output; only that
file's row changes. C++ class methods, optional/platform coverage and the
remaining ownership limits in `REMAINING_PORT.md` still apply.

The full port remains active: phoneme compiler state/control/output resources,
engine and MBROLA lifetime, ABI/platform retirement and real target/audio/
idle-active thermal proof remain. Checks are serial/offline; the recorded
thermal-provider failure is unavailable telemetry, not thermal-safety proof.
Jay's push approval is retained; publication/fresh remote CI await the recorded
automatic approval-service quota.

## Owned phoneme compiler resources

`rust/compiler_resource.rs` replaces complete `LoadDataFile`, `LoadWavefile`,
`LoadEnvelope`, `LoadEnvelope2`, `Hash8` and sample decoding policy. Rust owns
bounded path/fallback admission, actual source snapshots and dispatch, exact
name cache lookup and insertion, reference/duplicate/frame progress, source
diagnostics, WAV scaling/rounding/tagged addresses, envelope parsing and
interpolation, complete payloads and manifest rows. Normal spectral compilation
packs directly from the already-read owned source instead of probing it in C
and reopening it through the spectral API. All source content reads use the
existing engine proactor; its explicit fallback remains available when the
proactor feature/backend is unavailable. Open/metadata and path decoding use
the existing platform primitives. No custom thread, sleep or timer is added.

Four registered compiler owners retain source, payload, name and reference
arenas. Each admits 16 MiB sources, 65,536 reference records and 8 MiB of names;
close clears logical contents and reuses storage for the next generation.
Reference chains preserve exact names, bucket order, duplicate-node insertion,
cached address, phoneme mnemonic and table metadata without per-reference C
malloc/free. Token-independent path/map/descriptor snapshots precede I/O;
borrowed wavegen state exists only during synchronous spectrum packing.
Stale owners, exhausted registration and callback reentry reject before result
publication. Caller progress publishes after every callback finishes. Shared
Windows/raw Unix path decoding also compiles without the C ABI feature.

WAV records preserve declared length, the fixed 44-byte legacy sample offset,
all complete sample pairs through EOF, signed decoding, peak-derived scale,
single-precision division/rounding, byte clamps, absolute output alignment and
the sample-address bit. Rate/mono diagnostics preserve zero-address success.
Text envelopes preserve bounded 127-byte fgets chunks, ignored/non-point lines,
locale whitespace/decimal maps, direct binary32 decimal/hexadecimal rounding,
linear flags, sentinel points, polynomial window movement and output clamps.
Zero/one-point, excess-point, nonfinite/overflow and unsafe cursor inputs reject
instead of reading uninitialized data. The historical spectrum-in-a-`.wav`
fallback still attempts the unsuffixed spectral name. Final phondata/manifest
streams and parent compiler control/lifetime remain C; their checked stream
write callbacks are not described as proactor output I/O.

Binary `PITC` envelopes deliberately correct the old C item-count bug:
`fread(buf, 128, 1, f) != 128` always rejected a successful one-item read and
returned stale errno without writing. A native valid envelope writes exactly
128 initialized bytes at offset 12; truncation rejects before output. The
fixture proves the old zero-errno/no-write behavior separately, then checks
the native payload, manifest/cache progress and subsequent duplicate hit.
This correction is outside the retained-C equality claim. None of the current
source corpus's 927 spectra, 203 WAVs or 29 text envelopes use binary PITC.

Four safe regressions cover wave sign/scale/length/alignment, decimal/hexadecimal
binary32 rounding and locale parsing, malformed envelope bounds, actual source
reads, cache reuse after source deletion, exact-name collisions, record metadata,
counter/budget admission and retained capacities. Complete test-only resource
functions from `200656eb` are frozen with their license; the original combined
fragment SHA-256 is
`8bce2c18b17fe1da0e920a0ae00e68cd91354eaf6c8826e8193c247b7bcb099c`.
Two inherited space-before-tab indent prefixes are normalized without any code
change; the resulting fragment SHA-256 is
`7a5e8f654cb625b3d44af6eb37497c0f3a9b52c23f818bc94245ec40e0bc0150`.
The full fixture has SHA-256
`3cb5e6caa466747096ab36fecd87ddddaed51c554f47fa401ef3eb8605d1796e`.
It uses the existing frozen full C spectral controller/reader/interpolation,
initialized unused spectral fields and current common/IEEE80/harmonic primitives.
The parity claim concerns actual retained-C behavior on tested targets, not
portable safety of C's historical sample sign-shifts or unchecked inputs.

The resource fixture passes 3,976 complete comparisons over 11,935 owned
requests: all current sources with both controls, generated spectra/WAVs/
envelopes, every generated truncated spectrum prefix, duplicate and nonzero
input addresses, missing/unsupported sources, `.wav` fallback, complete payload/
manifest/diagnostic bytes, counters and status. Native-only checks cover
data/manifest write failure, source bounds, corrected/truncated binary envelopes,
admission, stale/exhausted owners, reentry/end rejection and changed caller
paths/descriptors. Undefined old C envelope cases are excluded from equality.

Serial main gate `70463` exited zero: 523 enabled Rust tests (502 unit and
21 integration), 436 minimal tests, native async/shared/core suites with
103/103/100 runnable CTests (one physical-audio test skipped in each), 103
audio-disabled tests, retained-C core/async 20/19 and 47 selected compatibility
checks. Strict all-target/all-feature Clippy, generated/format checks, four-target
Linux/Windows/iOS/Android library checks and Windows library Clippy pass.
The first run stopped on the C-ABI-only path decoder; that prerequisite was
corrected before the successful main gate. Only extra test assertions for
source/name limits and position failure changed during that run.

Final gate `17410` exited zero after renewed enabled/minimal safe tests,
strict Clippy and final static/shared C fixtures. Windows without the C ABI
also compiles. Fifteen complete C/native CLI phoneme/intonation scenarios
match their intended status, diagnostics and payloads; two native-only
overflow probes reject and all five regenerated master payloads match.
The selected C fixture object passes AddressSanitizer; Rust and other library
objects were not instrumented. Write/position failures are injected, not
real hardware/disk-full tests. Cross-checks are compilation, not target runtime
proof; known proactor-disabled cancellation remains excluded and open.
Only the two fixture indent prefixes and their documented provenance changed
after these gates; the exact normalized original fragments were reverified.

Fresh core/async inventories count 4,305/4,775 C/mixed lines; `compiledata.c`
falls from 1,414 to 1,232. All 29 current appendix rows match generated output,
with only that row changed. Evidence is in `/private/tmp/espeak-resource-*`,
including final/corrected gate logs, C provenance and complete CLI receipts.
Scanner C++/optional/platform limits still apply. Checks remain serial/offline,
with no hardware hold or new thermal-safety claim; the recorded provider is
unavailable. Push approval remains held; publication/fresh remote CI await
the recorded automatic approval-service quota.

The full engine migration remains active. Parent compiler state/control and
output ownership, manifest/resource reload, engine/MBROLA/ABI/platform retirement,
target execution and real audio/idle-active thermal proof still remain.

## Owned phoneme compiler output set (2026-10-11)

`rust/compiler_output.rs` owns the complete five-file output lifetime:
`phondata-manifest`, `phondata`, `phonindex`, `phontab` and `compile_prog_log`.
Creation preserves C order, partial-failure filenames and cleanup, optional
manifest fallback and program log, the exact manifest header, and initial
little-endian version/sample-rate words. Windows manifest text expands LF to
CRLF; the borrowed stderr fallback keeps the caller's own text mode.

The engine writer has a separate pool of five retained 256 KiB buffers;
existing dictionary/intonation single-writer admission and storage reuse
remain unchanged. Every owned output uses the existing engine proactor with
one write flight. Positions include staged bytes, flush waits for completion,
and explicit finish attempts every admitted stream before unregister/close.
Creation/metadata/close and borrowed diagnostic streams remain OS/CRT primitives.
No new worker, sleep, timer or polling scheduler is introduced.

`rust/compiler_output_compat.rs` registers thread-confined owners with globally
unique nonwrapping generations. Descriptor, bounded paths and each byte loan
are copied into Rust storage before effects. Snapshot/text storage is retained;
each write admits at most 16 MiB and each file has the existing signed-position
bound. Four registry slots and the five-buffer pool bound admission. Callback
reentry and stale/foreign-thread IDs reject; caller result publication follows
all creation/header effects. Callback context stays live on its owner thread
through explicit end; no foreign input reference spans a callback or wait.
The 64-bit descriptor/result sizes are 48/4,112 bytes. The nonce keeps the
Rust 1.85-compatible atomic API while passing current strict Clippy.

Native `compiledata.c` no longer owns those FILEs. Resource position/publication,
phoneme programs, table records and optional logs route through the registered
Rust output set. C still produces program/table bytes and retains compiler
state/control, diagnostic FILEs and manifest/reload ownership. The first output
error is sticky, all admitted streams finish, and failure returns before
resource reload. Native also never closes borrowed stderr. These two failure
corrections are outside successful C-output equality; no real disk-full or
hardware failure is claimed.

Four safe tests cover all five simultaneous streams, multi-chunk writes and
source reads on the same port, 64 reopen cycles with identical buffer addresses,
sixth-buffer refusal, exact headers/positions, partial-open cleanup, optional
streams, Windows text bytes and injected fallback failures. A complete frozen
C creation/header/failure-cleanup and normal-close oracle in
`tests/compiler_output_reference.inc` comes from `a2adc349:compiledata.c`.
Creation/close fragment SHA-256 values are
`bc18cf738358df2ba8c2db0ef5eceac81739754d7daf73eeaf329756b80fe08d` /
`c8b1320512cf0ac6f83a329336f4087ad6924c6088ea547fd0971755e296ecf4`;
the licensed fixture is
`6b45b66d8bb00443e8790915c63986a4ddf0b193c0946e541da1442f989bf734`.
The original blocks are unchanged. Surrounding source/context/error projections
are test adapters; inherited borrowed-stderr closes are recorded and flushed
without destroying the oracle capture.

`rust_compiler_output` passes 80 complete C-output-set comparisons over 420
begin/write requests, checking exact files/fallback bytes, positions, required
file failures and optional output behavior. Native guards cover caller
descriptor/path/byte mutation during callbacks, deferred result publication,
nested begin/write/flush/end refusal, stale generations, bad lengths/NUL/width,
exhausted buffers, sticky errors and cleanup/reuse. Eight inherited borrowed
stderr closes are recorded separately. Selected C-fixture-object ASan passes;
Rust and other library objects are not instrumented.

Final serial offline gate `96605` exits zero: 527 enabled-feature Rust tests,
440 minimal tests, strict Clippy/format/generated tables; native async/shared/
sync suites have 104/104/101 runnable CTests (one physical-audio skip each),
explicit audio-off 104, retained C 20/19, and selected proactor-off 48. Linux,
Windows, iOS and Android library cross-checks pass; Windows strict and minimal
checks pass. Cross-checks establish compilation, not target execution. The
known proactor-off cancellation exclusion remains open. Fifteen complete CLI
compiler cases match status/diagnostics/bytes; two native-only overflow probes
reject, all five master payloads match, and the complete 25,632-byte master
program log is identical. Fresh inventories count 4,255/4,725 C/mixed lines;
`compiledata.c` falls from 1,232 to 1,182, with 33 C/eight mixed/four bridges.
All 29 appendix rows match generated output; only that row changes.

Evidence is in `/private/tmp/espeak-output-*`, including provenance, CLI hashes,
gates, ASan and row verification. The pinned Loadngo `843ae1de` includes IOCP
reused-handle fix `e421f5ea` and deadline fix `84b2b6a8`; local Loadngo dev
`6b2489d1` has no newer proactor diff. No remote refresh was attempted while
the recorded approval-service quota remains unchanged. Jay's push approval is
retained; publication/fresh remote CI are pending that quota. No device is held
and no thermal-safety claim is made: the recorded provider remains unavailable.
The full engine goal remains active, including remaining C/compiler control,
MBROLA/ABI/platform retirement and real target/audio/idle-active thermal proof.

## Owned phoneme compiler tables (2026-10-11)

`rust/compiler_tables.rs` replaces complete `ReservePhCodes`, `LookupPhoneme`,
`FindPhonemeTable`/`FindPhoneme`, `ImportPhoneme`, `StartPhonemeTable`,
`EndPhonemeTable` and `WritePhonemeTables` policy. Rust owns reserved codes,
sparse/NULL lookup, control-start offsets, line narrowing, inherited table
copies/local flags, qualified references (including defined no-slash/empty
mnemonics), import override/preserved identity and nonvowel end-type rules,
undeclared diagnostics with their doubled error weight, and explicit 16-byte
records and table headers. Truncated 31-byte names, first-match ordering,
optional base `_`, record selection and legacy byte-count narrowing remain.
Packing coalesces all records into one bounded publication, preserving final
bytes and state while deliberately changing primitive write granularity.

Four registered owners retain fixed-capacity metadata/row arenas. Each session
admits at most 149 tables of 256 rows; owned and compatibility row arenas are
separate, each at most 610,304 bytes, with a 615,672-byte packed-payload bound.
The remaining C property/bytecode producer uses only current compatibility
rows between calls. Older returned rows are read-only; all views are quiescent
during a call/callback and expire on end. Current rows snapshot before effects;
results/views publish after effects. C no longer owns table arrays or per-table
allocations. No foreign row reference spans a callback/proactor wait. The
compatibility descriptor/result are 64/32 bytes on 64-bit targets, with copied
name/target inputs, nonwrapping generation IDs and reentry/stale/admission
refusal. The bridge reads its temporary `phoneme_out` target only for import,
while it is live, rather than reading an expired procedure-stack pointer for
unrelated operations. Final table output uses the existing registered compiler
output owner/proactor; no new thread, timer, sleep or polling loop is added.

Native handles 255-byte qualified references without the old `buf[200]`
overflow. The parent's unused n+1 terminator is written only where it fits;
count 255 previously wrote past the C table allocation. Both have direct
native safe/ABI regressions and are outside C equality for unsafe inputs.
Callback base failure restores the previous active table/count. Packing admits
only closed bounded tables, is not replayed after failure, and compiler table
errors join sticky output errors before resource reload. C still owns initial
property and bytecode production, keyword/control/manifest/reload and diagnostic
FILEs; raw-view lease correctness is a serialized C ABI contract, not proof of
full engine memory safety or a pure Rust engine.

Four safe tests cover all reserved codes, hole/NULL/control lookup, mnemonic
and line narrowing, inherited/imported properties, repeated undeclared
diagnostics, qualified empty references, explicit header/record bytes, local
filtering, failed publication/base rollback, 149-table/255-code bounds and 64
reopens with identical row/payload storage addresses. The complete frozen C
controllers in `tests/compiler_tables_reference.inc` come from
`92980b44:compiledata.c`. Original fragment SHA-256 values are
`d4d245c9b95071f4491e682c18ea14bce06053010f098fb2c0cfd865a7665583`,
`a59a358405107741648621635f943f41385d0fd46ac88f55e7e89d6a537813bd`,
`193281c3a500a1e15078463e423fc448112e6d1205bea91693f77478f05b4f7b`;
the licensed combined fixture is
`b6de4ecadb25bdb114e8508c7c4a84f4e7b31bc77b53f431115fc2e9116303d9`.
The blocks are unchanged; adapters select retained C output and defer its row
free only until post-packing state is captured, then actually free every row.

`rust_compiler_tables` passes 1,201 complete C episodes and 222,883 exact
state/diagnostic/record comparisons, including sparse/randomized rows, all
lookup controls, NULL/invalid names, inherited/truncated/missing bases, imports,
qualified references, undeclared rows, table cap and final packed bytes.
The compatibility view address is stable across all episodes. Native guards
cover descriptor/name/target mutation during callbacks, deferred publication,
nested operation/begin/end refusal, stale/full owners, bad NUL/operation/target
inputs, wide references, full-table terminator, failure/no replay and cleanup.
The fixture records 1,202 packed writes including an injected failure. Selected
C-fixture-object ASan passes; Rust/library objects are not instrumented.

Serial gate `41744` passes Rust 531 enabled/444 minimal, strict Clippy/format/
generated tables, native async/shared/sync (105/105/102 runnable, one physical
audio skip each), explicit audio-off 105, retained C 20/19, selected proactor-off
49, four-target compilation and Windows strict/minimal checks. That script
stops on an expanded CLI harness loop-selection mistake, not an engine mismatch.
After correcting the harness and rebuilding the async C bridge's target-lifetime
change, follow-up `39223` exits zero with the complete renewed async suite,
21 intended-status CLI C/native comparisons, two native overflow probes, all
five master payloads and the complete 25,632-byte program log identical, selected
C-object ASan and fresh inventories. Final `14943` exits zero after the additional
native ABI boundary assertions, renewed static/shared table fixtures and ASan.
Cross checks are compilation only; the known legacy cancellation exclusion
and target/audio/idle-active thermal proof remain open.

Only a surplus final blank line in the frozen fixture was removed after these
gates to satisfy staged whitespace checks; every original controller fragment
and the licensed artifact's updated hash were reverified, with no code change.

Fresh C/mixed inventories count 4,107/4,577 lines. `compiledata.c` falls from
1,182 to 1,034 (34 C/seven mixed/six bridges); all 29 appendix rows match, with
only that row changed. The scanner calls small indirect forwarding wrappers C;
their old policies live only in the retained build/oracle. Evidence is in
`/private/tmp/espeak-tables-*`. Checks stay serial/offline with no hardware hold
or thermal-safety claim; the recorded provider is unavailable. Push approval
remains retained, with publication/fresh remote CI pending the unchanged
approval-service quota. The full engine migration remains active.

## Owned phoneme bytecode assembler (2026-10-11)

`rust/compiler_program.rs` now owns the retained word buffer, branch stack,
single-statement THEN folding, ELSE backward chains, IF/NOT/AND/OR/property/
stress tests, ELIF/ENDIF patching, vowel transition packing, switch words,
sound parameter clamps/resource addresses and tone envelopes. The native C
property/keyword controller appends through bounded calls; its raw program
buffer and branch pointers are excluded from the native build. Resource calls
still use the existing Rust resource owner and proactor source reads. Completed
program/log bytes go through the existing owned proactor output set.

Four registered assembler leases retain a 2,020-word arena and 4,040-byte
publication arena each, preserving C's twenty initialized slack words beyond
the nominal limit. Its separate 1,999-word condition admission limit remains.
The twelve-entry branch array uses checked indexes;
eleven nested levels fit its original one-based convention. Normal assembly,
token scratch and diagnostic composition require no per-instruction allocation.
All words and program-log fields pack explicitly little endian. A phoneme's
program/log address preserves the original 16-bit narrowing, while a procedure
call address retains the wider index position. No assembler storage is exposed
as a borrowed C view, and reset/reopen retains its capacity.

The compatibility descriptor, emitted word loans and callback results are
copied before later effects. Token/path scratch is owned and initialized;
callbacks are synchronous and cannot retain it or unwind. Publication occurs
after callbacks, supports overlapping caller input/result storage, and rejects
reentry, stale IDs and pool exhaustion. Output failure is terminal for that
program: it cannot replay either write before reset. Cleanup releases the
assembler before the remaining context owners, without adding a worker, timer,
polling loop or sleep. Local Loadngo `dev` remains `6b2489d1`; its proactor tree
matches the pinned `843ae1de` tree. This is a local inspection, not a remote
refresh or Windows execution claim.

Native bounds also cover paths the retained C controllers cannot safely prove:
overfull word/IF buffers, invalid stress shifts, stale offsets after a stress
directive clears a live program, and condition EOF that previously kept asking
for input. The defined-input oracle retains the C controller policies; native
refusals do not replay C. C still owns the outer keyword/property/feature/IPA
dispatcher, procedure name/address registry, source-file controller, manifest
reload, diagnostics and engine projections. In particular, this checkpoint does
not retire that registry's old name-array dimensions or the public compiler's
unbounded path copies. Those owners still require migration.

The eleven complete C controllers/helpers in
`tests/compiler_program_reference.inc` are frozen from
`13eb13e9:src/libespeak-ng/compiledata.c`, retaining their license. Original
transition/range and tone/sound/branch/switch fragment SHA-256 values are
`529b71e3d1d51252dc32d7a45bdc1d90013b5fc7eec4042d0675fd20a68048c1`
and `bc2c7af912eac5cbadbc742ce9c75ad14de3fe2a5615c11eb766383073011d79`;
the bracket helper is
`b2fbcd4888e6bcc36858a9c803b82aeed9318f7db1201259af2117bfc5136ce7`.
The licensed normalized artifact is
`54e1ee9f7cb1e4496512838f6a87a97abc66fe103396af03c5e1f3072fce68c3`.
Controller bodies are unchanged; only fragment separator/EOF whitespace is
normalized. The C fixture compares complete word prefixes, live branch state,
diagnostic bytes/counts, token consumption and literal primitive traces across
2,000 complete episodes plus the separate IF-limit boundary and 84,135
comparisons. It covers all transition
attributes, signed range/parameter boundaries, sound and tone paths, NOT/
extended phoneme selectors/stress lists, nested ELSE/ELIF chains, returning
branches, single-statement folding and wider procedure positions. Native-only
guards cover descriptor/token/word admission, source failure, reentry, loan
mutation, overlapping publication, stale/full owners, bounds and write failure.
Five Rust regressions include retained buffer addresses across 64 publications.

Renewed complete compiler probes match 29 native/full-C scenarios in exact
status, stdout/stderr, all four phoneme payloads and complete program log;
five native-only overflow/EOF probes reject. All five regenerated master
payloads and the complete master program log match full C byte for byte.
Both the successful 2,020-word CLI boundary and the original IF-limit error
path match C, including its partial program and later length instruction.

Serial full gate `81751` exits zero: complete native static/shared/sync suites
(106/106/103 runnable, one physical-audio skip each), audio-off 106, retained
C 20/19, selected proactor-off 50, strict/generated and four-target checks.
After the capacity and cleared-branch review, final gate `59219` exits zero
with source edits frozen throughout: Rust 536 enabled/449 minimal, strict
Clippy/format, renewed complete static native suite, complete assembler
fixtures rebuilt in shared/sync/proactor-off/audio-off configurations, renewed
four-target and Windows strict/minimal checks, all 29 CLI comparisons and
five native-only probes, byte-identical master payloads/program log, selected
C-object ASan and fresh inventories. ASan instruments the selected C fixture
object, not Rust or other library objects. Cross checks prove compilation;
the known legacy cancellation exclusion remains open. The last full native
suite took 522 seconds versus approximately 143 earlier; no telemetry here
establishes the cause or thermal safety.

Fresh C/mixed inventories count 3,788/4,258 lines. `compiledata.c` falls from
1,034 to 715 (39 C/seven mixed/nine bridges); all 29 appendix rows match the
generated output. Indirect `ProgramCall` forwarding wrappers appear as C in
the scanner, although the old assembler controllers are excluded from the
native build. Remaining C property/instruction and engine/platform owners
still need migration; this is not full engine completion.
Evidence lives in `/private/tmp/espeak-program-*`. Cross-target checks remain
compilation evidence; physical audio, real target execution and idle/active
thermal proof remain open. The full migration remains active, and approved
publication/fresh remote CI still awaits the recorded approval-service quota.

## 2026-10-12: Complete phoneme definitions, procedures and IPA

`rust/compiler_definition.rs` now owns complete `CompilePhoneme`,
`CallPhoneme`, `DecThenCount` and numeric maximum policy: declarations,
property/type/feature accumulation, imports, start/end/voicing/stress rules,
all instruction groups, branches/transitions/sound/tone dispatch, final
voiced/deleted/default-length/local flags and program publication.
The existing assembler pool supplies four retained compiler leases. Each
holds at most 50 procedure records with initialized 256-byte names and full
i32 addresses. Reset clears the records while retaining their allocation;
empty procedures do not consume a slot, first matching procedure names take
precedence over phonemes, and CALL inherits the original selected properties
and non-articulation flags. Row and log addresses retain their u16 narrowing;
procedure CALL retains high address bits.

This retires the native C registry whose rows were only 50 bytes wide while
the scanner admitted 255-byte names. Native 255-byte declaration/call probes
avoid that overwrite; the frozen C oracle admits only names within its actual
slot width. IPA assembly uses initialized fixed byte/word scratch. It retains
NULL-as-space, permissive UTF-8, partial four-character hex scanning, join
flags, embedded-NUL strlen behavior and the incomplete `0x` prefix accepted
as zero by scanf. Native unsafe shifts/expanded output and length modifiers
that formerly tripped the C assertion now refuse with a bounded diagnostic.
These guards are outside defined-input C equality.

The compatibility descriptor and locale map copy before effects; row/token
scratch is owned. Copied current rows project before dependent effects and
final publication, preserving self calls and import/resource observations.
Callbacks are synchronous and cannot retain loans or unwind; results publish
only after effects. Reentry, stale/full leases, caller descriptor/map mutation
and terminal output failure without replay are checked. Procedure state shares
the assembler lease rather than adding a nested lock or foreign stack row.
Source/resource reads and program/log writes retain the existing bounded
proactor paths; no worker, scheduler, timer, polling loop or sleep is added.
Local Loadngo `dev` at `6b2489d1` still has the same proactor tree as pinned
`843ae1de`, including HANDLE reuse `e421f5ea`; no remote refresh or Windows
runtime claim is made.

Four original C controller/helper bodies are frozen from
`13eb13e9:src/libespeak-ng/compiledata.c`, retaining their license in
`tests/compiler_definition_reference.inc`. Original maximum-number and
complete definition/CALL fragments have SHA-256
`36156aef1aaeeaae38151c5f3da0e2b52777850ff0b242d87d1d5eefa6462138`
and `4b8dd1478387103e80034fbf4579ed0181f49ce9d229df77d9221ecac963a059`;
the licensed normalized artifact is
`b9a472cf1e9014442df3c747c3c1f5bc80992acc64b3e9d4714ae58780d34c51`.
All four bodies match the source commit. The fixture reuses the eleven
previously frozen assembler controllers and shared primitive effects; the
independent full-C CLI provides complete compiler evidence. One thousand
complete episodes plus registry/edge cases pass 40,641 comparisons of rows,
words, index/log bytes, diagnostics, token consumption and literal primitive
traces. Coverage includes all simple instruction groups, property/feature
errors, self/missing/procedure calls, imports, stress clearing, sound/tone/
transition dispatch, branches, EOF/early endings, the 50-procedure cap and
positions beyond 65,535. Four safe Rust regressions check IPA encodings,
partial-hex/NUL and flag behavior, bounds and retained registry allocation.

Serial final gate `86832` exits zero with implementation source frozen:
Rust 540 enabled/453 minimal; strict Clippy, formatting and generated tables;
complete static/shared/sync native suites (107/107/104 runnable, one physical
speaker test skipped in each), retained C 20/19, selected proactor-off 51 and
audio-disabled 107. Linux, Windows, iOS and Android library checks plus
Windows strict/minimal checks pass; they establish compilation only. All
55 complete CLI compiler cases match status, stdout/stderr, payloads and log;
seven native bounds cases refuse, and all five regenerated master payloads
and the complete program log match C byte for byte. Selected C-fixture ASan
passes; it instruments that fixture object, not Rust/library objects. The
known legacy cancellation exclusion remains open.

Fresh C/mixed inventories are 3,428/3,898 lines, a net reduction of 360 in
both configurations. `compiledata.c` is 355 lines (29 C/six mixed/11 bridges),
and all 29 appendix rows match generated output. Its native `CompilePhoneme`
is now an eight-line forwarding bridge; source-file control, keyword/property
maps and token projections, public path admission, manifest/reload/diagnostic
ownership and raw engine/global views remain. Full engine/MBROLA/ABI/platform
migration, real target/audio execution and representative idle/active thermal
proof remain open. Validation was serial; unavailable thermal telemetry does
not establish thermal safety. Evidence lives in
`/private/tmp/espeak-definition-*`. The full goal remains active; approved
publication and fresh remote CI await the unchanged approval-service quota.

## 2026-10-12: Complete source-file and token policy

`rust/compiler_items.rs` now owns complete native `NextItem` control over
owned scanner results: EOF/bracket behavior, unchanged initialized token tails,
line/terminator updates, numeric diagnostics/values, property/condition/keyword
classification and phoneme lookup. `compiler_items_tables.rs` stores all
122 ordered entries (75 keywords, 18 conditions and 29 properties). Exact
case and first-match precedence preserve the duplicate `ChangeIfStressed`
entry rather than silently changing it. The native C tables, string search
and numeric/token controller are excluded; the remaining wrapper projects
owned results into the serialized engine context.

The adapter copies its descriptor and 256 previous-token bytes before
scanner/effects, ends the short source borrow before callbacks, and publishes
only afterward. Lookup and diagnostics observe the scanner's updated line.
Full descriptor/previous-token snapshots, invalid admission, overlapping input/
result storage and callback reentry are checked. A single immediate admission
attempt refuses reentry; it does not spin or add a scheduler. The existing
source session/retained arenas/proactor reads still own actual contents.

`rust/compiler_files.rs` also owns complete `CompilePhonemeFiles`: initial
progress/line reset, BOM/unknown-keyword policy, include opens and missing-file
continuation, table and phoneme/procedure dispatch, missing-table early return,
include EOF/pop and final table termination. Twelve initialized frames and
4,096-byte maximum path scratch are fixed storage. Copied roots use the
platform width supplied by the caller; joins preserve snprintf truncation.
Include names preserve strncpy0 padding and the previous initialized filename
tail on strcpy restoration, with exact saved lines/files. No C include stack
survives in the native context. C retains OS/path open, primitive projections,
context/diagnostic ownership and the public compiler driver.

Callback descriptors/root/token/frame inputs copy before later effects and
cannot be retained or unwound through. Reentry, overlapping publication,
missing terminators/oversized roots and effect failure refuse without replay.
A refused token read now propagates its first status through `ProgramNext`;
subsequent reads return that status before touching the source. The actual
production C bridge is extracted by CMake into the file fixture. Its native
regression proves refusal leaves the cursor unchanged and terminates the
controller, rather than retrying without progress. This C-only correction
was applied after the first frozen gate reached terminal state.

Original table definitions and the complete file controller are frozen from
`c8393cc0:src/libespeak-ng/compiledata.c`, retaining their license in
`tests/compiler_{items,files}_reference.inc`; the item fixture also uses the
previously frozen full C scanner. Original metadata/controller SHA-256 values
are `beba25c3fdb86c096e82f2f441507c2a2b4f6783e0a1755ca2d4dd5ce893ff14`
and `965e583a107b323115c7a836590cdd0177555a12110a16bc365b0b828fa676b9`.
Licensed fixture hashes are
`1fc7860fe586d087025a23b0c9f1ff53a0a445197ec58d963a2d03e60f317b27`
and `9e0cd637f63f4324a93287d4837d94b1d212e0d7601bb3a12dd4767cd0c3ba62`.
All match the source checkpoint. Six safe regressions pass; complete C
NextItem/table/check/replay tests compare 278,601 results, full initialized
text/terminators, numeric diagnostics, lines and EOF/replay behavior. Two
thousand complete C include/file episodes compare 12,000 trace/state/row/
diagnostic results, covering empty/failed/repeated/nested includes, the depth
limit, truncated names, restored tails/lines, table/definition dispatch and
early exit. Native guards cover descriptor/root/previous-token mutation,
reentry, result overlap, bounds, injected effect failure and actual bridge
refusal without retry.

Serial broad gate `31286` exits zero with implementation source frozen:
Rust 546 enabled/459 minimal, strict Clippy/format/generated checks, complete
static/shared/sync native suites (109/109/106 runnable with one physical-audio
skip each), audio-disabled 109, retained C 20/19, selected proactor-off 53,
four-target library checks and Windows strict/minimal checks. After the
C-only bridge correction, source-frozen renewal `64558` exits zero: complete
static suite, five compiler fixtures in each shared/sync/proactor-off/audio-off
configuration, renewed independent full-C CLI comparisons, both selected
C-object ASan fixtures and fresh inventories. Sixty-six complete phoneme/
intonation CLI cases match exact status, stdout/stderr, all payloads and log;
seven native bounds cases refuse safely. All five master payloads and the
complete program log match C byte for byte. ASan covers the selected C fixture
objects, not Rust/library objects. Cross-target checks prove compilation;
physical audio, real target execution and the legacy cancellation exclusion
remain open. Serial validation does not prove thermal safety; the recorded
thermal provider remains unavailable.

Fresh C/mixed totals are 3,343/3,813 (net 85 removed in both configurations).
`compiledata.c` is 270 lines (41 C/four mixed/14 bridges), and all 29 appendix
rows match generated output. `NextItem` and `CompilePhonemeFiles` now forward
in six/three lines; indirect callback forwarding appears as C in the scanner.
The public compiler's unbounded source/destination path copies, C manifest
reads, context/diagnostic/reload and broader engine/MBROLA/ABI/platform owners
remain to be replaced. The full migration stays active. Local Loadngo
`cddc8fa0` has the same proactor tree as pinned `843ae1de`; this is a local
snapshot, not remote freshness or Windows execution evidence. Approved
publication/fresh remote CI await the unchanged approval-service quota.
Evidence lives in `/private/tmp/espeak-files-*` and
`/private/tmp/espeak-items-*`.

## Remaining migration

The definitive list of what is still C, and where loadngo's proactor
applies, is [REMAINING_PORT.md](REMAINING_PORT.md). It is generated from the
Rust-core build by `tools/c_inventory.py`, which preprocesses each source as
the build compiles it. In short: the text front end (translation, numbers,
dictionary and clause glue), the synthesis and engine glue (`synthesize.c`,
`synthdata.c`, `speech.c`, `voices.c`), Klatt and speechPlayer, MBROLA, the
asynchronous API, audio output and libsonic, the data compilers, the CLI,
and the Android, Windows and Emscripten front ends.

Keep each replacement runnable against the retained C oracle. A complete
port must cover the whole current feature set; passing the first-stage
regressions does not establish a pure Rust speech engine or thermal safety.
