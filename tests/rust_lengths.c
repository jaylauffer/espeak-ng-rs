/* Native clause lengths and envelope tables against independently extracted
 * retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/encoding.h>
#include "phoneme.h"
#include "setlengths.h"
#include "synthdata.h"
#include "synthesize.h"
#include "translate.h"
#include "voice.h"
#include "rust_data.h"
#define env_fall reference_env_fall
#define envelope_data reference_envelope_data
#include "envelope_reference.inc"
#undef env_fall
#undef envelope_data
static int reference_len_speeds[3];
static PHONEME_TAB *ReferenceTone(const PHONEME_LIST *p){return p->tone_ph_data?p->tone_ph_data:phoneme_tab[p->tone_ph];}
// The engine's DoEmbedded2, with speed computed into the reference factors.
static void ReferenceEmbedded(int *embix)
{
	unsigned int word;
	do {
		word = embedded_list[(*embix)++];
		if ((word & 0x1f) == EMBED_S) {
			RustSonicEffects effects;
			SetEmbedded(word & 0x7f, word >> 8);
			TEST_ASSERT(espeak_rs_speed_configure(voice, &speed, &reference_len_speeds, embedded_value[EMBED_S],
				embedded_value[EMBED_S2], 1, USE_LIBSONIC, &effects) == 0);
		}
	} while ((word & 0x80) == 0);
}
#define CalcLengths ReferenceCalcLengths
#define len_speeds reference_len_speeds
#define TonePhoneme ReferenceTone
#define DoEmbedded2 ReferenceEmbedded
#include "lengths_reference.inc"
#undef CalcLengths
#undef len_speeds
#undef TonePhoneme
#undef DoEmbedded2
#define TAIL 8
static PHONEME_LIST generated[N_PHONEME_LIST+1],expected[N_PHONEME_LIST+1];
static unsigned seed=0x6a09e667u;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t compared,vowels,embedded,tones,rejected;
static unsigned char codes[N_PHONEME_TAB];static int n_codes;
static void random_entry(PHONEME_LIST *p)
{
	int code=codes[next()%n_codes];
	memset(p,0,sizeof(*p));
	p->ph=phoneme_tab[code];p->phcode=code;p->type=next()%12?p->ph->type:next()%10;
	bool syllable=p->type==phVOWEL?next()%10!=0:next()%12==0;
	p->synthflags=(unsigned short)((next()&0xfff0&~SFLAG_EMBEDDED)|(syllable?SFLAG_SYLLABLE:0)|(next()%4==0?SFLAG_LENGTHEN:0)|(next()&SFLAG_SEQCONTINUE));
	p->stresslevel=next()%16;
	p->newword=next()%3==0?(unsigned char)(1+next()%15):0;
	p->env=next()%200==0?(unsigned char)(18+next()%6):(unsigned char)(next()%18);
	p->pitch1=next();p->pitch2=next();p->length=next()%400;p->prepause=next()%80;p->amp=next();
	if(next()%6==0){p->tone_ph=codes[next()%n_codes];if(next()%3==0)p->tone_ph_data=phoneme_tab[codes[next()%n_codes]];}
}
// Clause-shaped list: random entries, then the end-of-clause and short pauses
// MakePhonemeList appends, then stale entries a scan may reach.
static int random_list(int *n_embedded)
{
	int n=next()%8==0?(int)(next()%(N_PHONEME_LIST-TAIL-2))+3:(int)(next()%40)+3;
	*n_embedded=0;
	for(int ix=0;ix<n+TAIL;ix++)random_entry(&generated[ix]);
	generated[0].type=phPAUSE;generated[0].synthflags=0;
	for(int ix=1;ix<n-2;ix++)if(next()%20==0){generated[ix].synthflags|=SFLAG_EMBEDDED;(*n_embedded)++;}
	PHONEME_LIST *end=&generated[n-2];
	end->ph=phoneme_tab[phonPAUSE];end->type=phPAUSE;end->newword=PHLIST_END_OF_CLAUSE;end->synthflags=0;end->tone_ph=0;
	end=&generated[n-1];
	end->ph=phoneme_tab[phonPAUSE_SHORT];end->type=phPAUSE;end->newword=0;end->synthflags=0;end->tone_ph=0;
	generated[n].newword|=1;
	generated[n+TAIL].ph=NULL;
	return n;
}
static void random_embedded(int count)
{
	// Speed commands are absolute, or relative with 0x40 (+) / 0x60 (-).
	static const unsigned signs[]={0,0x40,0x60};
	int ix=0;
	for(int group=0;group<count;group++) {
		int commands=1+next()%3;
		for(int c=0;c<commands;c++) {
			unsigned sign=signs[next()%3],value;
			if(next()%3==0)value=EMBED_S|sign|((sign?next()%60:80+next()%300)<<8);
			else value=(EMBED_P+next()%4)|((next()%100)<<8);
			embedded_list[ix++]=value|(c==commands-1?0x80:0);
		}
	}
}
static void compare(Translator *tr)
{
	int n_embedded,n=random_list(&n_embedded);random_embedded(n_embedded);
	SetParameter(espeakRATE,80+next()%420,0);
	RustSonicEffects effects;SPEED_FACTORS saved_speed=speed;int saved_values[N_EMBEDDED_VALUES];
	memcpy(saved_values,embedded_value,sizeof(saved_values));
	TEST_ASSERT(espeak_rs_speed_configure(voice,&speed,&reference_len_speeds,embedded_value[EMBED_S],embedded_value[EMBED_S2],3,USE_LIBSONIC,&effects)==0);
	speed=saved_speed;
	memcpy(phoneme_list,generated,sizeof(PHONEME_LIST)*(n+TAIL+1));n_phoneme_list=n;
	ReferenceCalcLengths(tr);
	memcpy(expected,phoneme_list,sizeof(PHONEME_LIST)*(n+TAIL+1));
	SPEED_FACTORS reference_speed=speed;
	speed=saved_speed;memcpy(embedded_value,saved_values,sizeof(saved_values));
	memcpy(phoneme_list,generated,sizeof(PHONEME_LIST)*(n+TAIL+1));
	CalcLengths(tr);
	TEST_ASSERT(memcmp(&speed,&reference_speed,sizeof(speed))==0);
	if(memcmp(phoneme_list,expected,sizeof(PHONEME_LIST)*(n+TAIL+1))) {
		for(int ix=0;ix<n+TAIL;ix++)if(memcmp(&phoneme_list[ix],&expected[ix],sizeof(PHONEME_LIST)))
			fprintf(stderr,"entry %d/%d lang=%x type=%d flags=%x: length %u/%u prepause %d/%d amp %d/%d pitch %d,%d/%d,%d env %d/%d synthflags %x/%x\n",
				ix,n,tr->translator_name,generated[ix].type,generated[ix].synthflags,phoneme_list[ix].length,expected[ix].length,
				phoneme_list[ix].prepause,expected[ix].prepause,phoneme_list[ix].amp,expected[ix].amp,
				phoneme_list[ix].pitch1,phoneme_list[ix].pitch2,expected[ix].pitch1,expected[ix].pitch2,
				phoneme_list[ix].env,expected[ix].env,phoneme_list[ix].synthflags,expected[ix].synthflags);
		TEST_ASSERT(false);
	}
	compared++;embedded+=n_embedded;
	for(int ix=1;ix<n;ix++)if((generated[ix].synthflags&SFLAG_SYLLABLE)||generated[ix].type==phVOWEL){vowels++;if(generated[ix].tone_ph)tones++;}
}
static void rejection(Translator *tr)
{
	// A scan past the last entry with a phoneme is rejected, list unchanged.
	int n_embedded,n=random_list(&n_embedded);
	for(int ix=0;ix<n;ix++)generated[ix].synthflags&=~SFLAG_EMBEDDED;
	generated[n-1].synthflags|=SFLAG_SYLLABLE;generated[n].ph=NULL;
	memcpy(phoneme_list,generated,sizeof(PHONEME_LIST)*(n+1));n_phoneme_list=n;
	CalcLengths(tr);
	TEST_ASSERT(memcmp(phoneme_list,generated,sizeof(PHONEME_LIST)*(n+1))==0);rejected++;
}
int main(void)
{
	_Static_assert(sizeof(RustLengthEntry)==28 && sizeof(RustLengthSettings)==260,"length snapshot layout");
	for(int env=0;env<N_ENVELOPE_DATA;env++)TEST_ASSERT(memcmp(envelope_data[env],reference_envelope_data[env],128)==0);
	TEST_ASSERT(memcmp(env_fall,reference_env_fall,128)==0);
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050);
	static const char *languages[]={"en","de","fr","es","it","ru","sv","nl","hi","ja","vi","cmn","pl","hu","fi","sw"};
	for(int table=0;table<N_PHONEME_TABS && phoneme_tab_list[table].n_phonemes>0;table++) {
		SelectPhonemeTable(table);n_codes=0;
		for(int code=1;code<n_phoneme_tab;code++)if(phoneme_tab[code])codes[n_codes++]=code;
		if(n_codes==0 || !phoneme_tab[phonPAUSE] || !phoneme_tab[phonPAUSE_SHORT])continue;
		for(size_t language=0;language<sizeof(languages)/sizeof(*languages);language++) {
			Translator *tr=SelectTranslator(languages[language]);TEST_ASSERT(tr);
			LANGUAGE_OPTIONS saved=tr->langopts;
			for(int trial=0;trial<40;trial++) {
				tr->langopts=saved;
				if(next()%2)tr->langopts.word_gap=next()&0x30;
				if(next()%3==0)tr->langopts.stress_flags^=(next()%2?S_EO_CLAUSE1:0)|(next()%2?S_NO_EOC_LENGTHEN:0);
				if(next()%4==0)tr->langopts.lengthen_tonic=next()%40;
				if(next()%4==0)tr->langopts.max_lengthmod=100+next()%500;
				if(next()%4==0)tr->langopts.param[LOPT_MAXAMP_EOC]=next()%30;
				compare(tr);
			}
			rejection(tr);
			tr->langopts=saved;
			DeleteTranslator(tr);
		}
	}
	TEST_ASSERT(espeak_Terminate()==EE_OK);
	printf("Envelope tables match; matched %zu clauses (%zu syllables, %zu with tones, %zu embedded groups); %zu rejected scans unchanged\n",
		compared,vowels,tones,embedded,rejected);
	return 0;
}
