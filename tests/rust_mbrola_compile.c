/* Whole unchanged C compiler versus the native Rust public entry point. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "speech.h"
#include "error.h"
#include <espeak-ng/espeak_ng.h>
#include <dirent.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/resource.h>
#include <signal.h>
#include <unistd.h>

extern espeak_ng_STATUS previous_CompileMbrolaVoice(const char *,FILE *,espeak_ng_ERROR_CONTEXT *);
static unsigned cases;
#ifdef __APPLE__
static const char *nested_source;
static unsigned nested_calls;
static int nested_write(void *cookie,const char *bytes,int length) {
 (void)cookie;(void)bytes;
 TEST_ASSERT(espeak_ng_CompileMbrolaVoice(nested_source,NULL,NULL)==EAGAIN);
 nested_calls++;return length;
}
#endif
static void bytes_equal(FILE *a,FILE *b) {
 rewind(a); rewind(b);
 for (;;) { int x=fgetc(a),y=fgetc(b); TEST_ASSERT(x==y); if(x==EOF)break; }
}
static void compare(const char *source,const char *output) {
 FILE *a=tmpfile(),*b=tmpfile(),*copy=tmpfile(); TEST_ASSERT(a&&b&&copy);
 espeak_ng_ERROR_CONTEXT ea=NULL,eb=NULL;
 int old=previous_CompileMbrolaVoice(source,a,&ea);
 if(old==0) {
  FILE *f=fopen(output,"rb"); TEST_ASSERT(f); int c; while((c=fgetc(f))!=EOF)fputc(c,copy); fclose(f);
 }
 int actual=espeak_ng_CompileMbrolaVoice(source,b,&eb);
 TEST_ASSERT(actual==old); bytes_equal(a,b);
 TEST_ASSERT((ea==NULL)==(eb==NULL));
 if(ea) { TEST_ASSERT(ea->type==eb->type); TEST_ASSERT(strcmp(ea->name,eb->name)==0); }
 if(old==0) { FILE *f=fopen(output,"rb"); TEST_ASSERT(f); bytes_equal(copy,f); fclose(f); TEST_ASSERT(unlink(output)==0); }
 espeak_ng_ClearErrorContext(&ea); espeak_ng_ClearErrorContext(&eb);
 fclose(a);fclose(b);fclose(copy); cases++;
}
static void write_source(const char *path,const void *data,size_t n) {
 FILE *f=fopen(path,"wb"); TEST_ASSERT(f); TEST_ASSERT(fwrite(data,1,n,f)==n); TEST_ASSERT(fclose(f)==0);
}
static void rejected(const char *source,const char *output) {
 FILE *log=tmpfile(); TEST_ASSERT(log); espeak_ng_ERROR_CONTEXT error=NULL;
 TEST_ASSERT(espeak_ng_CompileMbrolaVoice(source,log,&error)==EINVAL);
 TEST_ASSERT(error && strcmp(error->name,source)==0); TEST_ASSERT(ftell(log)==0);
 TEST_ASSERT(access(output,F_OK)!=0); espeak_ng_ClearErrorContext(&error); fclose(log);
}
int main(void) {
 char saved[N_PATH_BUF],root[]="/tmp/espeak-mbrcompile-XXXXXX",directory[N_PATH_BUF];
 strcpy(saved,path_home);TEST_ASSERT(mkdtemp(root));strcpy(path_home,root);
 snprintf(directory,sizeof(directory),"%s/mbrola_ph",root);TEST_ASSERT(mkdir(directory,0700)==0);
 DIR *dir=opendir(REPO_SOURCE_DIR "/phsource/mbrola"); TEST_ASSERT(dir); struct dirent *entry;
 while((entry=readdir(dir))) {
  if(entry->d_name[0]=='.')continue;
  char source[N_PATH_BUF],output[N_PATH_BUF];
  snprintf(source,sizeof(source),"%s/phsource/mbrola/%s",REPO_SOURCE_DIR,entry->d_name);
  struct stat st;TEST_ASSERT(stat(source,&st)==0);if(!S_ISREG(st.st_mode))continue;
  snprintf(output,sizeof(output),"%s/%s_phtrans",directory,entry->d_name);compare(source,output);
 }
 closedir(dir); unsigned corpus=cases;
 char source[N_PATH_BUF],output[N_PATH_BUF];snprintf(source,sizeof(source),"%s/input",root);snprintf(output,sizeof(output),"%s/input_phtrans",directory);
 const char *inputs[]={"", "volume\n", "volume-2147483648\n", "volume+2147483647junk\n", " volume40\n0 a NULL 1 b\n", "0 a VWL 20foo bar extra\n", "1 abcd NULL -4 NULL NULL\n", "0 a NULL 20 b //tail\nvolume 32//comment\n", "0 a NULL 3 b\r\n0 c VWL 7 d", "+ a NULL 0 b\n0 a NULL - b\n0 a NULL +1 b", "0 abcd\0ignored NULL 0 q\n", "0 a NULL 0 b\v\f1 c NULL 0 d\n"};
 for(size_t i=0;i<sizeof(inputs)/sizeof(inputs[0]);i++){write_source(source,inputs[i],strlen(inputs[i]));compare(source,output);}
 const unsigned char embedded_nul[]="0 a NULL 0 b\0trailing\n0 c NULL 0 d\n";
 write_source(source,embedded_nul,sizeof(embedded_nul)-1);compare(source,output);
 const char *vertical="volume\v31\n0\va NULL 50 b\n";
 write_source(source,vertical,strlen(vertical));compare(source,output);
 for(unsigned spaces=0;spaces<120;spaces++) {
  char bytes[512];memset(bytes,' ',spaces);snprintf(bytes+spaces,sizeof(bytes)-spaces,"0 a NULL 10 b\nvolume 25\n2 x VWL -12 y z\n");
  write_source(source,bytes,strlen(bytes));compare(source,output);
 }
 for(unsigned rows=1;rows<=255;rows+=7) {
  FILE *f=fopen(source,"wb");TEST_ASSERT(f);
  for(unsigned i=0;i<rows;i++)fprintf(f,"%u %c NULL %d %c %c\n",i%127,'a'+i%26,(int)i-100,'A'+i%26,'0'+i%10);
  fclose(f);compare(source,output);
 }
 FILE *f=fopen(source,"wb");TEST_ASSERT(f);for(unsigned i=0;i<256;i++)fputs("0 a NULL 0 b\n",f);fclose(f);rejected(source,output);
 write_source(source,"volume2147483648",16);rejected(source,output);
 write_source(source,"0 a NULL 0 b\n",13);
#ifdef __APPLE__
 nested_source=source;
 FILE *nested=funopen(NULL,NULL,nested_write,NULL,NULL);TEST_ASSERT(nested);setbuf(nested,NULL);
 TEST_ASSERT(espeak_ng_CompileMbrolaVoice(source,nested,NULL)==ENS_OK);fclose(nested);
 TEST_ASSERT(nested_calls>0);TEST_ASSERT(unlink(output)==0);
#endif
 // Real completion write failure: restore the process limit/signal afterward.
 struct rlimit original,limited;TEST_ASSERT(getrlimit(RLIMIT_FSIZE,&original)==0);limited=original;limited.rlim_cur=0;
 void (*old_signal)(int)=signal(SIGXFSZ,SIG_IGN);TEST_ASSERT(old_signal!=SIG_ERR);
 FILE *failed_log=tmpfile();TEST_ASSERT(failed_log);espeak_ng_ERROR_CONTEXT failed_context=NULL;
 TEST_ASSERT(setrlimit(RLIMIT_FSIZE,&limited)==0);
 int failed_status=espeak_ng_CompileMbrolaVoice(source,failed_log,&failed_context);
 TEST_ASSERT(setrlimit(RLIMIT_FSIZE,&original)==0);TEST_ASSERT(signal(SIGXFSZ,old_signal)!=SIG_ERR);
 TEST_ASSERT(failed_status==EFBIG);TEST_ASSERT(failed_context&&strcmp(failed_context->name,output)==0);
 TEST_ASSERT(ftell(failed_log)==0);fclose(failed_log);espeak_ng_ClearErrorContext(&failed_context);TEST_ASSERT(unlink(output)==0);
 TEST_ASSERT(unlink(source)==0);compare(source,output); // missing input/error context
 write_source(source,"0 a NULL 0 b\n",13);TEST_ASSERT(rmdir(directory)==0);compare(source,output); // missing output parent
 TEST_ASSERT(mkdir(directory,0700)==0);TEST_ASSERT(mkdir(output,0700)==0);compare(source,output); // output directory
 TEST_ASSERT(rmdir(output)==0);TEST_ASSERT(unlink(source)==0);TEST_ASSERT(rmdir(directory)==0);TEST_ASSERT(rmdir(root)==0);
 strcpy(path_home,saved);
 printf("Matched %u complete compiler cases (%u corpus files), logs, outputs and file error contexts; bounded overflow refusals passed\n",cases,corpus);
 return 0;
}
