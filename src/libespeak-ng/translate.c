/*
 * Copyright (C) 2005 to 2014 by Jonathan Duddington
 * email: jonsd@users.sourceforge.net
 * Copyright (C) 2015-2017 Reece H. Dunn
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

#include "config.h"

#include <ctype.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>

#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/encoding.h>

#include "translate.h"
#include "common.h"
#include "dictionary.h"           // for TranslateRules, LookupDictList, Cha...
#include "phoneme.h"              // for phonSWITCH, PHONEME_TAB, phonPAUSE_...
#include "phonemelist.h"          // for MakePhonemeList
#include "readclause.h"           // for towlower2, Eof, ReadClause, is_str_...
#include "synthdata.h"            // for SelectPhonemeTable, LookupPhonemeTable
#include "synthesize.h"           // for PHONEME_LIST2, N_PHONEME_LIST, PHON...
#include "ucd/ucd.h"              // for ucd_toupper
#include "voice.h"                // for voice, voice_t
#include "speech.h"               // for MAKE_MEM_UNDEFINED
#include "translateword.h"
#ifdef USE_RUST_CORE
#include "rust_data.h"
#include "rust_translate_word2.h"
#include "rust_replacement.h"
#include "rust_translate_clause.h"
#endif

#if !defined(USE_RUST_CORE) || defined(ESPEAK_RUST_CORE_TEST)
static int CalcWordLength(int source_index, int charix_top, short int *charix, WORD_TAB *words, int word_count);
#endif
static void CombineFlag(Translator *tr, WORD_TAB *wtab, int wtab_remaining, char *word, int *flags, unsigned char *p, char *word_phonemes);
static void SwitchLanguage(char *word, char *word_phonemes);
static int TranslateWordWithBounds(Translator *tr, char *word_start, WORD_TAB *wtab, int wtab_remaining, char *word_out, int depth);

Translator *translator = NULL; // the main translator
Translator *translator2 = NULL; // secondary translator for certain words
static char translator2_language[20] = { 0 };
Translator *translator3 = NULL; // tertiary translator for certain words
static char translator3_language[20] = { 0 };

FILE *f_trans = NULL; // phoneme output text
int option_tone_flags = 0; // bit 8=emphasize allcaps, bit 9=emphasize penultimate stress
int option_phonemes = 0;
int option_phoneme_events = 0;
int option_endpause = 0; // suppress pause after end of text
int option_capitals = 0;
int option_punctuation = 0;
int option_sayas = 0;
static int option_sayas2 = 0; // used in translate_clause()
static int option_emphasis = 0; // 0=normal, 1=normal, 2=weak, 3=moderate, 4=strong
int option_ssml = 0;
int option_phoneme_input = 0; // allow [[phonemes]] in input
int option_wordgap = 0;

static int count_sayas_digits;
int skip_sentences;
int skip_words;
int skip_characters;
char skip_marker[N_MARKER_LENGTH];
bool skipping_text; // waiting until word count, sentence count, or named marker is reached
int end_character_position;
int count_sentences;
static int count_words;
int clause_start_char;
int clause_start_word;
static bool new_sentence;
static int word_emphasis = 0; // set if emphasis level 3 or 4
static int embedded_flag = 0; // there are embedded commands to be applied to the next phoneme, used in TranslateWord2()

static int max_clause_pause = 0;
static bool any_stressed_words;
int pre_pause;
static ALPHABET *current_alphabet;

char word_phonemes[N_WORD_PHONEMES]; // a word translated into phoneme codes
int n_ph_list2;
PHONEME_LIST2 ph_list2[N_PHONEME_LIST]; // first stage of text->phonemes

wchar_t option_punctlist[N_PUNCTLIST] = { 0 };

// these are overridden by defaults set in the "speak" file
int option_linelength = 0;

static int embedded_ix;
static int embedded_read;
unsigned int embedded_list[N_EMBEDDED_LIST];

// the source text of a single clause (UTF8 bytes)
static char source[N_TR_SOURCE+40]; // extra space for embedded command & voice change info at end

int n_replace_phonemes;
REPLACE_PHONEMES replace_phonemes[N_REPLACE_PHONEMES];

// other characters which break a word, but don't produce a pause
static const unsigned short breaks[] = { '_', 0 };

void DeleteTranslator(Translator *tr)
{
	if (!tr) return;

#ifdef USE_RUST_CORE
	if (tr->rust_dictionary_owner != NULL)
		espeak_rs_dictionary_handle_destroy(tr->rust_dictionary_owner);
	else if (tr->data_dictlist != NULL)
		free(tr->data_dictlist);
#else
	if (tr->data_dictlist != NULL)
		free(tr->data_dictlist);
#endif
	free(tr);
}

void FreeAlternateTranslators(void)
{
	DeleteTranslator(translator2);
	translator2 = NULL;
	translator2_language[0] = 0;
	DeleteTranslator(translator3);
	translator3 = NULL;
	translator3_language[0] = 0;
}

int lookupwchar(const unsigned short *list, int c)
{
	// Is the character c in the list ?
	int ix;

	for (ix = 0; list[ix] != 0; ix++) {
		if (list[ix] == c)
			return ix+1;
	}
	return 0;
}

char *strchr_w(const char *s, int c)
{
	// return NULL for any non-ascii character
	if (c >= 0x80)
		return NULL;
	return strchr((char *)s, c); // (char *) is needed for Borland compiler
}

#ifndef USE_RUST_CORE
/* Begin retained replacement segmentation. */
static bool ShouldSplitIdeographs(Translator *tr, int c, int prev)
{
	if (tr->langopts.ideographs && ((c > 0x3040) || (prev > 0x3040)))
		return true;

	// A non-ideographic base language may nominate an ideographic alternate
	// translator (Arabic uses Mandarin for Han characters). Split that run in
	// the same way as the alternate translator would, so every Han character
	// reaches its dictionary entry instead of one long foreign-script token.
	if (tr->langopts.alt_alphabet != 0) {
		const ALPHABET *alpha_c = AlphabetFromChar(c);
		const ALPHABET *alpha_prev = AlphabetFromChar(prev);
		if (((alpha_c != NULL) && (alpha_c->offset == tr->langopts.alt_alphabet)
				&& (alpha_c->flags & AL_IDEOGRAPHS))
				|| ((alpha_prev != NULL) && (alpha_prev->offset == tr->langopts.alt_alphabet)
				&& (alpha_prev->flags & AL_IDEOGRAPHS)))
			return true;
	}
	return false;
}

static void SegmentReplacement(Translator *tr, const char *text, char *out, int out_size)
{
	// Apply the word-splitting rules of the clause tokenizer (TranslateClause)
	// to dictionary replacement text, so that a $textmode replacement is
	// re-translated in the same word units as if it had been normal input:
	// for languages with langopts.ideographs each CJK character is a separate
	// word (so that hanzi words can match their multi-word *_list entries),
	// and a non-alpha mark such as a Thai tone mark terminates a word instead
	// of derailing the letter-to-phoneme rules mid-word.  A hyphen between two
	// letters also ends a word, as it does in normal input; without this a
	// hyphenated replacement name ("Burkina-Faso", "États-Unis") is handed to
	// the rules as one token, which stops at the hyphen and drops the rest.
	// Other ASCII characters are kept in-word, so that Latin-script
	// replacement text such as "cai3hong2" stays a single word.
	int ix = 0, out_ix = 0;
	int c = 0;

	while (text[ix] != 0) {
		int prev = c;
		int nbytes = utf8_in(&c, &text[ix]);
		bool insert_space = false;

		if (IsAlpha(prev)) {
			if (IsAlpha(c)) {
				if (ShouldSplitIdeographs(tr, c, prev))
					insert_space = true; // each ideograph is a separate word
			} else if (c == '-') {
				int next;

				utf8_in(&next, &text[ix + nbytes]);
				if (IsAlpha(next)) {
					// hyphen between two letters: the clause tokenizer turns it
					// into a word break, so do the same and drop the hyphen
					if (out_ix + 2 > out_size)
						break;
					out[out_ix++] = ' ';
					ix += nbytes;
					c = ' ';
					continue;
				}
			} else if (!IsSpace(c) && (c >= 0x80) && (wcschr(tr->punct_within_word, c) == 0))
				insert_space = true; // eg. a Thai tone mark ends the word, as in the clause tokenizer
		} else if ((prev >= 0x80) && !IsSpace(prev) && IsAlpha(c) && (wcschr(tr->punct_within_word, prev) == 0))
			insert_space = true; // a letter after such a mark starts a new word

		if (out_ix + nbytes + 2 > out_size)
			break;
		if (insert_space)
			out[out_ix++] = ' ';
		memcpy(&out[out_ix], &text[ix], nbytes);
		out_ix += nbytes;
		ix += nbytes;
	}
	out[out_ix] = 0;
}
/* End retained replacement segmentation. */
#endif

#ifndef USE_RUST_CORE
/* Begin retained replacement expansion. */
static int TranslateWordWithBounds(Translator *tr, char *word_start, WORD_TAB *wtab, int wtab_remaining, char *word_out, int depth)
{
	char words_phonemes[N_WORD_PHONEMES]; // a word translated into phoneme codes
	char *phonemes = words_phonemes;


	int flags = TranslateWord3(tr, word_start, wtab, wtab_remaining, word_out, &any_stressed_words, current_alphabet, word_phonemes, sizeof(word_phonemes));
	if (flags & FLAG_TEXTMODE && word_out) {
		// Ensure that start of word rules match with the replaced text,
		// so that emoji and other characters are pronounced correctly.
		// The buffer allows for a space inserted after every character of
		// the replacement by SegmentReplacement().
		char word[3+N_WORD_BYTES*2];
		word[0] = 0;
		word[1] = ' ';
		word[2] = ' ';
		SegmentReplacement(tr, word_out, word+3, sizeof(word)-3);
		word_out = word+3;

#ifdef USE_RUST_CORE
		// Replacement rules need the whole owned phrase for backward context.
		// Nested replacements restore the caller's source window below.
		const char *saved_base = tr->rule_text_base;
		size_t saved_length = tr->rule_text_length;
		tr->rule_text_base = word;
		tr->rule_text_length = strlen(word+3)+4;
#endif

			bool first_word = true;
			int available = N_WORD_PHONEMES;
		while (*word_out && available > 1) {
			int c;
			int nbytes = utf8_in(&c, word_out);
			if (iswupper(c)) {
				char lower_buf[8];
				int n_lower;

				wtab->flags |= FLAG_FIRST_UPPER;
				// towlower2(), not tolower(): the latter only knows ASCII, and
				// on a non-ASCII capital (Казахстан, Καζακστάν) it returned a
				// value whose UTF-8 form is a different length, which corrupted
				// the first letter of the word. Leave the letter alone if its
				// lower case form does not fit the same number of bytes.
				n_lower = utf8_out(towlower2(c, tr), lower_buf);
				if (n_lower == nbytes)
					memcpy(word_out, lower_buf, nbytes);
			} else {
				wtab->flags &= ~FLAG_FIRST_UPPER;
			}

			// dictionary_skipwords is a global variable and TranslateWord3 will reset it to 0 at the beginning.
			// However, dictionary_skipwords value is still needed outside this scope.
			// So we backup and restore it at the end of this scope.
			int skipwords = dictionary_skipwords;
			if (depth < 3) {
				// translate through TranslateWordWithBounds so that a replacement
				// word which itself has a $textmode entry is expanded too: eg. an
				// emoji entry replaces the emoji by hanzi, and the hanzi words map
				// to pinyin through *_list. The depth limit guards against an
				// entry cycle in the dictionary (a $textmode b, b $textmode a).
				char word_replacement2[N_WORD_BYTES+1];
				word_replacement2[0] = 0;
				TranslateWordWithBounds(tr, word_out, wtab, wtab_remaining, word_replacement2, depth+1);
			} else
				TranslateWord3(tr, word_out, wtab, wtab_remaining, NULL, &any_stressed_words, current_alphabet, word_phonemes, sizeof(word_phonemes));

			int n;
			if (first_word) {
				n = snprintf(phonemes, available, "%s", word_phonemes);
				first_word = false;
			} else {
				n = snprintf(phonemes, available, "%c%s", phonEND_WORD, word_phonemes);
			}

			available -= n;
			phonemes += n;

			// skip to the next word in a multi-word replacement. Always skip at least one word.
			for (dictionary_skipwords++; dictionary_skipwords > 0; dictionary_skipwords--) {
				while (!isspace(*word_out)) ++word_out;
				while (isspace(*word_out))  ++word_out;
			}
			dictionary_skipwords = skipwords;
		}

		// If the list file contains a text replacement to another
		// entry in the list file, e.g.:
		//     ripost     riposte $text
		//     riposte    rI#p0st
		// calling it from a prefix or suffix rule such as 'riposted'
		// causes word_out[0] to be NULL, as TranslateWord3 has the
		// information needed to perform the mapping. In this case,
		// no phonemes have been written in this loop and the phonemes
		// have been calculated, so don't override them.
		if (phonemes != words_phonemes) {
			snprintf(word_phonemes, sizeof(word_phonemes), "%s", words_phonemes);
		}
#ifdef USE_RUST_CORE
		tr->rule_text_base = saved_base;
		tr->rule_text_length = saved_length;
#endif
	}
	return flags;
}
/* End retained replacement expansion. */
#else
/* Begin native replacement projections. */
typedef struct { Translator *tr; char *word, *text; WORD_TAB *rows; int remaining; } RustReplacementHost;
static int ReplacementValue(void *opaque, unsigned field)
{
    RustReplacementHost *h=opaque;
    switch(field){case 0:return h->tr->langopts.ideographs;case 1:return h->tr->langopts.alt_alphabet;
    case 2:return h->rows!=NULL?(int)h->rows->flags:0;case 3:return dictionary_skipwords;default:return 0;}
}
static int ReplacementStore(void *opaque, unsigned field, int value)
{
    RustReplacementHost *h=opaque;
    if(field==2 && h->rows!=NULL)h->rows->flags=(unsigned)value;
    else if(field==3)dictionary_skipwords=value;else return -1;return 0;
}
static int ReplacementClass(void *opaque, unsigned code, unsigned kind)
{
    RustReplacementHost *h=opaque;
    switch(kind){case 0:return IsAlpha(code);case 1:return IsSpace(code);case 2:return iswupper(code)!=0;
    case 3:return code<128 && isspace((unsigned char)code)!=0;
    case 4:return wcschr(h->tr->punct_within_word,code)!=NULL;default:return 0;}
}
static unsigned ReplacementLower(void *opaque, unsigned code)
{
    RustReplacementHost *h=opaque;return (unsigned)towlower2(code,h->tr);
}
static int ReplacementMain(void *opaque, char *source, size_t initialized, size_t offset, char *text, unsigned *flags)
{
    RustReplacementHost *h=opaque;
    char *word=h->word,*destination=text;
    const char *base=h->tr->rule_text_base;size_t length=h->tr->rule_text_length;
    if(source!=NULL){if(initialized>3+N_WORD_BYTES*2||offset>=initialized)return -1;
        word=source+offset;h->tr->rule_text_base=source;h->tr->rule_text_length=initialized;}
    else if(text!=NULL)destination=h->text;
    *flags=TranslateWord3(h->tr,word,h->rows,h->remaining,destination,&any_stressed_words,
        current_alphabet,word_phonemes,sizeof(word_phonemes));
    if(source!=NULL){h->tr->rule_text_base=base;h->tr->rule_text_length=length;}
    if(source==NULL && text!=NULL && (*flags&FLAG_TEXTMODE)) {
        if(destination==NULL)return -1;
        const char *end=memchr(destination,0,N_WORD_BYTES+1);if(end==NULL)return -1;
        memcpy(text,destination,(size_t)(end-destination)+1);
    }
    return 0;
}
static int ReplacementSnapshot(void *opaque, char *out)
{
    (void)opaque;memcpy(out,word_phonemes,N_WORD_PHONEMES);return 0;
}
static int ReplacementPublish(void *opaque, const char *output)
{
    (void)opaque;const char *end=memchr(output,0,N_WORD_PHONEMES);if(end==NULL)return -1;
    memcpy(word_phonemes,output,(size_t)(end-output)+1);return 0;
}
static RustReplacement ReplacementCallbacks(RustReplacementHost *h)
{
    return (RustReplacement){h,ReplacementValue,ReplacementStore,ReplacementClass,ReplacementLower,
        ReplacementMain,ReplacementSnapshot,ReplacementPublish};
}
#ifdef ESPEAK_RUST_CORE_TEST
static bool ShouldSplitIdeographs(Translator *tr, int c, int previous)
{
    if(tr==NULL)return false;RustReplacementHost h={.tr=tr};RustReplacement callbacks=ReplacementCallbacks(&h);
    return espeak_rs_split_ideographs(&callbacks,(unsigned)c,(unsigned)previous)!=0;
}
#endif
static int TranslateWordWithBounds(Translator *tr, char *word, WORD_TAB *rows, int remaining, char *text, int depth)
{
    if(tr==NULL||word==NULL)return 0;
    RustReplacementHost h={tr,word,text,rows,remaining};RustReplacement callbacks=ReplacementCallbacks(&h);unsigned flags=0;
    return espeak_rs_replacement(&callbacks,text!=NULL,depth,&flags)==0?(int)flags:0;
}
/* End native replacement projections. */
#endif

int TranslateWord(Translator *tr, char *word_start, WORD_TAB *wtab, char *word_out)
{
	return TranslateWordWithBounds(tr, word_start, wtab, 0, word_out, 0);
}

#ifndef USE_RUST_CORE
/* Retained word-to-list entry initialization. */
static void SetPlist2(PHONEME_LIST2 *p, unsigned char phcode)
{
	p->phcode = phcode;
	p->stresslevel = 0;
	p->tone_ph = 0;
	p->synthflags = embedded_flag;
	p->sourceix = 0;
	embedded_flag = 0;
}
#endif

static int CountSyllables(unsigned char *phonemes)
{
	int count = 0;
	int phon;
	while ((phon = *phonemes++) != 0) {
		if (phoneme_tab[phon]->type == phVOWEL)
			count++;
	}
	return count;
}

static void Word_EmbeddedCmd(void)
{
	// Process embedded commands for emphasis, sayas, and break
	int embedded_cmd;
	do {
		embedded_cmd = embedded_list[embedded_read++];
		int value = embedded_cmd >> 8;

		switch (embedded_cmd & 0x1f)
		{
		case EMBED_Y:
			option_sayas = value;
			break;

		case EMBED_F:
			option_emphasis = value;
			break;

		case EMBED_B:
			// break command
			if (value == 0)
				pre_pause = 0; // break=none
			else
				pre_pause += value;
			break;
		}
	} while (((embedded_cmd & 0x80) == 0) && (embedded_read < embedded_ix));
}

static int SetAlternateTranslator(const char *new_language, Translator **translator, char translator_language[20])
{
	// Set alternate translator to a second language
	int new_phoneme_tab;

	if ((new_phoneme_tab = SelectPhonemeTableName(new_language)) >= 0) {
		if ((*translator != NULL) && (strcmp(new_language, translator_language) != 0)) {
			// we already have an alternative translator, but not for the required language, delete it
			DeleteTranslator(*translator);
			*translator = NULL;
		}

		if (*translator == NULL) {
			*translator = SelectTranslator(new_language);
			strcpy(translator_language, new_language);

			if (LoadDictionary(*translator, (*translator)->dictionary_name, 0) != 0) {
				SelectPhonemeTable(voice->phoneme_tab_ix); // revert to original phoneme table
				new_phoneme_tab = -1;
				translator_language[0] = 0;
			}
			(*translator)->phoneme_tab_ix = new_phoneme_tab;
		}
	}
	if (*translator != NULL)
		(*translator)->phonemes_repeat[0] = 0;
	return new_phoneme_tab;
}

int SetTranslator2(const char *new_language)
{
	return SetAlternateTranslator(new_language, &translator2, translator2_language);
}

int SetTranslator3(const char *new_language)
{
	return SetAlternateTranslator(new_language, &translator3, translator3_language);
}

#ifndef USE_RUST_CORE
/* Begin retained word-to-list policy. */
static int TranslateWord2(Translator *tr, char *word, WORD_TAB *wtab, int wtab_remaining, int pre_pause)
{
	int flags = 0;
	int stress;
	int next_stress;
	int next_tone = 0;
	unsigned char *p;
	int srcix;
	int found_dict_flag;
	unsigned char ph_code;
	PHONEME_LIST2 *plist2;
	PHONEME_TAB *ph;
	int max_stress;
	int max_stress_ix = 0;
	int prev_vowel = -1;
	int pitch_raised = 0;
	int switch_phonemes = -1;
	bool first_phoneme = true;
	int source_ix;
	int len;
	int bad_phoneme;
	int word_flags;
	char word_copy[N_WORD_BYTES+1];
	char word_replaced[N_WORD_BYTES+1];
	char old_dictionary_name[40];

	len = wtab->length;
	if (len > 31) len = 31;
	source_ix = (wtab->sourceix & 0x7ff) | (len << 11); // bits 0-10 sourceix, bits 11-15 word length

	word_flags = wtab[0].flags;
	if (word_flags & FLAG_EMBEDDED) {
		wtab[0].flags &= ~FLAG_EMBEDDED; // clear it in case we call TranslateWord2() again for the same word
		embedded_flag = SFLAG_EMBEDDED;

		Word_EmbeddedCmd();
	}

	if (n_ph_list2 >= N_PHONEME_LIST-2) {
		// No room, can't translate anything
		return 0;
	}

	if ((word[0] == 0) || (word_flags & FLAG_DELETE_WORD)) {
		// nothing to translate.  Add a dummy phoneme to carry any embedded commands
		if (embedded_flag) {
			SetPlist2(&ph_list2[n_ph_list2], phonEND_WORD);
			ph_list2[n_ph_list2].wordstress = 0;
			n_ph_list2++;
			embedded_flag = 0;
		}
		word_phonemes[0] = 0;
		return 0;
	}

	if (n_ph_list2 >= N_PHONEME_LIST-7-2) {
		// We may require up to 7 phonemes, plus the 2 phonemes from the caller, can't translate safely
		return 0;
	}

	// after a $pause word attribute, ignore a $pause attribute on the next two words
	if (tr->prepause_timeout > 0)
		tr->prepause_timeout--;

	if ((option_sayas & 0xf0) == 0x10) {
		if (!(word_flags & FLAG_FIRST_WORD)) {
			// SAYAS_CHARS, SAYAS_GLYPHS, or SAYAS_SINGLECHARS.  Pause between each word.
			pre_pause += 4;
		}
	}

	if (word_flags & FLAG_FIRST_UPPER) {
		if ((option_capitals > 2) && (embedded_ix < N_EMBEDDED_LIST-6)) {
			// indicate capital letter by raising pitch
			if (embedded_flag)
				embedded_list[embedded_ix-1] &= ~0x80; // already embedded command before this word, remove terminator
			if ((pitch_raised = option_capitals) == 3)
				pitch_raised = 20; // default pitch raise for capitals
			embedded_list[embedded_ix++] = EMBED_P+0x40+0x80 + (pitch_raised << 8); // raise pitch
			embedded_flag = SFLAG_EMBEDDED;
		}
	}

	p = (unsigned char *)word_phonemes;
	if (word_flags & FLAG_PHONEMES) {
		// The input is in phoneme mnemonics, not language text

		if (memcmp(word, "_^_", 3) == 0) {
			SwitchLanguage(word, word_phonemes);
		} else {
			EncodePhonemes(word, word_phonemes, &bad_phoneme);
		}

		flags = FLAG_FOUND;
	} else {
		int c2;
		int ix = 0;
		int word_copy_len;
		while (((c2 = word_copy[ix] = word[ix]) != ' ') && (c2 != 0) && (ix < N_WORD_BYTES)) ix++;
		word_copy_len = ix;

		word_replaced[2] = 0;
		flags = TranslateWordWithBounds(translator, word, wtab, wtab_remaining, &word_replaced[2], 0);

		if (flags & FLAG_SPELLWORD) {
			// re-translate the word as individual letters, separated by spaces
			memcpy(word, word_copy, word_copy_len);
			return flags;
		}

		if ((flags & FLAG_COMBINE) && (wtab_remaining > 1) && !(wtab[1].flags & FLAG_PHONEMES)) {
			CombineFlag(tr, wtab, wtab_remaining, word, &flags, p, word_phonemes);
		}

		if (p[0] == phonSWITCH) {
			int switch_attempt;
			strcpy(old_dictionary_name, dictionary_name);
			for (switch_attempt = 0; switch_attempt < 2; switch_attempt++) {
				// this word uses a different language
				memcpy(word, word_copy, word_copy_len);

				const char *new_language;
				new_language = (char *)(&p[1]);
				if (new_language[0] == 0)
					new_language = ESPEAKNG_DEFAULT_VOICE;

				switch_phonemes = SetTranslator2(new_language);

				if (switch_phonemes >= 0) {
					// re-translate the word using the new translator
					wtab[0].flags |= FLAG_TRANSLATOR2;
					if (word_replaced[2] != 0) {
						word_replaced[0] = 0; // byte before the start of the word
						word_replaced[1] = ' ';
						flags = TranslateWordWithBounds(translator2, &word_replaced[1], wtab, wtab_remaining, NULL, 0);
					} else
						flags = TranslateWordWithBounds(translator2, word, wtab, wtab_remaining, &word_replaced[2], 0);
				}

				if (p[0] != phonSWITCH)
					break;
			}

			if (p[0] == phonSWITCH)
				return FLAG_SPELLWORD;

			if (switch_phonemes < 0) {
				// language code is not recognised or 2nd translator won't translate it
				p[0] = phonSCHWA; // just say something
				p[1] = phonSCHWA;
				p[2] = 0;
			}

			if (switch_phonemes == -1) {
				strcpy(dictionary_name, old_dictionary_name);
				SelectPhonemeTable(voice->phoneme_tab_ix);

				// leave switch_phonemes set, but use the original phoneme table number.
				// This will suppress LOPT_REGRESSIVE_VOICING
				switch_phonemes = voice->phoneme_tab_ix; // original phoneme table
			}
		}

		if (!(word_flags & FLAG_HYPHEN)) {
			if (flags & FLAG_PAUSE1) {
				if (pre_pause < 1)
					pre_pause = 1;
			}
			if ((flags & FLAG_PREPAUSE) && !(word_flags & (FLAG_LAST_WORD | FLAG_FIRST_WORD)) && !(wtab[-1].flags & FLAG_FIRST_WORD) && (tr->prepause_timeout == 0)) {
				// the word is marked in the dictionary list with $pause
				if (pre_pause < 4) pre_pause = 4;
				tr->prepause_timeout = 3;
			}
		}

		if ((option_emphasis >= 3) && (pre_pause < 1))
			pre_pause = 1;
	}

	stress = 0;
	next_stress = 1;
	srcix = 0;
	max_stress = -1;

	found_dict_flag = 0;
	if ((flags & FLAG_FOUND) && !(flags & FLAG_TEXTMODE))
		found_dict_flag = SFLAG_DICTIONARY;

	// Each iteration may require up to 1 phoneme
	// and after this loop we may require up to 7 phonemes
	// and our caller requires 2 phonemes
	while ((pre_pause > 0) && (n_ph_list2 < N_PHONEME_LIST-7-2)) {
		// add pause phonemes here. Either because of punctuation (brackets or quotes) in the
		// text, or because the word is marked in the dictionary lookup as a conjunction
		if (pre_pause > 1) {
			SetPlist2(&ph_list2[n_ph_list2++], phonPAUSE);
			pre_pause -= 2;
		} else {
			SetPlist2(&ph_list2[n_ph_list2++], phonPAUSE_NOLINK);
			pre_pause--;
		}
		tr->end_stressed_vowel = 0; // forget about the previous word
		tr->prev_dict_flags[0] = 0;
		tr->prev_dict_flags[1] = 0;
	}
	plist2 = &ph_list2[n_ph_list2];
	// From here we may require up to 4+1+3 phonemes

	// This may require up to 4 phonemes
	if ((option_capitals == 1) && (word_flags & FLAG_FIRST_UPPER)) {
		SetPlist2(&ph_list2[n_ph_list2++], phonPAUSE_SHORT);
		SetPlist2(&ph_list2[n_ph_list2++], phonCAPITAL);
		if ((word_flags & FLAG_ALL_UPPER) && IsAlpha(word[1])) {
			// word > 1 letter and all capitals
			SetPlist2(&ph_list2[n_ph_list2++], phonPAUSE_SHORT);
			SetPlist2(&ph_list2[n_ph_list2++], phonCAPITAL);
		}
	}

	// This may require up to 1 phoneme
	if (switch_phonemes >= 0) {
		if ((p[0] == phonPAUSE) && (p[1] == phonSWITCH)) {
			// the new word starts with a phoneme table switch, so there's no need to switch before it.
			if (ph_list2[n_ph_list2-1].phcode == phonSWITCH) {
				// previous phoneme is also a phonSWITCH, delete it
				n_ph_list2--;
			}
		} else {
			// this word uses a different phoneme table
			if (ph_list2[n_ph_list2-1].phcode == phonSWITCH) {
				// previous phoneme is also a phonSWITCH, just change its phoneme table number
				n_ph_list2--;
			} else
				SetPlist2(&ph_list2[n_ph_list2], phonSWITCH);
			ph_list2[n_ph_list2++].tone_ph = switch_phonemes; // temporary phoneme table number
		}
	}

	// remove initial pause from a word if it follows a hyphen
	if ((word_flags & FLAG_HYPHEN) && (phoneme_tab[*p]->type == phPAUSE))
		p++;

	if ((p[0] == 0) && (embedded_flag)) {
		// no phonemes.  Insert a very short pause to carry an embedded command
		p[0] = phonPAUSE_VSHORT;
		p[1] = 0;
	}

	// Each iteration may require up to 1 phoneme
	// and after this loop we may require up to 3 phonemes
	// and our caller requires 2 phonemes
	while (((ph_code = *p++) != 0) && (n_ph_list2 < N_PHONEME_LIST-3-2)) {
		if (ph_code == 255)
			continue; // unknown phoneme

		// Add the phonemes to the first stage phoneme list (ph_list2)
		ph = phoneme_tab[ph_code];
		if (ph == NULL) {
			printf("Invalid phoneme code %d\n", ph_code);
			continue;
		}

		if (ph_code == phonSWITCH) {
			ph_list2[n_ph_list2].phcode = ph_code;
			ph_list2[n_ph_list2].stresslevel = 0;
			ph_list2[n_ph_list2].sourceix = 0;
			ph_list2[n_ph_list2].synthflags = 0;
			ph_list2[n_ph_list2++].tone_ph = *p;
			SelectPhonemeTable(*p);
			p++;
		} else if (ph->type == phSTRESS) {
			// don't add stress phonemes codes to the list, but give their stress
			// value to the next vowel phoneme
			// std_length is used to hold stress number or (if >10) a tone number for a tone language
			if (ph->program == 0)
				next_stress = ph->std_length;
			else {
				// for tone languages, the tone number for a syllable follows the vowel
				if (prev_vowel >= 0)
					ph_list2[prev_vowel].tone_ph = ph_code;
				else
					next_tone = ph_code; // no previous vowel, apply to the next vowel
			}
		} else if (ph_code == phonSYLLABIC) {
			// mark the previous phoneme as a syllabic consonant
			prev_vowel = n_ph_list2-1;
			ph_list2[prev_vowel].synthflags |= SFLAG_SYLLABLE;
			ph_list2[prev_vowel].stresslevel = next_stress;
		} else if (ph_code == phonLENGTHEN)
			ph_list2[n_ph_list2-1].synthflags |= SFLAG_LENGTHEN;
		else if (ph_code == phonEND_WORD) {
			// a || symbol in a phoneme string was used to indicate a word boundary
			// Don't add this phoneme to the list, but make sure the next phoneme has
			// a newword indication
			srcix = source_ix+1;
		} else if (ph_code == phonX1) {
			// a language specific action
				flags |= FLAG_DOUBLING;
		} else {
			ph_list2[n_ph_list2].phcode = ph_code;
			ph_list2[n_ph_list2].tone_ph = 0;
			ph_list2[n_ph_list2].synthflags = embedded_flag | found_dict_flag;
			embedded_flag = 0;
			ph_list2[n_ph_list2].sourceix = srcix;
			srcix = 0;

			if (ph->type == phVOWEL) {
				stress = next_stress;
				next_stress = 1; // default is 'unstressed'

				if (stress >= 4)
					any_stressed_words = true;

				if ((prev_vowel >= 0) && (n_ph_list2-1) != prev_vowel)
					ph_list2[n_ph_list2-1].stresslevel = stress; // set stress for previous consonant

				ph_list2[n_ph_list2].synthflags |= SFLAG_SYLLABLE;
				prev_vowel = n_ph_list2;

				if (stress > max_stress) {
					max_stress = stress;
					max_stress_ix = n_ph_list2;
				}
				if (next_tone != 0) {
					ph_list2[n_ph_list2].tone_ph = next_tone;
					next_tone = 0;
				}
			} else {
				if (first_phoneme && tr->prev_dict_flags[0] & FLAG_DOUBLING) {
						// double the initial consonant if the previous word is marked with a flag
					ph_list2[n_ph_list2].synthflags |= SFLAG_LENGTHEN;
				}
			}

			ph_list2[n_ph_list2].stresslevel = stress;
			n_ph_list2++;
			first_phoneme = false;
		}
	}
	// From here, we may require up to 3 phonemes

	// This may require up to 1 phoneme
	if (word_flags & FLAG_COMMA_AFTER)
		SetPlist2(&ph_list2[n_ph_list2++], phonPAUSE_CLAUSE);

	// don't set new-word if there is a hyphen before it
	if ((word_flags & FLAG_HYPHEN) == 0)
		plist2->sourceix = source_ix;

	tr->end_stressed_vowel = 0;
	if ((stress >= 4) && (phoneme_tab[ph_list2[n_ph_list2-1].phcode]->type == phVOWEL))
		tr->end_stressed_vowel = 1; // word ends with a stressed vowel

	// This may require up to 1 phoneme
	if (switch_phonemes >= 0) {
		// this word uses a different phoneme table, now switch back
		strcpy(dictionary_name, old_dictionary_name);
		SelectPhonemeTable(voice->phoneme_tab_ix);
		SetPlist2(&ph_list2[n_ph_list2], phonSWITCH);
		ph_list2[n_ph_list2++].tone_ph = voice->phoneme_tab_ix; // original phoneme table number
	}


	// This may require up to 1 phoneme
	if (pitch_raised > 0) {
		embedded_list[embedded_ix++] = EMBED_P+0x60+0x80 + (pitch_raised << 8); // lower pitch
		SetPlist2(&ph_list2[n_ph_list2], phonPAUSE_SHORT);
		ph_list2[n_ph_list2++].synthflags = SFLAG_EMBEDDED;
	}

	if (flags & FLAG_STRESS_END2) {
		// this's word's stress could be increased later
		ph_list2[max_stress_ix].synthflags |= SFLAG_PROMOTE_STRESS;
	}

	tr->prev_dict_flags[0] = flags;
	return flags;
}
/* End retained word-to-list policy. */
#else
/* Begin native word-to-list projections. */
typedef struct { Translator *tr; char *word; WORD_TAB *rows; int remaining; size_t source_length; } RustWord2Host;
static int Word2Value(void *opaque, unsigned field)
{
    RustWord2Host *h=opaque;
    switch(field) {
    case 0:return h->rows->length; case 1:return h->rows->sourceix; case 2:return h->rows->flags;
    case 3:return h->rows[-1].flags; case 4:return h->remaining>1?h->rows[1].flags:0; case 5:return h->remaining;
    case 6:return n_ph_list2; case 7:return embedded_flag; case 8:return embedded_ix;
    case 9:return option_sayas; case 10:return option_capitals; case 11:return option_emphasis;
    case 12:return h->tr->prepause_timeout; case 13:return h->tr->end_stressed_vowel;
    case 14:return h->tr->prev_dict_flags[0]; case 15:return h->tr->prev_dict_flags[1];
    case 16:return any_stressed_words; case 17:return voice->phoneme_tab_ix;
    default:return 0;
    }
}
static int Word2Store(void *opaque, unsigned field, int value)
{
    RustWord2Host *h=opaque;
    switch(field) {
    case 2:h->rows->flags=value;break;
    case 6:if(value<0||value>=N_PHONEME_LIST)return -1;n_ph_list2=value;break;
    case 7:embedded_flag=value;break;
    case 8:if(value<0||value>N_EMBEDDED_LIST)return -1;embedded_ix=value;break;
    case 12:h->tr->prepause_timeout=value;break;case 13:h->tr->end_stressed_vowel=value;break;
    case 14:h->tr->prev_dict_flags[0]=value;break;case 15:h->tr->prev_dict_flags[1]=value;break;
    case 16:any_stressed_words=value!=0;break;default:return -1;
    }
    return 0;
}
static int Word2Byte(void *opaque, size_t offset, unsigned pronunciation)
{
    RustWord2Host *h=opaque;
    if(offset >= (pronunciation?N_WORD_PHONEMES:h->source_length))return -1;
    return (unsigned char)(pronunciation?word_phonemes:h->word)[offset];
}
static int Word2Write(void *opaque, size_t offset, unsigned char byte, unsigned pronunciation)
{
    RustWord2Host *h=opaque;
    if(offset >= (pronunciation?N_WORD_PHONEMES:h->source_length))return -1;
    (pronunciation?word_phonemes:h->word)[offset]=(char)byte;return 0;
}
static int Word2Entry(void *opaque, int index, PHONEME_LIST2 *out)
{
    (void)opaque;if(index<0||index>=N_PHONEME_LIST)return -1;*out=ph_list2[index];return 0;
}
static int Word2Put(void *opaque, int index, const PHONEME_LIST2 *entry)
{
    (void)opaque;if(index<0||index>=N_PHONEME_LIST)return -1;ph_list2[index]=*entry;return 0;
}
static int Word2Command(void *opaque, int index, unsigned *out)
{
    (void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;*out=embedded_list[index];return 0;
}
static int Word2PutCommand(void *opaque, int index, unsigned value)
{
    (void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;embedded_list[index]=value;return 0;
}
static int Word2Embedded(void *opaque){(void)opaque;Word_EmbeddedCmd();return 0;}
static int Word2Encode(void *opaque, unsigned language)
{
    RustWord2Host *h=opaque;int bad;
    if(language)SwitchLanguage(h->word,word_phonemes);else EncodePhonemes(h->word,word_phonemes,&bad);
    return 0;
}
static int Word2Translate(void *opaque, unsigned alternate, unsigned mode, char *replacement, unsigned *flags)
{
    RustWord2Host *h=opaque;Translator *tr=alternate?translator2:translator;
    if(tr==NULL)return -1;
    if(mode) {
        const char *base=tr->rule_text_base;size_t length=tr->rule_text_length;
        tr->rule_text_base=replacement;tr->rule_text_length=N_WORD_BYTES+3;
        *flags=TranslateWordWithBounds(tr,replacement+1,h->rows,h->remaining,NULL,0);
        tr->rule_text_base=base;tr->rule_text_length=length;
    } else *flags=TranslateWordWithBounds(tr,h->word,h->rows,h->remaining,replacement+2,0);
    return 0;
}
static int Word2Combine(void *opaque, unsigned *flags)
{
    RustWord2Host *h=opaque;int result=(int)*flags;
    CombineFlag(h->tr,h->rows,h->remaining,h->word,&result,(unsigned char *)word_phonemes,word_phonemes);
    *flags=(unsigned)result;return 0;
}
static int Word2Dictionary(void *opaque, char *out)
{
    (void)opaque;const char *end=memchr(dictionary_name,0,40);if(end==NULL)return -1;
    memcpy(out,dictionary_name,(size_t)(end-dictionary_name)+1);return 0;
}
static int Word2RestoreDictionary(void *opaque, const char *name)
{
    (void)opaque;const char *end=memchr(name,0,40);if(end==NULL)return -1;
    memcpy(dictionary_name,name,(size_t)(end-name)+1);return 0;
}
static int Word2Alternate(void *opaque, const char *name, int *table)
{
    (void)opaque;*table=SetTranslator2(name[0]?name:ESPEAKNG_DEFAULT_VOICE);return 0;
}
static int Word2Select(void *opaque, int table){(void)opaque;SelectPhonemeTable(table);return 0;}
static int Word2Phoneme(void *opaque, unsigned char code, RustWord2Phoneme *out)
{
    (void)opaque;PHONEME_TAB *ph=phoneme_tab[code];if(ph==NULL)return -1;
    *out=(RustWord2Phoneme){ph->type,ph->std_length,ph->program};return 0;
}
static int Word2Alpha(void *opaque, unsigned char code){(void)opaque;return IsAlpha(code);}
static void Word2Invalid(void *opaque, unsigned char code){(void)opaque;printf("Invalid phoneme code %d\n",code);}
static int TranslateWord2(Translator *tr, char *word, WORD_TAB *rows, int remaining, int pause)
{
    if(tr==NULL||word==NULL||rows==NULL)return 0;
    RustWord2Host h={tr,word,rows,remaining,0};
    const Translator *owners[4]={tr,translator,translator2,translator3};
    for(unsigned i=0;i<4;i++) {
        const Translator *owner=owners[i];if(owner==NULL||owner->rule_text_base==NULL)continue;
        uintptr_t address=(uintptr_t)word,base=(uintptr_t)owner->rule_text_base;
        if(address>=base && address-base<owner->rule_text_length){h.source_length=owner->rule_text_length-(address-base);break;}
    }
    if(h.source_length==0)h.source_length=strlen(word)+1;
    RustTranslateWord2 callbacks={&h,Word2Value,Word2Store,Word2Byte,Word2Write,Word2Entry,Word2Put,
        Word2Command,Word2PutCommand,Word2Embedded,Word2Encode,Word2Translate,Word2Combine,
        Word2Dictionary,Word2RestoreDictionary,Word2Alternate,Word2Select,Word2Phoneme,Word2Alpha,Word2Invalid};
    unsigned flags=0;
    return espeak_rs_translate_word2(&callbacks,pause,&flags)==0?(int)flags:0;
}
/* End native word-to-list projections. */
#endif

#ifdef USE_RUST_CORE
static int TranslateWord2WithContext(Translator *tr, char *word, WORD_TAB *wtab, int remaining, int pause, const char *base, size_t initialized)
{
	const char *saved_base = tr->rule_text_base;
	size_t saved_length = tr->rule_text_length;
	tr->rule_text_base = base;
	tr->rule_text_length = initialized;
	int result = TranslateWord2(tr,word,wtab,remaining,pause);
	tr->rule_text_base = saved_base;
	tr->rule_text_length = saved_length;
	return result;
}
#else
#define TranslateWord2WithContext(tr,word,wtab,remaining,pause,base,initialized) TranslateWord2(tr,word,wtab,remaining,pause)
#endif

static int EmbeddedCommand(unsigned int *source_index_out)
{
	// An embedded command to change the pitch, volume, etc.
	// returns number of commands added to embedded_list

	// pitch,speed,amplitude,expression,reverb,tone,voice,sayas
	const char *commands = "PSARHTIVYMUBF";
	int value = -1;
	int sign = 0;
	unsigned char c;
	char *p;
	int cmd;
	int source_index = *source_index_out;

	c = source[source_index];
	if (c == '+') {
		sign = 0x40;
		source_index++;
	} else if (c == '-') {
		sign = 0x60;
		source_index++;
	}

	if (IsDigit09(source[source_index])) {
		value = atoi(&source[source_index]);
		while (IsDigit09(source[source_index]))
			source_index++;
	}

	c = source[source_index++];
	if (embedded_ix >= (N_EMBEDDED_LIST - 2))
		return 0; // list is full

	if ((p = strchr_w(commands, c)) == NULL)
		return 0;
	cmd = (p - commands)+1;
	if (value == -1) {
		value = embedded_default[cmd];
		sign = 0;
	}

	if (cmd == EMBED_Y) {
		option_sayas2 = value;
		count_sayas_digits = 0;
	}
	if (cmd == EMBED_F) {
		if (value >= 3)
			word_emphasis = FLAG_EMPHASIZED;
		else
			word_emphasis = 0;
	}

	embedded_list[embedded_ix++] = cmd + sign + (value << 8);
	*source_index_out = source_index;
	return 1;
}

static const char *FindReplacementChars(Translator *tr, const char **pfrom, unsigned int c, const char *next, int *ignore_next_n)
{
	const char *from = *pfrom;
	while ( !is_str_totally_null(from, 4) ) {
		unsigned int fc = 0; // from character
		unsigned int nc = c; // next character
		const char *match_next = next;

		*pfrom = from;

		from += utf8_in((int *)&fc, from);
		if (nc == fc) {
			if (*from == 0) return from + 1;

			bool matched = true;
			int nmatched = 0;
			while (*from != 0) {
				from += utf8_in((int *)&fc, from);

				match_next += utf8_in((int *)&nc, match_next);
				nc = towlower2(nc, tr);

				if (nc != fc)
					matched = false;
				else
					nmatched++;
			}

			if (matched) {
				*ignore_next_n = nmatched;
				return from + 1;
			}
		}

		// replacement 'from' string (skip the remaining part, if any)
		while (*from != '\0') from++;
		from++;

		// replacement 'to' string
		while (*from != '\0') from++;
		from++;
	}
	return NULL;
}

// handle .replace rule in xx_rules file
static int SubstituteChar(Translator *tr, unsigned int c, unsigned int next_in, const char *next, int *insert, int *wordflags)
{
	unsigned int new_c, c2 = ' ', c_lower;
	int upper_case = 0;

	static int ignore_next_n = 0;
	if (ignore_next_n > 0) {
		ignore_next_n--;
		return 8;
	}

	if (c == 0) return 0;

	const char *from = (const char *)tr->langopts.replace_chars;
	if (from == NULL)
		return c;

	// there is a list of character codes to be substituted with alternative codes

	if (iswupper(c_lower = c)) {
		c_lower = towlower2(c, tr);
		upper_case = 1;
	}

	const char *to = FindReplacementChars(tr, &from, c_lower, next, &ignore_next_n);
	if (to == NULL)
		return c; // no substitution

	if (option_phonemes & espeakPHONEMES_TRACE)
		fprintf(f_trans, "Replace: %s > %s\n", from, to);

	to += utf8_in((int *)&new_c, to);
	if (*to != 0) {
		// there is a second character to be inserted
		// don't convert the case of the second character unless the next letter is also upper case
		to += utf8_in((int *)&c2, to);
		if (upper_case && iswupper(next_in))
			c2 = ucd_toupper(c2);
		*insert = c2;
	}

	if (upper_case)
		new_c = ucd_toupper(new_c);

	*wordflags |= FLAG_CHAR_REPLACED;
	return new_c;
}

static int TranslateChar(Translator *tr, char *ptr, int prev_in, unsigned int c, unsigned int next_in, int *insert, int *wordflags)
{
	// To allow language specific examination and replacement of characters

	int code;
	int next2;

	static const unsigned char hangul_compatibility[0x34] = {
		0,  0x00, 0x01, 0xaa, 0x02, 0xac, 0xad, 0x03,
		0x04, 0x05, 0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb4,
		0xb6, 0x06, 0x07, 0x08, 0xb9, 0x09, 0x0a, 0xbc,
		0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x61,
		0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
		0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f, 0x70, 0x71,
		0x72, 0x73, 0x74, 0x75
	};

	// check for Korean Hangul letters
	if (((code = c - 0xac00) >= 0) && (c <= 0xd7af)) {
		// break a syllable hangul into 2 or 3 individual jamo
		int initial = (code/28)/21;
		int medial = (code/28) % 21;
		int final = code % 28;

		if (initial == 11) {
			// null initial
			c = medial + 0x1161;
			if (final > 0)
				*insert = final + 0x11a7;
		} else {
			// extract the initial and insert the remainder with a null initial
			c = initial + 0x1100;
			*insert = (11*28*21) + (medial*28) + final + 0xac00;
		}
		return c;
	} else if (((code = c - 0x3130) >= 0) && (code < 0x34)) {
		// Hangul compatibility jamo
		return hangul_compatibility[code] + 0x1100;
	}

	switch (tr->translator_name)
	{
	case L('a', 'f'):
	case L('n', 'l'):
		// look for 'n  and replace by a special character (unicode: schwa)

		if ((c == '\'') && !iswalpha(prev_in)) {
			utf8_in(&next2, &ptr[1]);

			if (IsSpace(next2)) {
				if ((next_in == 'n') && (tr->translator_name == L('a', 'f'))) {
					// n preceded by either apostrophe or U2019 "right single quotation mark"
					ptr[0] = ' '; // delete the n
					return 0x0259; // replace  '  by  unicode schwa character
				}
				if ((next_in == 'n') || (next_in == 't')) {
					// Dutch, [@n] and [@t]
					return 0x0259; // replace  '  by  unicode schwa character
				}
			}
		}
		break;
	}
	// handle .replace rule in xx_rules file
	return SubstituteChar(tr, c, next_in, ptr, insert, wordflags);
}

static const char *const UCase_ga[] = { "bp", "bhf", "dt", "gc", "hA", "mb", "nd", "ng", "ts", "tA", "nA", NULL };

static int UpperCaseInWord(Translator *tr, char *word, int c)
{
	if (tr->translator_name == L('g', 'a')) {
		int ix;
		const char *p;

		for (ix = 0;; ix++) {
			int len;
			if ((p = UCase_ga[ix]) == NULL)
				break;

			len = strlen(p);
			if ((word[-len] == ' ') && (memcmp(&word[-len+1], p, len-1) == 0)) {
				if ((c == p[len-1]) || ((p[len-1] == 'A') && IsVowel(tr, c)))
					return 1;
			}
		}
	}
	return 0;
}

// Same as TranslateClause except we also get the clause terminator used (full stop, comma, etc.).
// Used by espeak_TextToPhonemesWithTerminator.
#ifndef USE_RUST_CORE
/* Begin retained clause policy. */
void TranslateClauseWithTerminator(Translator *tr, int *tone_out, char **voice_change, int *terminator_out)
{
	int ix;
	int c;
	int cc = 0;
	unsigned int source_index = 0;
	int source_index_word = 0;
	int prev_in;
	int prev_out = ' ';
	int prev_in_save = 0;
	int next_in;
	int next_in_nbytes;
	int char_inserted = 0;
	int clause_pause;
	int pre_pause_add = 0;
	int all_upper_case = FLAG_ALL_UPPER;
	int alpha_count = 0;
	bool finished = false;
	bool single_quoted = false;
	bool phoneme_mode = false;
	int dict_flags = 0; // returned from dictionary lookup
	int word_flags; // set here
	int next_word_flags;
	bool new_sentence2;
	int embedded_count = 0;
	int letter_count = 0;
	bool space_inserted = false;
	bool syllable_marked = false;
	bool decimal_sep_count = false;
	char *word;
	char *p;
	int j, k;
	int n_digits;
	int charix_top = 0;

	short charix[N_TR_SOURCE+4];
	WORD_TAB words[N_CLAUSE_WORDS];
	static char voice_change_name[40];
	int word_count = 0; // index into words

	char sbuf[N_TR_SOURCE];

	int terminator;
	int tone;

	if (tr == NULL)
		return;

	MAKE_MEM_UNDEFINED(&voice_change_name, sizeof(voice_change_name));

	embedded_ix = 0;
	embedded_read = 0;
	pre_pause = 0;
	any_stressed_words = false;

	if ((clause_start_char = count_characters) < 0)
		clause_start_char = 0;
	clause_start_word = count_words + 1;

	for (ix = 0; ix < N_TR_SOURCE; ix++)
		charix[ix] = 0;
	MAKE_MEM_UNDEFINED(&source, sizeof(source));
	terminator = ReadClause(tr, source, charix, &charix_top, N_TR_SOURCE, &tone, voice_change_name);

	if (terminator_out != NULL) {
		*terminator_out = terminator;
	}

	if (tone_out != NULL) {
		if (tone == 0)
			*tone_out = (terminator & CLAUSE_INTONATION_TYPE) >> 12; // tone type not overridden in ReadClause, use default
		else
			*tone_out = tone; // override tone type
	}

	charix[charix_top+1] = 0;
	charix[charix_top+2] = 0x7fff;
	charix[charix_top+3] = 0;

	clause_pause = (terminator & CLAUSE_PAUSE) * 10; // mS
	if (terminator & CLAUSE_PAUSE_LONG)
		clause_pause = clause_pause * 32; // pause value is *320mS not *10mS

	for (p = source; *p != 0; p++) {
		if (!isspace2(*p))
			break;
	}
	if (*p == 0) {
		// No characters except spaces. This is not a sentence.
		// Don't add this pause, just make up the previous pause to this value;
		clause_pause -= max_clause_pause;
		if (clause_pause < 0)
			clause_pause = 0;

		if (new_sentence)
			terminator |= CLAUSE_TYPE_SENTENCE; // carry forward an end-of-sentence indicator
		max_clause_pause += clause_pause;
		new_sentence2 = false;
	} else {
		max_clause_pause = clause_pause;
		new_sentence2 = new_sentence;
	}
	tr->clause_terminator = terminator;

	if (new_sentence2) {
		count_sentences++;
		if (skip_sentences > 0) {
			skip_sentences--;
			if (skip_sentences == 0)
				skipping_text = false;
		}
	}

	MAKE_MEM_UNDEFINED(&ph_list2, sizeof(ph_list2));
	memset(&ph_list2[0], 0, sizeof(ph_list2[0]));
	ph_list2[0].phcode = phonPAUSE_SHORT;

	n_ph_list2 = 1;
	tr->prev_last_stress = 0;
	tr->prepause_timeout = 0;
	tr->expect_verb = 0;
	tr->expect_noun = 0;
	tr->expect_past = 0;
	tr->expect_verb_s = 0;
	tr->phonemes_repeat_count = 0;
	tr->end_stressed_vowel = 0;
	tr->prev_dict_flags[0] = 0;
	tr->prev_dict_flags[1] = 0;

	word_count = 0;
	word_flags = 0;
	next_word_flags = 0;

	sbuf[0] = 0;
	sbuf[1] = ' ';
	sbuf[2] = ' ';
	ix = 3;
	prev_in = ' ';

	words[0].start = ix;
	words[0].flags = 0;

	words[0].length = CalcWordLength(source_index, charix_top, charix, words, 0);

	int prev_out2;
	while (!finished && (ix < (int)sizeof(sbuf) - 1)) {
		prev_out2 = prev_out;
		utf8_in2(&prev_out, &sbuf[ix-1], 1);

		if (tr->langopts.tone_numbers && IsDigit09(prev_out) && IsAlpha(prev_out2)) {
			// tone numbers can be part of a word, consider them as alphabetic
			prev_out = 'a';
		}

		if (prev_in_save != 0) {
			prev_in = prev_in_save;
			prev_in_save = 0;
		} else if (source_index > 0)
			utf8_in2(&prev_in, &source[source_index-1], 1);

		unsigned int prev_source_index = source_index;

		if (char_inserted) {
			c = char_inserted;
			char_inserted = 0;
		} else {
			source_index += utf8_in(&cc, &source[source_index]);
			c = cc;
		}

		if (c == 0) {
			finished = true;
			c = ' ';
			next_in = ' ';
			next_in_nbytes = 0;
		}
		else
			next_in_nbytes = utf8_in(&next_in, &source[source_index]);

		if (c == CTRL_EMBEDDED) {
			// start of embedded command in the text
			int srcix = source_index-1;

			if (prev_in != ' ') {
				c = ' ';
				prev_in_save = c;
				source_index--;
			} else {
				embedded_count += EmbeddedCommand(&source_index);
				prev_in_save = prev_in;
				// replace the embedded command by spaces
				memset(&source[srcix], ' ', source_index-srcix);
				source_index = srcix;
				continue;
			}
		}

		if ((option_sayas2 == SAYAS_KEY) && (c != ' ')) {
			if ((prev_in == ' ') && (next_in == ' '))
				option_sayas2 = SAYAS_SINGLE_CHARS; // single character, speak its name
			c = towlower2(c, tr);
		}


		if (phoneme_mode) {
			all_upper_case = FLAG_PHONEMES;

			if ((c == ']') && (next_in == ']')) {
				phoneme_mode = false;
				source_index++;
				c = ' ';
			}
		} else if ((option_sayas2 & 0xf0) == SAYAS_DIGITS) {
			if (iswdigit(c)) {
				count_sayas_digits++;
				if (count_sayas_digits > (option_sayas2 & 0xf)) {
					// break after the specified number of digits
					c = ' ';
					space_inserted = true;
					count_sayas_digits = 0;
				}
			} else {
				count_sayas_digits = 0;
				if (iswdigit(prev_out)) {
					c = ' ';
					space_inserted = true;
				}
			}
		} else if ((option_sayas2 & 0x10) == 0) {
			// speak as words

			if ((c == 0x92) || (c == 0xb4) || (c == 0x2019) || (c == 0x2032))
				c = '\''; // 'microsoft' quote or sexed closing single quote, or prime - possibly used as apostrophe

			if (((c == 0x2018) || (c == '?')) && IsAlpha(prev_out) && IsAlpha(next_in)) {
				// ? between two letters may be a smart-quote replaced by ?
				c = '\'';
			}

			if (c == CHAR_EMPHASIS) {
				// this character is a marker that the previous word is the focus of the clause
				c = ' ';
				word_flags |= FLAG_FOCUS;
			}

			if (c == CHAR_COMMA_BREAK) {
				c = ' ';
				word_flags |= FLAG_COMMA_AFTER;
			}
			// language specific character translations
			c = TranslateChar(tr, &source[source_index], prev_in, c, next_in, &char_inserted, &word_flags);
			if (c == 8)
				continue; // ignore this character

			if ((c == 0xfe0e) || (c == 0xfe0f))
				continue; // variation selector (text/emoji presentation style), not spoken;
				          // emoji dictionary keys are stripped of these at compile time to match

			if (char_inserted)
				next_in = char_inserted;

			// allow certain punctuation within a word (usually only apostrophe)
			if (!IsAlpha(c) && !IsSpace(c) && (wcschr(tr->punct_within_word, c) == 0)) {
				if (IsAlpha(prev_out)) {
					if (tr->langopts.tone_numbers && IsDigit09(c) && !IsDigit09(next_in)) {
						// allow a tone number as part of the word
					} else if ((c == 0x200d) && IsEmoji(prev_out) && IsEmoji(next_in)) {
						// keep U+200D ZERO WIDTH JOINER between two emoji in the word,
						// so the sequence can match a multi-codepoint emoji dictionary
						// entry (e.g. woman + ZWJ + microscope = woman scientist)
					} else if (IsEmojiTag(c) && IsEmoji(prev_out)) {
						// keep the tag characters of an emoji tag sequence in the
						// word, so that it can match its dictionary entry (black
						// flag + the tag letters "gbsct" + cancel tag = flag of
						// Scotland). The following tag characters get here with a
						// tag character as prev_out, which is not IsAlpha, so they
						// are kept without needing a case of their own.
					} else {
						c = ' '; // ensure we have an end-of-word terminator
						space_inserted = true;
					}
				}
			}

			if (iswdigit(prev_out)) {
				if (!iswdigit(c) && (c != '.') && (c != ',') && (c != ' ')) {
					c = ' '; // terminate digit string with a space
					space_inserted = true;
				}
			} else { // Prev output is not digit
				if (prev_in == ',') {
					// Workaround for several consecutive commas —
					// replace current character with space
					if (c == ',')
						c = ' ';
				} else {
					decimal_sep_count = false;
				}
			}

			if (c == '[') {
				if ((next_in == '\002') || ((next_in == '[') && option_phoneme_input)) {
					//  "[\002" is used internally to start phoneme mode
					phoneme_mode = true;
					source_index++;
					continue;
				}
			}

			if (IsAlpha(c)) {
				bool emoji_join = false;
				bool emoji_break = false;

				alpha_count++;
				if ((prev_out == 0x200d) && IsEmoji(c)) {
					// U+200D ZERO WIDTH JOINER: continue the emoji sequence only if
					// the current word began with an emoji; a word-initial ZWJ (stray
					// joiner in the input) must not absorb the following emoji, which
					// would prevent its dictionary lookup
					int wc_first;
					utf8_in(&wc_first, &sbuf[words[word_count].start]);
					emoji_join = IsEmoji(wc_first);
				} else if (IsEmoji(c) && IsEmoji(prev_out)) {
					// Two emoji written next to each other. They only belong to the
					// same token when they form one of the sequences that the emoji
					// dictionary has a key for; otherwise each emoji must become its
					// own word, or the whole run fails its dictionary lookup and is
					// spelled out codepoint by codepoint.
					if (IsEmojiModifier(c)) {
						// skin tone modifier, part of the preceding emoji
					} else if (IsRegionalIndicator(c) && IsRegionalIndicator(prev_out)) {
						// A flag is exactly two regional indicators (e.g. 🇰🇿), so
						// count how many of them the word already holds: an odd
						// count means this one completes a flag, an even count
						// means it starts the next one.
						int n_ri = 0;
						int j = ix;

						while (j > (int)words[word_count].start) {
							int wc_ri;
							utf8_in2(&wc_ri, &sbuf[j-1], 1);
							if (!IsRegionalIndicator(wc_ri))
								break;
							n_ri++;
							j -= 4; // a regional indicator is 4 bytes in UTF-8
						}
						emoji_break = ((n_ri & 1) == 0);
					} else
						emoji_break = true;
				}
				// Start a new word where an adjacent letter belongs to a script
				// that is foreign to the base language (e.g. Latin glued to
				// Malayalam, as in "BJPയുമായുള്ള"), so each run is translated in
				// its own language rather than the whole token being spelled out
				// letter by letter. Comparing foreign-ness to the base language
				// (rather than raw alphabet identity) keeps the language's own
				// script joined, including ranges outside the alphabet table such
				// as Greek Extended.
				const ALPHABET *alpha_c = AlphabetFromChar(c);
				const ALPHABET *alpha_prev = AlphabetFromChar(prev_out);
				bool c_foreign = (alpha_c != NULL) && (alpha_c->offset != tr->letter_bits_offset);
				bool prev_foreign = (alpha_prev != NULL) && (alpha_prev->offset != tr->letter_bits_offset);
				if (emoji_join) {
					// continuation of a ZWJ emoji sequence: not a word boundary
				} else if (emoji_break || !IsAlpha(prev_out) || ShouldSplitIdeographs(tr, c, prev_out) || (IsEmoji(c) != IsEmoji(prev_out)) || (c_foreign != prev_foreign)) {
					if (wcschr(tr->punct_within_word, prev_out) == 0)
						letter_count = 0; // don't reset count for an apostrophy within a word

					if ((prev_out != ' ') && (wcschr(tr->punct_within_word, prev_out) == 0)) {
						// start of word, insert space if not one there already
						c = ' ';
						space_inserted = true;

						if (!IsBracket(prev_out)) // ?? perhaps only set FLAG_NOSPACE for . - /  (hyphenated words, URLs, etc)
							next_word_flags |= FLAG_NOSPACE;
					} else {
						if (iswupper(c))
							word_flags |= FLAG_FIRST_UPPER;

						if ((prev_out == ' ') && iswdigit(sbuf[ix-2]) && !iswdigit(prev_in)) {
							// word, following a number, but with a space between
							// Add an extra space, to distinguish "2 a" from "2a"
							sbuf[ix++] = ' ';
							words[word_count].start++;
						}
					}
				}

				if (c != ' ') {
					letter_count++;

					if (tr->letter_bits_offset > 0) {
						if (((c < 0x250) && (prev_out >= tr->letter_bits_offset)) ||
						    ((c >= tr->letter_bits_offset) && (letter_count > 1) && (prev_out < 0x250))) {
							// Don't mix native and Latin characters in the same word
							// Break into separate words
							if (IsAlpha(prev_out)) {
								c = ' ';
								space_inserted = true;
								word_flags |= FLAG_HYPHEN_AFTER;
								next_word_flags |= FLAG_HYPHEN;
							}
						}
					}
				}

				if (iswupper(c)) {
					c = towlower2(c, tr);

					if (tr->langopts.param[LOPT_CAPS_IN_WORD]) {
						if (syllable_marked == false) {
							char_inserted = c;
							c = 0x2c8; // stress marker
							syllable_marked = true;
						}
					} else {
						if (iswlower(prev_in)) {
							// lower case followed by upper case, possibly CamelCase
							if ((prev_out != ' ') && UpperCaseInWord(tr, &sbuf[ix], c) == 0)
							{ // start a new word
								c = ' ';
								space_inserted = true;
								prev_in_save = c;
							}
						} else if ((c != ' ') && iswupper(prev_in) && iswlower(next_in)) {
							int next2_in;
							utf8_in(&next2_in, &source[source_index + next_in_nbytes]);

							if ((tr->translator_name == L('n', 'l')) && (letter_count == 2) && (c == 'j') && (prev_in == 'I')) {
								// Dutch words may capitalise initial IJ, don't split
							} else if ((prev_out != ' ') && IsAlpha(next2_in)) {
								// changing from upper to lower case, start new word at the last uppercase, if 3 or more letters
								c = ' ';
								space_inserted = true;
								prev_in_save = c;
								next_word_flags |= FLAG_NOSPACE;
							}
						}
					}
				} else {
					if ((all_upper_case) && (letter_count > 2)) {
						// Flag as plural only English
						if (tr->translator_name == L('e', 'n') && (c == 's') && (next_in == ' ')) {
							c = ' ';
							all_upper_case |= FLAG_HAS_PLURAL;

							if (sbuf[ix-1] == '\'')
								sbuf[ix-1] = ' ';
						} else
							all_upper_case = 0; // current word contains lower case letters, not "'s"
					} else
						all_upper_case = 0;
				}
			} else if (c == '-') {
				if (!IsSpace(prev_in) && IsAlpha(next_in)) {
					if (prev_out != ' ') {
						// previous 'word' not yet ended (not alpha or numeric), start new word now.
						c = ' ';
						space_inserted = true;
					} else {
						// '-' between two letters is a hyphen, treat as a space
						word_flags |= FLAG_HYPHEN;
						if (word_count > 0)
							words[word_count-1].flags |= FLAG_HYPHEN_AFTER;
						c = ' ';
					}
				} else if ((prev_in == ' ') && (next_in == ' ')) {
					// ' - ' dash between two spaces, treat as pause
					c = ' ';
					pre_pause_add = 4;
				} else if (next_in == '-') {
					// double hyphen, treat as pause
					source_index++;
					c = ' ';
					pre_pause_add = 4;
				} else if ((prev_out == ' ') && IsAlpha(prev_out2) && !IsAlpha(prev_in)) {
					// insert extra space between a word + space + hyphen, to distinguish 'a -2' from 'a-2'
					sbuf[ix++] = ' ';
					words[word_count].start++;
				}
			} else if (c == '.') {
				if (prev_out == '.') {
					// multiple dots, separate by spaces. Note >3 dots has been replaced by elipsis
					c = ' ';
					space_inserted = true;
				} else if ((word_count > 0) && !(words[word_count-1].flags & FLAG_NOSPACE) && IsAlpha(prev_in)) {
					// dot after a word, with space following, probably an abbreviation
					words[word_count-1].flags |= FLAG_HAS_DOT;

					if (IsSpace(next_in) || (next_in == '-'))
						c = ' '; // remove the dot if it's followed by a space or hyphen, so that it's not pronounced
				}
			} else if (c == '\'') {
				if (((prev_in == '.' && next_in == 's') || iswalnum(prev_in)) && IsAlpha(next_in)) {
					// between two letters, or in an abbreviation (eg. u.s.a.'s). Consider the apostrophe as part of the word
					single_quoted = false;
				} else if ((tr->langopts.param[LOPT_APOSTROPHE] & 1) && IsAlpha(next_in))
					single_quoted = false; // apostrophe at start of word is part of the word
				else if ((tr->langopts.param[LOPT_APOSTROPHE] & 2) && IsAlpha(prev_in))
					single_quoted = false; // apostrophe at end of word is part of the word
				else if ((wcschr(tr->char_plus_apostrophe, prev_in) != 0) && (prev_out2 == ' ')) {
					// consider single character plus apostrophe as a word
					single_quoted = false;
					if (next_in == ' ')
						source_index++; // skip following space
				} else {
					if ((prev_out == 's') && (single_quoted == false)) {
						// looks like apostrophe after an 's'
						c = ' ';
					} else {
						if (IsSpace(prev_out))
							single_quoted = true;
						else
							single_quoted = false;

						pre_pause_add = 4; // single quote
						c = ' ';
					}
				}
			} else if (lookupwchar(breaks, c) != 0)
				c = ' '; // various characters to treat as space
			else if (iswdigit(c)) {
				if (tr->langopts.tone_numbers && IsAlpha(prev_out) && !IsDigit(next_in)) {
				} else if ((prev_out != ' ') && !iswdigit(prev_out)) {
					if ((prev_out != tr->langopts.decimal_sep) || ((decimal_sep_count == true) && (tr->langopts.decimal_sep == ','))) {
						c = ' ';
						space_inserted = true;
					} else
						decimal_sep_count = true;
				} else if ((prev_out == ' ') && IsAlpha(prev_out2) && !IsAlpha(prev_in)) {
					// insert extra space between a word and a number, to distinguish 'a 2' from 'a2'
					sbuf[ix++] = ' ';
					words[word_count].start++;
				}
			}
		}

		if (IsSpace(c)) {
			if (prev_out == ' ') {
				word_flags |= FLAG_MULTIPLE_SPACES;
				continue; // multiple spaces
			}

			if ((cc == 0x09) || (cc == 0x0a))
				next_word_flags |= FLAG_MULTIPLE_SPACES; // tab or newline, not a simple space

			if (space_inserted) {
				// count the number of characters since the start of the word
				j = 0;
				k = source_index - 1;
				while ((k >= source_index_word) && (charix[k] != 0)) {
					if (charix[k] > 0) // don't count initial bytes of multi-byte character
						j++;
					k--;
				}
				words[word_count].length = j;
			}

			source_index_word = source_index;

			// end of 'word'
			sbuf[ix++] = ' ';

			if ((word_count < N_CLAUSE_WORDS-1) && (ix > words[word_count].start)) {
				if (embedded_count > 0) {
					// there are embedded commands before this word
					embedded_list[embedded_ix-1] |= 0x80; // terminate list of commands for this word
					words[word_count].flags |= FLAG_EMBEDDED;
					embedded_count = 0;
				}
				if (alpha_count == 0) {
					all_upper_case &= ~FLAG_ALL_UPPER;
				}
				words[word_count].pre_pause = pre_pause;
				words[word_count].flags |= (all_upper_case | word_flags | word_emphasis);

				if (pre_pause > 0) {
					// insert an extra space before the word, to prevent influence from previous word across the pause
					for (j = ix; j > words[word_count].start; j--)
						sbuf[j] = sbuf[j-1];
					sbuf[j] = ' ';
					words[word_count].start++;
					ix++;
				}

				word_count++;
				words[word_count].start = ix;
				words[word_count].flags = 0;

				words[word_count].length = CalcWordLength(source_index, charix_top, charix, words, word_count);

				word_flags = next_word_flags;
				next_word_flags = 0;
				pre_pause = 0;
				all_upper_case = FLAG_ALL_UPPER;
				alpha_count = 0;
				syllable_marked = false;
			}

			if (space_inserted) {
				source_index = prev_source_index; // rewind to the previous character
				char_inserted = 0;
				space_inserted = false;
			}
		} else {
			if ((ix < (N_TR_SOURCE - 4)))
				ix += utf8_out(c, &sbuf[ix]);
		}
		if (pre_pause_add > pre_pause)
			pre_pause = pre_pause_add;
		pre_pause_add = 0;
	}

	if ((word_count == 0) && (embedded_count > 0)) {
		// add a null 'word' to carry the embedded command flag
		embedded_list[embedded_ix-1] |= 0x80;
		words[word_count].flags |= FLAG_EMBEDDED;
		word_count = 1;
	}

	tr->clause_end = &sbuf[ix-1];
	sbuf[ix] = 0;
	words[0].pre_pause = 0; // don't add extra pause at beginning of clause
	words[word_count].pre_pause = 8;
	if (word_count > 0) {
		ix = word_count-1;
		while ((ix > 0) && (IsBracket(sbuf[words[ix].start])))
			ix--; // the last word is a bracket, mark the previous word as last
		words[ix].flags |= FLAG_LAST_WORD;

		// FLAG_NOSPACE check to avoid recognizing  .mr  -mr
		if ((terminator & CLAUSE_DOT_AFTER_LAST_WORD) && !(words[word_count-1].flags & FLAG_NOSPACE))
			words[word_count-1].flags |= FLAG_HAS_DOT;
	}
	words[0].flags |= FLAG_FIRST_WORD;

	// Each TranslateWord2 may require up to 7 phonemes
	// and after this loop we require 2 phonemes
	for (ix = 0; ix < word_count && (n_ph_list2 < N_PHONEME_LIST-7-2); ix++) {
		int nx;
		int c_temp;
		char *pn;
		char *pw;
		char number_buf[150];
		WORD_TAB num_wtab[N_CLAUSE_WORDS]; // copy of 'words', when splitting numbers into parts

		// start speaking at a specified word position in the text?
		count_words++;
		if (skip_words > 0) {
			skip_words--;
			if (skip_words == 0)
				skipping_text = false;
		}
		if (skipping_text)
			continue;

		current_alphabet = NULL;

		// digits should have been converted to Latin alphabet ('0' to '9')
		word = pw = &sbuf[words[ix].start];

		if (iswdigit(word[0]) && (tr->langopts.break_numbers != BREAK_THOUSANDS)) {
			// Languages with 100000 numbers.  Remove thousands separators so that we can insert them again later
			pn = number_buf;
			while (pn < &number_buf[sizeof(number_buf)-20]) {
				if (iswdigit(*pw))
					*pn++ = *pw++;
				else if ((*pw == tr->langopts.thousands_sep) && (pw[1] == ' ')
				           && iswdigit(pw[2]) && (pw[3] != ' ') && (pw[4] != ' ')) { // don't allow only 1 or 2 digits in the final part
					pw += 2;
					ix++; // skip "word"
				} else {
					nx = pw - word;
					memset(word, ' ', nx);
					nx = pn - number_buf;
					memcpy(word, number_buf, nx);
					break;
				}
			}
			pw = word;
		}

		for (n_digits = 0; iswdigit(word[n_digits]); n_digits++) // count consecutive digits
			;

		if (n_digits > 4 && n_digits <= 32) {
			// word is entirely digits, insert commas and break into 3 digit "words"
			int nw = 0;
			int num_wtab_count;

			number_buf[0] = ' ';
			number_buf[1] = ' ';
			number_buf[2] = ' ';
			pn = &number_buf[3];
			nx = n_digits;

			if ((n_digits > tr->langopts.max_digits) || (word[0] == '0'))
				words[ix].flags |= FLAG_INDIVIDUAL_DIGITS;

			while (pn < &number_buf[sizeof(number_buf)-20] && nw < N_CLAUSE_WORDS-2) {
				if (!IsDigit09(c = *pw++) && (c != tr->langopts.decimal_sep))
					break;

				*pn++ = c;
				nx--;
				if ((nx > 0) && (tr->langopts.break_numbers & (1U << nx))) {
					memcpy(&num_wtab[nw++], &words[ix], sizeof(WORD_TAB)); // copy the 'words' entry for each word of numbers

					if (tr->langopts.thousands_sep != ' ')
						*pn++ = tr->langopts.thousands_sep;
					*pn++ = ' ';

					if ((words[ix].flags & FLAG_INDIVIDUAL_DIGITS) == 0) {
						if (tr->langopts.break_numbers & (1 << (nx-1))) {
							// the next group only has 1 digits, make it three
							*pn++ = '0';
							*pn++ = '0';
						}
						if (tr->langopts.break_numbers & (1 << (nx-2))) {
							// the next group only has 2 digits (eg. Indian languages), make it three
							*pn++ = '0';
						}
					}
				}
			}
			pw--;
			memcpy(&num_wtab[nw], &words[ix], sizeof(WORD_TAB)*2); // the original number word, and the word after it
			num_wtab_count = nw + 2;

			for (j = 1; j <= nw; j++)
				num_wtab[j].flags &= ~(FLAG_MULTIPLE_SPACES | FLAG_EMBEDDED); // don't use these flags for subsequent parts when splitting a number

			// include the next few characters, in case there are an ordinal indicator or other suffix
			strncpy(pn, pw, 16);
			pn[16] = 0;
			nw = 0;

			for (pw = &number_buf[3]; pw < pn && nw < N_CLAUSE_WORDS;) {
				// keep wflags for each part, for FLAG_HYPHEN_AFTER
				dict_flags = TranslateWord2WithContext(tr, pw, &num_wtab[nw], num_wtab_count - nw, words[ix].pre_pause, number_buf, (size_t)(pn+17-number_buf));
				nw++;
				while (pw < pn && *pw++ != ' ')
					;
				words[ix].pre_pause = 0;
			}
		} else {
			pre_pause = 0;

			dict_flags = TranslateWord2WithContext(tr, word, &words[ix], word_count - ix, words[ix].pre_pause, sbuf, (size_t)(tr->clause_end-sbuf+2));

			if (pre_pause > words[ix+1].pre_pause) {
				words[ix+1].pre_pause = pre_pause;
				pre_pause = 0;
			}

			if (dict_flags & FLAG_SPELLWORD) {
				// redo the word, speaking single letters
				for (pw = word; *pw != ' ';) {
					memset(number_buf, 0, sizeof(number_buf));
					memset(number_buf+1, ' ', 9);
					nx = utf8_in(&c_temp, pw);
					memcpy(&number_buf[3], pw, nx);
					TranslateWord2WithContext(tr, &number_buf[3], &words[ix], word_count - ix, 0, number_buf, sizeof(number_buf));
					pw += nx;
				}
			}

			if ((dict_flags & (FLAG_ALLOW_DOT | FLAG_NEEDS_DOT)) && (ix == word_count - 1 - dictionary_skipwords) && (terminator & CLAUSE_DOT_AFTER_LAST_WORD)) {
				// probably an abbreviation such as Mr. or B. rather than end of sentence
				clause_pause = 10;
				if (tone_out != NULL)
					*tone_out = 4;
			}
		}

		if (dict_flags & FLAG_SKIPWORDS) {
			// dictionary indicates skip next word(s)
			while (dictionary_skipwords > 0) {
				if (dictionary_skipwords >= word_count - ix) {
					dictionary_skipwords = 0;
					break;
				}
				words[ix+dictionary_skipwords].flags |= FLAG_DELETE_WORD;
				dictionary_skipwords--;
			}
		}
	}

	if (embedded_read < embedded_ix) {
		// any embedded commands not yet processed?
		Word_EmbeddedCmd();
	}

	for (ix = 0; ix < 2; ix++) {
		// terminate the clause with 2 PAUSE phonemes
		PHONEME_LIST2 *p2;
		p2 = &ph_list2[n_ph_list2 + ix];
		p2->phcode = phonPAUSE;
		p2->stresslevel = 0;
		p2->sourceix = source_index;
		p2->synthflags = 0;
	}
	n_ph_list2 += 2;

	if (Eof() && ((word_count == 0) || (option_endpause == 0)))
		clause_pause = 10;

	MakePhonemeList(tr, clause_pause, new_sentence2);
	phoneme_list[N_PHONEME_LIST].ph = NULL; // recognize end of phoneme_list array, in Generate()
	phoneme_list[N_PHONEME_LIST].sourceix = 1;

	if (embedded_count) { // ???? is this needed
		phoneme_list[n_phoneme_list-2].synthflags = SFLAG_EMBEDDED;
		embedded_list[embedded_ix-1] |= 0x80;
		embedded_list[embedded_ix] = 0x80;
	}

	new_sentence = false;
	if (terminator & CLAUSE_TYPE_SENTENCE)
		new_sentence = true; // next clause is a new sentence

	if (voice_change != NULL) {
		// return new voice name if an embedded voice change command terminated the clause
		if (terminator & CLAUSE_TYPE_VOICE_CHANGE)
			*voice_change = voice_change_name;
		else
			*voice_change = NULL;
	}
}
/* End retained clause policy. */
#else
/* Begin native clause projections. */
typedef struct { Translator *tr; int *tone,*terminator; char **voice; } RustClauseHost;
static char clause_voice_name[40];
static int ClauseValue(void *opaque,unsigned field)
{
    RustClauseHost *h=opaque;switch(field){
    case 0:return (int)(embedded_ix);
    case 1:return (int)(embedded_read);
    case 2:return (int)(pre_pause);
    case 3:return (int)(any_stressed_words);
    case 4:return (int)(count_characters);
    case 5:return (int)(clause_start_char);
    case 6:return (int)(count_words);
    case 7:return (int)(clause_start_word);
    case 8:return (int)(max_clause_pause);
    case 9:return (int)(new_sentence);
    case 10:return (int)(count_sentences);
    case 11:return (int)(skip_sentences);
    case 12:return (int)(skipping_text);
    case 13:return (int)(n_ph_list2);
    case 14:return (int)(h->tr->prev_last_stress);
    case 15:return (int)(h->tr->prepause_timeout);
    case 16:return (int)(h->tr->expect_verb);
    case 17:return (int)(h->tr->expect_noun);
    case 18:return (int)(h->tr->expect_past);
    case 19:return (int)(h->tr->expect_verb_s);
    case 20:return (int)(h->tr->phonemes_repeat_count);
    case 21:return (int)(h->tr->end_stressed_vowel);
    case 22:return (int)(h->tr->prev_dict_flags[0]);
    case 23:return (int)(h->tr->prev_dict_flags[1]);
    case 24:return (int)(h->tr->clause_terminator);
    case 25:return (int)(h->tr->langopts.tone_numbers);
    case 26:return (int)(option_sayas2);
    case 27:return (int)(count_sayas_digits);
    case 28:return (int)(option_phoneme_input);
    case 29:return (int)(h->tr->letter_bits_offset);
    case 30:return (int)(h->tr->langopts.param[LOPT_CAPS_IN_WORD]);
    case 31:return (int)(h->tr->translator_name);
    case 32:return (int)(h->tr->langopts.param[LOPT_APOSTROPHE]);
    case 33:return (int)(word_emphasis);
    case 34:return (int)(h->tr->langopts.break_numbers);
    case 35:return (int)(h->tr->langopts.thousands_sep);
    case 36:return (int)(h->tr->langopts.decimal_sep);
    case 37:return (int)(h->tr->langopts.max_digits);
    case 38:return (int)(skip_words);
    case 39:return (int)(dictionary_skipwords);
    case 40:return (int)(option_endpause);
    case 41:return (int)(n_phoneme_list);
    case 42:return (int)(h->tr->langopts.ideographs);
    case 43:return (int)(h->tr->langopts.alt_alphabet);
    case 44:return (int)(0);
    default:return 0;}
}
static int ClauseStore(void *opaque,unsigned field,int value)
{
    RustClauseHost *h=opaque;
    if((field==0||field==1)&&(value<0||value>N_EMBEDDED_LIST))return -1;
    if(field==13&&(value<0||value>N_PHONEME_LIST))return -1;
    switch(field){
    case 0:embedded_ix=value;break;
    case 1:embedded_read=value;break;
    case 2:pre_pause=value;break;
    case 3:any_stressed_words=value;break;
    case 4:count_characters=value;break;
    case 5:clause_start_char=value;break;
    case 6:count_words=value;break;
    case 7:clause_start_word=value;break;
    case 8:max_clause_pause=value;break;
    case 9:new_sentence=value;break;
    case 10:count_sentences=value;break;
    case 11:skip_sentences=value;break;
    case 12:skipping_text=value;break;
    case 13:n_ph_list2=value;break;
    case 14:h->tr->prev_last_stress=value;break;
    case 15:h->tr->prepause_timeout=value;break;
    case 16:h->tr->expect_verb=value;break;
    case 17:h->tr->expect_noun=value;break;
    case 18:h->tr->expect_past=value;break;
    case 19:h->tr->expect_verb_s=value;break;
    case 20:h->tr->phonemes_repeat_count=value;break;
    case 21:h->tr->end_stressed_vowel=value;break;
    case 22:h->tr->prev_dict_flags[0]=value;break;
    case 23:h->tr->prev_dict_flags[1]=value;break;
    case 24:h->tr->clause_terminator=value;break;
    case 26:option_sayas2=value;break;
    case 27:count_sayas_digits=value;break;
    case 38:skip_words=value;break;
    case 39:dictionary_skipwords=value;break;
    case 44:if(value!=0)return -1;current_alphabet=NULL;break;
    default:return -1;}return 0;
}
static int ClauseByte(void *opaque,size_t index){(void)opaque;return index<sizeof(source)?(unsigned char)source[index]:-1;}
static int ClauseWrite(void *opaque,size_t index,unsigned char value){(void)opaque;if(index>=sizeof(source))return -1;source[index]=(char)value;return 0;}
static int ClauseClass(void *opaque,unsigned code,unsigned kind)
{
    RustClauseHost *h=opaque;switch(kind){case 0:return IsAlpha(code);case 1:return IsSpace(code);
    case 2:return iswupper(code)!=0;case 3:return iswlower(code)!=0;case 4:return iswdigit(code)!=0;
    case 5:return iswalnum(code)!=0;case 6:return IsBracket(code);case 7:return IsEmoji(code);
    case 8:return IsEmojiModifier(code);case 9:return IsRegionalIndicator(code);case 10:return IsEmojiTag(code);
    case 11:return wcschr(h->tr->punct_within_word,code)!=NULL;
    case 12:return wcschr(h->tr->char_plus_apostrophe,code)!=NULL;
    case 13:return lookupwchar(breaks,code)!=0;case 14:return isspace2(code);case 15:return IsDigit(code);
    default:return 0;}
}
static unsigned ClauseLower(void *opaque,unsigned code){RustClauseHost *h=opaque;return towlower2(code,h->tr);}
static int ClauseRead(void *opaque,int16_t *positions,char *name,RustClauseRead *out)
{
    RustClauseHost *h=opaque;int top=0,tone=0;
    int term=ReadClause(h->tr,source,positions,&top,N_TR_SOURCE,&tone,clause_voice_name);
    if(top<0||top>=N_TR_SOURCE)return -1;
    const char *end=memchr(clause_voice_name,0,sizeof(clause_voice_name));if(end==NULL)return -1;
    memcpy(name,clause_voice_name,(size_t)(end-clause_voice_name)+1);
    *out=(RustClauseRead){term,tone,(size_t)top};return 0;
}
static int ClauseEmbeddedCommand(void *opaque,size_t *index,int *result)
{
    (void)opaque;if(*index>=sizeof(source))return -1;unsigned ix=(unsigned)*index;
    *result=EmbeddedCommand(&ix);*index=ix;return ix<=sizeof(source)?0:-1;
}
static int ClauseChar(void *opaque,size_t index,unsigned previous,unsigned code,unsigned next,unsigned *insert,unsigned *flags,unsigned *result)
{
    RustClauseHost *h=opaque;if(index>=sizeof(source))return -1;int in=(int)*insert,wf=(int)*flags;
    *result=TranslateChar(h->tr,source+index,(int)previous,code,next,&in,&wf);*insert=(unsigned)in;*flags=(unsigned)wf;return 0;
}
static int ClauseUpper(void *opaque,char *text,size_t index,unsigned code,int *result)
{
    RustClauseHost *h=opaque;if(index<3||index>=N_TR_SOURCE)return -1;
    *result=UpperCaseInWord(h->tr,text+index,(int)code);return 0;
}
static int ClauseEntry(void *opaque,int index,void *out){(void)opaque;if(index<0||index>=N_PHONEME_LIST)return -1;memcpy(out,&ph_list2[index],sizeof(PHONEME_LIST2));return 0;}
static int ClausePut(void *opaque,int index,const void *in){(void)opaque;if(index<0||index>=N_PHONEME_LIST)return -1;memcpy(&ph_list2[index],in,sizeof(PHONEME_LIST2));return 0;}
static int ClauseCommand(void *opaque,int index,unsigned *out){(void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;*out=embedded_list[index];return 0;}
static int ClausePutCommand(void *opaque,int index,unsigned value){(void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;embedded_list[index]=value;return 0;}
static int ClauseTranslate(void *opaque,char *clause,size_t end,char *number,size_t initialized,size_t offset,RustClauseRow *rows,size_t row,size_t remaining,int pause,unsigned *flags)
{
    RustClauseHost *h=opaque;
    if(end>=N_TR_SOURCE-1||row>=N_CLAUSE_WORDS||remaining>N_CLAUSE_WORDS-row)return -1;
    char *base=number!=NULL?number:clause;size_t length=number!=NULL?initialized:end+2;
    if((number!=NULL&&(initialized==0||initialized>150))||offset>=length)return -1;
    char *saved_end=h->tr->clause_end;h->tr->clause_end=clause+end;
    *flags=TranslateWord2WithContext(h->tr,base+offset,(WORD_TAB *)(rows+row+1),(int)remaining,pause,base,length);
    h->tr->clause_end=saved_end;return 0;
}
static int ClauseWordEmbedded(void *opaque){(void)opaque;Word_EmbeddedCmd();return 0;}
static int ClauseEof(void *opaque){(void)opaque;return Eof();}
static int ClauseMakeList(void *opaque,int pause,int sentence){RustClauseHost *h=opaque;MakePhonemeList(h->tr,pause,sentence!=0);return 0;}
static int ClauseOutput(void *opaque,size_t index,unsigned field,unsigned value)
{
    (void)opaque;if(index>N_PHONEME_LIST)return -1;
    switch(field){case 0:if(value!=0)return -1;phoneme_list[index].ph=NULL;break;
    case 1:phoneme_list[index].sourceix=value;break;case 2:phoneme_list[index].synthflags=value;break;default:return -1;}return 0;
}
static int ClauseResult(void *opaque,unsigned kind,int value,const char *name)
{
    RustClauseHost *h=opaque;
    if(kind==0){if(h->terminator!=NULL)*h->terminator=value;}
    else if(kind==1){if(h->tone!=NULL)*h->tone=value;}
    else if(kind==2){if(value!=0){const char *end=memchr(name,0,40);if(end==NULL)return -1;
        memcpy(clause_voice_name,name,(size_t)(end-name)+1);if(h->voice!=NULL)*h->voice=clause_voice_name;}else if(h->voice!=NULL)*h->voice=NULL;}
    else if(kind!=2)return -1;return 0;
}
_Static_assert(sizeof(RustClauseRow)==sizeof(WORD_TAB),"clause row size");
_Static_assert(offsetof(RustClauseRow,start)==offsetof(WORD_TAB,start),"clause row start");
_Static_assert(offsetof(RustClauseRow,pause)==offsetof(WORD_TAB,pre_pause),"clause row pause");
_Static_assert(offsetof(RustClauseRow,source_index)==offsetof(WORD_TAB,sourceix),"clause row source");
_Static_assert(offsetof(RustClauseRow,length)==offsetof(WORD_TAB,length),"clause row length");
void TranslateClauseWithTerminator(Translator *tr,int *tone,char **voice,int *terminator)
{
    if(tr==NULL)return;RustClauseHost h={tr,tone,terminator,voice};
    RustTranslateClause callbacks={&h,ClauseValue,ClauseStore,ClauseByte,ClauseWrite,ClauseClass,ClauseLower,
        ClauseRead,ClauseEmbeddedCommand,ClauseChar,ClauseUpper,ClauseEntry,ClausePut,ClauseCommand,
        ClausePutCommand,ClauseTranslate,ClauseWordEmbedded,ClauseEof,ClauseMakeList,ClauseOutput,ClauseResult};
    (void)espeak_rs_translate_clause(&callbacks);
}
/* End native clause projections. */
#endif

void TranslateClause(Translator *tr, int *tone_out, char **voice_change)
{
	TranslateClauseWithTerminator(tr, tone_out, voice_change, NULL);
}

#if !defined(USE_RUST_CORE) || defined(ESPEAK_RUST_CORE_TEST)
/* Begin retained clause length helper. */
static int CalcWordLength(int source_index, int charix_top, short int *charix, WORD_TAB *words, int word_count) {
	int j;
	int k;

	for (j = source_index; j < charix_top && charix[j] <= 0; j++); // skip blanks
	words[word_count].sourceix = charix[j];
	k = 0;
	while (charix[j] != 0) {
		// count the number of characters (excluding multibyte continuation bytes)
		if (charix[j++] != -1)
			k++;
	}
	return k;
	}

/* End retained clause length helper. */
#endif
static void CombineFlag(Translator *tr, WORD_TAB *wtab, int wtab_remaining, char *word, int *flags, unsigned char *p, char *word_phonemes) {
	// combine a preposition with the following word


	int sylimit; // max. number of syllables in a word to be combined with a preceding preposition
	sylimit = tr->langopts.param[LOPT_COMBINE_WORDS];


	char *p2;
	p2 = word;
	while (*p2 != ' ') p2++;

	bool ok = true;
	int c_word2;

	if (wtab_remaining <= 1)
		ok = false;

	utf8_in(&c_word2, p2+1); // first character of the next word;

	if (!iswalpha(c_word2))
		ok = false;

	int flags2[2];
    flags2[0] = 0;


	if (ok) {
		char ph_buf[N_WORD_PHONEMES];
		strcpy(ph_buf, word_phonemes);

		flags2[0] = TranslateWordWithBounds(tr, p2+1, wtab+1, wtab_remaining-1, NULL, 0);
		if ((flags2[0] & FLAG_WAS_UNPRONOUNCABLE) || (word_phonemes[0] == phonSWITCH))
			ok = false;

		if ((sylimit & 0x100) && ((flags2[0] & FLAG_ALT_TRANS) == 0)) {
			// only if the second word has $alt attribute
			ok = false;
		}

		if ((sylimit & 0x200) && (wtab_remaining > 1) && ((wtab+1)->flags & FLAG_LAST_WORD)) {
			// not if the next word is end-of-sentence
			ok = false;
		}

		if (ok == false)
			strcpy(word_phonemes, ph_buf);
	}

	if (ok) {
		*p2 = '-'; // replace next space by hyphen
		wtab[0].flags &= ~FLAG_ALL_UPPER; // prevent it being considered an abbreviation
		*flags = TranslateWordWithBounds(translator, word, wtab, wtab_remaining, NULL, 0); // translate the combined word
		if ((sylimit > 0) && (CountSyllables(p) > (sylimit & 0x1f))) {
			// revert to separate words
			*p2 = ' ';
			*flags = TranslateWordWithBounds(translator, word, wtab, wtab_remaining, NULL, 0);
		} else {
			if (*flags == 0)
				*flags = flags2[0]; // no flags for the combined word, so use flags from the second word eg. lang-hu "nem december 7-e"
			*flags |= FLAG_SKIPWORDS;
			dictionary_skipwords = 1;
		}
	}
}

static void SwitchLanguage(char *word, char *word_phonemes) {
	char lang_name[12];
	int ix;

	word += 3;

	for (ix = 0;;) {
		int  c1;
		c1 = *word++;
		if ((c1 == ' ') || (c1 == 0))
			break;
		lang_name[ix++] = tolower(c1);
	}
	lang_name[ix] = 0;

	if ((ix = LookupPhonemeTable(lang_name)) > 0) {
		SelectPhonemeTable(ix);
		word_phonemes[0] = phonSWITCH;
		word_phonemes[1] = ix;
		word_phonemes[2] = 0;
	}
}

void InitText(int control)
{
	count_sentences = 0;
	count_words = 0;
	end_character_position = 0;
	skip_sentences = 0;
	skip_marker[0] = 0;
	skip_words = 0;
	skip_characters = 0;
	skipping_text = false;
	new_sentence = true;

	option_sayas = 0;
	option_sayas2 = 0;
	option_emphasis = 0;
	word_emphasis = 0;
	embedded_flag = 0;

	InitText2();

	if ((control & espeakKEEP_NAMEDATA) == 0)
		InitNamedata();
}
