/* Native Rust migration boundary. Offsets refer to validated resident bytes.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_DATA_H
#define ESPEAK_RUST_DATA_H
#include <stddef.h>
#include <stdint.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/encoding.h>
#include "phoneme.h"
#include "synthesize.h"
#include "voice.h"
#include <string.h>
#include "soundicon.h"
#include "mbrola.h"
#include <wchar.h>
#include "mnemonics.h"
#include "readclause.h"
#include "ssml.h"
typedef struct RustSsmlVoiceChoice RustSsmlVoiceChoice;
/* Clause input helpers are callback-free. Decoder/scalars are serialized and
 * disjoint, with retained input. Count overflow rejects before source advancement.
 * Word output has5 writable bytes; only prefix+NUL are written. Replacement
 * table contains initialized pairs through a zero key; output unchanged on error.
 * Legacy UTF8 reader requires directional storage through a non-continuation
 * head and up to3 following bytes or earlier NUL, disjoint from output. */
int32_t espeak_rs_clause_type(uint32_t);
int32_t espeak_rs_clause_properties(uint64_t);
int32_t espeak_rs_clause_roman(uint32_t);
int32_t espeak_rs_clause_phoneme_mode(int32_t,int32_t,int32_t,int32_t);
void espeak_rs_clause_word(unsigned char *,uint32_t);
int32_t espeak_rs_clause_replace(const uint16_t *,size_t,int32_t *);
int32_t espeak_rs_utf8_in2(int32_t *,const unsigned char *,int32_t);
int32_t espeak_rs_clause_eof(int32_t,espeak_ng_TEXT_DECODER *);
int32_t espeak_rs_clause_getc(int32_t *,int32_t *,espeak_ng_TEXT_DECODER *);
/* Punctuation backend/source callbacks retain their serialized owner and copy
 * names into initialized74-byte outputs (0 found,1 absent,other error). Context,
 * live flag/speed snapshots and mutable scalar/output storage are disjoint.
 * Callbacks cannot invalidate/reenter this output/announcement. Backend/source
 * effects can precede output rejection; emitted prefix+NUL is fully admitted. */
typedef struct {
    void *owner;int32_t (*icon)(int32_t);
    int32_t (*name)(void *,int32_t,uint32_t,unsigned char (*)[74]);
    int32_t (*eof)(void),(*read)(void);
    void (*unread)(int32_t),(*unread_second)(int32_t);
    const int32_t *flags,*speed;
} RustClausePunctuation;
int32_t espeak_rs_clause_announce(const RustClausePunctuation *,int32_t,int32_t *,unsigned char *,size_t,int32_t *,uint32_t);
/* Full owned-controller bridge. All scalar/active-frame fields and string
 * prefixes must be initialized; unused record/string tails may be undefined.
 * Context fields, mutable XML span, initialized output prefix and writable
 * capacity are disjoint and stay live through this serialized call. No callback
 * may invalidate/reenter tag/output/context storage. Host calls see copied
 * published state; refresh snapshots scalar/frame effects after callbacks.
 * Base metadata is copied before callbacks; original xmlbase must remain live
 * and cannot alias a growable name-arena entry. Existing separate-effect
 * rejection publishes earlier admitted effects and returns clause0. */
typedef struct {int32_t clause_pause,pause;} RustSsmlRate;
typedef struct {
    PARAM_STACK *parameters;int32_t *parameter_count;int32_t (*current)[15];
    SSML_STACK *voices;int32_t *voice_count;unsigned char *current_voice;
    unsigned char (*previous_identifier)[40];unsigned char *skip;
    int32_t *punctuation,*capitals;bool *audio,*ignore,*clear_skipping;
    int32_t *sayas_mode,*sayas_start;const espeak_VOICE *base_voice;
    const char *variant,*xmlbase;
    int (*wide_space)(uint32_t),(*byte_space)(uint32_t),(*lower)(uint32_t);
    int32_t (*append)(const char *,int32_t),(*load)(const char *);
    int (*uri)(int,const char *,const char *);
    void (*rate)(int32_t,RustSsmlRate *);
    int32_t (*resolve)(const unsigned char (*)[40],unsigned char (*)[40]);
    int32_t (*select)(const RustSsmlVoiceChoice *,unsigned char (*)[40]);
    uint32_t signed_bytes,decimal;int32_t tone;uint32_t sonic;
} RustSsmlContext;
int32_t espeak_rs_ssml_process(const RustSsmlContext *,wchar_t *,size_t,unsigned char *,size_t,int32_t *);
/* Resource request plans copy bounded attribute names into initialized owned
 *160-byte records; pure classifiers cannot mutate/invalidate/reenter the tag.
 * Marker action0 absent/1 clear awaited marker/2 append. File planning preserves
 * null/empty-base and absolute-path precedence, admits all256 bytes before any
 * file operation. Signal type1 mark/2 sound/3 URI; negative index emits nothing.
 * Audio effects request push, backend, merge, optional pop then text0 false/
 *1 true/2 preserve. Owner executes requests AFTER native borrows finish; URI
 * callbacks consume the copied request name, which remains live across arena
 * growth. Base strings must remain live across host calls. Nonzero leaves all
 * exclusive/disjoint outputs unchanged. Actual I/O/callback execution is host
 * work; these routines allocate nothing and perform no I/O or engine callbacks. */
typedef struct {int32_t kind;uint32_t present;unsigned char name[160];} RustSsmlResource;
typedef struct {uint32_t length;unsigned char bytes[256];} RustSsmlFile;
typedef struct {uint32_t length,silence;unsigned char bytes[16];} RustSsmlSignal;
typedef struct {uint32_t push,pop,text;int32_t terminator;} RustSsmlAudio;
int32_t espeak_rs_ssml_resource(int32_t,const wchar_t *,size_t,size_t,int (*)(uint32_t),int (*)(uint32_t),RustSsmlResource *);
int32_t espeak_rs_ssml_marker(const RustSsmlResource *,const char *,uint32_t *);
int32_t espeak_rs_ssml_file(const RustSsmlResource *,const char *,RustSsmlFile *);
int32_t espeak_rs_ssml_signal(uint32_t,int32_t,RustSsmlSignal *);
int32_t espeak_rs_ssml_audio(int32_t,uint32_t,RustSsmlAudio *);
/* Bounded marker/URI/wide name owner. Append consumes exactly one terminated
 * initialized opaque byte sequence, unit width1/2/4; source must be disjoint
 * from owner/backing storage and exclusive view output. Nonzero inputs beyond
 *128MiB reject; append -1 leaves prior entries/view untouched. Byte offsets
 * retain C behavior, including unaligned mixed wide/narrow entries; do not
 * cast those to aligned wide references. Views expire on growing append/reset/
 * destruction; serialize consumers and drain callbacks/events first. Reset
 * retains warmed allocation; destruction releases it. No callbacks or I/O. */
void *espeak_rs_names_create(size_t);
void espeak_rs_names_destroy(void *);
void espeak_rs_names_reset(void *);
int32_t espeak_rs_names_append(void *,const unsigned char *,size_t,size_t,const unsigned char **);
size_t espeak_rs_names_reserved(const void *);
/* Tag planning reads <=501 initialized immutable wide units, preserving legacy
 * byte narrowing and host casing. signed 0/1; pure locale classifiers cannot
 * mutate/invalidate/reenter. Output is exclusive/disjoint, unchanged on error.
 * Caller admits separation output before publishing slash replacement/offsets.
 * Directive plans style/prosody/emphasis frame from immutable tag, base/current
 * initialized15 values, tone snapshot and locale. Unknown emphasis rejects
 * before push; rejected individual prosody arithmetic keeps inherited -1.
 * No allocation, I/O, engine callbacks or accelerator work. */
typedef struct {int32_t kind;uint32_t attributes,separator,self_closing,ignore,slash_index;} RustSsmlTag;
/* Pause/voice plans have initialized exclusive disjoint outputs unchanged on
 * error. Tag classifier is pure and cannot mutate/invalidate/reenter. Pause
 * owner admits command capacity, emits command+NUL, then requests rate update
 * iff timed, snapshots resulting factors, and finishes native timing. No Rust
 * engine borrow crosses that host call. Voice planning reads ONLY initialized
 * tag_type fields in1..20 active records, not undefined property/string tails;
 * owner publishes count, executes tags[0..length] in order, ORs change flags,
 * and finishes the terminator. Frame-helper count remains passed by value. */
typedef struct {int32_t value,terminator;uint32_t timed;int32_t milliseconds,rate;uint32_t length;unsigned char command[4];} RustSsmlBreak;
typedef struct {uint32_t count,length;int32_t tags[3],terminator;uint32_t open;} RustSsmlVoiceClause;
int32_t espeak_rs_ssml_pause(const wchar_t *,size_t,size_t,int32_t,int32_t,int (*)(uint32_t),RustSsmlBreak *);
int32_t espeak_rs_ssml_pause_finish(const RustSsmlBreak *,int32_t,int32_t,uint32_t,int32_t *);
int32_t espeak_rs_ssml_voice_clause(int32_t,const SSML_STACK *,int32_t,RustSsmlVoiceClause *);
int32_t espeak_rs_ssml_voice_clause_finish(const RustSsmlVoiceClause *,int32_t,int32_t *);
/* Text dispatch borrows <=501 immutable initialized wide units, preceding unit
 * at start-1, and only initialized output prefix[0..state.offset]. Output has
 * exclusive writable capacity; unused tail may be uninitialized. Tag/state/
 * output are disjoint. Pure classifiers must not mutate/invalidate/reenter.
 * State ignore is0/1; offset/start/mode retain compatibility values. Nonzero
 * leaves output/state unchanged, including failed key-close old-end NUL writes.
 * Separator and self-close admission belong to the caller's tag dispatch. */
typedef struct {int32_t offset,mode,start;uint32_t ignore;} RustSsmlTextState;
int32_t espeak_rs_ssml_text(int32_t,const wchar_t *,size_t,size_t,unsigned char *,size_t,RustSsmlTextState *,int (*)(uint32_t),int (*)(uint32_t));
int32_t espeak_rs_ssml_tag(const wchar_t *,size_t,uint32_t,int (*)(uint32_t),int (*)(uint32_t),RustSsmlTag *);
int32_t espeak_rs_ssml_directive(int32_t,const wchar_t *,size_t,size_t,const int32_t (*)[15],const int32_t (*)[15],int32_t,uint32_t,int (*)(uint32_t),PARAM_STACK *);
/* Voice-frame planning borrows immutable tag span including initialized prior
 * unit at start-1. Pure locale classifiers cannot reenter/mutate/invalidate it.
 * Effects have action0 no change/1 close selection/2 install then select;
 * count is local compatibility count, not a host-count mutation. Inputs admit
 * complete prefix strings before output publication; count1..20, add needs<20.
 * Voice-changed borrows terminated inputs (source may alias current), admits
 * exclusive current capacity40, and writes only changed prefix+NUL, leaving
 * uninitialized unused tail alone; 1 changed/0 same/-1 invalid unchanged.
 * No allocation, I/O, engine callbacks or accelerator work. */
typedef struct {uint32_t action,count,index;SSML_STACK frame;} RustSsmlVoiceFrame;
int32_t espeak_rs_ssml_voice_frame(const wchar_t *,size_t,size_t,int32_t,int32_t,int (*)(uint32_t),int (*)(uint32_t),RustSsmlVoiceFrame *);
int32_t espeak_rs_ssml_voice_changed(unsigned char *,const char *);
/* Prosody uses <=513 initialized host-wide units (compatibility XML limit500),
 * locale decimal character and pure whitespace classifier. Decimal/hexadecimal
 * binary64 parsing, checked C-to-int range and ordered arithmetic; nonzero
 * leaves exclusive/disjoint outputs unchanged. Float returns 0 parsed/1 absent/
 * 2 invalid and publishes the double/tail only when parsed. No I/O/allocation/
 * callbacks other than pure classification, or accelerator work. */
typedef struct {int32_t kind,value;} RustSsmlProsody;
int32_t espeak_rs_ssml_float(const wchar_t *,size_t,uint32_t,int (*)(uint32_t),double *,size_t *);
int32_t espeak_rs_ssml_prosody(int32_t,const wchar_t *,size_t,uint32_t,int (*)(uint32_t),RustSsmlProsody *);
int32_t espeak_rs_ssml_prosody_parameter(int32_t,const wchar_t *,size_t,int32_t,int32_t,uint32_t,int (*)(uint32_t),int32_t *);
/* Voice choice reads 1..20 initialized immutable frames/base/prior identifier.
 * Name resolver returns 0 with a terminated <=39-byte copied identifier,
 * 1 unknown, other failure; no reentry or invalidation of input snapshots.
 * No exclusive engine/catalogue owner is held across callbacks. Effects are
 * exclusive/disjoint and published after ordered resolution finishes.
 * Base variant returns 1 copied/0 unnecessary/-1 invalid, output unchanged on
 * 0/-1. It borrows terminated strings, exclusive/disjoint fixed output.
 * No allocation, I/O or accelerator work in native composition. */
struct RustSsmlVoiceChoice {
	unsigned char name[40],identifier[40],language[40];
	uint32_t gender,age,variant;
};
int32_t espeak_rs_ssml_voice_choice(const SSML_STACK *,int32_t,const espeak_VOICE *,const unsigned char (*)[40],int32_t (*)(const unsigned char (*)[40],unsigned char (*)[40]),RustSsmlVoiceChoice *);
int32_t espeak_rs_ssml_base_variant(const char *,uint32_t,uint32_t,const char *,unsigned char (*)[40]);
/* Parameter planning borrows initialized count<=20 frames and 15 current
 * values; exclusive disjoint effects are published only after complete command
 * capacity admission. Unchanged output requires no capacity. Pop returns the
 * prospective count; C owner commits commands/options/values/count together.
 * Push exclusively borrows all 20 initialized frames plus disjoint count; the
 * saturated last slot is reset without increment, matching compatibility C.
 * No callbacks, I/O, allocations or accelerator work. */
typedef struct {
	int32_t values[15],punctuation,capitals;
	uint32_t length,changed,count;
	unsigned char commands[80];
} RustSsmlParameters;
int32_t espeak_rs_ssml_parameters(const PARAM_STACK *,int32_t,const int32_t (*)[15],int32_t,int32_t,uint32_t,int32_t,size_t,RustSsmlParameters *);
int32_t espeak_rs_ssml_push(PARAM_STACK *,int32_t *,int32_t);
/* SSML borrows initialized wide-unit spans of host wchar_t width. Attribute
 * names are terminated ASCII. Classifiers must be synchronous/pure locale
 * functions: no reentry, input mutation or invalidation. Outputs are exclusive
 * and disjoint; copy needs writable capacity but no initialized unused tail.
 * Copy returns length or -1 without writes; attribute 0 found/1 absent/2 invalid
 * and publishes an offset only on success (SIZE_MAX denotes a separate empty
 * value). Number failures return the default.
 * Reference retains scanf conversion counts and explicit first/second effects;
 * overflow rejects unchanged. Key replacement writes only the admitted prefix.
 * No I/O, allocation or accelerator work; execute on owner/worker. */
int espeak_rs_ssml_compare(const wchar_t *,size_t,const char *);
int32_t espeak_rs_ssml_lookup(const wchar_t *,size_t,const MNEM_TAB *);
int32_t espeak_rs_ssml_number(const wchar_t *,size_t,int32_t,int32_t);
int espeak_rs_ssml_attribute(const wchar_t *,size_t,size_t,const char *,int (*)(uint32_t),size_t *);
int32_t espeak_rs_ssml_copy(const wchar_t *,size_t,uint32_t,int (*)(uint32_t),unsigned char *,size_t);
int32_t espeak_rs_ssml_reference(const char *,int32_t *,int32_t *,int (*)(uint32_t));
int32_t espeak_rs_ssml_key(unsigned char *,int32_t,int32_t *);
/* Native synthesis calibration. Shared initialized voice/embedded snapshots,
 * exclusive initialized/disjoint outputs; formants require exclusive voice.
 * Checked intermediate arithmetic and indices, nonzero preserves outputs.
 * MBROLA pitch borrows 128 envelope bytes and writes only admitted text+NUL.
 * Scale borrows exclusively initialized 16-bit LE PCM after backend read ends;
 * unusual amplitudes are validated before writes. No callbacks, allocations,
 * I/O or accelerator work. Calls belong to the synthesis owner/worker. */
typedef struct {int32_t base,range;} RustPitch;
typedef struct {int32_t pitch,tone,range;} RustEmbeddedPitch;
typedef struct {int32_t increment,value,voiced;} RustAmplitude;
int espeak_rs_pitch(const voice_t *,int32_t,int32_t,const RustEmbeddedPitch *,RustPitch *);
int espeak_rs_pitch_formants(voice_t *,int32_t,int32_t);
int espeak_rs_general_amplitude(int32_t,int32_t,int32_t *);
int espeak_rs_amplitude(int32_t,int32_t,int32_t,int32_t,RustAmplitude *);
int espeak_rs_mbrola_pitch(const unsigned char (*)[128],int32_t,const RustPitch *,int32_t,uint32_t,unsigned char *,size_t);
int espeak_rs_mbrola_scale(unsigned char *,size_t,int32_t);
/* Native MBROLA owner: <=128 MiB combined reserved active/scratch mapping
 * buffers, little-endian validation and reusable chunked reads. File loading is
 * initialization/worker work; resident bytes use the safe API after completion.
 * Serialized unique owner for load/destroy; drain immutable views before either.
 * Paths and exclusive initialized outputs are disjoint. Load returns 0 success,
 * 1 I/O (C errno), 2 malformed, 3 memory, 4 capacity; failures preserve active
 * table/control. Selection accepts optional owner/neighbors and uses only shared
 * initialized inputs, disjoint output; prefix is an explicit ordered effect.
 * Selection has no callbacks, I/O, allocation or process operations. */
void *espeak_rs_mbrola_create(void);
void espeak_rs_mbrola_destroy(void *);
int espeak_rs_mbrola_load(void *,const char *,uint32_t *,int32_t *);
int espeak_rs_mbrola_view(const void *,const MBROLA_TAB **,size_t *,uint32_t *);
typedef struct {uint32_t word_start,next_word_start,synth_flags,stress,word_stress;int32_t prefix;} RustMbrolaContext;
typedef struct {int32_t name,second,percent,control,prefix;} RustMbrolaSelection;
int espeak_rs_mbrola_select(const void *,const PHONEME_TAB *,const PHONEME_TAB *,const PHONEME_TAB *,const PHONEME_TAB *,const RustMbrolaContext *,RustMbrolaSelection *);
/* Output has room for the encoded 1..4 bytes; retains legacy code-unit and
 * out-of-range handling. Writes no terminator or bytes beyond that length. */
int espeak_rs_utf8_out(uint32_t,unsigned char *);
/* Sound icon owner: 80 entries, <=128 MiB reserved aligned WAV bytes, reusable
 * storage, bounded filenames. Warm nonempty sounds retain stable addresses and
 * avoid I/O. Calls require serialized live owner, terminated shared paths and
 * initialized exclusive/disjoint 80-entry view and count outputs. Configure also
 * requires exclusive points[12], width 2..4096 and signed-character 0/1. Lookup
 * selects filename, or character if filename=NULL; separator is one byte.
 * Lookup snapshots filename before owner mutation, permitting a published-name
 * alias. Root/config paths and all outputs must be disjoint from the owner.
 * Configure returns 0/1 and publishes a valid completed prefix; lookup returns
 * index or -1 and publishes retained views. Filesystem/parsing work belongs to
 * initialization/the caller worker, never a proactor completion. Do not free or
 * mutate borrowed names/WAV bytes; drain all views/PCM before owner destruction. */
void *espeak_rs_soundicons_create(void);
void espeak_rs_soundicons_destroy(void *);
int espeak_rs_soundicons_configure(void *,const char *,int32_t (*)[12],size_t,uint32_t,SOUND_ICON [80],int *);
int espeak_rs_soundicons_lookup(void *,const char *,const char *,int32_t,int32_t,uint32_t,size_t,SOUND_ICON [80],int *);
typedef struct { uint32_t count; int32_t values[2]; } RustSonicEffects;
/* Initialized exclusive speed/three-length/effect outputs are mutually disjoint
 * and disjoint from the shared voice. Control 0..3, sonic 0/1. Returns 0 after
 * transactional commit; invalid arguments/arithmetic leave all outputs intact.
 * Deliver effects.values[0..count] in order after commit on the serialized owner.
 * No allocation, I/O, global state or callbacks in native computation. */
int espeak_rs_speed_configure(const voice_t *, SPEED_FACTORS *, int32_t (*)[3],
                             int32_t, int32_t, uint32_t, uint32_t, RustSonicEffects *);
/* Core asset owner: four reusable u64-aligned buffers, combined reserved capacity
 * bounded to 128 MiB. Slot 0 phontab, 1 phonindex, 2 phondata, 3 intonations.
 * Views expire on replacement/destroy; drain/serialize all consumers first.
 * Synchronous initialization/worker I/O only. Retain a terminated compatibility
 * path and initialized exclusive/disjoint owner/output storage. Outputs describe
 * the retained slot even on failure: admission/open errors keep earlier bytes;
 * started reads that fail clear that slot while keeping capacity.
 * Result 0 success, 1 missing, 2 permission, 3 allocation, 4 directory, 5 invalid,
 * 6 short read, 7 other I/O. Error output is Unix errno or zero on other hosts.
 * Invalid arguments leave outputs untouched. Never free borrowed asset views. */
void *espeak_rs_core_create(void);
void espeak_rs_core_destroy(void *);
uint32_t espeak_rs_core_load(void *,uint32_t,const char *,unsigned char **,int32_t *,int32_t *);

typedef struct {
    unsigned char name[80],gender_name[80],languages[300];
    uint32_t language_length,language_count;
    int32_t age,variants;
} RustVoiceMetadata;
/* Initialize to zero with variants=4. Each terminated fgets chunk is borrowed
 * and disjoint from exclusive metadata. Status 0 applied, 1 gender applied,
 * 2 rejected without mutation. Exact language length excludes final sentinel. */
int espeak_rs_voice_metadata_line(RustVoiceMetadata *,const char *);
int espeak_rs_voice_metadata_gender(const RustVoiceMetadata *);
/* Retain initialized voice records, terminated names and priority/name lists.
 * The optional selector name and exact language span are borrowed. Ranking
 * does no allocation or I/O; unsupported malformed inputs score zero. */
int espeak_rs_voice_score(const espeak_VOICE *,const char *,int32_t,size_t,const espeak_VOICE *);
/* Retain at most 499 initialized entries followed by NULL. Returns a borrowed
 * entry, preferring visible names, exact IDs, then final path components. */
espeak_VOICE *espeak_rs_voice_by_name(espeak_VOICE *const *,const char *,uint8_t);
/* Optional terminated name disjoint from two exclusive outputs. Failure
 * leaves both outputs untouched; suffix includes a terminating NUL. */
int espeak_rs_voice_variant(const char *,int32_t,uint32_t,uint8_t,size_t *,unsigned char (*)[40]);
/* One serialized workspace per catalogue; allocate during setup, destroy once
 * after calls drain. Ranking/selection reuse bounded storage without allocation. */
void *espeak_rs_voice_workspace_create(size_t);
void espeak_rs_voice_workspace_destroy(void *);
int espeak_rs_voice_filter(const char *,uint32_t,uint8_t,unsigned char (*)[80],int32_t *);
/* Output capacity includes NULL and exceeds roster length. Output pointers must
 * not alias the retained input pointer array. Selector must be disjoint from
 * records whose score fields are updated. Status is count or -1 rejection. */
int espeak_rs_voice_rank(void *,const espeak_VOICE *,espeak_VOICE *const *,espeak_VOICE **,size_t,uint32_t,uint32_t,uint8_t);
typedef struct {size_t index;uint32_t found;unsigned char suffix[40];} RustVoiceSelection;
/* Callback borrows normalized directory bytes; no reentrant selection. Returns
 * 0 selected, 1 no selection (output initialized), 2 rejected (output untouched).
 * Selector/output are disjoint from records whose scores are committed. */
int espeak_rs_voice_select(void *,const espeak_VOICE *,espeak_VOICE *const *,uint8_t,void *,uint32_t (*)(void *,const unsigned char *,size_t),RustVoiceSelection *);
/* Every admitted record has a terminated primary name after its priority byte,
 * including a zero priority. Only the exclusive pointer array is reordered. */
int espeak_rs_voice_order(espeak_VOICE **,size_t);
/* Initialization/offload only: no host handle is available in this compatibility
 * path, so directory/file reads are synchronous. At least 499 exclusive pointer
 * slots and count, disjoint from terminated root. The returned owner retains all
 * records/strings until destroy; never individually free these pointers. Callback
 * borrows identifier bytes synchronously and must not reenter catalogue mutation.
 * Failure leaves outputs unchanged. Windows paths use its active code page,
 * matching the legacy ANSI file/directory APIs. */
void *espeak_rs_voice_catalog_create(const char *,espeak_VOICE **,size_t,int *,void *,void (*)(void *,uint32_t,const unsigned char *,size_t));
void espeak_rs_voice_catalog_destroy(void *);
/* Borrowed workspace/result storage owned by the catalogue; never destroy or
 * free separately. List calls reuse the result buffer and retain score effects.
 * Serialized calls only; selectors/strings are disjoint from writable output
 * and score fields. Directory callbacks borrow bytes and must not reenter. */
void *espeak_rs_voice_catalog_workspace(void *);
espeak_VOICE **espeak_rs_voice_catalog_list(void *,const espeak_VOICE *,uint8_t,void *,uint32_t (*)(void *,const unsigned char *,size_t));
typedef struct {
    unsigned char translator[40],dictionary[40],phonemes[40],name[40],language[20],languages[100];
    uint32_t language_length,language_set,phonemes_set,tone_only;
    unsigned char gender,age;
} RustVoiceSetup;
typedef struct {unsigned char identifier[40],name[40],languages[100];} RustCurrentVoice;
/* One per serialized compatibility engine, allocated during setup and destroyed
 * at termination after borrowed API strings drain. Fields remain at stable
 * addresses; compatibility commits initialized metadata between native calls.
 * Preparation snapshots an aliased request before owner mutation; exclusive
 * initialized output/setup is disjoint from owner. Failure preserves both. */
RustCurrentVoice *espeak_rs_current_voice_create(void);
void espeak_rs_current_voice_destroy(RustCurrentVoice *);
int espeak_rs_current_voice_prepare(RustCurrentVoice *,const char *,const char *,uint32_t,uint8_t,uint8_t,const unsigned char (*)[20],RustVoiceSetup *);
/* Exclusive initialized setup/effect disjoint from terminated key/value. Return
 * 0 handled, 1 another layer, 2 rejected. Effect 1 selects translator/table;
 * effect 2 selects the first replacement table; effect 0 commits metadata only.
 * Failure leaves setup/effect unchanged. */
int espeak_rs_voice_setup_attribute(RustVoiceSetup *,const char *,const char *,uint32_t *);
typedef struct { unsigned char path[4096],name[40];uint32_t control; } RustVoiceRequest;
/* Request preparation is initialization/worker work. Owner probes path lengths
 * synchronously, retaining callback/opaque. Request output is initialized and
 * disjoint from terminated inputs. Callback paths are borrowed and terminated.
 * Return 0 prepared, 1 no request, 2 rejected; failure preserves output.
 * Fallback flags describe owner open/table-selection results; same statuses. */
int espeak_rs_voice_request(const char *,const char *,uint32_t,uint8_t,size_t,void *,int64_t (*)(void *,const unsigned char *,size_t),RustVoiceRequest *);
int espeak_rs_voice_fallback(const RustVoiceRequest *,uint32_t,uint32_t,const char *,unsigned char (*)[40]);
/* Exclusive initialized identifier, terminated request may alias that identifier.
 * Both are copied before mutation; rejected variants preserve the identifier. */
int espeak_rs_voice_identifier(unsigned char (*)[40],const char *,uint32_t);
/* Setup/worker-only synchronous native file reader; no host is supplied by the
 * compatibility API. Windows paths use the active ANSI code page and files use
 * Windows text conversion. One owner retains reusable 8 KiB input/4 KiB line
 * buffers. Borrowed terminated key/value survive until next read/close and must
 * not be changed. Exclusive pointer outputs are disjoint from the reader.
 * Open returns NULL on failure. Next returns 0 directive, 1 EOF, 2 error; failure
 * leaves outputs untouched. Close once after serialized reads drain. */
void *espeak_rs_voice_file_open(const char *,size_t);
int espeak_rs_voice_file_next(void *,const char **,const char **);
void espeak_rs_voice_file_close(void *);
/* Ordered native loop/finalization; initialized exclusive snapshots and optional
 * reader are disjoint from opaque owner state. Callback kinds: 1 directive,
 * 2 rejected directive, 3 ensure translator, 4 select table, 5 unknown table,
 * 6 set table index, 7 load dictionary. Kind 1 receives writable acoustic/fast
 * snapshots and returns nonzero on success; kind 4 returns table index, kind 7
 * nonzero on success. Other callback results are ignored. Snapshot/terminated
 * input pointers are borrowed only for the callback and must never be retained.
 * No reentry, synchronous initialization/worker work only. Return 0 configured,
 * 1 backend/dictionary failed, 2 rejected; no host proactor callbacks are invoked. */
typedef struct RustVoiceAction RustVoiceAction;
typedef int32_t (*RustVoiceLoadHost)(void *,uint32_t,const RustVoiceSetup *,const char *,const char *,const RustVoiceAction *,voice_t *,int32_t *);
int espeak_rs_voice_configure(void *,RustVoiceSetup *,voice_t *,int32_t *,uint32_t,uint32_t,void *,RustVoiceLoadHost);
/* Retained sparse selected table (at most 256 initialized slots/records).
 * Storage/count are exclusive and disjoint from input/table. Failed directives
 * preserve outputs. Setup effects: 0=none, 1=language, 2=first replacement table. */
uint32_t espeak_rs_phoneme_code(const PHONEME_TAB *const *,size_t,uint32_t);
/* Bounded owned word-stress planning; 256 initialized sparse pointer slots and
 * retained immutable records. Initialized input prefix contains NUL (<=200).
 * Word has up to 200 writable bytes for assignment; extraction only shrinks.
 * Stress has 100 writable signed bytes. Outputs/settings/table are disjoint;
 * no callbacks, I/O or allocation. Return zero committed, nonzero unchanged.
 * Only completed output prefixes are written, preserving uninitialized tails. */
typedef struct { uint32_t language,flags;int32_t rule,unstressed_one,unstressed_many,vowel_pause,lengthen,previous; } RustWordStress;
int espeak_rs_vowel_stress(unsigned char *,size_t,const PHONEME_TAB *const *,uint32_t,uint32_t,signed char *,int32_t *,int32_t *,int32_t *);
int espeak_rs_word_stress(unsigned char *,size_t,const PHONEME_TAB *const *,size_t,const RustWordStress *,const uint32_t *,int32_t,uint32_t,int32_t *);
/* Same selected-table/input lifetime contract; stress change needs 200 writable
 * word bytes. Append admits full tail before publication; counts initialized
 * exclusive/disjoint. Addition includes NUL and is disjoint from output capacity.
 * Attribute transform mutates only the initialized terminated word prefix.
 * No callbacks, I/O or allocations; nonzero preserves word/count effects. */
int espeak_rs_change_stress(unsigned char *,size_t,const PHONEME_TAB *const *,uint32_t,int32_t);
typedef struct {int32_t vowels,stressed;} RustWordCounts;
int espeak_rs_append_phonemes(unsigned char *,size_t,size_t,const unsigned char *,size_t,const PHONEME_TAB *const *,size_t,RustWordCounts *);
int espeak_rs_special_attribute(unsigned char *,size_t,const PHONEME_TAB *const *,size_t,int32_t,uint32_t,uint32_t);
uint32_t espeak_rs_phoneme_mnemonic(const char *);
int espeak_rs_voice_replacement(const char *,const PHONEME_TAB *const *,size_t,REPLACE_PHONEMES *,int *);
typedef struct { unsigned char voice[40],table[80];int32_t sample_rate; } RustMbrolaRequest;
int espeak_rs_mbrola_request(const char *,RustMbrolaRequest *);
struct RustVoiceAction {uint32_t action,argument;RustMbrolaRequest backend;};
/* Ordered dispatch: 1 language option, 2 acoustic/speed intent, 3 metadata/table
 * effect, 4 replacement/table effect, 5 backend request, 6/7 unavailable MBROLA/
 * Klatt, 0 unknown. Features bit0 Klatt, bit1 MBROLA. Initialized exclusive
 * snapshots/output are disjoint from terminated inputs. No callbacks, I/O or
 * allocation. Return 0 dispatched, 2 rejected with snapshots/output unchanged. */
int espeak_rs_voice_directive(voice_t *,RustVoiceSetup *,int32_t *,uint32_t,const char *,const char *,RustVoiceAction *);

typedef struct {
    int32_t dictionary_minimum; uint32_t dictionary_conditions; int32_t tone_flags;
    int16_t stress_lengths[8]; uint8_t stress_amplitudes[8];
    int32_t word_gap, vowel_pause, stress_rule; uint32_t stress_flags;
    int32_t unstressed_single, unstressed_multiple, parameters[18];
    uint32_t numbers, numbers2;
    int32_t thousands_separator, decimal_separator, intonation_group;
    uint8_t tunes[6], lowercase_sentence, spelling_stress;
} RustLanguageOptions;
/* Callback 0 looks up a borrowed tune name, 1 reports an invalid ordinal,
 * 2 reports a borrowed unknown tune. Serialize setup; no input may alias the
 * initialized exclusive options snapshot. Failed parse leaves it unchanged. */
typedef int32_t (*RustLanguageCallback)(void *, uint32_t, uint32_t, const unsigned char *, size_t, int32_t);
int espeak_rs_language_option(RustLanguageOptions *, uint32_t, const char *, void *, RustLanguageCallback);
int espeak_rs_language_ordinals(const char *, uint32_t *, int32_t, uint32_t, void *, RustLanguageCallback);
void espeak_rs_language_separators(uint32_t, int32_t *, int32_t *);
static inline void espeak_rust_language_capture(const Translator *tr, int tone, RustLanguageOptions *out)
{
    const LANGUAGE_OPTIONS *o=&tr->langopts;
    *out=(RustLanguageOptions){.dictionary_minimum=tr->dict_min_size,.dictionary_conditions=tr->dict_condition,
        .tone_flags=tone,.word_gap=o->word_gap,.vowel_pause=o->vowel_pause,.stress_rule=o->stress_rule,
        .stress_flags=o->stress_flags,.unstressed_single=o->unstressed_wd1,.unstressed_multiple=o->unstressed_wd2,
        .numbers=o->numbers,.numbers2=o->numbers2,.thousands_separator=o->thousands_sep,.decimal_separator=o->decimal_sep,
        .intonation_group=o->intonation_group,.lowercase_sentence=o->lowercase_sentence,.spelling_stress=o->spelling_stress};
    memcpy(out->stress_lengths,tr->stress_lengths,sizeof(out->stress_lengths));
    memcpy(out->stress_amplitudes,tr->stress_amps,sizeof(out->stress_amplitudes));
    memcpy(out->parameters,o->param,sizeof(out->parameters));
    memcpy(out->tunes,o->tunes,sizeof(out->tunes));
}
static inline void espeak_rust_language_commit(Translator *tr, const RustLanguageOptions *in)
{
    LANGUAGE_OPTIONS *o=&tr->langopts;
    tr->dict_min_size=in->dictionary_minimum; tr->dict_condition=in->dictionary_conditions;
    o->word_gap=in->word_gap; o->vowel_pause=in->vowel_pause; o->stress_rule=in->stress_rule;
    o->stress_flags=in->stress_flags; o->unstressed_wd1=in->unstressed_single; o->unstressed_wd2=in->unstressed_multiple;
    o->numbers=in->numbers; o->numbers2=in->numbers2; o->thousands_sep=in->thousands_separator;
    o->decimal_sep=in->decimal_separator; o->intonation_group=in->intonation_group;
    o->lowercase_sentence=in->lowercase_sentence!=0; o->spelling_stress=in->spelling_stress!=0;
    memcpy(tr->stress_lengths,in->stress_lengths,sizeof(in->stress_lengths));
    memcpy(tr->stress_amps,in->stress_amplitudes,sizeof(in->stress_amplitudes));
    memcpy(o->param,in->parameters,sizeof(in->parameters)); memcpy(o->tunes,in->tunes,sizeof(in->tunes));
}

typedef struct {
    uint32_t break_numbers;
    int32_t max_roman,min_roman,max_digits,accents,tone_language,long_stop,max_initial_consonants;
    int32_t tone_numbers,ideographs,textmode,dotless_i,listx,our_alphabet,alt_alphabet,alt_alphabet_lang;
    int32_t max_lengthmod,lengthen_tonic,suffix_add_e,transpose_min,transpose_max,encoding,letter_bits_offset;
} RustLanguageSettings;
typedef struct {
    RustLanguageOptions options;
    RustLanguageSettings settings;
    uint32_t selector;
    unsigned char dictionary[40];
    const unsigned char *bits,*tones,*transpose_map;
    const short *pairs;
    const unsigned char *lengths,*last_lengths;
    const wchar_t *apostrophe,*punctuation;
    const unsigned short *ignored;
    const wchar_t *groups[8];
    size_t group_lengths[8];
    const unsigned char *ordinal,*roman;
} RustLanguageSetup;
/* Exclusive aligned output disjoint from terminated name. Returned table
 * pointers are immutable and live for the process; no allocation or I/O.
 * Failed setup leaves output untouched. Name length is limited to 39 bytes. */
int espeak_rs_language_setup(const char *,RustLanguageSetup *);
int espeak_rs_alphabet_index(int32_t);
static inline void espeak_rust_language_setup_commit(Translator *tr,const RustLanguageSetup *s)
{
    espeak_rust_language_commit(tr,&s->options);
    LANGUAGE_OPTIONS *o=&tr->langopts;
    const RustLanguageSettings *f=&s->settings;
    o->break_numbers=f->break_numbers;o->max_roman=f->max_roman;o->min_roman=f->min_roman;
    o->max_digits=f->max_digits;o->accents=f->accents;o->tone_language=f->tone_language;o->long_stop=f->long_stop;
    o->max_initial_consonants=f->max_initial_consonants;o->tone_numbers=f->tone_numbers;o->ideographs=f->ideographs;
    o->textmode=f->textmode!=0;o->dotless_i=f->dotless_i;o->listx=f->listx;o->our_alphabet=f->our_alphabet;
    o->alt_alphabet=f->alt_alphabet;o->alt_alphabet_lang=f->alt_alphabet_lang;o->max_lengthmod=f->max_lengthmod;
    o->lengthen_tonic=f->lengthen_tonic;o->suffix_add_e=f->suffix_add_e;
    o->length_mods=s->lengths;o->length_mods0=s->last_lengths;o->ordinal_indicator=(const char*)s->ordinal;
    o->roman_suffix=s->roman;o->replace_chars=NULL;
    tr->translator_name=s->selector;tr->transpose_min=f->transpose_min;tr->transpose_max=f->transpose_max;
    tr->transpose_map=(const char*)s->transpose_map;tr->frequent_pairs=s->pairs;tr->encoding=f->encoding;
    tr->letter_bits_offset=f->letter_bits_offset;tr->char_plus_apostrophe=s->apostrophe;
    tr->punct_within_word=s->punctuation;tr->chars_ignore=s->ignored;
    memcpy(tr->dictionary_name,s->dictionary,sizeof(tr->dictionary_name));
    memcpy(tr->letter_bits,s->bits,sizeof(tr->letter_bits));memcpy(tr->punct_to_tone,s->tones,sizeof(tr->punct_to_tone));
    memcpy(tr->letter_groups,s->groups,sizeof(tr->letter_groups));memcpy(tr->letter_group_lengths,s->group_lengths,sizeof(tr->letter_group_lengths));
}

/* Aligned initialized voice; disjoint exclusive points/rates/fast outputs.
 * Acoustic defaults only: caller retains backend and language reset effects. */
int espeak_rs_voice_reset(voice_t *, int32_t, int32_t [12], int32_t [9], int32_t *);
/* Terminated borrowed keyword/value; status 0 handled, 1 other setup layer,
 * 2 rejected. Speed output is written only for handled attributes. */
int espeak_rs_voice_attribute(voice_t *, const char *, const char *, uint32_t, int32_t *, uint32_t *);

typedef struct { int32_t which; uint32_t klatt; int32_t formant_factor; uint32_t other_glottal; int32_t length_adjust; } RustFormantSettings;
typedef struct { int32_t length_adjust, modulation; uint32_t has_modulation, pause; int32_t return_length; } RustFormantEffects;
/* Storage kind 0 admits a full writable frame; 1 returns the handle iff it
 * belongs to the writable owner pool. Serialize queue/pool use and retain
 * all input handles for the call. Records are short-aligned and readable
 * for their ordinary/Klatt size; pool records are initialized full frames. */
frame_t *espeak_rs_frame_copy(frame_t *, uint32_t, void *, frame_t *(*)(void *, uint32_t, frame_t *));
int espeak_rs_formant_transition(frameref_t *, size_t, int *, uint32_t, uint32_t, const RustFormantSettings *,
    void *, frame_t *(*)(void *, uint32_t, frame_t *), RustFormantEffects *);
/* Exclusive initialized four-word ring, disjoint start and six-rate snapshot;
 * retain all frame handles. Bounds/arithmetic are checked before mutations. */
int espeak_rs_smooth_spectrum(intptr_t (*)[4], size_t, int *, int, int, const int32_t [6],
    void *, frame_t *(*)(void *, uint32_t, frame_t *));

typedef struct { int32_t which; uint32_t is_vowel, lengthened; int32_t lengthen_length; } RustSpectrumSettings;
typedef struct { size_t start, count; int32_t length_adjust; } RustSpectrumSelection;
/* Retain immutable, short-aligned phondata and exclusive initialized 25-entry output.
 * Transition may change refs/host-pool frames, never resident bytes; capacity
 * is supplied explicitly. Discard partial output on nonzero status. */
int espeak_rs_spectrum_lookup(const unsigned char *, size_t, const FMT_PARAMS *, const RustSpectrumSettings *,
    void *, int (*)(void *, frameref_t *, int *, const FMT_PARAMS *, int, int *, size_t),
    frameref_t [N_SEQ_FRAMES], RustSpectrumSelection *);
const unsigned char *espeak_rs_envelope(const unsigned char *, size_t, int32_t);

/* Borrow little-endian phonindex and an immutable phoneme for this call;
 * output is exclusive and disjoint from all callback state. Callback kinds:
 * 0 condition at word offset, 1 stress, 2 next vowel, 3 next start type,
 * 4 previous end type, 5 invalid instruction. Negative type means missing. */
int espeak_rs_phoneme_program(const unsigned char *, size_t, const PHONEME_TAB *, uint32_t, uint32_t,
    void *, int (*)(void *, uint32_t, size_t), PHONEME_DATA *);
typedef struct {
	size_t length, current;
	uint32_t control, has_translator;
	int32_t reduction;
	uint32_t klatt, mbrola;
} RustPhonemeSettings;
typedef struct {
	PHONEME_TAB phoneme;
	uint32_t present, code, stress, word_stress, source, flags;
} RustPhonemeEntry;
/* Storage kinds: 0 bounded list read, 1 table read, 2 list refresh,
 * 3 previous-vowel read, 4 previous-vowel refresh, 5 diagnostic. */
int espeak_rs_phoneme_program_with_context(const unsigned char *, size_t, const PHONEME_TAB *, const RustPhonemeSettings *,
    void *, int (*)(void *, uint32_t, size_t, RustPhonemeEntry *), PHONEME_DATA *);
int espeak_rs_phoneme_condition(const RustPhonemeSettings *, uint32_t, int32_t,
    void *, int (*)(void *, uint32_t, size_t, RustPhonemeEntry *));

typedef struct {
	size_t singles[256], offsets[128], pairs[120];
	uint32_t pair_names[120];
	size_t pair_count;
	unsigned char pair_starts[256], pair_counts[256];
	size_t letters[95], replacements;
} RustRuleIndex;

typedef struct {
	char name[32];
	size_t records_offset;
	uint32_t count, includes;
} RustTableMeta;

/* Inputs must remain readable for the duration of the call. Output arrays are
 * exclusive and sized exactly as declared. Indices own metadata, not bytes. */
int espeak_rs_dictionary_index(const unsigned char *, size_t, RustRuleIndex *, size_t [1024], size_t *);
/* Serialized dictionary cache: 128 cached/128 retired snapshots, at most 128 MiB
 * of reserved aligned bytes including reusable fresh-read scratch and pinned old
 * versions. Each load rereads bytes; unchanged data shares cached indices/views.
 * Setup/worker-only I/O, no completion work or callbacks. Initialized exclusive
 * handle output is disjoint from cache/path; failure preserves it. Return 0 load,
 * 1 read/empty, 2 malformed, 3 allocation, 4 capacity/backpressure. Every successful
 * handle is owned; drop it after its immutable views drain. View outputs are
 * exclusive/disjoint, initialized, and include 1024 bucket slots. Cache/handles
 * may be released independently; bytes survive until the last handle/cache owner
 * drains. Never free or mutate borrowed dictionary bytes. */
void *espeak_rs_dictionary_cache_create(void);
void espeak_rs_dictionary_cache_destroy(void *);
int espeak_rs_dictionary_cache_load(void *,const char *,void **);
void espeak_rs_dictionary_handle_destroy(void *);
int espeak_rs_dictionary_handle_view(const void *,const unsigned char **,size_t *,RustRuleIndex *,size_t [1024],size_t *);
void *espeak_rs_phontab_create(const unsigned char *, size_t, RustTableMeta [150], int *);
void espeak_rs_phontab_destroy(void *);
int espeak_rs_phontab_select(const void *, const unsigned char *, size_t, int, size_t [256]);
int espeak_rs_phontab_lookup(const void *, const char *);
int espeak_rs_sample_rate(const unsigned char *, size_t);
int espeak_rs_phondata_header(const unsigned char *, size_t, uint32_t [2]);
typedef struct {
	uint32_t conditions, end_flags, word_flags, lookup_symbol, language, previous_flags;
	int32_t expect_verb, expect_verb_s, expect_past, expect_noun;
	uint32_t native_translator, sentence, single_symbol;
	size_t clause_remaining;
} RustLookupContext;
typedef struct { uint32_t flags, length; } RustWordInfo;
typedef struct {
	size_t phonemes_offset, phonemes_length, word_end;
	uint32_t flags[2], trace_flags[2], copied, has_flags, found;
	int32_t skipwords;
} RustLookupOutcome;
int espeak_rs_transpose(unsigned char *, size_t, uint32_t, uint32_t, const unsigned char *, size_t, const int16_t *, size_t);
int espeak_rs_lookup_bucket(const unsigned char *, size_t, const char *, size_t, const char *, size_t, const RustLookupContext *, const RustWordInfo *, size_t, RustLookupOutcome *);
typedef struct {
	uint32_t conditions, word_flags, dictionary_flags;
	int32_t vowel_count, stressed_count, expect_verb, tone_numbers, suffix_options;
	uint32_t trace, word_start, signed_bytes;
} RustMatchContext;
typedef struct {
	size_t phonemes, delete_offset, advance;
	int32_t points, ending;
} RustRuleMatch;
typedef struct {
	const unsigned char *bits;
	const void *const *groups;
	const size_t *lengths;
	int32_t offset;
	uint32_t wide_bytes;
} RustLetters;
typedef struct {
	uint32_t language; int32_t added_character,expect_verb; uint32_t signed_bytes;
	unsigned char preceding[4];
} RustSuffixContext;
typedef struct { uint32_t flags; int32_t expect_verb; uint32_t added,preceding; } RustSuffixEffects;
/* Word is an initialized exclusive writable span with a space delimiter and
 * initialized trailing capacity for repair. Shared context/letters and exclusive
 * effects/optional 160-byte copy are all disjoint. Failure preserves outputs and
 * word. Preceding[0..3] is actual initialized history, or standalone spaces.
 * Effects.preceding==0 keeps history; otherwise owner writes value-1 to word[-1].
 * Apply owner effects and trace after successful commit. */
int espeak_rs_remove_ending(unsigned char *,size_t,uint32_t,const RustSuffixContext *,
                           const RustLetters *,unsigned char *,RustSuffixEffects *);
/* Borrow 256 bitfield bytes, eight group pointers and eight cached lengths.
 * Non-null lists contain length wchar_t units (2 or 4 bytes), excluding NUL.
 * All configuration stays immutable during the call and its callbacks. */
int espeak_rs_is_letter(const RustLetters *, int32_t, uint32_t);
int espeak_rs_letter_group(const unsigned char *, size_t, const unsigned char *, size_t, size_t, int);
int espeak_rs_match_group(const unsigned char *, size_t, const unsigned char *, size_t, size_t, size_t,
    const RustMatchContext *, const RustLetters *, void *,
    int (*)(void *, uint32_t, uint32_t, size_t, uint32_t),
    void (*)(void *, const unsigned char *, size_t, uint32_t [2]),
    void (*)(void *, size_t, size_t, int32_t), RustRuleMatch *);
#endif
