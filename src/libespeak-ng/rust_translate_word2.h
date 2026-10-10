/* Serialized live projections for complete word-to-phoneme-list policy.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_TRANSLATE_WORD2_H
#define ESPEAK_NG_RUST_TRANSLATE_WORD2_H
#include <stddef.h>
#include <stdint.h>
typedef struct { int kind, length; unsigned program; } RustWord2Phoneme;
typedef struct {
    void *context;
    int (*value)(void *, unsigned);
    int (*store)(void *, unsigned, int);
    int (*byte)(void *, size_t, unsigned);
    int (*write)(void *, size_t, unsigned char, unsigned);
    int (*entry)(void *, int, PHONEME_LIST2 *);
    int (*put)(void *, int, const PHONEME_LIST2 *);
    int (*command)(void *, int, unsigned *);
    int (*put_command)(void *, int, unsigned);
    int (*embedded)(void *);
    int (*encode)(void *, unsigned);
    int (*translate)(void *, unsigned, unsigned, char *, unsigned *);
    int (*combine)(void *, unsigned *);
    int (*dictionary)(void *, char *);
    int (*restore_dictionary)(void *, const char *);
    int (*alternate)(void *, const char *, int *);
    int (*select)(void *, int);
    int (*phoneme)(void *, unsigned char, RustWord2Phoneme *);
    int (*alpha)(void *, unsigned char);
    void (*invalid)(void *, unsigned char);
} RustTranslateWord2;
/* Every callback/context required. Byte/write resolve checked live original
 * source or 200-byte pronunciation. Entries/commands project checked owner
 * indices and preserve untouched fields. Translate owns initialized 163-byte
 * replacement scratch; alternate mode 1 uses its byte-1 word, disables output
 * and scopes/restores translator rule context. Dictionary scratch is 40 bytes;
 * alternate input is a terminated owned 200-byte name (empty means default).
 * No pointer is kept. Result slot is aligned/disjoint; -1 preserves it and
 * retains executed effects without replay. All owners remain serialized. */
int espeak_rs_translate_word2(const RustTranslateWord2 *, int, unsigned *);
#endif
