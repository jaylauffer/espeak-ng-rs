/* Every native preset and alphabet range against the retained C setup.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "rust_data.h"
#undef USE_RUST_CORE
#define SelectTranslator ReferenceTranslator
#define AlphabetFromChar ReferenceAlphabet
#define ProcessLanguageOptions ReferenceProcessLanguageOptions
void ReferenceProcessLanguageOptions(LANGUAGE_OPTIONS *);
#include "language_profile_reference.inc"
#undef SelectTranslator
#undef AlphabetFromChar
#undef ProcessLanguageOptions

static unsigned long languages, alphabet_checks;
static void wide_pair(const wchar_t *a,const wchar_t *b,size_t count)
{
    TEST_ASSERT((a==NULL)==(b==NULL));
    if(a)TEST_ASSERT(memcmp(a,b,(count+1)*sizeof(*a))==0);
}
static void bytes_pair(const unsigned char *a,const unsigned char *b)
{
    TEST_ASSERT((a==NULL)==(b==NULL));
    if(a)TEST_ASSERT(strcmp((const char*)a,(const char*)b)==0);
}
static void language_pair(const char *name)
{
    Translator *expected=ReferenceTranslator(name);
    TEST_ASSERT(expected!=NULL);
    RustLanguageSetup setup;
    TEST_ASSERT(espeak_rs_language_setup(name,&setup)==0);
    Translator actual={0};
    espeak_rust_language_setup_commit(&actual,&setup);
    RustLanguageOptions eo,ao;
    espeak_rust_language_capture(expected,0,&eo);espeak_rust_language_capture(&actual,0,&ao);
    if(memcmp(&eo,&ao,sizeof(eo))!=0){fprintf(stderr,"Option mismatch for %s\n",name);abort();}
#define SCALAR(field) TEST_ASSERT(expected->field==actual.field)
    SCALAR(translator_name);SCALAR(transpose_min);SCALAR(transpose_max);SCALAR(encoding);SCALAR(letter_bits_offset);
    SCALAR(langopts.break_numbers);SCALAR(langopts.max_roman);SCALAR(langopts.min_roman);SCALAR(langopts.max_digits);
    SCALAR(langopts.accents);SCALAR(langopts.tone_language);SCALAR(langopts.long_stop);SCALAR(langopts.max_initial_consonants);
    SCALAR(langopts.tone_numbers);SCALAR(langopts.ideographs);SCALAR(langopts.textmode);SCALAR(langopts.dotless_i);SCALAR(langopts.listx);
    SCALAR(langopts.our_alphabet);SCALAR(langopts.alt_alphabet);SCALAR(langopts.alt_alphabet_lang);
    SCALAR(langopts.max_lengthmod);SCALAR(langopts.lengthen_tonic);SCALAR(langopts.suffix_add_e);
#undef SCALAR
    TEST_ASSERT(strcmp(expected->dictionary_name,actual.dictionary_name)==0);
    TEST_ASSERT(memcmp(expected->letter_bits,actual.letter_bits,256)==0);
    TEST_ASSERT(memcmp(expected->punct_to_tone,actual.punct_to_tone,48)==0);
    TEST_ASSERT((expected->transpose_map==NULL)==(actual.transpose_map==NULL));
    if(expected->transpose_map)TEST_ASSERT(memcmp(expected->transpose_map,actual.transpose_map,expected->transpose_max-expected->transpose_min+1)==0);
    TEST_ASSERT((expected->frequent_pairs==NULL)==(actual.frequent_pairs==NULL));
    if(expected->frequent_pairs){int i=0;do{TEST_ASSERT(expected->frequent_pairs[i]==actual.frequent_pairs[i]);}while(expected->frequent_pairs[i++]!=0x7fff);}
    TEST_ASSERT(memcmp(expected->langopts.length_mods,actual.langopts.length_mods,100)==0);
    TEST_ASSERT(memcmp(expected->langopts.length_mods0,actual.langopts.length_mods0,100)==0);
    wide_pair(expected->char_plus_apostrophe,actual.char_plus_apostrophe,wcslen(expected->char_plus_apostrophe));
    /* xex has one declared wchar_t and lacks a legacy terminator. Compare only
     * that declared unit; native setup supplies its missing terminator. */
    if(expected->translator_name==L3('x','e','x')){
        TEST_ASSERT(expected->punct_within_word[0]==actual.punct_within_word[0]);TEST_ASSERT(actual.punct_within_word[1]==0);
    }else wide_pair(expected->punct_within_word,actual.punct_within_word,wcslen(expected->punct_within_word));
    for(int i=0;i<8;i++)wide_pair(expected->letter_groups[i],actual.letter_groups[i],expected->letter_group_lengths[i]);
    TEST_ASSERT(memcmp(expected->letter_group_lengths,actual.letter_group_lengths,sizeof(actual.letter_group_lengths))==0);
    int i=0;do{TEST_ASSERT(expected->chars_ignore[i]==actual.chars_ignore[i]);TEST_ASSERT(expected->chars_ignore[i+1]==actual.chars_ignore[i+1]);i+=2;}while(expected->chars_ignore[i-2]);
    bytes_pair((const unsigned char*)expected->langopts.ordinal_indicator,(const unsigned char*)actual.langopts.ordinal_indicator);
    bytes_pair(expected->langopts.roman_suffix,actual.langopts.roman_suffix);
    TEST_ASSERT(actual.langopts.replace_chars==NULL);
    free(expected);languages++;
}
int main(void)
{
    /* Exhaust the valid short namespace, including default-only languages.
     * Longer switch names are emitted from the actual case selectors. */
    char name[40]={0};language_pair(name);
    for(int a='a';a<='z';a++){name[0]=a;name[1]=0;language_pair(name);
        for(int b='a';b<='z';b++){name[1]=b;name[2]=0;language_pair(name);
            for(int c='a';c<='z';c++){name[2]=c;name[3]=0;language_pair(name);}}
    }
#include "language_names_reference.inc"
    for(int c=-100;c<=0x110100;c++){
        const ALPHABET *expected=ReferenceAlphabet(c),*actual=AlphabetFromChar(c);
        TEST_ASSERT((expected==NULL)==(actual==NULL));
        if(expected){TEST_ASSERT(strcmp(expected->name,actual->name)==0);TEST_ASSERT(expected->offset==actual->offset);
            TEST_ASSERT(expected->range_min==actual->range_min);TEST_ASSERT(expected->range_max==actual->range_max);
            TEST_ASSERT(expected->language==actual->language);TEST_ASSERT(expected->flags==actual->flags);}
        alphabet_checks++;
    }
    memset(name,'a',39);name[39]=0;language_pair(name);
    RustLanguageSetup untouched,copy;memset(&untouched,0xa5,sizeof(untouched));copy=untouched;
    char overlong[41];memset(overlong,'a',40);overlong[40]=0;
    TEST_ASSERT(espeak_rs_language_setup(overlong,&untouched)==2);TEST_ASSERT(memcmp(&untouched,&copy,sizeof(copy))==0);
    printf("Compared %lu language presets and %lu alphabet classifications\n",languages,alphabet_checks);
    return 0;
}
