/* Actual rule-host projections against the previous production C host.
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
static void FixtureMatch(Translator *tr,char **word,char *start,int width,char *rule,MatchRecord *out,int flags,int dictionary,size_t length)
{
    Fixture *f=(Fixture *)tr;char *scratch=out->phonemes;
    Trace((Fixture *)tr, (unsigned)length);
    MockMatch(tr,word,start,width,rule,out,flags,dictionary);
    switch(f->bad) {
    case 1: *word=f->text+512;break;
    case 2: out->del_fwd=(char *)((uintptr_t)f->text-1);break;
    case 3: out->phonemes="q";break;
    case 4: memset(f->rules+824,'q',200);out->phonemes=f->rules+824;break;
    case 5: strcpy(scratch,"xyz");out->phonemes=scratch;break;
    case 6: out->phonemes="";break;
    case 7: out->del_fwd=start-1;break;
    case 8: out->del_fwd=start+1;break;
    case 9: memset(scratch,'q',200);out->phonemes=scratch;break;
    case 10: out->phonemes=NULL;break;
    case 11: *word=start-1;break;
    case 12: out->del_fwd=f->text+512;break;
    }
}
#define espeak_rs_match_rule FixtureMatch
#define LookupBounded MockSymbol
#define LookupLetterBounded MockLetter
#define AppendPhonemes MockAppend
#define fprintf MockPrint
#define RustRulesHost PreviousRustRulesHost
#define RulesPointer PreviousRulesPointer
#define RulesByte PreviousRulesByte
#define RulesWrite PreviousRulesWrite
#define RulesValue PreviousRulesValue
#define RulesStore PreviousRulesStore
#define RulesLocale PreviousRulesLocale
#define RulesGroup PreviousRulesGroup
#define RulesMatched PreviousRulesMatched
#define RulesSymbol PreviousRulesSymbol
#define RulesLetter PreviousRulesLetter
#define RulesPublish PreviousRulesPublish
#define RulesHasEnding PreviousRulesHasEnding
#define RulesAppend PreviousRulesAppend
#define RulesTrace PreviousRulesTrace
#define TranslateRules PreviousTranslateRules
#include "rule_host_reference.inc"
#undef RustRulesHost
#undef RulesPointer
#undef RulesByte
#undef RulesWrite
#undef RulesValue
#undef RulesStore
#undef RulesLocale
#undef RulesGroup
#undef RulesMatched
#undef RulesSymbol
#undef RulesLetter
#undef RulesPublish
#undef RulesHasEnding
#undef RulesAppend
#undef RulesTrace
#undef TranslateRules
#define TranslateRules NativeRules
#include "native_rule_host_projections.inc"
#undef TranslateRules
#undef espeak_rs_match_rule
#undef LookupBounded
#undef LookupLetterBounded
#undef AppendPhonemes
#undef fprintf
static void Bind(Fixture *);
static char *expected_base;
static size_t expected_length, inspected;
static int InspectMatch(void *opaque,size_t group,size_t width,unsigned flags,unsigned dictionary,char **cursor,void *record)
{
    RustRulesHost *host=opaque;
    TEST_ASSERT(host->base==expected_base && host->length==expected_length);
    TEST_ASSERT(host->match_length==strlen(host->source)+2);
    TEST_ASSERT(espeak_rs_rules_pointer(host,-1)==host->source-1);
    TEST_ASSERT(espeak_rs_rules_pointer(host,INTPTR_MIN)==NULL);
    TEST_ASSERT(espeak_rs_rules_pointer(host,INTPTR_MAX)==NULL);
    inspected++;
    return RulesMatched(opaque,group,width,flags,dictionary,cursor,record);
}
static int FailMatch(void *opaque,size_t group,size_t width,unsigned flags,unsigned dictionary,char **cursor,void *record)
{
    char *scratch=((MatchRecord *)record)->phonemes;
    RulesMatched(opaque,group,width,flags,dictionary,cursor,record);
    strcpy(scratch,"changed");
    return -1;
}
static int EqualMatch(const RustRulesMatch *a,const RustRulesMatch *b)
{
    return a->points==b->points && a->ending==b->ending && a->cursor==b->cursor && a->delete_offset==b->delete_offset && memcmp(a->phonemes,b->phonemes,200)==0;
}
static void Conversion(void)
{
    for(int bad=0;bad<=12;bad++) {
        Fixture ref={0},native={0};ref.seed=17;ref.bad=bad;strcpy(ref.text+2,"abc ");native=ref;Bind(&ref);Bind(&native);
        PreviousRustRulesHost old={&ref.tr,ref.text+2,ref.output,ref.ending,ref.text,512,6,200,ref.flags};
        RustRulesHost host={&native.tr,native.text+2,native.output,native.ending,native.text,512,6,200,native.flags};
        RustRulesMatch wanted={19,13,0,RUST_RULES_NO_DELETE,{0}},got=wanted;
        memset(wanted.phonemes,0x97,200);strcpy(wanted.phonemes,"old");got=wanted;
        option_sayas=0;int previous=PreviousRulesMatched(&old,16,1,0,0,&wanted);int sayas=option_sayas;
        option_sayas=0;int status=espeak_rs_rules_match_convert(&host,16,1,0,0,&got,RulesMatched);
        TEST_ASSERT(status==previous && EqualMatch(&wanted,&got));
        TEST_ASSERT(ref.calls==native.calls && ref.trace==native.trace && sayas==option_sayas);
        TEST_ASSERT(memcmp(ref.text,native.text,512)==0 && memcmp(ref.rules,native.rules,1024)==0);
        TEST_ASSERT(ref.tr.langopts.tone_numbers==native.tr.langopts.tone_numbers);
        if(bad==1 || bad==11) TEST_ASSERT(status==-1 && got.cursor==0 && got.points==19 && got.ending==13);
        if(bad==2 || bad==3 || bad==4 || bad==12) TEST_ASSERT(status==-1 && got.cursor==1);
        if(bad==2 || bad==12) TEST_ASSERT(got.delete_offset==RUST_RULES_NO_DELETE);
        if(bad==5) TEST_ASSERT(strcmp(got.phonemes,"xyz")==0);
        if(bad==6 || bad==10) TEST_ASSERT(got.phonemes[0]==0);
        if(bad==7) TEST_ASSERT(got.delete_offset==-1);
        if(bad==8) TEST_ASSERT(got.delete_offset==1);
        if(bad==9) TEST_ASSERT(status==0 && memchr(got.phonemes,0,200)==NULL);
    }
    Fixture f={0};f.seed=17;strcpy(f.text+2,"abc ");Bind(&f);
    RustRulesHost host={&f.tr,f.text+2,f.output,f.ending,f.text,512,6,200,f.flags};
    RustRulesMatch out={19,13,0,RUST_RULES_NO_DELETE,{0}},saved;
    strcpy(out.phonemes,"old");saved=out;
    TEST_ASSERT(espeak_rs_rules_match_convert(&host,16,1,0,0,&out,NULL)==-1);
    TEST_ASSERT(espeak_rs_rules_match_convert(NULL,16,1,0,0,&out,RulesMatched)==-1);
    TEST_ASSERT(espeak_rs_rules_match_convert(&host,16,1,0,0,NULL,RulesMatched)==-1);
    TEST_ASSERT(espeak_rs_rules_match_convert(&host,16,5,0,0,&out,RulesMatched)==-1);
    TEST_ASSERT(espeak_rs_rules_match_convert(&host,1024,1,0,0,&out,RulesMatched)==-1);
    TEST_ASSERT(f.calls==0 && EqualMatch(&out,&saved));
    out.cursor=SIZE_MAX;saved=out;
    TEST_ASSERT(espeak_rs_rules_match_convert(&host,16,1,0,0,&out,RulesMatched)==-1);
    TEST_ASSERT(f.calls==0 && EqualMatch(&out,&saved));
    out.cursor=0;out.delete_offset=-3;saved=out;
    TEST_ASSERT(espeak_rs_rules_match_convert(&host,16,1,0,0,&out,RulesMatched)==-1);
    TEST_ASSERT(f.calls==0 && EqualMatch(&out,&saved));
    out.delete_offset=RUST_RULES_NO_DELETE;
    TEST_ASSERT(espeak_rs_rules_match_convert(&host,16,1,0,0,&out,FailMatch)==-1);
    TEST_ASSERT(f.calls==1 && out.cursor==0 && out.points==19 && out.ending==13 && out.delete_offset==RUST_RULES_NO_DELETE && strcmp(out.phonemes,"changed")==0);
}
static void Admission(void)
{
    Fixture f={0};Translator main={0};f.seed=17;f.capacity=200;f.text[1]=' ';strcpy(f.text+2,"abc ");Bind(&f);active=&f;
    RustRulesHost host={&f.tr,f.text+2,f.output,f.ending,NULL,0,0,200,f.flags};
    RustRulesGlobals globals={&option_sayas,&option_phonemes,&pre_pause,CHAR_MIN<0};
    RustRulesEffects effects={RulesLocale,InspectMatch,RulesSymbol,RulesLetter,RulesAppend,RulesTrace};
    int result=123;
    TEST_ASSERT(espeak_rs_translate_rules_host(NULL,&main,&globals,&effects,0,&result)==-1);
    TEST_ASSERT(espeak_rs_translate_rules_host(&host,&main,NULL,&effects,0,&result)==-1);
    TEST_ASSERT(espeak_rs_translate_rules_host(&host,&main,&globals,NULL,0,&result)==-1);
    TEST_ASSERT(espeak_rs_translate_rules_host(&host,&main,&globals,&effects,0,NULL)==-1);
    int *saved=globals.say_as;globals.say_as=NULL;
    TEST_ASSERT(espeak_rs_translate_rules_host(&host,&main,&globals,&effects,0,&result)==-1);globals.say_as=saved;
    effects.matched=NULL;TEST_ASSERT(espeak_rs_translate_rules_host(&host,&main,&globals,&effects,0,&result)==-1);effects.matched=InspectMatch;
    host.capacity=0;TEST_ASSERT(espeak_rs_translate_rules_host(&host,&main,&globals,&effects,0,&result)==-1);
    host.capacity=201;TEST_ASSERT(espeak_rs_translate_rules_host(&host,&main,&globals,&effects,0,&result)==-1);host.capacity=200;
    host.tr=NULL;TEST_ASSERT(espeak_rs_translate_rules_host(&host,&main,&globals,&effects,0,&result)==-1);host.tr=&f.tr;
    TEST_ASSERT(result==123 && f.calls==0);
    main.rule_text_base=f.text;main.rule_text_length=512;
    for(unsigned trial=0;trial<3;trial++) {
        memset(f.output,0,200);strcpy(f.text+2,"abc ");f.flags[0]=0;option_sayas=0;option_phonemes=0;
        f.tr.rule_text_base=trial==1?f.text+1:trial==2?f.text+2:NULL;
        f.tr.rule_text_length=trial==1?6:512;
        expected_base=trial==1?f.text+1:f.text;expected_length=trial==1?6:512;
        size_t before=inspected;
        TEST_ASSERT(espeak_rs_translate_rules_host(&host,&main,&globals,&effects,0,&result)==0);
        TEST_ASSERT(inspected>before);
    }
}
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
        if (trial%4 == 0) { ref.tr.rule_text_base=ref.text;ref.tr.rule_text_length=512;native.tr.rule_text_base=native.text;native.tr.rule_text_length=512; }
        if (trial%4 == 1) { ref.tr.rule_text_base=ref.text+2;ref.tr.rule_text_length=2;native.tr.rule_text_base=native.text+2;native.tr.rule_text_length=2; }
        unsigned wordflags=((trial&1)?FLAG_NO_PREFIX:0)|((trial&2)?FLAG_UNPRON_TEST:0)|((trial&4)?FLAG_NO_TRACE:0)|((trial&8)?FLAG_DONT_SWITCH_TRANSLATOR:0);
        int sayas=(int)(Random()&0x10),trace=(int)(Random()&1)?espeakPHONEMES_TRACE:0,pause=(int)(Random()%12);
        translator=&ref.tr;active=&ref;option_sayas=sayas;option_phonemes=trace;pre_pause=pause;
        int wanted=PreviousTranslateRules(&ref.tr,ref.text+2,ref.output,(int)ref.capacity,ref.has_ending?ref.ending:NULL,(int)wordflags,ref.flags);
        int saved_sayas=option_sayas,saved_trace=option_phonemes,saved_pause=pre_pause;
        translator=&native.tr;active=&native;option_sayas=sayas;option_phonemes=trace;pre_pause=pause;int got=NativeRules(&native.tr,native.text+2,native.output,(int)native.capacity,native.has_ending?native.ending:NULL,(int)wordflags,native.flags);
        int status=0;
        if(status||wanted!=got||memcmp(ref.text,native.text,512)||memcmp(ref.output,native.output,200)||memcmp(ref.ending,native.ending,200)||memcmp(ref.flags,native.flags,sizeof(ref.flags))||ref.trace!=native.trace||ref.calls!=native.calls||ref.symbols!=native.symbols||ref.letters!=native.letters||ref.appends!=native.appends||ref.tr.word_vowel_count!=native.tr.word_vowel_count||ref.tr.word_stressed_count!=native.tr.word_stressed_count||ref.tr.phonemes_repeat_count!=native.tr.phonemes_repeat_count||ref.tr.langopts.tone_numbers!=native.tr.langopts.tone_numbers||saved_sayas!=option_sayas||saved_trace!=option_phonemes||saved_pause!=pre_pause) {
            fprintf(stderr,"trial=%u seed=%x word=%s flags=%x status=%d ending=%x/%x trace=%llx/%llx counts=%d,%d/%d,%d phon=%s/%s\n",trial,ref.seed,words[trial%16],wordflags,status,wanted,got,(unsigned long long)ref.trace,(unsigned long long)native.trace,ref.tr.word_vowel_count,ref.tr.word_stressed_count,native.tr.word_vowel_count,native.tr.word_stressed_count,ref.output,native.output);TEST_ASSERT(false);
        }
        symbols+=native.symbols;letters+=native.letters;endings+=got!=0;
    }
    Admission();Conversion();
    printf("200000 actual previous-C/native-Rust rule-host comparisons passed; %u symbol calls, %u letter calls, %u endings; byte/output/state/order, scoped admission, 13 conversion cases and ABI/failure guards match\n",symbols,letters,endings);
    translator=NULL;
    return 0;
}
