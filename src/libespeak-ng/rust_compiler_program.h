/* Owned bytecode assembler ABI. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_PROGRAM_H
#define ESPEAK_RUST_COMPILER_PROGRAM_H
#include <stddef.h>
#include <stdint.h>
#define RUST_PROGRAM_WORDS 2020 /* original nominal buffer plus twenty slack words */
typedef struct { int32_t value, kind, terminator; unsigned char text[256]; } RustProgramToken;
typedef struct {
 uint64_t owner; void *context; const uint16_t *words; size_t length;
 int32_t control, kind;
 int32_t (*next)(void *, int32_t, RustProgramToken *);
 void (*unget)(void *);
 void (*error)(void *, const unsigned char *, size_t);
 int32_t (*lookup)(void *, const unsigned char *, size_t);
 int32_t (*load)(void *, const unsigned char *, size_t, int32_t);
 int64_t (*position)(void *);
 int32_t (*write)(void *, int32_t, const unsigned char *, size_t);
} RustCompilerProgram;
typedef struct { int32_t count, after_if, level, then_count, returned, value, start, end, status; } RustProgramResult;
enum { RUST_PROGRAM_RESET, RUST_PROGRAM_EMIT, RUST_PROGRAM_TRANSITION, RUST_PROGRAM_IF,
 RUST_PROGRAM_ELSE, RUST_PROGRAM_ELIF, RUST_PROGRAM_ENDIF, RUST_PROGRAM_SWITCH,
 RUST_PROGRAM_TONE, RUST_PROGRAM_SOUND, RUST_PROGRAM_DECREMENT, RUST_PROGRAM_RETURNED,
 RUST_PROGRAM_CLEAR, RUST_PROGRAM_FINISH, RUST_PROGRAM_PUBLISH };
/* Serialized calls. Inputs readable for declared extents; result writable.
 * Descriptor and emitted words are copied before callbacks; input/result may
 * overlap. Callbacks are synchronous, cannot retain loans or unwind. No views
 * of owner storage escape. Four retained leases; nested calls reject EAGAIN.
 * End invalidates IDs. Failed publication cannot be replayed before reset.
 * emit admits at most 256 copied input words per call; storage is bounded by
 * RUST_PROGRAM_WORDS. copy requires writable capacity words and never returns
 * a retained pointer. The original separate IF admission limit is preserved. */
uint64_t espeak_rs_compiler_program_begin(void);
int32_t espeak_rs_compiler_program_end(uint64_t);
int32_t espeak_rs_compiler_program_call(const RustCompilerProgram *, uint32_t, RustProgramResult *);
int32_t espeak_rs_compiler_program_copy(uint64_t, uint16_t *, size_t);
#endif
