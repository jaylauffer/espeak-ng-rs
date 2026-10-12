/* Native wave-generator host. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_WAVEGEN_HOST_H
#define ESPEAK_RUST_WAVEGEN_HOST_H
#include "rust_data.h"
typedef struct {double speed;void *stream;} RustSonicState;
extern RustSonicState espeak_rs_sonic_state;
extern espeak_ng_OUTPUT_HOOKS *output_hooks;
typedef struct {
 void *context;
 RustWaveMemory *memory;
 RustOutput *output;
 espeak_ng_OUTPUT_HOOKS **hooks;
 RustSonicState *sonic;
 const char *mbrola_name;
 uint32_t features;
 void (*marker)(int,unsigned int,int,int,unsigned char *);
 void (*release)(void *);
 int (*random)(int,int);
 void (*klatt_reset)(int);
 int (*klatt)(const RustWavegenEffect *);
 int (*mbrola)(int,int,int);
 void (*mbrola_reset)(void);
 void *(*create)(int,int);
 void (*destroy)(void *);
 int (*flush)(void *);
 int (*read)(void *,short *,int);
 int (*write)(void *,short *,int);
 float (*get_speed)(void *);
 void (*set_speed)(void *,float);
 int (*fill)(void *);
 void (*rescale)(int,int);
} RustWavegenHost;
/* Caller serializes every engine API and drains queue/resource views. Descriptor
 * and effect copy before callbacks; no reference to engine/output/hook/stream
 * storage spans a primitive. Hooks remain caller-owned mutable initialized
 * tables; registration from hooks is permitted, selected functions read fresh.
 * Features:1 Klatt,2 MBROLA,4 Sonic,8 ARCH_BIG. Queue has170 initialized rows.
 * Output cursor fields lie in the same live initialized PCM allocation; selected
 * extents fit int, aligned for short I/O, and outlive every synchronous loan.
 * Callbacks cannot retain loans, replace/free their backing allocations, unwind
 * or reenter the generator. Opaque stream slots are initialized; callbacks may
 * change them only to another live stream. Library read counts fit the capacity.
 * Source alignment/voice entries transfer C allocations, freed once after users
 * drain. Fill/stop refuse nested effect/fill without waiting; effects may occur
 * inside an admitted fill. Caller still serializes unrelated generator APIs.
 * Result is aligned, writable and disjoint from engine storage; publication is
 * after effects.0 success, EINVAL malformed/invalid bounds, EAGAIN reentry.
 * Accepted partial effects remain on failure. Stop preserves destroy-then-clear
 * ordering and MBROLA reset after queue/Sonic cleanup. No scheduler is added.
 * Generator retains its historical process lifetime because compiler/RMS raw
 * consumers still borrow it; whole-engine instance/lifetime ownership remains.
 */
RustWavegen *espeak_rs_native_wavegen_state(void);
int espeak_rs_native_output_hooks(espeak_ng_OUTPUT_HOOKS *);
int espeak_rs_wavegen_host_effect(const RustWavegenHost *,const RustWavegenEffect *,int *);
int espeak_rs_wavegen_host_fill(const RustWavegenHost *,int *);
int espeak_rs_wavegen_host_stop(const RustWavegenHost *);
#endif
