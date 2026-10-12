# Remaining port: the definitive list

This is everything in eSpeak NG that is still C (or C++, Java, JavaScript)
in the Rust-core build, as of 2026-10-12 (`dev`). It also says where loadngo's
proactor applies. It is derived, not recalled. `tools/c_inventory.py`
preprocesses every source file exactly as the build compiles it, so code
behind `#ifndef USE_RUST_CORE` drops out. It then lists every function that
survives:

    cmake -S . -B build-rust-core -DUSE_RUST_CORE=ON -DUSE_ASYNC=OFF -DUSE_MBROLA=OFF -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    python3 tools/c_inventory.py build-rust-core --functions

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

**Totals:** 3,280 lines of C and mixed logic in the current library/CLI
configuration. A separate current async/MBROLA-on macOS inventory
(`tools/c_inventory.py build-rust-async --functions`) counts 3,750 C/mixed
lines; it includes optional engine code and cannot be substituted for the
sync configuration above. `espeak_command.c` now contains 11 forwarding
bridges and no C/mixed logic in that native build. The earlier optional async/MBROLA inventory counted 1,855
lines in those sources; its queues have since moved to Rust in proactor
builds (item 17). The earlier C++ scan counted 622 lines in speechPlayer; class method bodies
are not included by the current top-level function scanner. Platform front
ends also remain. The text front end accounts for
1,428 lines (items 1 to 7), and the data compilers for 323 lines (item 19
plus two lines of spectrum diagnostic projection in this configuration).

## A. Text front end

1. **`translate.c`** (395 lines; 92 C, 0 mixed, 13 bridges). Native
   `rust/translate_word2.rs` owns complete `TranslateWord2`: embedded commands,
   capacity reserves, capital/pause control, spelling restoration, combination
   admission, two alternate-language retries, stress/tone/list assembly and
   dictionary/table/pitch restoration. Its old `SetPlist2` initializer also
   leaves the native build. Checked numeric source/list/command projections,
   fresh scalar/classification reads and primitive calls still retain C owners;
   global/row/translator lifetime and full engine memory safety remain open.
   Initialized 163-byte replacement scratch admits the primitive's complete
   text result plus two predecessor bytes, and scopes/restores alternate rule
   context. Ten Rust regressions and 200,000 unchanged-C comparisons pass:
   338,110 translations, 198,049 alternate attempts and 20,306 combinations.
   Current archive probes pass 2,211 full-C phoneme cases/27 voices, 945 traces
   and 25 deterministic WAV outputs; known Finnish full-C overflow cases remain
   separately documented. Native `rust/replacement.rs` now owns complete
   ideograph splitting, replacement segmentation and recursive
   `TranslateWordWithBounds` expansion. Initialized 323/161/200-byte scratch,
   bounded joins/skips, same-width casing and scoped rule context preserve live
   primitive effects without borrowing foreign buffers across callbacks.
   Eleven regressions, 400,000 unchanged-C comparisons and additional guards
   pass, including null-row symbol translation and Georgian UTF-8 boundaries.
   Current archive probes pass 2,931 full-C output cases/33 voices, 1,386 traces
   and 25 deterministic WAV cases. Three additional Arabic full-C braille
   overflow traps are audited separately; current native/probe matches on all
   three, and previously verified Finnish exception output remains unchanged.
   See the replacement stage in `RUST_PORT.md` for receipts and limits. Serialized
   scalar/classifier/main/output projections and short forwarding bridges
   still retain C owners. Native `rust/translate_clause.rs` now owns the
   complete clause controller: admission/reset, tokenization/source positions,
   script/emoji/capital boundaries, embedded commands, number/spelling dispatch,
   skips/abbreviation effects and final pause/phoneme/voice publication.
   Scoped clause/rule context, initialized row padding/predecessors and bounded
   source/number/row/command/list work preserve fresh primitive effects. The
   old controller and its word-length helper leave the production native build;
   test-only C oracles/probes retain them. The 200,000-episode oracle compares
   primitive order, initialized prefixes, meaningful row fields and complete
   retained fixture state; undefined C stack padding is outside its claim.
   Final serial gates pass 420 enabled/350 minimal Rust tests, strict/generated
   checks, four-target library checks and the native async suite (84 runnable
   CTests; physical audio skipped). Renewed probes pass 2,931 successful full-C
   CLI cases/33 voices, 1,386 native/probe traces and 25 offline WAV cases;
   six recorded full-C braille crashes remain excluded and separately checked
   against previous native output. Full configuration gates, fresh inventories
   and proof limits are recorded in the clause stage of `RUST_PORT.md`.
   Native `rust/character_conversion.rs` now owns complete ordered replacement
   matching/substitution, Hangul decomposition and compatibility-jamo conversion,
   Afrikaans/Dutch apostrophe handling and Irish uppercase policy. Bounded
   numeric source/dictionary projections preserve consumption, case/insert/flag
   effects and exact optional trace bytes without foreign loans or C replay.
   Seven Rust regressions and 200,000 unchanged-C sequences pass, including
   51,816 replacements, 12,771 consumed characters and 7,697 Irish uppercase
   matches. Dictionary/translator and pending-consumption owners remain C;
   427 enabled/357 minimal Rust tests and serial configuration/cross-target
   gates pass. Current archive substitution matches 3,141 successful full-C
   CLI cases/35 voices, 1,596 native/probe traces and 42 offline WAV cases.
   Added Afrikaans/Irish CLI voices run six focused character cases each;
   six recorded full-C braille traps remain excluded. Fresh inventory and
   validation scope are recorded in `RUST_PORT.md`.
   Native `rust/text_control.rs` now owns complete embedded parsing/application,
   word combination/syllable counting, phoneme-language switching and text
   initialization. Bounded scans and prefix scratch, scoped nested rule context
   and live flag publication preserve source/row/pronunciation/grammar effects,
   rejects/reversions, signed command values and reset/selection order.
   Eight Rust regressions and 1,200,000 unchanged-C episodes pass; final gates
   pass 435 enabled/365 minimal Rust tests and native configuration/cross-target
   checks. Current archive substitution matches 3,351 full-C CLI cases/35
   voices, 1,806 native/probe traces and 49 offline WAV cases. Added af/ga
   voices run 12 focused cases each; six recorded full-C braille traps remain
   excluded. Full proof scope and 6,776/7,246 inventories are in `RUST_PORT.md`.
   Native `rust/alternate_translator.rs` now owns complete alternate-language
   admission, cached-name comparison/publication, replacement, failed-load
   recovery, repeat reset and release order. Six regressions and 200,000
   unchanged-C episodes (2,400,000 operations) compare both slots, initialized
   names, translator/voice state and primitive order. Bounded copied names and
   allocation guards preserve completed effects without C replay.
   `rust/translator.rs` now owns the complete typed translator allocation,
   initialization, dictionary view/index publication and handle replacement/
   destruction. C retains alternate slots/cached names, global dictionary-cache
   admission and source/state projections; resident bytes remain Rust-owned.
   Serialized source, row, translator and global-state projections remain,
   alongside `lookupwchar` and `strchr_w`. Alternate language resource loading
   retains proactor-backed file reads. See the alternate lifecycle stage in
   `RUST_PORT.md` for current gates, inventory and output-comparison scope.
2. **`translateword.c`** (380; 59 C, 0 mixed, 9 bridges). Native
   `rust/translate_word.rs` owns complete `TranslateWord3` policy: key/textmode
   and dictionary admission, number/spelling, prefix/suffix retries, stress,
   grammar, bounded joins and restoration. Its short C admission bridge,
   checked numeric source windows, fresh state/output projections and primitive
   calls remain. `rust/word_admission.rs` owns complete dotted-abbreviation and
   unpronounceable-word policy with bounded scratch and scoped predecessor
   restoration; its primitive/source projections still retain serialized C
   owners. Retained-C helper comparisons pass 200,000 inputs for each policy,
   including 39,600 capacity refusals and 4,930 nested rule probes.
   `rust/word_helpers.rs` owns complete individual-letter spelling and plural
   selection. Checked source progress preserves zero-byte language switches
   before nonprogress refusal, fresh nested output, empty-input stress and
   scoped owned plural rule input. Another 200,000 comparisons for each helper
   pass, including 21,204 zero-byte switches and 133,333 rule calls. The native
   path separately verifies alias-safe switch copies; original C traps on that
   alias case. The permanent Russian spelling-switch regression passes native
   and retained-C pronunciation suites. The new owner/primitive projections
   add 36 C lines while replacing 34 C policy lines, increasing the raw count
   by two at that checkpoint. `rust/emoji_fallback.rs` now owns complete
   modifier/tag stripping, base lookup, tag-only retry, modifier-name joining
   and fresh trace/publication. Its former global replacement is an initialized
   translator-owned field; checked in-place writes preserve nested owner
   effects. The 200,000 retained-C episodes cover 672,376 list calls and 125,249
   replacements; native two-translator isolation and partial-failure guards
   pass separately. Complete word helper policies are native, with broader
   word/clause glue and retained source/state/output projections still open.
   The retained-C oracle passes 200,000 episodes including full prefix limits,
   multibyte prefixes, stacked suffixes and zero-character grammar transitions.
   Current archive probes pass 1,671 full-C phoneme comparisons across 27 voices
   and 621 traces, freshly rerun with the original emoji helper substituted
   into the actual current archive member. Three separately recorded Finnish
   braille cases overflow the full-C letter scratch; current native and
   retained-helper outputs match. Full-C crashes were not rerun in this slice.
   Source/row/translator/output ownership and word/clause glue remain open.
   `rust/translate_letter.rs` now owns complete isolated-letter policy: capitals,
   superscripts, non-ASCII digits, script names, secondary/Hangul fallback,
   unknown-character hexadecimal/braille and final joins. `TranslateLetter` is
   a nine-line admission bridge over the existing native UTF-8 decoder.
   `IsolatedOwner`/`IsolatedValue`, classification, named/letter lookup,
   secondary setup/table restoration, scoped Hangul rule/stress, fixed phoneme
   encoding and fresh output publication still project serialized C owners.
   Actual child extents and complete scratch joins are bounded; legacy scratch
   overflows reject without replay. Secondary setup uses existing proactor-backed
   file reads. The controller introduces no scheduler or polling loop; engine
   ownership and the broader word/clause glue still need migration.
   The two older Tamil phrase-boundary discrepancies in slightly smiling face
   and copyright-sign names are repaired by scoping replacement source context
   in `translate.c`. Correct archive-member probes reproduce them with published
   `f64db19d` frontend sources. The other nine expanded glyph/emoji differences
   were letter-controller regressions repaired by preserving rule-input
   pronunciation. All 864 final corpus cases match full C and original letter
   controllers; broader frontend migration remains open.
   Earlier claims attributing all eleven to the prior frontend were based on
   ineffective static-library relinking and are superseded; see the correction
   and pronunciation/context repair stages in `RUST_PORT.md` for evidence.
3. **`numbers.c`** (106; 27 C, 6 bridges).
   `rust/letter_host_compat.rs` now owns connected letter/accent host language/
   accent reads, fresh secondary selection and voice-table restoration, and
   owned rule-source scoping/restoration. Six primitive effects remain: raw
   lookup/rules/stress, platform classification, default-secondary setup and
   table selection. Missing secondary/voice and failure publication are bounded
   without replay. The generic seven-callback ABI stays supported. Actual
   previous/native host comparisons, malformed guards, complete serial gates and
   current CLI/trace/WAV comparisons pass. Main source/dictionary/state primitives (69 including
   original-source translation) and Roman primitives (23) remain. Native
   `rust/letter_lookup.rs` and its resources replace all three letter/symbol/
   diacritic controllers and tables. Actual caller extents reach accent fallback
   through `LookupBounded`/`LookupDictListBounded`; remaining raw C dictionary
   writes and word-level joins still need migration. `rust/number_roman.rs`
   owns Roman pronunciation, name placement,
   numeric/ordinal suffix assembly, dot/Hungarian form admission and previous
   dictionary reset. The adapter calls the native main controller with owned
   initialized numeric text, using only C dictionary/class/state projections;
   synthetic translation scopes/restores its rule/source context. Obsolete
   C recognition/Hungarian/dot wrappers and ordinal projections no longer compile
   in Rust-core builds. `rust/number_frontend.rs` owns the main number controller:
   grouping, leading zeros/time exceptions, ordinal suffixes, isolated/percent
   forms, all decimal modes and pause/skip-word completion. Each invocation owns
   its cache, count and form control, and calls native digit, thousand, ordinal
   and recognition primitives directly. The old C cache pointer/control and
   obsolete native child bridges no longer compile in Rust-core builds.
   Source owners admit complete initialized extents, including embedded NUL
   thousands separators; source/state projections avoid foreign loans across
   callbacks. The five-line bridge admits actual output capacity and two flags.
   Primary/alternate ordinal and missing-thousand state, remaining number host
   projections, source/word rows and phoneme slots still need migration. Typed
   translator/dictionary storage is Rust-owned, with C compatibility views.
   `rust/number_ordinal.rs`
   owns dot/uppercase/spacing/Roman policy and Hungarian month/range decisions.
   `rust/number_primitives.rs` owns superscripts, spelling, groups/variants,
   Hungarian suffixes and Roman recognition; `rust/number_lookup.rs` owns
   thousand-name dictionary ordering and fallback; `rust/number_digits.rs` owns
   cached/feminine/ordinal/vigesimal/year/hundred pronunciation, native nested
   thousand joins, vowel elision and stress. Initialized bounded scratch,
   unsigned phoneme projections and checked integer/power arithmetic exclude
   undefined legacy cases. These are bounded CPU operations; no separate
   proactor I/O role or scheduler is added.
4. **`dictionary.c`** (90; 20 C, 2 mixed, 21 bridges). `rust/translate_rules.rs` now owns
   complete rule-translation policy: group order/scoring, digits, fallback,
   accent/diaeresis retries, language switches, ending/deletion and source
   restoration. `rust/translate_rules_host_compat.rs` now owns connected source
   admission/scoping, checked byte/pointer access, translator/global/flag reads
   and writes, dictionary-group bounds, ending presence and output publication.
   The typed translator methods keep resource fields private. Eight C helpers
   are removed; the primitive ABI shrinks from thirteen callbacks to six.
   `TranslateRules` is a short forwarding bridge. `RulesLocale`, `RulesMatched`,
   `RulesSymbol`, `RulesLetter`, `RulesAppend` and `RulesTrace` still retain
   primitive effects and append admission. `rust/rule_conversion_compat.rs`
   now owns checked match cursor/deletion conversion and bounded pronunciation
   copying, preserving partial publication and executed effects on failure.
   `RulesMatched` only forwards the raw matcher; the six-slot native table
   now exposes its cursor/record primitive, while the generic thirteen-slot
   controller ABI remains unchanged. `rust/rule_match_host_compat.rs` owns native VM
   admission, current/main source scoping, typed context and letter configuration,
   bounded string-letter-group predicates and pointer/result publication.
   `espeak_rs_match_rule` is a forwarding bridge; `RustMatchPredicate` is removed.
   Prefix lookup and diagnostic stream writes remain C effects, with immutable
   dictionary/source/language configuration through VM execution. Bounded Rust
   flag/rule rendering owns mnemonic tables and fixed stack scratch; the native
   host carries its admitted rule extent and the C trace callback forwards to
   Rust before writing diagnostics. The generic
   VM ABI remains supported. Match pronunciation is copied
   into owned bounded storage before nested operations; controller-level foreign
   loans do not survive callbacks. The existing VM adapter and engine owners
   remain. `rust/lookup_host_compat.rs` now owns connected dictionary key
   preparation/transposition, bounded word snapshots, grammatical/main-translator
   context, bucket/next-window admission, attempted pronunciation/skip effects,
   selected flags and trace admission/snippets. The native dictionary lookup
   and transposer are forwarding bridges. Locale classification and diagnostic
   phoneme projection and diagnostic stream writes remain primitives; no foreign owner reference crosses
   them. `rust/list_host_compat.rs` now owns native list/symbol/prefix source
   admission, typed repeat/mode/replacement projections, skip/say-as globals,
   returned pointer checks and trace snippets. `rust/lookup_list.rs` owns the
   complete list policy; `LookupDictListBounded` is a forwarding bridge.
   Dotted abbreviation folding, repeat suppression/cache padding, accent
   fallback, stem retries and textmode/replacement policy no longer compile as C
   in native builds. Keys/pronunciation/repeat scratch are initialized and bounded;
   replacement bytes reside in the serialized typed Rust translator. Retained-C
   policy, callback/state/trace and corpus evidence are in `RUST_PORT.md`.
   `rust/lookup_symbol.rs` now owns symbol/name translation and ordinary prefix
   flag-query policies; `LookupBounded`/`LookupFlags` are forwarding bridges.
   The native Rust host scopes/restores child rule source and copies bounded
   pronunciation after translation returns. Caller output can be the global
   child buffer; no foreign output loan spans that effect. Four C primitives
   remain (`ListRawLookup`, `ListAccent`, `ListRawTranslate`, `ListRawTrace`).
   Generic list/symbol callback ABIs remain supported. Actual previous/current
   native-host episodes, malformed guards, serial configuration/cross-target gates
   and current CLI/trace/WAV comparisons pass. Full engine integration remains open.
   `rust/phoneme_encode.rs` now owns mnemonic matching, separator bars,
   language switch text and bad-phoneme/cursor policy. Its allocation-free
   plan admits the complete written footprint, preserving default-language
   tail bytes. Locale rules and selected records are snapshotted before
   publication; native compiler/runtime callers pass their actual capacities.
   `EncodePhonemesBounded` forwards to Rust; the extent-free compatibility
   wrapper still requires full caller storage. 251,021 actual retained-C
   comparisons and atomic short-buffer guards, four Rust regressions, complete
   serial gates, CLI/trace/WAV and compiler output comparisons pass. The byte
   classifier remains a C primitive. `rust/phoneme_render.rs` now owns complete
   mnemonic/IPA/stress writing and translated-clause assembly, with fresh list/
   selected-table state after interpretation effects and resolved tone priority.
   Bounded owned scratch and reusable clause storage replace 30/16-byte native
   buffers and C malloc/realloc growth. The interpreter/classifier remain C
   primitives; native writers/getter forward to Rust. Extent-free writer shims
   still require complete output storage. 161,020 actual-C writer and 20,000
   clause comparisons, six Rust regressions, malformed/long-name/reentry/partial
   node guards, all native lanes and expanded IPA/separator/tie CLI corpus pass.
   Defined initialized tail behavior after permissive UTF-8 is retained. Still C:
   `RemoveEnding` (mixed), remaining lookup/letter environments
   (`RustLetterConfig`, `RustPrefixFlags`, `RustMatchTrace`, `DictionaryTrace`),
   and `LoadDictionary`'s global name,
   path, cache-admission and diagnostic glue (mixed). Typed Rust translator
   storage now owns index/view rebinding and unique pinned snapshot handles;
   the C `InitDictionary` and `RustOffset` projections are removed. Rust data
   fixtures now use the native constructor, retaining C allocation only for
   the unchanged index oracle. Dictionary reads still use the engine proactor.
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

13. **`speech.c`** (88; 23 C, 0 mixed). Native `rust/engine_lifecycle.rs`
    owns initialization/output setup, cancel/synchronize/terminate sequencing,
    atomic mode/rate/error state and admitted audio-handle cleanup. Checked
    output sizing preserves defined C rounding and rejects overflow. Teardown
    destroys audio retained across a switch to retrieval mode, and relinquishes
    audio/translator/decoder slots before destruction callbacks. Initialization
    primitives, parameter arrays and most engine resources still have C owners;
    lifecycle admission is serialized. The optional pthread-only backend keeps
    its existing C synchronization loop; no Rust polling fallback is added.
    `rust/engine_request.rs` also owns text/mark preparation and public request
    admission: synchronous/queued selection, identifier publication, atomic
    text/notification pairs and cleanup after capture/admission failure. Its
    adapter calls the native owned command factory directly; queue and audio
    waits keep the proactor path. State and parameter/skip arrays still belong
    to the serialized hybrid engine. `rust/engine_driver.rs` now owns proactor-path
    synthesis startup and buffer/clause passes: readiness, flags, voice/decoder
    setup, delivery/status control and clause boundaries. Its adapter checks
    cursor/counter arithmetic and does not replay a refused runner. Three C
    primitives project encoding, configured default voice and current generation;
    the proactor-off build retains the original C driver/loop. Remaining control
    includes path lookup (`espeak_ng_InitializePath`, `check_data_path`),
    synchronous key/character and punctuation preparation, callback registration
    and phoneme-event setup. `rust/engine_audio.rs` owns the proactor-path audio
    dispatch, rate/open/write control, bounded event traversal and event-delay
    arithmetic, over the existing device and event-admission completion waits.
    The C error-formatting primitive and proactor-off dispatcher remain. Parameter
    lookup/application, `espeak_TextToPhonemes*`, `espeak_SetPhonemeTrace` and
    `espeak_Info` also remain. Native synthesis runs on the proactor and audio/event waits
    use completions. Native passes still report locally-ready/done, with pending
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

19. **Data compilers** (323 lines, plus `spect.c` two). `compiledata.c`
    (207; 43 C, 4 mixed, 15 bridges: source-open/context, primitive
    lifecycle/row/token projections, diagnostics, resource reload and intonation
    API/effects). Public path/lifecycle and manifest policy now belong to Rust.
    `rust/compiler_source.rs` owns source snapshots, EOF/line accounting,
    token scanning, escapes/comments, bounded numbers, replay and fgets chunks.
    Phoneme/include/intonation contents use the existing engine proactor with
    thirteen retained slots, 16 MiB per-file and 64 MiB retained-capacity bounds.
    Intonation's two passes share a snapshot, with Windows text conversion;
    failed reads make the slot unavailable. Locale maps are copied once per
    session, and token scanning has fixed scratch and no per-byte callback.
    Frozen C tests pass 278,481 token/check/replay/chunk comparisons plus guards
    and selected C ASan; fifteen complete C/native compile scenarios match
    status, diagnostics and payloads, two native-only overflow cases reject,
    and all five regenerated master payloads match. Native
    `rust/intonation_compile.rs` now owns the complete two-pass intonation
    compiler, keyword/envelope/default tables, name discovery, all statements,
    diagnostics and explicit 68-byte records. Fixed arenas bound it to 100 tunes;
    copied paths/classifiers precede all effects. Source selection and final
    output use the existing engine proactor and retained buffers; completion
    precedes summary/reload. Five safe tests and 515 actual full-C driver
    comparisons pass, including output/status/log closure/reload/file-error
    paths, duplicate/missing names, cross-tune flags, reentry and the tune cap.
    Selected C-fixture ASan passes; fifteen renewed complete CLI compiler
    comparisons and all five master payloads match. C retains default-path,
    diagnostic FILE/error-context and resource-reload projections.
    See the complete intonation stage in `RUST_PORT.md` for gates and limits.
    Native `rust/spect_compile.rs` owns the complete `LoadSpect` controller:
    owned source loading, keyframe/center/length policy, live-generator RMS,
    explicit ordinary/Klatt record packing, address/count progress and bounded
    output publication. A retained 1,604-byte payload admits 25 keyframes;
    generator borrowing follows proactor reads and ends before callbacks.
    Four safe regressions and a frozen complete C controller compare fields,
    payloads, status, address/count and diagnostics across all 927 spectrum
    files with both controls. Native-only bounds and injected write-failure
    probes cover unsafe old-C inputs. Native `rust/compiler_resource.rs` now
    also owns complete resource dispatch, `.wav` fallback, WAV and text/binary
    envelope encoding, hash/reference/duplicate storage, payload and manifest
    publication. Four registered owners retain bounded source/payload/name/
    record arenas; all actual sources use the engine proactor, and ordinary
    spectral compilation reuses its already-read source. Four safe tests and
    3,976 complete frozen-C comparisons over 11,935 native requests cover all
    927 spectra, 203 WAVs and 29 text envelopes with both controls, repeated
    addresses/cache hits, generated/truncated sources and diagnostics. Native
    guards cover source/name/record/counter limits, reentry/stale/exhausted
    owners, data/manifest/position failures and caller snapshots. Binary PITC
    envelopes correct the old fread item-count/no-write bug with a separate
    native regression; that change is outside C equality. Native
    `rust/compiler_output.rs` now owns the full output set: creation order,
    manifest header/fallback, optional program log, initial binary words,
    retained bounded staging and complete cleanup. All five owned files use
    the engine proactor, with five retained 256 KiB buffers and one write
    flight. Thread-confined compatibility owners snapshot descriptor/path/byte
    loans before effects and reject reentry/stale IDs. Four safe regressions
    and 80 complete frozen-C output-set comparisons cover exact bytes,
    positions, partial failures, fallback and optional log; native guards
    cover mutated caller loans, write failure, reentry and exhausted buffers.
    Native never closes borrowed stderr and propagates the first output error
    before resource reload; both correct inherited C failure behavior and
    are outside successful-output equality. See `RUST_PORT.md` for gates.
    `rust/compiler_tables.rs` now owns complete reserve/lookup/find/import,
    inheritance, table close/undeclared diagnostics and explicit final packing.
    Four retained owners bound each session to 149 tables of 256 rows; separate
    quiescent C views retain a temporary lease for compiler primitive
    projections. Current rows snapshot before effects and publish after;
    older returned rows are read-only. Packed output is one bounded publication
    through the owned proactor file set. Four safe regressions and 1,201 complete
    frozen-C episodes pass 222,883 state/diagnostic/record comparisons, exact
    packed bytes and native lifetime/cap/failure guards. Old unsafe 200-byte
    reference scratch and n+1 full-table terminator writes have native bounds
    regressions, outside defined-C equality. Twenty-one CLI scenarios, five
    master payloads and complete program log match C; two native overflow
    probes reject. The scanner includes short C wrappers through `TableCall`;
    its counts do not imply that their old policies survive. Complete keyword/property, source-file, public lifecycle and manifest
    policy are also now Rust. C context/primitive diagnostic/reload projections
    remain. See the table stage in `RUST_PORT.md`.
    `rust/compiler_program.rs` now owns the bounded bytecode word buffer,
    indexed branch stack, IF/NOT/AND/OR/stress/property conditions, THEN folding,
    ELSE/ELIF chains and jump patching, vowel transition packing, sound/tone
    instructions and final little-endian program/log publication. Four retained
    owners hold 2,020 words/4,040 payload bytes each, preserving C's initialized
    slack and separate IF limit; no raw C buffer or branch
    pointer survives in the native context. Existing resource/proactor reads
    and owned output/proactor writes serve its source-driven controllers.
    Five safe regressions, 2,000 complete frozen-C episodes and the separate
    IF-limit boundary compare 84,135
    word/state/diagnostic/literal primitive traces, including nested chains,
    returning branches and widened procedure positions. Native guards cover
    malformed tokens, source failure, full/stale/reentered owners, mutated
    caller loans, overlapping output, word/IF caps and failed publication
    without replay. Safe tests separately cover invalid stress shifts and
    cleared-program stale branch offsets. Twenty-nine complete CLI scenarios,
    all five master payloads and full program log match C; five native-only
    overflow/EOF cases refuse safely. The complete outer
    `CompilePhoneme` property/feature/instruction dispatcher, CALL inheritance,
    procedure precedence/registry and IPA assembly now belong to
    `rust/compiler_definition.rs`, sharing those same four assembler leases.
    Each lease retains 50 full scanner-width names and full i32 addresses;
    empty procedures consume no slot. Fixed IPA scratch preserves permissive
    UTF-8, partial hex, join flags and embedded-NUL truncation, with native
    guards for unsafe shifts/expanded output and invalid length modifiers.
    Four safe regressions and 1,000 complete frozen-C episodes plus registry/
    edge cases pass 40,641 comparisons; 55 complete CLI compiler scenarios,
    five master payloads and the full program log match C, and seven native
    bounds cases refuse. Complete `NextItem` numeric/error/keyword/property policy now belongs to
    `rust/compiler_items.rs`, with all 122 ordered table entries and their
    duplicate first-match behavior in `compiler_items_tables.rs`. Its adapter
    snapshots descriptor/previous token, ends the scanner borrow before
    lookup/diagnostic effects and publishes the full initialized token tail
    afterward. `rust/compiler_files.rs` owns complete source-file dispatch,
    include push/pop and twelve-frame storage, copied/truncated paths, filename
    tails/line restoration, missing-file continuation, table/definition control
    and final termination. The C stack and keyword/property tables leave the
    native build; existing source snapshots/proactor reads are reused.
    Six safe regressions, 278,601 complete C scanner/table comparisons and
    2,000 complete C file/include episodes (12,000 comparisons) pass, with
    reentry, snapshots, overlap, effect-error and admission guards. The actual
    production C token bridge is extracted into the file fixture: refused
    reads preserve the first status and stop without advancing or retrying.
    Sixty-six independent full-C CLI compiler cases, all master files/log and
    renewed selected C-object ASan pass. See the source-file/token stage in `RUST_PORT.md`.
    `rust/compiler_driver.rs` now owns the complete public phoneme compiler:
    bounded optional/default paths, setup and owner order, source/output errors,
    compiler/table execution, reference and error summaries, source/output
    completion, first output/table/program status, log closure, resource reload,
    manifest status precedence and exactly-once admitted-context cleanup.
    Explicit overlong paths reject before effects; default concatenations retain
    platform-width C clipping. All paths/maps/descriptor bytes snapshot before
    callbacks, and reentry refuses without waiting. C retains small allocator,
    row/token, diagnostic FILE/error-context, global-resource and locale/libc
    projections. `rust/compiler_manifest.rs` replaces both C stdio passes and
    per-name allocations with one bounded engine-proactor snapshot and retained
    120-byte-name rows: 16 MiB input and 131,072 chunks. Both discovery and parsing
    use fgets-width chunks, embedded NUL truncation, copied classifiers and
    Windows CRLF/CTRL-Z rules. A bounded host scanf probe preserves incomplete
    hex-prefix consumption without assuming that all libc implementations match.
    Unsafe long names and hex overflow reject without partial publication.
    Five safe regressions include both host-prefix policies, retained arenas and
    100 alternating file reopens. A complete frozen C public driver and manifest
    reader from `b8f52c7f`, paired with actual extracted production primitives,
    pass 2,000 driver and 10,000 manifest episodes / 206,546 comparisons. Early
    allocation/owner/voice/source/output failure, manifest empty/missing, exact
    logs, cleanup, status precedence, snapshots/reentry/bounds/overlap are covered.
    Sixty-six exact independent-C CLI cases plus seven guards, all five master
    payloads/full log and selected C-fixture-object ASan pass. Full serial gates
    and limits are recorded in the public-driver/manifest stage of `RUST_PORT.md`.
    Context/source-open, diagnostic/reload/global owners and primitive projections,
    broader engine/MBROLA/ABI/platform and physical target/audio/thermal evidence
    still need migration or validation.
    `compiledict.c` (116; 16 C,
    one mixed, six bridges: primitive list/rule projections,
    diagnostics and reload effects). Complete list/individual-rule/
    rule-file parsing, list precedence and buckets, letter definitions, group
    sorting/packing and bytecode assembly run in
    `rust/{dict_list,rule_compile,rule_file}.rs`. List and rule source contents
    use the engine proactor with pooled input/completion storage; record and
    node arenas are reused without per-entry allocation/free. Rust now owns
    top-level rule-path admission, source/output lifetimes, header patching and
    final writes through the same engine proactor, with retained 256 KiB
    staging/completion buffers and one write in flight. C retains diagnostic
    FILE ownership and native translator/classifier/reload projections.
    Retained-C comparisons cover list records/state/diagnostics/primitive calls,
    complete list files, every keyword, duplicate and extension precedence,
    permission fallback, compressed keys, initialized tails and Turkish widening;
    prior 24,350 individual-rule and 1,296 full rule-file comparisons remain.
    All 123 generated dictionaries match full C; bounded failure/reentry and
    storage-reuse regressions pass. Complete-driver comparisons check output,
    status, diagnostics, file-error contexts and filename selection; 64 output
    reopen cycles check chunking/header patching and storage reuse. Complete flag
    and rule trace rendering now runs in bounded Rust with explicit native rule
    extents. Ordered names, flag budgets, target-char conversion and defined
    legacy truncation/padding match actual C; metadata that underflowed C scratch
    has direct safe regression coverage. Native diagnostic FILE owners remain.
    Other compiler source/output I/O and native projections still need migration.
    `compilembrola.c` (86, MBROLA builds). `spect.c` (two C lines, five bridges)
    retains only API/stderr projections. `rust/spect_file.rs` owns all three
    source formats, IEEE80/short fields, raw name bytes, default/formant/peak/
    Klatt fields, bins, normalization/scales/duration/length adjustment,
    interpolation and RMS using the live Rust wave generator. Contiguous retained
    arenas replace per-frame allocation; source reads use the engine proactor
    with two retained input buffers and one read flight. C views occupy four
    stable leased owners, with bounded registry admission and callback reentry
    rejection. Five safe tests, 1,925 C-reader comparisons across generated and
    truncated sources plus all 927 current corpus files, 10,658 RMS and 100,000
    interpolation comparisons pass. The frozen C fixture initializes unused
    harmonic record fields before calling Rust; selected C-object ASan passes.
    Complete regenerated master compiler payloads remain byte-identical to full
    C. Float/count/cursor/admission overflow has native-only safe checks; the
    old C constructor's uninitialized unused fields/padding are not equality
    evidence. Full phoneme compiler control/output and foreign engine/ABI
    lifecycle remain open; see the spectral stage in `RUST_PORT.md` for gates.
    **Proactor: their file reads and writes**
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
| Data compilers and CLI file I/O | Complete dictionary list/rule parsing, precedence/buckets, rule grouping/bytecode and top-level driver are Rust; dictionary source contents and final writes use pooled proactor I/O. Phoneme/include/intonation source snapshots and scanning are Rust with pooled proactor reads and token replay. Path open/close remain OS primitives. Complete intonation, spectrum loading/packing, phoneme resource encoding/cache and all five phoneme output files are now Rust with proactor contents I/O. Table construction/lookup/import/packing is also Rust, with retained row arenas and quiescent C views. Bytecode buffers, condition/branch patching, vowel transitions and sound/tone controllers are now Rust with retained arenas; program/log publication uses the owned proactor writer. Complete phoneme property/feature/instruction dispatch, CALL inheritance, IPA assembly and the bounded procedure registry are now Rust on the same assembler leases. Complete keyword/property token policy and source-file/include control are also Rust, with fixed frame/path storage and refusal propagation. Complete public phoneme lifecycle/path policy and bounded manifest loading/parsing are also Rust, with retained rows/input and proactor contents I/O. C source-open/context, diagnostic/locale/libc/global-reload and primitive projections, MBROLA compiler I/O and CLI writes remain (items 19 and 20). |
| `<audio>` URI callback, voice catalogue directory listing | The callback is the caller's; directory listing has no proactor operation. Their file contents are read through the proactor. |
| Text front end, synthesis math, Klatt | Not applicable: CPU only, no I/O. |

## Appendix: the generated table

`python3 tools/c_inventory.py build-rust-core --markdown`, library and CLI sources in
the measured configuration:

| File | C logic (lines) | Mixed: C around Rust calls (lines) | Bridges to Rust |
| --- | --- | --- | --- |
| `src/espeak-ng.c` | `DisplayVoices` 49, `Write4Bytes` 6, `OpenWavFile` 25, `CloseWavFile` 12, `SynthCallback` 29, `PrintVersion` 6, `main` 429 | - | 0 |
| `src/libespeak-ng/common.c` | `GetFileLength` 7, `utf8_in` 2, `CommonAlpha` 1, `CommonDigit` 1, `CommonSpace` 1, `espeak_ng_SetRandSeed` 3 | - | 19 |
| `src/libespeak-ng/compiledata.c` | `error` 8, `error_from_status` 7, `TableBase` 6, `TableError` 15, `LookupPhoneme` 3, `ItemLookup` 1, `ItemError` 1, `OutputFallback` 4, `ResourceLog` 3, `ResourceError` 12, `ProgramNext` 6, `ProgramUnget` 1, `ProgramError` 1, `ProgramLookup` 1, `ProgramLoad` 1, `FindPhoneme` 2, `DefinitionLookup` 1, `DefinitionRow` 1, `DefinitionProject` 1, `DefinitionFind` 1, `DefinitionUnknown` 1, `WritePhonemeTables` 1, `EndPhonemeTable` 1, `StartPhonemeTable` 1, `FilesReset` 1, `FilesFrame` 1, `FilesProject` 1, `FilesOpen` 1, `FilesClose` 1, `FilesTables` 1, `FilesEnd` 1, `FilesStart` 1, `FilesCompile` 1, `FilesTerminate` 1, `espeak_ng_CompilePhonemeData` 2, `PhonemeCounts` 3, `PhonemeLog` 2, `PhonemeError` 2, `IntonationLog` 3, `IntonationClose` 3, `IntonationReload` 3, `IntonationError` 3, `espeak_ng_CompileIntonation` 2 | `SourceOpen` 18, `LoadDataFile` 16, `PhonemeEffect` 44, `espeak_ng_CompileIntonationPath` 16 | 15 |
| `src/libespeak-ng/compiledict.c` | `list_encode` 3, `list_translate` 4, `list_case` 5, `list_transpose` 3, `list_report` 19, `list_descriptor` 6, `rule_encode` 3, `rule_digit` 2, `rule_report` 21, `rule_file_report` 17, `dictionary_lists` 2, `dictionary_rules` 2, `dictionary_log` 5, `dictionary_reload` 3, `dictionary_errors` 2, `dictionary_error` 2 | `compile_dictrules_native` 17 | 6 |
| `src/libespeak-ng/dictionary.c` | `PhonemeEncodeLocale` 2, `EncodePhonemes` 2, `PhonemeTextAlpha` 2, `PhonemeRenderIpa` 5, `PhonemeRenderAlpha` 1, `PhonemeRenderEnvironment` 3, `WritePhMnemonic` 2, `WritePhMnemonicWithStress` 2, `RustLetterConfig` 3, `IsVowel` 2, `RustPrefixFlags` 4, `RulesLocale` 2, `RulesSymbol` 2, `RulesLetter` 2, `RulesAppend` 4, `RulesTrace` 5, `ListAccent` 2, `ListRawTranslate` 2, `ListRawTrace` 3, `ListGlobals` 2 | `LoadDictionary` 19, `RemoveEnding` 19 | 21 |
| `src/libespeak-ng/error.c` | `StatusErrno` 2, `StatusLock` 2, `StatusUnlock` 2, `StatusWrite` 2 | - | 5 |
| `src/libespeak-ng/espeak_api.c` | - | - | 14 |
| `src/libespeak-ng/intonation.c` | - | `CalcPitches` 42 | 0 |
| `src/libespeak-ng/klatt.c` | `KlattRandom` 1, `KlattSpeechPlayerReset` 2, `KlattFini` 2 | - | 4 |
| `src/libespeak-ng/langopts.c` | `RustLanguageEnvironment` 12, `LoadConfig` 2, `CheckTranslator` 5 | - | 1 |
| `src/libespeak-ng/numbers.c` | `LetterRawLookup` 2, `LetterSpace` 2, `LetterRawRules` 2, `LetterSetup` 2, `LetterRestore` 2, `LetterStress` 2, `LetterGlobals` 2, `NumberOrdinalTranslate` 3, `NumberFrontendByte` 3, `NumberFrontendWrite` 3, `NumberFrontendValue` 14, `NumberFrontendWord` 3, `NumberFrontendLookup` 2, `NumberFrontendList` 5, `NumberFrontendText` 4, `NumberFrontendStoreText` 5, `NumberFrontendClassify` 8, `NumberFrontendMissing` 3, `NumberFrontendSkip` 3, `NumberFrontendType` 3, `NumberFrontendLength` 10, `NumberRomanRange` 3, `NumberRomanSuffix` 2, `NumberRomanWord` 2, `NumberRomanClear` 3, `NumberRomanList` 3, `NumberRomanTranslate` 10 | - | 6 |
| `src/libespeak-ng/phonemelist.c` | `ListSelect` 6, `ListInvalidInstruction` 3 | `MakePhonemeList` 66 | 0 |
| `src/libespeak-ng/readclause.c` | `UngetC` 2, `CharacterQuery` 33, `PunctuationName` 8, `PunctuationUnreadSecond` 2, `SetVoiceStack` 18, `ClauseSnapshot` 13, `ClausePublish` 11, `ClauseSourceEof` 1, `ClauseSourceRead` 1, `ClauseSourcePeek` 1, `ClauseClassify` 8, `ClauseReplace` 2, `ClauseEffect` 17, `InitText2` 20, `ClausePhonemeAlpha` 2 | - | 13 |
| `src/libespeak-ng/sPlayer.c` | `MIN` 1, `needsMixWaveFile` 2, `mixWaveFile` 21, `fillSpeechPlayerFrame` 29, `KlattInitSP` 2, `KlattFiniSP` 4, `KlattResetSP` 3 | `Wavegen_KlattSP` 38 | 1 |
| `src/libespeak-ng/setlengths.c` | `SetParameter` 49, `DoEmbedded2` 9, `LengthEmbedded` 7, `LengthToneEnvelope` 10, `SetLengthMods` 5 | `CalcLengths` 63 | 1 |
| `src/libespeak-ng/soundicon.c` | - | - | 5 |
| `src/libespeak-ng/spect.c` | `SpectLog` 2 | - | 5 |
| `src/libespeak-ng/speech.c` | `check_data_path` 9, `espeak_ng_InitializePath` 8, `RustEngineNoop` 1, `RustEngineCurrentVoiceClear` 1, `RustEngineStackReset` 1, `RustEngineVoiceReset` 1, `RustEngineClock` 1, `espeak_ng_SetPhonemeEvents` 9, `espeak_ng_GetSampleRate` 2, `RustDriverEncoding` 1, `RustDriverDefaultVoice` 1, `RustDriverGenerate` 1, `RustRequestDiagnostic` 2, `sync_espeak_Key` 9, `sync_espeak_Char` 6, `sync_espeak_SetPunctuationList` 8, `espeak_SetSynthCallback` 2, `espeak_GetParameter` 4, `espeak_SetPhonemeTrace` 5, `espeak_TextToPhonemesWithTerminator` 8, `espeak_TextToPhonemes` 2, `espeak_IsPlaying` 2, `espeak_Info` 4 | - | 23 |
| `src/libespeak-ng/ssml.c` | `SsmlWideSpace` 1, `SsmlByteSpace` 1, `SsmlByteLower` 1, `SsmlDecimalPoint` 6, `SsmlResolveVoiceName` 8, `SsmlSelectVoice` 15, `SsmlUpdateRate` 4, `espeak_SetUriCallback` 2 | `ProcessSsmlTag` 20 | 1 |
| `src/libespeak-ng/synthdata.c` | `RustSpectrumTransition` 6, `SelectPhonemeTableName` 6, `InvalidInstn` 3, `RustPhonemeStorage` 34, `RustPhonemeDataLength` 2, `RustPhonemePrograms` 3, `RustInvalidInstruction` 2, `InterpretPhoneme2WithData` 16, `InterpretPhoneme2` 2, `TonePhoneme` 4 | `ReadPhFile` 19, `LoadPhData` 44, `FreePhData` 17, `SelectPhonemeTable` 16, `InterpretPhonemeWithLength` 19 | 5 |
| `src/libespeak-ng/synthesize.c` | `WordToString` 6, `SynthesizeInit` 5, `FormantTransition2` 2, `GenerateEnvelope` 6, `SpeakNextClause` 37, `espeak_SetPhonemeCallback` 2 | `CommandSettings` 21, `CommandEffect` 66, `FormantTransitionWithCapacity` 17, `GenerateEffect` 88, `Generate` 46 | 15 |
| `src/libespeak-ng/tr_languages.c` | - | - | 3 |
| `src/libespeak-ng/translate.c` | `AlternateSlot` 4, `AlternateCache` 4, `AlternatePresent` 2, `AlternateCached` 2, `AlternateWrite` 2, `AlternateLookup` 2, `AlternateCreate` 3, `AlternateLoad` 3, `AlternateMainTable` 2, `AlternateEffect` 10, `AlternateCallbacks` 3, `SetTranslator2` 1, `SetTranslator3` 1, `lookupwchar` 7, `strchr_w` 4, `ReplacementValue` 4, `ReplacementStore` 4, `ReplacementClass` 5, `ReplacementLower` 2, `ReplacementMain` 16, `ReplacementSnapshot` 2, `ReplacementPublish` 3, `ReplacementCallbacks` 3, `TranslateWord` 2, `ControlValue` 9, `ControlStore` 9, `ControlByte` 6, `ControlWrite` 5, `ControlCommand` 1, `ControlPutCommand` 1, `ControlDefault` 1, `ControlAlpha` 1, `ControlLower` 1, `ControlRow` 1, `ControlPutRow` 1, `ControlTranslate` 7, `ControlVowel` 1, `ControlLookup` 1, `ControlSelect` 1, `ControlInit` 1, `ControlCallbacks` 3, `ControlWordBounds` 5, `Word2Value` 12, `Word2Store` 12, `Word2Byte` 4, `Word2Write` 4, `Word2Entry` 2, `Word2Put` 2, `Word2Command` 2, `Word2PutCommand` 2, `Word2Embedded` 1, `Word2Encode` 5, `Word2Translate` 10, `Word2Combine` 4, `Word2Dictionary` 3, `Word2RestoreDictionary` 3, `Word2Alternate` 2, `Word2Select` 1, `Word2Phoneme` 3, `Word2Alpha` 1, `Word2Invalid` 1, `TranslateWord2WithContext` 9, `CharacterLength` 5, `CharacterValue` 3, `CharacterIgnore` 1, `CharacterByte` 4, `CharacterWrite` 1, `CharacterClass` 3, `CharacterLower` 1, `CharacterTrace` 6, `CharacterCallbacks` 2, `ClauseValue` 48, `ClauseStore` 36, `ClauseByte` 1, `ClauseWrite` 1, `ClauseClass` 9, `ClauseLower` 1, `ClauseRead` 7, `ClauseEmbeddedCommand` 3, `ClauseChar` 3, `ClauseUpper` 3, `ClauseEntry` 1, `ClausePut` 1, `ClauseCommand` 1, `ClausePutCommand` 1, `ClauseTranslate` 8, `ClauseWordEmbedded` 1, `ClauseEof` 1, `ClauseMakeList` 1, `ClauseOutput` 4, `ClauseResult` 7, `TranslateClause` 2 | - | 13 |
| `src/libespeak-ng/translateword.c` | `WordPointer` 15, `WordSource` 18, `WordByte` 3, `WordWrite` 5, `WordWindow` 14, `WordValue` 26, `WordStore` 9, `WordLocale` 3, `WordList` 6, `WordEmoji` 7, `WordText` 16, `WordDotted` 5, `WordNumberLanguage` 4, `WordNumber` 7, `WordSpell` 7, `WordLetter` 8, `WordUnpronounceable` 6, `WordSpellingStress` 4, `WordRules` 6, `WordRemove` 6, `WordPrefix` 6, `WordTraceSuffix` 5, `WordAppend` 4, `WordPlural` 4, `WordStress` 4, `WordSnapshot` 6, `WordPublish` 7, `WordChangeStress` 4, `WordSpecial` 4, `EmojiByte` 4, `EmojiList` 18, `EmojiWrite` 5, `EmojiTracing` 3, `EmojiSnapshot` 4, `EmojiTrace` 3, `EmojiFinish` 4, `HelpersByte` 3, `HelpersLetter` 8, `HelpersLeading` 3, `HelpersCopySwitch` 6, `HelpersStress` 4, `HelpersPlural` 9, `HelpersCallbacks` 3, `IsolatedOwner` 2, `IsolatedValue` 16, `IsolatedClassify` 3, `IsolatedNamed` 4, `IsolatedLetter` 4, `IsolatedSecondary` 2, `IsolatedRestore` 2, `IsolatedHangul` 11, `IsolatedEncode` 2, `IsolatedPublish` 11, `AdmissionValue` 9, `AdmissionAlpha` 3, `AdmissionVowel` 3, `AdmissionProbe` 6, `AdmissionSkip` 3, `AdmissionCallbacks` 3 | - | 9 |
| `src/libespeak-ng/voices.c` | `RustVoiceLength` 2, `RustCatalogDiagnostic` 5, `RustVoiceWorkspace` 3, `RustVoiceDirectory` 5, `RustOrdinalEnvironment` 4, `RustActiveVoiceHost` 33, `LoadVoiceVariant` 11, `espeak_ng_SetVoiceByFile` 22, `espeak_ng_SetVoiceByName` 35, `espeak_ng_SetVoiceByProperties` 10, `espeak_GetCurrentVoice` 2 | `LoadVoice` 39 | 10 |
| `src/libespeak-ng/wavegen.c` | `WavegenFini` 2, `Write4Bytes` 6, `WavegenFill` 4, `espeak_ng_SetOutputHooks` 3 | `WavegenEffect` 38 | 14 |
| `src/speechPlayer/src/frame.cpp` | `create` 1 | - | 0 |
| `src/speechPlayer/src/speechPlayer.cpp` | `speechPlayer_initialize` 7, `speechPlayer_queueFrame` 4, `speechPlayer_synthesize` 2, `speechPlayer_getLastIndex` 3, `speechPlayer_terminate` 5 | - | 0 |
| `src/speechPlayer/src/speechWaveGenerator.cpp` | `create` 1 | - | 0 |
