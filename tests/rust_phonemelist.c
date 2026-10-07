/* Native clause phoneme lists against independently extracted retained C.
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
#include "phonemelist.h"
#include "synthdata.h"
#include "synthesize.h"
#include "translate.h"
#include "speech.h"
#include "rust_data.h"
int reference_n2;
PHONEME_LIST2 reference_list2[N_PHONEME_LIST];
static PHONEME_LIST reference_list[N_PHONEME_LIST+1];
static int reference_n;
#define MakePhonemeList ReferenceMakePhonemeList
#define n_ph_list2 reference_n2
#define ph_list2 reference_list2
#define phoneme_list reference_list
#define n_phoneme_list reference_n
#include "phonemelist_reference.inc"
#undef MakePhonemeList
#undef n_ph_list2
#undef ph_list2
#undef phoneme_list
#undef n_phoneme_list
extern int n_ph_list2;
extern PHONEME_LIST2 ph_list2[N_PHONEME_LIST];
static PHONEME_LIST2 generated[N_PHONEME_LIST];
static unsigned seed=0x3c6ef372u;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t compared,entries,switches,replacements,dropped;
static unsigned char codes[N_PHONEME_TAB];static int n_codes,n_tables;
static int random_code(void)
{
	if(next()%40==0)return next()%N_PHONEME_TAB; // possibly absent from the table
	return codes[next()%n_codes];
}
// A first-stage list: words of random phonemes, table switches, then the two
// terminating pauses and readable entries after them.
static int random_list(Translator *tr)
{
	int n=next()%10==0?(int)(next()%(N_PHONEME_LIST-60))+3:(int)(next()%50)+3;
	memset(generated,0,sizeof(generated));
	for(int ix=0;ix<n+2 && ix<N_PHONEME_LIST;ix++) {
		PHONEME_LIST2 *p=&generated[ix];
		p->phcode=random_code();
		p->stresslevel=next()%8==0?(unsigned char)next():(unsigned char)(next()%8);
		p->sourceix=next()%4==0?(unsigned short)(1+next()%500):0;
		p->synthflags=(unsigned short)(next()%3==0?next()&0x7f:0);
		p->wordstress=next()%8;
		p->tone_ph=next()%6==0?(unsigned char)random_code():0;
		if(next()%200==0 && n_tables>1 && ix+3<n) {
			// a run of switches, some redundant, carrying a word start
			for(int k=0;k<3;k++){generated[ix+k].phcode=phonSWITCH;generated[ix+k].tone_ph=next()%n_tables;generated[ix+k].sourceix=next()%2?(unsigned short)(1+next()%500):0;generated[ix+k].synthflags=0;}
			ix+=2;switches+=3;continue;
		}
		if(next()%30==0 && n_tables>1) {
			p->phcode=phonSWITCH;
			p->tone_ph=next()%3==0?tr->phoneme_tab_ix:(unsigned char)(next()%n_tables);
			switches++;
		}
	}
	generated[0].phcode=phonPAUSE;
	for(int ix=n-2;ix<n;ix++){generated[ix].phcode=phonPAUSE;generated[ix].stresslevel=0;generated[ix].synthflags=0;}
	return n;
}
static void random_replacements(void)
{
	n_replace_phonemes=next()%3==0?(int)(next()%6):0;
	for(int ix=0;ix<n_replace_phonemes;ix++) {
		replace_phonemes[ix].old_ph=codes[next()%n_codes];
		replace_phonemes[ix].new_ph=next()%4==0?0:codes[next()%n_codes];
		replace_phonemes[ix].type=(char)(next()%8);
	}
	replacements+=n_replace_phonemes;
}
static void compare(Translator *tr)
{
	int n=random_list(tr),post_pause=next()%200;bool start=next()%2;
	random_replacements();
	memcpy(reference_list2,generated,sizeof(generated));reference_n2=n;
	memcpy(ph_list2,generated,sizeof(generated));n_ph_list2=n;
	memset(reference_list,0x5a,sizeof(reference_list));memset(phoneme_list,0x5a,sizeof(phoneme_list));
	int start_table=next()%n_tables;
	SelectPhonemeTable(start_table);ReferenceMakePhonemeList(tr,post_pause,start);
	SelectPhonemeTable(start_table);MakePhonemeList(tr,post_pause,start);
	TEST_ASSERT(n_ph_list2==reference_n2);
	TEST_ASSERT(memcmp(ph_list2,reference_list2,sizeof(generated))==0);
	if(n_phoneme_list!=reference_n || memcmp(phoneme_list,reference_list,sizeof(reference_list))) {
		fprintf(stderr,"lang=%x n2=%d: produced %d/%d\n",tr->translator_name,n,n_phoneme_list,reference_n);
		for(int ix=0;ix<=N_PHONEME_LIST;ix++)if(memcmp(&phoneme_list[ix],&reference_list[ix],sizeof(PHONEME_LIST))) {
			const PHONEME_LIST *a=&phoneme_list[ix],*b=&reference_list[ix];
			fprintf(stderr,"entry %d: code %d/%d ph %p/%p type %d/%d stress %d/%d ws %d/%d flags %x/%x src %d/%d nw %d/%d len %u/%u tone %d/%d %p/%p\n",ix,
				a->phcode,b->phcode,(void*)a->ph,(void*)b->ph,a->type,b->type,a->stresslevel,b->stresslevel,a->wordstress,b->wordstress,
				a->synthflags,b->synthflags,a->sourceix,b->sourceix,a->newword,b->newword,a->length,b->length,a->tone_ph,b->tone_ph,
				(void*)a->tone_ph_data,(void*)b->tone_ph_data);
			if(ix>8)break;
		}
		TEST_ASSERT(false);
	}
	compared++;entries+=n_phoneme_list;
	for(int ix=0;ix<n;ix++)if(phoneme_tab[generated[ix].phcode]==NULL)dropped++;
}
int main(void)
{
	_Static_assert(sizeof(RustPhonemeListOutput)==28,"phoneme list output layout");
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050);
	for(n_tables=0;n_tables<N_PHONEME_TABS && phoneme_tab_list[n_tables].n_phonemes>0;n_tables++);
	static const char *languages[]={"en","de","fr","es","it","pl","ru","nl","sv","hi","vi","cmn","hu","cs","sk","ca","pt","ta"};
	int saved_wordgap=option_wordgap;
	for(size_t language=0;language<sizeof(languages)/sizeof(*languages);language++) {
		Translator *tr=SelectTranslator(languages[language]);TEST_ASSERT(tr);
		int table=LookupPhonemeTable(languages[language]);
		if(table>=0)tr->phoneme_tab_ix=table;
		SelectPhonemeTable(tr->phoneme_tab_ix);n_codes=0;
		for(int code=1;code<n_phoneme_tab;code++)if(phoneme_tab[code] && code!=phonSWITCH)codes[n_codes++]=code;
		LANGUAGE_OPTIONS saved=tr->langopts;
		for(int trial=0;trial<400;trial++) {
			tr->langopts=saved;
			if(next()%3==0) {
				static const int regressions[]={0,0x01,0x02,0x03,0x0b,0x12,0x104,0x103,0x22};
				tr->langopts.param[LOPT_REGRESSIVE_VOICING]=regressions[next()%9];
			}
			if(next()%3==0)tr->langopts.stress_flags^=(next()%2?0x08:0)|(next()%2?S_NO_DIM:0);
			if(next()%3==0)tr->langopts.vowel_pause=next()&0x30f;
			if(next()%3==0)tr->langopts.word_gap=next()%8;
			if(next()%4==0)tr->langopts.param[LOPT_REDUCE]=next()%4;
			option_wordgap=next()%6==0?(int)(next()%5):0;
			compare(tr);
		}
		tr->langopts=saved;
		DeleteTranslator(tr);
	}
	option_wordgap=saved_wordgap;n_replace_phonemes=0;
	TEST_ASSERT(espeak_Terminate()==EE_OK);
	printf("Matched %zu clauses (%zu entries, %zu switches, %zu replacements, %zu absent phonemes)\n",
		compared,entries,switches,replacements,dropped);
	return 0;
}
