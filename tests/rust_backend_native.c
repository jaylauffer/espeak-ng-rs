/* Whole unchanged previous-native waveform/Klatt hosts and real queue data.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_data.h"
#include "rust_klatt.h"
#include "wavegen.h"
#include "synthdata.h"
#include "intonation.h"
#include "setlengths.h"
#include "speech.h"
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
RustWaveMemory previous_memory;
RustOutput previous_output;
int previous_samplerate,previous_embedded[N_EMBEDDED_VALUES];
espeak_ng_OUTPUT_HOOKS *previous_hooks;
RustSonicState previous_sonic={1.0,NULL};
RustWavegen *previous_NativeWavegenState(void){static RustWavegen *state; if(!state)state=espeak_rs_wavegen_new();TEST_ASSERT(state);return state;}
int previous_NativeOutputHooks(espeak_ng_OUTPUT_HOOKS *hooks){previous_hooks=hooks;return ENS_OK;}
void previous_WavegenInit(int,int),previous_WavegenFini(void),previous_WavegenSetVoice(voice_t *);
void previous_SetEmbedded(int,int),previous_InitBreath(void),previous_WcmdqStop(void),previous_WcmdqInc(void);
int previous_WavegenFill(void),previous_GetAmplitude(void),previous_WcmdqFree(void),previous_WcmdqUsed(void);
int previous_PeaksToHarmspect(wavegen_peaks_t *,int,int *,int);
void previous_SetPitch2(voice_t *,int,int,int *,int *),previous_Write4Bytes(FILE *,int);
int previous_SetOutputHooks(espeak_ng_OUTPUT_HOOKS *),previous_SetConstF0(int);
void KlattInit(void),KlattReset(int),KlattFini(void),previous_KlattInit(void),previous_KlattReset(int),previous_KlattFini(void);
int Wavegen_Klatt(int,int,frame_t *,frame_t *,WGEN_DATA *,voice_t *);
int previous_Wavegen_Klatt(int,int,frame_t *,frame_t *,WGEN_DATA *,voice_t *);
/* Original C marker primitives read the product's output-buffer base/rate.
 * Fence those borrowed scalar views for the independent previous host only. */
void previous_MarkerEvent(int kind,unsigned position,int value,int value2,unsigned char *cursor)
{
 unsigned char *base=espeak_rs_output.buffer;int rate=samplerate;
 espeak_rs_output.buffer=previous_output.buffer;samplerate=previous_samplerate;
 MarkerEvent(kind,position,value,value2,cursor);
 espeak_rs_output.buffer=base;samplerate=rate;
}
void previous_RescaleEventSamples(int input,int output)
{
#if USE_LIBSONIC
 int rate=samplerate;samplerate=previous_samplerate;RescaleEventSamples(input,output);samplerate=rate;
#else
 (void)input;(void)output;
#endif
}
#define BUFFER_SIZE 256
#define PCM_LIMIT 500000
static _Alignas(short) unsigned char buffers[2][BUFFER_SIZE],pcm[2][PCM_LIMIT];
static size_t lengths[2];
static unsigned waits[2],hook_counts[2][3],side,episodes,klatt_programs;
static espeak_EVENT saved_events[2][4096];
static unsigned event_counts[2];
static RustWaveMemory initial;
static void voiced(short value){(void)value;hook_counts[side][0]++;}
static void silence(short value){(void)value;hook_counts[side][1]++;}
static void unvoiced(short value){(void)value;hook_counts[side][2]++;}
static espeak_ng_OUTPUT_HOOKS hooks={NULL,silence,voiced,unvoiced};
static void reset(unsigned which)
{
 side=which;lengths[which]=waits[which]=event_counts[which]=0;memset(hook_counts[which],0,sizeof(hook_counts[which]));
 RustWaveMemory *memory=which?&previous_memory:&espeak_rs_wave_memory;
 *memory=initial;
 RustOutput *output=which?&previous_output:&espeak_rs_output;
 output->start=output->ptr=output->buffer=buffers[which];output->end=buffers[which]+BUFFER_SIZE;output->size=BUFFER_SIZE;
}
static void collect_events(unsigned which)
{
 TEST_ASSERT(event_counts[which]+espeak_rs_events.count<4096);
 memcpy(saved_events[which]+event_counts[which],espeak_rs_events.events,(size_t)espeak_rs_events.count*sizeof(espeak_EVENT));
 event_counts[which]+=(unsigned)espeak_rs_events.count;espeak_rs_events.count=0;
}
static void run(unsigned which,int rate)
{
 reset(which);
 if(which){previous_WavegenInit(rate,0);previous_WavegenSetVoice(voice);previous_SetOutputHooks(&hooks);previous_SetConstF0(0);}
 else{WavegenInit(rate,0);WavegenSetVoice(voice);espeak_ng_SetOutputHooks(&hooks);espeak_ng_SetConstF0(0);}
 collect_events(which);
 for(unsigned turn=0;;turn++){
  TEST_ASSERT(turn<10000);
  RustOutput *output=which?&previous_output:&espeak_rs_output;output->ptr=output->start=buffers[which];
  int result=which?previous_WavegenFill():WavegenFill();
  size_t length=(size_t)(output->ptr-buffers[which]);TEST_ASSERT(length<=BUFFER_SIZE&&lengths[which]+length<=PCM_LIMIT);
  memcpy(pcm[which]+lengths[which],buffers[which],length);lengths[which]+=length;
  collect_events(which);
  if(result)break;waits[which]++;
 }
 if(which){previous_WcmdqStop();previous_WavegenFini();previous_SetOutputHooks(NULL);}
 else{WcmdqStop();WavegenFini();espeak_ng_SetOutputHooks(NULL);}
}
static int reentry_calls;
static void reentry(short value)
{
 (void)value;if(reentry_calls++)return;
 TEST_ASSERT(WavegenFill()==1);TEST_ASSERT(espeak_ng_SetConstF0(99)==EAGAIN);
 WavegenInit(8000,1);WavegenFini();WcmdqStop();
 TEST_ASSERT(samplerate==22050);espeak_ng_SetOutputHooks(NULL);
}
int main(void)
{
 TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)==22050);
 RustOutput saved_output=espeak_rs_output;
 if(!p_decoder)p_decoder=create_text_decoder();TEST_ASSERT(p_decoder);
 TEST_ASSERT(espeak_rs_events_reserve(&espeak_rs_events,256)==0);
 /* Match initialization performed before the product host's first trial. */
 previous_WavegenInit(22050,0);
 static const char *voices[]={"en","fr","ga","de","ru","cmn","vi","en+klatt","en+klatt2","en+klatt3","en+klatt4","en+klatt5"};
 static const int rates[]={22050,16000,32000,44100};
 for(unsigned trial=0;trial<96;trial++){
  TEST_ASSERT(espeak_SetVoiceByName(voices[trial%12])==EE_OK);
  TEST_ASSERT(voice->klattv[0]!=6);
  TEST_ASSERT(text_decoder_decode_string(p_decoder,"A short phrase with vowels, consonants and 123 words.",-1,ESPEAKNG_ENCODING_UTF_8)==ENS_OK);
  int tone;char *change;TranslateClause(translator,&tone,&change);CalcPitches(translator,tone);CalcLengths(translator);
  WcmdqStop();TEST_ASSERT(Generate(phoneme_list,&n_phoneme_list,false)==0);
  initial=espeak_rs_wave_memory;
  /* Equal live queue programs, independent waveform and Klatt slots. */
  int rate=rates[(trial/12)%4];run(0,rate);run(1,rate);
  TEST_ASSERT(lengths[0]==lengths[1]&&waits[0]==waits[1]);TEST_ASSERT(!memcmp(pcm[0],pcm[1],lengths[0]));
  TEST_ASSERT(!memcmp(hook_counts[0],hook_counts[1],sizeof(hook_counts[0])));
  TEST_ASSERT(event_counts[0]==event_counts[1]&&!memcmp(saved_events[0],saved_events[1],event_counts[0]*sizeof(espeak_EVENT)));
  TEST_ASSERT(!memcmp(&espeak_rs_wave_memory,&previous_memory,sizeof(previous_memory)));
  TEST_ASSERT(samplerate==previous_samplerate&&!memcmp(embedded_value,previous_embedded,sizeof(previous_embedded)));episodes++;
 }
 unsigned pitches=0;
 for(unsigned i=0;i<1024;i++){
  int a,b,c,d;SetPitch2(voice,(int)(i%256),(int)((i*17)%256),&a,&b);previous_SetPitch2(voice,(int)(i%256),(int)((i*17)%256),&c,&d);
  TEST_ASSERT(a==c&&b==d);pitches++;
 }
 for(unsigned i=0;i<256;i++){
  int control=(int)(i%6)+1,value=(int)(i%101);
  SetEmbedded(control,value);previous_SetEmbedded(control,value);TEST_ASSERT(GetAmplitude()==previous_GetAmplitude());
  TEST_ASSERT(!memcmp(embedded_value,previous_embedded,sizeof(previous_embedded)));
 }
 FILE *a=tmpfile(),*b=tmpfile();TEST_ASSERT(a&&b);
 for(int i=0;i<10000;i++){Write4Bytes(a,i*71111);previous_Write4Bytes(b,i*71111);}
 rewind(a);rewind(b);for(int i=0;i<40000;i++)TEST_ASSERT(fgetc(a)==fgetc(b));fclose(a);fclose(b);
 memset(&initial,0,sizeof(initial));initial.tail=1;initial.queue[0][0]=5;initial.queue[0][1]=100;
 reset(0);WavegenInit(22050,0);WavegenSetVoice(voice);
 espeak_ng_OUTPUT_HOOKS guard={NULL,reentry,reentry,reentry};reentry_calls=0;espeak_ng_SetOutputHooks(&guard);
 TEST_ASSERT(WavegenFill()==1&&reentry_calls>0);TEST_ASSERT(wcmdq_head==wcmdq_tail);
 espeak_ng_SetOutputHooks(NULL);WcmdqStop();WavegenFini();
 /* Output buffers here are static test storage, not owned allocations. */
 espeak_rs_output=saved_output;memset(&previous_output,0,sizeof(previous_output));
 espeak_Terminate();
 printf("%u complete previous-native waveform/Klatt episodes across 12 voices/four sample rates, %u pitch cases,256 embedded cases and10000 stream words; PCM/queues/echo/events/hooks/resumption and native reentry guards match\n",episodes,pitches);
 (void)klatt_programs;return 0;
}
