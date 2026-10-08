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
| Letter-to-phoneme template VM and string groups | `rust/rule_match.rs` | Replaces `MatchRule`, `$list`/`$p_alt` scoring and `IsLetterGroup`; prefix lookup frontend and trace formatting supplied through an explicit environment; `TranslateRules` orchestration still C |
| Scalar language letter predicates | `rust/letters.rs`, generated accent table | Replaces `IsLetter` used by rule matching, vowels and stress; borrows prepared native language configuration |
| Suffix removal and UTF-8 output | `rust/suffix.rs` | Replaces `RemoveEnding` and `utf8_out`; bounded edit planning, spelling repairs and explicit grammatical/history effects; long words supported |
| Word-stress extraction and assignment | `rust/word_stress.rs` | Replaces `GetVowelStress` and `SetWordStress`; sparse selected tables, all language stress-position rules and explicit previous-stress effects; clause/intonation stress remains C |
| Translation word transforms | `rust/word_stress.rs`, `rust/phoneme_word.rs` | Replaces `ChangeWordStress`, `AppendPhonemes` and `ApplySpecialAttribute2`; admitted writes, checked vowel/stress counters and explicit compatibility byte signedness |
| MBROLA mapping storage and names | `rust/mbrola.rs` | Replaces table reads/storage and `GetMbrName`; reusable transactional owners, validated little-endian records and explicit prefix effects; backend process/audio remains C |
| Contextual dictionary exception lookup | `rust/lookup.rs` | Replaces `LookupDict2`; explicit grammatical context, conditions, stress/word flags, multiword matches, precedence and legacy output side effects |
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
| Static translator presets and alphabet classification | `rust/language.rs`, generated native tables | Replaces `SelectTranslator` configuration and `AlphabetFromChar` classification; shared immutable tables, native instance options, bounded dictionary names and prepared letter/compression views; C adapter retains translator allocation |
| Data I/O and resident assets | `rust/data_io.rs`, `rust/resident.rs`, optional `proactor` feature | Native library loads and indexes complete resident asset sets; caller-owned loadngo proactor, reusable bounded buffer, one plan/read in flight; legacy C byte loader still uses stdio |
| Accelerator capability | `rust/acceleration.rs`, optional `npu` feature | Core ML device discovery on macOS; portable CPU fallback; no NPU speech computation enabled |
| Common copy, stream-word and random primitives | `rust/common_primitives.rs`, `rust/common_primitives_compat.rs` | Replaces `strncpy0`, `Read4Bytes`, `espeak_rand` and `espeak_srand`; owned native random state, atomic compatibility state, bounded copy/padding and host CRT stream reads |
| `intonation.c` pitch calculation | `rust/intonation.rs`, `rust/intonation_compat.rs` | Replaces `CalcPitches` and `CalcPitches_Tone`; copied phoneme-list snapshot, borrowed compiled tunes and phoneme table, native head/nucleus tables, atomic rejection of inputs C reads out of bounds |
| `setlengths.c` `CalcLengths`, `intonation.c` envelope tables | `rust/lengths.rs`, `rust/lengths_compat.rs`, `rust/envelope.rs` | Replaces `CalcLengths`; copied list snapshot including the entries C reads past the clause, explicit cross-clause syllable state, ordered host callbacks for embedded speed and tone envelopes; `envelope_data` and `env_fall` are exported from Rust |
| `phonemelist.c` | `rust/phoneme_list.rs`, `rust/phoneme_list_compat.rs` | Replaces `MakePhonemeList`, `SubstitutePhonemes`, `SetRegressiveVoicing` and `ReInterpretPhoneme`; owned working list with native phoneme-program storage, host table selection, outputs as (table, slot) references resolved to the legacy record pointers |
| `synthesize.c` `Generate` | `rust/generate.rs`, `rust/generate_compat.rs` | Replaces the clause driver's decisions and resumable state; queue writers (`DoPause`, `DoPitch`, `DoAmplitude`, `DoSpect2`, `DoSample3`, markers, embedded commands), phoneme programs and frames stay C behind one ordered effect callback; MBROLA keeps its own generator |
| `synthesize.c` command writers | `rust/commands.rs`, `rust/commands_compat.rs` | Replaces `DoSpect2`, `DoSample2`/`DoSample3`, `DoPause`/`PauseLength`, `DoPitch`, `DoAmplitude`, `EndPitch`, `EndAmplitude` and `StartSyllable`; Rust owns their shared state; the queue, spectrum lookup, smoothing and frame copies are host operations, and formant-transition pauses are returned from the lookup rather than issued re-entrantly |
| `wavegen.c` | `rust/wavegen.rs`, `rust/wavegen_compat.rs` | Replaces the formant wave generator and queue consumer: `WavegenFill2`, `Wavegen`, `SetSynth`, `PeaksToHarmspect`, `AdvanceParameters`, breath resonators, `PlaySilence`, `PlayWave`, `SetPitch`, `SetAmplitude`, `SetEmbedded`, echo setup, `WavegenSetVoice`, `GetAmplitude`, `InitBreath` and `WavegenInit`; Rust owns the generator state and voice copy; the embedded values and sample rate stay shared C memory read in place; Klatt, MBROLA, sonic, markers, output hooks and the random generator are host operations |
| `wavegen.c` queue and echo ring | `rust/wave_memory.rs`, `rust/wavegen_compat.rs` | Rust owns `wcmdq` with its head/tail and the echo ring (`espeak_rs_wave_memory`) and replaces `WcmdqFree`, `WcmdqUsed`, `WcmdqInc`, `WcmdqIncHead`, the queue reset in `WcmdqStop`, and Klatt's echo reads and writes; the queue writers below run in Rust |
| `synthesize.c`/`synth_mbrola.c` queue writers | `rust/wave_memory.rs`, `rust/commands.rs`, `rust/wave_memory_compat.rs` | Replaces `DoMarker`, `DoPhonemeMarker`, `DoPhonemeAlignment`, `DoSonicSpeed`, `DoVoiceChange`'s queue entry, `DoEmbedded` (speed changes, sound icons, marks, audio and generator commands) and the MBROLA output entries; the command writers write the Rust queue directly; smoothing and Klatt's and speechPlayer's look-ahead still read it in place |
| `speech.c` output buffer, `synthesize.c` frame pool | `rust/output.rs`, `rust/wave_memory_compat.rs` | Rust allocates, resizes and frees the PCM output buffer and owns its cursor (`espeak_rs_output`; C's `out_ptr`/`out_end` are macros over it), and owns the round-robin pool of modified frames (`espeak_rs_frame_pool`) with the storage callback the frame copy, transition and smoothing calls use; Klatt, speechPlayer, MBROLA, sonic and events still advance or read the cursor in place |
| `speech.c` event list, `wavegen.c` embedded values | `rust/events.rs`, `rust/wave_memory_compat.rs` | Rust allocates, resizes and frees the event list (`espeak_rs_events`) and replaces `MarkerEvent`, `RescaleEventSamples`, list termination and the message terminator; the embedded values and their defaults are Rust statics under C's names, which the remaining C readers and writers address in place |
| Engine file I/O | `rust/engine_io.rs`, `rust/data_io.rs` | Every engine file read (phoneme data, dictionaries, voices, variants, sound icons and their configuration, MBROLA tables, the voice catalogue's files) goes through loadngo's proactor (io_uring; epoll, kqueue or IOCP elsewhere); `std::fs` only without the `proactor` feature or proactor |

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
Rust matcher. Abbreviations, text replacement, repetition and ending handling
in `LookupDictList` remain C.

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
