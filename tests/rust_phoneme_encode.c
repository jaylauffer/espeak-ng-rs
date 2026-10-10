/* Differential oracle is the actual 98f1dc3c C encoder, not a transcription.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "dictionary.h"
#include "rust_data.h"
#include "common.h"
#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define EncodePhonemes ReferenceEncode
#include "phoneme_encode_reference.inc"
#undef EncodePhonemes

static uint32_t random_state = 0x119ce321;
static uint32_t Next(void) {
    random_state ^= random_state << 13;
    random_state ^= random_state >> 17;
    random_state ^= random_state << 5;
    return random_state;
}
static PHONEME_TAB records[256];
static uint32_t Locale(unsigned byte) {
    return (unsigned)tolower((unsigned char)byte) | (isspace((unsigned char)byte) ? 256u : 0u);
}
static void Table(unsigned episode) {
    memset(records,0,sizeof(records));
    n_phoneme_tab = episode%257;
    for(unsigned i=0;i<256;i++) {
        phoneme_tab[i] = Next()%7==0 ? NULL : &records[i];
        records[i].code=(unsigned char)Next();
        records[i].type=Next()%8==0 ? phINVALID : phFRICATIVE;
        records[i].mnemonic=Next();
        if(i%3==0)records[i].mnemonic&=0xffffff;
        if(i%5==0)records[i].mnemonic&=0xffff;
    }
    if(n_phoneme_tab>8) {
        for(unsigned i=1;i<9;i++) { phoneme_tab[i]=&records[i];records[i].type=phFRICATIVE;records[i].code=(unsigned char)(i+30); }
        records[1].mnemonic='a'; records[2].mnemonic='a'|('b'<<8); records[3].mnemonic=records[2].mnemonic;
        records[4].mnemonic='|'|('|'<<8);records[4].code=phonEND_WORD;
        records[5].mnemonic='_'|('^'<<8)|('_'<<16);records[5].code=phonSWITCH;
        records[6].mnemonic=0;records[6].type=episode%3 ? phINVALID : phFRICATIVE;
        records[7].mnemonic='a'|('b'<<8)|('c'<<16)|('d'<<24);
        records[8].mnemonic=Next()&0xff;
    }
}
static unsigned compared, short_guards;
static void Compare(const char *input) {
    char expected[1024], actual[1024], bounded[1024];
    memset(expected,0xa5,sizeof(expected));memset(actual,0xa5,sizeof(actual));memset(bounded,0xa5,sizeof(bounded));
    int bad_expected=0x7788,bad_actual=0x7788,bad_bounded=0x7788;
    const char *end=ReferenceEncode(input,expected,&bad_expected);
    const char *next=EncodePhonemes(input,actual,&bad_actual);
    TEST_ASSERT(next==end);TEST_ASSERT(bad_expected==bad_actual);TEST_ASSERT(memcmp(expected,actual,sizeof(actual))==0);
    size_t footprint=sizeof(expected);
    while(footprint && (unsigned char)expected[footprint-1]==0xa5)footprint--;
    /* All emitted terminators are zero, so last changed byte is the last
     * write even when an earlier output byte happens to equal the sentinel. */
    TEST_ASSERT(footprint>0);
    next=EncodePhonemesBounded(input,bounded,footprint,&bad_bounded);
    TEST_ASSERT(next==end);TEST_ASSERT(bad_expected==bad_bounded);TEST_ASSERT(memcmp(expected,bounded,sizeof(bounded))==0);
    memset(bounded,0xa5,sizeof(bounded));bad_bounded=0x7788;
    TEST_ASSERT(EncodePhonemesBounded(input,bounded,footprint-1,&bad_bounded)==NULL);
    TEST_ASSERT(bad_bounded==0x7788);
    for(unsigned i=0;i<sizeof(bounded);i++)TEST_ASSERT((unsigned char)bounded[i]==0xa5);
    short_guards++;
    memset(actual,0xa5,sizeof(actual));next=EncodePhonemes(input,actual,NULL);
    TEST_ASSERT(next==end);TEST_ASSERT(memcmp(expected,actual,sizeof(actual))==0);
    compared++;
}
int main(void) {
    PHONEME_TAB *saved[256];memcpy(saved,phoneme_tab,sizeof(saved));int saved_count=n_phoneme_tab;
    static const char *cases[]={""," \t\r\n","a","ab","abc","abcd","a|ab||a|","_^_EN","_^_en","_^_RU tail",
        "_^_EN \tab","_^_","_^_EN\t","\xc3\xa9","a\xe2","a\x80\xc3\xa9","\xff","a\xc0\xaf","a\xe2\x41\xa9",
        "ab\vnext","||||","_^_ABCDEFGHIJKLMNOP", "_^_EN|", "abcdabcda"};
    for(unsigned episode=0;episode<10000;episode++) {
        Table(episode);
        for(unsigned i=0;i<sizeof(cases)/sizeof(cases[0]);i++) Compare(cases[i]);
        char input[256]={0};unsigned length=Next()%255;
        static const unsigned char alphabet[]="abcde_|^ ENru\t\n\xc3\xa9\xe2\x80\xff";
        for(unsigned i=0;i<length;i++) input[i]=(char)alphabet[Next()%(sizeof(alphabet)-1)];
        Compare(input);
    }
    /* Explicitly exercise every unsigned input byte and all four mnemonic
     * widths; records' code is deliberately different from the table index. */
    for(unsigned byte=1;byte<256;byte++) {
        n_phoneme_tab=2;phoneme_tab[1]=&records[1];
        records[1].type=phFRICATIVE;records[1].code=41;
        char input[8]={(char)byte,'b','c','d',0};
        records[1].mnemonic=byte;Compare(input);
        records[1].mnemonic=byte|('b'<<8);Compare(input);
        records[1].mnemonic=byte|('b'<<8)|('c'<<16);Compare(input);
        records[1].mnemonic=byte|('b'<<8)|('c'<<16)|('d'<<24);Compare(input);
    }
    n_phoneme_tab=2;records[1].mnemonic='_'|('^'<<8)|('_'<<16);records[1].code=phonSWITCH;
    char long_language[256]="_^_";memset(long_language+3,'A',250);long_language[253]=0;
    Compare(long_language);
    char output[16];memset(output,0x55,sizeof(output));int bad=0x7788;
    TEST_ASSERT(EncodePhonemesBounded(NULL,output,sizeof(output),&bad)==NULL);
    TEST_ASSERT(EncodePhonemesBounded("a",NULL,16,&bad)==NULL);
    TEST_ASSERT(EncodePhonemesBounded("a",output,0,&bad)==NULL);
    char alias[16]="ab";TEST_ASSERT(EncodePhonemesBounded(alias,alias,sizeof(alias),&bad)==NULL);
    TEST_ASSERT(strcmp(alias,"ab")==0 && bad==0x7788);
    TEST_ASSERT(EncodePhonemesBounded(long_language,output,sizeof(output),&bad)==NULL);
    const PHONEME_TAB *const *table=(const PHONEME_TAB *const *)phoneme_tab;
    TEST_ASSERT(espeak_rs_encode_phonemes("a",(unsigned char *)output,16,&bad,NULL,2,"en",Locale)==NULL);
    TEST_ASSERT(espeak_rs_encode_phonemes("a",(unsigned char *)output,16,&bad,table,2,NULL,Locale)==NULL);
    TEST_ASSERT(espeak_rs_encode_phonemes("a",(unsigned char *)output,16,&bad,table,2,"en",NULL)==NULL);
    union { int integer; unsigned char bytes[16]; } overlap;
    memset(&overlap,0x55,sizeof(overlap));
    TEST_ASSERT(espeak_rs_encode_phonemes("a",overlap.bytes,16,&overlap.integer,table,2,"en",Locale)==NULL);
    for(unsigned i=0;i<sizeof(overlap);i++)TEST_ASSERT(((unsigned char *)&overlap)[i]==0x55);
    n_phoneme_tab=257;TEST_ASSERT(EncodePhonemesBounded("a",output,sizeof(output),&bad)==NULL);
    n_phoneme_tab=-1;TEST_ASSERT(EncodePhonemesBounded("a",output,sizeof(output),&bad)==NULL);
    for(unsigned i=0;i<sizeof(output);i++)TEST_ASSERT(output[i]==0x55);
    TEST_ASSERT(bad==0x7788);
    memcpy(phoneme_tab,saved,sizeof(saved));n_phoneme_tab=saved_count;
    printf("%u actual retained-C encoder cases and %u atomic short-buffer guards passed\n",compared,short_guards);
    return 0;
}
