/* Retained production number/spelling bodies as independent behavior oracles.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "numbers.h"
#include "phoneme.h"
#include "common.h"
#include "rust_number_primitives.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#define L_SUB 0x4000
#define L_SUP 0x8000
#define IsSuperscript ReferenceSuperscript
#define SetSpellingStress ReferenceSpelling
#define hu_number_e ReferenceHungarian
#define M_Variant ReferenceVariant
#define CheckThousandsGroup ReferenceGroup
#define RecognizeRoman ReferenceRoman
#include "number_primitives_reference.inc"
static Translator fixture;
static uint32_t seed=0x37278421;
static uint32_t Random(void) { seed ^= seed<<13; seed ^= seed>>17; seed ^= seed<<5; return seed; }
static unsigned roman_count;
static void Roman(const char *text, unsigned settings)
{
    unsigned char frame[164], before[164];
    memset(frame,0,sizeof(frame)); size_t length=strlen(text); TEST_ASSERT(length<159);
    frame[0]=(settings&1) ? '2' : ' '; frame[1]=' '; memcpy(frame+2,text,length);
    frame[length+2]=(settings&2) ? 0 : ' '; frame[length+3]=(settings&4) ? '7' : ' ';
    memcpy(before,frame,sizeof(frame));
    fixture.langopts.numbers=((settings&8) ? NUM_ROMAN_CAPITALS : 0) | ((settings&16) ? NUM_ROMAN_ORDINAL : 0) | ((settings&32) ? NUM_ORDINAL_DOT : 0);
    fixture.langopts.min_roman=(settings&64) ? 10 : 0; fixture.langopts.max_roman=(settings&128) ? 99 : 10000;
    WORD_TAB word={0}; word.flags=((settings&256) ? FLAG_ALL_UPPER : 0) | ((settings&512) ? FLAG_HAS_DOT : 0);
    char *cursor=(char *)frame+2; int old_value=-717; int old=ReferenceRoman(&fixture,&cursor,&word,&old_value);
    int value=-717; size_t after=919;
    int actual=espeak_rs_number_roman(frame+2,frame[0],word.flags,fixture.langopts.numbers,fixture.langopts.min_roman,fixture.langopts.max_roman,&value,&after);
    TEST_ASSERT(actual==old && value==old_value);
    TEST_ASSERT(old ? after==(size_t)(cursor-(char *)frame-2) : after==919);
    TEST_ASSERT(memcmp(frame,before,sizeof(frame))==0); ++roman_count;
}
int main(void)
{
    Translator *previous=translator; translator=&fixture;
    unsigned superscripts=0, spellings=0, variants=0, hungarians=0, groups=0;
    for(int code=-5; code<=0x110001; ++code) { TEST_ASSERT(espeak_rs_superscript(code)==ReferenceSuperscript(code)); ++superscripts; }
    for(unsigned trial=0; trial<200000; ++trial) {
        unsigned char source[200], expected[200], actual[200];
        memset(source,0x97,sizeof(source)); size_t length=Random()%199;
        for(size_t i=0; i<length; ++i) {
            static const unsigned char codes[]={6,6,6,21,255,255,5,7,11,23,128,129,254,31};
            source[i]=codes[Random()%(sizeof(codes)/sizeof(codes[0]))];
        }
        source[length]=0; memcpy(expected,source,sizeof(source)); memcpy(actual,source,sizeof(source));
        int control=(int)(Random()%8)-2, chars=(int)(Random()%20)-2; fixture.langopts.spelling_stress=(Random()&1)!=0;
        ReferenceSpelling(&fixture,(char *)expected,control,chars);
        int written=espeak_rs_spelling(actual,length+1,sizeof(actual),fixture.langopts.spelling_stress,control,chars);
        TEST_ASSERT(written>=0 && (size_t)written==strlen((char *)expected));
        TEST_ASSERT(memcmp(actual,expected,sizeof(actual))==0); ++spellings;
    }
    for(int option=0; option<8; ++option) for(int value=-10000; value<=10000; ++value) {
        fixture.langopts.numbers2=option*64+0x4000;
        TEST_ASSERT(strcmp(espeak_rs_number_variant(value,fixture.langopts.numbers2),ReferenceVariant(value))==0); ++variants;
    }
    for(unsigned trial=0; trial<100000; ++trial) {
        char word[5]; static const char bytes[]="aelz tbn";
        for(int i=0;i<4;++i)word[i]=bytes[Random()%(sizeof(bytes)-1)]; word[4]=0;
        int plex=(int)(Random()%5)-2, value=(int)(Random()%12000)-2000;
        TEST_ASSERT(espeak_rs_number_hungarian((unsigned char *)word,plex,value)==ReferenceHungarian(word,plex,value)); ++hungarians;
        char frame[19]; int digits=Random()%17;
        for(int i=0;i<19;++i)frame[i]=(Random()%5) ? (char)('0'+Random()%10) : ' ';
        TEST_ASSERT(espeak_rs_number_group((unsigned char *)frame+1,digits)==ReferenceGroup(frame+1,digits)); ++groups;
    }
    for(unsigned settings=0; settings<1024; ++settings) {
        static const char *const words[]={"","i","ii","iii","iiii","iv","v","vv","ix","x","xi","xix","xx","xl","l","lx","xc","c","cd","d","cm","m","mm","mmm","mmmm","iix","iviv","ixc","ic","vx","Il","ab","x7","i.x"};
        for(size_t i=0;i<sizeof(words)/sizeof(words[0]);++i)Roman(words[i],settings);
    }
    for(unsigned trial=0;trial<200000;++trial) {
        char word[34]; unsigned length=Random()%33;
        for(unsigned i=0;i<length;++i)word[i]="ixcmvldq"[Random()%8]; word[length]=0;
        Roman(word,Random()%1024);
    }
    for(int value=1;value<=4999;++value) {
        static const char *const names[]={"m","cm","d","cd","c","xc","l","xl","x","ix","v","iv","i"};
        static const int values[]={1000,900,500,400,100,90,50,40,10,9,5,4,1};
        char word[40]={0}; int remaining=value;
        for(int i=0;i<13;++i)while(remaining>=values[i]){strcat(word,names[i]);remaining-=values[i];}
        Roman(word,256); Roman(word,768); Roman(word,256|128); Roman(word,256|64);
    }
    /* Defined native failures: preflight must preserve every output byte. */
    unsigned char full[200], saved[200]; memset(full,31,sizeof(full)); full[199]=0; memcpy(saved,full,sizeof(full));
    TEST_ASSERT(espeak_rs_spelling(full,200,200,0,2,5)==-1 && memcmp(full,saved,sizeof(full))==0);
    full[199]=31; memcpy(saved,full,sizeof(full));
    TEST_ASSERT(espeak_rs_spelling(full,200,200,0,0,5)==-1 && memcmp(full,saved,sizeof(full))==0);
    TEST_ASSERT(espeak_rs_spelling(NULL,1,1,0,0,1)==-1);
    TEST_ASSERT(espeak_rs_number_group((unsigned char *)"x",-1)==0);
    TEST_ASSERT(espeak_rs_number_hungarian((unsigned char *)"",1,1000)==0);
    TEST_ASSERT(espeak_rs_number_hungarian((unsigned char *)"a",1,1000)==1);
    int value=77; size_t after=81; memset(full,'i',160);
    TEST_ASSERT(espeak_rs_number_roman(full,' ',0,0,1,10000,&value,&after)==0 && value==77 && after==81);
    translator=previous;
    printf("%u superscript; %u spelling/tail; %u variant; %u Hungarian; %u grouping; %u Roman retained-C comparisons passed\n",superscripts,spellings,variants,hungarians,groups,roman_count);
    return 0;
}
