/* Complete dictionary-list policy against the unchanged C controller.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "translate.h"
#include "speech.h"
#include "rust_lookup_list.h"

typedef struct {
    Translator tr;
    char source[512], *cursor;
    unsigned seed, calls;
    int skip, bad;
    uint64_t trace;
} Fixture;
static void Trace(Fixture *f, unsigned value) { f->trace = (f->trace ^ value)*UINT64_C(1099511628211); }
static void Bytes(Fixture *f, const char *text) { for (const unsigned char *p=(const unsigned char *)text;*p;p++) Trace(f,*p); Trace(f,0); }
static const char *Lookup(Translator *tr,const char *key,const char *next,char *out,unsigned *flags,int end,WORD_TAB *words,int remaining)
{
    Fixture *f=(Fixture *)tr;
    Trace(f,1); Bytes(f,key); Trace(f,(unsigned)(next-f->source));
    Trace(f,flags[0]); Trace(f,flags[1]); Bytes(f,out);
    Trace(f,(unsigned)end); Trace(f,words!=NULL); Trace(f,(unsigned)remaining);
    unsigned hash=f->seed+(++f->calls)*0x192731;
    for(const unsigned char *p=(const unsigned char *)key;*p;p++) hash=hash*33+*p;
    flags[0]=((hash&1)?FLAG_MAX3:0)|((hash&2)?FLAG_TEXTMODE:0)|((hash&4)?FLAG_SKIPWORDS:0)|0x80000000u;
    flags[1]=(hash&8)?FLAG_ACCENT:0;
    /* A legacy synthetic accent pointer has no original-source trace extent. */
    if(option_phonemes&espeakPHONEMES_TRACE) flags[1]=0;
    if(hash&16) {
        unsigned count=(hash>>6)%8;
        for(unsigned i=0;i<count;i++) out[i]=(hash&256)?(char)(0x82+i):(char)('a'+i);
        out[count]=0;
    }
    if(f->bad==1) memset(out,0x82,200);
    if(hash&512) tr->langopts.textmode=!tr->langopts.textmode;
    dictionary_skipwords=(int)((hash>>14)&7);
    if(f->bad==2) { flags[0]=1;flags[1]=0;tr->langopts.textmode=false;strcpy(out,"abc");return f->source+4; }
    if(!(hash&32)) return NULL;
    size_t matched=(size_t)(next-f->source)+((hash>>10)&3), limit=strlen(f->source)+1;
    if(matched>limit) matched=limit;
    return f->source+matched;
}
static void Accent(Translator *tr,unsigned code,char *out,size_t capacity)
{
    Fixture *f=(Fixture *)tr;
    Trace(f,2);Trace(f,code);Trace(f,(unsigned)capacity);Bytes(f,out);
    if(f->seed&1) { out[0]=(char)0x83;out[1]=0; }
    if(f->seed&2) tr->langopts.textmode=!tr->langopts.textmode;
}
#define LookupDict2 Lookup
#define LookupAccentedLetterBounded Accent
#define LookupDictListBounded ReferenceList
#include "lookup_list_reference.inc"
#undef LookupDict2
#undef LookupAccentedLetterBounded
#undef LookupDictListBounded

static int Byte(void *opaque,size_t position) { Fixture *f=opaque;return position<sizeof(f->source)?(unsigned char)f->source[position]:-1; }
static int Query(void *opaque,const char *key,size_t next,unsigned *flags,char *out,size_t *matched)
{
    Fixture *f=opaque;if(next>sizeof(f->source)) return -1;
    const char *found=Lookup(&f->tr,key,f->source+next,out,flags,0,NULL,0);
    if(!found) return 0;*matched=(size_t)(found-f->source);return 1;
}
/* Actual source flags/word rows stay C projections, not native policy. */
static int Repeat(void *opaque,char *out) { Fixture *f=opaque;memcpy(out,f->tr.phonemes_repeat,20);return f->tr.phonemes_repeat_count; }
static void SetRepeat(void *opaque,const char *out,int count) { Fixture *f=opaque;memcpy(f->tr.phonemes_repeat,out,20);f->tr.phonemes_repeat_count=count; }
static int TextMode(void *opaque) { return ((Fixture *)opaque)->tr.langopts.textmode; }
static void Skip(void *opaque,int count) { (void)opaque;dictionary_skipwords=count; }
static void NativeAccent(void *opaque,unsigned code,size_t capacity,char *out) { Accent(&((Fixture *)opaque)->tr,code,out,capacity); }
static void Replacement(void *opaque,const char *text) { Fixture *f=opaque;memcpy(f->tr.rust_list_replacement,text,160);f->cursor=f->tr.rust_list_replacement+2; }
static void ReplacementTrace(void *opaque,size_t matched)
{
    Fixture *f=opaque;if(!(option_phonemes&espeakPHONEMES_TRACE)) return;
    TEST_ASSERT(matched<160);char word[160];memcpy(word,f->source,matched);word[matched]=0;
    fprintf(f_trans,"Replace: %s  %s\n",word,f->cursor);
}
static unsigned random_state=0x82463ab9;
static unsigned Random(void) { random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state; }
static int query_end;
static WORD_TAB *query_words;
static int query_remaining;
static int FullQuery(void *opaque,const char *key,size_t next,unsigned *flags,char *out,size_t *matched)
{
    Fixture *f=opaque;const char *found=Lookup(&f->tr,key,f->source+next,out,flags,query_end,query_words,query_remaining);
    if(!found) return 0;*matched=(size_t)(found-f->source);return 1;
}
static void Compare(unsigned trial,FILE *reference_trace,FILE *native_trace)
{
    const char *sources[]={"cat", "cat dog ", "a . b . c tail ", "é . à . 9 ", "_é ", "cake ", "fall ", "123.abc ", "! ", "_e ", ". ", "a . b ", "é . a ", "x__ ", "a . b . c . d . e ", "", "a . b . "};
    Fixture ref={0},native={0};
    ref.seed=Random();ref.trace=UINT64_C(1469598103934665603);
    strcpy(ref.source,sources[trial%(sizeof(sources)/sizeof(*sources))]);
    if(trial>=64 && trial%17==0) { memset(ref.source,'a',220);ref.source[220]=0; }
    if(trial>=64 && trial%19==0) { for(unsigned i=0;i<180;i+=4) memcpy(ref.source+i,"a . ",4);ref.source[180]='b'; }
    strcpy(ref.tr.phonemes_repeat,(trial%2)?"abcdefg":"");ref.tr.phonemes_repeat_count=(int)(Random()%6);
    ref.tr.langopts.textmode=Random()&1;
    native=ref;ref.cursor=ref.source;native.cursor=native.source;
    unsigned flags_ref[2]={Random(),Random()},flags_native[2];memcpy(flags_native,flags_ref,sizeof(flags_ref));
    /* The actual query primitive may overwrite all flags, or retain them on a miss. */
    char expected[200],actual[200];memset(expected,0x97,sizeof(expected));memset(actual,0x97,sizeof(actual));expected[0]=actual[0]=0;
    query_end=(int)(Random()&(FLAG_ALLOW_TEXTMODE|FLAG_SUFX_E_ADDED|SUFX_D));
    WORD_TAB words[2]={{0}};query_words=(trial&1)?words:NULL;query_remaining=query_words?2:0;
    size_t capacity=10+Random()%191;
    option_phonemes=trial<64?espeakPHONEMES_TRACE:0;
    dictionary_skipwords=77;f_trans=reference_trace;
    int wanted=ReferenceList(&ref.tr,&ref.cursor,expected,flags_ref,query_end,query_words,query_remaining,capacity);
    int expected_skip=dictionary_skipwords;
    dictionary_skipwords=77;f_trans=native_trace;
    RustLookupList callbacks={&native,Byte,FullQuery,Repeat,SetRepeat,TextMode,Skip,NativeAccent,Replacement,ReplacementTrace};
    int got=espeak_rs_lookup_list(&callbacks,(unsigned)query_end,flags_native,actual,capacity);
    if(wanted!=got||strcmp(expected,actual)||memcmp(flags_ref,flags_native,sizeof(flags_ref))||expected_skip!=dictionary_skipwords||ref.trace!=native.trace||ref.calls!=native.calls||ref.tr.phonemes_repeat_count!=native.tr.phonemes_repeat_count||memcmp(ref.tr.phonemes_repeat,native.tr.phonemes_repeat,20)||ref.tr.langopts.textmode!=native.tr.langopts.textmode||(ref.cursor!=ref.source)!=(native.cursor!=native.source)||strcmp(ref.cursor,native.cursor)) {
        fprintf(stderr,"list trial=%u seed=%x source=%s capacity=%zu result=%d/%d flags=%x,%x/%x,%x repeat=%d/%d trace=%llx/%llx phon=%s/%s\n",trial,ref.seed,ref.source,capacity,wanted,got,flags_ref[0],flags_ref[1],flags_native[0],flags_native[1],ref.tr.phonemes_repeat_count,native.tr.phonemes_repeat_count,(unsigned long long)ref.trace,(unsigned long long)native.trace,expected,actual);TEST_ASSERT(false);
    }
    for(size_t i=strlen(actual)+1;i<sizeof(actual);i++) TEST_ASSERT((unsigned char)actual[i]==0x97);
    if(option_phonemes) {
        fflush(reference_trace);fflush(native_trace);rewind(reference_trace);rewind(native_trace);
        char a[2048],b[2048];size_t x=fread(a,1,sizeof(a),reference_trace),y=fread(b,1,sizeof(b),native_trace);
        TEST_ASSERT(x==y&&x<sizeof(a)&&memcmp(a,b,x)==0);
        rewind(reference_trace);rewind(native_trace);TEST_ASSERT(ftruncate(fileno(reference_trace),0)==0);TEST_ASSERT(ftruncate(fileno(native_trace),0)==0);
    }
}
int main(void)
{
    FILE *saved=f_trans,*a=tmpfile(),*b=tmpfile();TEST_ASSERT(a&&b);
    for(unsigned trial=0;trial<200000;trial++) Compare(trial,a,b);
    fclose(a);fclose(b);f_trans=saved;
    Fixture f={0};strcpy(f.source,"cat ");f.cursor=f.source;f.seed=51;
    RustLookupList callbacks={&f,Byte,Query,Repeat,SetRepeat,TextMode,Skip,NativeAccent,Replacement,ReplacementTrace};
    char out[200];unsigned flags[2]={7,8};memset(out,0x97,sizeof(out));
    TEST_ASSERT(espeak_rs_lookup_list(NULL,0,flags,out,sizeof(out))==-1);
    TEST_ASSERT(espeak_rs_lookup_list(&callbacks,0,NULL,out,sizeof(out))==-1);
    TEST_ASSERT(espeak_rs_lookup_list(&callbacks,0,flags,NULL,sizeof(out))==-1);
    TEST_ASSERT(espeak_rs_lookup_list(&callbacks,0,flags,out,0)==-1);
    TEST_ASSERT(espeak_rs_lookup_list(&callbacks,0,flags,out,201)==-1);
    callbacks.byte=NULL;TEST_ASSERT(espeak_rs_lookup_list(&callbacks,0,flags,out,sizeof(out))==-1);callbacks.byte=Byte;
    f.bad=1;TEST_ASSERT(espeak_rs_lookup_list(&callbacks,0,flags,out,sizeof(out))==-1);
    f.bad=2;TEST_ASSERT(espeak_rs_lookup_list(&callbacks,0,flags,out,3)==-1);
    TEST_ASSERT(flags[0]==7&&flags[1]==8);for(unsigned i=0;i<sizeof(out);i++) TEST_ASSERT((unsigned char)out[i]==0x97);
    printf("200000 dictionary-list comparisons passed, including reduced publication capacities, callback state and traces; malformed callback/argument/output guards passed\n");
    return 0;
}
