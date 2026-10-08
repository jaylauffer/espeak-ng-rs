/* Whole native Klatt versus independently extracted retained C. Includes
 * partial-buffer resume, all five sources, interpolation, discontinuities,
 * mix samples, echo and repeated resets/initialization.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <math.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include "klatt.h"
#include "rust_klatt.h"
#include "test_wgen_data.h"

static RustWaveMemory memories[2];
static RustOutput outputs[2];
static unsigned char buffers[2][8192],envelope[128],mixed[8192];
static uint32_t randoms[2];
static int side,resets[2];
static size_t invalid_pcm;
static int stress_resume,trial_number;
/* The original C cast is undefined outside the int range. Tiny-buffer
 * parameter overshoot exercises that domain; Rust specifies saturation and
 * NaN->0. This guarded oracle leaves every defined truncation unchanged. */
static int NormalizedPcm(double value)
{
	double whole=trunc(value);
	if(!stress_resume && (isnan(whole) || whole>INT_MAX || whole<INT_MIN)) {
		fprintf(stderr,"defined-domain Klatt fixture escaped at trial %d: %.17g\n",trial_number,value);
		TEST_ASSERT(0);
	}
	if(isnan(whole)){invalid_pcm++;return 0;}
	if(whole>INT_MAX){invalid_pcm++;return INT_MAX;}
	if(whole<INT_MIN){invalid_pcm++;return INT_MIN;}
	return (int)value;
}
static int NormalizedEcho(double sample,int echo)
{
	/* Native PCM conversion followed by wrapping echo addition. The C
	 * sum itself is undefined if saturation plus echo overflows signed int. */
	int pcm=NormalizedPcm(sample);
	int64_t sum=(int64_t)pcm+echo;
	TEST_ASSERT(stress_resume || (sum>=INT_MIN && sum<=INT_MAX));
	uint32_t bits=(uint32_t)pcm+(uint32_t)echo;
	int32_t value;memcpy(&value,&bits,sizeof(value));return value;
}
static long Rand(long min,long max)
{
	randoms[side]=randoms[side]*1103515245u+12345u;
	return min+(long)((randoms[side]>>8)%(unsigned)(max-min+1));
}
static int32_t Random(void) { return (int32_t)Rand(-8191,8191); }
static void ResetSP(void) { resets[side]++; }

/* Compile the original source over the test's side-0 queue and PCM/echo
 * memory, without a Rust echo helper or a speechPlayer dependency. */
#undef USE_RUST_CORE
#undef USE_SPEECHPLAYER
#define USE_SPEECHPLAYER 0
#undef wcmdq
#undef wcmdq_head
#undef wcmdq_tail
#define wcmdq memories[0].queue
#define wcmdq_head memories[0].head
#define wcmdq_tail memories[0].tail
#undef out_ptr
#undef out_end
#define out_ptr outputs[0].ptr
#define out_end outputs[0].end
#define echo_buf memories[0].echo_buf
#define echo_head memories[0].echo_head
#define echo_tail memories[0].echo_tail
int echo_amp;
#define espeak_rand Rand
#define KlattInit ReferenceInit
#define KlattReset ReferenceReset
#define KlattFini ReferenceFini
#define Wavegen_Klatt ReferenceFill
#include "klatt_reference.inc"
#undef KlattInit
#undef KlattReset
#undef KlattFini
#undef Wavegen_Klatt
#undef echo_buf
#undef echo_head
#undef echo_tail

static unsigned fixture=0x346abd91u;
static unsigned next(void) { fixture^=fixture<<13;fixture^=fixture>>17;fixture^=fixture<<5;return fixture; }
static size_t samples,calls;
static void short_frame_at_guard_page(void)
{
	long page=sysconf(_SC_PAGESIZE);TEST_ASSERT(page>=64);
	unsigned char *mapping=mmap(NULL,(size_t)page*2,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANON,-1,0);
	TEST_ASSERT(mapping!=MAP_FAILED);
	TEST_ASSERT(mprotect(mapping+page,(size_t)page,PROT_NONE)==0);
	frame_t ordinary={0};ordinary.ffreq[1]=688;ordinary.ffreq[2]=1064;
	unsigned char *bytes=mapping+page-44;memcpy(bytes,&ordinary,44);
	voice_t voice={0};voice.klattv[0]=1;
	for(int i=0;i<N_PEAKS;i++){voice.freq[i]=256;voice.width[i]=256;}
	WGEN_DATA data={0};data.pitch_env=envelope;data.pitch=data.pitch_base=100*4096;
	memset(&memories[1],0,sizeof(memories[1]));
	outputs[1].start=outputs[1].ptr=buffers[1];outputs[1].end=buffers[1]+sizeof(buffers[1]);
	RustKlattShared shared={&memories[1],&outputs[1],Random,ResetSP};
	RustKlatt *state=espeak_rs_klatt_new();TEST_ASSERT(state);side=1;
	TEST_ASSERT(espeak_rs_klatt_fill(state,&shared,64,0,(frame_t *)bytes,(frame_t *)bytes,&data,&voice)==0);
	TEST_ASSERT(outputs[1].ptr-buffers[1]==256);
	espeak_rs_klatt_free(state);
	TEST_ASSERT(munmap(mapping,(size_t)page*2)==0);
}
static void initialize(RustKlatt *state)
{
	side=0;ReferenceInit();
	side=1;espeak_rs_klatt_init(state);
}
static void trial(RustKlatt *state,int number)
{
	trial_number=number;
	frame_t frames[3]={{0}};
	voice_t voice={0};
	voice.klattv[0]=number%5+1;
	voice.flutter=(int)(next()%1025);
	for(int f=0;f<3;f++) {
		frame_t *fr=&frames[f];fr->frflags=number%7?FRFLAG_KLATT:0;
		for(int i=0;i<N_PEAKS && i<7;i++) {
			fr->ffreq[i]=(short)(300+i*730+(int)(next()%401));
			fr->klatt_ap[i]=(unsigned char)(next()%65);
			fr->klatt_bp[i]=(unsigned char)(10+next()%80);
			voice.freq[i]=(short)(200+next()%90);voice.width[i]=(short)(200+next()%100);
			voice.freqadd[i]=(short)((int)(next()%81)-40);
		}
		for(int i=0;i<4;i++)fr->bw[i]=(unsigned char)(20+next()%80);
		fr->klattp[KLATT_AV]=(unsigned char)(40+next()%31);
		fr->klattp[KLATT_FNZ]=(unsigned char)(number%3?next()%151:0);
		fr->klattp[KLATT_Tilt]=(unsigned char)(next()%25);
		fr->klattp[KLATT_Aspr]=(unsigned char)(next()%50);
		fr->klattp[KLATT_Skew]=(unsigned char)(next()%21);
	}
	if(number%3==0)frames[2]=frames[1];
	if(!stress_resume && number%11==0) {
		/* One-sample resumes keep C's 64-sample advancement. Use stationary
		 * endpoints to exercise resume without driving bandwidths negative.
		 * The original moving endpoints remain in the separate stress lane. */
		frames[1]=frames[0];
		for(int i=0;i<N_PEAKS;i++)voice.width[i]=256;
	}
	memset(memories,0,sizeof(memories));
	for(int s=0;s<2;s++) {
		memories[s].head=number%170;memories[s].tail=(memories[s].head+3)%170;
		int q=(memories[s].head+1)%170;
		memories[s].queue[q][0]=number%4==0?WCMD_PAUSE:WCMD_MARKER;
		q=(q+1)%170;memories[s].queue[q][0]=number%4==1?WCMD_WAVE:WCMD_KLATT;
		memories[s].queue[q][2]=(intptr_t)&frames[2];
		memories[s].echo_amp=number%3==0?80:0;
		for(int i=0;i<N_ECHO_BUF;i++)memories[s].echo_buf[i]=(short)(i*37);
		memories[s].echo_tail=number%5500;memories[s].echo_head=(number+900)%5500;
	}
	echo_amp=memories[0].echo_amp;
	WGEN_DATA initial={0};initial.pitch_env=envelope;initial.pitch_base=80*4096;
	initial.pitch_range=60*4096;initial.pitch=140*4096;initial.pitch_inc=(int)(next()%90);
	initial.amplitude=(int)(20+next()%120);initial.amplitude_v=(int)(100+next()%700);
	if(number%2) {
		initial.mix_wavefile=mixed;initial.n_mix_wavefile=4000;
		initial.mix_wavefile_max=4096;initial.mix_wavefile_offset=0;
		initial.mix_wave_scale=number%3==0?256:0;initial.mix_wave_amp=(int)(next()%50);
	}
	WGEN_DATA data[2]={initial,initial};
	int length=1+(int)(next()%1800),resume=0,full[2];
	RustKlattShared shared={&memories[1],&outputs[1],Random,ResetSP};
	for(int pass=0;;pass++) {
		TEST_ASSERT(pass<2000);
		int size=number%11==0?2:2*(1+(int)(next()%4096));
		for(int s=0;s<2;s++) {
			memset(buffers[s],0xcd,sizeof(buffers[s]));outputs[s].start=buffers[s];
			outputs[s].ptr=buffers[s];outputs[s].end=buffers[s]+size;
			side=s;
			full[s]=s?espeak_rs_klatt_fill(state,&shared,length,resume,&frames[0],&frames[1],&data[s],&voice):
				ReferenceFill(length,resume,&frames[0],&frames[1],&data[s],&voice);
		}
		if(full[0]!=full[1] || memcmp(buffers[0],buffers[1],sizeof(buffers[0])) ||
			outputs[0].ptr-buffers[0]!=outputs[1].ptr-buffers[1] ||
			!test_wgen_equal(&data[0],&data[1]) || randoms[0]!=randoms[1] ||
			memcmp(memories[0].echo_buf,memories[1].echo_buf,sizeof(memories[0].echo_buf)) ||
			memories[0].echo_head!=memories[1].echo_head || memories[0].echo_tail!=memories[1].echo_tail) {
			fprintf(stderr,"Klatt mismatch trial=%d source=%d pass=%d size=%d full=%d/%d bytes=%ld/%ld pitch_ix=%d/%d\n",
				number,voice.klattv[0],pass,size,full[0],full[1],(long)(outputs[0].ptr-buffers[0]),
				(long)(outputs[1].ptr-buffers[1]),data[0].pitch_ix,data[1].pitch_ix);
			for(int i=0;i<size;i++)if(buffers[0][i]!=buffers[1][i]){fprintf(stderr,"first byte %d: %u/%u\n",i,buffers[0][i],buffers[1][i]);break;}
			TEST_ASSERT(0);
		}
		samples+=(size_t)(outputs[0].ptr-buffers[0])/2;calls++;
		if(!full[0])break;
		resume=1;
	}
	if(number%9==0) { int control=(number/9)%3;side=0;ReferenceReset(control);side=1;espeak_rs_klatt_reset(state,control); }
}
int main(int argc,char **argv)
{
	if(argc==2 && strcmp(argv[1],"--stress-resume")==0)stress_resume=1;
	else if(argc!=1){fprintf(stderr,"usage: %s [--stress-resume]\n",argv[0]);return 2;}
	RustKlatt *state=espeak_rs_klatt_new();TEST_ASSERT(state);
	for(int i=0;i<128;i++)envelope[i]=(unsigned char)(i*2);
	for(int i=0;i<8192;i++)mixed[i]=(unsigned char)(i*173);
	randoms[0]=randoms[1]=0x348201u;initialize(state);
	for(int i=0;i<3000;i++) { if(i%17==0)initialize(state);trial(state,i); }
	espeak_rs_klatt_free(state);
	short_frame_at_guard_page();
	printf("Klatt C parity: 3000 commands, %zu calls, %zu samples; %zu undefined C PCM conversions normalized\n",calls,samples,invalid_pcm);
	return 0;
}
