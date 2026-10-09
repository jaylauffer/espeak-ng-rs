/* Native audio/event dispatch over serialized owner primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_ENGINE_AUDIO_H
#define ESPEAK_NG_RUST_ENGINE_AUDIO_H
#include "rust_data.h"
struct audio_object;
typedef struct {
    uint32_t capabilities; /* async, audio, latency */
    int format;
    int32_t (*value)(unsigned);
    void (*store)(unsigned, int32_t);
    struct audio_object *(*audio)(void);
    int (*enabled)(void);
    void (*close)(struct audio_object *);
    int (*open)(struct audio_object *, int, int, int);
    int (*write)(struct audio_object *, const void *, size_t);
    void (*diagnostic)(unsigned, int);
    void (*event_init)(void);
    int (*latency)(struct audio_object *);
    espeak_ng_STATUS (*declare)(const espeak_EVENT *, int);
    const long *samples;
    const int *mbrola_delay;
    t_espeak_callback **callback;
    const RustEventList *events;
} RustEngineAudio;
int espeak_rs_audio_dispatch(const RustEngineAudio *, short *, int, espeak_EVENT *);
int espeak_rs_audio_events(const RustEngineAudio *, short *, int, espeak_EVENT *);
espeak_ng_STATUS espeak_rs_audio_declare(const RustEngineAudio *, espeak_EVENT *);
#endif
