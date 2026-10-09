/* Complete isolated-character control against unchanged retained C policy.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
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
#include "voice.h"
#include "rust_translate_letter.h"
typedef struct {
    Translator owners[3]; char output[200]; unsigned seed,calls,names,letters,setups,restores,hanguls;
    uint64_t trace; int primary,bad;
} Fixture;
static Fixture *active;
static void Trace(unsigned value) { active->trace=(active->trace^value)*UINT64_C(1099511628211); }
static void Bytes(const char *s) { for(const unsigned char *p=(const unsigned char *)s;*p;p++) Trace(*p);Trace(0); }
static unsigned Hash(unsigned value) { unsigned h=active->seed+(++active->calls)*0x12345;h^=value*0x478c24;return h^(h>>16); }
static unsigned Owner(Translator *tr) { TEST_ASSERT(tr!=NULL);return (unsigned)(tr-active->owners); }
static int MockName(Translator *tr,const char *key,char *out,size_t capacity)
{
    Trace(1);Trace(Owner(tr));Bytes(key);Trace((unsigned)capacity);active->names++;
    unsigned sum=0;for(const unsigned char *p=(const unsigned char *)key;*p;p++) sum=sum*31+*p;
    unsigned hash=Hash(sum+Owner(tr));size_t length=(hash&3)?(hash%4)+3:0;
    if(length>=capacity) length=capacity-1;
    for(size_t i=0;i<length;i++) out[i]=(char)('N'+i);out[length]=0;
    if(hash&4) strcpy(active->output,"named");
    if(hash&8) tr->langopts.accents^=2;
    return length!=0?(int)(hash|1u):0;
}
static void MockLetter(Translator *tr,int code,int next,char *out,int control,size_t capacity)
{
    Trace(2);Trace(Owner(tr));Trace((unsigned)code);Trace((unsigned)next);Trace((unsigned)control);Trace((unsigned)capacity);active->letters++;
    unsigned hash=Hash((unsigned)code+Owner(tr));size_t length=(hash%3)?(hash%4)+1:0;
    if(length>=capacity) length=capacity-1;
    for(size_t i=0;i<length;i++) out[i]=(char)('a'+i);out[length]=0;
    if(length>=3&&hash%11==0) {out[0]=phonSWITCH;out[1]='f';out[2]='r';out[3]=0;}
    if(hash&1) strcpy(active->output,"letter");
    if(hash&2) tr->langopts.accents^=2;
}
static int MockSetup(const char *name)
{
    Trace(3);Bytes(name);active->setups++;unsigned sum=0;for(const unsigned char *p=(const unsigned char *)name;*p;p++) sum=sum*31+*p;
    unsigned hash=Hash(sum);translator3=&active->owners[2];translator3->phoneme_tab_ix=hash%8;return translator3->phoneme_tab_ix;
}
static void MockRestore(int table) {Trace(4);Trace((unsigned)table);active->restores++;}
static int MockRules(Translator *tr,char *source,char *out,int capacity,char *ending,int flags,unsigned *dictionary)
{
    TEST_ASSERT(ending==NULL&&flags==0&&dictionary==NULL);Trace(5);Trace(Owner(tr));Bytes(source);Trace((unsigned)capacity);active->hanguls++;Hash(17);strcpy(out,"han");return 0;
}
static void MockStress(Translator *tr,char *out,unsigned *flags,int stress,int control)
{
    TEST_ASSERT(flags==NULL&&stress==-1&&control==0);Trace(6);Trace(Owner(tr));Bytes(out);out[0]='H';tr->word_stressed_count++;
}
static void MockEncode(const char *text,char *out,int *bad) {TEST_ASSERT(bad==NULL);Trace(7);Bytes(text);Hash(31);strcpy(out,"enc");}
#define LookupBounded MockName
#define LookupLetterBounded MockLetter
#define SetTranslator3 MockSetup
#define SelectPhonemeTable MockRestore
#define TranslateRules MockRules
#define SetWordStress MockStress
#define EncodePhonemes MockEncode
#define TranslateLetter ReferenceLetter
#include "isolated-letter_resources_reference.inc"
#include "non-ASCII_digit_reference.inc"
#include "isolated-letter_translation_reference.inc"
#undef LookupBounded
#undef LookupLetterBounded
#undef SetTranslator3
#undef SelectPhonemeTable
#undef TranslateRules
#undef SetWordStress
#undef EncodePhonemes
#undef TranslateLetter
static int Value(void *opaque,unsigned field)
{
    Fixture *f=opaque;Translator *tr=&f->owners[0];
    switch(field) {
    case 0:return translator->phoneme_tab_ix;case 1:return translator==tr;case 2:return translator->letter_bits_offset;
    case 3:return translator->langopts.alt_alphabet;case 4:return translator->langopts.our_alphabet;case 5:return tr->translator_name;
    case 6:return tr->phoneme_tab_ix;case 7:return tr->langopts.accents;case 8:return tr->langopts.dotless_i;case 9:return translator3!=NULL;
    case 10:return translator->langopts.alt_alphabet_lang;default:TEST_ASSERT(false);return 0;
    }
}
static int Classify(void *opaque,unsigned code,unsigned kind) {(void)opaque;return kind==0?iswupper(code)!=0:kind==1?iswalpha(code)!=0:iswspace(code)!=0;}
static Translator *Get(Fixture *f,unsigned which) {return which==0?&f->owners[0]:which==1?translator:translator3;}
static int Named(void *opaque,unsigned which,const char *key,size_t capacity,char *out,int *flags) {*flags=MockName(Get(opaque,which),key,out,capacity);return 0;}
static int Letter(void *opaque,unsigned which,unsigned code,int next,unsigned control,size_t capacity,char *out)
{
    Fixture *f=opaque;MockLetter(Get(f,which),(int)code,next,out,(int)control,capacity);
    if(f->bad==1) memset(out,1,200);return 0;
}
static int Setup(void *opaque,const char *name) {(void)opaque;return MockSetup(name);}
static void Restore(void *opaque) {(void)opaque;MockRestore(voice->phoneme_tab_ix);}
static int Hangul(void *opaque,char *source,char *out) {(void)opaque;MockRules(translator3,source+1,out,77,NULL,0,NULL);MockStress(translator3,out,NULL,-1,0);return 0;}
static int Encode(void *opaque,const char *text,char *out) {(void)opaque;MockEncode(text,out,NULL);return 0;}
static int Publish(void *opaque,unsigned replace,const char *text)
{
    Fixture *f=opaque;if(f->bad==2) return -1;
    const char *end=memchr(text,0,200);TEST_ASSERT(end!=NULL);size_t len=(size_t)(end-text),used=strlen(f->output);
    if(replace) memcpy(f->output,text,len+1);else if(used+len<200) memcpy(f->output+used,text,len+1);return 0;
}
static RustTranslateLetter Callbacks(Fixture *f) {return (RustTranslateLetter){f,Value,Classify,Named,Letter,Setup,Restore,Hangul,Encode,Publish};}
static void Activate(Fixture *f) {active=f;translator=&f->owners[f->primary];translator3=&f->owners[2];}
int main(void)
{
    static const unsigned characters[]={0,'I','A','z',0xe041,0xe028,0xe9,0x131,0x2074,0x2082,0x2099,0x661,0x6f9,0xbe9,0xe54,0x1095,0x416,0x3a9,0x54e,0x5d0,0x620,0x915,0x1100,0xac00,0xc544,0xd7af,0x2800,0x2805,0x28ff,0x3100,0x4e2d,0x10450,0x1f600,0x10ffff,0x1fffff};
    static const int languages[]={0x656e,0x6875,0x6b6f,0x6672,0x7461};
    voice_t saved_voice={0};saved_voice.phoneme_tab_ix=7;voice=&saved_voice;
    unsigned long names=0,letters=0,setups=0,hanguls=0,switches=0;
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture old={0},native;old.seed=trial*0xabcdefu+17;old.trace=UINT64_C(1469598103934665603);old.primary=(int)(trial%2);
        memset(old.output,0x5a,sizeof(old.output));size_t prefix=trial%23==0?195:trial%5;memset(old.output,'p',prefix);old.output[prefix]=0;
        for(unsigned i=0;i<3;i++) {old.owners[i].translator_name=languages[(trial+i)%5];old.owners[i].phoneme_tab_ix=(trial+i)%8;old.owners[i].langopts.accents=trial%4;old.owners[i].langopts.dotless_i=trial%3==0;}
        Translator *primary=&old.owners[old.primary];primary->letter_bits_offset=trial%7==0?0x400:0;
        primary->langopts.alt_alphabet=trial%11==0?0xa700:0;primary->langopts.alt_alphabet_lang=0x6b6f;primary->langopts.our_alphabet=trial%13==0?0x2800:0;
        unsigned code=characters[trial%(sizeof(characters)/sizeof(*characters))];char word[8]={0};int consumed=utf8_out((int)code,word);if(code==0) consumed=1;
        word[consumed]=trial%3==0?(char)0xff:' ';word[consumed+1]=0;
        const ALPHABET *current=trial%4==0?AlphabetFromChar(code):NULL;unsigned current_first=current?current->range_min:UINT32_MAX;
        native=old;Activate(&old);int expected=ReferenceLetter(&old.owners[0],word,old.output,(int)(trial%8),current);
        Activate(&native);RustTranslateLetter table=Callbacks(&native);int actual=123;
        int decoded;int width=utf8_in(&decoded,word);
        int status=espeak_rs_translate_letter(&table,(unsigned)decoded,word[width],trial%8,current_first,width,&actual);
        if(status!=0||actual!=expected||memcmp(old.output,native.output,200)||old.trace!=native.trace||memcmp(old.owners,native.owners,sizeof(old.owners))) {
            fprintf(stderr,"isolated driver mismatch trial=%u code=%x status=%d result=%d/%d trace=%llu/%llu calls=%u/%u\n",trial,code,status,actual,expected,(unsigned long long)native.trace,(unsigned long long)old.trace,native.calls,old.calls);return 1;
        }
        TEST_ASSERT(old.calls==native.calls&&old.names==native.names&&old.letters==native.letters&&old.setups==native.setups&&old.restores==native.restores&&old.hanguls==native.hanguls);
        names+=native.names;letters+=native.letters;setups+=native.setups;hanguls+=native.hanguls;switches+=actual==0;
    }
    Fixture f={0};f.owners[0].translator_name=0x656e;Activate(&f);RustTranslateLetter table=Callbacks(&f);int result=123;
    TEST_ASSERT(espeak_rs_translate_letter(NULL,'a',32,0,UINT32_MAX,1,&result)==-1&&result==123);
    TEST_ASSERT(espeak_rs_translate_letter(&table,'a',32,0,UINT32_MAX,0,&result)==-1&&result==123);
    TEST_ASSERT(espeak_rs_translate_letter(&table,'a',32,0,UINT32_MAX,1,NULL)==-1);
    table.context=NULL;TEST_ASSERT(espeak_rs_translate_letter(&table,'a',32,0,UINT32_MAX,1,&result)==-1&&result==123);
    table=Callbacks(&f);table.letter=NULL;TEST_ASSERT(espeak_rs_translate_letter(&table,'a',32,0,UINT32_MAX,1,&result)==-1&&result==123);
    table=Callbacks(&f);f.bad=1;TEST_ASSERT(espeak_rs_translate_letter(&table,'a',32,0,UINT32_MAX,1,&result)==-1&&result==123&&f.letters==1);
    f=(Fixture){0};f.owners[0].translator_name=0x656e;f.bad=2;Activate(&f);table=Callbacks(&f);
    TEST_ASSERT(espeak_rs_translate_letter(&table,'a',32,0,UINT32_MAX,1,&result)==-1&&result==123);
    printf("200000 isolated-letter driver comparisons passed; %lu named, %lu letter, %lu secondary, %lu Hangul calls, %lu early switches; output/state/order and malformed ABI guards passed\n",names,letters,setups,hanguls,switches);
    return 0;
}
