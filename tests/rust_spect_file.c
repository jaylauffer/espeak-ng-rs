/* Complete frozen C spectral reader vs retained Rust ownership.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_spect_file.h"
#include "wavegen.h"
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static int ReferenceHarmonics(wavegen_peaks_t *peaks,int pitch,int *table,int control)
{
    wavegen_peaks_t initialized[9];memset(initialized,0,sizeof(initialized));
    for(int i=0;i<9;i++){initialized[i].freq=peaks[i].freq;initialized[i].height=peaks[i].height;initialized[i].left=peaks[i].left;initialized[i].right=peaks[i].right;}
    return PeaksToHarmspect(initialized,pitch,table,control);
}
#define PeaksToHarmspect ReferenceHarmonics
#define polint ReferencePolint
#define GetFrameRms ReferenceRms
#define SpectSeqCreate ReferenceCreate
#define SpectSeqDestroy ReferenceDestroy
#define LoadSpectSeq ReferenceLoad
#include "spect_file_reference.inc"
#undef polint
#undef GetFrameRms
#undef SpectSeqCreate
#undef SpectSeqDestroy
#undef LoadSpectSeq
#undef PeaksToHarmspect
/* SpectFrame holds a double: 248 bytes wherever doubles are 8-byte aligned in
 * structs (64-bit, and 32-bit ARM EABI), 240 where they are 4-byte (i386). */
_Static_assert(sizeof(SpectFrame)==(offsetof(SpectFrame,rms)==16?248:240),"spectral frame ABI");
_Static_assert(sizeof(SpectSeq)==(sizeof(void *)==8?200:184),"spectral owner ABI");
static unsigned cases,rms_cases,corpus_cases;
static uint32_t seed=0x7ba98113;
static uint32_t Next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static void Float(float a,float b){TEST_ASSERT(memcmp(&a,&b,sizeof(a))==0);}
static void CompareFrame(SpectFrame *a,SpectFrame *b,int amplitude,int complete)
{
    TEST_ASSERT(a->keyframe==b->keyframe);TEST_ASSERT(a->amp_adjust==b->amp_adjust);
    Float(a->length_adjust,b->length_adjust);Float(a->time,b->time);Float(a->pitch,b->pitch);
    Float(a->length,b->length);Float(a->dx,b->dx);
    TEST_ASSERT(a->nx==b->nx && a->markers==b->markers && a->max_y==b->max_y);
    TEST_ASSERT(memcmp(a->klatt_param,b->klatt_param,sizeof(a->klatt_param))==0);
    TEST_ASSERT(memcmp(a->formants,b->formants,sizeof(a->formants))==0);
    TEST_ASSERT(memcmp(a->peaks,b->peaks,sizeof(a->peaks))==0);
    TEST_ASSERT(memcmp(a->spect,b->spect,sizeof(unsigned short)*a->nx)==0);
    if(complete) {
        int admitted=amplitude>=0 && a->amp_adjust>=0;
        for(int i=0;i<9;i++) {
            int64_t h=(int64_t)a->peaks[i].pkheight*amplitude*a->amp_adjust;
            if(h<0 || h>INT_MAX || h/10000>INT_MAX/256 || a->peaks[i].pkfreq<0 || a->peaks[i].pkwidth<0 || a->peaks[i].pkright<0)admitted=0;
        }
        if(admitted) {double expected=ReferenceRms(a,amplitude),actual=GetFrameRms(b,amplitude);TEST_ASSERT(memcmp(&expected,&actual,sizeof(double))==0);TEST_ASSERT(memcmp(&b->rms,&actual,sizeof(double))==0);rms_cases++;}
    }
}
static void Pair(const char *path)
{
    unsigned char header[12];uint32_t name_length=0;FILE *input=fopen(path,"rb");TEST_ASSERT(input);
    if(fread(header,1,sizeof(header),input)==sizeof(header))for(int i=0;i<4;i++)name_length|=(uint32_t)header[8+i]<<(8*i);
    fclose(input);
    SpectSeq *a=ReferenceCreate(),*b=SpectSeqCreate();TEST_ASSERT(a&&b);
    /* These fields are uninitialized by the original constructor. They are
     * unused by the compiler, but LoadSpectSeq copies them into scalar fields. */
    memset(&a->pitchenv,0,sizeof(a->pitchenv));a->pitchenv.pitch1=b->pitchenv.pitch1=123;a->pitchenv.pitch2=b->pitchenv.pitch2=456;
    int c=ReferenceLoad(a,path),r=LoadSpectSeq(b,path);
    if(c!=r)fprintf(stderr,"status mismatch %s C=%x Rust=%x\n",path,c,r);
    TEST_ASSERT(c==r);TEST_ASSERT(a->numframes==b->numframes);
    for(int i=0;i<a->numframes;i++)CompareFrame(a->frames[i],b->frames[i],a->amplitude,c==0);
    if(c==0) {
        TEST_ASSERT(a->amplitude==b->amplitude && a->max_x==b->max_x && a->max_y==b->max_y && a->file_format==b->file_format);
        TEST_ASSERT(a->duration==b->duration && a->pitch1==b->pitch1 && a->pitch2==b->pitch2);
        TEST_ASSERT(a->grid==b->grid && a->bass_reduction==b->bass_reduction);
        if(a->name){TEST_ASSERT(memcmp(a->name,b->name,name_length)==0);}else{TEST_ASSERT(!b->name);}
    }
    ReferenceDestroy(a);SpectSeqDestroy(b);cases++;
}
static void Word(unsigned char *b,size_t *n,uint32_t v){for(int i=0;i<4;i++)b[(*n)++]=(unsigned char)(v>>(i*8));}
static void Short(unsigned char *b,size_t *n,int v){b[(*n)++]=(unsigned char)v;b[(*n)++]=(unsigned char)(v>>8);}
static void Extended(unsigned char *b,size_t *n,double v)
{
    uint64_t bits;memcpy(&bits,&v,8);unsigned char out[10]={0};
    if(v!=0) {uint16_t e=(uint16_t)(((bits>>52)&2047)-1023+16383);e|=(uint16_t)((bits>>48)&0x8000);out[0]=e>>8;out[1]=e;
        uint64_t m=((bits&UINT64_C(0xfffffffffffff))|UINT64_C(0x10000000000000))<<11;for(int i=0;i<8;i++)out[2+i]=(unsigned char)(m>>(56-i*8));}
    memcpy(b+*n,out,10);*n+=10;
}
static size_t Data(unsigned char *b,int format,unsigned trial)
{
    size_t n=0;Word(b,&n,FILEID1_SPECTSEQ);Word(b,&n,(uint32_t[]){FILEID2_SPECTSEQ,FILEID2_SPECTSEK,FILEID2_SPECTSQ2}[format]);
    Word(b,&n,6);memcpy(b+n,"owned",6);if(trial%2)b[n+5]='!';n+=6;int count=1+(int)(trial%8);Short(b,&n,count);Short(b,&n,40+Next()%80);Short(b,&n,trial%2?0:100);Short(b,&n,0);
    for(int f=0;f<count;f++) {
        Extended(b,&n,10.0+f*0.125);Extended(b,&n,70+Next()%100);Extended(b,&n,20+Next()%80);Extended(b,&n,8.0+Next()%10);
        Short(b,&n,7);Short(b,&n,Next()%64);Short(b,&n,50+Next()%80);if(format==2){Short(b,&n,17);Short(b,&n,29);}
        for(int p=0;p<9;p++){Short(b,&n,Next()%9000);Short(b,&n,Next()%400);Short(b,&n,100+p*800);Short(b,&n,(trial+f)%3?Next()%30:0);Short(b,&n,300+Next()%600);Short(b,&n,300+Next()%600);if(format==2){Short(b,&n,50+Next()%200);Short(b,&n,Next()%20);Short(b,&n,50+Next()%200);}}
        if(format>0)for(int p=0;p<14;p++)Short(b,&n,Next()%80);
        for(int p=0;p<7;p++)Short(b,&n,p==6?65535:Next()%32768);
    }
    return n;
}
static void Write(const char *path,const unsigned char *b,size_t n){FILE *f=fopen(path,"wb");TEST_ASSERT(f);TEST_ASSERT(fwrite(b,1,n,f)==n);fclose(f);}
static void GuardLog(const unsigned char *b,size_t n){(void)b;(void)n;TEST_ASSERT(SpectSeqCreate()==NULL);}
static void Guards(const char *path)
{
    SpectSeq *slots[4];for(int i=0;i<4;i++){slots[i]=SpectSeqCreate();TEST_ASSERT(slots[i]);}
    TEST_ASSERT(SpectSeqCreate()==NULL);SpectSeqDestroy(NULL);SpectSeqDestroy((SpectSeq *)(uintptr_t)1);
    TEST_ASSERT(espeak_rs_spect_load(NULL,path,GuardLog)==ENS_UNSUPPORTED_PHON_FORMAT);
    TEST_ASSERT(espeak_rs_spect_load(slots[0],NULL,GuardLog)==ENS_UNSUPPORTED_PHON_FORMAT);
    TEST_ASSERT(espeak_rs_spect_load(slots[0],path,NULL)==ENS_UNSUPPORTED_PHON_FORMAT);
    TEST_ASSERT(espeak_rs_spect_load((SpectSeq *)(uintptr_t)1,path,GuardLog)==ENS_UNSUPPORTED_PHON_FORMAT);
    char bytes[4096];memset(bytes,'a',sizeof(bytes));TEST_ASSERT(espeak_rs_spect_load(slots[0],bytes,GuardLog)==ENS_UNSUPPORTED_PHON_FORMAT);
    for(int i=1;i<4;i++)SpectSeqDestroy(slots[i]);
    TEST_ASSERT(espeak_rs_spect_load(slots[0],"missing-spect-file.tmp",GuardLog)==ENOENT);
    for(int i=0;i<4;i++)SpectSeqDestroy(slots[i]);SpectSeqDestroy(slots[0]);
    TEST_ASSERT(espeak_rs_spect_rms((SpectFrame *)(uintptr_t)1,100,RustWavegenState(),22050)==0);
    TEST_ASSERT(espeak_rs_spect_polint(NULL,NULL,1,0)==0);
}
int main(void)
{
    TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050);TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
    char path[128];snprintf(path,sizeof(path),"spect-file-%ld.tmp",(long)getpid());unsigned char bytes[12000];
    for(unsigned i=0;i<300;i++){size_t n=Data(bytes,i%3,i);Write(path,bytes,n);Pair(path);}
    for(int format=0;format<3;format++){size_t n=Data(bytes,format,0);for(size_t end=0;end<n;end++){Write(path,bytes,end);Pair(path);}}
    size_t n=Data(bytes,0,0);bytes[18]=bytes[19]=0;Write(path,bytes,n);Pair(path);
    bytes[0]=0;Write(path,bytes,n);Pair(path);
    FILE *paths=fopen(TEST_SPECT_PATHS,"r");TEST_ASSERT(paths);char input[4096];
    while(fgets(input,sizeof(input),paths)){size_t length=strlen(input);TEST_ASSERT(length>0&&input[length-1]=='\n');input[length-1]=0;
        FILE *f=fopen(input,"rb");TEST_ASSERT(f);unsigned char magic[4];size_t read=fread(magic,1,4,f);fclose(f);if(read==4&&!memcmp(magic,"SPEC",4)){Pair(input);corpus_cases++;}}
    fclose(paths);
    for(unsigned i=0;i<100000;i++){int count=1+Next()%8;float x[8],y[8],at=(int)(Next()%400)-100;for(int p=0;p<count;p++){x[p]=p*100+(Next()%17);y[p]=(int)(Next()%10000)-5000;}if(i%3==0 && count>1)x[1]=x[0];Float(ReferencePolint(x,y,count,at),polint(x,y,count,at));}
    Guards(path);TEST_ASSERT(unlink(path)==0);TEST_ASSERT(espeak_Terminate()==EE_OK);
    printf("%u frozen C/Rust spectral parse/status/field comparisons (%u corpus files), %u RMS comparisons and 100000 interpolation cases; lease/path/reentry guards passed\n",cases,corpus_cases,rms_cases);return 0;
}
