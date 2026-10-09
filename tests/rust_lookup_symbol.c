/* Symbol/prefix policy against the unchanged C wrappers and same native list.
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
    char source[512], *cursor, **word_slot, phonemes[200];
    unsigned seed, queries, translations;
    int bad;
    uint64_t trace;
} Fixture;
static void Trace(Fixture *f,unsigned value) { f->trace=(f->trace^value)*UINT64_C(1099511628211); }
static void Bytes(Fixture *f,const char *text) { for(const unsigned char *p=(const unsigned char *)text;*p;p++) Trace(f,*p);Trace(f,0); }
static int Byte(void *opaque,size_t position) { Fixture *f=opaque;return position<sizeof(f->source)?(unsigned char)f->source[position]:-1; }
static int Query(void *opaque,const char *key,size_t next,unsigned *flags,char *out,size_t *matched)
{
    Fixture *f=opaque;Trace(f,1);Bytes(f,key);Trace(f,(unsigned)next);Trace(f,flags[0]);Trace(f,flags[1]);Bytes(f,out);
    unsigned hash=f->seed+(++f->queries)*0x1834ad;
    for(const unsigned char *p=(const unsigned char *)key;*p;p++) hash=hash*33+*p;
    flags[0]=((hash&1)?FLAG_MAX3:0)|((hash&2)?FLAG_TEXTMODE:0)|((hash&4)?FLAG_SKIPWORDS:0)|0x80000000u;
    flags[1]=(hash&8)?FLAG_ACCENT:0;
    /* Keep source trace extents valid; the list oracle covers its traces. */
    if(hash&16) { unsigned count=(hash>>6)%9;for(unsigned i=0;i<count;i++) out[i]=(hash&256)?(char)(0x82+i):(char)('a'+i);out[count]=0; }
    if(hash&512) f->tr.langopts.textmode=!f->tr.langopts.textmode;
    if((hash&1024)&&!f->bad) option_sayas=(int)(hash>>18);
    dictionary_skipwords=(int)((hash>>14)&7);
    if(f->bad==4||f->bad==5) {
        flags[0]=FLAG_FOUND;flags[1]=0;f->tr.langopts.textmode=false;*matched=next;
        memset(out,'a',f->bad==4?200:100);if(f->bad==5) out[100]=0;return 1;
    }
    if(f->bad) { flags[0]=FLAG_TEXTMODE;flags[1]=0;strcpy(out,"name");f->tr.langopts.textmode=false;*matched=next;return 1; }
    if(!(hash&32)) return 0;
    *matched=next;return 1;
}
static int Repeat(void *opaque,char *out) { Fixture *f=opaque;memcpy(out,f->tr.phonemes_repeat,20);return f->tr.phonemes_repeat_count; }
static void SetRepeat(void *opaque,const char *out,int count) { Fixture *f=opaque;memcpy(f->tr.phonemes_repeat,out,20);f->tr.phonemes_repeat_count=count; }
static int TextMode(void *opaque) { return ((Fixture *)opaque)->tr.langopts.textmode; }
static void Skip(void *opaque,int count) { (void)opaque;dictionary_skipwords=count; }
static void Accent(void *opaque,unsigned code,size_t capacity,char *out)
{
    Fixture *f=opaque;Trace(f,2);Trace(f,code);Trace(f,(unsigned)capacity);Bytes(f,out);
    if(f->seed&1) { out[0]=(char)0x83;out[1]=0; }
    if(f->seed&2) f->tr.langopts.textmode=!f->tr.langopts.textmode;
}
static void Replacement(void *opaque,const char *text) { Fixture *f=opaque;memcpy(f->tr.rust_list_replacement,text,160);f->cursor=f->tr.rust_list_replacement+2;if(f->word_slot) *f->word_slot=f->cursor; }
static void ListTrace(void *opaque,size_t matched) { (void)opaque;(void)matched; }
static RustLookupList List(Fixture *f) { return (RustLookupList){f,Byte,Query,Repeat,SetRepeat,TextMode,Skip,Accent,Replacement,ListTrace}; }
static int MockList(Translator *tr,char **word,char *out,unsigned *flags,int end,WORD_TAB *rows,int remaining,size_t capacity)
{
    Fixture *f=(Fixture *)tr;TEST_ASSERT((end==FLAG_ALLOW_TEXTMODE||end==0)&&rows==NULL&&remaining==0);
    f->word_slot=word;RustLookupList table=List(f);
    int found=espeak_rs_lookup_list(&table,(unsigned)end,flags,out,capacity);f->word_slot=NULL;
    TEST_ASSERT(found>=0);return found;
}
static int Translate(Translator *tr,char *word,WORD_TAB *rows,char *replacement)
{
    Fixture *f=(Fixture *)tr;TEST_ASSERT(rows==NULL&&replacement==NULL);TEST_ASSERT(option_sayas==0);
    Trace(f,3);Trace(f,(unsigned)option_sayas);for(unsigned i=0;i<80;i++) Trace(f,(unsigned char)word[(int)i-3]);
    unsigned hash=f->seed+(++f->translations)*0x321917;
    for(const unsigned char *p=(const unsigned char *)word;*p;p++) hash=hash*33+*p;
    unsigned count=(hash>>5)%9;for(unsigned i=0;i<count;i++) f->phonemes[i]=(hash&16)?(char)(0x82+i):(char)('j'+i);f->phonemes[count]=0;
    option_sayas=(int)(hash>>13);dictionary_skipwords=(int)((hash>>14)&7);
    return (hash&1)?0:(int)(hash|0x80000000u);
}
#define LookupDictListBounded MockList
#define TranslateWord Translate
#define word_phonemes (((Fixture *)tr)->phonemes)
#define LookupBounded ReferenceSymbol
#include "lookup_symbol_reference.inc"
#undef LookupDictListBounded
#undef TranslateWord
#undef word_phonemes
#undef LookupBounded
#define LookupDictListBounded MockList
#define LookupFlags ReferenceFlags
#include "lookup_flags_reference.inc"
#undef LookupDictListBounded
#undef LookupFlags
static int SymbolByte(void *opaque,size_t position) { Fixture *f=opaque;if(f->bad==2) return -1;size_t bound=f->cursor==f->source?sizeof(f->source):158;return position<bound?(unsigned char)f->cursor[position]:-1; }
static int SayAs(void *opaque) { (void)opaque;return option_sayas; }
static void SetSayAs(void *opaque,int value) { (void)opaque;option_sayas=value; }
static int NativeTranslate(void *opaque,char *text,char *out)
{
    Fixture *f=opaque;const char *base=f->tr.rule_text_base;size_t length=f->tr.rule_text_length;
    f->tr.rule_text_base=text;f->tr.rule_text_length=80;
    int result=Translate(&f->tr,text+3,NULL,NULL);
    if(f->bad==1) memset(out,0x82,200);else if(f->bad==3) strcpy(out,"long");else memcpy(out,f->phonemes,200);
    f->tr.rule_text_base=base;f->tr.rule_text_length=length;
    return result;
}
static RustLookupSymbol Table(Fixture *f) { return (RustLookupSymbol){List(f),SymbolByte,SayAs,SetSayAs,NativeTranslate}; }
static unsigned seed=0x184ad65e;
static unsigned Random(void) { seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed; }
int main(void)
{
    const char *sources[]={"_name", "_é ", "a . b . c tail ", "cat dog ", "! ", "_emoji ", "é . à . 9 ", ""};
    unsigned translated=0,reduced=0;
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture ref={0},native={0};ref.seed=Random();ref.trace=UINT64_C(1469598103934665603);
        strcpy(ref.source,sources[trial%(sizeof(sources)/sizeof(*sources))]);
        if(trial%17==0) { memset(ref.source,'a',220);ref.source[220]=0; }
        if(trial%19==0) { for(unsigned i=0;i<180;i+=4) memcpy(ref.source+i,"a . ",4);ref.source[180]='b'; }
        strcpy(ref.tr.phonemes_repeat,(trial&1)?"abcdefg":"");ref.tr.phonemes_repeat_count=(int)(Random()%6);ref.tr.langopts.textmode=Random()&1;
        native=ref;ref.cursor=ref.source;native.cursor=native.source;ref.tr.rule_text_base=ref.source;native.tr.rule_text_base=native.source;ref.tr.rule_text_length=native.tr.rule_text_length=sizeof(ref.source);
        int saved_sayas=(int)Random();size_t capacity=10+Random()%191;
        char expected[200],actual[200];memset(expected,0x97,200);memset(actual,0x97,200);expected[0]=actual[0]=0;
        option_sayas=saved_sayas;dictionary_skipwords=77;
        int wanted=ReferenceSymbol(&ref.tr,ref.source,expected,capacity),expected_sayas=option_sayas,expected_skip=dictionary_skipwords;
        option_sayas=saved_sayas;dictionary_skipwords=77;int got=123;
        native.word_slot=&native.cursor;RustLookupSymbol table=Table(&native);
        int status=espeak_rs_lookup_symbol(&table,actual,capacity,&got);
        if(status!=0||wanted!=got||strcmp(expected,actual)||expected_sayas!=option_sayas||expected_skip!=dictionary_skipwords||ref.trace!=native.trace||ref.queries!=native.queries||ref.translations!=native.translations||ref.tr.phonemes_repeat_count!=native.tr.phonemes_repeat_count||memcmp(ref.tr.phonemes_repeat,native.tr.phonemes_repeat,20)||ref.tr.langopts.textmode!=native.tr.langopts.textmode||strcmp(ref.cursor,native.cursor)) {
            fprintf(stderr,"symbol trial=%u seed=%x capacity=%zu status=%d flags=%x/%x sayas=%d/%d trace=%llx/%llx phon=%s/%s\n",trial,ref.seed,capacity,status,(unsigned)wanted,(unsigned)got,expected_sayas,option_sayas,(unsigned long long)ref.trace,(unsigned long long)native.trace,expected,actual);TEST_ASSERT(false);
        }
        TEST_ASSERT(native.tr.rule_text_base==native.source&&native.tr.rule_text_length==sizeof(native.source));
        for(size_t i=strlen(actual)+1;i<200;i++) TEST_ASSERT((unsigned char)actual[i]==0x97);
        translated+=native.translations!=0;reduced+=capacity<200;
    }
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture ref={0},native={0};ref.seed=Random();ref.trace=UINT64_C(1469598103934665603);
        strcpy(ref.source,sources[trial%(sizeof(sources)/sizeof(*sources))]);
        strcpy(ref.tr.phonemes_repeat,(trial&1)?"abcdefg":"");ref.tr.phonemes_repeat_count=(int)(Random()%6);ref.tr.langopts.textmode=Random()&1;
        native=ref;ref.cursor=ref.source;native.cursor=native.source;
        unsigned wanted[2]={123,456},got[2]={123,456};int saved_sayas=(int)Random();
        option_sayas=saved_sayas;dictionary_skipwords=77;
        int result=ReferenceFlags(&ref.tr,ref.source,wanted),expected_sayas=option_sayas,expected_skip=dictionary_skipwords;
        TEST_ASSERT((unsigned)result==wanted[0]);
        option_sayas=saved_sayas;dictionary_skipwords=77;native.word_slot=&native.cursor;
        RustLookupList table=List(&native);int status=espeak_rs_lookup_flags(&table,got);
        if(status!=0||memcmp(wanted,got,sizeof(got))||expected_sayas!=option_sayas||expected_skip!=dictionary_skipwords||ref.trace!=native.trace||ref.queries!=native.queries||ref.tr.phonemes_repeat_count!=native.tr.phonemes_repeat_count||memcmp(ref.tr.phonemes_repeat,native.tr.phonemes_repeat,20)||ref.tr.langopts.textmode!=native.tr.langopts.textmode||strcmp(ref.cursor,native.cursor)) {
            fprintf(stderr,"prefix flags trial=%u seed=%x status=%d flags=%x,%x/%x,%x trace=%llx/%llx\n",trial,ref.seed,status,wanted[0],wanted[1],got[0],got[1],(unsigned long long)ref.trace,(unsigned long long)native.trace);TEST_ASSERT(false);
        }
        TEST_ASSERT(native.translations==0&&ref.translations==0);
    }
    Fixture f={0};strcpy(f.source,"_name ");f.cursor=f.source;f.word_slot=&f.cursor;f.tr.rule_text_base=f.source;f.tr.rule_text_length=sizeof(f.source);
    RustLookupSymbol table=Table(&f);char out[200];memset(out,0x97,200);int flags=123;
    TEST_ASSERT(espeak_rs_lookup_symbol(NULL,out,200,&flags)==-1);
    TEST_ASSERT(espeak_rs_lookup_symbol(&table,NULL,200,&flags)==-1);
    TEST_ASSERT(espeak_rs_lookup_symbol(&table,out,200,NULL)==-1);
    TEST_ASSERT(espeak_rs_lookup_symbol(&table,out,0,&flags)==-1);
    TEST_ASSERT(espeak_rs_lookup_symbol(&table,out,201,&flags)==-1);
    table.translate=NULL;TEST_ASSERT(espeak_rs_lookup_symbol(&table,out,200,&flags)==-1);table.translate=NativeTranslate;
    table.list.byte=NULL;TEST_ASSERT(espeak_rs_lookup_symbol(&table,out,200,&flags)==-1);table.list.byte=Byte;
    for(int bad=1;bad<=3;bad++) {
        f.bad=bad;option_sayas=7;unsigned calls=f.translations;
        TEST_ASSERT(espeak_rs_lookup_symbol(&table,out,bad==3?4:200,&flags)==-1);
        TEST_ASSERT(option_sayas==7&&flags==123);TEST_ASSERT(f.translations==calls+(bad!=2));
        for(unsigned i=0;i<200;i++) TEST_ASSERT((unsigned char)out[i]==0x97);
    }
    RustLookupList list=List(&f);unsigned attributes[2]={123,456};
    TEST_ASSERT(espeak_rs_lookup_flags(NULL,attributes)==-1);
    TEST_ASSERT(espeak_rs_lookup_flags(&list,NULL)==-1);
    list.byte=NULL;TEST_ASSERT(espeak_rs_lookup_flags(&list,attributes)==-1);list.byte=Byte;
    for(int bad=4;bad<=5;bad++) {
        f.bad=bad;option_sayas=7;unsigned calls=f.queries;
        TEST_ASSERT(espeak_rs_lookup_flags(&list,attributes)==-1);
        TEST_ASSERT(attributes[0]==123&&attributes[1]==456&&option_sayas==7&&f.queries==calls+1);
    }
    printf("200000 symbol policy comparisons passed (%u translations, %u reduced capacities); 200000 prefix flag comparisons and malformed ABI guards passed\n",translated,reduced);
    return 0;
}
