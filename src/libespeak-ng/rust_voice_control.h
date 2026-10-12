/* Complete public voice-selection controller. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_VOICE_CONTROL_H
#define ESPEAK_RUST_VOICE_CONTROL_H
#include <stdint.h>
#include <espeak-ng/speak_lib.h>
struct voice_t;
typedef struct {
 void *context;int separator;
 void (*classes)(void *,unsigned char *);
 void *(*load)(void *,const char *,int);
 int (*empty)(void *);
 void (*catalog)(void *);
 const char *(*by_name)(void *,const char *);
 const char *(*select)(void *,const espeak_VOICE *,int *);
 void (*change)(void *);
 void *(*current)(void *);
 void (*stack)(void *,const espeak_VOICE *,const char *);
} RustVoiceControl;
enum {RUST_VOICE_NAME,RUST_VOICE_FILE,RUST_VOICE_PROPERTIES,RUST_VOICE_VARIANT};
/* Whole-engine serialization. Descriptor/selected inputs copy before effects;
 * synchronous primitives cannot retain, mutate owned loans, reenter or unwind.
 * Request has initialized bytes through NUL or59 readable bytes: reproduce the
 * old59-byte truncation without scanning its unused tail. Incoming properties
 * name/id/language supply bytes through NUL or4095 readable prefix bytes; copied
 * prefixes terminate in owned storage without rejecting unused long tails.
 * Native names/identifiers fit below that bound, selection consumes a59-byte
 * fallback name and refuses languages longer than79. Fresh stack language
 * terminates within20 (the old stack used strcpy into that fixed field).
 * Long selection languages that find no voice still return VOICE_NOT_FOUND.
 * Selected output names from by_name/select terminate within4096 and are copied
 * before further effects. NULL properties strings retain their absent meaning.
 * classes writes256 initialized byte lower-case entries; index0 must remain0.
 * current returns a live initialized voice_t through its immediate language
 * snapshot, whose20-byte language contains NUL. streams/engine storage borrowed.
 * NAME uses every primitive; FILE needs classes/load/change/current/stack;
 * PROPERTIES needs load/select/change/stack; VARIANT needs load and output only.
 * Original request/selector/used strings remain live until return. Selection and
 * load inputs snapshot first; stack name/language/gender/age/variant reread fresh
 * after engine effects, preserving aliases of mutable current-voice metadata.
 * Stack consumes only those fields; other pointers are not dereferenced.
 * Request/selector/output are selected by mode; unused ones may be NULL. Variant
 * output is an aligned writable pointer slot, unchanged on refusal. Descriptor
 * and selected inputs may overlap it; they cannot overlap live callback state.
 * Snapshot refusal/invalid mode/map/callback EINVAL; nested/concurrent admission
 * EAGAIN; ordinary absence VOICE_NOT_FOUND. Already performed effects are visible.
 * No rollback: optional named tone failure is ignored, and found property
 * selection still changes/updates stack even if its base/tone load failed.
 * Variant loading itself returns the last successful load pointer or failure.
 */
int espeak_rs_voice_control_call(const RustVoiceControl *,uint32_t,const char *,const espeak_VOICE *,int,void **);
#endif
