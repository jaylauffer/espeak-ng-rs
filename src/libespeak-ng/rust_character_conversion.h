/* Complete character conversion over checked serialized resource owners.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_CHARACTER_CONVERSION_H
#define ESPEAK_NG_RUST_CHARACTER_CONVERSION_H
#include <stddef.h>
#include <stdint.h>
typedef struct {
    size_t index; uint32_t previous, code, next, insert, flags;
} RustCharacterCall;
typedef struct { uint32_t code, insert, flags; } RustCharacterResult;
typedef struct {
    void *context;
    int (*value)(void *,unsigned);
    int (*ignore)(void *,int);
    int (*byte)(void *,unsigned,size_t);
    int (*write)(void *,size_t,unsigned char);
    int (*classify)(void *,unsigned,unsigned);
    unsigned (*lower)(void *,unsigned);
    int (*trace)(void *,size_t,size_t);
} RustCharacterConversion;
/* Table and scalar inputs are aligned, initialized, immutable and retained for
 * this serialized call, disjoint from mutable owners/output. Byte callbacks
 * validate current clause/dictionary extents; no foreign slices or pointers are
 * kept across effects. Upper receives three initialized owned predecessor bytes.
 * Failure preserves public result slots and retains already executed effects,
 * with no C replay. Dictionary/translator/ignore-state ownership remains C. */
int espeak_rs_character_translate(const RustCharacterConversion *,const RustCharacterCall *,RustCharacterResult *);
int espeak_rs_character_upper(const RustCharacterConversion *,const unsigned char *,unsigned,int *);
#endif
