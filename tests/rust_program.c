/* Phoneme VM parity with the retained interpreter, including state effects.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "speech.h"
#include "phoneme.h"
#include "synthesize.h"
#include "synthdata.h"
#include "translate.h"
#include "voice.h"
#include "rust_data.h"
#if USE_MBROLA
#include "mbrola.h"
#endif
static unsigned short *phoneme_index;
#define InterpretPhoneme ReferenceInterpretPhoneme
#include "program_reference.inc"
#undef InterpretPhoneme
static uint32_t seed = 0x8137952b;
static uint32_t random32(void) { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; return seed; }
static unsigned long comparisons;
static void mismatch(PHONEME_TAB *ph, const PHONEME_DATA *expected, const PHONEME_DATA *actual, int control)
{
	if (memcmp(expected, actual, sizeof(*expected))) {
		char name[5];
		fprintf(stderr,"program mismatch phoneme=%s program=%d control=%x\n",WordToString(name,ph->mnemonic),ph->program,control);
		const int *a = (const int *)expected, *b = (const int *)actual;
		for (int ix = 0; ix < 33; ix++) if (a[ix] != b[ix]) fprintf(stderr,"field=%d expected=%x actual=%x\n",ix,a[ix],b[ix]);
		TEST_ASSERT(false);
	}
}
static void real_programs(void)
{
	char filename[N_PATH_BUF]; snprintf(filename,sizeof(filename),"%s/phonindex",path_home);
	FILE *file = fopen(filename,"rb"); TEST_ASSERT(file != NULL);
	TEST_ASSERT(fseek(file,0,SEEK_END) == 0); long length = ftell(file); rewind(file);
	TEST_ASSERT(length > 0 && length % 2 == 0);
	// Give the unchanged oracle defined lookahead for terminal sound records.
	phoneme_index = malloc(length + sizeof(*phoneme_index)); TEST_ASSERT(phoneme_index != NULL);
	TEST_ASSERT(fread(phoneme_index,1,length,file) == (size_t)length); fclose(file);
	phoneme_index[length / 2] = INSTN_RETURN;
	TEST_ASSERT(espeak_SetVoiceByName("en") == EE_OK);
	Translator language = *translator;
	for (int table = 0; table < N_PHONEME_TABS && phoneme_tab_list[table].n_phonemes > 0; table++) {
		SelectPhonemeTable(table);
		for (int code = 0; code < 256; code++) {
			PHONEME_TAB *ph = phoneme_tab[code];
			if (ph == NULL) continue;
			for (int trial = 0; trial < 12; trial++) {
				PHONEME_LIST expected[9] = {0}, actual[9];
				WORD_PH_DATA word_expected = {0}, word_actual;
				for (int ix = 0; ix < 9; ix++) {
					PHONEME_TAB *entry = NULL; int selected = phonPAUSE;
					for (int attempt = 0; attempt < 256 && entry == NULL; attempt++) {
						selected = random32() & 255; entry = phoneme_tab[selected];
					}
					expected[ix].ph = entry; expected[ix].phcode = selected;
					expected[ix].type = entry ? entry->type : phPAUSE;
					expected[ix].stresslevel = random32() & 7;
					expected[ix].wordstress = random32() % 5;
					expected[ix].sourceix = ix == 0 || ix >= 6 || (random32() & 3) == 0;
					expected[ix].synthflags = random32() & (SFLAG_DICTIONARY | SFLAG_NEXT_PAUSE);
				}
				expected[3].ph = ph; expected[3].phcode = code; expected[3].type = ph->type;
				// Explicit word boundaries bound all forward/backward scans.
				if (trial == 1) expected[2].ph = NULL;
				if (trial == 2) expected[4].ph = NULL;
				memcpy(actual,expected,sizeof(actual));
				word_actual = word_expected;
				language.langopts.param[LOPT_REDUCE] = trial & 3;
				Translator *tr = trial & 1 ? &language : NULL;
				int control = (trial & 4 ? 0x100 : 0) | (trial & 2 ? 1 : 0);
				PHONEME_DATA data_expected, data_actual;
				ReferenceInterpretPhoneme(tr,control,expected+3,expected,&data_expected,&word_expected);
				InterpretPhoneme(tr,control,actual+3,actual,&data_actual,&word_actual);
				mismatch(ph,&data_expected,&data_actual,control);
				TEST_ASSERT(memcmp(expected,actual,sizeof(actual)) == 0);
				TEST_ASSERT(memcmp(&word_expected,&word_actual,sizeof(word_actual)) == 0);
				comparisons++;
			}
		}
	}
	free(phoneme_index); phoneme_index = NULL;
	SelectPhonemeTable(0);
}
typedef struct { Translator *tr; PHONEME_LIST *current, *start; } SyntheticHost;
static int synthetic_context(void *opaque, uint32_t kind, size_t value)
{
	SyntheticHost *host = opaque;
	if (kind == 0) return InterpretCondition(host->tr,0,host->current,host->start,phoneme_index+value,NULL);
	if (kind == 1) return value <= 4 && StressCondition(host->tr,host->current,value,1);
	if (kind == 2) return true;
	if (kind == 3) return 28 + (seed % 6);
	if (kind == 4) return 28 + (seed % 6);
	return 0;
}
static void synthetic_programs(void)
{
	TEST_ASSERT(espeak_SetVoiceByName("en") == EE_OK);
	PHONEME_TAB *vowel = NULL;
	for (int code = 0; code < 256 && vowel == NULL; code++)
		if (phoneme_tab[code] != NULL && phoneme_tab[code]->type == phVOWEL) vowel = phoneme_tab[code];
	TEST_ASSERT(vowel != NULL);
	PHONEME_TAB ph = *vowel; ph.program = 1;
	for (int trial = 0; trial < 500; trial++) {
		unsigned short words[128] = {0};
		int pc = 1;
		words[pc++] = 0x0c00 | (random32() & 255); // signed add length
		words[pc++] = 0x0d03; words[pc++] = 0xc9aa; words[pc++] = 0x7800;
		words[pc++] = 0x9201; words[pc++] = 0xabcd;
		words[pc++] = 0x9302; words[pc++] = 0x1234;
		words[pc++] = 0xa123; words[pc++] = 0x7654; words[pc++] = 0xfedc; words[pc++] = 0xba98;
		words[pc++] = 0xa287; words[pc++] = 0x6543; words[pc++] = 0xcdef; words[pc++] = 0x9876;
		words[pc++] = 0x9100; words[pc++] = 100; // procedure
		// Vowel switch has six address/parameter pairs and advances 13 words.
		words[pc++] = 0x6a00;
		for (int ix = 0; ix < 12; ix++) words[pc++] = 0x100 + (random32() & 255);
		words[pc++] = 0x6c00;
		for (int ix = 0; ix < 12; ix++) words[pc++] = 0x100 + (random32() & 255);
		words[pc++] = 0x2182; // thisPh isNotStressed
		if (trial & 1) words[pc++] = i_NOT;
		words[pc++] = 0x6803; words[pc++] = 0x0711; words[pc++] = 0x6002; words[pc++] = 0x0722;
		words[pc++] = 0xdff1; words[pc++] = 0x1234; words[pc++] = INSTN_CONTINUE;
		words[pc++] = 0xeff2; words[pc++] = 0x5678; // signed vowel ending
		words[pc++] = trial & 2 ? 0xcff3 : 0xbff3; words[pc++] = 0x2345;
		words[pc++] = 0xfff4; words[pc++] = 0x6789; words[pc++] = INSTN_RETURN;
		words[100] = 0x051a; words[101] = 0x0923; words[102] = INSTN_RETURN;
		phoneme_index = words;
		PHONEME_LIST expected[4] = {0}, actual[4];
		for (int ix=0; ix<4; ix++) { expected[ix].ph = &ph; expected[ix].phcode = ph.code; expected[ix].type = ph.type; }
		PHONEME_TAB before = ph, after = ph;
		before.end_type = after.start_type = 28 + seed % 6;
		expected[0].ph = &before; expected[2].ph = &after;
		expected[3].sourceix = 1;
		memcpy(actual,expected,sizeof(actual));
		PHONEME_DATA a, b;
		ReferenceInterpretPhoneme(translator,0,expected+1,expected,&a,NULL);
		SyntheticHost host = {translator,actual+1,actual};
		TEST_ASSERT(espeak_rs_phoneme_program((const unsigned char *)words,sizeof(words),&ph,0,1,&host,synthetic_context,&b) == 0);
		mismatch(&ph,&a,&b,0);
		comparisons++;
	}
	phoneme_index = NULL;
}
int main(void)
{
	TEST_ASSERT(sizeof(PHONEME_DATA) == 152);
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0) == 22050);
	real_programs(); printf("Compared %lu real phoneme executions and state updates\n",comparisons);
	comparisons = 0; synthetic_programs(); printf("Compared %lu synthetic procedure, transition and sound programs\n",comparisons);
	TEST_ASSERT(espeak_Terminate() == EE_OK);
	return 0;
}
