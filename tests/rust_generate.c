/* Native clause synthesis driver against independently extracted retained C:
 * both issue the same ordered host effects for the same phoneme lists.
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
#include "synthdata.h"
#include "synthesize.h"
#include "translate.h"
#include "rust_data.h"

typedef struct { int op,index,a,b,c; const unsigned char *env; FMT_PARAMS fmt; PHONEME_DATA data; } Rec;
#define MAX_RECS 40000
static Rec recs[2][MAX_RECS];
static int n_recs[2],side;
static PHONEME_LIST lists[2][N_PHONEME_LIST+1];
static unsigned clause_seed;
static int queue_calls[2],free_budget;
static int pitch_cmd[2];
static bool hooks;

static Rec *rec(int op,int index,int a,int b,int c)
{
	TEST_ASSERT(n_recs[side]<MAX_RECS);
	Rec *r=&recs[side][n_recs[side]++];
	memset(r,0,sizeof(*r));
	r->op=op;r->index=index;r->a=a;r->b=b;r->c=c;
	return r;
}
static unsigned mix(unsigned x){x^=x>>16;x*=0x7feb352du;x^=x>>15;x*=0x846ca68bu;x^=x>>16;return x;}
// GetEnvelope addresses map to distinct fake envelopes.
static unsigned char fake_envelopes[64][128];
static const unsigned char *MockEnvelope(int address){return fake_envelopes[(unsigned)address%64];}
static void fill_data(PHONEME_DATA *d,unsigned h)
{
	memset(d,0,sizeof(*d));
	d->pd_control=(int)(mix(h)&6);
	for(int i=0;i<N_PHONEME_DATA_PARAM;i++)d->pd_param[i]=(int)(mix(h+1+i)%60);
	for(int i=0;i<5;i++){unsigned v=mix(h+20+i);d->sound_addr[i]=v%3==0?0:(int)(v%5000);d->sound_param[i]=(int)(mix(h+30+i)%200)-50;}
	for(int i=0;i<4;i++)d->vowel_transition[i]=(int)mix(h+40+i);
	d->pitch_env=(int)(mix(h+50)%64);d->amp_env=mix(h+51)%3==0?0:(int)(mix(h+52)%64);
}
static int queue_free(void)
{
	// mostly plenty of space, sometimes short so generation suspends
	unsigned v=mix(clause_seed+(unsigned)queue_calls[side]++);
	if(free_budget>0 && v%7==0){free_budget--;return (int)(v%30);}
	return 100;
}
static void spect_effect(FMT_PARAMS *fmt){if(fmt->wav_addr & 1)fmt->wav_addr=0;}

/* Reference hooks: the legacy helpers and queue-layer state. */
static const PHONEME_LIST *ref_base;
static int ref_index(const PHONEME_LIST *p){return (int)(p-ref_base);}
static void RefPause(int length,int control){rec(10,0,length,control,0);}
static void RefPitch(const unsigned char *env,int p1,int p2){rec(12,0,p1,p2,0)->env=env;pitch_cmd[0]=1;}
static void RefAmplitude(int amp,const unsigned char *env){rec(11,0,amp,0,0)->env=env;}
static int RefSpect(PHONEME_TAB *ph,int which,FMT_PARAMS *fmt,PHONEME_LIST *plist,int modulation)
{
	TEST_ASSERT(ph==plist->ph);
	rec(16,ref_index(plist),which,modulation,0)->fmt=*fmt;spect_effect(fmt);return 0;
}
static int RefSample(PHONEME_DATA *data,int length_mod,int amp){rec(17,0,length_mod,amp,0)->data=*data;return 0;}
static void RefMarker(int type,int position,int length,int value){rec(6,type,position,length,value);}
static void RefPhonemeMarker(int type,int position,int length,char *name)
{
	int index,ipa;TEST_ASSERT(type==espeakEVENT_PHONEME && length==0 && sscanf(name,"%d:%d",&index,&ipa)==2);
	rec(7,index,ipa,position,0);
}
static void RefEmbedded(int *embix,int source){rec(4,0,*embix,source,0);*embix+=1+(source&1);}
static void RefStartSyllable(void){rec(13,0,0,0,0);}
static void RefEndAmplitude(void){rec(8,0,0,0,0);}
static void RefEndPitch(int voice_break){rec(9,0,voice_break,0,0);}
static void RefInterpret(Translator *tr,int control,PHONEME_LIST *plist,PHONEME_LIST *start,PHONEME_DATA *data,WORD_PH_DATA *word,size_t length)
{
	TEST_ASSERT(tr==NULL && start==ref_base);
	int index=ref_index(plist);rec(14,index,control,word!=NULL,(int)length);
	fill_data(data,clause_seed*31+(unsigned)index*7+(unsigned)control*3+(word!=NULL));
}
static PHONEME_TAB *RefTone(const PHONEME_LIST *p){return p->ph;}
static void RefToneProgram(int code,PHONEME_TAB *ph,PHONEME_DATA *data){(void)ph;rec(15,code,0,0,0);fill_data(data,clause_seed*17+(unsigned)code);}
static int RefFree(void){rec(0,0,0,0,0);return queue_free();}
static void RefAlignment(char *name,int type)
{
	int index,ipa;TEST_ASSERT(sscanf(name,"%d:%d",&index,&ipa)==2 && ipa==-1);
	rec(3,index,type,0,0);free(name);
}
static const char *RefName(char *buf,PHONEME_TAB *ph,PHONEME_LIST *plist,int use_ipa,int *flags)
{
	(void)ph;sprintf(buf,"%d:%d",ref_index(plist),flags?-1:use_ipa);return buf;
}
// Assignments the legacy driver makes to queue-layer state.
static frame_t *frame_slot;
static frame_t **RefBreakFrame(void){rec(5,0,0,0,0);return &frame_slot;}
static int reset_slot;
static int *RefReset(void){rec(1,0,0,0,0);return &reset_slot;}
static int other_state,ref_tail;
static void NoSymbol(char *code,int type){(void)code;(void)type;}
static espeak_ng_OUTPUT_HOOKS ref_hooks_value={.outputPhoSymbol=NoSymbol};
static espeak_ng_OUTPUT_HOOKS *ref_hooks;
static int ref_events,ref_start_char,ref_start_word,ref_sentences,ref_characters;
static Translator *ref_translator;
static char ref_mbrola_name[20];
static int RefMbrola(PHONEME_LIST *l,int *n,bool r){(void)l;(void)n;(void)r;TEST_ASSERT(false);return 0;}

#define Generate ReferenceGenerate
#define DoPause RefPause
#define DoPitch RefPitch
#define DoAmplitude RefAmplitude
#define DoSpect2 RefSpect
#define DoSample3 RefSample
#define DoMarker RefMarker
#define DoPhonemeMarker RefPhonemeMarker
#define DoEmbedded RefEmbedded
#define StartSyllable RefStartSyllable
#define EndAmplitude RefEndAmplitude
#define EndPitch RefEndPitch
#define InterpretPhonemeWithLength RefInterpret
#define InterpretPhoneme2WithData RefToneProgram
#define TonePhoneme RefTone
#define GetEnvelope MockEnvelope
#define WcmdqFree RefFree
#define DoPhonemeAlignment RefAlignment
#define WritePhMnemonicWithStress RefName
#define strdup(s) strcpy(malloc(strlen(s)+1),s)
#define last_frame (*RefBreakFrame())
#define pitch_length (*RefReset())
#define amp_length other_state
#define last_wcmdq other_state
#define syllable_start other_state
#define syllable_end other_state
#define syllable_centre other_state
#define last_pitch_cmd pitch_cmd[0]
#define wcmdq_tail ref_tail
#define output_hooks ref_hooks
#define option_phoneme_events ref_events
#define clause_start_char ref_start_char
#define clause_start_word ref_start_word
#define count_sentences ref_sentences
#define count_characters ref_characters
#define translator ref_translator
#define mbrola_name ref_mbrola_name
#define MbrolaGenerate RefMbrola
#include "generate_reference.inc"
#undef Generate
#undef strdup
#undef last_frame
#undef pitch_length
#undef translator

/* Native side: the same effects through the Rust driver's host callback. */
static const unsigned char *native_envelope(const RustGenerateEffect *e)
{
	if(e->envelope==1)return envelope_data[e->envelope_value];
	if(e->envelope==2)return MockEnvelope(e->envelope_value);
	return NULL;
}
static int NativeEffect(void *context,RustGenerateEffect *e)
{
	PHONEME_LIST *list=context;
	switch(e->op) {
	case 0: rec(0,0,0,0,0);return queue_free();
	case 1: rec(1,0,0,0,0);rec(5,0,0,0,0);pitch_cmd[1]=-1;break;
	case 2: return pitch_cmd[1]>=0;
	case 3: if(hooks)rec(3,e->index,list[e->index].type,0,0);break;
	case 4: rec(4,0,*e->embedded_ix,e->a,0);*e->embedded_ix+=1+(e->a&1);break;
	case 5: rec(5,0,0,0,0);break;
	case 6: rec(6,e->index,e->a,e->b,e->c);break;
	case 7: rec(7,e->index,e->a,e->b,0);break;
	case 8: rec(8,0,0,0,0);break;
	case 9: rec(9,0,e->a,0,0);break;
	case 10: rec(10,0,e->a,e->b,0);break;
	case 11: rec(11,0,e->a,0,0)->env=native_envelope(e);break;
	case 12: rec(12,0,e->a,e->b,0)->env=native_envelope(e);pitch_cmd[1]=1;break;
	case 13: rec(13,0,0,0,0);break;
	case 14: rec(14,e->index,e->a,e->b,n_phoneme_list); // the legacy list length is the clause count
		fill_data(e->data,clause_seed*31+(unsigned)e->index*7+(unsigned)e->a*3+(e->b!=0));break;
	case 15: {int code=list[e->index].tone_ph;rec(15,code,0,0,0);fill_data(e->data,clause_seed*17+(unsigned)code);break;}
	case 16: rec(16,e->index,e->a,e->b,0)->fmt=*e->fmt;spect_effect(e->fmt);break;
	case 17: rec(17,0,e->a,e->b,0)->data=*e->data;break;
	case 18: list[e->index].synthflags=(unsigned short)e->a;break;
	case 19: list[e->index].std_length=(unsigned char)e->a;break;
	}
	return 0;
}
static RustGenerateState native_state;
static int NativeGenerate(PHONEME_LIST *list,int *n_ph,bool resume)
{
	static RustGenerateEntry entries[N_PHONEME_LIST+1];
	size_t count=*n_ph>0?(size_t)*n_ph:0;
	size_t m=count+2<N_PHONEME_LIST+1?count+2:N_PHONEME_LIST+1;
	for(size_t ix=0;ix<m;ix++) {
		const PHONEME_LIST *p=&list[ix];RustGenerateEntry *e=&entries[ix];memset(e,0,sizeof(*e));
		if(p->ph){e->phoneme=*p->ph;e->present=1;}
		e->length=p->length;e->synthflags=p->synthflags;e->source=p->sourceix;e->type=p->type;e->newword=p->newword;
		e->prepause=p->prepause;e->amp=p->amp;e->env=p->env;e->pitch1=p->pitch1;e->pitch2=p->pitch2;e->stress=p->stresslevel;e->tone=p->tone_ph;
	}
	RustGenerateSettings settings={ref_events,ref_translator->langopts.param[LOPT_WORD_MERGE],ref_start_char,ref_start_word,ref_sentences,ref_characters};
	int rc=espeak_rs_generate(entries,m,&count,resume,&native_state,&settings,list,NativeEffect);
	TEST_ASSERT(rc==0 || rc==1);
	*n_ph=(int)count;
	return rc;
}

static unsigned seed=0x9b05688cu;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t clauses,effects,suspensions;
static unsigned char codes[N_PHONEME_TAB];static int n_codes;
static int random_list(PHONEME_LIST *list)
{
	int n=next()%8==0?(int)(next()%(N_PHONEME_LIST-10))+2:(int)(next()%40)+2;
	memset(list,0,sizeof(PHONEME_LIST)*(N_PHONEME_LIST+1));
	for(int ix=0;ix<n+2 && ix<=N_PHONEME_LIST;ix++) {
		PHONEME_LIST *p=&list[ix];int code=codes[next()%n_codes];
		if(next()%60==0)code=phonEND_WORD;
		p->ph=phoneme_tab[code]?phoneme_tab[code]:phoneme_tab[codes[0]];p->phcode=p->ph->code;
		p->type=next()%4?p->ph->type:next()%9;
		p->synthflags=(unsigned short)(next()%3==0?next()&0x200f:0);
		p->newword=next()%4==0?(unsigned char)(next()%16):0;
		p->prepause=next()%5==0?(unsigned char)(next()%80):0;
		p->length=next()%300;p->amp=next()%30;p->env=next()%20;p->pitch1=next();p->pitch2=next();
		p->stresslevel=next()%16;p->tone_ph=next()%6==0?(unsigned char)(1+next()%250):0;
		p->sourceix=(unsigned short)next();p->std_length=next();
	}
	list[0].type=phPAUSE;
	return n;
}
static void compare(void)
{
	int n=random_list(lists[0]);memcpy(lists[1],lists[0],sizeof(lists[0]));
	n_recs[0]=n_recs[1]=0;queue_calls[0]=queue_calls[1]=0;clause_seed=next();
	ref_events=next()%3==0?(int)(next()%4):0;ref_translator->langopts.param[LOPT_WORD_MERGE]=next()%2;
	hooks=next()%4==0;ref_hooks=hooks?&ref_hooks_value:NULL;
	ref_start_char=next()%1000;ref_start_word=next()%100;ref_sentences=next()%50;ref_characters=next()%5000;
	ref_base=lists[0];
	int budget=next()%4;
	for(side=0;side<2;side++) {
		int n_ph=n;bool resume=false;free_budget=budget;n_phoneme_list=n;
		for(int round=0;;round++) {
			TEST_ASSERT(round<100);
			int rc=side==0?ReferenceGenerate(lists[0],&n_ph,resume):NativeGenerate(lists[1],&n_ph,resume);
			if(rc==0)break;
			resume=true;if(side==0)suspensions++;
		}
		TEST_ASSERT(n_ph==0);
	}
	bool same=n_recs[0]==n_recs[1];
	for(int i=0;same && i<n_recs[0];i++)if(memcmp(&recs[0][i],&recs[1][i],sizeof(Rec)))same=false;
	if(!same) {
		fprintf(stderr,"n=%d traces %d/%d\n",n,n_recs[0],n_recs[1]);
		for(int i=0;i<n_recs[0] || i<n_recs[1];i++) {
			const Rec *a=&recs[0][i],*b=&recs[1][i];
			if(i<n_recs[0] && i<n_recs[1] && !memcmp(a,b,sizeof(Rec)))continue;
			fprintf(stderr,"#%d ref op%d ix%d %d %d %d | native op%d ix%d %d %d %d\n",i,a->op,a->index,a->a,a->b,a->c,b->op,b->index,b->a,b->b,b->c);
			break;
		}
		TEST_ASSERT(false);
	}
	for(int ix=0;ix<n+2;ix++)TEST_ASSERT(lists[0][ix].synthflags==lists[1][ix].synthflags && lists[0][ix].std_length==lists[1][ix].std_length);
	clauses++;effects+=n_recs[0];
}
int main(void)
{
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050);
	static Translator translator_value;ref_translator=&translator_value;
	static const char *tables[]={"en","de","fr","es","ru","hi","sv","vi"};
	for(size_t t=0;t<sizeof(tables)/sizeof(*tables);t++) {
		int table=LookupPhonemeTable(tables[t]);if(table<0)continue;
		SelectPhonemeTable(table);n_codes=0;
		for(int code=1;code<n_phoneme_tab;code++)if(phoneme_tab[code])codes[n_codes++]=code;
		for(int trial=0;trial<1500;trial++)compare();
	}
	TEST_ASSERT(espeak_Terminate()==EE_OK);
	printf("Matched %zu clauses: %zu effects, %zu suspensions\n",clauses,effects,suspensions);
	return 0;
}
