/*
 * Copyright (C) 2005 to 2015 by Jonathan Duddington
 * email: jonsd@users.sourceforge.net
 * Copyright (C) 2015-2018 Reece H. Dunn
 * Copyright (C) 2018 Juho Hiltunen
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, see: <http://www.gnu.org/licenses/>.
 */

#ifndef ESPEAK_NG_DICTIONARY_H
#define ESPEAK_NG_DICTIONARY_H

#include "espeak-ng/espeak_ng.h"       // for ESPEAK_NG_API
#include "phoneme.h"                   // for PHONEME_TAB
#include "synthesize.h"                // for PHONEME_LIST
#include "translate.h"                 // for Translator, WORD_TAB

#ifdef __cplusplus
extern "C"
{
#endif

extern const char stress_phonemes[];

typedef struct {
	int points;
	const char *phonemes;
	int end_type;
	char *del_fwd;
} MatchRecord;
/* Internal Rust-core matcher; text_length includes the preceding byte and NUL. */
void espeak_rs_match_rule(Translator *, char **, char *, int, char *, MatchRecord *, int, int, size_t);

int LoadDictionary(Translator *tr, const char *name, int no_error);
void FreeDictionaryCache(void);
int HashDictionary(const char *string);
const char *EncodePhonemes(const char *p, char *outptr, int *bad_phoneme);
void DecodePhonemes(const char *inptr, char *outptr);
char *WritePhMnemonic(char *phon_out, PHONEME_TAB *ph, PHONEME_LIST *plist, int use_ipa, int *flags);
char *WritePhMnemonicWithStress(char *phon_out, PHONEME_TAB *ph, PHONEME_LIST *plist, int use_ipa, int *flags);
const char *GetTranslatedPhonemeString(int phoneme_mode);
int GetVowelStress(Translator *tr, unsigned char *phonemes, signed char *vowel_stress, int *vowel_count, int *stressed_syllable, int control);
int IsVowel(Translator *tr, int letter);
void SetWordStress(Translator *tr, char *output, unsigned int *dictionary_flags, int tonic, int control);
void AppendPhonemes(Translator *tr, char *string, int size, const char *ph);
int TranslateRules(Translator *tr, char *p_start, char *phonemes, int ph_size, char *end_phonemes, int word_flags, unsigned int *dict_flags);
int TransposeAlphabet(Translator *tr, char *text);
#ifdef USE_RUST_CORE
/* Caller extent is forwarded to native accent fallback. The remaining C
 * dictionary driver and raw lookup/text-mode writes are still to be ported. */
int LookupBounded(Translator *, const char *, char *, size_t);
int LookupDictListBounded(Translator *, char **, char *, unsigned int *, int, WORD_TAB *, int, size_t);
#else
int Lookup(Translator *tr, const char *word, char *ph_out);
int LookupDictList(Translator *tr, char **wordptr, char *ph_out, unsigned int *flags, int end_flags, WORD_TAB *wtab, int wtab_remaining);
#define LookupBounded(tr,word,out,capacity) Lookup(tr,word,out)
#define LookupDictListBounded(tr,word,out,flags,end,wtab,remaining,capacity) LookupDictList(tr,word,out,flags,end,wtab,remaining)
#endif
/* Internal compatibility adapter, available only in USE_RUST_CORE builds. */
const char *espeak_rs_lookup_dict(Translator *tr, const char *word, const char *word2, char *phonetic, unsigned int *flags, int end_flags, WORD_TAB *wtab, int wtab_remaining);
int RemoveEnding(Translator *tr, char *word, int end_type, char *word_copy);

#ifdef __cplusplus
}
#endif

#endif
