/* Complete clause controller over scoped serialized owners.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_TRANSLATE_CLAUSE_H
#define ESPEAK_NG_RUST_TRANSLATE_CLAUSE_H
#include <stddef.h>
#include <stdint.h>
typedef struct {
    uint32_t flags; uint16_t start; uint8_t pause, reserved0;
    uint16_t source_index; uint8_t length, reserved1;
} RustClauseRow;
typedef struct { int32_t terminator, tone; size_t top; } RustClauseRead;
typedef struct {
    void *context;
    int (*value)(void *, unsigned);
    int (*store)(void *, unsigned, int);
    int (*byte)(void *, size_t);
    int (*write)(void *, size_t, unsigned char);
    int (*classify)(void *, unsigned, unsigned);
    unsigned (*lower)(void *, unsigned);
    int (*read)(void *, int16_t *, char *, RustClauseRead *);
    int (*embedded_command)(void *, size_t *, int *);
    int (*translate_char)(void *, size_t, unsigned, unsigned, unsigned, unsigned *, unsigned *, unsigned *);
    int (*upper_word)(void *, char *, size_t, unsigned, int *);
    int (*entry)(void *, int, void *);
    int (*put)(void *, int, const void *);
    int (*command)(void *, int, unsigned *);
    int (*put_command)(void *, int, unsigned);
    int (*translate)(void *, char *, size_t, char *, size_t, size_t, RustClauseRow *, size_t, size_t, int, unsigned *);
    int (*word_embedded)(void *);
    int (*eof)(void *);
    int (*make_list)(void *, int, int);
    int (*output)(void *, size_t, unsigned, unsigned);
    int (*result)(void *, unsigned, int, const char *);
} RustTranslateClause;
/* Table/context are aligned, immutable and retained for this serialized call,
 * disjoint from output slots and all owners. Scalar/index callbacks check live
 * owner bounds. Read, source/number/row buffers and results are initialized
 * disjoint owned scratch. Row slot zero is an initialized predecessor, logical
 * row i is slot i+1. Translate scopes/restores both rule and clause-end context,
 * retains no scratch pointer, and applies fresh primitive mutations in place.
 * Failure retains executed effects and completed publications, with no C
 * replay. Read writes stable C-owned voice storage, preserving previous returned
 * pointers even when current output slots are absent; only its initialized
 * terminated prefix is copied into scratch. Result voice publication retains
 * this stable owner and never keeps a pointer into Rust scratch. */
int espeak_rs_translate_clause(const RustTranslateClause *);
#endif
