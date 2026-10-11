/* Complete frozen C output-set lifecycle vs owned Rust proactor output.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <errno.h>
#include <sys/stat.h>
#ifdef _WIN32
#include <direct.h>
#include <process.h>
#define mkdir_path(p) _mkdir(p)
#define rmdir_path(p) _rmdir(p)
#define process_id _getpid
#else
#include <unistd.h>
#define mkdir_path(p) mkdir(p,0700)
#define rmdir_path(p) rmdir(p)
#define process_id getpid
#endif
#include "test_assert.h"
#include "speech.h"
#include "rust_compiler_output.h"

typedef struct {char name[4096];} Error;
typedef struct {
    char phsrc[N_PATH_BUF];FILE *f_phcontents,*f_phdata,*f_phindex,*f_phtab,*f_prog_log;
} CompileContext;
static FILE *reference_stderr;
static unsigned borrowed_closes,pairs,requests;
static void SourceClose(CompileContext *ctx) {(void)ctx;}
static void clean_context(CompileContext *ctx) {(void)ctx;}
static int create_file_error_context(Error *error,int code,const char *name)
{snprintf(error->name,sizeof(error->name),"%s",name);return code;}
static void Write4Bytes(FILE *file,uint32_t value)
{for(unsigned i=0;i<4;i++){TEST_ASSERT(fputc((value>>(i*8))&255,file)!=EOF);}}
static int ReferenceClose(FILE *file)
{
    /* Record the inherited borrowed-stderr close without destroying the test
     * capture. Native never owns or closes that stream. */
    if(file==reference_stderr){borrowed_closes++;return fflush(file);}
    return fclose(file);
}
#pragma push_macro("stderr")
#undef stderr
#define stderr reference_stderr
#define fclose ReferenceClose
#include "compiler_output_reference.inc"
#undef fclose
#undef stderr
#pragma pop_macro("stderr")
_Static_assert(sizeof(void *)!=8 || sizeof(RustCompilerOutput)==48,"output descriptor ABI");
_Static_assert(sizeof(void *)!=8 || sizeof(RustCompilerOutputResult)==4112,"output result ABI");
static const char *names[]={"phondata-manifest","phondata","phonindex","phontab","compile_prog_log"};
static unsigned char payload[3*256*1024+41],a[1024*1024],b[1024*1024];
static void Path(char *out,size_t size,const char *base,const char *name)
{int n=snprintf(out,size,"%s/%s",base,name);TEST_ASSERT(n>0&&(size_t)n<size);}
static void Make(const char *path) {TEST_ASSERT(mkdir_path(path)==0);}
static size_t Contents(FILE *file,unsigned char *bytes)
{TEST_ASSERT(fflush(file)==0);TEST_ASSERT(fseek(file,0,SEEK_END)==0);long n=ftell(file);TEST_ASSERT(n>=0&&n<1024*1024);rewind(file);TEST_ASSERT(fread(bytes,1,(size_t)n,file)==(size_t)n);return (size_t)n;}
static void SameFile(const char *left,const char *right,unsigned file,int blocked)
{
    char x[4096],y[4096];Path(x,sizeof(x),left,names[file]);Path(y,sizeof(y),right,names[file]);
    struct stat sx,sy;int lx=stat(x,&sx),ly=stat(y,&sy);TEST_ASSERT((lx==0)==(ly==0));
    if(lx!=0)return;
    if(blocked){TEST_ASSERT((sx.st_mode&S_IFMT)==S_IFDIR);TEST_ASSERT((sy.st_mode&S_IFMT)==S_IFDIR);return;}
    FILE *fx=fopen(x,"rb"),*fy=fopen(y,"rb");TEST_ASSERT(fx&&fy);
    size_t nx=Contents(fx,a),ny=Contents(fy,b);TEST_ASSERT(nx==ny);TEST_ASSERT(memcmp(a,b,nx)==0);
    TEST_ASSERT(fclose(fx)==0&&fclose(fy)==0);
}
static void Remove(const char *path)
{
    char file[4096];for(unsigned i=0;i<5;i++){Path(file,sizeof(file),path,names[i]);struct stat st;
        if(stat(file,&st)==0){if((st.st_mode&S_IFMT)==S_IFDIR){TEST_ASSERT(rmdir_path(file)==0);}else{TEST_ASSERT(remove(file)==0);}}}
    TEST_ASSERT(rmdir_path(path)==0);
}
typedef struct {
    FILE *capture;uint64_t owner;int inspect,mutate,fail,events;
    RustCompilerOutput *descriptor;RustCompilerOutputResult *out;
    char *source,*destination;unsigned char *loan;size_t length;
} Effects;
static int Fallback(void *opaque,const unsigned char *bytes,size_t length)
{
    Effects *e=opaque;e->events++;
    if(e->inspect){
        RustCompilerOutputResult nested;memset(&nested,0xa5,sizeof(nested));
        TEST_ASSERT(espeak_rs_compiler_output_begin(e->descriptor,&nested)==11);
        TEST_ASSERT(nested.owner==UINT64_C(0xa5a5a5a5a5a5a5a5));
        TEST_ASSERT(espeak_rs_compiler_output_position(e->owner,RUST_OUTPUT_DATA)==-1);
        TEST_ASSERT(espeak_rs_compiler_output_write(e->owner?e->owner:1,RUST_OUTPUT_DATA,(const unsigned char *)"x",1)==11);
        TEST_ASSERT(espeak_rs_compiler_output_flush(e->owner,RUST_OUTPUT_DATA)==11);
        if(e->owner){TEST_ASSERT(espeak_rs_compiler_output_end(e->owner)==11);}
        else {TEST_ASSERT(e->out->owner==UINT64_C(0xa5a5a5a5a5a5a5a5));}
    }
    if(e->mutate){
        if(e->loan){memset(e->loan,'X',e->length);}
        else {memset(e->source,'X',strlen(e->source));memset(e->destination,'Y',strlen(e->destination));memset(e->descriptor,0,sizeof(*e->descriptor));}
    }
    TEST_ASSERT(fwrite(bytes,1,length,e->capture)==length);
    return e->fail;
}
static void Pair(const char *base,unsigned trial,int fail,int manifest,int log)
{
    char parent[4096],left[4096],right[4096],name[4096];snprintf(name,sizeof(name),"pair-%u",trial);Path(parent,sizeof(parent),base,name);Make(parent);
    Path(left,sizeof(left),parent,"reference");Make(left);Path(right,sizeof(right),parent,"native");Make(right);
    for(unsigned lane=0;lane<2;lane++){
        const char *dir=lane?right:left;
        for(unsigned file=0;file<5;file++){if((int)file==fail||(file==0&&manifest)||(file==4&&log)){Path(name,sizeof(name),dir,names[file]);Make(name);}}
    }
    CompileContext ctx={0};snprintf(ctx.phsrc,sizeof(ctx.phsrc),"%s",left);Error error={{0}};
    reference_stderr=tmpfile();TEST_ASSERT(reference_stderr);
    Effects effects={.capture=tmpfile()};TEST_ASSERT(effects.capture);
    RustCompilerOutput d={right,right,N_PATH_BUF,0x12345678,22050,&effects,Fallback};RustCompilerOutputResult out={0};
    int c=ReferenceBegin(&ctx,left,d.version,d.rate,&error),r=espeak_rs_compiler_output_begin(&d,&out);requests++;
    TEST_ASSERT(c==r&&out.status==r);
    if(r==0){
        TEST_ASSERT(out.owner!=0);TEST_ASSERT(out.mask==(uint32_t)(31-(manifest?1:0)-(log?16:0)));
        FILE *files[]={ctx.f_phcontents,ctx.f_phdata,ctx.f_phindex,ctx.f_phtab,ctx.f_prog_log};
        size_t length=trial==0?sizeof(payload):41+(trial*37)%997;
        for(unsigned i=0;i<5;i++){
            TEST_ASSERT(espeak_rs_compiler_output_write(out.owner,i,payload,length)==0);requests++;
            if(files[i]){TEST_ASSERT(fwrite(payload,1,length,files[i])==length);}
            if(i!=0&&files[i]){TEST_ASSERT(espeak_rs_compiler_output_position(out.owner,i)==ftell(files[i]));}
        }
        TEST_ASSERT(espeak_rs_compiler_output_flush(out.owner,RUST_OUTPUT_INDEX)==0);
        TEST_ASSERT(espeak_rs_compiler_output_end(out.owner)==0);ReferenceFinish(&ctx);
        TEST_ASSERT(espeak_rs_compiler_output_end(out.owner)==9);
    }else{TEST_ASSERT(out.owner==0&&out.mask==0);TEST_ASSERT(fail>=1&&fail<=3);
        TEST_ASSERT(strstr(error.name,names[fail]));TEST_ASSERT(strstr((char *)out.filename,names[fail]));}
    for(unsigned i=0;i<5;i++){SameFile(left,right,i,(int)i==fail||(i==0&&manifest)||(i==4&&log));}
    size_t x=Contents(reference_stderr,a),y=Contents(effects.capture,b);TEST_ASSERT(x==y&&memcmp(a,b,x)==0);
    /* Both fallback captures remain usable; borrowed C close is only recorded. */
    TEST_ASSERT(fputc('!',effects.capture)!=EOF);TEST_ASSERT(fclose(reference_stderr)==0&&fclose(effects.capture)==0);
    Remove(left);Remove(right);TEST_ASSERT(rmdir_path(parent)==0);pairs++;
}
static void Guards(const char *base)
{
    char dir[4096],path[4096];Path(dir,sizeof(dir),base,"guards");Make(dir);
    Path(path,sizeof(path),dir,names[0]);Make(path);Path(path,sizeof(path),dir,names[4]);Make(path);
    char source[4096],destination[4096];snprintf(source,sizeof(source),"%s",dir);snprintf(destination,sizeof(destination),"%s",dir);
    Effects e={.capture=tmpfile(),.inspect=1,.mutate=1,.source=source,.destination=destination};TEST_ASSERT(e.capture);
    RustCompilerOutput d={source,destination,N_PATH_BUF,0x12345678,22050,&e,Fallback};RustCompilerOutputResult out;
    e.descriptor=&d;e.out=&out;memset(&out,0xa5,sizeof(out));
    TEST_ASSERT(espeak_rs_compiler_output_begin(&d,&out)==0);TEST_ASSERT(out.mask==14&&out.owner);e.owner=out.owner;
    TEST_ASSERT(espeak_rs_compiler_output_position(out.owner,RUST_OUTPUT_DATA)==8);
    /* Restore input for reentry, then mutate the caller byte loan in callback. */
    snprintf(source,sizeof(source),"%s",dir);snprintf(destination,sizeof(destination),"%s",dir);
    d=(RustCompilerOutput){source,destination,N_PATH_BUF,1,8000,&e,Fallback};
    unsigned char loan[]="S original\n";e.loan=loan;e.length=sizeof(loan)-1;
    TEST_ASSERT(espeak_rs_compiler_output_write(out.owner,0,loan,e.length)==0);TEST_ASSERT(loan[0]=='X');
    TEST_ASSERT(espeak_rs_compiler_output_write(out.owner,1,(const unsigned char *)"pending",7)==0);
    e.fail=28;TEST_ASSERT(espeak_rs_compiler_output_write(out.owner,0,(const unsigned char *)"fail",4)==28);
    TEST_ASSERT(espeak_rs_compiler_output_write(out.owner,2,(const unsigned char *)"ignored",7)==28);
    TEST_ASSERT(espeak_rs_compiler_output_end(out.owner)==28);TEST_ASSERT(espeak_rs_compiler_output_end(out.owner)==9);
    size_t n=Contents(e.capture,a);TEST_ASSERT(n>sizeof(loan));TEST_ASSERT(memcmp(a+n-4-e.length,"S original\n",e.length)==0);
    Path(path,sizeof(path),dir,names[1]);FILE *data=fopen(path,"rb");TEST_ASSERT(data);n=Contents(data,a);TEST_ASSERT(n==15&&memcmp(a+8,"pending",7)==0);TEST_ASSERT(fclose(data)==0);
    e.inspect=0;e.mutate=0;e.fail=0;d=(RustCompilerOutput){dir,dir,N_PATH_BUF,1,8000,&e,Fallback};
    TEST_ASSERT(espeak_rs_compiler_output_begin(&d,&out)==0);TEST_ASSERT(out.owner!=e.owner);
    TEST_ASSERT(espeak_rs_compiler_output_write(out.owner,5,NULL,0)==22);
    TEST_ASSERT(espeak_rs_compiler_output_write(out.owner,1,NULL,1)==22);
    TEST_ASSERT(espeak_rs_compiler_output_write(out.owner,1,a,16*1024*1024+1)==22);
    TEST_ASSERT(espeak_rs_compiler_output_write(out.owner,1,NULL,0)==0);
    RustCompilerOutputResult rejected;memset(&rejected,0xa5,sizeof(rejected));
    TEST_ASSERT(espeak_rs_compiler_output_begin(&d,&rejected)!=0&&rejected.owner==0);
    TEST_ASSERT(espeak_rs_compiler_output_position(out.owner,1)==8); /* Other owner untouched. */
    TEST_ASSERT(espeak_rs_compiler_output_end(out.owner)==0);
    d.width=1;memset(&out,0xa5,sizeof(out));TEST_ASSERT(espeak_rs_compiler_output_begin(&d,&out)==22&&out.owner==UINT64_C(0xa5a5a5a5a5a5a5a5));
    d.width=4097;TEST_ASSERT(espeak_rs_compiler_output_begin(&d,&out)==22);d.width=N_PATH_BUF;
    d.fallback=NULL;TEST_ASSERT(espeak_rs_compiler_output_begin(&d,&out)==22);d.fallback=Fallback;
    memset(source,'X',sizeof(source));d.source=source;TEST_ASSERT(espeak_rs_compiler_output_begin(&d,&out)==22);
    TEST_ASSERT(espeak_rs_compiler_output_begin(NULL,&out)==22);TEST_ASSERT(espeak_rs_compiler_output_begin(&d,NULL)==22);
    TEST_ASSERT(espeak_rs_compiler_output_end(0)==0);TEST_ASSERT(espeak_rs_compiler_output_position(e.owner,1)==-1);
    TEST_ASSERT(fputc('!',e.capture)!=EOF);TEST_ASSERT(fclose(e.capture)==0);Remove(dir);
}
int main(void)
{
    char base[128];snprintf(base,sizeof(base),"compiler-output-%ld",(long)process_id());Make(base);
    for(size_t i=0;i<sizeof(payload);i++){payload[i]=(unsigned char)(i*19);}
    unsigned trial=0;
    for(unsigned i=0;i<64;i++){Pair(base,trial++,-1,0,0);}
    for(int fail=-1;fail<=3;fail++){
        if(fail==0)continue;
        for(int manifest=0;manifest<2;manifest++){for(int log=0;log<2;log++){Pair(base,trial++,fail,manifest,log);}}
    }
    Guards(base);TEST_ASSERT(borrowed_closes==8);TEST_ASSERT(rmdir_path(base)==0);
    printf("%u complete C output-set comparisons and %u owned write requests; native loan/reentry/stale/admission/cleanup guards pass; %u inherited borrowed-stderr closes recorded, never owned by Rust\n",pairs,requests,borrowed_closes);
    return 0;
}
