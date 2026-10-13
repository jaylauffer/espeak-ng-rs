/* Native speechPlayer engine admission; GPL-3.0-or-later. */
#ifndef ESPEAK_RUST_SPLAYER_H
#define ESPEAK_RUST_SPLAYER_H
#include <stddef.h>
#include "synthesize.h"
#include "voice.h"
/* Serialized initialized WaveMemory/Output owners. Pitch retains at least 128
 * bytes and matches WGEN_DATA.pitch_env on new frames. Mix describes the whole
 * immutable resident extent, not the nominal repeating-wave length. Output and
 * mutable metadata/result are disjoint from source views. The ordinary used
 * frame fields stay initialized; resume needs no voice/frame/pitch inputs.
 * Only C-used frame frequencies[1..6], bw[0..3] and klattp[1] are read; flags
 * and optional frame extensions are ignored, as in the original controller.
 * Voice reads only voicing, breath[1], freq/freqadd[1..6] and width[1..4].
 * Those fields and queue/pitch values copy before effects. The private Rust owner
 * and its 256-sample scratch reset/reuse in place; no per-fill allocation.
 * 0 success, EINVAL malformed/expired source, EAGAIN active/capacity refusal.
 * Result unchanged on refusal. A late bad mix read preserves completed
 * pitch/queue/DSP/PCM/cursor effects; callers handle status without C replay.
 * Drain engine calls before initialization/reset/finish; the only C runtime
 * effect is rand, which has no engine/resource callback or retained loan. */
typedef struct {
 const void *memory;
 void *output;
 const unsigned char *pitch;
 size_t pitch_size;
 const unsigned char *mix;
 size_t mix_size;
} RustSplayerShared;
int espeak_rs_splayer_initialize(void);
int espeak_rs_splayer_finish(void);
int espeak_rs_splayer_fill(const RustSplayerShared *, WGEN_DATA *, const voice_t *, int, int, const frame_t *, const frame_t *, int *);
#endif
