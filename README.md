# eSpeak NG Text-to-Speech

This fork is being ported to Rust. Native text/Unicode, contextual dictionary
lookup, alphabet compression and compiled phoneme-table code replace C routines
with `-DUSE_RUST_CORE=ON`. The compiled rule matcher, scalar language letter
predicates, phoneme-program VM, phoneme condition/stress evaluation and spectrum
selection, formant transitions, spectrum smoothing and acoustic voice
configuration, mutable language options, fixed translator presets and voice
metadata/matching, candidate selection, ordered active-voice setup, phoneme
replacement and MBROLA directive parsing also run in Rust.
Voice catalogue discovery, metadata storage, result arrays and selection
workspace have native Rust owners. Active voice request paths, fallback controls
and current identifiers also run in Rust. Ordered voice directive dispatch is
native, with explicit translator/table/backend actions. Active voice files stream
through reusable Rust buffers, with native ordered load/finalization and owned
current voice metadata. Translation, the wavegen queue and waveform
generation still use C.
Core phoneme asset reads and aligned reusable storage are owned by Rust.
Dictionary files use a bounded Rust snapshot cache with shared immutable bytes,
cached indices and reusable fresh-read storage.
Speech-rate calibration and ordered Sonic speed effects are computed in Rust.
Global configuration parsing and sound-icon WAV/name storage have Rust owners.
Suffix removal, spelling repairs and permissive UTF-8 character encoding and
forward/backward decoding run in Rust. Clause input replay/count handling,
punctuation classes and character preprocessing use native routines.
Punctuation announcement also runs in Rust, preserving repeated-name speed
commands, source pushback and pause selection around explicit host callbacks.
Internal phoneme mnemonic decoding and clause phoneme wrappers run in Rust,
with checked output and retained legacy stress/language-switch formatting.
Character/special-name lookup is native, with ordered copied dictionary/rules
and default-voice backend effects and phoneme-table restoration.
The main clause controller runs in Rust with owned replay/state snapshots,
bounded output/index writes and copied SSML/name/punctuation backend effects.
Engine character extensions, emoji/token predicates, bracket lookup, digit/space
rules, packed byte words, null scans and Turkish-aware lowercase use Rust.
Vowel-stress extraction and language-specific word-stress assignment run in Rust.
Clause intonation (tone-group pitch contours, emphasis and tone-language
sandhi) runs in Rust over a copied phoneme-list snapshot.
Phoneme lengths, pre-pauses, amplitudes and pre-vocalic pitch also run in
Rust, with engine callbacks for embedded speed commands and tone envelopes;
the pitch envelope tables are Rust data.
Clause phoneme lists are built in Rust: stress promotion, table-switch cleanup,
regressive voicing, voice replacements and phoneme programs run natively on a
Rust-owned working list.
The clause synthesis driver (`Generate`) decides each phoneme's queue commands
in Rust and suspends/resumes on queue space; the command queue and frames
remain C behind ordered host effects.
The synthesis command writers (pauses, pitch/amplitude envelopes, samples and
spectrum sequences) run in Rust and own their shared state; the wavegen queue,
frame pool, spectrum lookup and smoothing remain host operations.
The formant wave generator and queue consumer (`wavegen.c`) run in Rust and own
the generator state; the output buffer stays shared C memory, and Klatt, MBROLA,
sonic, markers and output hooks are host operations. The synthesis command queue
and the echo ring belong to Rust, and every queue writer (command writers,
markers, alignment, voice changes, embedded commands, MBROLA) runs in Rust;
smoothing and Klatt's look-ahead still read the queue in place.
Stress changes, phoneme appends and alternate pronunciation transforms use
bounded native Rust planning and explicit counter effects.
MBROLA mapping tables have bounded Rust owners and contextual name selection.
Shared pitch, formant and amplitude calibration, MBROLA pitch text and PCM
scaling run in Rust with checked arithmetic and bounded output.
SSML attribute scans, mnemonic/integer values, UTF-8 copies, character
references and key-name replacement use bounded Rust routines.
Nested SSML parameter stacks and embedded command effects are planned in Rust
before complete output-capacity admission.
SSML voice-stack properties and base-variant identifiers are composed in Rust,
with ordered name-resolution callbacks and copied identifier snapshots.
SSML decimal/hexadecimal prosody values, percentages, semitones and parameter
updates are parsed and computed in Rust with fixed storage and checked effects.
SSML voice frames and identifier changes use native plans with bounded strings,
explicit local counts and preserved caller tails.
SSML tag decoding and style, prosody and emphasis directives run in Rust,
with generated tag aliases and parameter plans admitted before frame mutation.
Phoneme wrappers, say-as commands/key closing, substitutions and ignore-text
directives use native output plans that preserve compatibility byte tails.
SSML break timing and clause/voice transitions use native plans, with ordered
host rate and voice selection effects applied outside Rust owner borrows.
Marker, URI and compatibility wide names use a bounded Rust owner with warm
utterance resets and explicit release after shutdown drains events/workers.
SSML marker/audio requests, resource paths and embedded output effects are
planned in Rust; copied URI text stays live across name-arena growth callbacks.
The compatibility dispatcher uses an owned native SSML controller with explicit
host effects and reusable output, verified against combined retained-C cases.
See [Rust port status and build instructions](docs/RUST_PORT.md), including
native resident asset loading through loadngo's proactor and optional NPU
capability integration.

- [Features](#features)
- [Supported languages](docs/languages.md)
- [Documentation](#documentation)
- [eSpeak Compatibility](#espeak-compatibility)
- [History](#history)
- [License Information](#license-information)
----------

The eSpeak NG is a compact open source software text-to-speech synthesizer for 
Linux, Windows, Android and other operating systems. It supports 
[more than 100 languages and accents](docs/languages.md). It is based on the eSpeak engine
created by Jonathan Duddington.

eSpeak NG uses a "formant synthesis" method. This allows many languages to be
provided in a small size. The speech is clear, and can be used at high speeds,
but is not as natural or smooth as larger synthesizers which are based on human
speech recordings. It also supports Klatt formant synthesis, and the ability
to use MBROLA as backend speech synthesizer.

eSpeak NG is available as:

*  A [command line](src/espeak-ng.1.ronn) program (Linux and Windows) to speak text from a file or
   from stdin.
*  A [shared library](docs/integration.md) version for use by other programs. (On Windows this is
   a DLL).
*  A SAPI5 version for Windows, so it can be used with screen-readers and
   other programs that support the Windows SAPI5 interface.
*  eSpeak NG has been ported to other platforms, including Solaris and Mac
   OSX.

## Features

*  Includes different Voices, whose characteristics can be altered.
*  Can produce speech output as a WAV file.
*  SSML (Speech Synthesis Markup Language) is supported (not complete),
   and also HTML.
*  Compact size.  The program and its data, including many languages,
   totals about few Mbytes.
*  Can be used as a front-end to [MBROLA diphone voices](docs/mbrola.md).
   eSpeak NG converts text to phonemes with pitch and length information.
*  Can translate text into phoneme codes, so it could be adapted as a
   front end for another speech synthesis engine.
*  Potential for other languages. Several are included in varying stages
   of progress. Help from native speakers for these or other languages is
   welcome.
*  C engine with an incremental native Rust port.

See the [ChangeLog](ChangeLog.md) for a description of the changes in the
various releases and with the eSpeak NG project.

The following platforms are supported:

| Platform    | Minimum Version | Status |
|-------------|-----------------|--------|
| Linux       |                 | ![CI](https://github.com/espeak-ng/espeak-ng/actions/workflows/ci.yml/badge.svg) |
| BSD         |                 |        |
| Android     | 4.0             |        |
| Windows     | Windows 8       |        |
| Mac         |                 |        |

## Documentation

1. [User guide](docs/guide.md) explains how to set up and use eSpeak NG from command line or as a library.
2. [Building guide](docs/building.md) provides info how to compile and build eSpeak NG from the source.
4. [Index](docs/index.md) provides full list of more detailed information for contributors and developers.
5. Look at [contribution guide](docs/contributing.md) to start your contribution.
6. Look at [eSpeak NG roadmap](https://github.com/espeak-ng/espeak-ng/wiki/eSpeak-NG-roadmap) to participate in development of eSpeak NG.

## eSpeak Compatibility

The *espeak-ng* binaries use the same command-line options as *espeak*, with
several additions to provide new functionality from *espeak-ng* such as specifying
the output audio device name to use. The build creates symlinks of `espeak` to
`espeak-ng`, and `speak` to `speak-ng`.

The espeak `speak_lib.h` include file is located in `espeak-ng/speak_lib.h` with
an optional symlink in `espeak/speak_lib.h`. This file contains the espeak 1.48.15
API, with a change to the `ESPEAK_API` macro to fix building on Windows
and some minor changes to the documentation comments. This C API is API and ABI
compatible with espeak.

The `espeak-data` data has been moved to `espeak-ng-data` to avoid conflicts with
espeak. There have been various changes to the voice, dictionary and phoneme files
that make them incompatible with espeak.

The *espeak-ng* project does not include the *espeakedit* program. It has moved
the logic to build the dictionary, phoneme and intonation binary files into the
`libespeak-ng.so` file that is accessible from the `espeak-ng` command line and
C API.

## Related projects

* **[espeak-ng-sapi](https://github.com/gozaltech/espeak-ng-sapi)** –  
  A third-party Windows SAPI 5 engine implementation for eSpeak NG.

## History

The program was originally known as __speak__ and originally written
for Acorn/RISC\_OS computers starting in 1995 by Jonathan Duddington. This was
enhanced and re-written in 2007 as __eSpeak__, including a relaxation of the
original memory and processing power constraints, and with support for additional
languages.

In 2010, Reece H. Dunn started maintaining a version of eSpeak on GitHub that
was designed to make it easier to build eSpeak on POSIX systems, porting the
build system to autotools in 2012. In late 2015, this project was officially
forked to a new __eSpeak NG__ project. The new eSpeak NG project is a significant
departure from the eSpeak project, with the intention of cleaning up the
existing codebase, adding new features, and adding to and improving the
supported languages.

The *historical* branch contains the available older releases of the original
eSpeak that are not contained in the subversion repository.

1.24.02 is the first version of eSpeak to appear in the subversion
repository, but releases from 1.05 to 1.24 are available at
[http://sourceforge.net/projects/espeak/files/espeak/](http://sourceforge.net/projects/espeak/files/espeak/).

These early releases have been checked into the historical branch,
with the 1.24.02 release as the last entry. This makes it possible
to use the replace functionality of git to see the earlier history:

	git replace 8d59235f 63c1c019

__NOTE:__ The source releases contain the `big_endian`, `espeak-edit`,
`praat-mod`, `riskos`, `windows_dll` and `windows_sapi` folders. These
do not appear in the source repository until later releases, so have
been excluded from the historical commits to align them better with
the 1.24.02 source commit.

## License Information

eSpeak NG Text-to-Speech is released under the [GPL version 3](COPYING) or
later license.

The `getopt.c` compatibility implementation for getopt support on Windows is
taken from the NetBSD `getopt_long` implementation, which is licensed under a
[2-clause BSD](COPYING.BSD2) license.

Android is a trademark of Google LLC.

## Acknowledgements

The catalan extension was funded by [Departament de la Vicepresidència i de Polítiques Digitals i Territori de la Generalitat de Catalunya](https://politiquesdigitals.gencat.cat/ca/inici/index.html#googtrans(ca|en))
within the framework of
[Projecte AINA](https://politiquesdigitals.gencat.cat/ca/economia/catalonia-ai/aina).
