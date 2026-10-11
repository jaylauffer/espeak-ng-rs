/* Actual top-level legacy driver vs the current library's native driver.
 * Sources, output/status/diagnostics and file-error contexts are compared.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "common.h"
#include "compiledict.h"
#include "dictionary.h"
#include "translate.h"
#include "speech.h"
#include "synthesize.h"
#include "error.h"
#include "rust_data.h"
#include <espeak-ng/speak_lib.h>
#include <ctype.h>
#include <errno.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <wctype.h>
_Static_assert(sizeof(RustDictionaryCompiler)==(sizeof(void *)==8?104:52),"native dictionary compiler ABI");
typedef struct {
    FILE *f_log;char word_phonemes[N_WORD_PHONEMES];
    int linenum,error_count;bool text_mode;int debug_flag,error_need_dictionary;
    char letterGroupsDefined[95];char group_name[13];int group3_ix;
} CompileContext;
static void clean_context(CompileContext *ctx) {free(ctx);}
static int FixtureWrite(void *file,const uint8_t *bytes,size_t length) {return fwrite(bytes,1,length,file)==length;}
static int FixtureTell(void *file,uint64_t *position) {long result=ftell(file);if(result<0)return 0;*position=result;return 1;}
#define espeak_rs_dictionary_write FixtureWrite
#define espeak_rs_dictionary_tell FixtureTell
#include "native_dict_list.inc"
#include "native_rule_compile.inc"
#include "native_rule_file.inc"
#undef espeak_rs_dictionary_write
#undef espeak_rs_dictionary_tell
#define espeak_ng_CompileDictionary ReferenceDriver
#include "dictionary_compile_reference.inc"
#undef espeak_ng_CompileDictionary
static unsigned compared;
static unsigned effects;
static int GuardCompile(void *context,const char *path,void *writer) {(void)context;(void)path;(void)writer;effects++;return 0;}
static void GuardLog(void *context,int kind,const char *text) {(void)context;(void)kind;(void)text;effects++;}
static void GuardReload(void *context,const char *name) {(void)context;(void)name;effects++;}
static int GuardErrors(void *context) {(void)context;effects++;return 0;}
static int GuardError(void *context,int status,const char *path) {(void)context;(void)path;effects++;return status;}
static void Guards(void)
{
    TEST_ASSERT(espeak_rs_compile_dictionary(NULL)==ENS_COMPILE_ERROR);
    RustDictionaryCompiler d={0};TEST_ASSERT(espeak_rs_compile_dictionary(&d)==ENS_COMPILE_ERROR);
    d=(RustDictionaryCompiler){NULL,"","en","","en",1,'/',GuardCompile,GuardCompile,GuardLog,GuardReload,GuardErrors,GuardError};
    TEST_ASSERT(espeak_rs_compile_dictionary(&d)==ENS_COMPILE_ERROR);
    d.path_width=4097;TEST_ASSERT(espeak_rs_compile_dictionary(&d)==ENS_COMPILE_ERROR);
    char unterminated[4096];memset(unterminated,'x',sizeof(unterminated));d.path_width=4096;d.name=unterminated;
    TEST_ASSERT(espeak_rs_compile_dictionary(&d)==ENS_COMPILE_ERROR);
    TEST_ASSERT(espeak_rs_dictionary_write(NULL,(const uint8_t *)"x",1)==0);
    uint64_t position=91;TEST_ASSERT(espeak_rs_dictionary_tell(NULL,&position)==0&&position==91);
    TEST_ASSERT(effects==0);
}
static uint32_t random_state=0x71618fff;
static uint32_t Next(void) {random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state;}
static void CompareStream(FILE *a,FILE *b,const char *kind)
{
    fflush(a);fflush(b);long x=ftell(a),y=ftell(b);TEST_ASSERT(x==y);rewind(a);rewind(b);
    for(long i=0;i<x;i++)if(fgetc(a)!=fgetc(b)) {fprintf(stderr,"%s mismatch at %ld\n",kind,i);abort();}
}
static void Compare(const char *prefix,const char *name,int flags)
{
    FILE *a=tmpfile(),*b=tmpfile();TEST_ASSERT(a&&b);
    espeak_ng_ERROR_CONTEXT error_a=NULL,error_b=NULL;
    char resolved[80];snprintf(resolved,sizeof(resolved),"%s",name?name:dictionary_name);
    char output[N_PATH_BUF];snprintf(output,sizeof(output),"%s/%s_dict",path_home,resolved);
    espeak_ng_STATUS x=ReferenceDriver(prefix,name,a,flags,&error_a);
    FILE *expected=tmpfile(),*file=fopen(output,"rb");TEST_ASSERT(expected);
    bool exists_a=file!=NULL;
    if(file) {int ch;while((ch=fgetc(file))!=EOF)fputc(ch,expected);fclose(file);}
    espeak_ng_STATUS y=espeak_ng_CompileDictionary(prefix,name,b,flags,&error_b);
    if(x!=y) {fprintf(stderr,"driver status C=%x Rust=%x flags=%d\n",x,y,flags);abort();}
    TEST_ASSERT((error_a==NULL)==(error_b==NULL));
    if(error_a) {TEST_ASSERT(error_a->type==error_b->type&&strcmp(error_a->name,error_b->name)==0);}
    CompareStream(a,b,"diagnostics");file=fopen(output,"rb");TEST_ASSERT(exists_a==(file!=NULL));
    if(file) {FILE *actual=tmpfile();TEST_ASSERT(actual);int ch;while((ch=fgetc(file))!=EOF)fputc(ch,actual);CompareStream(expected,actual,"output");fclose(actual);fclose(file);}
    fclose(expected);fclose(a);fclose(b);espeak_ng_ClearErrorContext(&error_a);espeak_ng_ClearErrorContext(&error_b);compared++;
}
int main(void)
{
    Guards();
    TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)>0);TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
    char original_home[N_PATH_BUF];strcpy(original_home,path_home);
    char root[]="/tmp/espeak-dictionary-compile-XXXXXX";TEST_ASSERT(mkdtemp(root));
    char prefix[N_PATH_BUF],path[N_PATH_BUF];snprintf(prefix,sizeof(prefix),"%s/",root);strcpy(path_home,root);
    Compare(prefix,"en",0); /* missing rules and complete error context */
    const char *lists[]={"word a\nword b $pause\n", "(one two) a\n?!2 THREE b $allcaps\n", "$textmode\nhello world\n_special word\n", "word a $unknown\n", "️ a\nU+1f600 b\n"};
    const char *rules[]={".group a\na a\n", ".replace\na bc\n.group a\na a\n", ".L01 j// voiced\n.group a\na a\n", ".group 9\n9 a\n२ a\n", ".Lxx bad\n.group a\nb b\n", ".group a\na a\n.group a\na b\n"};
    for(unsigned i=0;i<240;i++) {
        const char *extension=i%2?".txt":"";
        snprintf(path,sizeof(path),"%s/en_rules%s",root,extension);FILE *f=fopen(path,"w");TEST_ASSERT(f);fputs(rules[Next()%6],f);fclose(f);
        const char *names[]={"roots","list","listx","emoji","extra"};
        for(unsigned slot=0;slot<5;slot++) {snprintf(path,sizeof(path),"%s/en_%s",root,names[slot]);f=fopen(path,"w");TEST_ASSERT(f);fprintf(f,"precedence a $alt%u\n",slot+1);fputs(lists[Next()%5],f);fclose(f);}
        translator->langopts.listx=i%2;
        Compare(prefix,"en",i%4);
        snprintf(path,sizeof(path),"%s/en_rules%s",root,extension);TEST_ASSERT(unlink(path)==0);
        for(unsigned slot=0;slot<5;slot++) {snprintf(path,sizeof(path),"%s/en_%s",root,names[slot]);TEST_ASSERT(unlink(path)==0);}
    }
    snprintf(path,sizeof(path),"%s/en_rules",root);FILE *f=fopen(path,"w");TEST_ASSERT(f);fputs(".group a\na a\n",f);fclose(f);
    char original_cwd[N_PATH_BUF];TEST_ASSERT(getcwd(original_cwd,sizeof(original_cwd)));TEST_ASSERT(chdir(root)==0);
    Compare(NULL,NULL,1);TEST_ASSERT(chdir(original_cwd)==0);
    snprintf(path,sizeof(path),"%s/en_rules.txt",root);f=fopen(path,"w");TEST_ASSERT(f);fputs(".group b\nb b\n",f);fclose(f);Compare(prefix,"en",0);TEST_ASSERT(unlink(path)==0);
    TEST_ASSERT(mkdir(path,0700)==0);Compare(prefix,"en",0);TEST_ASSERT(rmdir(path)==0);
    snprintf(path,sizeof(path),"%s/en_dict",root);TEST_ASSERT(unlink(path)==0);TEST_ASSERT(mkdir(path,0700)==0);Compare(prefix,"en",0);TEST_ASSERT(rmdir(path)==0);
    snprintf(path,sizeof(path),"%s/en_rules",root);TEST_ASSERT(unlink(path)==0);
    strcpy(path_home,original_home);TEST_ASSERT(rmdir(root)==0);espeak_Terminate();
    printf("%u complete actual C dictionary drivers/output/status/error-context/diagnostic comparisons and seven admission guards passed\n",compared);return 0;
}
