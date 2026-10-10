/* Complete character conversion versus unchanged retained C policy.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <locale.h>
#include <wchar.h>
#include <wctype.h>
#include "translate.h"
#include "common.h"
#include "dictionary.h"
#include "readclause.h"
#include "ucd/ucd.h"
#include "rust_character_conversion.h"
static char test_source[840];
#define source test_source
#define FindReplacementChars ReferenceFind
#define SubstituteChar ReferenceSubstitute
#define TranslateChar ReferenceTranslate
#define UpperCaseInWord ReferenceUpper
#include "retained_character_conversion.inc"
#undef FindReplacementChars
#undef SubstituteChar
#undef TranslateChar
#undef UpperCaseInWord
#define TranslateChar NativeTranslate
#define UpperCaseInWord NativeUpper
#include "native_character_projections.inc"
#undef TranslateChar
#undef UpperCaseInWord
static unsigned translated,replaced,ignored,inserted,upper_matches;
static unsigned Random(unsigned *seed){*seed=*seed*1664525u+1013904223u;return *seed>>8;}
static int RefuseTrace(void *opaque,size_t from,size_t to){(void)opaque;(void)from;(void)to;return -1;}
static void Drain(Translator *tr)
{
    for(unsigned i=0;i<8;i++){int a=1,b=2,x=1,y=2;
        TEST_ASSERT(ReferenceTranslate(tr,source,32,0,32,&a,&b)==NativeTranslate(tr,source,32,0,32,&x,&y));}
    TEST_ASSERT(character_ignore==0);
}
static void Guards(Translator *tr)
{
    RustCharacterHost h={tr};RustCharacterConversion table=CharacterCallbacks(&h);
    RustCharacterCall c={0,32,97,98,71,72};RustCharacterResult out={73,74,75},saved=out;
    TEST_ASSERT(espeak_rs_character_translate(NULL,&c,&out)==-1&&memcmp(&out,&saved,sizeof(out))==0);
    table.lower=NULL;TEST_ASSERT(espeak_rs_character_translate(&table,&c,&out)==-1&&memcmp(&out,&saved,sizeof(out))==0);
    table=CharacterCallbacks(&h);size_t size=tr->data_dict_size;tr->data_dict_size=1;
    TEST_ASSERT(espeak_rs_character_translate(&table,&c,&out)==-1&&memcmp(&out,&saved,sizeof(out))==0);tr->data_dict_size=size;
    const unsigned char *replacement=tr->langopts.replace_chars;
    tr->langopts.replace_chars=(const unsigned char *)tr->data_dictlist+size;
    TEST_ASSERT(espeak_rs_character_translate(&table,&c,&out)==-1&&memcmp(&out,&saved,sizeof(out))==0);
    tr->langopts.replace_chars=NULL;
    TEST_ASSERT(espeak_rs_character_translate(&table,&c,&out)==0&&out.code==97&&out.insert==71&&out.flags==72);
    tr->langopts.replace_chars=replacement;out=saved;
    c.index=SIZE_MAX;tr->translator_name=L('a','f');c.code=39;
    TEST_ASSERT(espeak_rs_character_translate(&table,&c,&out)==-1&&memcmp(&out,&saved,sizeof(out))==0);
    tr->translator_name=L('e','n');c.code=97;c.index=839;source[839]=(char)0x80;
    TEST_ASSERT(espeak_rs_character_translate(&table,&c,&out)==-1&&memcmp(&out,&saved,sizeof(out))==0);
    int flag=123;TEST_ASSERT(espeak_rs_character_upper(&table,NULL,65,&flag)==-1&&flag==123);
    source[839]=0;
    c.index=0;memcpy(source,"bc ",4);table.trace=RefuseTrace;
    TEST_ASSERT(espeak_rs_character_translate(&table,&c,&out)==-1&&memcmp(&out,&saved,sizeof(out))==0&&character_ignore==2);
    character_ignore=0;
}
int main(void)
{
    setlocale(LC_CTYPE,"");
    static const char dictionary[]="abc\0xyz\0ab\0Ωz\0a\0u\0ñx\0éQ\0i\0ı\0q\0\0\0\0\0";
    Translator tr;memset(&tr,0,sizeof(tr));tr.data_dictlist=(char *)dictionary;tr.data_dict_size=sizeof(dictionary);
    tr.langopts.replace_chars=(const unsigned char *)dictionary;
    for(const char *v="AEIOU";*v;v++)tr.letter_bits[(unsigned char)*v]=1u<<LETTERGP_VOWEL2;
    FILE *old_trace=tmpfile(),*native_trace=tmpfile();TEST_ASSERT(old_trace&&native_trace);
    const char *tails[]={"bc ","BC ","bx ","b ","x ","X ","n ","t ","nA", "漢🙂 ","é ","\0"};
    const unsigned codes[]={97,65,98,100,0,39,0x2019,0xf1,0xd1,113,73,105,0xac00,0xc544,0xd7af,0x3133};
    const int langs[]={L('e','n'),L('g','a'),L('a','f'),L('n','l'),L('k','o'),L('t','r')};
    unsigned seed=0x71eabcd;
    for(unsigned trial=0;trial<200000;trial++) {
        tr.translator_name=langs[Random(&seed)%6];Drain(&tr);
        memset(source,0,sizeof(test_source));strcpy(source+3,tails[Random(&seed)%12]);
        char initial[840];memcpy(initial,source,840);unsigned code=codes[Random(&seed)%16];
        if(trial%7==0)code=0xac00+trial%11184;else if(trial%11==0)code=0x3130+trial%52;
        int next;utf8_in(&next,source+3);int previous=trial%3==0?65:32;
        int a=trial%5?0:77,b=(int)(Random(&seed)&0xffff),x=a,y=b;
        option_phonemes=trial%2?espeakPHONEMES_TRACE:0;f_trans=old_trace;rewind(old_trace);
        int expected=ReferenceTranslate(&tr,source+3,previous,code,(unsigned)next,&a,&b);
        long old_len=ftell(old_trace);TEST_ASSERT(old_len>=0&&old_len<256);char changed[840];memcpy(changed,source,840);
        memcpy(source,initial,840);f_trans=native_trace;rewind(native_trace);
        int actual=NativeTranslate(&tr,source+3,previous,code,(unsigned)next,&x,&y);
        long native_len=ftell(native_trace);TEST_ASSERT(native_len==old_len);
        char old_text[256]={0},native_text[256]={0};rewind(old_trace);rewind(native_trace);
        TEST_ASSERT(fread(old_text,1,(size_t)old_len,old_trace)==(size_t)old_len);
        TEST_ASSERT(fread(native_text,1,(size_t)native_len,native_trace)==(size_t)native_len);
        if(expected!=actual||a!=x||b!=y||memcmp(changed,source,840)||memcmp(old_text,native_text,(size_t)old_len)){
            fprintf(stderr,"character mismatch trial=%u code=%x lang=%x result=%x/%x insert=%x/%x flags=%x/%x trace=%s/%s\n",trial,code,tr.translator_name,actual,expected,x,a,y,b,native_text,old_text);return 1;}
        translated++;replaced+=(y&FLAG_CHAR_REPLACED)!=0;inserted+=x!=0;
        for(unsigned i=0;i<4;i++){
            a=x=9;b=y=11;expected=ReferenceTranslate(&tr,source+3,32,122,32,&a,&b);
            actual=NativeTranslate(&tr,source+3,32,122,32,&x,&y);
            TEST_ASSERT(expected==actual&&a==x&&b==y);ignored+=actual==8;
        }
        const char *patterns[]={" bh","x b","x h","x n","abh","  t","  g"};
        char before[8]={0};memcpy(before,patterns[trial%7],3);
        int upper_code=trial%5==0?65:trial%5==1?69:trial%5==2?102:trial%5==3?112:116;
        TEST_ASSERT(ReferenceUpper(&tr,before+3,upper_code)==NativeUpper(&tr,before+3,upper_code));
        upper_matches+=NativeUpper(&tr,before+3,upper_code);
    }
    printf("coverage: %u replacements, %u consumed, %u uppercase matches\n",replaced,ignored,upper_matches);
    TEST_ASSERT(replaced>1000&&ignored>1000&&upper_matches>1000);
    option_phonemes=0;Drain(&tr);Guards(&tr);fclose(old_trace);fclose(native_trace);f_trans=NULL;
    printf("200000 retained-C character sequences: %u translations, %u replacements, %u consumed characters, %u nonzero inserts, %u Irish uppercase matches; source/trace/ABI/bounds guards pass\n",translated,replaced,ignored,inserted,upper_matches);
    return 0;
}
