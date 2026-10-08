/* Native Klatt owner adapter. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_KLATT_H
#define ESPEAK_RUST_KLATT_H
#include "rust_data.h"
typedef struct RustKlatt RustKlatt;
typedef struct {
    RustWaveMemory *memory;
    RustOutput *output;
    int32_t (*random)(void);
    void (*reset_speechplayer)(void);
} RustKlattShared;
RustKlatt *espeak_rs_klatt_new(void);
void espeak_rs_klatt_free(RustKlatt *);
void espeak_rs_klatt_init(RustKlatt *);
void espeak_rs_klatt_reset(RustKlatt *,int32_t);
int32_t espeak_rs_klatt_fill(RustKlatt *,const RustKlattShared *,int32_t,int32_t,
    const frame_t *,const frame_t *,WGEN_DATA *,const voice_t *);
#endif
