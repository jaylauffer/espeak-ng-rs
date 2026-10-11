/* Complete frozen C resource controller vs owned Rust sources/cache/encoding.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include <stdio.h>
#include <stdarg.h>
#include <locale.h>
#include <ctype.h>
static FILE *source_log;
static int SourcePrintf(FILE *f,const char *format,...)
{
    va_list args;va_start(args,format);int result=vfprintf(f==stderr&&source_log?source_log:f,format,args);va_end(args);return result;
}
#define main ReaderFixtureEntry
#define Pair ReaderPair
#define Guards ReaderGuards
#define TEST_SPECT_PATHS TEST_RESOURCE_PATHS
#define fprintf SourcePrintf
#include "rust_spect_file.c"
#undef main
#undef Pair
#undef Guards
#undef fprintf
#include "rust_compiler_resource.h"
#include "speech.h"
#include "common.h"
typedef struct {void *link;int value,ph_mnemonic;short ph_table;char string[1];} REF_HASH_TAB;
typedef struct {
    char phsrc[N_PATH_BUF];FILE *f_phdata,*f_phcontents,*f_errors,*log;
    int count_references,duplicate_references,count_frames,error_count,n_phoneme_tabs;
    PHONEME_TAB *phoneme_out;REF_HASH_TAB *ref_hash_tab[256];
    uint64_t owner;RustCompilerResource *descriptor;RustResourceProgress *output;
    int mutate,fail_data,fail_manifest,positions,writes,fail_position;char name[4096];
} CompileContext;
static void error(CompileContext *ctx,const char *format,...)
{
    va_list args;va_start(args,format);vfprintf(ctx->f_errors,format,args);fputc('\n',ctx->f_errors);va_end(args);ctx->error_count++;
}
static SpectSeq *ControllerCreate(void)
{
    SpectSeq *s=ReferenceCreate();if(s)memset(&s->pitchenv,0,sizeof(s->pitchenv));return s;
}
#define SpectSeqCreate ControllerCreate
#define SpectSeqDestroy ReferenceDestroy
#define LoadSpectSeq ReferenceLoad
#define GetFrameRms ReferenceRms
#define LoadSpect ReferenceSpectrum
#include "spect_compile_reference.inc"
#undef LoadSpect
#undef GetFrameRms
#undef LoadSpectSeq
#undef SpectSeqDestroy
#undef SpectSeqCreate
static int CalculateSample(unsigned char,int);
#define LoadSpect ReferenceSpectrum
#define LoadWavefile ReferenceWave
#define LoadEnvelope ReferenceEnvelope
#define Hash8 ReferenceHash
#define LoadEnvelope2 ReferenceEnvelopeText
#define LoadDataFile ReferenceResource
#define polint ReferencePolint
#include "compiler_resource_reference.inc"
#undef polint
#undef LoadDataFile
#undef LoadEnvelope2
#undef Hash8
#undef LoadEnvelope
#undef LoadWavefile
#undef LoadSpect
_Static_assert(sizeof(void *)!=8 || sizeof(RustCompilerResource)==136,"resource descriptor ABI");
_Static_assert(sizeof(RustResourceProgress)==16,"resource progress ABI");
static unsigned pairs,corpus[3],requests;
static void Sentinel(CompileContext *ctx)
{
    TEST_ASSERT(ctx->output->address==-17 && ctx->output->references==-19 && ctx->output->duplicates==23 && ctx->output->frames==-29);
}
static void NativeLog(void *opaque,const unsigned char *bytes,size_t length)
{
    CompileContext *ctx=opaque;Sentinel(ctx);TEST_ASSERT(fwrite(bytes,1,length,ctx->log)==length);
}
static void NativeError(void *opaque,int32_t code,int32_t a,int32_t b,const unsigned char *bytes,size_t length)
{
    CompileContext *ctx=opaque;Sentinel(ctx);char name[4096];TEST_ASSERT(length<sizeof(name));memcpy(name,bytes,length);name[length]=0;
    switch(code){
    case 1:error(ctx,"Can't read file: %s",name);break;
    case 2:error(ctx,"File not SPEC or RIFF: %s",name);break;
    case 3:error(ctx,"Bad vowel file: '%s'",name);break;
    case 4:error(ctx,"Can't resample (%d to %d): %s",a,b,name);break;
    case 5:error(ctx,"WAV file is not mono: %s",name);break;
    default:TEST_ASSERT(0);
    }
}
static int64_t NativePosition(void *opaque)
{
    CompileContext *ctx=opaque;Sentinel(ctx);ctx->positions++;
    if(ctx->fail_position)return -1;
    if(ctx->mutate){
        RustResourceProgress out={-17,-19,23,-29};TEST_ASSERT(espeak_rs_compiler_resource_load(ctx->descriptor,&out)==ENS_UNSUPPORTED_PHON_FORMAT);
        TEST_ASSERT(out.address==-17 && out.references==-19 && out.duplicates==23 && out.frames==-29);
        TEST_ASSERT(espeak_rs_compiler_resources_end(ctx->owner)==ENS_UNSUPPORTED_PHON_FORMAT);
        strcpy(ctx->phsrc,"mutated-after-admission");strcpy(ctx->name,"mutated-after-admission");
        ctx->descriptor->generator=NULL;ctx->descriptor->write=NULL;ctx->descriptor->initial.references=INT_MAX;
    }
    return ftell(ctx->f_phdata);
}
static int NativeWrite(void *opaque,int32_t destination,const unsigned char *bytes,size_t length)
{
    CompileContext *ctx=opaque;Sentinel(ctx);ctx->writes++;
    if((destination==0 && ctx->fail_data)||(destination==1 && ctx->fail_manifest))return 5;
    FILE *f=destination==0?ctx->f_phdata:ctx->f_phcontents;return fwrite(bytes,1,length,f)==length?0:5;
}
static size_t Contents(FILE *f,unsigned char *bytes,size_t capacity)
{
    long at=ftell(f);fflush(f);rewind(f);size_t n=fread(bytes,1,capacity,f);TEST_ASSERT(feof(f));TEST_ASSERT(fseek(f,at,SEEK_SET)==0);return n;
}
static void Setup(CompileContext *ctx,PHONEME_TAB *phoneme,const char *source,const char *name)
{
    memset(ctx,0,sizeof(*ctx));TEST_ASSERT(strlen(source)<sizeof(ctx->phsrc)&&strlen(name)<sizeof(ctx->name));strcpy(ctx->phsrc,source);strcpy(ctx->name,name);
    ctx->f_phdata=tmpfile();ctx->f_phcontents=tmpfile();ctx->f_errors=tmpfile();ctx->log=tmpfile();TEST_ASSERT(ctx->f_phdata&&ctx->f_phcontents&&ctx->f_errors&&ctx->log);
    for(int i=0;i<7;i++)fputc(0x5a,ctx->f_phdata);ctx->count_frames=31;ctx->count_references=11;ctx->duplicate_references=5;ctx->n_phoneme_tabs=3;ctx->phoneme_out=phoneme;
}
static void Tidy(CompileContext *ctx)
{
    for(int i=0;i<256;i++){REF_HASH_TAB *p=ctx->ref_hash_tab[i];while(p){REF_HASH_TAB *next=p->link;free(p);p=next;}}
    fclose(ctx->f_phdata);fclose(ctx->f_phcontents);fclose(ctx->f_errors);fclose(ctx->log);
    if(ctx->owner)TEST_ASSERT(espeak_rs_compiler_resources_end(ctx->owner)==0);
}
static int Native(CompileContext *ctx,int control,int *address,int mutate)
{
    int32_t lower[256];unsigned char space[256];for(int i=0;i<256;i++){lower[i]=tolower(i);space[i]=isspace(i)!=0;}
    RustCompilerResource d={ctx->owner,ctx,ctx->phsrc,ctx->name,RustWavegenState(),lower,space,N_PATH_BUF,samplerate,control,ctx->phoneme_out->mnemonic,ctx->n_phoneme_tabs-1,(unsigned char)localeconv()->decimal_point[0],{*address,ctx->count_references,ctx->duplicate_references,ctx->count_frames},NativeLog,NativeError,NativePosition,NativeWrite};
    RustResourceProgress out={-17,-19,23,-29};ctx->descriptor=&d;ctx->output=&out;ctx->mutate=mutate;
    int status=espeak_rs_compiler_resource_load(&d,&out);*address=out.address;ctx->count_references=out.references;ctx->duplicate_references=out.duplicates;ctx->count_frames=out.frames;requests++;return status;
}
static void Pair(const char *source,const char *name,int control,int mutate)
{
    PHONEME_TAB phoneme={0};phoneme.mnemonic=0x6162;CompileContext ctx[2];Setup(&ctx[0],&phoneme,source,name);Setup(&ctx[1],&phoneme,source,name);
    ctx[1].owner=espeak_rs_compiler_resources_begin();TEST_ASSERT(ctx[1].owner);int address[2],status[2];
    for(int phase=0;phase<3;phase++){
        address[0]=address[1]=phase==2?777:0;
        source_log=ctx[0].log;errno=0;status[0]=ReferenceResource(&ctx[0],name,control,&address[0]);source_log=NULL;
        status[1]=Native(&ctx[1],control,&address[1],mutate&&phase==0);
        if(status[0]!=status[1]||address[0]!=address[1])fprintf(stderr,"resource mismatch %s/%s phase%d C=%x/%d Rust=%x/%d\n",source,name,phase,status[0],address[0],status[1],address[1]);
        TEST_ASSERT(status[0]==status[1]&&address[0]==address[1]);
        TEST_ASSERT(ctx[0].count_references==ctx[1].count_references&&ctx[0].duplicate_references==ctx[1].duplicate_references&&ctx[0].count_frames==ctx[1].count_frames&&ctx[0].error_count==ctx[1].error_count);
        static unsigned char a[1000000],b[1000000];FILE *expected[]={ctx[0].f_phdata,ctx[0].f_phcontents,ctx[0].f_errors,ctx[0].log};FILE *actual[]={ctx[1].f_phdata,ctx[1].f_phcontents,ctx[1].f_errors,ctx[1].log};
        for(int i=0;i<4;i++){size_t x=Contents(expected[i],a,sizeof(a)),y=Contents(actual[i],b,sizeof(b));if(x!=y||memcmp(a,b,x)!=0)fprintf(stderr,"resource bytes mismatch %s/%s phase%d stream%d sizes%zu/%zu\n",source,name,phase,i,x,y);TEST_ASSERT(x==y&&memcmp(a,b,x)==0);}
        strcpy(ctx[1].phsrc,source);strcpy(ctx[1].name,name);
    }
    Tidy(&ctx[0]);Tidy(&ctx[1]);pairs++;
}
static size_t Wave(unsigned char *b,unsigned trial)
{
    unsigned count=trial%127;memset(b,0,44);memcpy(b,"RIFF",4);int rate=trial%19==0?21000:samplerate;int byte_rate=trial%23==0?rate*4:rate*2;
    for(int i=0;i<4;i++){b[24+i]=(unsigned)rate>>(i*8);b[28+i]=(unsigned)byte_rate>>(i*8);b[40+i]=(count*2+trial%3)>>(i*8);}
    for(unsigned i=0;i<count;i++){unsigned sample=Next();b[44+2*i]=sample;b[45+2*i]=sample>>8;}size_t n=44+count*2;if(trial%2)b[n++]=137;return n;
}
static void Guards(const char *path)
{
    PHONEME_TAB phoneme={0};CompileContext ctx;Setup(&ctx,&phoneme,".",path);ctx.owner=espeak_rs_compiler_resources_begin();TEST_ASSERT(ctx.owner);
    int address=0;ctx.fail_data=1;TEST_ASSERT(Native(&ctx,0,&address,0)==5);TEST_ASSERT(ftell(ctx.f_phdata)==7&&ctx.duplicate_references==5);ctx.fail_data=0;
    address=0;ctx.fail_position=1;int frames=ctx.count_frames;TEST_ASSERT(Native(&ctx,0,&address,0)==5&&address==0&&ctx.count_frames==frames&&ftell(ctx.f_phdata)==7);ctx.fail_position=0;
    address=0;ctx.fail_manifest=1;TEST_ASSERT(Native(&ctx,0,&address,0)==5);TEST_ASSERT(ftell(ctx.f_phdata)>7&&ctx.duplicate_references==5);ctx.fail_manifest=0;
    ctx.count_references=INT_MAX;address=0;int writes=ctx.writes;TEST_ASSERT(Native(&ctx,0,&address,0)==ENS_UNSUPPORTED_PHON_FORMAT);TEST_ASSERT(ctx.writes==writes);
    uint64_t old=ctx.owner;Tidy(&ctx);TEST_ASSERT(espeak_rs_compiler_resources_end(old)==ENS_UNSUPPORTED_PHON_FORMAT);
    uint64_t owners[4];for(int i=0;i<4;i++){owners[i]=espeak_rs_compiler_resources_begin();TEST_ASSERT(owners[i]&&owners[i]!=old);}TEST_ASSERT(espeak_rs_compiler_resources_begin()==0);for(int i=0;i<4;i++)TEST_ASSERT(espeak_rs_compiler_resources_end(owners[i])==0);
    RustResourceProgress out={-17,-19,23,-29};RustCompilerResource d={0};TEST_ASSERT(espeak_rs_compiler_resource_load(NULL,&out)==ENS_UNSUPPORTED_PHON_FORMAT);TEST_ASSERT(espeak_rs_compiler_resource_load(&d,NULL)==ENS_UNSUPPORTED_PHON_FORMAT);TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);
    int32_t lower[256];unsigned char space[256];for(int i=0;i<256;i++){lower[i]=tolower(i);space[i]=isspace(i)!=0;}
    d=(RustCompilerResource){old,NULL,".",path,RustWavegenState(),lower,space,N_PATH_BUF,samplerate,0,0,0,'.',{0},NativeLog,NativeError,NativePosition,NativeWrite};
    TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);
    d.owner=espeak_rs_compiler_resources_begin();TEST_ASSERT(d.owner);d.width=1;TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.width=4097;TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.width=N_PATH_BUF;
    d.lower=NULL;TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.lower=lower;d.space=NULL;TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.space=space;
    d.generator=NULL;TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.generator=RustWavegenState();d.write=NULL;TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.write=NativeWrite;
    char input[4096];memset(input,'a',sizeof(input));d.source=input;TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.source=NULL;TEST_ASSERT(espeak_rs_compiler_resource_load(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);
    TEST_ASSERT(espeak_rs_compiler_resources_end(d.owner)==0);TEST_ASSERT(out.address==-17&&out.references==-19&&out.duplicates==23&&out.frames==-29);
}
static void BinaryEnvelope(const char *path)
{
    unsigned char bytes[140];memset(bytes,0,12);memcpy(bytes,"PITC",4);for(int i=0;i<128;i++)bytes[12+i]=i;Write(path,bytes,sizeof(bytes));
    PHONEME_TAB phoneme={0};CompileContext ctx;Setup(&ctx,&phoneme,".",path);FILE *f=fopen(path,"rb");TEST_ASSERT(f);int address=0;errno=0;
    TEST_ASSERT(ReferenceEnvelope(&ctx,f,&address)==0&&address==7&&ftell(ctx.f_phdata)==7);fclose(f);Tidy(&ctx);
    Setup(&ctx,&phoneme,".",path);ctx.owner=espeak_rs_compiler_resources_begin();address=0;TEST_ASSERT(Native(&ctx,0,&address,0)==0&&address==7&&ftell(ctx.f_phdata)==135);
    unsigned char actual[200];size_t n=Contents(ctx.f_phdata,actual,sizeof(actual));TEST_ASSERT(n==135&&memcmp(actual+7,bytes+12,128)==0);address=0;TEST_ASSERT(Native(&ctx,0,&address,0)==0&&ctx.duplicate_references==6&&ftell(ctx.f_phdata)==135);Tidy(&ctx);
    Write(path,bytes,139);Setup(&ctx,&phoneme,".",path);ctx.owner=espeak_rs_compiler_resources_begin();address=0;TEST_ASSERT(Native(&ctx,0,&address,0)==ENS_UNSUPPORTED_PHON_FORMAT&&ftell(ctx.f_phdata)==7);Tidy(&ctx);
}
int main(void)
{
    TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050);TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
    char path[128],fallback[132];snprintf(path,sizeof(path),"resource-controller-%ld.tmp",(long)getpid());snprintf(fallback,sizeof(fallback),"%s.wav",path);unsigned char bytes[12000];
    for(unsigned i=0;i<600;i++){size_t n=Data(bytes,i%3,i);Write(path,bytes,n);Pair(".",path,i%2,i==1);}
    for(unsigned i=0;i<256;i++){size_t n=Wave(bytes,i);Write(path,bytes,n);Pair(".",path,i%2,i==1);}
    for(unsigned i=0;i<100;i++){int points=2+i%18;size_t n=snprintf((char *)bytes,sizeof(bytes),"ENVELOPE\nignored\n");for(int p=0;p<points;p++)n+=snprintf((char *)bytes+n,sizeof(bytes)-n,"%.6f %.6f %u trailing\n",100.0*p/(points-1),(int)(Next()%20000)/100.0-50,i%2);Write(path,bytes,n);Pair(".",path,i%2,i==1);}
    for(int format=0;format<3;format++){size_t n=Data(bytes,format,0);for(size_t end=0;end<n;end++){Write(path,bytes,end);Pair(".",path,1,0);}}
    Write(path,(const unsigned char *)"NOT-SPEC",8);Pair(".",path,0,0);TEST_ASSERT(unlink(path)==0);Pair(".",path,0,0);Pair(".","NULL",0,0);Pair(".","DFT",0,0);
    size_t n=Wave(bytes,1);Write(fallback,bytes,n);Pair(".",path,0,0);n=Data(bytes,2,1);Write(fallback,bytes,n);Pair(".",path,0,0);TEST_ASSERT(unlink(fallback)==0);
    FILE *paths=fopen(TEST_RESOURCE_PATHS,"r");TEST_ASSERT(paths);char filename[4096];
    while(fgets(filename,sizeof(filename),paths)){size_t length=strlen(filename);TEST_ASSERT(length&&filename[length-1]=='\n');filename[length-1]=0;FILE *f=fopen(filename,"rb");TEST_ASSERT(f);unsigned char magic[4];size_t got=fread(magic,1,4,f);fclose(f);int kind=got==4&&!memcmp(magic,"SPEC",4)?0:got==4&&!memcmp(magic,"RIFF",4)?1:got==4&&!memcmp(magic,"ENVE",4)?2:-1;if(kind>=0){char *slash=strrchr(filename,'/');TEST_ASSERT(slash);*slash=0;Pair(filename,slash+1,0,0);Pair(filename,slash+1,1,0);corpus[kind]++;}}
    fclose(paths);n=Data(bytes,2,1);Write(path,bytes,n);Guards(path);BinaryEnvelope(path);TEST_ASSERT(unlink(path)==0);TEST_ASSERT(espeak_Terminate()==EE_OK);
    printf("%u full frozen C/Rust resource comparisons, %u owned requests; corpus %u spectra/%u waves/%u envelopes, both controls; native write/lifetime/reentry/counter guards and corrected binary envelope passed\n",pairs,requests,corpus[0],corpus[1],corpus[2]);return 0;
}
