/* Complete original speechPlayer C++ backend vs native Rust public API.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "test_assert.h"
#include <speechPlayer.h>
#include <errno.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern speechPlayer_handle_t reference_speechPlayer_initialize(int);
extern void reference_speechPlayer_queueFrame(speechPlayer_handle_t,speechPlayer_frame_t*,unsigned,unsigned,int,bool);
extern int reference_speechPlayer_synthesize(speechPlayer_handle_t,unsigned,sample*);
extern int reference_speechPlayer_getLastIndex(speechPlayer_handle_t);
extern void reference_speechPlayer_terminate(speechPlayer_handle_t);
static uint32_t seed;
static uint32_t Next(void) {seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static void Frame(speechPlayer_frame_t *f)
{
 memset(f,0,sizeof(*f));
 f->voicePitch=90+Next()%160;f->endVoicePitch=f->voicePitch+(int)(Next()%21)-10;
 f->vibratoPitchOffset=(Next()%11)/10.0;f->vibratoSpeed=Next()%7;
 f->voiceTurbulenceAmplitude=(Next()%10)/20.0;f->glottalOpenQuotient=(Next()%9)/10.0;
 f->voiceAmplitude=(Next()%7+1)/10.0;f->aspirationAmplitude=(Next()%3)/10.0;
 f->cf1=450;f->cf2=1100;f->cf3=2300;f->cf4=3300;f->cf5=4000;f->cf6=4700;f->cfN0=250;f->cfNP=200;
 f->cb1=100;f->cb2=150;f->cb3=200;f->cb4=400;f->cb5=1000;f->cb6=1000;f->cbN0=100;f->cbNP=100;
 f->caNP=(Next()%3)/10.0;f->fricationAmplitude=(Next()%4)/10.0;
 f->pf1=600;f->pf2=1500;f->pf3=2500;f->pf4=3500;f->pf5=4200;f->pf6=4900;
 f->pb1=160;f->pb2=170;f->pb3=200;f->pb4=300;f->pb5=400;f->pb6=500;
 f->pa1=0.2;f->pa2=0.1;f->pa3=0.1;f->pa4=0.1;f->pa5=0.1;f->pa6=0.1;
 f->parallelBypass=(Next()%5)/10.0;f->preFormantGain=(Next()%8+1)/10.0;f->outputGain=(Next()%8+1)/10.0;
 double *frequencies[]={&f->cf1,&f->cf2,&f->cf3,&f->cf4,&f->cf5,&f->cf6,&f->cfN0,&f->cfNP,&f->pf1,&f->pf2,&f->pf3,&f->pf4,&f->pf5,&f->pf6};
 double *bandwidths[]={&f->cb1,&f->cb2,&f->cb3,&f->cb4,&f->cb5,&f->cb6,&f->cbN0,&f->cbNP,&f->pb1,&f->pb2,&f->pb3,&f->pb4,&f->pb5,&f->pb6};
 for(unsigned i=0;i<14;i++){*frequencies[i]+=(int)(Next()%121)-60;*bandwidths[i]+=Next()%51;}
 // This NaN affects comparisons/parameter fades, never an int conversion.
 if(Next()%11==0)f->glottalOpenQuotient=NAN;
}
typedef struct {int count,index,random;sample samples[257];} Result;
static Result results[2][64];
static unsigned pairs,calls,samples;
static void Case(unsigned trial)
{
 static const int rates[]={8000,11025,22050,44100,48000};
 for(int lane=0;lane<2;lane++) {
  seed=0x10203040u+trial*37;srand(17+trial);
  speechPlayer_handle_t handle=lane?speechPlayer_initialize(rates[trial%5]):reference_speechPlayer_initialize(rates[trial%5]);TEST_ASSERT(handle);
  for(int step=0;step<40;step++) {
   unsigned option=Next();speechPlayer_frame_t f;Frame(&f);
   unsigned minimum=1+Next()%180,fade=Next()%100;int index=option%3?-1:step;
   speechPlayer_frame_t *source=option%7?&f:NULL;bool purge=option%9==0;
   if(lane)speechPlayer_queueFrame(handle,source,minimum,fade,index,purge);else reference_speechPlayer_queueFrame(handle,source,minimum,fade,index,purge);
   f.voicePitch+=1000;f.outputGain=0; // queued frames must already be owned copies
   Result *r=&results[lane][step];memset(r,0,sizeof(*r));for(int i=0;i<257;i++)r->samples[i].value=0x6b3d;
   unsigned capacity=Next()%258;
   r->count=lane?speechPlayer_synthesize(handle,capacity,r->samples):reference_speechPlayer_synthesize(handle,capacity,r->samples);
   r->index=lane?speechPlayer_getLastIndex(handle):reference_speechPlayer_getLastIndex(handle);r->random=rand();
   TEST_ASSERT(r->count>=0&&(unsigned)r->count<=capacity);
  }
  // Drain before legacy destruction: its destructor does not free pending
  // request allocations. Rust pending-teardown is checked independently.
  sample tail[257]={0};int n;
  do {n=lane?speechPlayer_synthesize(handle,257,tail):reference_speechPlayer_synthesize(handle,257,tail);}while(n==257);
  if(lane)speechPlayer_terminate(handle);else reference_speechPlayer_terminate(handle);
 }
 for(int i=0;i<40;i++) {
  Result *a=&results[0][i],*b=&results[1][i];
  if(memcmp(a,b,sizeof(*a))) {fprintf(stderr,"speechPlayer mismatch trial=%u step=%d count=%d/%d index=%d/%d rand=%d/%d\n",trial,i,a->count,b->count,a->index,b->index,a->random,b->random);for(int j=0;j<257;j++)if(a->samples[j].value!=b->samples[j].value){fprintf(stderr,"sample=%d old=%d rust=%d\n",j,a->samples[j].value,b->samples[j].value);break;}abort();}
  calls++;samples+=(unsigned)a->count;
 }
 pairs++;
}
static void Guards(void)
{
 speechPlayer_handle_t h=speechPlayer_initialize(22050);TEST_ASSERT(h);speechPlayer_frame_t frame;Frame(&frame);
 TEST_ASSERT(espeak_rs_speech_player_queue_frame(h,&frame,0,1,1,false)==EINVAL);
 for(int i=0;i<256;i++)TEST_ASSERT(espeak_rs_speech_player_queue_frame(h,&frame,1,1,i,false)==0);
 TEST_ASSERT(espeak_rs_speech_player_queue_frame(h,&frame,1,1,99,false)==EAGAIN);
 unsigned result=0x5555;TEST_ASSERT(espeak_rs_speech_player_synthesize(h,1,NULL,&result)==EINVAL&&result==0x5555);
 TEST_ASSERT(espeak_rs_speech_player_queue_frame(h,NULL,0,0,901,true)==0);
 sample data[8]={0};TEST_ASSERT(espeak_rs_speech_player_synthesize(h,8,data,&result)==0&&result<=8);
 TEST_ASSERT(speechPlayer_getLastIndex(h)==901);
 TEST_ASSERT(espeak_rs_speech_player_terminate(h)==0);
 TEST_ASSERT(espeak_rs_speech_player_terminate(h)==EINVAL);
 TEST_ASSERT(espeak_rs_speech_player_queue_frame(h,&frame,1,1,3,false)==EINVAL);
 speechPlayer_handle_t next=speechPlayer_initialize(22050);TEST_ASSERT(next&&next!=h);speechPlayer_terminate(next);
}
int main(void)
{
 _Static_assert(sizeof(speechPlayer_frame_t)==47*sizeof(double),"speechPlayer frame ABI");
 for(unsigned i=0;i<1024;i++)Case(i);
 Guards();printf("%u complete original-C++/Rust speechPlayer programs match exact PCM, initialized tails, sample counts, last indices and native rand order across %u calls/%u samples; capacity, invalid-input, purge and stale-owner guards pass\n",pairs,calls,samples);return 0;
}
