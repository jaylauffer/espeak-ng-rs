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
| Scalar language letter predicates | `rust/letters.rs`, generated accent table | Replaces `IsLetter` used by rule matching, vowels and stress; borrows prepared language configuration; full language/voice setup still C |
| Contextual dictionary exception lookup | `rust/lookup.rs` | Replaces `LookupDict2`; explicit grammatical context, conditions, stress/word flags, multiword matches, precedence and legacy output side effects |
| Dictionary alphabet compression | `rust/word_key.rs` | Replaces `TransposeAlphabet`; language maps, frequent pairs, six-bit packing and byte-exact legacy hash tails |
| Compiled phoneme tables and header | `rust/phoneme_data.rs` | Replaces C table parsing, inheritance overlays, name lookup and phondata header decoding; spectrum interpretation still C |
| Compiled phoneme-program VM | `rust/phoneme_program.rs` | Replaces `InterpretPhoneme` bytecode execution, instruction widths and vowel-switch decoding; uses native bounded context or an explicit owner environment |
| Phoneme condition/stress evaluation | `rust/phoneme_context.rs` | Replaces `InterpretCondition`, `StressCondition` and vowel-position counting; explicit initialized list bounds, table resolution and isolated previous-vowel snapshot; phoneme-list construction and stress assignment still C |
| Data I/O and resident assets | `rust/data_io.rs`, `rust/resident.rs`, optional `proactor` feature | Native library loads and indexes complete resident asset sets; caller-owned loadngo proactor, reusable bounded buffer, one plan/read in flight; legacy C byte loader still uses stdio |
| Accelerator capability | `rust/acceleration.rs`, optional `npu` feature | Core ML device discovery on macOS; portable CPU fallback; no NPU speech computation enabled |

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
outside proactor completion handlers. Full clause ownership, stress assignment
and spectrum lookup remain to be ported.

The last compiled sound record can end at EOF. The original C interpreter
reads one word past its allocation for sound lookahead; native execution
treats exactly that terminal boundary as Return. Other truncated reads and
invalid/cyclic control flow return an error. The C adapter keeps default
phoneme parameters on failure. Bounded storage access prevents native condition
scans from crossing clause ends or scanning past the previous-vowel snapshot.
The serialized C adapter still requires valid live pointers and initialized
spans; these guards do not establish safety of the remaining C engine.

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

## Remaining migration

1. Port language/voice configuration and spectrum interpretation.
   Connect compatibility C data
   loading to caller-owned resident assets during native engine-instance work.
2. Port clause/SSML parsing, number pronunciation and translation. Replace
   process-global mutable state with explicitly owned engine instances while
   retaining the C API's serialized compatibility behavior.
3. Port phoneme lists, stress, intonation, lengths and synthesis command queues.
4. Port formant waveform generation, Klatt, optional speechPlayer/MBROLA/sonic
   support; reuse PCM buffers and integrate bounded output/cancellation with
   the host. Evaluate NPU eligibility against measured actual workloads.
5. Port CLI/data compilers and remaining platform integrations (Android,
   Windows/SAPI, Emscripten and audio output). Remove the C dependency only
   once complete language/audio/API parity and real-platform gates pass.

Keep each replacement runnable against the retained C oracle. A complete
port must cover the whole current feature set; passing the first-stage
regressions does not establish a pure Rust speech engine or thermal safety.
