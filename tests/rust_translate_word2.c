/* Complete word-to-list control versus the unchanged retained C policy.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "translate.h"
#include "synthesize.h"
#include "phoneme.h"
#include "voice.h"
#include "common.h"
#include "rust_translate_word2.h"
typedef struct {
    Translator tr,alt; WORD_TAB rows[4]; char source[620],output[200],dictionary[40];
    PHONEME_LIST2 list[1000]; unsigned commands[250]; PHONEME_TAB tables[256],*table[256]; voice_t voice_value;
    int count,embedded,index,sayas,capitals,emphasis,any,selected;
    unsigned seed,calls,switches,combines,invalids; uint64_t trace;
} Fixture;
static Fixture *active;
static unsigned translations,alternates,combines,entries,invalids;
static int wide_replacement,probe_context;
static void Trace(unsigned value){active->trace=(active->trace^value)*UINT64_C(1099511628211);}
static void Bytes(const char *p){while(*p)Trace((unsigned char)*p++);Trace(0);}
#define n_ph_list2 (active->count)
#define embedded_flag (active->embedded)
#define embedded_ix (active->index)
#define option_sayas (active->sayas)
#define option_capitals (active->capitals)
#define option_emphasis (active->emphasis)
#define any_stressed_words (active->any)
#define word_phonemes (active->output)
#define dictionary_name (active->dictionary)
#define ph_list2 (active->list)
#define embedded_list (active->commands)
#define phoneme_tab (active->table)
#define voice (&active->voice_value)
static void MockSet(PHONEME_LIST2 *p,unsigned char c){p->phcode=c;p->stresslevel=0;p->tone_ph=0;p->synthflags=embedded_flag;p->sourceix=0;embedded_flag=0;}
static void MockEmbedded(void){Trace(1);active->sayas=(active->seed&1)?0x12:0;active->emphasis=(active->seed>>8)%5;}
static void Output(void)
{
    static const unsigned char choices[][30]={
        {35,40,50,41,0},{36,50,35,40,36,50,41,0},{50,20,12,15,40,22,41,0},
        {9,21,3,35,40,21,1,50,0},{255,99,35,40,50,0},{0},
        {50,40,35,50,50,41,27,0},{9,35,40,50,41,0}};
    unsigned choice=active->seed%8;
    memcpy(word_phonemes,choices[choice],sizeof(choices[choice]));
}
static int MockEncode(char *word,char *out,int *bad){Trace(2);Bytes(word);TEST_ASSERT(out==word_phonemes);*bad=0;Output();return 0;}
static void MockLanguage(char *word,char *out){int bad;Trace(3);MockEncode(word,out,&bad);}
static int MockTranslate(Translator *tr,char *word,WORD_TAB *rows,int remaining,char *out,int depth)
{
    TEST_ASSERT((tr==&active->tr||tr==&active->alt)&&rows==active->rows+1&&remaining==(int)(active->seed%4)&&depth==0);
    Trace(4);Trace(tr==&active->alt);Trace(out!=NULL);Bytes(word);active->calls++;
    if(probe_context){TEST_ASSERT(out==NULL&&tr->rule_text_base==word-1&&tr->rule_text_length==163);tr->rule_text_base=active->source;tr->rule_text_length=3;}
    if(wide_replacement){TEST_ASSERT(out!=NULL);memset(out,'r',160);out[160]=0;Output();return FLAG_FOUND;}
    active->source[4]='!'; rows->flags^=FLAG_FIRST_UPPER; tr->expect_noun++;
    active->emphasis=(active->seed>>8)%5;active->capitals=(active->seed>>9)%5;
    active->voice_value.phoneme_tab_ix=1+(active->seed%3);
    if(out && active->seed%7==0)strcpy(out,"replacement ");
    unsigned mode=(active->seed>>2)%7;
    if(active->calls==1&&mode==0){Output();return FLAG_SPELLWORD|FLAG_FOUND;}
    if(mode>=2&&active->calls<=mode-1){word_phonemes[0]=phonSWITCH;strcpy(word_phonemes+1,active->seed&2?"ru":"");}
    else Output();
    return FLAG_FOUND|((active->seed&8)?FLAG_PREPAUSE:0)|((active->seed&16)?FLAG_PAUSE1:0)|
        ((active->seed&32)?FLAG_STRESS_END2:0)|((active->seed&64)?FLAG_COMBINE:0)|((active->seed&128)?FLAG_TEXTMODE:0);
}
static void MockCombine(Translator *tr,WORD_TAB *rows,int remaining,char *word,int *flags,unsigned char *p,char *out)
{
    TEST_ASSERT(tr==&active->tr&&rows==active->rows+1&&remaining>1&&word==active->source+3&&p==(unsigned char *)word_phonemes&&out==word_phonemes);
    Trace(5);active->combines++;*flags^=FLAG_PAUSE1;rows[1].flags|=FLAG_DELETE_WORD;
}
static int MockAlternate(const char *name)
{
    Trace(6);Bytes(name);active->switches++;strcpy(dictionary_name,"alternate");
    if(active->seed%13==0){Output();return -1;}
    if(active->seed%17==0)return -2;
    return 2+(active->seed%5);
}
static int MockSelect(int table){Trace(7);Trace(table);active->selected=table;active->tables[50].type=table==3?phVOWEL:phFRICATIVE;return table;}
static int MockPrint(const char *format,int code){TEST_ASSERT(strcmp(format,"Invalid phoneme code %d\n")==0);Trace(8);Trace(code);active->invalids++;return 0;}
#define SetPlist2 MockSet
#define Word_EmbeddedCmd MockEmbedded
#define SwitchLanguage MockLanguage
#define EncodePhonemes MockEncode
#define TranslateWordWithBounds MockTranslate
#define CombineFlag MockCombine
#define SetTranslator2 MockAlternate
#define SelectPhonemeTable MockSelect
#define printf MockPrint
#define TranslateWord2 ReferenceWord2
#include "retained_word-to-list_policy.inc"
#undef TranslateWord2
#define TranslateWord2 NativeWord2
#include "native_word-to-list_projections.inc"
#undef TranslateWord2
#undef printf
static void Init(Fixture *f,unsigned trial)
{
    memset(f,0,sizeof(*f));f->seed=trial*0xabcdu+29;f->trace=UINT64_C(1469598103934665603);
    strcpy(f->source,"   Abcd xyz ");strcpy(f->dictionary,"original");f->voice_value.phoneme_tab_ix=1;
    f->count=f->seed%20+1;if(trial%23==0)f->count=989+f->seed%10;
    f->embedded=f->seed&2;f->index=3;f->commands[2]=0x80|0x121;
    f->capitals=(f->seed>>9)%5;f->emphasis=(f->seed>>8)%5;f->sayas=f->seed&1?0x12:0;
    f->tr.prepause_timeout=f->seed%5;f->tr.prev_dict_flags[0]=f->seed&1?FLAG_DOUBLING:0;
    f->tr.prev_dict_flags[1]=0x319;f->tr.end_stressed_vowel=1;
    memset(f->list,0x53,sizeof(f->list));f->list[f->count-1].phcode=f->seed%3==0?phonSWITCH:phonPAUSE;
    for(unsigned i=0;i<256;i++){f->table[i]=&f->tables[i];f->tables[i].type=phFRICATIVE;}
    f->table[99]=NULL;f->tables[0].type=phPAUSE;f->tables[9].type=phPAUSE;f->tables[27].type=phPAUSE;
    f->tables[35].type=phSTRESS;f->tables[35].std_length=4;
    f->tables[36].type=phSTRESS;f->tables[36].program=1;
    f->tables[40].type=f->tables[41].type=phVOWEL;
    unsigned wf=0;
    if(f->seed&1)wf|=FLAG_FIRST_UPPER;if(f->seed&2)wf|=FLAG_ALL_UPPER;
    if(f->seed&4)wf|=FLAG_EMBEDDED;if(f->seed&8)wf|=FLAG_HYPHEN;
    if(f->seed&16)wf|=FLAG_COMMA_AFTER;if(f->seed&32)wf|=FLAG_FIRST_WORD;
    if(f->seed&64)wf|=FLAG_LAST_WORD;if(trial%19==0)wf|=FLAG_DELETE_WORD;
    if(trial%11==0)wf|=FLAG_PHONEMES;if(trial%43==0)f->source[3]=0;
    if(trial%47==0)strcpy(f->source+3,"_^_ru ");
    f->rows[1].flags=wf;f->rows[1].length=f->seed%40;f->rows[1].sourceix=f->seed%2048;
    f->rows[0].flags=f->seed&16?FLAG_FIRST_WORD:0;f->rows[2].flags=f->seed&32?FLAG_PHONEMES:0;
}
static void Activate(Fixture *f)
{
    active=f;translator=&f->tr;translator2=&f->alt;translator3=NULL;
    f->tr.rule_text_base=f->source;f->tr.rule_text_length=sizeof(f->source);
    for(unsigned i=0;i<256;i++)if(i!=99)f->table[i]=&f->tables[i];
}
static void Normalize(Fixture *f){f->tr.rule_text_base=NULL;for(unsigned i=0;i<256;i++)f->table[i]=NULL;}
static RustTranslateWord2 Callbacks(RustWord2Host *h)
{
    RustTranslateWord2 c={h,Word2Value,Word2Store,Word2Byte,Word2Write,Word2Entry,Word2Put,
        Word2Command,Word2PutCommand,Word2Embedded,Word2Encode,Word2Translate,Word2Combine,
        Word2Dictionary,Word2RestoreDictionary,Word2Alternate,Word2Select,Word2Phoneme,Word2Alpha,Word2Invalid};return c;
}
static int FailTranslate(void *opaque,unsigned alternate,unsigned mode,char *replacement,unsigned *flags)
{
    Word2Translate(opaque,alternate,mode,replacement,flags);return -1;
}
static void NativeGuards(void)
{
    Fixture f;Init(&f,3);Activate(&f);f.rows[1].flags=0;f.count=1;
    RustWord2Host h={&f.tr,f.source+3,f.rows+1,f.seed%4,sizeof(f.source)-3};
    RustTranslateWord2 c=Callbacks(&h);unsigned flags=0x12345;
    TEST_ASSERT(espeak_rs_translate_word2(NULL,0,&flags)==-1&&flags==0x12345);
    c.alpha=NULL;TEST_ASSERT(espeak_rs_translate_word2(&c,0,&flags)==-1&&flags==0x12345);
    c=Callbacks(&h);c.translate=FailTranslate;
    TEST_ASSERT(espeak_rs_translate_word2(&c,0,&flags)==-1&&flags==0x12345&&f.calls==1&&f.source[4]=='!');
    TEST_ASSERT(f.count==1);
    char storage[165];memset(storage,0x67,sizeof(storage));storage[1]=0;storage[2]=' ';storage[3]='r';storage[4]=0;
    wide_replacement=1;
    TEST_ASSERT(Word2Translate(&h,0,0,storage+1,&flags)==0);
    TEST_ASSERT(storage[0]==0x67&&storage[164]==0x67&&storage[163]==0&&storage[162]=='r');
    wide_replacement=0;probe_context=1;
    const char *saved=f.alt.rule_text_base;size_t length=f.alt.rule_text_length;
    TEST_ASSERT(Word2Translate(&h,1,1,storage+1,&flags)==0);
    TEST_ASSERT(f.alt.rule_text_base==saved&&f.alt.rule_text_length==length);probe_context=0;
    PHONEME_LIST2 entry;unsigned command;
    TEST_ASSERT(Word2Entry(&h,-1,&entry)==-1&&Word2Entry(&h,1000,&entry)==-1);
    TEST_ASSERT(Word2Command(&h,-1,&command)==-1&&Word2Command(&h,250,&command)==-1);
    TEST_ASSERT(Word2Byte(&h,200,1)==-1&&Word2Write(&h,h.source_length,'x',0)==-1);
}
int main(void)
{
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture old,native;Init(&old,trial);native=old;
        Activate(&old);int expected=ReferenceWord2(&old.tr,old.source+3,old.rows+1,old.seed%4,old.seed%8);
        Activate(&native);int actual=NativeWord2(&native.tr,native.source+3,native.rows+1,native.seed%4,native.seed%8);
        Normalize(&old);Normalize(&native);
        if(expected!=actual||memcmp(&old,&native,sizeof(old))) {
            fprintf(stderr,"word2 mismatch trial=%u flags=%x/%x count=%d/%d trace=%llu/%llu calls=%u/%u\n",trial,actual,expected,native.count,old.count,(unsigned long long)native.trace,(unsigned long long)old.trace,native.calls,old.calls);
            const unsigned char *a=(const unsigned char *)&old,*b=(const unsigned char *)&native;
            for(size_t i=0;i<sizeof(old);i++)if(a[i]!=b[i]){fprintf(stderr,"byte %zu %u/%u\n",i,b[i],a[i]);break;}return 1;
        }
        translations+=native.calls;alternates+=native.switches;combines+=native.combines;entries+=native.count;invalids+=native.invalids;
    }
    NativeGuards();
    printf("200000 word2 comparisons passed; %u translations, %u alternate attempts, %u combinations, %u accumulated entries, %u invalid phonemes; ABI/failure/context/wide-result guards passed\n",translations,alternates,combines,entries,invalids);
    return 0;
}
