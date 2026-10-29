/* Complete unchanged native C speechPlayer controller vs Rust, GPL-3.0-or-later. */
#include "test_assert.h"
#include "rust_data.h"
#include "rust_splayer.h"
#include "sPlayer.h"
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifndef _WIN32
#include <sys/mman.h>
#include <unistd.h>
#endif
void reference_KlattInitSP(void);
void reference_KlattFiniSP(void);
int reference_Wavegen_KlattSP(WGEN_DATA *,voice_t *,int,int,frame_t *,frame_t *);
static uint32_t seed;
static unsigned Next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static unsigned char envelope[128],mix[8192];
typedef struct {WGEN_DATA data;unsigned char pcm[516];unsigned produced;int full,random;} Result;
static Result results[2][30];
static uint64_t drain_hash[2];static unsigned drain_samples[2];
static unsigned pairs,calls,samples;
static int Fill(int lane,WGEN_DATA *data,voice_t *voice,int length,int resume,frame_t *a,frame_t *b)
{
 if(!lane)return reference_Wavegen_KlattSP(data,voice,length,resume,a,b);
 RustSplayerShared shared={&espeak_rs_wave_memory,&espeak_rs_output,envelope,sizeof(envelope),mix,sizeof(mix)};
 int full=-1;TEST_ASSERT(espeak_rs_splayer_fill(&shared,data,voice,length,resume,a,b,&full)==0);return full;
}
static void Case(unsigned trial)
{
 for(int lane=0;lane<2;lane++){
  seed=0x10403020u+trial*13;srand(23+trial);
  for(int i=0;i<128;i++)envelope[i]=(unsigned char)Next();for(int i=0;i<8192;i++)mix[i]=(unsigned char)Next();
  WGEN_DATA data={0};data.pitch_env=envelope;data.mix_wavefile=mix;
  data.pitch_base=(90+Next()%60)*4096;data.pitch_range=(Next()%25)*4096;data.pitch=data.pitch_base;
  data.pitch_inc=Next()%71;data.amplitude=30+Next()%150;data.amplitude_v=512+Next()%1300;
  data.mix_wave_scale=trial%3?1+Next()%80:0;data.mix_wave_amp=20+Next()%61;
  data.n_mix_wavefile=trial%4?100+Next()%2000:0;data.mix_wavefile_max=96+Next()%160;
  if(!data.mix_wave_scale)data.mix_wavefile_max=(data.mix_wavefile_max/2)*2;
  voice_t voice={0};voice.voicing=30+Next()%35;voice.breath[1]=Next()%15;
  for(int i=1;i<7;i++){voice.freq[i]=180+Next()%160;voice.freqadd[i]=(int)(Next()%81)-40;voice.width[i]=200+Next()%180;}
  if(lane){TEST_ASSERT(espeak_rs_splayer_initialize()==0);}else reference_KlattInitSP();
  int resume=0;
  for(int step=0;step<30;step++){
   frame_t a={0},b={0};a.frflags=b.frflags=trial%2;
   for(int i=1;i<7;i++){a.ffreq[i]=b.ffreq[i]=300+i*650+Next()%101;}
   for(int i=0;i<4;i++){a.bw[i]=30+Next()%60;b.bw[i]=30+Next()%60;}
   a.klattp[KLATT_FNZ]=Next()%3?0:80;b.klattp[KLATT_FNZ]=Next()%3?0:90;
   memset(espeak_rs_wave_memory.queue,0,sizeof(espeak_rs_wave_memory.queue));
   espeak_rs_wave_memory.head=Next()%170;espeak_rs_wave_memory.tail=(espeak_rs_wave_memory.head+1+Next()%8)%170;
   for(int i=(espeak_rs_wave_memory.head+1)%170;i!=espeak_rs_wave_memory.tail;i=(i+1)%170)espeak_rs_wave_memory.queue[i][0]=(int[]){1,2,5,6,10,11}[Next()%6];
   unsigned capacity=Next()%258;int length=Next()%750;
   Result *r=&results[lane][step];memset(r,0,sizeof(*r));memset(r->pcm,0x5d,sizeof(r->pcm));
   espeak_rs_output.start=espeak_rs_output.ptr=r->pcm;espeak_rs_output.end=r->pcm+capacity*2+(trial%7==0);
   r->full=Fill(lane,&data,&voice,length,resume,&a,&b);r->data=data;r->produced=(unsigned)(espeak_rs_output.ptr-r->pcm);r->random=rand();
   TEST_ASSERT(r->produced<=capacity*2);resume=r->full;
  }
  drain_hash[lane]=1469598103934665603ULL;drain_samples[lane]=0;
  unsigned char tail[514]={0};int done=0;
  for(int i=0;i<4096;i++){
   espeak_rs_output.start=espeak_rs_output.ptr=tail;espeak_rs_output.end=tail+sizeof(tail);
   int full=Fill(lane,&data,NULL,0,1,NULL,NULL);size_t size=(size_t)(espeak_rs_output.ptr-tail);
   for(size_t j=0;j<size;j++)drain_hash[lane]=(drain_hash[lane]^tail[j])*1099511628211ULL;
   drain_samples[lane]+=(unsigned)(size/2);if(!full){done=1;break;}
  }
  TEST_ASSERT(done);
  if(lane){TEST_ASSERT(espeak_rs_splayer_finish()==0);}else reference_KlattFiniSP();
 }
 for(int i=0;i<30;i++){
  if(memcmp(&results[0][i],&results[1][i],sizeof(Result))){fprintf(stderr,"sPlayer mismatch trial=%u step=%d bytes=%u/%u full=%d/%d\n",trial,i,results[0][i].produced,results[1][i].produced,results[0][i].full,results[1][i].full);abort();}
  calls++;samples+=results[0][i].produced/2;
 }
 TEST_ASSERT(drain_hash[0]==drain_hash[1]&&drain_samples[0]==drain_samples[1]);pairs++;
}
static void Guards(void)
{
 WGEN_DATA data={0};data.pitch_env=envelope;data.pitch_base=90*4096;data.pitch=data.pitch_base;data.pitch_inc=64;
 voice_t voice={0};frame_t frame={0};unsigned char pcm[8]={0};
 memset(&espeak_rs_wave_memory,0,sizeof(espeak_rs_wave_memory));espeak_rs_wave_memory.tail=1;
 espeak_rs_output.start=espeak_rs_output.ptr=pcm;espeak_rs_output.end=pcm+sizeof(pcm);
 RustSplayerShared shared={&espeak_rs_wave_memory,&espeak_rs_output,envelope,sizeof(envelope),mix,sizeof(mix)};
 int full=77;TEST_ASSERT(espeak_rs_splayer_initialize()==0);
 shared.pitch_size=127;TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==EINVAL&&full==77&&data.pitch_ix==0);
 shared.pitch_size=128;espeak_rs_wave_memory.head=-1;TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==EINVAL&&full==77);
 espeak_rs_wave_memory.head=0;shared.mix=pcm;shared.mix_size=sizeof(pcm);TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==EINVAL&&full==77);
 shared.mix=mix;shared.mix_size=sizeof(mix);
 espeak_rs_output.end=pcm;espeak_rs_output.ptr=pcm+2;TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==EINVAL&&full==77);
 espeak_rs_output.ptr=pcm;data.pitch_ix=-256;data.pitch_inc=0;TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==EINVAL&&full==77);
 data.pitch_ix=0;
#ifndef _WIN32
 long page=sysconf(_SC_PAGESIZE);TEST_ASSERT(page>44);
 unsigned char *mapping=mmap(NULL,(size_t)page*2,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANON,-1,0);TEST_ASSERT(mapping!=MAP_FAILED);
 TEST_ASSERT(mprotect(mapping+page,(size_t)page,PROT_NONE)==0);
 unsigned char *ordinary=mapping+page-44;memset(ordinary,0,44);
 for(int flag=0;flag<2;flag++){
  ordinary[0]=(unsigned char)flag;
  TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,(frame_t *)ordinary,(frame_t *)ordinary,&full)==0&&full==1);
  TEST_ASSERT(espeak_rs_splayer_initialize()==0);
 }
 TEST_ASSERT(munmap(mapping,(size_t)page*2)==0);
 TEST_ASSERT(espeak_rs_splayer_initialize()==0);
#endif
 // No output produces no samples; each admitted command adds four frames.
 for(int i=0;i<64;i++)TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==0&&full==1);
 full=77;WGEN_DATA before=data;TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==EAGAIN&&full==77&&!memcmp(&before,&data,sizeof(data)));
 TEST_ASSERT(espeak_rs_splayer_initialize()==0);
 data.pitch_inc=64;data.mix_wavefile=mix+sizeof(mix);data.n_mix_wavefile=1;data.mix_wave_scale=1;
 espeak_rs_output.end=pcm+2;full=77;
 TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==EINVAL&&full==77&&data.pitch_ix==256&&espeak_rs_output.ptr==pcm);
 TEST_ASSERT(espeak_rs_splayer_initialize()==0);data.pitch_ix=0;data.mix_wavefile=mix+sizeof(mix)-1;data.n_mix_wavefile=2;data.mix_wavefile_ix=0;
 espeak_rs_output.end=pcm+4;full=77;
 TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==EINVAL&&full==77&&data.mix_wavefile_ix==1&&espeak_rs_output.ptr==pcm+2);
 TEST_ASSERT(espeak_rs_splayer_finish()==0);
 data.n_mix_wavefile=0;espeak_rs_output.ptr=pcm;full=77;TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,&voice,256,0,&frame,&frame,&full)==EINVAL&&full==77);
 // Resume consumes no frame/voice/pitch input, including null pointers.
 TEST_ASSERT(espeak_rs_splayer_initialize()==0);shared.pitch=NULL;shared.pitch_size=0;full=77;
 TEST_ASSERT(espeak_rs_splayer_fill(&shared,&data,NULL,0,1,NULL,NULL,&full)==0&&full==0&&espeak_rs_output.ptr==pcm);
 TEST_ASSERT(espeak_rs_splayer_finish()==0);
 espeak_rs_output.start=espeak_rs_output.ptr=espeak_rs_output.end=NULL;
}
int main(void)
{
 _Static_assert(sizeof(RustSplayerShared)==(sizeof(void *)==8?48:24),"native descriptor ABI");
 for(unsigned i=0;i<1024;i++)Case(i);
 Guards();
 printf("%u complete unchanged-C/Rust speechPlayer engine programs/%u calls/%u samples match PCM/tails, pitch/mix state, look-ahead, output cursors, rand order and drained PCM; native admission, protected ordinary-frame, capacity, source-expiry, inactive and null-resume guards pass\n",pairs,calls,samples);
 return 0;
}
