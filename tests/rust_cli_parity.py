#!/usr/bin/env python3
"""Compare the Rust CLI with an unchanged C main linked to the same engine.

This is a process-level differential test, not an audio-device test. Every
synthesis case either requests quiet mode or writes a deterministic WAV. Both
executables run in the same recreated directory: only their executable names
are normalized in diagnostics. Compiler cases receive private copies of data.
"""

import argparse
import dataclasses
import hashlib
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import threading
import time


@dataclasses.dataclass
class Case:
    name: str
    args: tuple
    stdin: bytes = b""
    inputs: dict = dataclasses.field(default_factory=dict)
    directories: tuple = ()
    fifo: bytes = None
    private_data: bool = False
    status: int = 0
    stdout_nonempty: bool = False
    required_files: tuple = ()
    absent_files: tuple = ()
    wav_files: tuple = ()
    wav_stdout: bool = False
    split_pattern: str = ""


@dataclasses.dataclass
class Result:
    status: int
    stdout: bytes
    stderr: bytes
    files: dict


def corpus(data, mbrola):
    cases = []

    def add(name, *args, **kwargs):
        cases.append(Case(name, tuple(args), **kwargs))

    def phon(name, *args, **kwargs):
        add(name, "-D", "-q", "-x", *args, **kwargs)

    def wave(name, *args, **kwargs):
        add(name, "-D", "-w", "out.wav", *args,
            wav_files=("out.wav",), **kwargs)

    for flag in ("-h", "--help", "--version", "--voices", "--voices=en",
                 "--voices=", "--voices=not-a-language"):
        add("info-" + flag.lstrip("-"), flag, stdout_nonempty=True)
    add("help-before-invalid", "--help", "--unknown", stdout_nonempty=True)
    add("version-before-input", "--version", "-f", "absent.txt",
        stdout_nonempty=True)
    add("voices-before-invalid-path", "--voices=en", "--path=absent",
        stdout_nonempty=True)
    add("version-ignores-path", "--path=absent", "--version",
        stdout_nonempty=True)
    add("explicit-data-path", "--path", str(data), "--voices=en",
        stdout_nonempty=True)
    for index, args in enumerate((
        ("--unknown",), ("-!",), ("--ipa=1", "--unknown"),
        ("--help=yes",), ("--ver",), ("--comp",), ("--ph",),
        ("--path",), ("-v",), ("--ssml-break",), ("--stdin=yes",),
        ("-qx!",), ("--unknown", "--help"),
    )):
        add("getopt-" + str(index), *args)
    for flag in ("--compile", "--compile-debug"):
        for suffix in ((), ("",), ("en",)):
            args = (flag + "=",) if suffix == ("",) else (flag, *suffix)
            add("compile-required-" + flag[2:] + repr(suffix), *args, status=1)

    phon("positional", "Hello, world!", stdout_nonempty=True)
    phon("first-positional-only", "First sentence.", "Ignored second sentence.")
    phon("empty-positional", "")
    phon("permuted-options", "Hello world.", "-ven", "--ipa=1")
    phon("double-dash", "--", "-x --voices is text.")
    phon("single-dash", "-")
    phon("clustered-short", "-zmk2", "<speak>NASA asks: why?</speak>")
    phon("last-voice-wins", "-vfr", "-ven", "Hello world.")
    phon("unknown-voice", "-v", "not-a-real-espeak-voice", "Hello", status=1)
    phon("voice-name-truncation", "-v", "unknown" * 20, "Hello", status=1)
    phon("quiet-device", "-d", "unused-device" * 30, "Hello.")
    for voice, text in (("en-gb", "Hello world."), ("fr", "Bonjour été."),
                        ("de", "Guten Tag."), ("en+f3", "Hello world.")):
        phon("voice-" + voice, "-v", voice, text, stdout_nonempty=True)
    for option in ("--ipa", "--ipa=", "--ipa=0", "--ipa=1", "--ipa=2",
                   "--ipa=3", "--ipa=garbage", "--sep", "--sep=",
                   "--sep=z", "--sep=·", "--sep=abc", "--tie", "--tie=",
                   "--tie=z", "--tie=͡", "--tie=abc", "--pho"):
        phon("phonemes-" + option[2:], option, "A cheerful giant enjoys tea.")
    phon("separator-last-wins", "--sep=_", "--tie=z", "--sep=|", "Church.")
    phon("trace", "-X", "Hello world.", stdout_nonempty=True)
    phon("phoneme-input", "[[h@l'oU w'3:ld]]")
    for option in ("--punct", "--punct=", "--punct=!?", "--punct=é!?",
                   "--punct=" + "!?" * 70):
        phon("punctuation-" + str(len(cases)), option, "Hello, world! Why?")
    phon("ssml", "-m", "--ssml-break=75", "<speak>Hello <emphasis>world</emphasis>"
         "<break time='40ms'/>Again.</speak>")
    phon("line-length", "-l15", "We are\nWe are testing\nWe are testing line length.")
    wchar = 2 if os.name == "nt" else 4
    for encoding in ("0", "1", "2", "3", "4", "5", "bad", " 1suffix"):
        text = "Hello café.\n"
        if encoding == "3":
            # wchar_t input ends at a zero wchar_t. The original adds one NUL
            # byte after the file, so the file supplies the rest of that unit;
            # UTF-8 bytes here made the C reference read past its buffer.
            width = "16" if wchar == 2 else "32"
            order = "le" if sys.byteorder == "little" else "be"
            payload = text.encode(f"utf-{width}-{order}") + b"\0" * (wchar - 1)
        else:
            payload = text.encode("utf-16-le" if encoding == "4" else "utf-8")
        phon("encoding-" + encoding, "-b", encoding, "-f", "input.txt",
             inputs={"input.txt": payload})
    phon("parameter-atoi", "-s180suffix", "-a+90", "-p40tail", "-P35", "-g1",
         "-l12", "-k2", "--ssml-break=100tail", "NASA says hello.")
    phon("parameter-nonpositive", "-s0", "-a-1", "-p-1", "-P-1", "-g-1",
         "-l-1", "-k-1", "--ssml-break=-1", "Hello.")
    phon("parameter-repeated", "-s80", "-s200", "-a10", "-a100", "Hello.")

    for name, content in (
        ("empty", b""), ("one", b"Hello"),
        ("lines", b"One line.\nSecond line.\nLast line."),
        ("crlf", b"Hello.\r\nWorld.\r\n"),
        ("ctrl-z", b"First.\x1aSecond.\n"),
        ("embedded-nul", b"First.\x00Hidden.\nLast.\n"),
        ("utf8", "Café, naïve, κόσμος.\n".encode()),
    ):
        phon("file-" + name, "-finput.txt", inputs={"input.txt": content})
        phon("stdin-lines-" + name, stdin=content)
        phon("stdin-bulk-" + name, "--stdin", stdin=content)
    # fgets has a 1,000-byte buffer: the newline may fall on either side of
    # the 999-byte payload boundary. Bulk stdin must preserve a different
    # call boundary, including when a UTF-8 code point straddles that limit.
    for length in (998, 999, 1000, 1998, 1999, 2000):
        content = (b"a " * (length // 2) + b"x" * (length % 2)
                   + b"\nLast sentence.\n")
        phon("stdin-record-" + str(length), stdin=content)
        phon("bulk-record-" + str(length), "--stdin", stdin=content)
    phon("stdin-split-utf8", stdin=b" " * 998 + "é hello.\n".encode())
    phon("bulk-split-utf8", "--stdin", stdin=b" " * 998 + "é hello.\n".encode())
    phon("stdin-overrides-positional", "--stdin", "Ignored.", stdin=b"Read me.\n")
    phon("file-overrides-positional", "-finput.txt", "Ignored.",
         inputs={"input.txt": b"Read the file.\n"})
    phon("file-plus-stdin", "-finput.txt", "--stdin", stdin=b"Read stdin.\n",
         inputs={"input.txt": b"Not the stdin bytes.\n"})
    phon("input-missing", "-f", "absent.txt", status=1)
    long_input = "i" * 199
    phon("input-path-truncation", "-f", long_input + "discarded",
         inputs={long_input: b"Truncated filename.\n"})
    voice_name = "v" * 39
    voice_source = b"name Local\nlanguage en\ngender female 35\n"
    phon("load-voice-file", "--load", "-vvoice", "Hello.",
         inputs={"voice": voice_source})
    phon("load-voice-truncation", "--load", "-v", voice_name + "discarded",
         "Hello.", inputs={voice_name: voice_source})
    phon("load-missing", "--load", "-vabsent", "Hello.", status=1)
    if hasattr(os, "mkfifo") and os.name != "nt":
        phon("fifo-lines", "-finput.fifo", fifo=b"First.\nSecond.\n")
        phon("fifo-record-boundary", "-finput.fifo", fifo=b"a " * 600 + b"\nEnd.\n")
        phon("fifo-empty", "-finput.fifo", fifo=b"")
        phon("fifo-overrides-bulk", "--stdin", "-finput.fifo", fifo=b"From FIFO.\n")

    for option in ("-x", "-X", "--ipa", "--pho"):
        add("phonout-" + option.lstrip("-"), "-D", "-q", option,
            "--phonout=trace.txt", "Hello world.", required_files=("trace.txt",))
    phon("phonout-repeated", "--phonout=first.txt", "--phonout=last.txt", "Hello.",
         required_files=("first.txt", "last.txt"))
    add("phonout-before-help", "--phonout=trace.txt", "--help",
        required_files=("trace.txt",), stdout_nonempty=True)
    add("phonout-open-failure", "--phonout=absent/trace.txt", "--version",
        stdout_nonempty=True)

    wave("wav-basic", "Hello world.")
    wave("wav-no-final-pause", "-z", "Hello world.")
    wave("wav-parameters", "-a80", "-p65", "-P30", "-s210", "-g2", "Hello world.")
    wave("wav-input-file", "-finput.txt", inputs={"input.txt": b"Hello.\nWorld.\n"})
    wave("wav-stdin-lines", stdin=b"Hello.\nWorld.\n")
    wave("wav-stdin-bulk", "--stdin", stdin=b"Hello.\nWorld.\n")
    wave("wav-ssml-break", "-m", "--ssml-break=50",
         "<speak>Hello.<break time='300ms'/>Again.</speak>")
    add("wav-stdout", "-D", "--stdout", "Hello world.", wav_stdout=True)
    add("wav-stdout-name", "-D", "-wstdout", "Hello.", wav_stdout=True)
    add("wav-stdout-leading-space", "-D", "-w", "  stdout", "Hello.", wav_stdout=True)
    add("wav-leading-space", "-D", "-w", " \ttrim.wav", "Hello.", wav_files=("trim.wav",))
    add("wav-last-file-wins", "-D", "--stdout", "-wout.wav", "Hello.",
        wav_files=("out.wav",))
    add("wav-last-stdout-wins", "-D", "-wout.wav", "--stdout", "Hello.",
        wav_stdout=True, absent_files=("out.wav",))
    add("wav-quiet", "-D", "-q", "-wout.wav", "Hello.", absent_files=("out.wav",))
    add("wav-quiet-stdout", "-D", "-q", "--stdout", "Hello.")
    add("wav-empty-stdin", "-D", "-wout.wav", "--stdin", absent_files=("out.wav",))
    add("wav-open-failure", "-D", "-wabsent/out.wav", "Hello.")
    add("wav-directory-failure", "-D", "-woutput", "Hello.", directories=("output",))
    add("wav-empty-path", "-D", "-w", "", "Hello.")
    long_wave = "w" * 199
    add("wav-path-truncation", "-D", "-w", long_wave + "discarded", "Hello.",
        wav_files=(long_wave,))
    add("wav-with-phonout", "-D", "-wout.wav", "-x", "--phonout=trace.txt", "Hello.",
        wav_files=("out.wav",), required_files=("trace.txt",))
    for option in ("--split", "--split=0", "--split=garbage", "--split=-1"):
        expected = "out.wav" if option in ("--split=0", "--split=garbage") else "out_01.wav"
        add("wav-" + option[2:], "-D", "-wout.wav", option, "Hello.", wav_files=(expected,))
    # This produces over a minute of PCM quickly, exercising actual sentence
    # rollover and finalized headers instead of merely testing name formatting.
    split_text = "One two three four five. " * 90
    for filename, pattern in (("split.wav", "split_*.wav"),
                              ("split.long", "split.long_*"),
                              ("split", "split_*")):
        add("wav-rollover-" + filename, "-D", "-w", filename, "--split=1",
            split_text, split_pattern=pattern)

    # No compiler may target the supplied build data. The fixture's isolated
    # copy is recreated before each executable, including failure cases.
    dictionary = {"en_list": b"hello h@l'oU\n", "en_rules": b".group\n"}
    for flag in ("--compile=en", "--compile-debug=en"):
        add("dictionary-" + flag[2:], flag, private_data=True, inputs=dictionary,
            required_files=("data/espeak-ng-data/en_dict",))
    add("dictionary-missing-source", "--compile=en", private_data=True, status=1)
    add("phonemes-missing-source", "--compile-phonemes=absent", private_data=True, status=1)
    add("phonemes-default-missing-source", "--compile-phonemes", private_data=True, status=1)
    minimal_phonemes = b"phoneme a\nvwl\nlength 100\nendphoneme\n"
    add("phonemes-compile", "--compile-phonemes=source", private_data=True,
        inputs={"source/phonemes": minimal_phonemes},
        required_files=("data/espeak-ng-data/phondata", "data/espeak-ng-data/phontab"))
    add("intonations-missing-source", "--compile-intonations", private_data=True, status=1)
    intonation = b"".join(
        b"tune " + name + b"\nprehead 46 57\nheadenv fall 16\nhead 4 80 50 -8 -5\n"
        b"nucleus0 fall 64 8\nnucleus fall 70 18 24 12\nendtune\n"
        for name in (b"s1", b"c1", b"q1", b"e1"))
    add("intonations-compile", "--compile-intonations", private_data=True,
        inputs={"data/phsource/intonation": intonation},
        required_files=("data/espeak-ng-data/intonations",))
    if mbrola:
        add("mbrola-missing-argument", "--compile-mbrola", private_data=True, status=1)
        add("mbrola-missing-source", "--compile-mbrola=absent", private_data=True, status=1)
        add("mbrola-compile", "--compile-mbrola=voice", private_data=True,
            inputs={"voice": b"volume 18\n0 a NULL 0 a\n0 t VWL 50 t d\n"},
            directories=("data/espeak-ng-data/mbrola_ph",),
            required_files=("data/espeak-ng-data/mbrola_ph/voice_phtrans",))
    return cases


def normalized_diagnostics(data, executable):
    # getopt differs between platforms in whether it prints argv[0], basename,
    # or basename without .exe. Do not normalize whitespace, paths, or messages.
    names = {str(executable), executable.name, executable.stem}
    for name in sorted(names, key=len, reverse=True):
        data = data.replace(os.fsencode(name) + b":", b"<executable>:")
    return data


def digest(path):
    hasher = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            hasher.update(chunk)
            size += len(chunk)
    return size, hasher.hexdigest()


def check_wav(content, streaming, description):
    if len(content) < 44 or content[:4] != b"RIFF" or content[8:16] != b"WAVEfmt ":
        raise AssertionError(description + ": missing canonical WAV header")
    riff_size, = struct.unpack_from("<I", content, 4)
    fmt_size, encoding, channels, rate, byte_rate, align, bits = struct.unpack_from("<IHHIIHH", content, 16)
    data_size, = struct.unpack_from("<I", content, 40)
    if (fmt_size, encoding, channels, align, bits) != (16, 1, 1, 2, 16):
        raise AssertionError(description + ": expected mono PCM16")
    if rate == 0 or byte_rate != rate * 2 or content[36:40] != b"data":
        raise AssertionError(description + ": inconsistent rate/data fields")
    expected = (0x7FFFF024, 0x7FFFF000) if streaming else (len(content) - 8, len(content) - 44)
    if (riff_size, data_size) != expected or (len(content) - 44) % 2:
        raise AssertionError(description + ": wrong streaming/finalized WAV lengths")
    if len(content) == 44:
        raise AssertionError(description + ": unexpectedly empty PCM")


def run_case(executable, case, work, data, env, timeout):
    if work.exists():
        shutil.rmtree(work)
    work.mkdir()
    case_env = dict(env)
    if case.private_data:
        private = work / "data"
        shutil.copytree(data / "espeak-ng-data", private / "espeak-ng-data")
        case_env["ESPEAK_DATA_PATH"] = str(private)
        prefix = ["--path", str(private)]
    else:
        prefix = []
    for name in case.directories:
        (work / name).mkdir(parents=True, exist_ok=True)
    for name, content in case.inputs.items():
        path = work / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    fifo = work / "input.fifo"
    writer = None
    writer_errors = []
    if case.fifo is not None:
        os.mkfifo(fifo)

        def produce():
            try:
                with fifo.open("wb", buffering=0) as stream:
                    remaining = memoryview(case.fifo)
                    while remaining:
                        count = stream.write(remaining)
                        remaining = remaining[count:]
            except OSError as error:
                writer_errors.append(error)

        writer = threading.Thread(target=produce, daemon=True)
        writer.start()
    try:
        completed = subprocess.run([str(executable), *prefix, *case.args], cwd=work,
                                   env=case_env, input=case.stdin, capture_output=True,
                                   timeout=timeout)
    finally:
        if writer is not None:
            # A parser failure can exit without ever opening the FIFO. Opening
            # a nonblocking reader releases a producer still blocked in open;
            # no timer loop or sleeping producer is needed.
            rescue = os.open(fifo, os.O_RDONLY | os.O_NONBLOCK)
            try:
                writer.join(timeout=5)
            finally:
                os.close(rescue)
            if writer.is_alive():
                raise AssertionError(case.name + ": FIFO producer did not terminate")
    if writer_errors:
        raise AssertionError(case.name + ": FIFO producer failed: " + str(writer_errors[0]))
    if completed.returncode != case.status:
        raise AssertionError("{}: {} exited {}, expected {}; stderr={!r}".format(
            case.name, executable.name, completed.returncode, case.status, completed.stderr[:500]))
    if case.stdout_nonempty and not completed.stdout:
        raise AssertionError(case.name + ": expected nonempty stdout")
    for name in case.required_files:
        if not (work / name).is_file():
            raise AssertionError(case.name + ": missing output " + name)
    for name in case.absent_files:
        if (work / name).exists():
            raise AssertionError(case.name + ": unexpected output " + name)
    waves = [work / name for name in case.wav_files]
    if case.split_pattern:
        split = sorted(work.glob(case.split_pattern))
        if len(split) < 2:
            raise AssertionError(case.name + ": input failed to exercise WAV rollover")
        waves.extend(split)
    for path in waves:
        check_wav(path.read_bytes(), False, case.name + "/" + path.name)
    if case.wav_stdout:
        check_wav(completed.stdout, True, case.name + "/stdout")
    files = {path.relative_to(work).as_posix(): digest(path)
             for path in sorted(work.rglob("*")) if path.is_file()}
    return Result(completed.returncode, completed.stdout,
                  normalized_diagnostics(completed.stderr, executable), files)


def difference(expected, actual):
    for field in ("status", "stdout", "stderr", "files"):
        left, right = getattr(expected, field), getattr(actual, field)
        if left == right:
            continue
        if isinstance(left, bytes):
            offset = next((i for i, (a, b) in enumerate(zip(left, right)) if a != b),
                          min(len(left), len(right)))
            start = max(0, offset - 40)
            return "{} differs at byte {} (reference {} bytes, native {} bytes)\n  reference {!r}\n  native    {!r}".format(
                field, offset, len(left), len(right), left[start:start + 180], right[start:start + 180])
        if field == "files":
            changed = [name for name in sorted(set(left) | set(right)) if left.get(name) != right.get(name)]
            return "file content/size differs: " + ", ".join(changed[:12])
        return "{} differs: reference {!r}, native {!r}".format(field, left, right)
    return None


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="Example:\n  python3 tests/rust_cli_parity.py --native build/src/espeak-ng "
               "\\\n    --reference build/src/espeak-ng-cli-reference --data build\n\n"
               "FIFO cases run only where os.mkfifo is available. No audio device or\n"
               "external MBROLA executable is used; --help detects MBROLA compile support.")
    parser.add_argument("--native", required=True, type=Path,
                        help="required: executable containing the Rust CLI")
    parser.add_argument("--reference", required=True, type=Path,
                        help="required: unchanged C CLI linked to the same engine")
    parser.add_argument("--data", required=True, type=Path,
                        help="required: directory containing compiled espeak-ng-data")
    parser.add_argument("--timeout", type=float, default=30,
                        help="optional: timeout per executable/case in seconds (default: 30)")
    parser.add_argument("--case", default="",
                        help="optional: run only case names containing this substring")
    parser.add_argument("--verbose", action="store_true",
                        help="optional: print each passing case")
    args = parser.parse_args()
    native, reference, data = args.native.resolve(), args.reference.resolve(), args.data.resolve()
    for executable in (native, reference):
        if not executable.is_file():
            parser.error("executable does not exist: " + str(executable))
    if not (data / "espeak-ng-data" / "phondata").is_file():
        parser.error("--data must contain compiled espeak-ng-data/phondata")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    env = dict(os.environ, ESPEAK_DATA_PATH=str(data), LC_ALL="C", LANG="C")
    # Make argument permutation deterministic even in POSIXLY_CORRECT shells.
    env.pop("POSIXLY_CORRECT", None)
    env.pop("GETOPT_COMPATIBLE", None)
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="espeak-cli-parity-") as temporary:
        root = Path(temporary)
        help_result = subprocess.run([str(reference), "--help"], env=env, cwd=root,
                                     input=b"", capture_output=True, timeout=args.timeout,
                                     check=True)
        mbrola = b"--compile-mbrola" in help_result.stdout
        cases = [case for case in corpus(data, mbrola) if args.case in case.name]
        if not cases:
            parser.error("--case matched no cases")
        failures = []
        for case in cases:
            try:
                expected = run_case(reference, case, root / "case", data, env, args.timeout)
                actual = run_case(native, case, root / "case", data, env, args.timeout)
                failure = difference(expected, actual)
                if failure:
                    raise AssertionError(failure)
            except (AssertionError, OSError, subprocess.TimeoutExpired) as error:
                failures.append(case.name)
                print("FAIL {}: {}".format(case.name, error), file=sys.stderr, flush=True)
            else:
                if args.verbose:
                    print("PASS " + case.name, flush=True)
        print("CLI parity: {}/{} cases passed; FIFO {}; MBROLA compiler {}; {:.1f}s".format(
            len(cases) - len(failures), len(cases), "on" if hasattr(os, "mkfifo") and os.name != "nt" else "off",
            "on" if mbrola else "off", time.monotonic() - started), flush=True)
        return int(bool(failures))


if __name__ == "__main__":
    sys.exit(main())
