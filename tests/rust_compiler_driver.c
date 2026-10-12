/* Frozen complete public driver/manifest versus Rust and actual C primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include <espeak-ng/espeak_ng.h>
#include "speech.h"
#include "common.h"
#include "rust_compiler_driver.h"
#include "rust_compiler_output.h"
#include "test_assert.h"
#include <ctype.h>
#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
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
_Static_assert(sizeof(RustCompilerCounts)==12,"counts");
_Static_assert(sizeof(RustManifestRow)==124,"manifest row");
#if UINTPTR_MAX==UINT64_MAX
_Static_assert(sizeof(RustPhonemeCompiler)==96,"driver descriptor");
_Static_assert(sizeof(RustManifest)==48,"manifest descriptor");
#endif
#define version_phdata 123
static int samplerate;static void *voice=(void *)1;
char path_home[N_PATH_BUF];
typedef struct {unsigned value;char *name;} NAMETAB;
typedef struct {
 uint64_t table_owner,program_owner,output_owner;uint32_t output_mask;
 int f_in,n_phoneme_tabs,table_status,program_status,count_references,duplicate_references,error_count;
 FILE *f_errors;NAMETAB *manifest;int n_manifest;char current_fname[80],phsrc[N_PATH_BUF];
} CompileContext;
typedef struct {
 unsigned trace[256],count,fail;int errors,table_status,program_status,output_status;
 unsigned references,reused;char source[4096],destination[4096],input[4096],error_path[4096];
 int error_status,cleaned;unsigned char diagnostics[8192];size_t diagnostic_len;
} Plan;
static Plan *current;
static FILE *retained_log;
static void Trace(unsigned op);
static int IgnoreClose(FILE *f){if(f==retained_log){Trace(RUST_COMPILER_CLOSE_LOG);return 0;}return fclose(f);}
static void Trace(unsigned op){TEST_ASSERT(current->count<256);current->trace[current->count++]=op;}
static void *OracleCalloc(size_t n,size_t width){Trace(RUST_COMPILER_ALLOCATE);if(current->fail==1){return NULL;}return calloc(n,width);}
static uint64_t OracleTablesBegin(void){Trace(RUST_COMPILER_TABLES);return current->fail==2?0:1;}
static uint64_t OracleProgramBegin(void){Trace(RUST_COMPILER_PROGRAM);return current->fail==3?0:2;}
static void clean_context(CompileContext *c){Trace(RUST_COMPILER_CLEANUP);current->cleaned++;for(int i=0;i<c->n_manifest;i++){free(c->manifest[i].name);}free(c->manifest);free(c);}
static void LoadPhData(void *a,void *b){(void)a;(void)b;Trace(RUST_COMPILER_RELOAD);}
static void *LoadVoice(const char *name,int mode){TEST_ASSERT(!*name&&mode==8);Trace(RUST_COMPILER_VOICE);return current->fail==4?NULL:voice;}
static void WavegenInit(long rate,int x){TEST_ASSERT(rate==22050&&x==0);Trace(RUST_COMPILER_WAVE_INIT);}
static void WavegenSetVoice(void *v){TEST_ASSERT(v==voice);}
static void WavegenFini(void){Trace(RUST_COMPILER_WAVE_FINI);}
static int SourceOpen(CompileContext *c,const char *path,int depth,int ignored){(void)c;TEST_ASSERT(depth==0&&ignored==0);Trace(RUST_COMPILER_SOURCE_OPEN);strcpy(current->input,path);errno=2;return current->fail==5?0:1;}
static void SourceClose(CompileContext *c){Trace(RUST_COMPILER_SOURCE_CLOSE);c->f_in=0;}
static int OutputFallback(void *c,const unsigned char *bytes,size_t n){(void)c;(void)bytes;(void)n;return 0;}
static int OracleOutputBegin(const RustCompilerOutput *d,RustCompilerOutputResult *out){Trace(RUST_COMPILER_OUTPUT_OPEN);strcpy(current->source,d->source);strcpy(current->destination,d->destination);TEST_ASSERT(d->width==N_PATH_BUF&&d->version==123&&d->rate==22050);if(current->fail==6){snprintf((char *)out->filename,sizeof(out->filename),"%s/phonindex",d->destination);return 13;}out->owner=3;out->mask=31;return 0;}
static int OracleOutputEnd(uint64_t owner){TEST_ASSERT(owner==3);Trace(RUST_COMPILER_OUTPUT_CLOSE);return current->output_status;}
static void StartPhonemeTable(CompileContext *c,const char *name){TEST_ASSERT(strcmp(name,"base")==0);Trace(RUST_COMPILER_BASE_TABLE);c->n_phoneme_tabs++;}
static void CompilePhonemeFiles(CompileContext *c){Trace(RUST_COMPILER_COMPILE);c->count_references=current->references;c->duplicate_references=current->reused;c->error_count=current->errors;c->table_status=current->table_status;c->program_status=current->program_status;}
static void EndPhonemeTable(CompileContext *c){(void)c;Trace(RUST_COMPILER_END_TABLE);}
static void WritePhonemeTables(CompileContext *c){(void)c;Trace(RUST_COMPILER_WRITE_TABLES);}
static void error_from_status(CompileContext *c,int status,const char *name){TEST_ASSERT(status!=0&&strcmp(name,"compiler output")==0);Trace(RUST_COMPILER_OUTPUT_ERROR);c->error_count++;fprintf(c->f_errors,"output error %d\n",status);}
static espeak_ng_STATUS create_file_error_context(espeak_ng_ERROR_CONTEXT *ctx,int status,const char *path){(void)ctx;current->error_status=status;strcpy(current->error_path,path);return status;}
#define fclose IgnoreClose
#define espeak_ng_CompilePhonemeDataPath OracleCompile
#define calloc OracleCalloc
#define espeak_rs_compiler_tables_begin OracleTablesBegin
#define espeak_rs_compiler_program_begin OracleProgramBegin
#define espeak_rs_compiler_output_begin OracleOutputBegin
#define espeak_rs_compiler_output_end OracleOutputEnd
#include "compiler_driver_reference.inc"
#include "compiler_driver_primitives.inc"
#undef fclose
#undef espeak_ng_CompilePhonemeDataPath
#undef calloc
#undef espeak_rs_compiler_tables_begin
#undef espeak_rs_compiler_program_begin
#undef espeak_rs_compiler_output_begin
#undef espeak_rs_compiler_output_end
static int Prefix(void){unsigned value=0;char tail=0;sscanf("0x g","%x %c",&value,&tail);return tail=='x';}
static unsigned comparisons;
static void ReadLog(FILE *f,Plan *p){TEST_ASSERT(fflush(f)==0);long n=ftell(f);TEST_ASSERT(n>=0&&n<(long)sizeof(p->diagnostics));rewind(f);p->diagnostic_len=fread(p->diagnostics,1,(size_t)n,f);TEST_ASSERT(p->diagnostic_len==(size_t)n);}
static void Maps(unsigned char a[256],unsigned char s[256]){for(int i=0;i<256;i++){a[i]=isalpha(i)!=0;s[i]=isspace(i)!=0;}}
static void Pair(unsigned trial)
{
 Plan a={0},b={0};a.fail=b.fail=trial%8;a.errors=b.errors=trial%3;a.references=b.references=trial*17;a.reused=b.reused=trial*3;
 a.output_status=b.output_status=trial%11==0?28:0;a.table_status=b.table_status=trial%7==0?22:0;a.program_status=b.program_status=trial%5==0?11:0;
 unsigned char alpha[256],spaces[256];Maps(alpha,spaces);
 const char *src=trial%3==0?NULL:"/custom/source",*dst=trial%4==0?NULL:"/custom/destination";
 /* Driver closes owned logs; standard stdout is the borrowed projection. */
 FILE *fa=tmpfile(),*fb=tmpfile();TEST_ASSERT(fa&&fb);
 /* Track fclose while keeping the file readable for exact diagnostics. */
 current=&a;retained_log=fa;espeak_ng_STATUS sa=OracleCompile(22050,src,dst,fa,NULL);
 ReadLog(fa,&a);
 PhonemeEffects effects={NULL,22050,fb,NULL};RustPhonemeCompiler d={&effects,path_home,src,dst,alpha,spaces,N_PATH_BUF,0,Prefix(),PhonemeEffect,PhonemeCounts,PhonemeLog,PhonemeError};
 current=&b;retained_log=fb;int sb=espeak_rs_compile_phonemes(&d);ReadLog(fb,&b);
 TEST_ASSERT((int)sa==sb);TEST_ASSERT(a.count==b.count&&memcmp(a.trace,b.trace,a.count*sizeof(unsigned))==0);
 TEST_ASSERT(a.cleaned==b.cleaned&&a.error_status==b.error_status&&strcmp(a.error_path,b.error_path)==0);
 TEST_ASSERT(strcmp(a.source,b.source)==0&&strcmp(a.destination,b.destination)==0&&strcmp(a.input,b.input)==0);
 TEST_ASSERT(a.diagnostic_len==b.diagnostic_len&&memcmp(a.diagnostics,b.diagnostics,a.diagnostic_len)==0);
 fclose(fa);fclose(fb);comparisons+=5;
}


static char manifest_path[4096];
static void WriteManifest(const unsigned char *bytes,size_t length)
{FILE *f=fopen(manifest_path,"wb");TEST_ASSERT(f);TEST_ASSERT(fwrite(bytes,1,length,f)==length);TEST_ASSERT(fclose(f)==0);}
static void ManifestPair(unsigned trial)
{
 unsigned char bytes[16000],alpha[256],spaces[256];Maps(alpha,spaces);size_t length=0;
 const char *hex[]={"0","ff","0x12","-1","+AB","0x","0Xg","bad","100g","not-a-number","429","-FFFFFFFF"};
 if(trial%7){length+=(size_t)snprintf((char *)bytes+length,sizeof(bytes)-length,"# manifest %u\n",trial);}
 for(unsigned i=0;i<trial%47;i++){
  unsigned n=(trial+i)%117+1;char name[120];memset(name,'a'+i%26,n);name[n]=0;
  length+=(size_t)snprintf((char *)bytes+length,sizeof(bytes)-length,"%c %s\t%s extra\n",i%7==0?'#':(i%2?'W':'E'),hex[(trial+i)%12],name);
 }
 TEST_ASSERT(length<sizeof(bytes));WriteManifest(bytes,length);
 Plan plan={0};current=&plan;retained_log=NULL;CompileContext c={0};
 int sa=ReadPhondataManifest(&c,NULL);RustManifestRow rows[128];memset(rows,0x55,sizeof(rows));size_t count=555;
 RustManifest d={bytes,length,N_PATH_BUF,0,Prefix(),alpha,spaces};int sb=espeak_rs_compiler_manifest_parse(&d,rows,128,&count);
 TEST_ASSERT(sa==sb&&count==(size_t)c.n_manifest);
 for(size_t i=0;i<count;i++){if(rows[i].value!=c.manifest[i].value||strcmp((char *)rows[i].name,c.manifest[i].name)!=0){fprintf(stderr,"manifest trial %u row %zu: C=%x '%s', Rust=%x '%s'\n",trial,i,c.manifest[i].value,c.manifest[i].name,rows[i].value,rows[i].name);}TEST_ASSERT(rows[i].value==c.manifest[i].value&&strcmp((char *)rows[i].name,c.manifest[i].name)==0);}
 for(int i=0;i<c.n_manifest;i++){free(c.manifest[i].name);}free(c.manifest);comparisons+=2+(unsigned)count;
}
static RustPhonemeCompiler *guard_descriptor;static unsigned char *guard_alpha;static char *guard_source;
static int GuardEffect(void *ctx,int op,int value,const char *source,const char *destination,unsigned char *path)
{
 if(op==RUST_COMPILER_ALLOCATE){TEST_ASSERT(espeak_rs_compile_phonemes(guard_descriptor)==11);guard_descriptor->effect=NULL;guard_source[0]='X';memset(guard_alpha,0,256);}
 return PhonemeEffect(ctx,op,value,source,destination,path);
}
static void Guards(void)
{
 Plan plan={0};current=&plan;FILE *f=tmpfile();TEST_ASSERT(f);retained_log=f;
 unsigned char alpha[256],spaces[256];Maps(alpha,spaces);char source[N_PATH_BUF]="/snapshotted/source";
 PhonemeEffects effects={NULL,22050,f,NULL};RustPhonemeCompiler d={&effects,path_home,source,NULL,alpha,spaces,N_PATH_BUF,0,Prefix(),GuardEffect,PhonemeCounts,PhonemeLog,PhonemeError};
 guard_descriptor=&d;guard_alpha=alpha;guard_source=source;
 TEST_ASSERT(espeak_rs_compile_phonemes(&d)==0&&strcmp(plan.source,"/snapshotted/source")==0&&plan.cleaned==1);
 unsigned events=plan.count;d.effect=PhonemeEffect;d.width=1;TEST_ASSERT(espeak_rs_compile_phonemes(&d)==22&&plan.count==events);
 d.width=N_PATH_BUF;memset(source,'x',sizeof(source));TEST_ASSERT(espeak_rs_compile_phonemes(&d)==ENAMETOOLONG&&plan.count==events);
 TEST_ASSERT(espeak_rs_compile_phonemes(NULL)==22);fclose(f);Maps(alpha,spaces);
 union {unsigned char bytes[124];RustManifestRow row;} overlap;memset(&overlap,0x55,sizeof(overlap));memcpy(overlap.bytes,"W f name\n",9);
 RustManifest m={overlap.bytes,9,N_PATH_BUF,0,Prefix(),alpha,spaces};size_t used=555;
 TEST_ASSERT(espeak_rs_compiler_manifest_parse(&m,&overlap.row,1,&used)==0&&used==1&&overlap.row.value==15&&strcmp((char *)overlap.row.name,"name")==0);
 const unsigned char valid[]="W 1 a\n";m.bytes=valid;m.length=sizeof(valid)-1;used=555;
 TEST_ASSERT(espeak_rs_compiler_manifest_parse(&m,NULL,0,&used)==28&&used==555);
 m.length=16*1024*1024+1;TEST_ASSERT(espeak_rs_compiler_manifest_parse(&m,NULL,0,&used)==22&&used==555);
 m.bytes=(const unsigned char *)"W 100000000 name\n";m.length=strlen((const char *)m.bytes);TEST_ASSERT(espeak_rs_compiler_manifest_parse(&m,&overlap.row,1,&used)==22&&used==555);
 unsigned char large[125];memcpy(large,"W 1 ",4);memset(large+4,'x',120);large[124]='\n';m.bytes=large;m.length=125;TEST_ASSERT(espeak_rs_compiler_manifest_parse(&m,&overlap.row,1,&used)==22&&used==555);
 m.bytes=(const unsigned char *)"W 1 a\r\nW 2 b\r\n\x1aW 3 c\n";m.length=strlen((const char *)m.bytes);m.windows=1;RustManifestRow rows[3];
 TEST_ASSERT(espeak_rs_compiler_manifest_parse(&m,rows,3,&used)==0&&used==2&&rows[1].value==2);
}
int main(void)
{
 snprintf(path_home,sizeof(path_home),"%s/espeak-compiler-driver-%ld",getenv("TMPDIR")?getenv("TMPDIR"):"/tmp",(long)process_id());
 TEST_ASSERT(mkdir_path(path_home)==0);snprintf(manifest_path,sizeof(manifest_path),"%s/phondata-manifest",path_home);
 for(unsigned i=0;i<2000;i++){
  if(i%4==0){WriteManifest((const unsigned char *)"",0);}else if(i%4==1){remove(manifest_path);}else{WriteManifest((const unsigned char *)"W ff name\n",10);}
  Pair(i);
 }
 for(unsigned i=0;i<10000;i++){ManifestPair(i);}WriteManifest((const unsigned char *)"W ff name\n",10);Guards();
 TEST_ASSERT(remove(manifest_path)==0&&rmdir_path(path_home)==0);
 printf("2000 frozen complete compiler-driver episodes and 10000 C manifest episodes: %u comparisons; actual primitive extraction and snapshot/reentry/bounds/overlap guards passed\n",comparisons);
 return 0;
}
