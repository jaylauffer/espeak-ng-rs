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
| Compiled dictionary storage and indices | `rust/dictionary.rs`, `rust/rules.rs` | Replaces C bucket/rule indexing and `HashDictionary`; native resident owner caches indices; letter-to-phoneme rule VM still C |
| Contextual dictionary exception lookup | `rust/lookup.rs` | Replaces `LookupDict2`; explicit grammatical context, conditions, stress/word flags, multiword matches, precedence and legacy output side effects |
| Dictionary alphabet compression | `rust/word_key.rs` | Replaces `TransposeAlphabet`; language maps, frequent pairs, six-bit packing and byte-exact legacy hash tails |
| Compiled phoneme tables and header | `rust/phoneme_data.rs` | Replaces C table parsing, inheritance overlays, name lookup and phondata header decoding; remaining phoneme-program/spectrum interpreter still C |
| Data I/O and resident assets | `rust/data_io.rs`, `rust/resident.rs`, optional `proactor` feature | Native library loads and indexes complete resident asset sets; caller-owned loadngo proactor, reusable bounded buffer, one plan/read in flight; legacy C byte loader still uses stdio |
| Accelerator capability | `rust/acceleration.rs`, optional `npu` feature | Core ML device discovery on macOS; portable CPU fallback; no NPU speech computation enabled |

The safe library has no runtime dependency on the C engine. The `c-abi`
feature adds only the compatibility exports; it does not wrap C algorithms.
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
interpreter port; their complete instruction/spectrum schema is not validated
yet. The `resident_data` example is a standalone host, loading real assets
through loadngo before indexing them outside the completion loop.

`OwnedDictionary::lookup` uses those cached indices and an immutable
`lookup::Context`; it borrows the selected phonemes and does no allocation or
I/O. Prepare keys with `word_key::Alphabet` for languages using compression.
Keep the returned descriptor and the whole buffer: embedded NUL bytes count
toward matching, while the unchanged tail can still affect the dictionary hash.
The compatibility C frontend snapshots its grammatical state for this same
Rust matcher. Abbreviations, text replacement, repetition and ending handling
in `LookupDictList` and the letter-to-phoneme `MatchRule`/`TranslateRules` VM
remain C.

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

## Validation evidence, 2026-10-04

All results below are local to this checkout and Mac; Linux/Windows
runtime execution awaits CI. The new `.github/workflows/rust.yml` runs
Cargo checks/tests on Linux, macOS and Windows, plus both static and
shared speech parity on Linux/macOS. No push or CI run has been performed.

### Contextual lookup and compression stage

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

1. Port the letter-to-phoneme rule VM, voice/language options
   and the phoneme-program/spectrum interpreter. Connect compatibility C data
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
