/* Native wave generator against the independently extracted legacy C: the
 * same queue programs, buffer sizes and direct calls must leave identical
 * output, echo rings, queue heads, embedded values and host calls.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/encoding.h>
#include "phoneme.h"
#include "synthesize.h"
#include "voice.h"
#include "wavegen.h"
#include "rust_data.h"
#include "test_wgen_data.h"
#if USE_LIBSONIC
#include "sonic.h"
#endif
#include "sintab.h"

/* Host calls, per side. */
typedef struct { int op,a,b,c,d; WGEN_DATA data; } Event;
static int event_equal(const Event *a,const Event *b)
{
	return a->op==b->op && a->a==b->a && a->b==b->b && a->c==b->c && a->d==b->d && test_wgen_equal(&a->data,&b->data);
}
#define MAX_EVENTS 200000
static Event events[2][MAX_EVENTS];
static int n_events[2],side;
static void event(int op,int a,int b,int c,int d,const WGEN_DATA *data)
{
	TEST_ASSERT(n_events[side]<MAX_EVENTS);
	Event *e=&events[side][n_events[side]++];memset(e,0,sizeof(*e));
	e->op=op;e->a=a;e->b=b;e->c=c;e->d=d;if(data)e->data=*data;
}

/* Per-side shared memory. */
static int samplerates[2],embedded[2][N_EMBEDDED_VALUES];
static RustWaveMemory memories[2];
#define OUT_SIZE 8192
#define OUT_SLACK 64
static unsigned char outbufs[2][OUT_SIZE+OUT_SLACK];
static RustOutput outputs[2]; // cursors over outbufs; side 1's is the Rust generator's
int echo_amp; // the reference's: a macro would rename voice_t's member too
static unsigned rands[2];
static int klatt_left[2],mbrola_left[2];

/* Read-only fixtures both sides address. */
#define DATA_SIZE 65536
static unsigned char sample_data[DATA_SIZE];
static unsigned char envelopes[8][128];
static frame_t frames[256];

static int Rand(int min,int max)
{
	unsigned *r=&rands[side];*r=*r*1103515245u+12345u;
	return min+(int)((*r>>8)%(unsigned)(max-min+1));
}
static int buffer_offset(unsigned char *p){return (int)(p-outbufs[side]);}
static void Marker(int type,unsigned int pos,int value,int value2,unsigned char *out)
{
	event(1,type,(int)pos,value,value2,NULL);event(2,buffer_offset(out),0,0,0,NULL);
}
static void Output(unsigned char **out,unsigned char *end,int value){(*out)[0]=value;(*out)[1]=value>>8;*out+=2;(void)end;}
static int MockKlatt(int length,int resume,frame_t *fr1,frame_t *fr2,WGEN_DATA *wdata,voice_t *wvoice)
{
	unsigned char **out=&outputs[side].ptr;
	event(3,length,resume,(int)(fr1-frames),(int)(fr2-frames),wdata);
	event(4,wvoice?wvoice->voicing:-1,wvoice?wvoice->freq[1]:-1,0,0,NULL);
	wdata->pitch_ix+=3; // seen by the generator afterwards
	if(!resume)klatt_left[side]=length%300;
	while(klatt_left[side]-->0) {
		Output(out,outputs[side].end,klatt_left[side]*37);
		if(*out+2>outputs[side].end)return 1;
	}
	return 0;
}
static void MockKlattReset(int control){event(5,control,0,0,0,NULL);}
static void MockKlattInit(void){event(6,0,0,0,0,NULL);}
static int MockMbrola(int length,bool resume,int amplitude)
{
	unsigned char **out=&outputs[side].ptr;
	event(7,length,resume,amplitude,0,NULL);
	if(!resume)mbrola_left[side]=length%200;
	while(mbrola_left[side]-->0) {
		Output(out,outputs[side].end,mbrola_left[side]*amplitude);
		if(*out+2>outputs[side].end)return 1;
	}
	return 0;
}
static void HookPho(char *code,int type){unsigned h=0;for(char *p=code;*p;p++)h=h*31+(unsigned char)*p;event(8,(int)h,type,0,0,NULL);}
static void HookSilence(short v){event(9,v,0,0,0,NULL);}
static void HookVoiced(short v){event(10,v,0,0,0,NULL);}
static void HookUnvoiced(short v){event(11,v,0,0,0,NULL);}
static espeak_ng_OUTPUT_HOOKS hook_sets[4]={
	{HookPho,HookSilence,HookVoiced,HookUnvoiced},
	{HookPho,NULL,HookVoiced,NULL},
	{HookPho,HookSilence,NULL,HookUnvoiced},
	{NULL,NULL,NULL,NULL},
};
static espeak_ng_OUTPUT_HOOKS *hooks;

/* The reference over side 0's memory; its sample rate and echo amplitude are
 * the library's globals, which voice_t member names would collide with. */
static voice_t roughness_voice,*ref_voice=&roughness_voice;
#if USE_LIBSONIC
static sonicStream sonicSpeedupStream=NULL;
static double sonicSpeed=1.0;
#endif
#define embedded_value embedded[0]
#define echo_head memories[0].echo_head
#define echo_tail memories[0].echo_tail
#define echo_buf memories[0].echo_buf
#define out_ptr outputs[0].ptr
#define out_end outputs[0].end
#define output_hooks hooks
#define wcmdq memories[0].queue
#define wcmdq_head memories[0].head
#define wcmdq_tail memories[0].tail
#define WcmdqFree RefWcmdqFree
#define WcmdqUsed RefWcmdqUsed
#define WcmdqInc RefWcmdqInc
#define WcmdqIncHead RefWcmdqIncHead
#ifndef MAKE_MEM_UNDEFINED
#define MAKE_MEM_UNDEFINED(addr, len) ((void)(addr), (void)(len))
#endif
#include "wave_queue_reference.inc"
#define voice ref_voice
#define MarkerEvent Marker
#define espeak_rand Rand
#define Wavegen_Klatt MockKlatt
#define KlattReset MockKlattReset
#define KlattInit MockKlattInit
#define MbrolaFill MockMbrola
#define WavegenInit RefWavegenInit
#define GetAmplitude RefGetAmplitude
#define PeaksToHarmspect RefPeaksToHarmspect
#define InitBreath RefInitBreath
#define SetEmbedded RefSetEmbedded
#define WavegenSetVoice RefWavegenSetVoice
#define SetPitch2 RefSetPitch2
#include "wavegen_reference.inc"
#undef embedded_value
#undef echo_head
#undef echo_tail
#undef echo_buf
#undef out_ptr
#undef out_end
#undef output_hooks
#undef wcmdq
#undef wcmdq_head
#undef wcmdq_tail
#undef WcmdqFree
#undef WcmdqUsed
#undef WcmdqInc
#undef WcmdqIncHead
#undef voice
#undef MarkerEvent
#undef espeak_rand
#undef Wavegen_Klatt
#undef KlattReset
#undef KlattInit
#undef MbrolaFill
#undef WavegenInit
#undef GetAmplitude
#undef PeaksToHarmspect
#undef InitBreath
#undef SetEmbedded
#undef WavegenSetVoice
#undef SetPitch2

/* The native generator over side 1's memory. */
static RustWavegen *native;
static const RustWavegenShared shared={
	&memories[1],&outputs[1],&samplerates[1],embedded[1]
};
static int NativeEffect(void *context,RustWavegenEffect *e)
{
	(void)context;
	intptr_t *q=memories[1].queue[e->index];
	TEST_ASSERT(e->index>=0 && e->index<N_WCMDQ);
	switch(e->op) {
	case 1:
		TEST_ASSERT(hooks!=NULL);
		if(e->a==1){TEST_ASSERT(hooks->outputVoiced!=NULL);hooks->outputVoiced(e->b);}
		else if(e->a==2){TEST_ASSERT(hooks->outputSilence!=NULL);hooks->outputSilence(e->b);}
		else{TEST_ASSERT(e->a==4 && hooks->outputUnvoiced!=NULL);hooks->outputUnvoiced(e->b);}
		break;
	case 2: Marker(q[0]>>8,q[1],*(int *)&q[2],*((int *)&q[2]+1),outputs[1].ptr);break;
	case 3: hooks->outputPhoSymbol((char *)q[1],q[2]);free((char *)q[1]);break;
	case 4: Marker(espeakEVENT_SAMPLERATE,0,e->a,0,outputs[1].ptr);break;
	case 5: break;
	case 6: return Rand(e->a,e->b);
	case 7: free((voice_t *)e->value);break;
	case 8: MockKlattReset(1);break;
	case 9: return MockKlatt(e->a,e->b,(frame_t *)e->value,(frame_t *)e->value2,e->data,e->voice);
	case 10: return MockMbrola(e->a,e->b,e->c);
	default: TEST_ASSERT(false);
	}
	return 0;
}
static int NativeFill(void)
{
	RustWavegenOptions o={USE_KLATT!=0,USE_MBROLA!=0,USE_LIBSONIC!=0,ref_voice->roughness,0};
	if(hooks)o.hooks=(hooks->outputVoiced?1:0)|(hooks->outputSilence?2:0)|(hooks->outputUnvoiced?4:0);
	return espeak_rs_wavegen_fill(native,&shared,NULL,NativeEffect,&o,env_fall);
}

/* Random programs. */
static unsigned seed=0x6a09e667u;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static int range(int min,int max){return min+(int)(next()%(unsigned)(max-min+1));}
static size_t programs,commands,fills,samples;
static bool single; // WAVEGEN_ORACLE_SINGLE: one-sample buffers, to locate a difference

static void random_voice(voice_t *v)
{
	memset(v,0,sizeof(*v));
	v->pitch_base=range(0x40000,0x70000);v->pitch_range=range(0,3000);
	v->flutter=next()%3==0?0:range(0,128);v->roughness=range(0,9);
	v->echo_delay=next()%2?0:range(0,130);v->echo_amp=range(0,120);
	v->n_harmonic_peaks=range(1,5);v->peak_shape=next()%2;v->voicing=next()%3?64:range(0,100);
	v->consonant_amp=range(0,200);v->consonant_ampv=range(0,150);v->samplerate=range(8000,48000);
	for(int pk=0;pk<N_PEAKS;pk++) {
		v->freq[pk]=v->freq2[pk]=(short)range(200,300);
		v->height[pk]=v->height2[pk]=(short)range(64,280);
		v->width[pk]=(short)range(64,350);
		v->freqadd[pk]=(short)range(-20,20);
		v->breath[pk]=next()%4==0?range(0,60):0;
		v->breathw[pk]=range(50,700);
	}
	if(next()%3==0)v->breath[0]=0;
	for(int i=0;i<N_TONE_ADJUST;i++)v->tone_adjust[i]=(unsigned char)(next()%4?128:range(0,255));
}
static void random_frame(frame_t *f)
{
	memset(f,0,sizeof(*f));
	for(int i=0;i<7;i++)f->ffreq[i]=(short)(next()%8==0?0:range(150,700*(i+1)));
	for(int i=0;i<8;i++)f->fheight[i]=(unsigned char)(next()%6==0?0:range(1,64));
	for(int i=0;i<6;i++)f->fwidth[i]=(unsigned char)range(1,255);
	for(int i=0;i<3;i++)f->fright[i]=(unsigned char)range(1,255);
	f->length=(unsigned char)next();f->rms=(unsigned char)next();
}
static unsigned char *random_envelope(bool allow_null)
{
	switch(next()%4) {
	case 0: return allow_null?NULL:envelopes[next()%8];
	case 1: return (unsigned char *)envelope_data[next()%N_ENVELOPE_DATA];
	default: return envelopes[next()%8];
	}
}
static intptr_t frame_address(void){return (intptr_t)&frames[next()%256];}
// One command, pushed to both queues; allocations are per side.
static void push(bool alignment)
{
	intptr_t q[4]={0,0,0,0};
	voice_t *voices[2]={NULL,NULL};
	char *codes[2]={NULL,NULL};
	switch(next()%20) {
	case 0: case 1: q[0]=WCMD_PITCH;q[1]=range(0,20000);q[2]=(intptr_t)random_envelope(true);q[3]=(range(0,255)<<16)|range(0,255);break;
	case 2: q[0]=WCMD_AMPLITUDE;q[1]=range(0,20000);q[2]=(intptr_t)random_envelope(true);q[3]=range(0,80);break;
	case 3: case 4: case 5: case 6: case 7:
		q[0]=next()%4?WCMD_SPECT:WCMD_SPECT2;
		// A segment shorter than a cycle extrapolates its formants to the
		// cycle's end; between distinct frames that can run the harmonics
		// past C's tables, which it then overwrites. Short ones hold a frame.
		q[1]=(next()%3?range(1700,3000):range(0,1699))|((intptr_t)(range(0,7)|(next()%3==0?(range(0,3)<<8)|(next()%2?0x400:0x800):0))<<16);
		q[2]=frame_address();q[3]=(q[1]&0xffff)<1700?q[2]:frame_address();break;
	case 8: case 9: q[0]=WCMD_PAUSE;q[1]=next()%5==0?0:range(1,3000);break;
	case 10: {
		int scale=next()%2?0:range(1,255),length=range(0,4000);
		q[0]=WCMD_WAVE;q[1]=length;q[2]=(intptr_t)&sample_data[range(0,DATA_SIZE-2*length-4)];q[3]=scale|(range(0,255)<<8);break;
	}
	case 11: {
		int scale=next()%2?0:range(1,255),n=range(0,0xffff),max=range(8,20000);
		q[0]=WCMD_WAVE2;q[1]=n|(max<<16);q[2]=(intptr_t)&sample_data[range(0,DATA_SIZE-2*max-8)];q[3]=scale|(range(0,255)<<8);break;
	}
	case 12: q[0]=WCMD_MARKER|(range(0,12)<<8);q[1]=range(0,100000);{int v[2]={(int)next(),(int)next()};memcpy(&q[2],v,sizeof(v));}break;
	case 13: {
		voice_t v;random_voice(&v);q[0]=WCMD_VOICE;
		for(int s=0;s<2;s++){voices[s]=malloc(sizeof(voice_t));TEST_ASSERT(voices[s]!=NULL);*voices[s]=v;}
		break;
	}
	case 14: q[0]=WCMD_EMBEDDED;q[1]=range(0,0x7f);q[2]=range(-50,400);break;
	case 15: q[0]=WCMD_FMT_AMPLITUDE;q[1]=next()%4==0?0:range(1,150);break;
	case 16: q[0]=next()%2?WCMD_KLATT:WCMD_KLATT2;q[1]=range(0,3000)|(range(0,7)<<16);q[2]=frame_address();q[3]=frame_address();break;
	case 17: q[0]=WCMD_MBROLA_DATA;q[1]=range(0,600);break;
	case 18:
		if(alignment) {
			char code[16];snprintf(code,sizeof(code),"ph%u",next()%1000);q[0]=WCMD_PHONEME_ALIGNMENT;q[2]=range(0,5);
			for(int s=0;s<2;s++){codes[s]=strdup(code);TEST_ASSERT(codes[s]!=NULL);}
		} else
			q[0]=WCMD_SONIC_SPEED,q[1]=range(512,4096);
		break;
	default: q[0]=next()%2?0:range(17,40);q[1]=(intptr_t)next();break;
	}
	for(int s=0;s<2;s++) {
		RustWaveMemory *m=&memories[s];
		memcpy(m->queue[m->tail],q,sizeof(q));
		if(voices[s])m->queue[m->tail][2]=(intptr_t)voices[s];
		if(codes[s])m->queue[m->tail][1]=(intptr_t)codes[s];
		if(s==0)RefWcmdqInc();else espeak_rs_wcmdq_inc(m);
	}
	commands++;
}

static void compare(const char *what,int r0,int r1)
{
	if(r0!=r1)fprintf(stderr,"%s: program %zu: %d != %d\n",what,programs,r0,r1);
	TEST_ASSERT(r0==r1);
	TEST_ASSERT(n_events[0]==n_events[1]);
	for(int i=0;i<n_events[0];i++) {
		if(!event_equal(&events[0][i],&events[1][i]))
			fprintf(stderr,"%s: program %zu event %d: op %d/%d a %d/%d b %d/%d c %d/%d d %d/%d\n",what,programs,i,
				events[0][i].op,events[1][i].op,events[0][i].a,events[1][i].a,events[0][i].b,events[1][i].b,
				events[0][i].c,events[1][i].c,events[0][i].d,events[1][i].d);
		if(!event_equal(&events[0][i],&events[1][i]))
			fprintf(stderr,"head %d command %ld %ld; reference samplecount %d of %d, pitch %d\n",memories[0].head,(long)memories[0].queue[memories[0].head][0],
				(long)memories[0].queue[memories[0].head][1],samplecount,nsamples,wdata.pitch);
		TEST_ASSERT(event_equal(&events[0][i],&events[1][i]));
	}
	n_events[0]=n_events[1]=0;
	if(memcmp(outbufs[0],outbufs[1],sizeof(outbufs[0]))!=0) {
		int i=0;while(outbufs[0][i]==outbufs[1][i])i++;
		fprintf(stderr,"%s: program %zu: output differs at byte %d of %d: %02x/%02x; head %d cmd %ld\n",what,programs,i,(int)(outputs[0].ptr-outbufs[0]),outbufs[0][i],outbufs[1][i],memories[0].head,(long)memories[0].queue[memories[0].head][0]);
	}
	TEST_ASSERT(memcmp(outbufs[0],outbufs[1],sizeof(outbufs[0]))==0);
	TEST_ASSERT(outputs[0].ptr-outbufs[0]==outputs[1].ptr-outbufs[1]);
	memories[0].echo_amp=echo_amp;
	// queue words can hold per-side allocations; the rest must match
	TEST_ASSERT(memories[0].head==memories[1].head && memories[0].tail==memories[1].tail);
	TEST_ASSERT(memcmp(memories[0].echo_buf,memories[1].echo_buf,sizeof(memories[0].echo_buf))==0);
	TEST_ASSERT(memories[0].echo_head==memories[1].echo_head && memories[0].echo_tail==memories[1].echo_tail);
	TEST_ASSERT(memories[0].echo_amp==memories[1].echo_amp);
	TEST_ASSERT(RefWcmdqFree()==espeak_rs_wcmdq_free(&memories[1]) && RefWcmdqUsed()==espeak_rs_wcmdq_used(&memories[1]));
	TEST_ASSERT(memcmp(embedded[0],embedded[1],sizeof(embedded[0]))==0);
	TEST_ASSERT(samplerate==samplerates[1]);
	TEST_ASSERT(rands[0]==rands[1]);
}

static void direct_calls(void)
{
	int r[2];
	switch(next()%5) {
	case 0: {
		int control=range(0,0x7f),value=range(-50,400);
		side=0;RefSetEmbedded(control,value);side=1;espeak_rs_wavegen_set_embedded(native,&shared,NULL,NativeEffect,control,value);
		compare("embedded",0,0);
		break;
	}
	case 1:
		side=0;r[0]=RefGetAmplitude();side=1;r[1]=espeak_rs_wavegen_amplitude(native,&shared);
		compare("amplitude",r[0],r[1]);
		break;
	case 2: {
		wavegen_peaks_t pk[N_PEAKS];int htab[2][MAX_HARMONIC];
		memset(pk,0,sizeof(pk));
		for(int i=0;i<N_PEAKS;i++) {
			pk[i].height=next()%5==0?0:range(1,64*280)<<6;
			pk[i].freq=next()%8==0?0:range(100,1000*(i+1))<<16;
			pk[i].left=range(1,900)<<16;pk[i].right=range(1,900)<<16;
		}
		// control 1 reads the spectrum being played; C's pointer is set by the first synthesis
		int control=harmspect!=NULL && next()%2?1:0,pitch=range(25,500)<<16;
		for(int s=0;s<2;s++)for(int h=0;h<MAX_HARMONIC;h++)htab[s][h]=(int)next();
		memcpy(htab[1],htab[0],sizeof(htab[0]));
		side=0;r[0]=RefPeaksToHarmspect(pk,pitch,htab[0],control);
		side=1;r[1]=espeak_rs_wavegen_harmonics(native,samplerates[1],pk,pitch,htab[1],control);
		compare("harmonics",r[0],r[1]);
		TEST_ASSERT(memcmp(htab[0],htab[1],sizeof(htab[0]))==0);
		break;
	}
	case 3: {
		voice_t v;random_voice(&v);
		side=0;RefWavegenSetVoice(&v);side=1;espeak_rs_wavegen_set_voice(native,&shared,NULL,NativeEffect,&v);
		compare("voice",0,0);
		break;
	}
	default: {
		int f0=next()%3==0?range(60,300):0;
		const_f0=f0;espeak_rs_wavegen_set_const_f0(native,f0);
		break;
	}
	}
}

static void run_program(void)
{
	static const int rates[]={8000,11025,16000,22050,22050,22050,24000,32000,40000};
	int rate=rates[next()%9],fact=next()%3==0?0:range(20,120);
	hooks=next()%3==0?NULL:&hook_sets[next()%4];
	random_voice(ref_voice); // the global voice's roughness
	unsigned r=next();rands[0]=rands[1]=r;
	for(int s=0;s<2;s++){memset(outbufs[s],0xa5,sizeof(outbufs[s]));outputs[s].start=outputs[s].ptr=outputs[s].end=outbufs[s];}
	side=0;RefWavegenInit(rate,fact);RefInitBreath();
	side=1;espeak_rs_wavegen_init(native,&shared,NULL,NativeEffect,rate,fact);espeak_rs_wavegen_init_breath(native,samplerates[1]);
#if USE_KLATT
	MockKlattInit(); // the owner initializes Klatt
#endif
	compare("init",0,0);
	if(next()%4) {
		voice_t v;random_voice(&v);
		side=0;RefWavegenSetVoice(&v);side=1;espeak_rs_wavegen_set_voice(native,&shared,NULL,NativeEffect,&v);
		compare("voice",0,0);
	}
	int n=range(1,N_WCMDQ-1-RefWcmdqUsed());
	bool alignment=hooks!=NULL && hooks->outputPhoSymbol!=NULL;
	for(int i=0;i<n;i++)push(alignment);
	for(int round=0;round<(single?100000:400);round++) {
		if(next()%8==0)direct_calls();
		int size=next()%4==0?2*range(1,8):2*range(1,OUT_SIZE/2);
		if(single)size=2; // compare after every sample
		for(int s=0;s<2;s++){memset(outbufs[s],0xa5,sizeof(outbufs[s]));outputs[s].start=outputs[s].ptr=outbufs[s];outputs[s].end=outbufs[s]+size;}
		side=0;int r0=WavegenFill2();
		side=1;int r1=NativeFill();
		samples+=(size_t)(outputs[0].ptr-outbufs[0])/2;fills++;
		compare("fill",r0,r1);
		if(r0==1 && next()%2)break;
		if(next()%16==0) {
			int more=range(0,N_WCMDQ-1-RefWcmdqUsed());
			for(int i=0;i<more;i++)push(alignment);
		}
	}
	// drain what is left so allocations are freed
	while(RefWcmdqUsed()>0) {
		for(int s=0;s<2;s++){outputs[s].start=outputs[s].ptr=outbufs[s];outputs[s].end=outbufs[s]+OUT_SIZE;}
		side=0;int r0=WavegenFill2();
		side=1;int r1=NativeFill();
		compare("drain",r0,r1);
	}
	programs++;
}

int main(void)
{
	single=getenv("WAVEGEN_ORACLE_SINGLE")!=NULL;
	native=espeak_rs_wavegen_new();
	TEST_ASSERT(native!=NULL);
	for(int i=0;i<DATA_SIZE;i++)sample_data[i]=(unsigned char)next();
	for(int e=0;e<8;e++)for(int i=0;i<128;i++)envelopes[e][i]=(unsigned char)next();
	for(int i=0;i<256;i++)random_frame(&frames[i]);
	for(int p=0;p<(single?1500:400);p++)run_program();
	espeak_rs_wavegen_free(native);
	printf("wavegen oracle: %zu programs, %zu commands, %zu fills, %zu samples matched\n",programs,commands,fills,samples);
	return EXIT_SUCCESS;
}
