/* Complete text control over serialized checked source/resource owners.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_TEXT_CONTROL_H
#define ESPEAK_NG_RUST_TEXT_CONTROL_H
#include <stddef.h>
#include <stdint.h>
typedef struct { uint32_t value; size_t index; } RustTextResult;
typedef struct {
    void *context;
    int (*value)(void *,unsigned);
    int (*store)(void *,unsigned,int);
    int (*byte)(void *,unsigned,size_t);
    int (*write)(void *,unsigned,size_t,unsigned char);
    int (*command)(void *,int,unsigned *);
    int (*put_command)(void *,int,unsigned);
    int (*default_value)(void *,size_t,int *);
    int (*alpha)(void *,unsigned);
    unsigned char (*lower)(void *,unsigned char);
    int (*row)(void *,size_t,unsigned *);
    int (*put_row)(void *,size_t,unsigned);
    int (*translate)(void *,unsigned,size_t,size_t,size_t,unsigned *);
    int (*vowel)(void *,unsigned char,int *);
    int (*lookup)(void *,const char *,int *);
    int (*select)(void *,int);
    int (*init)(void *,unsigned);
} RustTextControl;
/* Immutable aligned table/context remain retained for the serialized call,
 * disjoint from public output and live flag slots. Source/row/table callbacks
 * check current owner bounds; no foreign Rust loans cross effects. Translate
 * scopes/restores rule contexts; lookup receives a terminated owned 12-byte
 * name and retains none of it. Failure preserves the result and retains live
 * effects and completed flag publications, without C replay. Modes: embedded
 * parse 0, word commands 1, combination 2, language switch 3, text init 4,
 * syllable count 5. Scalar argument is source offset or text-init flags. */
int espeak_rs_text_control(const RustTextControl *,unsigned,size_t,RustTextResult *);
#endif
