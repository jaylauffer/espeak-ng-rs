/* Native active-voice admission and host. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_VOICE_NATIVE_H
#define ESPEAK_RUST_VOICE_NATIVE_H
#include "rust_data.h"
typedef struct {
 void *voice,*translator;
 int32_t *fast,*translator_index;
 espeak_VOICE *selected;
 int dictionary_ready;
} RustVoiceEnvironment;
typedef struct {
 void *context;
 const char *root,*default_voice;
 RustCurrentVoice **current;
 int separator;
 size_t width;
 uint32_t features;
 void (*environment)(void *,RustVoiceEnvironment *);
 int64_t (*length)(void *,const unsigned char *,size_t);
 void (*bind)(void *,const RustCurrentVoice *);
 void (*reset)(void *,int);
 void (*delete_translator)(void *,int);
 void (*translator)(void *,const char *);
 int (*table)(void *,const char *);
 void (*options)(void *,uint32_t,const char *);
 void (*speed)(void *);
 void (*replacement)(void *,const char *);
 void (*dictionary)(void *,const char *,uint32_t);
 uint32_t (*mbrola)(void *,const RustMbrolaRequest *,int32_t *);
 void (*backend_error)(void *,uint32_t);
 void (*diagnostic)(void *,const char *,const char *);
} RustVoiceNative;
/* Whole-engine access remains serialized by the caller. Descriptor, root and
 * initial path-name copy before effects. Root terminates within4096; default
 * within40. Width/path capacity is2..4096, separator '/' or '\\', features0..3.
 * Ordinary requests supply bytes through NUL or41 readable bytes: initial path
 * consumes39, current preparation rereads the fresh original41-byte prefix.
 * Explicit paths terminate within width. NULL request retains compilation use.
 * Stable aligned current slot owns only create()-allocated RustCurrentVoice;
 * no callback replaces/destroys it or retained views. It allocates only after
 * accepted fallback, and has stable initialized mutable metadata fields.
 * Environment initializes all fields and returns fresh quiescent engine views:
 * voice/fast/selected are live initialized writable projections; translator_index
 * is writable when a translator exists. No reference spans a native primitive.
 * Rust owns diagnostic templates; the printf primitive preserves unbuffered
 * stream effects and ignores historical write errors. Callback loans are
 * synchronous, terminated, immutable and disjoint
 * from native context/engine storage; callbacks cannot retain, reenter or unwind.
 * Actual source reads complete through the existing engine proactor before
 * preparation/reset; one guarded reusable input buffer retains at most the16MiB
 * logical text-file bound. No I/O/native effect runs under the buffer pool lock.
 * Native/current clear and nested public selection refuse active admission.
 * Callers must still serialize every engine API and drain views before clear.
 * load returns0 and stores fresh voice on success; EINVAL malformed, EAGAIN
 * nested/concurrent, VOICE_NOT_FOUND ordinary absence/backend/dictionary failure.
 * Output is aligned writable and disjoint from live engine/owner state; it stays
 * unchanged on refusal. Accepted partial effects remain visible; no rollback.
 * Source closes before final status/language publication, which updates only
 * languages, preserving a fresh visible name changed by preceding effects.
 * clear leaves owner/slot intact on refusal; success releases and clears the
 * slot after the caller has drained its views.
 */
int espeak_rs_voice_native_load(const RustVoiceNative *,const char *,uint32_t,void **);
int espeak_rs_voice_native_clear(RustCurrentVoice **);
#endif
