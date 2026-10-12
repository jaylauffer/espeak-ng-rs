/* Frozen complete native C reporters vs Rust via frozen generic adapters.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "test_assert.h"
#include "common.h"
#include "translate.h"
#include "rust_dictionary_diagnostic.h"
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
typedef struct {FILE *f_log;int linenum,error_count;char group_name[13];} CompileContext;
typedef struct {CompileContext *ctx;void *output;} DictListIo;
typedef struct {CompileContext *compiler;void *output;} RuleFileIo;
typedef struct {CompileContext *compiler;void *error;} DictionaryIo;
static FILE *host_error;
#pragma push_macro("stderr")
#undef stderr
#define stderr host_error
#define list_report PreviousList
#define rule_report PreviousRule
#define rule_file_report PreviousFile
#define dictionary_log PreviousDriver
#include "dictionary_diagnostic_reference.inc"
#undef list_report
#undef rule_report
#undef rule_file_report
#undef dictionary_log
#include "native_dictionary_diagnostics.inc"
#undef stderr
#pragma pop_macro("stderr")
_Static_assert(sizeof(RustDictionaryDiagnostic)==(sizeof(void *)==8?48:24),"diagnostic ABI");
static unsigned pairs,callbacks;
static uint32_t random_state=0xe591abb2;
static uint32_t Next(void){random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state;}
static void Reset(FILE *f){clearerr(f);TEST_ASSERT(fflush(f)==0&&ftruncate(fileno(f),0)==0&&fseek(f,0,SEEK_SET)==0);}
static size_t Read(FILE *f,unsigned char *bytes){TEST_ASSERT(fflush(f)==0&&fseek(f,0,SEEK_SET)==0);size_t n=fread(bytes,1,20000,f);TEST_ASSERT(feof(f));return n;}
static CompileContext contexts[2];static FILE *logs[2],*errors[2];
static char first[4096],second[4096];static unsigned char group[13];
static void Pair(unsigned trial)
{
 unsigned family=trial%4;int kind=(int)((trial/4)%26)-2;
 int line=trial%3?(int)(trial%61)-30:INT_MIN,value=(int32_t)Next();
 if(trial%9==0)value=0;if(trial%9==1)value=-1;if(trial%9==2)value=3000;
 size_t n=(trial%7==0)?4095:trial%251;
 for(size_t j=0;j<n;j++){first[j]=(char)(Next()%255+1);second[j]=(char)(Next()%255+1);}first[n]=second[n]=0;
 for(unsigned j=0;j<13;j++)group[j]=(unsigned char)(Next()%255+1);group[trial%13]=0;
 for(int lane=0;lane<2;lane++){
  Reset(logs[lane]);Reset(errors[lane]);CompileContext *c=&contexts[lane];c->f_log=logs[lane];c->linenum=line;c->error_count=(int)(trial%13);
  memset(c->group_name,0x51,13);memcpy(c->group_name,"prev",5);host_error=errors[lane];
  DictListIo list={c,NULL};RuleFileIo file={c,NULL};DictionaryIo driver={c,NULL};
  if(!lane){switch(family){case 0:PreviousList(&list,line+1,kind,value,first,second);break;case 1:PreviousRule(c,kind,value,first);break;case 2:PreviousFile(&file,line+1,group,kind,value,first);break;default:PreviousDriver(&driver,kind,first);}}
  else {switch(family){case 0:list_report(&list,line+1,kind,value,first,second);break;case 1:rule_report(c,kind,value,first);break;case 2:rule_file_report(&file,line+1,group,kind,value,first);break;default:dictionary_log(&driver,kind,first);}}
 }
 unsigned char a[20000],b[20000];size_t na=Read(logs[0],a),nb=Read(logs[1],b);
 if(na!=nb||memcmp(a,b,na)){fprintf(stderr,"dictionary diagnostic trial %u family %u kind %d mismatch\n",trial,family,kind);TEST_ASSERT(0);}
 na=Read(errors[0],a);nb=Read(errors[1],b);TEST_ASSERT(na==nb&&memcmp(a,b,na)==0);
 TEST_ASSERT(contexts[0].linenum==contexts[1].linenum&&contexts[0].error_count==contexts[1].error_count&&memcmp(contexts[0].group_name,contexts[1].group_name,13)==0);
 TEST_ASSERT(fcntl(fileno(logs[1]),F_GETFD)!=-1&&fcntl(fileno(errors[1]),F_GETFD)!=-1);pairs++;
}
static RustDictionaryDiagnostic Descriptor(CompileContext *c)
{RustDictionaryDiagnostic d={c->f_log,errors[1],&c->linenum,&c->error_count,(unsigned char *)c->group_name,13};return d;}
#if defined(__APPLE__)
static RustDictionaryDiagnostic *mutated;static unsigned char captured[20000];static size_t captured_length;
static int GuardWrite(void *cookie,const char *bytes,int length)
{
 CompileContext *c=cookie;TEST_ASSERT(length>=0&&captured_length+(size_t)length<sizeof(captured));
 memcpy(captured+captured_length,bytes,(size_t)length);captured_length+=(size_t)length;
 if(!callbacks){RustDictionaryDiagnostic copy=*mutated;
  TEST_ASSERT(c->linenum==41&&!strcmp(c->group_name,"new"));
  TEST_ASSERT(espeak_rs_dictionary_diagnostic(&copy,RUST_DIAGNOSTIC_RULE,0,5,0,NULL,NULL,NULL)==EAGAIN);
  c->error_count=9;c->linenum=99;strcpy(c->group_name,"after");first[0]='X';group[0]='X';
  mutated->log=NULL;mutated->line=NULL;mutated->count=NULL;mutated->group=NULL;mutated->width=0;callbacks++;
 }return length;
}
#endif
static void Guards(void)
{
 CompileContext *c=&contexts[1];Reset(logs[1]);c->f_log=logs[1];c->linenum=7;c->error_count=3;strcpy(c->group_name,"before");
 RustDictionaryDiagnostic d=Descriptor(c),bad=d;
 TEST_ASSERT(espeak_rs_dictionary_diagnostic(NULL,0,0,1,0,NULL,NULL,NULL)==EINVAL);
 TEST_ASSERT(espeak_rs_dictionary_diagnostic(&d,4,0,1,0,NULL,NULL,NULL)==EINVAL);
 bad.count=NULL;TEST_ASSERT(espeak_rs_dictionary_diagnostic(&bad,0,0,2,0,NULL,NULL,NULL)==EINVAL);
 memset(first,'x',4096);TEST_ASSERT(espeak_rs_dictionary_diagnostic(&d,0,0,1,0,first,NULL,NULL)==EINVAL&&c->error_count==3);
 memset(group,'x',13);TEST_ASSERT(espeak_rs_dictionary_diagnostic(&d,2,91,15,0,NULL,NULL,group)==EINVAL&&c->linenum==7&&!strcmp(c->group_name,"before"));
 c->error_count=INT_MAX;TEST_ASSERT(espeak_rs_dictionary_diagnostic(&d,1,0,5,0,NULL,NULL,NULL)==EINVAL&&c->error_count==INT_MAX);
 c->error_count=3;strcpy(c->group_name,"old");strcpy((char *)group,"new");
 TEST_ASSERT(espeak_rs_dictionary_diagnostic(&d,2,41,8,0,c->group_name,NULL,group)==0&&c->linenum==41&&!strcmp(c->group_name,"new")&&c->error_count==4);
 unsigned char bytes[20000];size_t n=Read(logs[1],bytes);const char *expected="   41: Wrong initial letters 'old' for group 'new'\n";
 TEST_ASSERT(n==strlen(expected)&&memcmp(bytes,expected,n)==0);
 FILE *readonly=fopen("rust-dictionary-diagnostic-readonly.tmp","wb");TEST_ASSERT(readonly&&fclose(readonly)==0);readonly=fopen("rust-dictionary-diagnostic-readonly.tmp","rb");TEST_ASSERT(readonly);
 for(int lane=0;lane<2;lane++){clearerr(readonly);contexts[lane].f_log=readonly;contexts[lane].linenum=7;contexts[lane].error_count=3;if(!lane)PreviousRule(&contexts[lane],5,0,"");else {d=Descriptor(&contexts[lane]);TEST_ASSERT(espeak_rs_dictionary_diagnostic(&d,1,0,5,0,NULL,NULL,NULL)==0);}TEST_ASSERT(contexts[lane].error_count==4&&ferror(readonly));}
 TEST_ASSERT(fclose(readonly)==0&&unlink("rust-dictionary-diagnostic-readonly.tmp")==0);
#if defined(__APPLE__)
 c->f_log=funopen(c,NULL,GuardWrite,NULL,NULL);TEST_ASSERT(c->f_log&&setvbuf(c->f_log,NULL,_IONBF,0)==0);
 d=Descriptor(c);mutated=&d;strcpy(first,"copied");strcpy((char *)group,"new");
 TEST_ASSERT(espeak_rs_dictionary_diagnostic(&d,2,41,8,0,first,NULL,group)==0&&callbacks==1&&c->error_count==10&&c->linenum==99&&!strcmp(c->group_name,"after"));
 expected="   41: Wrong initial letters 'copied' for group 'new'\n";TEST_ASSERT(captured_length==strlen(expected)&&memcmp(captured,expected,captured_length)==0);
 TEST_ASSERT(fclose(c->f_log)==0);
#endif
}
int main(void)
{
 for(int lane=0;lane<2;lane++){logs[lane]=tmpfile();errors[lane]=tmpfile();TEST_ASSERT(logs[lane]&&errors[lane]);}
 for(unsigned i=0;i<12000;i++)Pair(i);Guards();
 for(int lane=0;lane<2;lane++)TEST_ASSERT(fclose(logs[lane])==0&&fclose(errors[lane])==0);
 printf("%u exact previous-C/Rust dictionary diagnostic pairs; list/rule/file/driver bytes, streams, line/group/count and borrowed FILE match; %u stream mutation/reentry callback; malformed/overflow/overlap/readonly guards pass\n",pairs,callbacks);return 0;
}
