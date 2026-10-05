/* Native waveform calibration, MBROLA pitch text and PCM versus retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "common.h"
#include "phoneme.h"
#include "speech.h"
#include "synthesize.h"
#include "translate.h"
#include "voice.h"
#include "wavegen.h"
#include "rust_data.h"
static voice_t *wvoice;
static int amp_ix,amp_inc,general_amplitude;
static unsigned char *amplitude_env;
static struct {int amplitude,amplitude_v;} wdata;
#define GetAmplitude ReferenceGetAmplitude
#define SetAmplitude ReferenceSetAmplitude
#define SetPitch2 ReferenceSetPitch2
#define SetPitchFormants ReferenceSetPitchFormants
#include "wave_parameters_reference.inc"
#include "mbrola_pitch_reference.inc"
#undef GetAmplitude
#undef SetAmplitude
#undef SetPitch2
#undef SetPitchFormants
static unsigned seed=0x48134563u;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t pitches,formants,amplitudes,contours,samples;
static void calibration(void)
{
	for(int trial=0;trial<200000;trial++) {
		voice_t actual={0};actual.pitch_base=(int)(next()%500000)-200000;actual.pitch_range=(int)(next()%10000)-5000;
		actual.consonant_ampv=(int)(next()%401)-200;
		for(int i=0;i<N_PEAKS;i++){actual.freq2[i]=(int16_t)next();actual.height2[i]=(int16_t)next();actual.freq[i]=(int16_t)next();actual.height[i]=(int16_t)next();}
		voice_t expected=actual;wvoice=&expected;
		embedded_value[EMBED_P]=(int)(next()%241)-40;embedded_value[EMBED_T]=next()%31;embedded_value[EMBED_R]=(int)(next()%201)-50;
		int first=(int)(next()%301)-30,second=(int)(next()%301)-30,base,range;
		ReferenceSetPitch2(&expected,first,second,&base,&range);
		int actual_base=77,actual_range=88;SetPitch2(&actual,first,second,&actual_base,&actual_range);
		TEST_ASSERT(actual_base==base && actual_range==range);pitches++;
		ReferenceSetPitchFormants();TEST_ASSERT(espeak_rs_pitch_formants(&actual,embedded_value[EMBED_P],embedded_value[EMBED_T])==0);
		TEST_ASSERT(memcmp(&actual,&expected,sizeof(actual))==0);formants++;
		embedded_value[EMBED_A]=(int)(next()%1001)-500;embedded_value[EMBED_F]=next()%5;
		int amplitude=ReferenceGetAmplitude();TEST_ASSERT(GetAmplitude()==amplitude);
		int length=(trial%10==0)?0:(int)(next()%15000)-5000,value=(int)(next()%401)-200;
		ReferenceSetAmplitude(length,NULL,value);RustAmplitude result={0};
		TEST_ASSERT(espeak_rs_amplitude(length,value,general_amplitude,actual.consonant_ampv,&result)==0);
		TEST_ASSERT(result.increment==amp_inc && result.value==wdata.amplitude && result.voiced==wdata.amplitude_v && amp_ix==0 && amplitude_env==NULL);amplitudes++;
	}
	voice_t actual={0};actual.pitch_base=INT_MAX;actual.pitch_range=100;voice_t saved=actual;
	RustEmbeddedPitch embedded={50,0,50};RustPitch pitch={77,88};
	TEST_ASSERT(espeak_rs_pitch(&actual,1,2,&embedded,&pitch)!=0);TEST_ASSERT(pitch.base==77 && pitch.range==88);
	TEST_ASSERT(espeak_rs_pitch_formants(&actual,101,INT_MAX)!=0);TEST_ASSERT(memcmp(&actual,&saved,sizeof(actual))==0);
	RustAmplitude amplitude={77,88,99};TEST_ASSERT(espeak_rs_amplitude(1,INT_MAX,60,40,&amplitude)!=0);TEST_ASSERT(amplitude.increment==77 && amplitude.value==88 && amplitude.voiced==99);
	int32_t general=77;TEST_ASSERT(espeak_rs_general_amplitude(100,5,&general)!=0 && general==77);
	wvoice=NULL;
}
static void pitch_contours(void)
{
	voice_t snapshot={0};snapshot.pitch_base=100*4096;snapshot.pitch_range=4096;
	voice_t *saved=voice;voice=&snapshot;
	const int splits[]={-100,-75,-50,-25,-1,0,1,25,50,75,100};
	for(int env=0;env<N_ENVELOPE_DATA;env++)if(envelope_data[env])for(int trial=0;trial<300;trial++) {
		embedded_value[EMBED_P]=next()%102;embedded_value[EMBED_T]=next()%31;embedded_value[EMBED_R]=next()%101;
		int first=next()%100,second=next()%100,split=splits[next()%(sizeof(splits)/sizeof(*splits))],final=next()%2;
		char *expected=WritePitch(env,first,second,split,final);unsigned char actual[128];memset(actual,0x5a,sizeof(actual));
		RustPitch pitch;ReferenceSetPitch2(voice,first,second,&pitch.base,&pitch.range);
		TEST_ASSERT(espeak_rs_mbrola_pitch((const unsigned char (*)[128])envelope_data[env],env,&pitch,split,final,actual,sizeof(actual))==0);
		TEST_ASSERT(strcmp((char*)actual,expected)==0);TEST_ASSERT(actual[strlen((char*)actual)+1]==0x5a);contours++;
	}
	unsigned char actual[5];memset(actual,0x5a,5);RustPitch pitch={409600,4096};
	TEST_ASSERT(espeak_rs_mbrola_pitch((const unsigned char (*)[128])envelope_data[0],0,&pitch,0,0,actual,5)!=0);
	for(int i=0;i<5;i++)TEST_ASSERT(actual[i]==0x5a);
	voice=saved;
}
static void reference_scale(unsigned char *out_ptr,int result,int amplitude)
{
	int ix,value;short value16;
#include "mbrola_pcm_reference.inc"
}
static void pcm(void)
{
	size_t length=65536*2;unsigned char *actual=malloc(length),*expected=malloc(length);TEST_ASSERT(actual&&expected);
	const int amplitudes[]={-65535,-32768,-1024,-100,-40,-1,0,1,40,100,1024,32768,65535};
	for(size_t trial=0;trial<sizeof(amplitudes)/sizeof(*amplitudes);trial++) {
		for(int value=-32768;value<=32767;value++){size_t index=(value+32768)*2;actual[index]=(unsigned)value&0xff;actual[index+1]=((unsigned)value>>8)&0xff;}
		memcpy(expected,actual,length);reference_scale(expected,65536,amplitudes[trial]);TEST_ASSERT(espeak_rs_mbrola_scale(actual,length,amplitudes[trial])==0);
		TEST_ASSERT(memcmp(actual,expected,length)==0);samples+=65536;
	}
	unsigned char bad[]={1,0,2,0},saved[4];memcpy(saved,bad,4);TEST_ASSERT(espeak_rs_mbrola_scale(bad,4,INT_MAX)!=0);TEST_ASSERT(memcmp(bad,saved,4)==0);
	TEST_ASSERT(espeak_rs_mbrola_scale(bad,3,40)!=0);TEST_ASSERT(memcmp(bad,saved,4)==0);
	free(actual);free(expected);
}
int main(void)
{
	_Static_assert(sizeof(RustPitch)==8,"pitch layout");_Static_assert(sizeof(RustEmbeddedPitch)==12,"embedding layout");_Static_assert(sizeof(RustAmplitude)==12,"amplitude layout");
	calibration();pitch_contours();pcm();
	printf("Matched %zu pitch, %zu formant, %zu amplitude snapshots, %zu contours and %zu PCM samples\n",pitches,formants,amplitudes,contours,samples);
	return 0;
}
