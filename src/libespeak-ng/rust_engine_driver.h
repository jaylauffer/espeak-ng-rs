/* Native bounded synthesis control; resource admission remains serialized.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_ENGINE_DRIVER_H
#define ESPEAK_NG_RUST_ENGINE_DRIVER_H
#include "rust_data.h"
typedef struct {
    RustOutput *output;
    RustEventList *events;
    long *samples;
    int *options[3]; /* SSML, phoneme input, end pause */
    Translator **translator;
    espeak_ng_TEXT_DECODER **decoder;
    unsigned (*encoding)(Translator *);
    espeak_ng_STATUS (*voice)(void);
    espeak_ng_TEXT_DECODER *(*create_decoder)(void);
    espeak_ng_STATUS (*decode)(espeak_ng_TEXT_DECODER *, const void *, espeak_ng_ENCODING, int);
    void (*begin)(RustOutput *);
    int (*fill)(void);
    void (*terminate_events)(RustEventList *, int, unsigned, void *);
    unsigned *identifier;
    void **user;
    int32_t (*value)(unsigned);
    t_espeak_callback **callback;
    int (*play)(short *, int, espeak_EVENT *);
    int (*dispatch)(short *, int, espeak_EVENT *);
    int (*generate)(void);
    int (*queued)(void);
    int (*clause)(int);
    int (*run)(int (*)(void *), void *);
} RustEngineDriver;
espeak_ng_STATUS espeak_rs_driver_synthesize(const RustEngineDriver *, unsigned, const void *, int);
int espeak_rs_driver_step(const RustEngineDriver *, unsigned, espeak_ng_STATUS *);
#endif
