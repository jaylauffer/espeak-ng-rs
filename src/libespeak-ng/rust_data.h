/* Native Rust migration boundary. Offsets refer to validated resident bytes.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_DATA_H
#define ESPEAK_RUST_DATA_H
#include <stddef.h>
#include <stdint.h>
#include "phoneme.h"
#include "synthesize.h"
#include "voice.h"
#include <string.h>

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
