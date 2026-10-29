/* Retained Rust spectrum owners; views are leased by create/load/destroy.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_SPECT_FILE_H
#define ESPEAK_RUST_SPECT_FILE_H
#include <stddef.h>
#include "spect.h"
struct RustWavegen;
struct RustWavegen *RustWavegenState(void);
SpectSeq *espeak_rs_spect_create(void);
void espeak_rs_spect_destroy(SpectSeq *);
int espeak_rs_spect_load(SpectSeq *,const char *,void (*)(const unsigned char *,size_t));
/* The same, logging to the C runtime's stderr. */
int espeak_rs_spect_load_crt(SpectSeq *,const char *);
float espeak_rs_spect_polint(const float *,const float *,int,float);
double espeak_rs_spect_rms(SpectFrame *,int,void *,int);
#endif
