/* Native synthesis command writers against independently extracted retained
 * C: the same operation scripts must leave identical queues, state, copied
 * frames and host calls.
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
#include "voice.h"
#include "rust_data.h"

typedef struct { int op,a,b,c,d; FMT_PARAMS fmt; } Event;
#define MAX_EVENTS 4096
static Event events[2][MAX_EVENTS];
static int n_events[2],side;
static size_t pushes;
static void event(int op,int a,int b,int c,int d,const FMT_PARAMS *fmt)
{
	TEST_ASSERT(n_events[side]<MAX_EVENTS);
	Event *e=&events[side][n_events[side]++];memset(e,0,sizeof(*e));
	e->op=op;e->a=a;e->b=b;e->c=c;e->d=d;if(fmt)e->fmt=*fmt;
}
static unsigned mix(unsigned x){x^=x>>16;x*=0x7feb352du;x^=x>>15;x*=0x846ca68bu;x^=x>>16;return x;}

/* Shared fixtures: phoneme sound data, frames, a frame-copy pool. */
#define WAVE_SIZE 65536
static unsigned char wave[WAVE_SIZE];
static frame_t frames_data[256];
static frame_t copy_pool[2][64];
static int copy_cursor[2];
static unsigned lookup_seed;
static int lookup_calls[2];

static intptr_t queues[2][N_WCMDQ][4];
static int tails[2];

/* Lookups are deterministic in the call number, which and format address;
 * transitions may request pauses (C issued them inside the lookup). */
typedef struct { bool found; int count,modulation,n_pauses,pauses[3]; frameref_t frames[N_SEQ_FRAMES]; } MockLookup;
static void mock_lookup(MockLookup *m,int which,const FMT_PARAMS *fmt)
{
	unsigned h=mix(lookup_seed+(unsigned)lookup_calls[side]++*977+(unsigned)which*31+(unsigned)fmt->fmt_addr);
	memset(m,0,sizeof(*m));
	m->found=h%9!=0;
	m->count=1+(int)(mix(h+1)%N_SEQ_FRAMES); // a found sequence has a first frame
	m->modulation=mix(h+2)%3==0?(int)(mix(h+3)&0xf00):0;
	m->n_pauses=(int)(mix(h+4)%5==0?1+mix(h+5)%3:0);
	for(int i=0;i<m->n_pauses;i++)m->pauses[i]=(int)(mix(h+6+i)%60)+1;
	for(int i=0;i<m->count;i++) {
		unsigned v=mix(h+20+i);
		m->frames[i].length=(short)(v%60);
		m->frames[i].frflags=(short)(mix(v)%4==0?FRFLAG_LEN_MOD:mix(v)%4==1?FRFLAG_LEN_MOD2:0);
		m->frames[i].frame=&frames_data[mix(v+1)%256];
	}
}

/* Reference hooks. */
static int ref_samplerate,ref_tail;
static intptr_t (*ref_q)[4];
static SPEED_FACTORS ref_speed;
static voice_t ref_voice_value,*ref_voice=&ref_voice_value;
static Translator ref_translator_value,*ref_translator=&ref_translator_value;
static int last_pitch_cmd,last_amp_cmd,last_wcmdq,pitch_length,amp_length,fmt_amplitude,syllable_start,syllable_end,syllable_centre,modn_flags,wave_flag;
static frame_t *last_frame;
static void RefInc(void){if(++ref_tail>=N_WCMDQ)ref_tail=0;}
static int adjust_slot;
static int *RefAdjust(void){event(8,0,0,0,0,NULL);return &adjust_slot;}
static void RefSmooth(void){event(4,syllable_start,syllable_end,syllable_centre,0,NULL);syllable_start=syllable_end;}
static void DoPause(int length,int control);
static frameref_t *RefLookup(PHONEME_TAB *ph,int which,FMT_PARAMS *fmt,int *n_frames,PHONEME_LIST *plist)
{
	static MockLookup m;(void)ph;(void)plist;
	event(5,which,0,0,0,fmt);
	mock_lookup(&m,which,fmt);
	modn_flags=m.modulation;
	for(int i=0;i<m.n_pauses;i++)DoPause(m.pauses[i],0);
	*n_frames=m.count;
	return m.found?m.frames:NULL;
}
static frame_t *RefCopy(frame_t *frame,int force)
{
	TEST_ASSERT(force==1);
	frame_t *copy=&copy_pool[0][copy_cursor[0]++%64];*copy=*frame;
	event(7,(int)(frame-frames_data),0,0,0,NULL);
	return copy;
}
#define wcmdq ref_q
#define wcmdq_tail ref_tail
#define WcmdqInc RefInc
#define speed ref_speed
#define samplerate ref_samplerate
#define wavefile_data wave
#define voice ref_voice
#define translator ref_translator
#define SmoothSpect RefSmooth
#define LookupSpect RefLookup
#define CopyFrame RefCopy
#define seq_len_adjust (*RefAdjust())
#define PauseLength RefPauseLength
#define DoSample3 RefSample3
#define DoSpect2 RefSpect2
#include "commands_reference.inc"
#undef wcmdq
#undef wcmdq_tail
#undef WcmdqInc
#undef speed
#undef samplerate
#undef voice
#undef translator
#undef SmoothSpect
#undef LookupSpect
#undef CopyFrame
#undef seq_len_adjust
#undef PauseLength
#undef DoSample3
#undef DoSpect2

/* Native side: the Rust writers over their own state and queue. */
static RustCommandState native;
static int NativeEffect(void *context,RustCommandEffect *e)
{
	(void)context;
	intptr_t (*q)[4]=queues[1];
	switch(e->op) {
	case 0: {int index=tails[1];pushes++;for(size_t i=0;i<e->count;i++)q[tails[1]][i]=e->words[i];if(++tails[1]>=N_WCMDQ)tails[1]=0;return index;}
	case 1: return tails[1];
	case 2: TEST_ASSERT(e->index>=0 && e->index<N_WCMDQ);e->value=q[e->index][e->slot];break;
	case 3: TEST_ASSERT(e->index>=0 && e->index<N_WCMDQ);q[e->index][e->slot]=e->value;break;
	case 4: event(4,e->a,e->b,e->c,0,NULL);return e->b;
	case 5: {
		MockLookup m;event(5,e->a,0,0,0,e->fmt);mock_lookup(&m,e->a,e->fmt);
		e->lookup->found=m.found;e->lookup->count=m.count;e->lookup->modulation=m.modulation;e->lookup->n_pauses=m.n_pauses;
		for(int i=0;i<m.n_pauses;i++)e->lookup->pauses[i]=m.pauses[i];
		if(m.found)memcpy(e->frames,m.frames,sizeof(frameref_t)*(size_t)m.count);
		break;
	}
	case 6: {const frame_t *f=(const frame_t *)e->value;TEST_ASSERT(f!=NULL);e->a=f->length;e->b=f->frflags;break;}
	case 7: {
		frame_t *frame=(frame_t *)e->value,*high=(frame_t *)e->words[0];
		frame_t *copy=&copy_pool[1][copy_cursor[1]++%64];*copy=*frame;
		event(7,(int)(frame-frames_data),0,0,0,NULL);
		for(int ix=3;ix<8;ix++){if(ix<7)copy->ffreq[ix]=high->ffreq[ix];copy->fheight[ix]=high->fheight[ix];}
		e->value=(intptr_t)copy;break;
	}
	case 8: event(8,0,0,0,0,NULL);break;
	}
	return 0;
}
static RustCommandSettings native_settings(void)
{
	RustCommandSettings s;memset(&s,0,sizeof(s));
	s.settings.samplerate=ref_samplerate;s.settings.pause_factor=ref_speed.pause_factor;
	s.settings.clause_pause_factor=ref_speed.clause_pause_factor;s.settings.min_pause=ref_speed.min_pause;
	s.settings.wav_factor=ref_speed.wav_factor;s.settings.lenmod_factor=ref_speed.lenmod_factor;
	s.settings.lenmod2_factor=ref_speed.lenmod2_factor;s.settings.min_sample_len=ref_speed.min_sample_len;
	s.settings.klatt=ref_voice->klattv[0]!=0;
	s.settings.long_vowel_threshold=ref_translator->langopts.param[LOPT_LONG_VOWEL_THRESHOLD];
	s.settings.sonorant_min=ref_translator->langopts.param[LOPT_SONORANT_MIN];
	s.settings.fall_envelope=(uintptr_t)envelope_data[PITCHfall];
	s.wave=wave;s.wave_length=WAVE_SIZE;
	return s;
}

static unsigned seed=0x510e527fu;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t scripts,operations;
static PHONEME_TAB phonemes[3];
static PHONEME_LIST plists[3];
static int wave_index(void){return next()%4==0?0:(int)(next()%(WAVE_SIZE/8-1))*8;}
static void random_fmt(FMT_PARAMS *f)
{
	memset(f,0,sizeof(*f));
	f->fmt_control=next()%3==0?pd_DONTLENGTHEN:0;f->use_vowelin=next()%2;
	f->fmt_addr=next()%8==0?0:(int)(1+next()%5000);f->fmt_length=next()%40;f->fmt_amp=next()%4==0?(int)(next()%3):0;
	f->fmt2_addr=next()%3;f->fmt2_lenadj=next()%20;f->wav_addr=next()%3==0?wave_index():0;f->wav_amp=next()%3==0?0:(int)(next()%120);
	f->transition0=(int)next();f->transition1=(int)next();f->std_length=next()%300;
}
static void random_data(PHONEME_DATA *d)
{
	memset(d,0,sizeof(*d));
	d->pd_control=(int)(next()&pd_DONTLENGTHEN);
	for(int i=0;i<N_PHONEME_DATA_PARAM;i++)d->pd_param[i]=next()%120;
	d->sound_addr[pd_WAV]=next()%5==0?0:wave_index();
	d->sound_param[pd_WAV]=next()%4==0?0:(int)(next()%150);
}
// Run the script once on the reference, then on the native writers.
static void run_script(unsigned script_seed,int n_ops,bool reference)
{
	unsigned saved=seed;seed=script_seed;
	for(int op=0;op<n_ops;op++) {
		unsigned kind=next()%10;
		RustCommandSettings s=native_settings();
		if(kind==0) {
			int length=next()%5==0?0:next()%25==0?(int)(70000+next()%400000):(int)(next()%400),control=next()%2; // some past the 90000 mS overflow guard
			if(reference)DoPause(length,control);else espeak_rs_command_pause(&native,&s,NULL,NativeEffect,length,control);
		} else if(kind==1) {
			const unsigned char *env=envelope_data[next()%N_ENVELOPE_DATA];
			int p1=next()%6==0?255:(int)(next()%200),p2=next()%6==0?-(int)(next()%20):(int)(next()%200);
			if(reference)DoPitch(env,p1,p2);else espeak_rs_command_pitch(&native,&s,NULL,NativeEffect,env,p1,p2);
		} else if(kind==2) {
			const unsigned char *env=next()%2?NULL:envelope_data[next()%N_ENVELOPE_DATA];int amp=next()%40;
			if(reference)DoAmplitude(amp,env);else espeak_rs_command_amplitude(&native,&s,NULL,NativeEffect,amp,env);
		} else if(kind==3) {
			int vb=next()%2;
			if(reference)EndPitch(vb);else espeak_rs_command_end_pitch(&native,&s,NULL,NativeEffect,vb);
		} else if(kind==4) {
			if(reference)EndAmplitude();else espeak_rs_command_end_amplitude(&native,&s,NULL,NativeEffect);
		} else if(kind==5) {
			if(reference)StartSyllable();else espeak_rs_command_start_syllable(&native,&s,NULL,NativeEffect);
		} else if(kind==6) {
			PHONEME_DATA d;random_data(&d);int length_mod=next()%3==0?0:(int)(next()%400),amp=next()%4==0?-1:(int)(next()%3);
			int len=reference?RefSample3(&d,length_mod,amp):espeak_rs_command_sample(&native,&s,NULL,NativeEffect,&d,length_mod,amp);
			event(20,len,0,0,0,NULL);
		} else {
			FMT_PARAMS f;random_fmt(&f);
			int which=next()%3,modulation=next()%6==0?-1:(int)(next()%7);
			PHONEME_TAB *ph=&phonemes[1];PHONEME_LIST *plist=&plists[1];
			ph->type=next()%9;ph->std_length=next()%200;ph->phflags=next()%4==0?phLONG:0;
			plist->synthflags=(unsigned short)(next()%4==0?SFLAG_LENGTHEN:0);plist->length=next()%3==0?0:next()%400;
			plists[0].type=next()%9;
			int len;
			if(reference)len=RefSpect2(ph,which,&f,plist,modulation);
			else {
				RustSpectPhoneme p;memset(&p,0,sizeof(p));
				p.type=ph->type;p.std_length=ph->std_length;p.phflags=ph->phflags;p.synthflags=plist->synthflags;p.length=plist->length;
				p.prev_type=which==1?plists[0].type:0;
				len=espeak_rs_command_spect(&native,&s,NULL,NativeEffect,&p,which,&f,modulation);
			}
			event(21,len,0,0,0,&f);
		}
	}
	seed=saved;
}
static void compare(void)
{
	// settings and the starting state, identical on both sides
	// 22051 tells the two pause conversions apart (exact for multiples of 25)
	static const int rates[]={22050,16000,44100,22051};
	ref_samplerate=rates[next()%4];
	ref_speed.pause_factor=50+next()%300;ref_speed.clause_pause_factor=50+next()%300;ref_speed.min_pause=next()%20;
	ref_speed.wav_factor=100+next()%300;ref_speed.lenmod_factor=next()%256;ref_speed.lenmod2_factor=next()%256;
	ref_speed.min_sample_len=next()%500;
	ref_voice->klattv[0]=next()%2;
	ref_translator->langopts.param[LOPT_LONG_VOWEL_THRESHOLD]=next()%2?0:(int)(next()%200);
	ref_translator->langopts.param[LOPT_SONORANT_MIN]=next()%300;
	RustCommandState start;memset(&start,0,sizeof(start));
	start.last_pitch_cmd=next()%3==0?-1:(int)(next()%N_WCMDQ);start.last_amp_cmd=next()%N_WCMDQ;
	start.last_wcmdq=next()%N_WCMDQ;start.last_frame=next()%3==0?NULL:&frames_data[next()%256];
	start.pitch_length=next()%3==0?0:(int)(next()%500);start.amp_length=next()%3==0?0:(int)(next()%500);
	start.syllable_start=next()%N_WCMDQ;start.syllable_end=next()%2?start.syllable_start:(int)(next()%N_WCMDQ);
	start.syllable_centre=next()%3==0?-1:(int)(next()%N_WCMDQ);start.fmt_amplitude=next()%3;start.wave_flag=next()%2;
	int tail=next()%N_WCMDQ;
	for(int i=0;i<N_WCMDQ;i++)for(int j=0;j<4;j++)queues[0][i][j]=next()%3==0?0:(intptr_t)next();
	memcpy(queues[1],queues[0],sizeof(queues[0]));
	memset(copy_pool,0,sizeof(copy_pool));
	lookup_seed=next();
	unsigned script_seed=next();int n_ops=1+next()%40;

	side=0;n_events[0]=0;lookup_calls[0]=0;copy_cursor[0]=0;ref_q=queues[0];ref_tail=tail;
	last_pitch_cmd=start.last_pitch_cmd;last_amp_cmd=start.last_amp_cmd;last_wcmdq=start.last_wcmdq;last_frame=start.last_frame;
	pitch_length=start.pitch_length;amp_length=start.amp_length;syllable_start=start.syllable_start;syllable_end=start.syllable_end;
	syllable_centre=start.syllable_centre;fmt_amplitude=start.fmt_amplitude;wave_flag=start.wave_flag;
	run_script(script_seed,n_ops,true);

	side=1;n_events[1]=0;lookup_calls[1]=0;copy_cursor[1]=0;tails[1]=tail;native=start;
	run_script(script_seed,n_ops,false);

	// copies live in per-side pools: compare them by position and content
	for(int i=0;i<N_WCMDQ;i++)for(int j=2;j<4;j++) {
		intptr_t a=queues[0][i][j],b=queues[1][i][j];
		if(a>=(intptr_t)copy_pool[0] && a<(intptr_t)(copy_pool[0]+64) && b>=(intptr_t)copy_pool[1] && b<(intptr_t)(copy_pool[1]+64)) {
			TEST_ASSERT(a-(intptr_t)copy_pool[0]==b-(intptr_t)copy_pool[1]);
			queues[1][i][j]=a;
		}
	}
	if((uintptr_t)native.last_frame>=(uintptr_t)copy_pool[1] && (uintptr_t)native.last_frame<(uintptr_t)(copy_pool[1]+64))
		native.last_frame=copy_pool[0]+(native.last_frame-copy_pool[1]);
	TEST_ASSERT(memcmp(copy_pool[0],copy_pool[1],sizeof(copy_pool[0]))==0 && copy_cursor[0]==copy_cursor[1]);
	bool same=n_events[0]==n_events[1];
	for(int i=0;same && i<n_events[0];i++)if(memcmp(&events[0][i],&events[1][i],sizeof(Event)))same=false;
	if(!same) {
		for(int i=0;i<n_events[0] || i<n_events[1];i++) {
			const Event *a=&events[0][i],*b=&events[1][i];
			if(i<n_events[0] && i<n_events[1] && !memcmp(a,b,sizeof(Event)))continue;
			fprintf(stderr,"event #%d of %d/%d: ref op%d %d %d %d | native op%d %d %d %d\n",i,n_events[0],n_events[1],a->op,a->a,a->b,a->c,b->op,b->a,b->b,b->c);
			break;
		}
		TEST_ASSERT(false);
	}
	if(memcmp(queues[0],queues[1],sizeof(queues[0]))) {
		for(int i=0;i<N_WCMDQ;i++)if(memcmp(queues[0][i],queues[1][i],sizeof(queues[0][i])))
			fprintf(stderr,"queue %d: %ld %ld %ld %ld | %ld %ld %ld %ld\n",i,(long)queues[0][i][0],(long)queues[0][i][1],(long)queues[0][i][2],(long)queues[0][i][3],
				(long)queues[1][i][0],(long)queues[1][i][1],(long)queues[1][i][2],(long)queues[1][i][3]);
		TEST_ASSERT(false);
	}
	TEST_ASSERT(ref_tail==tails[1]);
	TEST_ASSERT(native.last_pitch_cmd==last_pitch_cmd && native.last_amp_cmd==last_amp_cmd && native.last_wcmdq==last_wcmdq);
	TEST_ASSERT(native.last_frame==last_frame && native.pitch_length==pitch_length && native.amp_length==amp_length);
	TEST_ASSERT(native.syllable_start==syllable_start && native.syllable_end==syllable_end && native.syllable_centre==syllable_centre);
	TEST_ASSERT(native.fmt_amplitude==fmt_amplitude && native.wave_flag==wave_flag);
	scripts++;operations+=n_ops;
}
int main(void)
{
	_Static_assert(sizeof(RustCommandState)==48,"command state layout");
	for(int i=0;i<WAVE_SIZE;i++)wave[i]=(unsigned char)next();
	// sample headers at aligned addresses: zero length, or long enough to
	// split (C loops forever on a sample of fewer than four units)
	for(int i=0;i+8<=WAVE_SIZE;i+=8){unsigned length=next()%10==0?0:16+next()%4000;wave[i]=length&0xff;wave[i+1]=(length>>8)&0xff;}
	for(int i=0;i<256;i++) {
		frame_t *f=&frames_data[i];memset(f,0,sizeof(*f));
		f->length=next()%4;
		static const short flags[]={0,FRFLAG_VOWEL_CENTRE,FRFLAG_BREAK,FRFLAG_BREAK_LF,FRFLAG_MODULATE,FRFLAG_DEFER_WAV};
		f->frflags=flags[next()%6]|(short)(next()%3==0?flags[next()%6]:0);
		for(int k=0;k<7;k++)f->ffreq[k]=(short)next();
		for(int k=0;k<8;k++)f->fheight[k]=next();
	}
	for(int trial=0;trial<60000;trial++)compare();
	// Where C loops forever (a sample under four units) or reads past the
	// sound data, the native writers report -1 instead.
	ref_speed.min_sample_len=100;ref_speed.wav_factor=256;
	RustCommandSettings settings=native_settings();PHONEME_DATA d;memset(&d,0,sizeof(d));
	memset(&native,0,sizeof(native));native.last_pitch_cmd=-1;side=1;
	wave[8]=2;wave[9]=0;wave[10]=1;d.sound_addr[pd_WAV]=8; // two 8-bit units, lengthened to 100
	TEST_ASSERT(espeak_rs_command_sample(&native,&settings,NULL,NativeEffect,&d,0,0)==-1);
	d.sound_addr[pd_WAV]=WAVE_SIZE-2;
	TEST_ASSERT(espeak_rs_command_sample(&native,&settings,NULL,NativeEffect,&d,0,0)==-1);
	printf("Matched %zu scripts: %zu operations, %zu queue writes; unbounded samples rejected\n",scripts,operations,pushes);
	return 0;
}
