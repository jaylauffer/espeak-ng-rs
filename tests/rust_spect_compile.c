/* Full frozen C spectral-to-phondata controller vs owned Rust driver.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include <stdio.h>
#include <stdarg.h>
static FILE *source_log;
static int SourcePrintf(FILE *stream,const char *format,...)
{
    va_list args;va_start(args,format);int result=vfprintf(stream==stderr&&source_log?source_log:stream,format,args);va_end(args);return result;
}
/* Reuse the original complete C reader, initialized harmonic projection and
 * source generators from the reader fixture; its entry point is not executed. */
#define main SpectReaderFixtureEntry
#define fprintf SourcePrintf
#include "rust_spect_file.c"
#undef main
#undef fprintf
#include "rust_spect_compile.h"
#include "speech.h"
typedef struct {
    char phsrc[N_PATH_BUF];
    FILE *f_phdata,*f_errors,*log;
    int count_frames,error_count,positions,writes,mutate,fail_write;
    char path[4096];RustSpectCompiler *descriptor;RustSpectResult *output;
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
#define LoadSpect ReferenceController
#include "spect_compile_reference.inc"
#undef LoadSpect
#undef GetFrameRms
#undef LoadSpectSeq
#undef SpectSeqDestroy
#undef SpectSeqCreate
_Static_assert(sizeof(RustSpectCompiler)==(sizeof(void *)==8?88:48),"spectral compiler descriptor ABI");
_Static_assert(sizeof(RustSpectResult)==12 && sizeof(frame_t)==64 && sizeof(frame_t2)==44,"spectral payload ABI");
static unsigned controller_cases,controller_corpus;
static void NativeLog(void *opaque,const unsigned char *bytes,size_t length)
{
    CompileContext *ctx=opaque;TEST_ASSERT(fwrite(bytes,1,length,ctx->log)==length);
}
static void NativeBad(void *opaque,const char *path){error(opaque,"Bad vowel file: '%s'",path);}
static int64_t NativePosition(void *opaque)
{
    CompileContext *ctx=opaque;ctx->positions++;
    TEST_ASSERT(ctx->output->address==-17 && ctx->output->count_frames==-19 && ctx->output->has_address==23);
    if(ctx->mutate) {
        RustSpectResult nested={-17,-19,23};TEST_ASSERT(espeak_rs_compile_spect(ctx->descriptor,&nested)==ENS_UNSUPPORTED_PHON_FORMAT);
        TEST_ASSERT(nested.address==-17 && nested.count_frames==-19 && nested.has_address==23);
        strcpy(ctx->phsrc,"changed-after-admission");strcpy(ctx->path,"changed-after-admission");
        ctx->descriptor->generator=NULL;ctx->descriptor->write=NULL;ctx->descriptor->initial_count=INT_MAX;
    }
    return ftell(ctx->f_phdata);
}
static int NativeWrite(void *opaque,const unsigned char *bytes,size_t length)
{
    CompileContext *ctx=opaque;ctx->writes++;
    TEST_ASSERT(ctx->output->address==-17 && ctx->output->count_frames==-19 && ctx->output->has_address==23);
    TEST_ASSERT(length<=1604 && length%4==0);if(ctx->fail_write)return 5;return fwrite(bytes,1,length,ctx->f_phdata)==length?0:5;
}
static size_t Contents(FILE *f,unsigned char *bytes,size_t capacity)
{
    fflush(f);rewind(f);size_t n=fread(bytes,1,capacity,f);TEST_ASSERT(feof(f));return n;
}
static void ControllerPair(const char *source,const char *name,int control,int mutate)
{
    CompileContext ctx[2]={{0},{0}};int status[2],address[2]={-42,-42};
    for(int lane=0;lane<2;lane++) {
        TEST_ASSERT(strlen(source)<sizeof(ctx[lane].phsrc));strcpy(ctx[lane].phsrc,source);TEST_ASSERT(strlen(name)<sizeof(ctx[lane].path));strcpy(ctx[lane].path,name);
        ctx[lane].f_phdata=tmpfile();ctx[lane].f_errors=tmpfile();ctx[lane].log=tmpfile();TEST_ASSERT(ctx[lane].f_phdata&&ctx[lane].f_errors&&ctx[lane].log);
        ctx[lane].count_frames=31;for(int i=0;i<7;i++)fputc(0x5a,ctx[lane].f_phdata);
        if(lane==0) {source_log=ctx[0].log;status[0]=ReferenceController(&ctx[0],name,control,&address[0]);source_log=NULL;}
        else {
            RustSpectCompiler d={&ctx[1],ctx[1].phsrc,ctx[1].path,RustWavegenState(),N_PATH_BUF,samplerate,control,31,NativeLog,NativeBad,NativePosition,NativeWrite};
            RustSpectResult out={-17,-19,23};ctx[1].descriptor=&d;ctx[1].output=&out;ctx[1].mutate=mutate;
            status[1]=espeak_rs_compile_spect(&d,&out);ctx[1].count_frames=out.count_frames;if(out.has_address)address[1]=out.address;
        }
    }
    if(status[0]!=status[1])fprintf(stderr,"controller mismatch %s/%s: C=%x Rust=%x\n",source,name,status[0],status[1]);
    TEST_ASSERT(status[0]==status[1] && address[0]==address[1] && ctx[0].count_frames==ctx[1].count_frames && ctx[0].error_count==ctx[1].error_count);
    unsigned char expected[8192],actual[8192];FILE *a[]={ctx[0].f_phdata,ctx[0].f_errors,ctx[0].log};FILE *b[]={ctx[1].f_phdata,ctx[1].f_errors,ctx[1].log};
    for(int i=0;i<3;i++){size_t x=Contents(a[i],expected,sizeof(expected)),y=Contents(b[i],actual,sizeof(actual));TEST_ASSERT(x==y);TEST_ASSERT(memcmp(expected,actual,x)==0);fclose(a[i]);fclose(b[i]);}
    if(status[0]==0){TEST_ASSERT(ctx[1].positions==1 && ctx[1].writes==1);}else{TEST_ASSERT(ctx[1].positions==0 && ctx[1].writes==0);}
    controller_cases++;
}
static void ControllerGuards(void)
{
    RustSpectResult out={-17,-19,23};RustSpectCompiler d={NULL,".","x",RustWavegenState(),N_PATH_BUF,22050,0,0,NativeLog,NativeBad,NativePosition,NativeWrite};
    TEST_ASSERT(espeak_rs_compile_spect(NULL,&out)==ENS_UNSUPPORTED_PHON_FORMAT);TEST_ASSERT(espeak_rs_compile_spect(&d,NULL)==ENS_UNSUPPORTED_PHON_FORMAT);
    d.width=1;TEST_ASSERT(espeak_rs_compile_spect(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.width=4097;TEST_ASSERT(espeak_rs_compile_spect(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.width=N_PATH_BUF;
    d.generator=NULL;TEST_ASSERT(espeak_rs_compile_spect(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.generator=RustWavegenState();d.write=NULL;TEST_ASSERT(espeak_rs_compile_spect(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.write=NativeWrite;
    char input[4096];memset(input,'a',sizeof(input));d.source=input;TEST_ASSERT(espeak_rs_compile_spect(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);d.source=NULL;TEST_ASSERT(espeak_rs_compile_spect(&d,&out)==ENS_UNSUPPORTED_PHON_FORMAT);
    TEST_ASSERT(out.address==-17 && out.count_frames==-19 && out.has_address==23);
}
static void NativeDriverGuard(const char *path,int initial_count,int fail_write,int expected_status,int expected_count,int expected_writes)
{
    CompileContext ctx={0};strcpy(ctx.phsrc,".");ctx.f_phdata=tmpfile();ctx.f_errors=tmpfile();ctx.log=tmpfile();TEST_ASSERT(ctx.f_phdata&&ctx.f_errors&&ctx.log);
    for(int i=0;i<7;i++)fputc(0x5a,ctx.f_phdata);ctx.fail_write=fail_write;
    RustSpectCompiler d={&ctx,ctx.phsrc,path,RustWavegenState(),N_PATH_BUF,samplerate,0,initial_count,NativeLog,NativeBad,NativePosition,NativeWrite};RustSpectResult out={-17,-19,23};ctx.output=&out;
    TEST_ASSERT(espeak_rs_compile_spect(&d,&out)==expected_status);TEST_ASSERT(out.address==7&&out.has_address==1&&out.count_frames==expected_count);
    TEST_ASSERT(ctx.positions==1&&ctx.writes==expected_writes&&ctx.error_count==0);TEST_ASSERT(ftell(ctx.f_phdata)==7);
    fclose(ctx.f_phdata);fclose(ctx.f_errors);fclose(ctx.log);
}
int main(void)
{
    TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050);TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
    char path[128];snprintf(path,sizeof(path),"spect-controller-%ld.tmp",(long)getpid());unsigned char bytes[12000];
    for(unsigned i=0;i<600;i++){size_t n=Data(bytes,i%3,i);Write(path,bytes,n);ControllerPair(".",path,i%2,i==0);}
    for(int format=0;format<3;format++){size_t n=Data(bytes,format,0);for(size_t end=0;end<n;end++){Write(path,bytes,end);ControllerPair(".",path,1,0);}}
    size_t two=Data(bytes,2,1);size_t frame_size=(two-26)/2;unsigned char frame[512];TEST_ASSERT(frame_size<=sizeof(frame));memcpy(frame,bytes+26,frame_size);frame[56]=1;frame[57]=0;
    bytes[18]=25;bytes[19]=0;for(int i=0;i<25;i++)memcpy(bytes+26+i*frame_size,frame,frame_size);
    Write(path,bytes,26+25*frame_size);ControllerPair(".",path,0,0);ControllerPair(".",path,1,0);
    unsigned char ordinary[12000];memcpy(ordinary,bytes,26+25*frame_size);
    for(int i=0;i<25;i++){unsigned char *f=ordinary+26+i*frame_size;f[42]=f[43]=0;memset(f+222,0,18);}
    Write(path,ordinary,26+25*frame_size);ControllerPair(".",path,0,0);ControllerPair(".",path,1,0);
    Write(path,bytes,26+25*frame_size);
    NativeDriverGuard(path,INT_MAX,0,ENS_UNSUPPORTED_PHON_FORMAT,INT_MAX,0);
    NativeDriverGuard(path,31,1,5,56,1);
    bytes[18]=26;memcpy(bytes+26+25*frame_size,frame,frame_size);Write(path,bytes,26+26*frame_size);NativeDriverGuard(path,31,0,ENS_UNSUPPORTED_PHON_FORMAT,31,0);
    bytes[18]=1;bytes[20]=255;bytes[21]=127;memcpy(bytes+26,frame,frame_size);bytes[26+44]=255;bytes[26+45]=127;bytes[26+56]=255;bytes[26+57]=127;Write(path,bytes,26+frame_size);NativeDriverGuard(path,31,0,ENS_UNSUPPORTED_PHON_FORMAT,31,0);
    size_t n=Data(bytes,2,0);bytes[18]=bytes[19]=0;Write(path,bytes,n);ControllerPair(".",path,1,0);bytes[0]=0;Write(path,bytes,n);ControllerPair(".",path,1,0);
    TEST_ASSERT(unlink(path)==0);ControllerPair(".",path,0,0);
    FILE *paths=fopen(TEST_SPECT_PATHS,"r");TEST_ASSERT(paths);char filename[4096];
    while(fgets(filename,sizeof(filename),paths)){size_t length=strlen(filename);TEST_ASSERT(length&&filename[length-1]=='\n');filename[length-1]=0;FILE *f=fopen(filename,"rb");TEST_ASSERT(f);unsigned char magic[4];size_t got=fread(magic,1,4,f);fclose(f);if(got==4&&!memcmp(magic,"SPEC",4)){
        char *slash=strrchr(filename,'/');TEST_ASSERT(slash);*slash=0;ControllerPair(filename,slash+1,0,0);ControllerPair(filename,slash+1,1,0);controller_corpus++;}}
    fclose(paths);ControllerGuards();TEST_ASSERT(espeak_Terminate()==EE_OK);
    printf("%u frozen C/Rust spectral controller output/status/address/count/diagnostic comparisons (%u corpus files, both controls); admission/reentry/mutated-caller and four native driver guards passed\n",controller_cases,controller_corpus);return 0;
}
