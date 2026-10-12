/* Synchronous intonation compiler; descriptor strings/maps are copied first.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_INTONATION_COMPILE_H
#define ESPEAK_RUST_INTONATION_COMPILE_H
#include <stddef.h>
typedef struct {
    void *context;
    const char *source, *destination;
    const unsigned char *spaces, *digits;
    size_t width;
    int windows;
    void (*log)(void *,const unsigned char *,size_t);
    void (*close)(void *);
    void (*reload)(void *);
    int (*error)(void *,int,const char *);
} RustIntonationCompiler;
int espeak_rs_compile_intonation(const RustIntonationCompiler *);
/* Native public host: paths/defaults selected and copied before I/O/effects.
 * Selected strings initialize through NUL within 4096 (all 4096 on refusal).
 * Selected diagnostic FILE is live/exclusive and obeys retained C close policy.
 * Admission refusal and normal/semantic-error completion leave it open;
 * source/discovery/output failure closes it. Locale/engine access stays serialized;
 * synchronous primitives cannot retain path/effects loans or unwind. */
typedef struct {
    const char *source, *destination, *home;
    void *log, *fallback_log, *error_context;
    size_t width;
    void (*reload)(void *);
    int (*error)(void *,int,const char *);
} RustNativeIntonationCompiler;
int espeak_rs_compile_intonation_native(const RustNativeIntonationCompiler *);
#endif
