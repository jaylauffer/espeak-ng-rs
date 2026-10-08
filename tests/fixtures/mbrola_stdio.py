#!/usr/bin/env python3
"""Blocking child protocol fixture; the Rust host drives all three sockets."""
# SPDX-License-Identifier: GPL-3.0-or-later
import os
import struct
import sys


def write_all(fd, data):
    view = memoryview(data)
    while view:
        view = view[os.write(fd, view):]


mode = sys.argv[1]
if mode == "exit":
    write_all(2, b"early child exit")
    sys.exit(7)

header = struct.pack(
    "<4sI4s4sIHHIIHH4sI",
    b"RIFF", 0, b"WAVE", b"fmt ", 16, 1, 1, 22050, 44100, 2, 16, b"data", 0,
)
if mode == "invalid":
    write_all(1, b"BAD!" + header[4:])
    sys.exit(0)
if mode == "truncated":
    write_all(1, header[:17])
    sys.exit(0)
if mode == "odd":
    write_all(1, header + b"\x01\x02\x03")
    sys.exit(0)
if mode == "stall":
    # Wait on actual input, without producing output: cancellation test.
    os.read(0, 1)
    sys.exit(0)

write_all(1, header)
if mode == "flood":
    # More than a socket's capacity, before consuming input. A host that
    # waits for stdout or command completion before reading stderr deadlocks.
    write_all(2, (b"warning " + b"x" * 242 + b"\n") * 4096)

while chunk := os.read(0, 4096):
    # Each input byte becomes an independent little-endian u16 sample.
    write_all(1, b"".join(struct.pack("<H", byte * 257) for byte in chunk))
write_all(2, b"latest warning without newline")
