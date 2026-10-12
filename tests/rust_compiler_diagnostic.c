/* Complete previous-C/native compiler diagnostics with actual native contexts.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include <espeak-ng/espeak_ng.h>
#include "test_assert.h"
#include "rust_compiler_diagnostic.h"
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
typedef RustCompilerContext CompileContext;
#define error PreviousError
#define error_from_status PreviousStatus
#define TableError PreviousTable
#define ResourceError PreviousResource
#define ItemError PreviousItem
#define ProgramError PreviousProgram
#include "compiler_diagnostic_reference.inc"
#undef error
#undef error_from_status
#undef TableError
#undef ResourceError
#undef ItemError
#undef ProgramError
#include "compiler_diagnostic_compiler_diagnostic_projections.inc"
#include "compiler_diagnostic_table_diagnostic_projection.inc"
#include "compiler_diagnostic_item_diagnostic_projection.inc"
#include "compiler_diagnostic_program_diagnostic_projection.inc"
#include "compiler_diagnostic_resource_diagnostic_projection.inc"
_Static_assert(sizeof(RustCompilerDiagnosticStatus)==(sizeof(void *)==8?40:20),"diagnostic ABI");
static unsigned pairs,callbacks;
static CompileContext *contexts[2];
static FILE *files[2];
static unsigned char before_bytes[12000],after_bytes[12000],name[4096];
static uint32_t random_state=0x19a54421;
static uint32_t Next(void){random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state;}
static size_t Read(FILE *file,unsigned char *bytes)
{
 TEST_ASSERT(fflush(file)==0&&fseek(file,0,SEEK_SET)==0);
 size_t length=fread(bytes,1,12000,file);TEST_ASSERT(feof(file));return length;
}
static void Reset(FILE *file)
{ clearerr(file);TEST_ASSERT(fflush(file)==0&&ftruncate(fileno(file),0)==0&&fseek(file,0,SEEK_SET)==0); }
static void Pair(unsigned trial)
{
 size_t length;
 switch(trial%7){case 0:length=0;break;case 1:length=1;break;case 2:length=255;break;case 3:length=256;break;case 4:length=511;break;case 5:length=4095;break;default:length=4096;}
 for(size_t j=0;j<length;j++){name[j]=(unsigned char)(Next()%255+1);}
 if(length&&trial%3==0){name[length/2]=0;}
 unsigned family=trial%5;int code=(trial/5)%12,value=(int32_t)Next(),a=(int32_t)Next(),b=(int32_t)Next();
 int line=trial%3?(int)(trial%31)-17:INT_MIN+1;
 int count=(int)(trial%71);
 static const int statuses[]={0,2,EINVAL,ENS_UNKNOWN_PHONEME_FEATURE,ENS_VOICE_NOT_FOUND,0x12345};
 int status=statuses[trial%6];char terminated[4097];memcpy(terminated,name,length);terminated[length<4096?length:4095]=0;
 const char *context=trial%3?terminated:NULL;
 for(int lane=0;lane<2;lane++) {
  CompileContext *c=contexts[lane];Reset(files[lane]);c->linenum=line;c->error_count=count;
  memset(c->current_fname,0,80);memcpy(c->current_fname,"source-\xff-file",13);
  if(!lane) {
   switch(family){case 0:PreviousProgram(c,name,length);break;case 1:PreviousItem(c,line,name,length);break;case 2:PreviousTable(c,code,value,name,length);break;case 3:PreviousResource(c,code,a,b,name,length);break;default:PreviousStatus(c,status,context);}
  } else {
   switch(family){case 0:ProgramError(c,name,length);break;case 1:ItemError(c,line,name,length);break;case 2:TableError(c,code,value,name,length);break;case 3:ResourceError(c,code,a,b,name,length);break;default:error_from_status(c,status,context);}
  }
  TEST_ASSERT(fcntl(fileno(files[lane]),F_GETFD)!=-1);
 }
 size_t length0=Read(files[0],before_bytes),length1=Read(files[1],after_bytes);
 TEST_ASSERT(length0==length1&&memcmp(before_bytes,after_bytes,length0)==0);
 TEST_ASSERT(contexts[0]->linenum==contexts[1]->linenum&&contexts[0]->error_count==contexts[1]->error_count);
 pairs++;
}
static RustCompilerDiagnosticStatus *mutable_status;
static char *mutable_name;
static void Classes(unsigned char *spaces,unsigned char *digits)
{ (void)spaces;(void)digits;TEST_ASSERT(0); }
static void ScopedMessage(int status,char *bytes,size_t length)
{
 CompileContext *c=contexts[1];TEST_ASSERT(status==123&&length==512);
 TEST_ASSERT(espeak_rs_compiler_diagnostic_text(c,(const unsigned char *)"nested",6)==EAGAIN);
 TEST_ASSERT(espeak_rs_compiler_context_end(c)==EAGAIN);
 /* Source admission refuses before its unused classifier primitive. */
 RustCompilerSourceOpen source={c,".",2,0,0,Classes};int error=0;
 TEST_ASSERT(espeak_rs_compiler_context_source(&source,&error)==0&&error==EAGAIN);
 strcpy(c->current_fname,"fresh");c->linenum=9;c->error_count=11;
 mutable_name[0]='X';mutable_status->context=NULL;mutable_status->message=NULL;
 strcpy(bytes,"scoped status");callbacks++;
}
static void Unterminated(int status,char *bytes,size_t length)
{ (void)status;memset(bytes,'x',length); }
static void *WrongThread(void *unused)
{ (void)unused;TEST_ASSERT(espeak_rs_compiler_diagnostic_text(contexts[1],name,0)==EAGAIN);return NULL; }
static void Guards(void)
{
 CompileContext *c=contexts[1];Reset(files[1]);c->error_count=1;c->linenum=1;strcpy(c->current_fname,"before");
 char path[16]="selected";RustCompilerDiagnosticStatus d={c,123,path,sizeof(path),ScopedMessage};
 mutable_name=path;mutable_status=&d;TEST_ASSERT(espeak_rs_compiler_diagnostic_status(&d)==0&&c->error_count==12&&c->linenum==9);
 size_t n=Read(files[1],after_bytes);const char *expected="fresh(8): scoped status: 'selected'.\n";
 TEST_ASSERT(n==strlen(expected)&&memcmp(after_bytes,expected,n)==0);
 Reset(files[1]);c->error_count=0;c->linenum=1;
 TEST_ASSERT(espeak_rs_compiler_diagnostic_text(NULL,name,0)==EINVAL);
 TEST_ASSERT(espeak_rs_compiler_diagnostic_text(c,NULL,1)==EINVAL);
 TEST_ASSERT(espeak_rs_compiler_diagnostic_text(c,name,4097)==EINVAL);
 TEST_ASSERT(espeak_rs_compiler_diagnostic_item(c,INT_MIN,name,0)==EINVAL&&c->linenum==1);
 c->error_count=INT_MAX;TEST_ASSERT(espeak_rs_compiler_diagnostic_text(c,name,0)==EINVAL&&c->error_count==INT_MAX);
 c->error_count=INT_MAX-1;espeak_rs_compiler_diagnostic_table(c,4,1,name,0);TEST_ASSERT(c->error_count==INT_MAX-1);
 c->error_count=0;memset(c->current_fname,'x',80);TEST_ASSERT(espeak_rs_compiler_diagnostic_text(c,name,0)==EINVAL);
 strcpy(c->current_fname,"fresh");d=(RustCompilerDiagnosticStatus){c,0,NULL,0,NULL};
 TEST_ASSERT(espeak_rs_compiler_diagnostic_status(NULL)==EINVAL&&espeak_rs_compiler_diagnostic_status(&d)==EINVAL);
 d.message=Unterminated;TEST_ASSERT(espeak_rs_compiler_diagnostic_status(&d)==EINVAL);
 d.message=CompilerStatusMessage;d.name=(char *)name;d.width=4096;memset(name,'x',4096);
 TEST_ASSERT(espeak_rs_compiler_diagnostic_status(&d)==EINVAL);
 TEST_ASSERT(Read(files[1],after_bytes)==0&&c->error_count==0);
 pthread_t thread;TEST_ASSERT(pthread_create(&thread,NULL,WrongThread,NULL)==0&&pthread_join(thread,NULL)==0);
 /* Copy the diagnostic byte loan before overwriting the item line field. */
 memcpy(&c->linenum,"ABCD",4);TEST_ASSERT(espeak_rs_compiler_diagnostic_item(c,7,(unsigned char *)&c->linenum,4)==0);
 n=Read(files[1],after_bytes);expected="fresh(6): ABCD\n";TEST_ASSERT(n==strlen(expected)&&memcmp(after_bytes,expected,n)==0);
 Reset(files[1]);FILE *saved=c->f_errors;c->f_errors=NULL;
 TEST_ASSERT(espeak_rs_compiler_diagnostic_item(c,8,name,0)==EINVAL&&c->linenum==7);c->f_errors=saved;
 /* Both hosts ignore a diagnostic write failure and leave the FILE borrowed. */
 FILE *read_only=fopen("compiler-diagnostic-readonly.tmp","wb");TEST_ASSERT(read_only);fclose(read_only);
 for(int lane=0;lane<2;lane++) {
  read_only=fopen("compiler-diagnostic-readonly.tmp","rb");TEST_ASSERT(read_only);
  contexts[lane]->f_errors=read_only;contexts[lane]->error_count=10;contexts[lane]->linenum=1;strcpy(contexts[lane]->current_fname,"fresh");
  if(lane){ProgramError(contexts[lane],(unsigned char *)"text",4);}else{PreviousProgram(contexts[lane],(unsigned char *)"text",4);}
  TEST_ASSERT(ferror(read_only)&&contexts[lane]->error_count==11&&fcntl(fileno(read_only),F_GETFD)!=-1);
  fclose(read_only);contexts[lane]->f_errors=files[lane];
 }
 TEST_ASSERT(unlink("compiler-diagnostic-readonly.tmp")==0);
}
int main(void)
{
 for(int i=0;i<2;i++){int error=-1;contexts[i]=espeak_rs_compiler_context_begin(".",2,&error);files[i]=tmpfile();TEST_ASSERT(contexts[i]&&files[i]&&error==0);contexts[i]->f_errors=files[i];}
 for(unsigned trial=0;trial<5000;trial++){Pair(trial);}Guards();
 for(int i=0;i<2;i++){TEST_ASSERT(espeak_rs_compiler_context_end(contexts[i])==0&&fcntl(fileno(files[i]),F_GETFD)!=-1);fclose(files[i]);}
 printf("%u complete previous-C/native diagnostic pairs; %u scoped status callbacks; admission/count/line/overlap/thread/reentry/borrowed-FILE guards pass\n",pairs,callbacks);return 0;
}
