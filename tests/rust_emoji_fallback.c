/* Complete emoji fallback against the unchanged retained C policy.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "translate.h"
#include "translateword.h"
#include "dictionary.h"
#include "numbers.h"
#include "phoneme.h"
#include "common.h"
#include "readclause.h"
#include "rust_emoji_fallback.h"
#include "rust_translate_word.h"
typedef struct {
    Translator tr; WORD_TAB rows[3]; char source[620], phonemes[200];
    char *owner; unsigned flags, seed, calls, traces; uint64_t trace;
} Fixture;
static Fixture *active;
static void Trace(unsigned n) {active->trace=(active->trace^n)*UINT64_C(1099511628211);}
static void Bytes(const char *s) {for(const unsigned char *p=(const unsigned char *)s;*p;p++)Trace(*p);Trace(0);}
static int MockList(Translator *tr,char **word,char *phonemes,unsigned *flags,int ending,WORD_TAB *rows,int remaining,size_t capacity)
{
    TEST_ASSERT(ending==FLAG_ALLOW_TEXTMODE&&rows==active->rows&&remaining==(int)(active->seed%4)&&capacity==200);
    TEST_ASSERT(flags[0]==0&&flags[1]==0);
    Trace(1);Bytes(*word);Trace((unsigned)remaining);active->calls++;
    tr->expect_noun=(int)(active->seed%19)+ (int)active->calls;
    active->flags^=0x10000;memset(phonemes,0x67,200);strcpy(phonemes,"unused");
    int selected=active->calls==1?active->seed%6!=0:active->seed%5!=0;
    flags[0]=selected?FLAG_TEXTMODE:0;flags[1]=active->seed^0x1cc;
    if(active->seed%11==0) option_phonemes=espeakPHONEMES_TRACE;
    if(active->calls>1) {
        Trace(2);Bytes(active->owner+2);
        if(active->seed%7==0) {active->owner[2]='X';active->owner[101]=(char)active->calls;}
        if(active->seed%13==0) option_phonemes=0;
        if(active->seed&16) active->source[3]='!';
    }
    if(!selected) {if(active->seed%17==0) (*word)[0]='?';return 0;}
    if(active->seed%9==0) {*word+=1;return 0;}
    memset(tr->rust_list_replacement,0,160);
    char *text=tr->rust_list_replacement+2;
    size_t length=active->calls==1?(active->seed%10==0?157:active->seed%10==1?156:5):
        (active->seed%8==0?155:active->seed%8==1?150:5);
    memset(text,active->calls==1?'b':'m',length);text[length]=0;
    if(length==5) {memcpy(text,active->calls==1?"base ":"tone ",5);}
    *word=text;return 0;
}
static int MockPrint(FILE *file,const char *format,const char *source,const char *replacement)
{
    (void)file;TEST_ASSERT(strcmp(format,"Replace: %s  %s\n")==0);
    Trace(3);Bytes(source);Bytes(replacement);active->traces++;active->flags^=0x8000;return 0;
}
#define LookupDictListBounded MockList
#define fprintf MockPrint
#define LookupEmojiBaseSequence ReferenceEmoji
#include "retained_emoji_fallback.inc"
#undef LookupEmojiBaseSequence
#define LookupEmojiBaseSequence NativeEmoji
#include "native_word_source_projections.inc"
#include "native_emoji_projections.inc"
#undef LookupEmojiBaseSequence
#undef fprintf
static void Init(Fixture *f,unsigned trial,const char previous[160])
{
    memset(f,0,sizeof(*f));f->seed=trial*0xabcdefu+29;f->trace=UINT64_C(1469598103934665603);
    f->flags=0x40030011;memcpy(f->tr.rust_emoji_replacement,previous,160);memcpy(f->source,"  !",3);
}
static void Activate(Fixture *f,char *owner)
{
    active=f;f->owner=owner;translator=&f->tr;translator2=translator3=NULL;
    f->tr.rule_text_base=f->source;f->tr.rule_text_length=sizeof(f->source);
    option_phonemes=f->seed&1?espeakPHONEMES_TRACE:0;
}
static void Append(char **p,unsigned code) {*p+=utf8_out((int)code,*p);}
static void Compare(Fixture *old,Fixture *native,unsigned trial,int expected,int actual,int trace_mode_old,int trace_mode_native)
{
    old->tr.rule_text_base=native->tr.rule_text_base=NULL;
    if(expected!=actual||old->flags!=native->flags||old->calls!=native->calls||old->traces!=native->traces||old->trace!=native->trace||trace_mode_old!=trace_mode_native||memcmp(old->source,native->source,620)||memcmp(&old->tr,&native->tr,sizeof(old->tr))) {
        fprintf(stderr,"emoji mismatch trial=%u return=%d/%d flags=%x/%x calls=%u/%u traces=%u/%u hash=%llu/%llu\n",trial,actual,expected,native->flags,old->flags,native->calls,old->calls,native->traces,old->traces,(unsigned long long)native->trace,(unsigned long long)old->trace);exit(1);
    }
}
static int FailWrite(void *opaque,size_t offset,const char *bytes,size_t length)
{
    EmojiWrite(opaque,offset,bytes,length);return -1;
}
int main(void)
{
    char previous[160]={0};Fixture warm;Init(&warm,0,previous);
    /* Seed the unchanged function's static owner, then preserve it across all
     * episodes. Normalize only its location into the native translator field. */
    warm.seed=3;strcpy(warm.source+3,"👩🏽 ");Activate(&warm,previous);
    char *word=warm.source+3;TEST_ASSERT(ReferenceEmoji(&warm.tr,&word,&warm.flags,warm.rows,3));
    char *reference_owner=word-2;memcpy(previous,reference_owner,160);
    unsigned long calls=0,traces=0,successes=0,misses=0;
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture old,native;Init(&old,trial,previous);char *p=old.source+3;
        unsigned mode=trial%18;
        if(mode==0) {strcpy(p,"text ");p+=5;}
        else {
            if(mode!=2) Append(&p,mode==1?0x1f642:mode==4?0x1f3f4:0x1f469);
            if(mode>=6) for(unsigned i=0;i<(trial/18)%65;i++) Append(&p,trial&4?0x1f52c:'x');
            if(mode!=1&&mode!=4) for(unsigned i=0;i<1+(trial/1170)%13;i++) Append(&p,0x1f3fb+(i%5));
            if(mode==4||mode>=8) for(unsigned i=0;i<1+(trial/15210)%5;i++) Append(&p,0xe0067+i);
            if(mode==5) {Append(&p,0x200d);Append(&p,0x1f52c);}
            if(trial&2) {*p++=' ';memcpy(p,"tail",4);p+=4;}
        }
        *p=0;native=old;
        Activate(&old,reference_owner);word=old.source+3;
        int expected=ReferenceEmoji(&old.tr,&word,&old.flags,old.rows,(int)(old.seed%4));
        TEST_ASSERT(word==(expected?reference_owner+2:old.source+3));
        memcpy(old.tr.rust_emoji_replacement,reference_owner,160);memcpy(previous,reference_owner,160);
        int old_mode=option_phonemes;
        Activate(&native,native.tr.rust_emoji_replacement);word=native.source+3;
        int actual=NativeEmoji(&native.tr,&word,&native.flags,native.rows,(int)(native.seed%4));
        TEST_ASSERT(word==(actual?native.tr.rust_emoji_replacement+2:native.source+3));
        Compare(&old,&native,trial,expected,actual,old_mode,option_phonemes);
        calls+=old.calls;traces+=old.traces;successes+=expected!=0;misses+=expected==0;
    }
    Fixture first,second;Init(&first,0,previous);Init(&second,0,previous);
    first.seed=3;second.seed=1;strcpy(first.source+3,"👩🏽 ");strcpy(second.source+3,"👩🏽 ");
    Activate(&first,first.tr.rust_emoji_replacement);char *first_word=first.source+3;
    TEST_ASSERT(NativeEmoji(&first.tr,&first_word,&first.flags,first.rows,3));
    char first_saved[160];memcpy(first_saved,first.tr.rust_emoji_replacement,160);
    Activate(&second,second.tr.rust_emoji_replacement);char *second_word=second.source+3;
    TEST_ASSERT(NativeEmoji(&second.tr,&second_word,&second.flags,second.rows,1));
    TEST_ASSERT(first_word==first.tr.rust_emoji_replacement+2&&second_word==second.tr.rust_emoji_replacement+2);
    TEST_ASSERT(memcmp(first_saved,first.tr.rust_emoji_replacement,160)==0);
    TEST_ASSERT(strcmp(first_word,second_word)!=0);
    Fixture f;Init(&f,0,previous);f.seed=3;strcpy(f.source+3,"👩🏽 ");Activate(&f,f.tr.rust_emoji_replacement);
    f.tr.rust_emoji_replacement[0]=f.tr.rust_emoji_replacement[1]='Q';
    word=f.source+3;RustEmojiHost host={.source={.tr=&f.tr,.rows=f.rows,.remaining=3},.wordptr=&word};
    TEST_ASSERT(WordWindow(&host.source,word)==0);
    RustEmojiFallback callbacks={&host,EmojiByte,EmojiList,FailWrite,EmojiTracing,EmojiSnapshot,EmojiTrace,EmojiFinish};
    unsigned flags=123;TEST_ASSERT(espeak_rs_lookup_emoji(&callbacks,&flags)==-1&&flags==123&&word==f.source+3);
    TEST_ASSERT(f.calls==1&&f.tr.rust_emoji_replacement[0]==0&&f.tr.rust_emoji_replacement[1]==' ');
    TEST_ASSERT(espeak_rs_lookup_emoji(NULL,&flags)==-1&&flags==123);
    callbacks.list=NULL;TEST_ASSERT(espeak_rs_lookup_emoji(&callbacks,&flags)==-1&&flags==123);
    printf("200000 emoji comparisons passed; %lu list calls, %lu traces, %lu replacements, %lu refusals; fresh flags/owner effects and ABI guards passed\n",calls,traces,successes,misses);
    return 0;
}
