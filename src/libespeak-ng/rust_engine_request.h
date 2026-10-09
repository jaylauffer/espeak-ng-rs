/* Native request admission over serialized engine primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_ENGINE_REQUEST_H
#define ESPEAK_NG_RUST_ENGINE_REQUEST_H
#include <stdbool.h>
#include <stdint.h>
#include "rust_async_command.h"
struct audio_object;
typedef struct {
    unsigned capabilities; /* asynchronous = 1, audio = 2 */
    int32_t (*value)(unsigned);
    struct audio_object *(*audio)(void);
    void (*init_text)(int);
    espeak_ng_STATUS (*synthesize)(unsigned, const void *, int);
    espeak_ng_STATUS (*key)(const char *);
    espeak_ng_STATUS (*character)(wchar_t);
    espeak_ng_STATUS (*parameter)(int, int, int);
    void (*punctuation)(const wchar_t *);
    t_espeak_command *(*create)(const t_espeak_command *, size_t);
    espeak_ng_STATUS (*single)(t_espeak_command *);
    espeak_ng_STATUS (*pair)(t_espeak_command *, t_espeak_command *);
    int (*delete_command)(t_espeak_command *);
    unsigned *identifier;
    void **user;
    const int *current;
    int *saved;
    int *skip[3];
    bool *skipping;
    int *end;
    char *marker; /* N_MARKER_LENGTH = 50 */
    int (*flush)(struct audio_object *);
    int (*drain)(struct audio_object *);
    const char *(*audio_error)(struct audio_object *, int);
    void (*diagnose)(const char *, const char *);
} RustEngineRequest;
espeak_ng_STATUS espeak_rs_request_submit(const RustEngineRequest *, const t_espeak_command *, size_t, unsigned *);
espeak_ng_STATUS espeak_rs_request_synthesize(const RustEngineRequest *, const t_espeak_text *);
espeak_ng_STATUS espeak_rs_request_mark(const RustEngineRequest *, const t_espeak_mark *);
#endif
