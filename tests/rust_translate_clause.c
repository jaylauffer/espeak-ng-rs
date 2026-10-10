/* Whole clause policy versus unchanged retained C, with live primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>
#include <locale.h>
#include "translate.h"
#include "common.h"
#include "phoneme.h"
#include "synthesize.h"
#include "readclause.h"
#include "phonemelist.h"
#include "rust_translate_clause.h"
typedef struct {
    Translator tr;
    int embedded_ix,embedded_read,pause_state,count_characters,clause_start_char,count_words,clause_start_word;
    int max_clause_pause,count_sentences,skip_sentences,n_ph_list2,option_sayas2,count_sayas_digits;
    int option_phoneme_input,word_emphasis,skip_words,dictionary_skipwords,option_endpause,n_phoneme_list;
    bool any_stressed_words,new_sentence,skipping_text;
    ALPHABET *current_alphabet;
    char source[N_TR_SOURCE+40],input[N_TR_SOURCE];
    unsigned embedded_list[N_EMBEDDED_LIST];
    PHONEME_LIST2 ph_list2[N_PHONEME_LIST];
    PHONEME_LIST phoneme_list[N_PHONEME_LIST+1];
    int tone,terminator,pause;char name[40];
    unsigned seed,calls,commands,number_calls,spelling_calls;uint64_t trace;
} Fixture;
static Fixture *active;
static int fail_word,mutate_context;
static void Trace(unsigned n){active->trace=(active->trace^n)*UINT64_C(1099511628211);}
static void Bytes(const char *p,size_t length){for(size_t i=0;i<length;i++)Trace((unsigned char)p[i]);}
#define embedded_ix (active->embedded_ix)
#define embedded_read (active->embedded_read)
#define any_stressed_words (active->any_stressed_words)
#define count_characters (active->count_characters)
#define clause_start_char (active->clause_start_char)
#define count_words (active->count_words)
#define clause_start_word (active->clause_start_word)
#define max_clause_pause (active->max_clause_pause)
#define new_sentence (active->new_sentence)
#define count_sentences (active->count_sentences)
#define skip_sentences (active->skip_sentences)
#define skipping_text (active->skipping_text)
#define n_ph_list2 (active->n_ph_list2)
#define option_sayas2 (active->option_sayas2)
#define count_sayas_digits (active->count_sayas_digits)
#define option_phoneme_input (active->option_phoneme_input)
#define word_emphasis (active->word_emphasis)
#define skip_words (active->skip_words)
#define dictionary_skipwords (active->dictionary_skipwords)
#define option_endpause (active->option_endpause)
#define n_phoneme_list (active->n_phoneme_list)
#define current_alphabet (active->current_alphabet)
#define source (active->source)
#define embedded_list (active->embedded_list)
#define ph_list2 (active->ph_list2)
#define phoneme_list (active->phoneme_list)
#define MAKE_MEM_UNDEFINED(p,n) ((void)0)
static const unsigned short breaks[]={'_',0};
static int MockRead(Translator *tr,char *text,short *positions,int *top,int capacity,int *tone,char *name)
{
    TEST_ASSERT(tr==&active->tr&&text==source&&capacity==800);
    Trace(1);Trace(embedded_ix);Trace(embedded_read);Trace(pre_pause);Trace(any_stressed_words);
    Trace(clause_start_char);Trace(clause_start_word);
    size_t n=strlen(active->input);TEST_ASSERT(n<700);memcpy(text,active->input,n+1);
    for(size_t i=0;i<n;i++)positions[i]=isspace2((unsigned char)text[i])?0:((text[i]&0xc0)==0x80?-1:(short)(i+1));
    *top=(int)n;*tone=(active->seed%9==0)?3:0;
    snprintf(name,40,"test-voice-%u",active->seed);count_characters+=(int)n;
    int term=active->seed%8==0?CLAUSE_VOICE:CLAUSE_PERIOD|CLAUSE_DOT_AFTER_LAST_WORD;
    if(active->seed%13==0)term|=CLAUSE_PAUSE_LONG;
    return term;
}
static int MockEmbedded(unsigned *index)
{
    Trace(2);Trace(*index);active->commands++;
    while(IsDigit09(source[*index]))(*index)++;
    char code=source[(*index)++];
    TEST_ASSERT(embedded_ix<N_EMBEDDED_LIST);embedded_list[embedded_ix++]=(unsigned char)code;
    if(code=='y')option_sayas2=SAYAS_DIGITS|3;
    else if(code=='e')word_emphasis=FLAG_EMPHASIZED;
    return 1;
}
static unsigned MockChar(Translator *tr,char *text,int previous,unsigned c,unsigned next,int *insert,int *flags)
{
    TEST_ASSERT(tr==&active->tr);Trace(3);Trace((unsigned)(text-source));Trace(previous);Trace(c);Trace(next);Trace(*insert);Trace(*flags);
    if(c=='~'){*insert='a';*flags|=FLAG_CHAR_REPLACED;tr->langopts.tone_numbers^=1;word_emphasis=FLAG_FOCUS;return 'b';}
    if(c=='`')return 8;
    return c;
}
static int MockUpper(Translator *tr,char *text,int code){TEST_ASSERT(tr==&active->tr);Trace(4);Trace(code);Trace((unsigned char)text[-1]);return active->seed%3==0;}
static int MockWord(Translator *tr,char *word,WORD_TAB *rows,int remaining,int pause,const char *base,size_t initialized)
{
    TEST_ASSERT(tr==&active->tr&&remaining>0&&initialized<=800);
    const char *saved=tr->rule_text_base;size_t saved_length=tr->rule_text_length;
    tr->rule_text_base=base;tr->rule_text_length=initialized;
    Trace(5);Trace(pause);Trace(remaining);Trace((unsigned)(word-base));Trace((unsigned)initialized);Bytes(base,initialized);
    for(int i=0;i<remaining;i++){Trace(rows[i].flags);Trace(rows[i].start);Trace(rows[i].pre_pause);Trace(rows[i].sourceix);Trace(rows[i].length);}
    uintptr_t b=(uintptr_t)base,e=(uintptr_t)tr->clause_end;
    Trace(e>=b&&e-b<initialized?(unsigned)(e-b):UINT32_MAX);
    active->calls++;if(base[0]==' ')active->number_calls++;else if(initialized==150)active->spelling_calls++;
    current_alphabet=NULL;pre_pause=active->seed%5;dictionary_skipwords=active->seed%7==0?2:0;
    unsigned result=FLAG_FOUND;if(dictionary_skipwords)result|=FLAG_SKIPWORDS;
    if(active->seed%11==0)result|=FLAG_ALLOW_DOT;
    if(word[0]=='z')result|=FLAG_SPELLWORD;
    word[0]=word[0]=='q'?'x':word[0];rows->flags^=FLAG_HYPHEN;tr->expect_noun++;
    if(active->seed%17==0&&remaining>1)rows[1].flags|=FLAG_FOCUS;
    TEST_ASSERT(n_ph_list2<N_PHONEME_LIST);ph_list2[n_ph_list2].phcode=40;ph_list2[n_ph_list2].sourceix=rows->sourceix;n_ph_list2++;
    if(mutate_context){tr->rule_text_base=source;tr->rule_text_length=7;tr->clause_end=source;}
    tr->rule_text_base=saved;tr->rule_text_length=saved_length;
    return (int)result;
}
static void MockWordEmbedded(void){Trace(6);embedded_read=embedded_ix;option_sayas2=SAYAS_SINGLE_CHARS;}
static bool MockEof(void){Trace(7);return active->seed%2!=0;}
static void MockMake(Translator *tr,int pause,bool sentence)
{
    TEST_ASSERT(tr==&active->tr);Trace(8);Trace(pause);Trace(sentence);active->pause=pause;
    n_phoneme_list=n_ph_list2;
    for(int i=0;i<n_ph_list2;i++){Trace(ph_list2[i].phcode);Trace(ph_list2[i].stresslevel);Trace(ph_list2[i].sourceix);Trace(ph_list2[i].synthflags);}
}
#define ReadClause MockRead
#define EmbeddedCommand MockEmbedded
#define TranslateChar MockChar
#define UpperCaseInWord MockUpper
#define TranslateWord2WithContext MockWord
#define Word_EmbeddedCmd MockWordEmbedded
#define Eof MockEof
#define MakePhonemeList MockMake
#include "retained_replacement_segmentation.inc"
#include "retained_clause_length_helper.inc"
#define TranslateClauseWithTerminator ReferenceClause
#include "retained_clause_policy.inc"
#undef TranslateClauseWithTerminator
#define TranslateClauseWithTerminator NativeClause
#include "native_clause_projections.inc"
#undef TranslateClauseWithTerminator
static const wchar_t punctuation[]={39,0},apostrophe[]={0};
static void Init(Fixture *f,unsigned trial)
{
    memset(f,0,sizeof(*f));active=f;f->seed=trial*0x31fdu+47;f->trace=UINT64_C(1469598103934665603);
    const char *pieces[]={"Alpha beta CamelCase XMLTest NASA's.","123456789 000012345 1,234,567 21. next.","😙 🙂 © 🇰🇿 🇺🇸 👩🏽‍💻 🏴\U000e0067\U000e007f.","汉字 中國 日本語 한국어 กข้เ.","Côte-d’Ivoire Казахстан Καζακστάν BJPയുമായുള്ള.","[[a:||b 'a]] next.","zebra query quiet ~ ` end."," a \00112p b \0013e c "," \0013y 123456789 abc", " ","", "a -2 a-2 a -- b . mr. u.s.a.'s."};
    for(unsigned i=0;i<1+f->seed%4;i++)strcat(f->input,pieces[(trial+i*7)%12]);
    count_characters=f->seed%19-9;count_words=f->seed%11;max_clause_pause=f->seed%500;
    new_sentence=f->seed&1;skip_sentences=f->seed%3;skip_words=f->seed%5;skipping_text=f->seed%7==0;
    option_endpause=f->seed&2;option_phoneme_input=f->seed&4;option_sayas2=f->seed%8==0?SAYAS_KEY:f->seed%8==1?SAYAS_SINGLE_CHARS:f->seed%8==2?SAYAS_DIGITS|2:0;
    f->tr.langopts.tone_numbers=f->seed&8;f->tr.langopts.param[LOPT_CAPS_IN_WORD]=f->seed%13==0;
    f->tr.langopts.param[LOPT_APOSTROPHE]=f->seed%4;f->tr.langopts.max_digits=8;
    f->tr.langopts.break_numbers=f->seed&16?BREAK_THOUSANDS:0x24924924;
    f->tr.langopts.thousands_sep=',';f->tr.langopts.decimal_sep='.';
    f->tr.langopts.ideographs=f->seed&32;f->tr.langopts.alt_alphabet=f->seed&64?0x3100:0;
    f->tr.letter_bits_offset=f->seed&128?0x400:0;f->tr.translator_name=f->seed&256?L('n','l'):L('e','n');
    f->tr.punct_within_word=punctuation;f->tr.char_plus_apostrophe=apostrophe;
    f->tr.rule_text_base=source;f->tr.rule_text_length=sizeof(source);f->tr.clause_end=source;
    f->tone=-91;f->terminator=-92;strcpy(f->name,"previous");
    for(int i=0;i<N_PHONEME_LIST;i++){ph_list2[i].wordstress=0x55;ph_list2[i].tone_ph=0x56;}
}
static void Normalize(Fixture *f){f->tr.rule_text_base=NULL;f->tr.clause_end=NULL;}
static int FailWord(void *opaque,char *clause,size_t end,char *number,size_t initialized,size_t offset,RustClauseRow *rows,size_t row,size_t remaining,int pause,unsigned *flags)
{int result=ClauseTranslate(opaque,clause,end,number,initialized,offset,rows,row,remaining,pause,flags);return fail_word?-1:result;}
int main(void)
{
    TEST_ASSERT(setlocale(LC_CTYPE,"en_US.UTF-8")||setlocale(LC_CTYPE,"C.UTF-8"));
    Fixture *old=calloc(1,sizeof(*old)),*native=calloc(1,sizeof(*native));TEST_ASSERT(old&&native);
    unsigned words=0,commands=0,numbers=0,spelling=0;
    for(unsigned trial=0;trial<200000;trial++) {
        char *voice=NULL;Init(old,trial);ReferenceClause(&old->tr,&old->tone,&voice,&old->terminator);old->pause_state=pre_pause;if(voice)strcpy(old->name,voice);else old->name[0]=0;
        Init(native,trial);voice=NULL;NativeClause(&native->tr,&native->tone,&voice,&native->terminator);native->pause_state=pre_pause;if(voice)strcpy(native->name,voice);else native->name[0]=0;
        Normalize(old);Normalize(native);
        if(memcmp(old,native,sizeof(*old))!=0){fprintf(stderr,"clause mismatch trial=%u calls=%u/%u trace=%llu/%llu\n",trial,old->calls,native->calls,(unsigned long long)old->trace,(unsigned long long)native->trace);return 1;}
        words+=native->calls;commands+=native->commands;numbers+=native->number_calls;spelling+=native->spelling_calls;
    }
    char *old_voice=NULL,*native_voice=NULL;
    Init(old,5);ReferenceClause(&old->tr,NULL,&old_voice,NULL);
    Init(native,5);NativeClause(&native->tr,NULL,&native_voice,NULL);
    TEST_ASSERT(old_voice&&native_voice&&strcmp(old_voice,native_voice)==0);
    Init(old,6);ReferenceClause(&old->tr,NULL,NULL,NULL);
    Init(native,6);NativeClause(&native->tr,NULL,NULL,NULL);
    TEST_ASSERT(strcmp(old_voice,native_voice)==0);
    Init(native,7);RustClauseHost h={&native->tr,&native->tone,NULL,NULL};
    RustTranslateClause callbacks={&h,ClauseValue,ClauseStore,ClauseByte,ClauseWrite,ClauseClass,ClauseLower,ClauseRead,ClauseEmbeddedCommand,ClauseChar,ClauseUpper,ClauseEntry,ClausePut,ClauseCommand,ClausePutCommand,FailWord,ClauseWordEmbedded,ClauseEof,ClauseMakeList,ClauseOutput,ClauseResult};
    TEST_ASSERT(espeak_rs_translate_clause(NULL)==-1);callbacks.lower=NULL;TEST_ASSERT(espeak_rs_translate_clause(&callbacks)==-1&&native->calls==0);callbacks.lower=ClauseLower;
    fail_word=1;skipping_text=false;skip_words=0;skip_sentences=0;strcpy(native->input,"alpha beta ");
    TEST_ASSERT(espeak_rs_translate_clause(&callbacks)==-1&&native->calls==1&&native->tone!=-91);fail_word=0;
    char clause[800]={0},number[150]={0};RustClauseRow rows[301]={0};unsigned flags=0;strcpy(clause+3,"query next ");clause[1]=clause[2]=' ';mutate_context=1;
    TEST_ASSERT(ClauseTranslate(&h,clause,13,NULL,0,3,rows,0,2,0,&flags)==0);
    TEST_ASSERT(native->tr.clause_end==source&&native->tr.rule_text_base==source&&native->tr.rule_text_length==sizeof(source));mutate_context=0;
    TEST_ASSERT(ClauseTranslate(&h,clause,800,NULL,0,3,rows,0,2,0,&flags)==-1);
    TEST_ASSERT(ClauseTranslate(&h,clause,SIZE_MAX,NULL,0,3,rows,0,2,0,&flags)==-1);
    TEST_ASSERT(ClauseTranslate(&h,clause,13,number,151,3,rows,0,2,0,&flags)==-1);
    TEST_ASSERT(ClauseTranslate(&h,clause,13,NULL,0,14,rows,300,2,0,&flags)==-1);
    printf("200000 retained-C clause comparisons: %u words, %u embedded commands, %u number parts, %u spelling calls; ABI/scope/failure guards pass\n",words,commands,numbers,spelling);
    free(old);free(native);return 0;
}
