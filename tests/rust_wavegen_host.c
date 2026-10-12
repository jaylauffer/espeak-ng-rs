/* Complete preceding native effect/Sonic/fill/stop controllers vs Rust.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_data.h"
#include "speech.h"
#include <errno.h>
#include <pthread.h>
#include <stdio.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#undef USE_KLATT
#undef USE_MBROLA
#undef USE_LIBSONIC
#define USE_KLATT 1
#define USE_MBROLA 1
#define USE_LIBSONIC 1
typedef void *sonicStream;
static RustWaveMemory memory;
static RustOutput output;
static RustSonicState sonic;
static _Alignas(short) unsigned char bytes[512];
static char backend_name[40],alignment_text[]="Native alignment";
static voice_t acoustic;
static WGEN_DATA data;
typedef struct {float speed;int pending;} StreamState;
static StreamState streams[3];
static unsigned config,pairs,guards;
static int native_guard;
static RustWavegenHost *live_descriptor;
typedef struct {
 RustWaveMemory memory;unsigned char bytes[512];voice_t acoustic;WGEN_DATA data;
 double speed;int stream,ptr,end,hooks;unsigned char streams[sizeof(streams)];
} State;
typedef struct {int op,a,b,c,d;State state;} Trace;
static Trace trace[2][32];static unsigned counts[2];static int lane;
static espeak_ng_OUTPUT_HOOKS hook_sets[2];
static int Stream(void *p){return p?(int)((char *)p-(char *)streams)+1:0;}
static void Capture(State *s)
{
 memset(s,0,sizeof(*s));s->memory=memory;memcpy(s->bytes,bytes,sizeof(bytes));s->acoustic=acoustic;s->data=data;
 s->speed=sonic.speed;s->stream=Stream(sonic.stream);s->ptr=(int)((uintptr_t)output.ptr-(uintptr_t)bytes);s->end=(int)((uintptr_t)output.end-(uintptr_t)bytes);
 s->hooks=output_hooks==&hook_sets[1];memcpy(s->streams,streams,sizeof(streams));
}
static void *Nested(void *opaque)
{
 RustWavegenEffect e={.op=6,.a=1,.b=9};int out=0x55;
 TEST_ASSERT(espeak_rs_wavegen_host_effect(opaque,&e,&out)==EAGAIN&&out==0x55);
 TEST_ASSERT(espeak_rs_wavegen_host_fill(opaque,&out)==EAGAIN&&out==0x55);
 TEST_ASSERT(espeak_rs_wavegen_host_stop(opaque)==EAGAIN);return NULL;
}
static void Observe(int op,int a,int b,int c,int d)
{
 TEST_ASSERT(counts[lane]<32);Trace *t=&trace[lane][counts[lane]++];memset(t,0,sizeof(*t));t->op=op;t->a=a;t->b=b;t->c=c;t->d=d;Capture(&t->state);
 if(native_guard){native_guard=0;Nested(live_descriptor);pthread_t thread;TEST_ASSERT(pthread_create(&thread,NULL,Nested,live_descriptor)==0&&pthread_join(thread,NULL)==0);live_descriptor->marker=NULL;live_descriptor->fill=NULL;live_descriptor->rescale=NULL;}
}
static void HookPho(char *text,int type){Observe(1,text==alignment_text,type,0,0);if(config&32)memory.queue[0][1]=0;}
static void HookSilence(short sample){Observe(2,sample,0,0,0);if(config&8)TEST_ASSERT(espeak_ng_SetOutputHooks(&hook_sets[1])==0);}
static void HookVoiced(short sample){Observe(3,sample,0,0,0);if(config&8)TEST_ASSERT(espeak_ng_SetOutputHooks(&hook_sets[1])==0);}
static void HookUnvoiced(short sample){Observe(4,sample,0,0,0);if(config&8)TEST_ASSERT(espeak_ng_SetOutputHooks(&hook_sets[1])==0);}
static void HookOther(short sample){Observe(5,sample,0,0,0);}
static espeak_ng_OUTPUT_HOOKS hook_sets[2]={
 {HookPho,HookSilence,HookVoiced,HookUnvoiced},{HookPho,HookOther,HookOther,HookOther}
};
static void Marker(int type,unsigned int position,int value,int value2,unsigned char *cursor)
{Observe(6,type,(int)position,value,value2);Observe(7,(int)(cursor-bytes),0,0,0);}
static void Release(void *p){Observe(8,p==alignment_text,p==&acoustic,0,0);}
static int Random(int minimum,int maximum){Observe(9,minimum,maximum,0,0);return minimum+maximum;}
static void ResetKlatt(int control){Observe(10,control,0,0,0);}
static int Klatt(int length,int resume,frame_t *first,frame_t *second,WGEN_DATA *d,voice_t *v)
{Observe(11,length,resume,(int)(uintptr_t)first,(int)(uintptr_t)second);d->pitch_ix+=11;v->roughness+=3;return (int)(config%3);}
static int NativeKlatt(const RustWavegenEffect *e){return Klatt(e->a,e->b,(frame_t*)e->value,(frame_t*)e->value2,e->data,e->voice);}
static int Mbrola(int length,bool resume,int amplitude){Observe(12,length,resume,amplitude,0);return (int)(config%2);}
static int NativeMbrola(int length,int resume,int amplitude){return Mbrola(length,resume,amplitude);}
static void ResetMbrola(void){Observe(13,0,0,0,0);}
static void *Create(int rate,int channels){Observe(14,rate,channels,0,0);return config&256?NULL:&streams[0];}
static void Destroy(void *p){Observe(15,Stream(p),0,0,0);if(config&16)sonic.stream=&streams[2];}
static int Flush(void *p){Observe(16,Stream(p),0,0,0);if(config&4)output.ptr+=4;return 1;}
static float GetSpeed(void *p){Observe(17,Stream(p),0,0,0);float speed=((StreamState *)p)->speed;if(config&1)sonic.stream=&streams[1];return speed;}
static void MockSonicSetSpeed(void *p,float speed){Observe(18,Stream(p),(int)(speed*1024),0,0);((StreamState*)p)->speed=speed;}
static int Write(void *p,short *source,int samples){Observe(19,Stream(p),(int)((unsigned char*)source-bytes),samples,0);((StreamState*)p)->pending+=samples;return 1;}
static int Read(void *p,short *destination,int capacity)
{
 Observe(20,Stream(p),(int)((unsigned char*)destination-bytes),capacity,0);int n=(int)(config%13);if(n>capacity)n=capacity;
 for(int i=0;i<n;i++)destination[i]=(short)(i*71+config);if(config&2)output.ptr+=8;return config&128?capacity+1:n;
}
static int Fill(void *context)
{
 (void)context;Observe(21,0,0,0,0);int n=(int)(config%17);for(int i=0;i<n;i++){output.ptr[0]=(unsigned char)i;output.ptr[1]=0;output.ptr+=2;}
 if(config&64)sonic.speed=1.0;if(config&512)output.ptr=bytes+200;return config%2;
}
static int CoreFill(void){return Fill(NULL);}
static void Rescale(int before,int after){Observe(22,before,after,0,0);}
static RustWavegenHost Descriptor(unsigned features)
{
 RustWavegenHost d={0};d.memory=&memory;d.output=&output;d.hooks=&output_hooks;d.sonic=&sonic;d.mbrola_name=backend_name;d.features=features;
 d.marker=Marker;d.release=Release;d.random=Random;d.klatt_reset=ResetKlatt;d.klatt=NativeKlatt;d.mbrola=NativeMbrola;d.mbrola_reset=ResetMbrola;
 d.create=Create;d.destroy=Destroy;d.flush=Flush;d.read=Read;d.write=Write;d.get_speed=GetSpeed;d.set_speed=MockSonicSetSpeed;d.fill=Fill;d.rescale=Rescale;return d;
}
#undef wcmdq
#undef out_ptr
#undef out_end
#define wcmdq memory.queue
#define out_ptr output.ptr
#define out_end output.end
#define sonicSpeed sonic.speed
#define sonicSpeedupStream sonic.stream
#define MarkerEvent Marker
#define free Release
#define espeak_rand Random
#define KlattReset ResetKlatt
#define Wavegen_Klatt Klatt
#define MbrolaFill Mbrola
#define MbrolaReset ResetMbrola
#define mbrola_name backend_name
#define espeak_rs_wave_memory memory
#define sonicCreateStream Create
#define sonicDestroyStream Destroy
#define sonicFlushStream Flush
#define sonicReadShortFromStream Read
#define sonicWriteShortToStream Write
#define sonicGetSpeed GetSpeed
#define sonicSetSpeed MockSonicSetSpeed
#define WavegenFill2 CoreFill
#define RescaleEventSamples Rescale
#define WavegenEffect PreviousEffect
#define SpeedUp PreviousSpeedUp
#define WavegenFill PreviousFill
#define WcmdqStop PreviousStop
#include "wavegen_host_reference.inc"
#undef USE_KLATT
#undef USE_MBROLA
#undef USE_LIBSONIC
#define USE_KLATT 0
#define USE_MBROLA 0
#define USE_LIBSONIC 0
#undef WavegenEffect
#undef SpeedUp
#undef WavegenFill
#undef WcmdqStop
#define WavegenEffect DisabledEffect
#define SpeedUp DisabledSpeedUp
#define WavegenFill DisabledFill
#define WcmdqStop DisabledStop
#include "wavegen_host_reference.inc"
#undef wcmdq
#undef out_ptr
#undef out_end
#undef sonicSpeed
#undef sonicSpeedupStream
#undef free
#undef espeak_rs_wave_memory
#undef WavegenEffect
#undef SpeedUp
#undef WavegenFill
#undef WcmdqStop
static void Init(unsigned options)
{
 config=options;native_guard=0;memset(&memory,0,sizeof(memory));memory.head=71;memory.tail=92;
 memset(bytes,0x91,sizeof(bytes));memset(&acoustic,0,sizeof(acoustic));memset(&data,0,sizeof(data));
 memset(streams,0,sizeof(streams));for(int i=0;i<3;i++)streams[i].speed=0.8f;
 sonic.speed=options%3==0?0.8:2.5;sonic.stream=options%2?&streams[0]:NULL;
 memset(&output,0,sizeof(output));output.start=bytes;output.ptr=bytes+16;output.end=bytes+128;
 strcpy(backend_name,options%2?"en1":"");TEST_ASSERT(espeak_ng_SetOutputHooks(&hook_sets[0])==0);counts[lane]=0;
 for(int i=0;i<170;i++){memory.queue[i][0]=((intptr_t)9<<8);memory.queue[i][1]=options%3?800:2300;memory.queue[i][2]=0x123456789abcdef;memory.queue[i][3]=0x71717171;}
}
static void Pair(unsigned operation,unsigned options,unsigned features)
{
 State state[2];int results[2];
 for(lane=0;lane<2;lane++) {
  Init(options);RustWavegenHost d=Descriptor(features);int index=options%4==0?-1:options%4==1?0:options%4==2?169:170;
  RustWavegenEffect e={.op=(int)operation,.index=index,.a=(int)(options%4),.b=0x12345678,.c=17,.value=(uintptr_t)&acoustic,.value2=37,.data=&data,.voice=&acoustic};
  if(operation==3)for(int i=0;i<170;i++){memory.queue[i][1]=(intptr_t)alignment_text;memory.queue[i][2]=11;}
  if(operation<11){if(lane){int out=0x55;TEST_ASSERT(espeak_rs_wavegen_host_effect(&d,&e,&out)==0);results[lane]=out;}else results[lane]=features?PreviousEffect(NULL,&e):DisabledEffect(NULL,&e);}
  else if(operation==11){if(lane){int out=0x55;TEST_ASSERT(espeak_rs_wavegen_host_fill(&d,&out)==0);results[lane]=out;}else results[lane]=features?PreviousFill():DisabledFill();}
  else {if(lane){TEST_ASSERT(espeak_rs_wavegen_host_stop(&d)==0);}else if(features)PreviousStop();else DisabledStop();results[lane]=0;}
  Capture(&state[lane]);
 }
 if(results[0]!=results[1]||counts[0]!=counts[1]||memcmp(trace[0],trace[1],counts[0]*sizeof(Trace))||memcmp(&state[0],&state[1],sizeof(State))) {
  fprintf(stderr,"wavegen host mismatch op=%u config=%u features=%u result=%d/%d trace=%u/%u\n",operation,options,features,results[0],results[1],counts[0],counts[1]);abort();
 }
 pairs++;
}
static void Guards(void)
{
 lane=1;Init(0);RustWavegenHost d=Descriptor(7),bad=d;RustWavegenEffect e={.op=6,.a=1,.b=7};int out=0x55;
 TEST_ASSERT(espeak_rs_wavegen_host_effect(NULL,&e,&out)==EINVAL&&out==0x55);TEST_ASSERT(espeak_rs_wavegen_host_effect(&d,NULL,&out)==EINVAL);guards++;
 bad.features=16;TEST_ASSERT(espeak_rs_wavegen_host_effect(&bad,&e,&out)==EINVAL&&counts[1]==0);guards++;
 bad=d;bad.read=NULL;TEST_ASSERT(espeak_rs_wavegen_host_fill(&bad,&out)==EINVAL&&out==0x55&&counts[1]==0);guards++;
 bad=d;bad.memory=NULL;TEST_ASSERT(espeak_rs_wavegen_host_stop(&bad)==EINVAL&&counts[1]==0);guards++;
 live_descriptor=&d;native_guard=1;TEST_ASSERT(espeak_rs_wavegen_host_effect(&d,&e,&out)==0&&out==8);guards++;
 Init(2);d=Descriptor(7);sonic.stream=&streams[0];sonic.speed=0.5;output_hooks=NULL;e.op=1;out=0x55;TEST_ASSERT(espeak_rs_wavegen_host_effect(&d,&e,&out)==EINVAL&&out==0x55&&counts[1]==0);guards++;
 Init(129);d=Descriptor(7);e.op=5;e.index=0;memory.queue[0][1]=800;out=0x55;TEST_ASSERT(espeak_rs_wavegen_host_effect(&d,&e,&out)==EINVAL&&out==0x55);guards++;
 Init(258);d=Descriptor(7);sonic.stream=NULL;sonic.speed=2;out=0x55;TEST_ASSERT(espeak_rs_wavegen_host_fill(&d,&out)==EINVAL&&out==0x55);guards++;
 Init(514);d=Descriptor(7);sonic.speed=2;out=0x55;TEST_ASSERT(espeak_rs_wavegen_host_fill(&d,&out)==EINVAL&&out==0x55&&counts[1]==1);guards++;
 Init(2);d=Descriptor(7);hook_sets[0].outputVoiced=NULL;e.op=1;e.a=1;out=0x55;TEST_ASSERT(espeak_rs_wavegen_host_effect(&d,&e,&out)==EINVAL&&out==0x55&&counts[1]==0);hook_sets[0].outputVoiced=HookVoiced;guards++;
 RustWavegen *generator=RustWavegenState();TEST_ASSERT(generator);for(int i=0;i<1000;i++)TEST_ASSERT(espeak_rs_native_wavegen_state()==generator);guards++;
 TEST_ASSERT(espeak_ng_SetOutputHooks(NULL)==0&&output_hooks==NULL);guards++;
}
int main(void)
{
 _Static_assert(sizeof(RustWavegenHost)==184,"native descriptor ABI");
 for(unsigned trial=0;trial<4096;trial++){Pair(trial%12+1,(trial*37)%128,7);if(trial<1024)Pair(trial%12+1,(trial*37)%128,0);}
 Guards();printf("%u complete previous-native-C/Rust effect/Sonic/fill/stop pairs match exact callback arguments/order, PCM/cursors, acoustic/queue/stream state and fresh hook/resource changes; %u guard groups\n",pairs,guards);return 0;
}
