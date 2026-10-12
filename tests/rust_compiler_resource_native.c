/* Previous C native host vs owned Rust context/output/resource host.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#define main ReaderFixtureEntry
#define Pair ReaderPair
#define Guards ReaderGuards
#include "rust_spect_file.c"
#undef main
#undef Pair
#undef Guards
#include "rust_compiler_resource.h"
#include "rust_compiler_diagnostic.h"
#include "rust_compiler_output.h"
#include "speech.h"
#include "common.h"
#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <locale.h>
#include <pthread.h>
#include <stdarg.h>
#include <sys/stat.h>
typedef RustCompilerContext CompileContext;
static FILE *host_log;
static RustNativeCompilerResource *mutable_descriptor;
static RustResourceProgress before;
static unsigned host_pairs,host_requests,guard_callbacks;
static int classes_called;
static void Classes(unsigned char *spaces,unsigned char *digits)
{
    classes_called++;memset(spaces,0,256);memset(digits,0,256);
}
static void Guard(CompileContext *ctx)
{
    TEST_ASSERT(ctx->resource_owner);
    if(mutable_descriptor) {
        TEST_ASSERT(ctx->count_references==before.references&&ctx->duplicate_references==before.duplicates&&ctx->count_frames==before.frames);
        int address=-77;
        TEST_ASSERT(espeak_rs_compiler_resource_native(mutable_descriptor,&address)==EAGAIN&&address==-77);
        TEST_ASSERT(espeak_rs_compiler_context_end(ctx)==EAGAIN);
        RustCompilerSourceOpen source={ctx,"ignored",N_PATH_BUF,0,0,Classes};int status=0;
        TEST_ASSERT(espeak_rs_compiler_context_source(&source,&status)==0&&status==EAGAIN&&classes_called==0);
        strcpy(ctx->phsrc,"mutated-after-admission");
        mutable_descriptor->name=NULL;mutable_descriptor->generator=NULL;mutable_descriptor->error=NULL;
        guard_callbacks++;mutable_descriptor=NULL;
    }
}
static void error(CompileContext *ctx,const char *format,...)
{
    Guard(ctx);
    va_list args;va_start(args,format);
    fprintf(ctx->f_errors,"%s(%d): ",ctx->current_fname,ctx->linenum-1);
    vfprintf(ctx->f_errors,format,args);fputc('\n',ctx->f_errors);ctx->error_count++;va_end(args);
}
#pragma push_macro("stderr")
#undef stderr
#define stderr host_log
#define ResourceLog PreviousLog
#define ResourceError PreviousError
#define ResourcePosition PreviousPosition
#define ResourceWrite PreviousWrite
#define LoadDataFile PreviousLoad
#include "compiler_resource_native_reference.inc"
#undef LoadDataFile
#undef ResourceLog
#undef ResourceError
#undef ResourcePosition
#undef ResourceWrite
static void NativeDiagnostic(void *context,int code,int a,int b,const unsigned char *bytes,size_t length)
{
    Guard(context);
    espeak_rs_compiler_diagnostic_resource(context,code,a,b,bytes,length);
}
#define espeak_rs_compiler_diagnostic_resource NativeDiagnostic
#include "compiler_resource_native_diagnostic.inc"
#undef espeak_rs_compiler_diagnostic_resource
static int NativeLoad(CompileContext *ctx,const char *path,int control,int *addr)
{
#include "compiler_resource_native_admission.inc"
}
#undef stderr
#pragma pop_macro("stderr")
_Static_assert(sizeof(RustNativeCompilerResource)==(sizeof(void *)==8?56:32),"native resource ABI");
static int Fallback(void *context,const unsigned char *bytes,size_t length)
{
    (void)context;(void)bytes;(void)length;return 0;
}
static size_t Bytes(const char *path,unsigned char *bytes,size_t capacity)
{
    FILE *f=fopen(path,"rb");TEST_ASSERT(f);size_t n=fread(bytes,1,capacity,f);
    TEST_ASSERT(feof(f));fclose(f);return n;
}
static void Pair(const char *root,const char *source,const char *name,int control,int guard,unsigned trial)
{
    static unsigned char captured[3][7][100000],actual[100000];
    size_t sizes[3][7];int expected_status[3],expected_address[3],expected_errors[3];
    RustResourceProgress progress[3];
    const char *files[]={"log","errors","phondata-manifest","phondata","phonindex","phontab","compile_prog_log"};
    PHONEME_TAB phoneme={0};phoneme.mnemonic=trial%2?0xff806261:0x6261;
    /* The five retained writer buffers admit one complete output set at a time.
     * Run full C and native episodes serially; retain every phase's exact bytes. */
    for(int i=0;i<2;i++) {
        char lane[512],logpath[1024],errorpath[1024],path[1024];
        snprintf(lane,sizeof(lane),"%s/lane%d",root,i);
        snprintf(logpath,sizeof(logpath),"%s/log",lane);snprintf(errorpath,sizeof(errorpath),"%s/errors",lane);
        FILE *log=fopen(logpath,"wb"),*diagnostics=fopen(errorpath,"wb");TEST_ASSERT(log&&diagnostics);
        int status=-1;CompileContext *ctx=espeak_rs_compiler_context_begin(source,N_PATH_BUF,&status);TEST_ASSERT(ctx&&status==0);
        RustCompilerOutput d={lane,lane,N_PATH_BUF,123,22050,NULL,Fallback};RustCompilerOutputResult output={0};
        TEST_ASSERT(espeak_rs_compiler_output_begin(&d,&output)==0&&output.owner);
        ctx->output_owner=output.owner;ctx->output_mask=output.mask;ctx->f_errors=diagnostics;
        ctx->count_references=11;ctx->duplicate_references=5;ctx->count_frames=31;
        ctx->n_phoneme_tabs=3;ctx->phoneme_out=trial%5?&phoneme:NULL;
        strcpy(ctx->current_fname,"resource-fixture");ctx->linenum=17;uint64_t resource=0;
        for(int phase=0;phase<3;phase++) {
            int address=phase==2?777:0;host_log=log;
            before=(RustResourceProgress){address,ctx->count_references,ctx->duplicate_references,ctx->count_frames};
            if(!i) {status=PreviousLoad(ctx,name,control,&address);}
            else if(!guard||phase) {status=NativeLoad(ctx,name,control,&address);}
            else {
                RustNativeCompilerResource native={ctx,name,RustWavegenState(),log,N_PATH_BUF,samplerate,control,ResourceError};
                mutable_descriptor=&native;status=espeak_rs_compiler_resource_native(&native,&address);
                TEST_ASSERT(mutable_descriptor==NULL);
            }
            TEST_ASSERT(ctx->resource_owner&&(!resource||resource==ctx->resource_owner));resource=ctx->resource_owner;
            strcpy(ctx->phsrc,source);
            for(unsigned f=0;f<5;f++) {TEST_ASSERT(espeak_rs_compiler_output_flush(ctx->output_owner,f)==0);}
            TEST_ASSERT(fflush(log)==0&&fflush(diagnostics)==0);host_requests++;
            RustResourceProgress result={address,ctx->count_references,ctx->duplicate_references,ctx->count_frames};
            if(!i) {expected_status[phase]=status;expected_address[phase]=address;expected_errors[phase]=ctx->error_count;progress[phase]=result;}
            else {TEST_ASSERT(status==expected_status[phase]&&address==expected_address[phase]&&ctx->error_count==expected_errors[phase]&&memcmp(&result,&progress[phase],sizeof(result))==0);}
            for(unsigned f=0;f<7;f++) {
                snprintf(path,sizeof(path),"%s/%s",lane,files[f]);
                if(!i) {sizes[phase][f]=Bytes(path,captured[phase][f],sizeof(captured[phase][f]));}
                else {size_t n=Bytes(path,actual,sizeof(actual));TEST_ASSERT(n==sizes[phase][f]&&memcmp(actual,captured[phase][f],n)==0);}
            }
        }
        uint64_t owner=ctx->output_owner;
        TEST_ASSERT(espeak_rs_compiler_context_end(ctx)==0);
        TEST_ASSERT(espeak_rs_compiler_resources_end(resource)!=0&&espeak_rs_compiler_output_end(owner)!=0);
        TEST_ASSERT(fcntl(fileno(diagnostics),F_GETFD)!=-1);fclose(diagnostics);fclose(log);
    }
    host_pairs++;
}
static CompileContext *thread_context;
static void *WrongThread(void *unused)
{
    (void)unused;int address=73;
    RustNativeCompilerResource d={thread_context,"NULL",RustWavegenState(),host_log,N_PATH_BUF,samplerate,0,ResourceError};
    TEST_ASSERT(espeak_rs_compiler_resource_native(&d,&address)==EAGAIN&&address==73);
    return NULL;
}
static void Guards(void)
{
    int status=-1;CompileContext *ctx=espeak_rs_compiler_context_begin(".",2,&status);TEST_ASSERT(ctx&&status==0);
    FILE *f=tmpfile();TEST_ASSERT(f);host_log=f;
    RustNativeCompilerResource good={ctx,"NULL",RustWavegenState(),f,N_PATH_BUF,samplerate,0,ResourceError};
    int address=73;TEST_ASSERT(espeak_rs_compiler_resource_native(NULL,&address)==ENS_UNSUPPORTED_PHON_FORMAT);
    TEST_ASSERT(espeak_rs_compiler_resource_native(&good,NULL)==ENS_UNSUPPORTED_PHON_FORMAT);
    char bad[4096];memset(bad,'a',sizeof(bad));
    for(int i=0;i<8;i++) {
        RustNativeCompilerResource d=good;
        switch(i) {
        case 0:d.width=1;break;case 1:d.width=4097;break;case 2:d.name=NULL;break;
        case 3:d.generator=NULL;break;case 4:d.log=NULL;break;case 5:d.error=NULL;break;
        case 6:d.name=bad;break;default:ctx->n_phoneme_tabs=INT_MIN;break;
        }
        TEST_ASSERT(espeak_rs_compiler_resource_native(&d,&address)==ENS_UNSUPPORTED_PHON_FORMAT&&address==73&&ctx->resource_owner==0);
        ctx->n_phoneme_tabs=0;
    }
    thread_context=ctx;pthread_t thread;TEST_ASSERT(pthread_create(&thread,NULL,WrongThread,NULL)==0&&pthread_join(thread,NULL)==0);
    TEST_ASSERT(ctx->resource_owner==0);
    uint64_t owners[4];for(int i=0;i<4;i++) {owners[i]=espeak_rs_compiler_resources_begin();TEST_ASSERT(owners[i]);}
    TEST_ASSERT(espeak_rs_compiler_resource_native(&good,&address)==ENOMEM&&address==73&&ctx->resource_owner==0);
    for(int i=0;i<4;i++) {TEST_ASSERT(espeak_rs_compiler_resources_end(owners[i])==0);}
    TEST_ASSERT(espeak_rs_compiler_resource_native(&good,&address)==0&&address==73&&ctx->resource_owner);
    union {int32_t address[2];char name[8];} overlap;
    memset(&overlap,0,sizeof(overlap));memcpy(overlap.name,"DFT",4);
    RustNativeCompilerResource copied=good;copied.name=overlap.name;
    TEST_ASSERT(espeak_rs_compiler_resource_native(&copied,overlap.address)==0&&overlap.address[0]==1);
    ctx->count_references=INT_MAX;copied.name="missing";
    TEST_ASSERT(espeak_rs_compiler_resource_native(&copied,&address)==ENS_UNSUPPORTED_PHON_FORMAT&&address==73&&ctx->count_references==INT_MAX);
    ctx->count_references=0;
    uint64_t old=ctx->resource_owner;TEST_ASSERT(espeak_rs_compiler_context_end(ctx)==0&&espeak_rs_compiler_resources_end(old)!=0);fclose(f);
}
int main(void)
{
    TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050&&espeak_SetVoiceByName("en")==EE_OK);
    char root[256],source[512],path[1024],fallback[1024];snprintf(root,sizeof(root),"native-resource-%ld.tmp",(long)getpid());
    TEST_ASSERT(mkdir(root,0700)==0);snprintf(source,sizeof(source),"%s/source",root);TEST_ASSERT(mkdir(source,0700)==0);
    for(int i=0;i<2;i++) {snprintf(path,sizeof(path),"%s/lane%d",root,i);TEST_ASSERT(mkdir(path,0700)==0);}
    snprintf(path,sizeof(path),"%s/resource",source);snprintf(fallback,sizeof(fallback),"%s/resource.wav",source);unsigned char bytes[12000];
    for(unsigned i=0;i<150;i++) {
        size_t n;
        if(i%3==0) {n=Data(bytes,(i/3)%3,i);}
        else if(i%3==1) {
            memset(bytes,0,44);memcpy(bytes,"RIFF",4);unsigned rate=i%7?samplerate:21000,count=32;
            for(int j=0;j<4;j++) {bytes[24+j]=rate>>(j*8);bytes[28+j]=(rate*2)>>(j*8);bytes[40+j]=(count*2)>>(j*8);}
            for(unsigned j=0;j<count*2;j++) {bytes[44+j]=Next();}n=44+count*2;
        } else {n=snprintf((char *)bytes,sizeof(bytes),"ENVELOPE\nignored\n0 20 1\n50 %u 1\n100 0 1\n",i);}
        Write(path,bytes,n);Pair(root,source,"resource",i%2,0,i);
    }
    Write(path,(const unsigned char *)"NOT-SPEC",8);Pair(root,source,"resource",0,1,1);
    TEST_ASSERT(unlink(path)==0);Pair(root,source,"resource",0,1,1);
    Pair(root,source,"NULL",0,0,0);Pair(root,source,"DFT",0,0,0);
    size_t n=Data(bytes,2,1);Write(fallback,bytes,n);Pair(root,source,"resource",0,1,1);TEST_ASSERT(unlink(fallback)==0);
    memset(bytes,0,140);memcpy(bytes,"PITC",4);for(int i=0;i<128;i++) {bytes[12+i]=i;}
    Write(path,bytes,140);Pair(root,source,"resource",0,0,0);TEST_ASSERT(unlink(path)==0);Guards();
    const char *files[]={"log","errors","phondata-manifest","phondata","phonindex","phontab","compile_prog_log"};
    for(int i=0;i<2;i++) {for(unsigned f=0;f<7;f++) {snprintf(path,sizeof(path),"%s/lane%d/%s",root,i,files[f]);TEST_ASSERT(unlink(path)==0);}snprintf(path,sizeof(path),"%s/lane%d",root,i);TEST_ASSERT(rmdir(path)==0);}
    TEST_ASSERT(rmdir(source)==0&&rmdir(root)==0&&espeak_Terminate()==EE_OK);
    printf("%u complete previous-C/native resource host pairs, %u real registered-output requests; %u guarded diagnostic callbacks; lease/backpressure/thread/reentry/admission guards pass\n",host_pairs,host_requests,guard_callbacks);return 0;
}
