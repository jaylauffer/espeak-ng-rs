/* Complete rule-translation policy against unchanged retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wctype.h>
#include <limits.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "translate.h"
#include "dictionary.h"
#include "speech.h"
#include "readclause.h"
#include "rust_translate_rules.h"
typedef struct {
    Translator tr;
    char text[512], rules[1024], output[200], ending[200];
    unsigned flags[2], seed, calls, symbols, letters, appends;
    size_t capacity;
    int has_ending, bad;
    uint64_t trace;
} Fixture;
static Fixture *active;
static void Trace(Fixture *f,unsigned value) { f->trace=(f->trace^value)*UINT64_C(1099511628211); }
static void Bytes(Fixture *f,const char *s) { for(const unsigned char *p=(const unsigned char *)s;*p;p++) Trace(f,*p);Trace(f,0); }
static unsigned Hash(Fixture *f,unsigned extra) { unsigned hash=f->seed+(++f->calls)*0x12345;hash^=extra*0x478c24;return hash^(hash>>16); }
static void MockMatch(Translator *tr,char **word,char *start,int width,char *rule,MatchRecord *out,int flags,int dictionary)
{
    Fixture *f=(Fixture *)tr;Trace(f,1);Trace(f,(unsigned)(*word-start));Trace(f,(unsigned)width);Trace(f,(unsigned)flags);Trace(f,(unsigned)dictionary);Trace(f,(unsigned)tr->word_vowel_count);Trace(f,(unsigned)tr->word_stressed_count);
    if(rule==NULL) { Trace(f,0);out->points=0;(*word)++;return; }
    unsigned id=(unsigned)(rule-f->rules);Trace(f,id+1);unsigned hash=Hash(f,id+(unsigned char)**word);
    *out=(MatchRecord){.phonemes=""};
    size_t remaining=strcspn(*word," ");size_t advance=width>0?(size_t)width:1;
    if(advance>remaining) advance=remaining;
    TEST_ASSERT(advance>0);*word+=advance;
    out->points=(hash%5)==0?0:(int)(hash%50)+1;
    if(out->points>0) {
        unsigned length=(hash>>6)%7;char *phonemes=f->rules+800+((id%8)*16);
        for(unsigned i=0;i<length;i++) phonemes[i]=(char)('k'+i);
        phonemes[length]=0;out->phonemes=phonemes;
        out->end_type=(hash&128)?SUFX_UNPRON:0;
        if(hash&256) out->end_type|=(hash&512)?SUFX_P:SUFX_E;
        if(hash&1024) out->end_type|=(hash>>11)&7;
        if((hash&2048)&&**word!=' '&&**word!=0) out->del_fwd=*word;
        if(hash%19==0) { phonemes[0]=phonSWITCH;strcpy(phonemes+1,"en"); }
    }
    if(hash&4096) option_sayas^=0x10;
    if(hash&8192) tr->langopts.tone_numbers=!tr->langopts.tone_numbers;
}
static int MockSymbol(Translator *tr,const char *key,char *out,size_t capacity)
{
    Fixture *f=(Fixture *)tr;Trace(f,2);Bytes(f,key);Trace(f,(unsigned)capacity);unsigned hash=Hash(f,(unsigned char)key[1]);f->symbols++;
    unsigned length=(hash>>4)%7;for(unsigned i=0;i<length;i++) out[i]=(char)('a'+i);out[length]=0;
    if(hash&1) strcpy(f->output,"shared");
    if(hash&2) tr->word_vowel_count+=2;
    return (int)(hash|0x80000000u);
}
static void MockLetter(Translator *tr,unsigned code,int next,char *out,int control,size_t capacity)
{
    Fixture *f=(Fixture *)tr;Trace(f,3);Trace(f,code);Trace(f,(unsigned)next);Trace(f,(unsigned)control);Trace(f,(unsigned)capacity);unsigned hash=Hash(f,code);f->letters++;
    unsigned length=(hash>>4)%7;for(unsigned i=0;i<length;i++) out[i]=(char)('s'+i);out[length]=0;
    if(hash&1) strcpy(f->output,"letter");
    if(hash&2) option_phonemes^=espeakPHONEMES_TRACE;
    if(hash&4) f->flags[0]^=1;
}
static void MockAppend(Translator *tr,char *out,int capacity,const char *addition)
{
    Fixture *f=(Fixture *)tr;Trace(f,4);Trace(f,(unsigned)capacity);Bytes(f,out);Bytes(f,addition);f->appends++;
    size_t length=strlen(addition);if(strlen(out)+length>=(size_t)capacity) return;
    tr->word_vowel_count+=(int)length;tr->word_stressed_count+=(int)(length/2);strcat(out,addition);
}
static int MockPrint(FILE *file,const char *format,...)
{
    (void)file;va_list ap;va_start(ap,format);
    if(*format=='\n') Trace(active,7);else { Trace(active,*format=='U'?5:6);Bytes(active,va_arg(ap,const char *)); }
    va_end(ap);return 0;
}
#define MatchRule MockMatch
#define LookupBounded MockSymbol
#define LookupLetterBounded MockLetter
#define AppendPhonemes MockAppend
#define fprintf MockPrint
#define TranslateRules ReferenceRules
#include "rule_accent_resources_reference.inc"
#include "rule_translation_reference.inc"
#undef MatchRule
#undef LookupBounded
#undef LookupLetterBounded
#undef AppendPhonemes
#undef fprintf
#undef TranslateRules
static int Byte(void *opaque,intptr_t position) { Fixture *f=opaque;return f->bad!=3&&position>=-1&&position<509?(unsigned char)f->text[position+2]:-1; }
static int Write(void *opaque,intptr_t position,unsigned char byte) { Fixture *f=opaque;if(position< -1||position>=509) return -1;f->text[position+2]=(char)byte;return 0; }
static int Value(void *opaque,unsigned field,unsigned index)
{
    Fixture *f=opaque;Translator *tr=&f->tr;
    switch(field) {
    case 0:return tr->data_dictrules!=NULL;case 1:return tr->letter_bits_offset;case 2:return tr->langopts.tone_numbers;case 3:return option_sayas;
    case 4:return tr->langopts.param[LOPT_DIERESES];case 5:return tr->langopts.alt_alphabet;case 6:return tr->langopts.alt_alphabet_lang;
    case 7:return (option_phonemes&espeakPHONEMES_TRACE)!=0;case 8:return tr->langopts.param[LOPT_BRACKET_PAUSE_ANNOUNCED];case 9:return tr->langopts.param[LOPT_BRACKET_PAUSE];case 10:return pre_pause;
    case 11:return tr->groups2_count[index];case 12:return tr->groups2_start[index];case 13:return tr->groups2_name[index];case 14:return f->flags[0];case 15:return CHAR_MIN<0;
    default:TEST_ASSERT(false);return 0;
    }
}
static void Store(void *opaque,unsigned field,int value)
{
    Fixture *f=opaque;switch(field) {
    case 0:f->tr.word_vowel_count=value;break;case 1:f->tr.word_stressed_count=value;break;case 2:f->tr.phonemes_repeat_count=value;break;
    case 3:pre_pause=value;break;case 4:f->flags[0]=(unsigned)value;break;default:TEST_ASSERT(false);
    }
}
static int Locale(void *opaque,unsigned code,unsigned digit) { (void)opaque;return digit?iswdigit(code)!=0:iswalpha(code)!=0; }
static size_t Group(void *opaque,unsigned kind,unsigned index)
{
    Fixture *f=opaque;if(f->bad==5) return SIZE_MAX-1;char *p=kind==0?f->tr.groups3[index]:kind==1?f->tr.groups2[index]:f->tr.groups1[index];return p?(size_t)(p-f->rules):SIZE_MAX;
}
static int Matched(void *opaque,size_t group,size_t width,unsigned flags,unsigned dictionary,RustRulesMatch *out)
{
    Fixture *f=opaque;char *cursor=f->text+2+out->cursor;
    MatchRecord result={out->points,out->phonemes,out->ending,out->delete_offset==RUST_RULES_NO_DELETE?NULL:f->text+2+out->delete_offset};
    MockMatch(&f->tr,&cursor,f->text+2,(int)width,group==SIZE_MAX?NULL:f->rules+group,&result,(int)flags,(int)dictionary);
    if(f->bad==1) return 0;
    out->cursor=(size_t)(cursor-(f->text+2));out->points=result.points;out->ending=result.end_type;out->delete_offset=result.del_fwd?result.del_fwd-(f->text+2):RUST_RULES_NO_DELETE;
    if(result.phonemes!=out->phonemes) strcpy(out->phonemes,result.phonemes);
    if(f->bad==2) memset(out->phonemes,0x82,200);
    return 0;
}
static int Symbol(void *opaque,const char *key,char *out) { MockSymbol(&((Fixture *)opaque)->tr,key,out,40);return 0; }
static int Letter(void *opaque,unsigned code,char *out) { MockLetter(&((Fixture *)opaque)->tr,code,-1,out,0,160);return 0; }
static int Publish(void *opaque,unsigned kind,const char *text)
{
    Fixture *f=opaque;if(f->bad==4) return -1;const char *end=memchr(text,0,200);if(!end||(size_t)(end-text)>=(kind?200:f->capacity)) return -1;
    memcpy(kind?f->ending:f->output,text,(size_t)(end-text)+1);return 0;
}
static int HasEnding(void *opaque) { return ((Fixture *)opaque)->has_ending; }
static int Append(void *opaque,const char *text) { Fixture *f=opaque;MockAppend(&f->tr,f->output,(int)f->capacity,text);return 0; }
static void Header(void *opaque,unsigned kind,const char *text) { (void)opaque;if(kind==0) MockPrint(NULL,"Unpronouncable? '%s'\n",text);else if(kind==1) MockPrint(NULL,"Translate '%s'\n",text);else MockPrint(NULL,"\n"); }
static RustTranslateRules Table(Fixture *f) { return (RustTranslateRules){f,Byte,Write,Value,Store,Locale,Group,Matched,Symbol,Letter,Publish,HasEnding,Append,Header}; }
static unsigned seed=0x13783ad;
static unsigned Random(void) { seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed; }
static void Bind(Fixture *f)
{
    f->tr.data_dictlist=f->rules;f->tr.data_dictrules=f->rules;f->tr.data_dict_size=sizeof(f->rules);
    f->tr.groups1[0]=f->rules+1;
    for(unsigned i=1;i<256;i++) f->tr.groups1[i]=(f->seed&(1u<<(i%23)))?f->rules+16+i:NULL;
    for(unsigned i=0;i<128;i++) f->tr.groups3[i]=(f->seed&(1u<<(i%29)))?f->rules+400+i:NULL;
    f->tr.groups2_count['a']=2;f->tr.groups2_start['a']=0;f->tr.groups2_name[0]='a'+('b'<<8);f->tr.groups2_name[1]='a'+('c'<<8);
    f->tr.groups2[0]=f->rules+600;f->tr.groups2[1]=f->rules+620;
}
int main(void)
{
    const char *words[]={"abc abc ","12345 ","a123 ","ábé ","über ","áé ","!?# ","еёя ","漢🙂 ","( ","\xee\x80\xa8 ","aéefä ","αβ ","हिन्दी ","𝔄 ",""};
    unsigned symbols=0,letters=0,endings=0;
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture ref={0},native={0};ref.seed=Random();ref.trace=UINT64_C(1469598103934665603);ref.capacity=32+Random()%169;ref.has_ending=(trial%3)!=0;
        ref.text[1]=' ';strcpy(ref.text+2,words[trial%16]);ref.flags[0]=Random();ref.flags[1]=Random();
        memset(ref.output,0x97,200);strcpy(ref.output,"prefix");memset(ref.ending,0x97,200);
        ref.tr.letter_bits_offset=(trial%4)==0?0x400:0;ref.tr.langopts.tone_numbers=Random()&1;ref.tr.langopts.param[LOPT_DIERESES]=Random()&1;
        ref.tr.langopts.alt_alphabet=(trial&1)?0x3b1:0x430;ref.tr.langopts.alt_alphabet_lang=L('e','n');ref.tr.langopts.param[LOPT_BRACKET_PAUSE]=17;ref.tr.langopts.param[LOPT_BRACKET_PAUSE_ANNOUNCED]=29;
        ref.tr.word_vowel_count=13;ref.tr.word_stressed_count=8;ref.tr.phonemes_repeat_count=5;native=ref;Bind(&ref);Bind(&native);
        unsigned wordflags=((trial&1)?FLAG_NO_PREFIX:0)|((trial&2)?FLAG_UNPRON_TEST:0)|((trial&4)?FLAG_NO_TRACE:0)|((trial&8)?FLAG_DONT_SWITCH_TRANSLATOR:0);
        int sayas=(int)(Random()&0x10),trace=(int)(Random()&1)?espeakPHONEMES_TRACE:0,pause=(int)(Random()%12);
        active=&ref;option_sayas=sayas;option_phonemes=trace;pre_pause=pause;
        int wanted=ReferenceRules(&ref.tr,ref.text+2,ref.output,(int)ref.capacity,ref.has_ending?ref.ending:NULL,(int)wordflags,ref.flags);
        int saved_sayas=option_sayas,saved_trace=option_phonemes,saved_pause=pre_pause;
        active=&native;option_sayas=sayas;option_phonemes=trace;pre_pause=pause;RustTranslateRules table=Table(&native);int got=123;
        int status=espeak_rs_translate_rules(&table,wordflags,&got);
        if(status||wanted!=got||memcmp(ref.text,native.text,512)||memcmp(ref.output,native.output,200)||memcmp(ref.ending,native.ending,200)||memcmp(ref.flags,native.flags,sizeof(ref.flags))||ref.trace!=native.trace||ref.calls!=native.calls||ref.symbols!=native.symbols||ref.letters!=native.letters||ref.appends!=native.appends||ref.tr.word_vowel_count!=native.tr.word_vowel_count||ref.tr.word_stressed_count!=native.tr.word_stressed_count||ref.tr.phonemes_repeat_count!=native.tr.phonemes_repeat_count||ref.tr.langopts.tone_numbers!=native.tr.langopts.tone_numbers||saved_sayas!=option_sayas||saved_trace!=option_phonemes||saved_pause!=pre_pause) {
            fprintf(stderr,"trial=%u seed=%x word=%s flags=%x status=%d ending=%x/%x trace=%llx/%llx counts=%d,%d/%d,%d phon=%s/%s\n",trial,ref.seed,words[trial%16],wordflags,status,wanted,got,(unsigned long long)ref.trace,(unsigned long long)native.trace,ref.tr.word_vowel_count,ref.tr.word_stressed_count,native.tr.word_vowel_count,native.tr.word_stressed_count,ref.output,native.output);TEST_ASSERT(false);
        }
        symbols+=native.symbols;letters+=native.letters;endings+=got!=0;
    }
    Fixture f={0};f.seed=1;f.capacity=200;strcpy(f.text+2,"abc ");strcpy(f.output,"prefix");Bind(&f);active=&f;RustTranslateRules table=Table(&f);int result=123;
    TEST_ASSERT(espeak_rs_translate_rules(NULL,0,&result)==-1);TEST_ASSERT(espeak_rs_translate_rules(&table,0,NULL)==-1);
    table.byte=NULL;TEST_ASSERT(espeak_rs_translate_rules(&table,0,&result)==-1);table.byte=Byte;
    for(int bad=1;bad<=5;bad++) {
        f.bad=bad;f.has_ending=bad==4;unsigned calls=f.calls;
        TEST_ASSERT(espeak_rs_translate_rules(&table,0,&result)==-1);
        TEST_ASSERT(result==123&&f.calls==calls+(bad<=2));TEST_ASSERT(strcmp(f.output,"prefix")==0);
    }
    table.context=NULL;TEST_ASSERT(espeak_rs_translate_rules(&table,0,&result)==-1);table.context=&f;
    f.tr.data_dictrules=NULL;f.bad=3;f.has_ending=1;f.tr.word_vowel_count=13;f.tr.word_stressed_count=17;
    TEST_ASSERT(espeak_rs_translate_rules(&table,0,&result)==0&&result==0);
    TEST_ASSERT(f.tr.word_vowel_count==13&&f.tr.word_stressed_count==17&&strcmp(f.output,"prefix")==0);
    printf("200000 complete rule-driver comparisons passed; %u symbol calls, %u letter calls, %u ending/unpronounceable returns; source/output/state/order and malformed ABI guards passed\n",symbols,letters,endings);
    return 0;
}
