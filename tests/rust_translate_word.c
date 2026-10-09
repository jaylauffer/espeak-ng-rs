/* Main word policy against the unchanged retained C controller. Primitive
 * mocks deliberately mutate live owners and log call/source/output ordering.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <ctype.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wctype.h>
#include <espeak-ng/speak_lib.h>
#include "translate.h"
#include "translateword.h"
#include "dictionary.h"
#include "numbers.h"
#include "phoneme.h"
#include "common.h"
#include "readclause.h"
#include "rust_translate_word.h"
typedef struct {
    Translator tr;
    WORD_TAB rows[3];
    char source[240], output[200], text[161];
    unsigned scenario, seed, lists, rules, removes, letters;
    uint64_t trace;
    int skip;
} Fixture;
static Fixture *active;
static void Trace(unsigned value) { active->trace=(active->trace^value)*UINT64_C(1099511628211); }
static void Bytes(const char *s) { for (const unsigned char *p=(const unsigned char *)s; *p; p++) Trace(*p); Trace(0); }
static void Start(unsigned call, const char *source) { Trace(call); if(source) Bytes(source); }
static void Pron(char *out, const char *text) { strcpy(out,text); }
static int MockList(Translator *tr,char **source,char *phonemes,unsigned *flags,int ending,WORD_TAB *rows,int remaining,size_t capacity)
{
    (void)rows;(void)remaining;TEST_ASSERT(capacity==200);
    Start(1,*source);Trace((unsigned)ending);active->lists++;
    unsigned mode=active->scenario;
    flags[0]=(active->seed&1?FLAG_ALT_TRANS:0)|(active->seed&2?FLAG_UNSTRESS_END:0)|(active->seed&4?FLAG_PAUSE1:0);
    flags[1]=(active->seed>>3)&0x10f;
    /* A nested primitive may mutate owner output/expectations even when the
     * parent works on a distinct local pronunciation scratch. */
    if(active->seed&8) Pron(active->output,"list-owner");
    tr->expect_verb_s=(int)(active->seed%4);
    if(mode==0 || mode==10) { Pron(phonemes,"dict"); return 1; }
    if(mode==2) {
        memcpy(tr->rust_list_replacement,"\0 replacement text ",19);
        *source=tr->rust_list_replacement+2;flags[0]|=FLAG_TEXTMODE;return 0;
    }
    if(mode==3 || (mode==19&&ending==FLAG_SUFX)) {Pron(phonemes,"\025fr");return 0;}
    if(mode==15 && ending==FLAG_SUFX) {Pron(phonemes,"stem");return 1;}
    if(mode==22) { flags[0]|=FLAG_SKIPWORDS;dictionary_skipwords=1; }
    return 0;
}
static bool MockEmoji(Translator *tr,char **source,unsigned *flags,WORD_TAB *rows,int remaining)
{
    (void)tr;(void)source;(void)flags;(void)rows;(void)remaining;Start(2,*source);return false;
}
static int MockDotted(char *source) {Start(3,source);if(active->scenario==23) {dictionary_skipwords=1;return 1;}return 0;}
static int MockNamed(Translator *tr,const char *name,char *out,size_t capacity)
{
    TEST_ASSERT(capacity==200);Start(4,name);Pron(out,active->scenario==6?"\025fr":"number-owner");
    if(active->seed&16) tr->expect_noun=4;
    return 1;
}
static int MockNumber(Translator *tr,char *source,char *out,char *end,unsigned *flags,WORD_TAB *rows,int remaining,int control)
{
    (void)tr;(void)rows;(void)remaining;TEST_ASSERT(end==out+200&&control==0);Start(5,source);flags[0]|=FLAG_ABBREV;Pron(out,"number");return 1;
}
static int MockRoman(Translator *tr,char *source,char *out,char *end,WORD_TAB *rows,int remaining)
{
    (void)tr;(void)rows;(void)remaining;TEST_ASSERT(end==out+200);Start(6,source);Pron(out,"roman");return active->scenario==9;
}
static char *MockSpell(Translator *tr,char *source,char *out,int mode,const ALPHABET *alphabet,char *owner)
{
    (void)tr;(void)alphabet;Start(7,source);Trace((unsigned)mode);Pron(owner,"spell-owner");
    if(active->scenario==5) {Pron(owner,"\025fr");return NULL;}
    Pron(out,"spelled");while(*source!=0&&*source!=' ') source++;return source;
}
static int MockLetter(Translator *tr,char *source,char *out,int non_initial,const ALPHABET *alphabet)
{
    (void)tr;(void)alphabet;Start(8,source);Trace((unsigned)non_initial);active->letters++;strcat(out,"letter");return 1;
}
static int MockUnpronounceable(Translator *tr,char *source,int position)
{
    (void)tr;Start(9,source);Trace((unsigned)position);return active->scenario==11&&position==0;
}
static void MockSpellingStress(Translator *tr,char *out,int control,int count)
{
    (void)tr;TEST_ASSERT(control==0);Start(10,out);Trace((unsigned)count);
}
static size_t WordLength(const char *s) {size_t n=0;while(s[n]&&s[n]!=' ')n++;return n;}
static int MockRules(Translator *tr,char *source,char *out,int capacity,char *ending,int word_flags,unsigned *flags)
{
    TEST_ASSERT(capacity==200);Start(11,source);Trace((unsigned)word_flags);Trace(ending!=NULL);active->rules++;
    if(active->seed&32) Pron(active->output,"rules-owner");
    tr->expect_past=(int)(active->seed%5);
    flags[0]^=active->seed&64?FLAG_STRESS_END:0;
    Pron(out,"rules");if(ending) Pron(ending,"");
    unsigned mode=active->scenario;
    if(mode==30) {
        if(active->rules==1) {if(ending) Pron(ending,"s");return SUFX_M|1;}
        if(active->rules==2) return SUFX_M|SUFX_V;
        return 0;
    }
    if(mode==24) {out[0]=0;return 0;}
    if(mode==20&&ending==NULL) {Pron(out,"\025de");return 0;}
    if(mode==21&&active->rules>2) {Pron(out,"\025de");return 0;}
    size_t count=WordLength(source);
    if(mode==12||mode==13||mode==21||mode==27||mode==28) {
        if(word_flags&FLAG_NO_PREFIX) return 0;
        if(count>5) {if(ending) Pron(ending,"\006p\006p");return SUFX_P|SUFX_V|2|(mode==13?SUFX_B|1:0);}
    }
    if(mode==14) {
        if(word_flags&FLAG_NO_PREFIX) {if(ending) Pron(ending,"s");Pron(out,"probe");return 1;}
        if(active->rules==1) {if(ending) Pron(ending,"p");return SUFX_P|1;}
        return 0;
    }
    if(mode>=15&&mode<=20&&count>3&&ending!=NULL) {
        Pron(ending,"suffix");
        return 1|SUFX_F|(mode==16?SUFX_Q:0)|(mode==17?SUFX_M|SUFX_A:0)|(mode==18?SUFX_T:0);
    }
    return 0;
}
static int MockRemove(Translator *tr,char *source,int ending,char *copy)
{
    Start(12,source);Trace((unsigned)ending);Trace(copy!=NULL);active->removes++;
    if(copy) {size_t n=WordLength(source);TEST_ASSERT(n<160);memcpy(copy,source,n);copy[n]=0;}
    size_t n=WordLength(source);TEST_ASSERT(n>0);
    if(ending&0x3f) source[n-1]=' ';
    if((ending&SUFX_V)&&tr->expect_verb==0) tr->expect_verb=1;
    return FLAG_SUFX;
}
static void MockDecode(const char *out,char *decoded) {Start(13,out);Pron(decoded,"decoded");}
static void MockAppend(Translator *tr,char *out,int capacity,const char *suffix)
{
    (void)tr;TEST_ASSERT(capacity==200);Start(14,out);Bytes(suffix);strncat(out,suffix,199-strlen(out));
}
static void MockPlural(int flags,Translator *tr,char last,char *owner)
{
    (void)tr;Start(15,owner);Trace((unsigned)flags);Trace((unsigned char)last);if(active->seed&128) Pron(owner,"plural-owner");
}
static void MockStress(Translator *tr,char *out,unsigned *flags,int position,int control)
{
    Start(16,out);Trace((unsigned)position);Trace((unsigned)control);Trace(out==active->output);
    if(out[0])out[0]='S';if(active->seed&256) flags[0]|=FLAG_ALT2_TRANS;
    tr->expect_verb=(int)(active->seed%4);
}
static void MockChange(Translator *tr,char *out,int level) {(void)tr;Start(17,out);Trace((unsigned)level);if(out[0]) out[0]=(char)('0'+level);}
static void MockSpecial(Translator *tr,char *out,int flags) {(void)tr;Start(18,out);Trace((unsigned)flags);if(out[0])out[0]='A';}
#define LookupDictListBounded MockList
#define LookupEmojiBaseSequence MockEmoji
#define CheckDottedAbbrev MockDotted
#define LookupBounded MockNamed
#define TranslateNumber MockNumber
#define TranslateRoman MockRoman
#define SpeakIndividualLetters MockSpell
#define TranslateLetter MockLetter
#define Unpronouncable MockUnpronounceable
#define SetSpellingStress MockSpellingStress
#define TranslateRules MockRules
#define RemoveEnding MockRemove
#define DecodePhonemes MockDecode
#define AppendPhonemes MockAppend
#define addPluralSuffixes MockPlural
#define SetWordStress MockStress
#define ChangeWordStress MockChange
#define ApplySpecialAttribute2 MockSpecial
#define TranslateWord3 ReferenceWord
#include "retained_main_word_translation.inc"
#undef TranslateWord3
#define TranslateWord3 NativeWord
#include "native_main_word_projection.inc"
#undef TranslateWord3
static void Activate(Fixture *f)
{
    active=f;translator=&f->tr;dictionary_skipwords=f->skip;
    f->tr.rule_text_base=f->source;f->tr.rule_text_length=sizeof(f->source);
}
static int FailedList(void *opaque,RustWordSource *source,char *out,unsigned *flags,int ending,int *found)
{
    (void)out;(void)flags;(void)ending;(void)found;
    RustWordHost *host=opaque;char *p=WordPointer(host,*source);TEST_ASSERT(p!=NULL);
    *p='!';Pron(host->output,"failed-owner");dictionary_skipwords=9;return -1;
}
int main(void)
{
    f_trans=tmpfile();TEST_ASSERT(f_trans!=NULL);
    unsigned long prefixes=0,suffixes=0,letters=0;
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture old={0},native;old.scenario=trial%31;old.seed=trial*12345u+17;
        old.trace=UINT64_C(1469598103934665603);old.skip=7;
        char *word=old.source+3;memcpy(old.source,"  \001",3);
        Pron(word,old.scenario>=6&&old.scenario<=8?"12345 ":old.scenario==23?"a . b ":old.scenario==24?"x ":old.scenario==26?" ":"abcdefgh . next ");
        if(old.scenario==27) {memset(word,'a',136);Pron(word+136," . next ");}
        if(old.scenario==28) Pron(word,"éю你abcdef . next ");
        if(old.scenario==29) Pron(word,"é ");
        memset(old.output,0x5a,200);Pron(old.output,"initial-owner");memset(old.text,0x3b,161);old.text[0]=0;
        old.tr.data_dictlist=old.scenario==25?NULL:(char *)(uintptr_t)1;
        old.tr.translator_name=trial%3?L('e','n'):L('f','r');
        old.tr.langopts.numbers=old.scenario==9?NUM_ROMAN:0;
        old.tr.langopts.numbers2=old.scenario==7?NUM2_ENGLISH_NUMERALS:0;
        old.tr.langopts.param[LOPT_PREFIXES]=(int)(trial%2);old.tr.langopts.param[LOPT_ALT]=2;
        old.tr.langopts.stress_flags=S_HYPEN_UNSTRESS;
        old.tr.clause_lower_count=7;old.tr.clause_upper_count=2;
        old.tr.expect_verb=old.scenario==30?0:2;old.tr.expect_verb_s=3;old.tr.expect_noun=2;old.tr.expect_past=1;
        old.rows[0].flags=(trial&1?FLAG_LAST_WORD:0)|(trial&2?FLAG_HYPHEN:0)|(trial&4?FLAG_FOCUS:0)|(old.scenario==10?FLAG_ALL_UPPER:0);
        old.rows[1].flags=FLAG_LAST_WORD;
        option_sayas=old.scenario==4||old.scenario==5?0x12:trial%43==0?SAYAS_KEY:0;
        option_tone_flags=old.scenario==10?OPTION_EMPHASIZE_ALLCAPS:0;
        option_phonemes=espeakPHONEMES_TRACE;
        native=old;bool any=false;bool *present=trial%2?&any:NULL;
        Activate(&old);int expected=ReferenceWord(&old.tr,word,old.rows,3,old.text,present,NULL,old.output,200);old.skip=dictionary_skipwords;
        Activate(&native);int actual=NativeWord(&native.tr,native.source+3,native.rows,3,native.text,present,NULL,native.output,200);native.skip=dictionary_skipwords;
        old.tr.rule_text_base=native.tr.rule_text_base=NULL;
        if(actual!=expected||old.trace!=native.trace||memcmp(old.source,native.source,sizeof(old.source))||memcmp(old.output,native.output,200)||memcmp(old.text,native.text,161)||memcmp(&old.tr,&native.tr,sizeof(old.tr))||old.skip!=native.skip) {
            fprintf(stderr,"word mismatch trial=%u mode=%u flags=%x/%x trace=%llu/%llu calls list=%u/%u rules=%u/%u output=%s/%s skip=%d/%d\n",trial,old.scenario,actual,expected,(unsigned long long)native.trace,(unsigned long long)old.trace,native.lists,old.lists,native.rules,old.rules,native.output,old.output,native.skip,old.skip);return 1;
        }
        prefixes+=(old.scenario>=12&&old.scenario<=14)||old.scenario==27||old.scenario==28?old.rules:0;
        suffixes+=old.removes;letters+=old.letters;
    }
    /* Public ABI refuses absent owners/callbacks without changing the result. */
    unsigned result=0xabcdef;RustTranslateWord absent={0};
    TEST_ASSERT(espeak_rs_translate_word(NULL,&result)==-1&&result==0xabcdef);
    TEST_ASSERT(espeak_rs_translate_word(&absent,&result)==-1&&result==0xabcdef);
    Fixture f={0};RustWordHost host={.tr=&f.tr,.rows=f.rows,.output=f.output};
    f.tr.data_dictlist=(char *)(uintptr_t)1;
    Pron(f.output,"guard-owner");dictionary_skipwords=7;
    RustTranslateWord table={&host,WordByte,WordWrite,WordValue,WordStore,WordLocale,
        WordList,WordEmoji,WordText,WordDotted,WordNumberLanguage,WordNumber,WordSpell,
        WordLetter,WordUnpronounceable,WordSpellingStress,WordRules,WordRemove,WordPrefix,
        WordTraceSuffix,WordAppend,WordPlural,WordStress,WordSnapshot,WordPublish,WordChangeStress,WordSpecial};
    TEST_ASSERT(espeak_rs_translate_word(&table,&result)==-1&&result==0xabcdef);
    TEST_ASSERT(dictionary_skipwords==0&&!strcmp(f.output,"guard-owner"));
    host.spans[0]=(RustWordSpan){f.source,f.source+3,sizeof(f.source)};
    TEST_ASSERT(WordPointer(&host,(RustWordSource){3,0})==NULL);
    TEST_ASSERT(WordPointer(&host,(RustWordSource){0,INTPTR_MIN})==NULL);
    TEST_ASSERT(WordPointer(&host,(RustWordSource){0,INTPTR_MAX})==NULL);
    TEST_ASSERT(WordPointer(&host,(RustWordSource){0,-3})==f.source);
    TEST_ASSERT(WordPointer(&host,(RustWordSource){0,-4})==NULL);
    Pron(f.source+3,"cat ");table.list=FailedList;
    TEST_ASSERT(espeak_rs_translate_word(&table,&result)==-1&&result==0xabcdef);
    TEST_ASSERT(dictionary_skipwords==9&&f.source[3]=='!'&&!strcmp(f.output,"failed-owner"));
    fclose(f_trans);f_trans=NULL;
    printf("200000 main word comparisons passed; %lu prefix rule calls, %lu suffix removals, %lu letter calls; ABI guards passed\n",prefixes,suffixes,letters);
    return 0;
}
