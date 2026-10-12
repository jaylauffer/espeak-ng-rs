/* Frozen complete C list compiler vs generic Rust via frozen native adapters.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "common.h"
#include "speech.h"
#include "dictionary.h"
#include "translate.h"
#include "rust_data.h"
#include <ctype.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <wctype.h>
#include <sys/stat.h>
#include <espeak-ng/speak_lib.h>
_Static_assert(sizeof(RustDictListState)==12,"native list state ABI");
_Static_assert(sizeof(RustDictList)==(sizeof(void *)==8?80:44),"native list descriptor ABI");
typedef struct {
    FILE *f_log;char word_phonemes[N_WORD_PHONEMES];
    int linenum,error_count;bool text_mode;int error_need_dictionary;
    char *hash_chains[N_HASH_DICT];
} CompileContext;
static uint64_t effect_hash;
static unsigned effect_count;
static unsigned char effect_bytes[65536],expected_bytes[65536];
static size_t effect_length;
static void Effect(unsigned kind,const void *bytes,size_t length)
{
    effect_count++;effect_hash=effect_hash*31+kind;
    TEST_ASSERT(length<65536 && effect_length+length+3<=sizeof(effect_bytes));
    effect_bytes[effect_length++]=kind;effect_bytes[effect_length++]=length;effect_bytes[effect_length++]=length>>8;
    memcpy(effect_bytes+effect_length,bytes,length);effect_length+=length;
    const unsigned char *p=bytes;for(size_t i=0;i<length;i++)effect_hash=effect_hash*31+p[i];
    effect_hash=effect_hash*31+0xff;
}
static int TraceTranslate(Translator *tr,char *input,WORD_TAB *rows,char *output)
{Effect(1,input,strlen(input)+1);return TranslateWord(tr,input,rows,output);}
static const char *TraceEncode(const char *input,char *output,size_t capacity,int *bad)
{Effect(2,input,strlen(input)+1);Effect(3,&capacity,sizeof(capacity));return EncodePhonemesBounded(input,output,capacity,bad);}
static int TraceUpper(wint_t code)
{Effect(4,&code,sizeof(code));return iswupper(code);}
static int TraceLower(unsigned int code,Translator *tr)
{Effect(5,&code,sizeof(code));return towlower2(code,tr);}
static int TraceTranspose(Translator *tr,char *input)
{Effect(6,input,strlen(input)+1);return TransposeAlphabet(tr,input);}
#undef TranslateWord
#undef EncodePhonemesBounded
#undef iswupper
#undef towlower2
#undef TransposeAlphabet
#define TranslateWord TraceTranslate
#define EncodePhonemesBounded TraceEncode
#define iswupper TraceUpper
#define towlower2 TraceLower
#define TransposeAlphabet TraceTranspose
/* The original mnemonic table is copied verbatim alongside the policy. */
#include "dict_list_flags_reference.inc"
#include "dict_list_reference.inc"
static int FixtureWrite(void *file,const uint8_t *bytes,size_t length) {return fwrite(bytes,1,length,file)==length;}
#define espeak_rs_dictionary_write FixtureWrite
#include "native_dict_list.inc"
#undef espeak_rs_dictionary_write
static unsigned compared,files;
static unsigned guards;
static uint32_t random_state=0x19abc179;
static uint32_t Next(void) {random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state;}
static void CompareFiles(FILE *a,FILE *b,const char *source,const char *kind)
{
    fflush(a);fflush(b);long x=ftell(a),y=ftell(b);
    if(x!=y) {fprintf(stderr,"%s length C=%ld Rust=%ld [%s]\n",kind,x,y,source);abort();}
    rewind(a);rewind(b);
    for(long i=0;i<x;i++)if(fgetc(a)!=fgetc(b)) {fprintf(stderr,"%s byte %ld [%s]\n",kind,i,source);abort();}
}
static void Compare(const char *source,int text,int need)
{
    if (getenv("ESPEAK_DICT_LIST_TRACE")) {fprintf(stderr,"compare text=%d [%s]\n",text,source);fflush(stderr);}
    char input[200]={0},a_out[256]={0},b_out[256]={0};strcpy(input,source);
    CompileContext a={0},b={0};a.f_log=tmpfile();b.f_log=tmpfile();TEST_ASSERT(a.f_log&&b.f_log);
    a.linenum=b.linenum=37;a.text_mode=b.text_mode=text;a.error_need_dictionary=b.error_need_dictionary=need;
    int a_hash=-1,b_hash=-1;
    effect_hash=0;effect_count=0;effect_length=0;int x=compile_line(&a,input,a_out,256,&a_hash);
    uint64_t expected_effects=effect_hash;unsigned expected_count=effect_count;
    size_t expected_length=effect_length;memcpy(expected_bytes,effect_bytes,effect_length);
    effect_hash=0;effect_count=0;effect_length=0;int y=compile_line_native(&b,source,b_out,256,&b_hash);
    if(expected_effects!=effect_hash || expected_count!=effect_count || expected_length!=effect_length || memcmp(expected_bytes,effect_bytes,effect_length)) {fprintf(stderr,"effect trace C=%u Rust=%u [%s]\n",expected_count,effect_count,source);abort();}
    if(x!=y || (x && (a_hash!=b_hash || memcmp(a_out,b_out,x)))) {
        fprintf(stderr,"record C=%d Rust=%d hash %d/%d text %d [%s]\n",x,y,a_hash,b_hash,text,source);abort();
    }
    TEST_ASSERT(a.text_mode==b.text_mode && a.error_count==b.error_count && a.error_need_dictionary==b.error_need_dictionary);
    CompareFiles(a.f_log,b.f_log,source,"diagnostics");fclose(a.f_log);fclose(b.f_log);compared++;
}
static void File(const char *root,const char *source,int listx)
{
    char prefix[512],path[512];snprintf(prefix,sizeof(prefix),"%s/en_",root);
    const char *names[]={"roots","list","listx","emoji","extra"};
    for(unsigned i=0;i<5;i++) {
        snprintf(path,sizeof(path),"%s%s%s",prefix,names[i],i%2?".txt":"");FILE *f=fopen(path,"w");TEST_ASSERT(f);fprintf(f,"precedence a $alt%u\n",i+1);fputs(source,f);fclose(f);
        if (i%2) {snprintf(path,sizeof(path),"%s%s",prefix,names[i]);f=fopen(path,"w");TEST_ASSERT(f);fputs("shadow b\n",f);fclose(f);}
#ifndef _WIN32
        if (i==2) {snprintf(path,sizeof(path),"%s%s.txt",prefix,names[i]);f=fopen(path,"w");TEST_ASSERT(f);fputs("unreadable b\n",f);fclose(f);TEST_ASSERT(chmod(path,0)==0);}
#endif
    }
    CompileContext a={0},b={0};a.f_log=tmpfile();b.f_log=tmpfile();FILE *out_a=tmpfile(),*out_b=tmpfile();TEST_ASSERT(a.f_log&&b.f_log&&out_a&&out_b);
    translator->langopts.listx=listx;compile_dictlist_start(&a);effect_hash=0;effect_count=0;effect_length=0;
    compile_dictlist_file(&a,prefix,"roots");
    compile_dictlist_file(&a,prefix,listx?"list":"listx");compile_dictlist_file(&a,prefix,listx?"listx":"list");
    compile_dictlist_file(&a,prefix,"emoji");compile_dictlist_file(&a,prefix,"extra");compile_dictlist_end(&a,out_a);
    uint64_t expected_effects=effect_hash;unsigned expected_count=effect_count;
    size_t expected_length=effect_length;memcpy(expected_bytes,effect_bytes,effect_length);
    effect_hash=0;effect_count=0;effect_length=0;
    TEST_ASSERT(compile_dictlists_native(&b,prefix,out_b)==ENS_OK);
    TEST_ASSERT(expected_effects==effect_hash && expected_count==effect_count && expected_length==effect_length && memcmp(expected_bytes,effect_bytes,effect_length)==0);
    TEST_ASSERT(a.text_mode==b.text_mode && a.linenum==b.linenum && a.error_count==b.error_count && a.error_need_dictionary==b.error_need_dictionary);
    CompareFiles(out_a,out_b,source,"bucket bytecode");CompareFiles(a.f_log,b.f_log,source,"file diagnostics");
    compile_dictlist_start(&a);fclose(out_a);fclose(out_b);fclose(a.f_log);fclose(b.f_log);
    for(unsigned i=0;i<5;i++) {
        snprintf(path,sizeof(path),"%s%s%s",prefix,names[i],i%2?".txt":"");unlink(path);
        if (i%2) {snprintf(path,sizeof(path),"%s%s",prefix,names[i]);unlink(path);}
#ifndef _WIN32
        if (i==2) {snprintf(path,sizeof(path),"%s%s.txt",prefix,names[i]);TEST_ASSERT(chmod(path,0600)==0);unlink(path);}
#endif
    }files++;
}
static int RefuseWrite(void *opaque,const uint8_t *bytes,size_t length) {(void)opaque;(void)bytes;(void)length;return 0;}
static void Guard(void)
{
    CompileContext ctx={0};ctx.f_log=tmpfile();TEST_ASSERT(ctx.f_log);
    uint8_t alnums[256];DictListIo io={&ctx,NULL};RustDictList host=list_descriptor(&io,alnums);
    uint8_t output[256];memset(output,0x55,sizeof(output));int hash=-37;RustDictListState state={19,0,0};
    TEST_ASSERT(espeak_rs_compile_dict_line(&host,"word a\n",output,1,&state,&hash)==0);
    TEST_ASSERT(hash==-37&&output[0]==0x55&&output[255]==0x55);guards++;
    char invalid[200];memset(invalid,'x',sizeof(invalid));
    TEST_ASSERT(espeak_rs_compile_dict_line(&host,invalid,output,256,&state,&hash)==0);
    TEST_ASSERT(hash==-37&&output[0]==0x55&&ctx.error_count==1);guards++;
    TEST_ASSERT(espeak_rs_compile_dict_line(&host,"word a $text",output,256,&state,&hash)==0);
    TEST_ASSERT(hash==-37&&output[0]==0x55&&ctx.error_count==2);guards++;
    TEST_ASSERT(espeak_rs_compile_dict_line(NULL,"word a\n",output,256,&state,&hash)==0);guards++;
    char invalid_path[4096];memset(invalid_path,'x',sizeof(invalid_path));
    TEST_ASSERT(espeak_rs_compile_dict_lists(&host,invalid_path,&state)==ENS_COMPILE_ERROR);guards++;
    char root[]="/tmp/espeak-dict-empty-XXXXXX";TEST_ASSERT(mkdtemp(root));
    char prefix[512];snprintf(prefix,sizeof(prefix),"%s/en_",root);
    FILE *out=tmpfile();TEST_ASSERT(out);io.output=out;
    TEST_ASSERT(espeak_rs_compile_dict_lists(&host,prefix,&state)==ENS_OK);TEST_ASSERT(ftell(out)==1024);
    TEST_ASSERT(state.line==19&&state.text_mode==0);guards++;
    host.write=RefuseWrite;TEST_ASSERT(espeak_rs_compile_dict_lists(&host,prefix,&state)==ENS_COMPILE_ERROR);guards++;
    fclose(out);TEST_ASSERT(rmdir(root)==0);fclose(ctx.f_log);
}
int main(void)
{
    TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)>0);TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
    const char *words[]={"word","CAPS","Mixed","_special","U+41","U+1f600","😊️","️","été","ÄÖ","(one two)","(2-three)","(one)","one)"};
    const char *ph[]={"a","b","a b","_^_FR","?","","a||b"};
    const char *flags[]={"","$1","$u3+","$alt","$text","$textmode","$phonememode","$onlys","$allcaps","$unknown","$native","$pause $verb","$textmode $phonememode","$abbrev // comment"};
    char line[200];
    for(unsigned i=0;i<18000;i++) {
        const char *word=words[Next()%14],*pronunciation=ph[Next()%7],*flag=flags[Next()%14];
        /* Retained C writes into static nullstring for empty _special text;
         * that malformed-input trap is covered by the safe Rust regression. */
        if(word[0]=='_' && pronunciation[0]==0)pronunciation="a";
        snprintf(line,sizeof(line),"%s%s %s %s\n",i%5==0?"?!12 ":i%7==0?"?3 ":"",word,pronunciation,flag);
        Compare(line,i%2,i%4);
    }
    const char *edges[]={"","\n","// comment\n","word\n","?99 WORD a\n","$textmode\n","(one two three four five six seven eight nine ten eleven twelve) a\n","U+-1 a\n","U+0x41 a\n","word a // comment\n","word a $text tail\n","️ a\n"};
    for(unsigned i=0;i<sizeof(edges)/sizeof(*edges);i++)Compare(edges[i],0,0);
    for(const MNEM_TAB *entry=mnem_flags;entry->mnem!=NULL;entry++) {
        snprintf(line,sizeof(line),"word a %s\n",entry->mnem);Compare(line,0,0);
    }
    char root[]="/tmp/espeak-dict-list-XXXXXX";TEST_ASSERT(mkdtemp(root));
    for(unsigned i=0;i<100;i++) {
        snprintf(line,sizeof(line),"word %s\nword b $pause\nCAPS a\n?2 other a\n(one two) b\n$%smode\nnext a\n",ph[Next()%7],i%2?"text":"phoneme");
        File(root,line,i%2);
    }
    char long_line[1200];strcpy(long_line,"base a // ");memset(long_line+10,'x',700);strcpy(long_line+710,"\n7 a\nlast b");File(root,long_line,1);
    File(root,"$textmode\nordinary word\n_special word\n$phonememode\nword a\n",0);
    File(root,"longer a $text tail more\nword a $text",0);
    TEST_ASSERT(rmdir(root)==0);
    TEST_ASSERT(espeak_SetVoiceByName("ru")==EE_OK);
    for(unsigned i=0;i<2000;i++) {snprintf(line,sizeof(line),"%s %s %s\n",i%2?"АБ":"ла",ph[Next()%7],flags[Next()%14]);Compare(line,i%2,0);}
    TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);int saved_dotless=translator->langopts.dotless_i;
    translator->langopts.dotless_i=1;
    Compare("(I TWO) a\n",0,0);Compare("(I Two) a\n",0,0);Compare("I a\n",0,0);
    char case_root[]="/tmp/espeak-dict-case-XXXXXX";TEST_ASSERT(mkdtemp(case_root));File(case_root,"(I TWO) a\n",0);TEST_ASSERT(rmdir(case_root)==0);
    translator->langopts.dotless_i=saved_dotless;
    Guard();
    espeak_Terminate();printf("%u actual C entry records/state/diagnostics, %u complete C list files/buckets and %u native guards passed\n",compared,files,guards);return 0;
}
