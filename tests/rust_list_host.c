/* Actual previous/native list, symbol and prefix host adapters, same effects.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "translate.h"
#include "speech.h"
#include "rust_lookup_symbol.h"

typedef struct {
    Translator tr;
    char source[512], *cursor, phonemes[200];
    unsigned seed, calls, children;
    int bad;
    uint64_t trace;
} Fixture;
static Fixture *active;
static void Trace(Fixture *f, unsigned value) { f->trace=(f->trace^value)*UINT64_C(1099511628211); }
static void Bytes(Fixture *f, const char *text) { for (const unsigned char *p=(const unsigned char *)text;*p;p++) Trace(f,*p);Trace(f,0); }
static const char *MockLookup(Translator *tr,const char *key,const char *next,char *out,unsigned *flags,int end,WORD_TAB *words,int remaining)
{
    Fixture *f=(Fixture *)tr;TEST_ASSERT(f==active);
    Trace(f,1);Bytes(f,key);Trace(f,(unsigned)(next-f->source));
    Trace(f,flags[0]);Trace(f,flags[1]);Bytes(f,out);
    Trace(f,(unsigned)end);Trace(f,words!=NULL);Trace(f,(unsigned)remaining);
    unsigned hash=f->seed+(++f->calls)*0x192731;
    for(const unsigned char *p=(const unsigned char *)key;*p;p++) hash=hash*33+*p;
    flags[0]=((hash&1)?FLAG_MAX3:0)|((hash&2)?FLAG_TEXTMODE:0)|((hash&4)?FLAG_SKIPWORDS:0)|0x80000000u;
    flags[1]=(hash&8)?FLAG_ACCENT:0;
    if(hash&16) { unsigned count=(hash>>6)%9;for(unsigned i=0;i<count;i++) out[i]=(hash&256)?(char)(0x82+i):(char)('a'+i);out[count]=0; }
    if(hash&512) tr->langopts.textmode=!tr->langopts.textmode;
    if(hash&1024) option_sayas=(int)(hash>>18);
    dictionary_skipwords=(int)((hash>>14)&7);
    /* Force a real original-source trace extent; accent has only key offsets. */
    if(option_phonemes&espeakPHONEMES_TRACE) flags[1]=0;
    if(f->bad) {
        flags[0]=FLAG_TEXTMODE;flags[1]=0;tr->langopts.textmode=0;strcpy(out,"name");
        if(f->bad==1) memset(out,0x82,200);
        if(f->bad==2) return (const char *)((uintptr_t)f->source-1);
        if(f->bad==3) return f->source+sizeof(f->source);
        if(f->bad==4) { flags[0]=FLAG_FOUND;strcpy(out,"long"); }
        return next;
    }
    if(!(hash&32)) return NULL;
    size_t length=strlen(f->source)+1,matched=(size_t)(next-f->source)+((hash>>10)&3);
    if(matched>length) matched=length;
    return f->source+matched;
}
static void MockAccent(Translator *tr,unsigned code,char *out,size_t capacity)
{
    Fixture *f=(Fixture *)tr;Trace(f,2);Trace(f,code);Trace(f,(unsigned)capacity);Bytes(f,out);
    if(f->seed&1) {out[0]=(char)0x83;out[1]=0;}
    if(f->seed&2) tr->langopts.textmode=!tr->langopts.textmode;
}
static int MockTranslate(Translator *tr,char *word,WORD_TAB *rows,char *replacement)
{
    Fixture *f=(Fixture *)tr;TEST_ASSERT(f==active&&rows==NULL&&replacement==NULL&&option_sayas==0);
    TEST_ASSERT(tr->rule_text_base==word-3&&tr->rule_text_length==80);
    TEST_ASSERT(word[-3]==0&&word[-2]==' '&&word[-1]==' '&&word[76]==0);
    Trace(f,3);for(unsigned i=0;i<80;i++) Trace(f,(unsigned char)word[(int)i-3]);
    unsigned hash=f->seed+(++f->children)*0x321917;
    for(const unsigned char *p=(const unsigned char *)word;*p;p++) hash=hash*33+*p;
    unsigned count=(hash>>5)%9;for(unsigned i=0;i<count;i++) f->phonemes[i]=(hash&16)?(char)(0x82+i):(char)('j'+i);f->phonemes[count]=0;
    if(f->bad==5) memset(f->phonemes,0x82,200);
    if(f->bad==6) strcpy(f->phonemes,"long");
    option_sayas=(int)(hash>>13);dictionary_skipwords=(int)((hash>>14)&7);
    /* Nested child can leave different source metadata: outer host restores it. */
    tr->rule_text_base=f->source+1;tr->rule_text_length=3;
    return (hash&1)?0:(int)(hash|0x80000000u);
}
static int MockPrint(FILE *file,const char *format,const char *word,const char *replacement)
{
    (void)file;TEST_ASSERT(strcmp(format,"Replace: %s  %s\n")==0);
    Trace(active,4);Bytes(active,word);Bytes(active,replacement);return 0;
}
#define espeak_rs_lookup_dict MockLookup
#define LookupAccentedLetterBounded MockAccent
#define TranslateWord MockTranslate
#define word_phonemes (active->phonemes)
#define fprintf MockPrint
#define RustListHost PreviousListHost
#define LookupDictListBounded PreviousList
#define LookupBounded PreviousSymbol
#define LookupFlags PreviousFlags
#include "list_host_reference.inc"
#undef RustListHost
#undef LookupDictListBounded
#undef LookupBounded
#undef LookupFlags
/* Production raw effects/wrappers are extracted directly at configure time. */
#define ListAccent NativeAccent
#define LookupDictListBounded NativeList
#define LookupBounded NativeSymbol
#define LookupFlags NativeFlags
#include "native_list_host_effects.inc"
#include "native_symbol_host_effect_bridge.inc"
#include "native_prefix_host_effect_bridge.inc"
#undef ListAccent
#undef LookupDictListBounded
#undef LookupBounded
#undef LookupFlags
#undef espeak_rs_lookup_dict
#undef LookupAccentedLetterBounded
#undef TranslateWord
#undef word_phonemes
#undef fprintf

static unsigned random_state=0x82463ab9;
static unsigned Random(void) {random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state;}
static void Bind(Fixture *f,int scope,size_t extent)
{
    f->cursor=f->source;
    f->tr.rule_text_base=scope?f->source:NULL;
    f->tr.rule_text_length=scope?extent:0;
}
static void Compare(unsigned trial,int kind,int bad,size_t fixed_capacity)
{
    const char *sources[]={"cat","cat dog ","a . b . c tail ","é . à . 9 ","_é ","cake ","fall ","123.abc ","! ","_e ",". ","a . b ","é . a ","x__ ","a . b . c . d . e ","","a . b . "};
    Fixture ref={0},native={0};ref.seed=Random();ref.bad=bad;ref.trace=UINT64_C(1469598103934665603);
    strcpy(ref.source,bad?"cat ":sources[trial%(sizeof(sources)/sizeof(*sources))]);
    if(!bad&&trial%17==0) {memset(ref.source,'a',220);ref.source[220]=0;}
    if(!bad&&trial%19==0) {for(unsigned i=0;i<180;i+=4) memcpy(ref.source+i,"a . ",4);ref.source[180]='b';}
    strcpy(ref.tr.phonemes_repeat,(trial%2)?"abcdefg":"");ref.tr.phonemes_repeat_count=(int)(Random()%6);
    ref.tr.langopts.textmode=(int)(Random()&1);memset(ref.tr.rust_list_replacement,0x93,160);
    native=ref;int scope=(int)(trial%3!=0);size_t extent=sizeof(ref.source);
    if(!bad&&trial%23==0) extent=2; /* Truncated registered source; prefix ignores it. */
    Bind(&ref,scope,extent);Bind(&native,scope,extent);
    unsigned expected_flags[2]={Random(),Random()},actual_flags[2];memcpy(actual_flags,expected_flags,sizeof(actual_flags));
    char expected[200],actual[200];memset(expected,0x97,200);memset(actual,0x97,200);
    int end=(int)(Random()&(FLAG_ALLOW_TEXTMODE|FLAG_SUFX_E_ADDED|SUFX_D));
    WORD_TAB words[2]={{0}};WORD_TAB *rows=(trial&1)?words:NULL;int remaining=rows?(int)(trial%5)-2:0;
    size_t capacity=fixed_capacity?fixed_capacity:1+Random()%200;
    int trace_option=(trial<1000||bad)?espeakPHONEMES_TRACE:0,say_as=(int)Random();
    active=&ref;dictionary_skipwords=77;option_sayas=say_as;option_phonemes=trace_option;
    int wanted=kind==0?PreviousList(&ref.tr,&ref.cursor,expected,expected_flags,end,rows,remaining,capacity):
        kind==1?PreviousSymbol(&ref.tr,ref.source,expected,capacity):PreviousFlags(&ref.tr,ref.source,expected_flags);
    int skip=dictionary_skipwords,say=option_sayas;
    active=&native;dictionary_skipwords=77;option_sayas=say_as;option_phonemes=trace_option;
    int got=kind==0?NativeList(&native.tr,&native.cursor,actual,actual_flags,end,rows,remaining,capacity):
        kind==1?NativeSymbol(&native.tr,native.source,actual,capacity):NativeFlags(&native.tr,native.source,actual_flags);
    if(wanted!=got||memcmp(expected,actual,200)||memcmp(expected_flags,actual_flags,sizeof(actual_flags))||skip!=dictionary_skipwords||say!=option_sayas||ref.trace!=native.trace||ref.calls!=native.calls||ref.children!=native.children||ref.tr.phonemes_repeat_count!=native.tr.phonemes_repeat_count||memcmp(ref.tr.phonemes_repeat,native.tr.phonemes_repeat,20)||memcmp(ref.tr.rust_list_replacement,native.tr.rust_list_replacement,160)||ref.tr.langopts.textmode!=native.tr.langopts.textmode||memcmp(ref.phonemes,native.phonemes,200)||(ref.cursor!=ref.source)!=(native.cursor!=native.source)||strcmp(ref.cursor,native.cursor)) {
        fprintf(stderr,"native list host trial=%u kind=%d bad=%d cap=%zu result=%d/%d calls=%u/%u children=%u/%u trace=%llx/%llx\n",trial,kind,bad,capacity,wanted,got,ref.calls,native.calls,ref.children,native.children,(unsigned long long)ref.trace,(unsigned long long)native.trace);TEST_ASSERT(false);
    }
    TEST_ASSERT(ref.tr.rule_text_base==(scope?ref.source:NULL)&&native.tr.rule_text_base==(scope?native.source:NULL));
    TEST_ASSERT(ref.tr.rule_text_length==(scope?extent:0)&&native.tr.rule_text_length==(scope?extent:0));
}
int main(void)
{
    for(int kind=0;kind<3;kind++) for(unsigned trial=0;trial<200000;trial++) Compare(trial,kind,0,0);
    for(int kind=0;kind<3;kind++) for(int bad=1;bad<=6;bad++) Compare(1,kind,bad,bad==4||bad==6?3:200);
    Fixture f={0};strcpy(f.source,"cat ");Bind(&f,1,sizeof(f.source));active=&f;
    char out[200];memset(out,0x97,200);unsigned flags[2]={7,8};
    TEST_ASSERT(NativeList(NULL,&f.cursor,out,flags,0,NULL,0,200)==0&&out[0]==(char)0x97);
    TEST_ASSERT(NativeSymbol(&f.tr,NULL,out,200)==0&&out[0]==(char)0x97);
    TEST_ASSERT(NativeList(&f.tr,&f.cursor,out,flags,0,NULL,0,0)==0&&out[0]==(char)0x97&&f.calls==0);
    /* Callback child writes the same global buffer as caller output. */
    f.bad=6;Fixture previous=f;Bind(&previous,1,sizeof(previous.source));
    option_sayas=9;dictionary_skipwords=77;unsigned prior_calls=f.calls;
    int got=NativeSymbol(&f.tr,f.source,f.phonemes,200),say=option_sayas,skip=dictionary_skipwords;
    active=&previous;option_sayas=9;dictionary_skipwords=77;
    int wanted=PreviousSymbol(&previous.tr,previous.source,out,200);
    TEST_ASSERT(got==wanted&&f.calls==prior_calls+1&&strcmp(f.phonemes,out)==0);
    TEST_ASSERT(f.tr.rule_text_base==f.source&&f.tr.rule_text_length==sizeof(f.source)&&say==option_sayas&&skip==dictionary_skipwords);
    printf("600000 previous/native host episodes, 18 malformed cases, native guards and aliased child output passed\n");
    return 0;
}
