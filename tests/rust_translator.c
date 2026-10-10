/* Typed native ownership, layout and pinned snapshot lifetimes.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "rust_data.h"
static void Layout(void)
{
    TEST_ASSERT(espeak_rs_translator_layout(0,0)==sizeof(Translator));
    TEST_ASSERT(espeak_rs_translator_layout(1,0)==_Alignof(Translator));
    TEST_ASSERT(espeak_rs_translator_layout(2,0)==sizeof(LANGUAGE_OPTIONS));
    TEST_ASSERT(espeak_rs_translator_layout(3,0)==_Alignof(LANGUAGE_OPTIONS));
    {const size_t offsets[]={
        offsetof(Translator,langopts),
        offsetof(Translator,translator_name),
        offsetof(Translator,transpose_max),
        offsetof(Translator,transpose_min),
        offsetof(Translator,transpose_map),
        offsetof(Translator,dictionary_name),
        offsetof(Translator,phonemes_repeat),
        offsetof(Translator,phonemes_repeat_count),
        offsetof(Translator,phoneme_tab_ix),
        offsetof(Translator,stress_amps),
        offsetof(Translator,stress_lengths),
        offsetof(Translator,dict_condition),
        offsetof(Translator,dict_min_size),
        offsetof(Translator,encoding),
        offsetof(Translator,char_plus_apostrophe),
        offsetof(Translator,punct_within_word),
        offsetof(Translator,chars_ignore),
        offsetof(Translator,letter_bits),
        offsetof(Translator,letter_bits_offset),
        offsetof(Translator,letter_groups),
        offsetof(Translator,letter_group_lengths),
        offsetof(Translator,punct_to_tone),
        offsetof(Translator,data_dictrules),
        offsetof(Translator,data_dictlist),
        offsetof(Translator,data_dict_size),
        offsetof(Translator,rust_dictionary_owner),
        offsetof(Translator,rule_text_base),
        offsetof(Translator,rule_text_length),
        offsetof(Translator,rust_list_replacement),
        offsetof(Translator,rust_emoji_replacement),
        offsetof(Translator,dict_hashtab),
        offsetof(Translator,letterGroups),
        offsetof(Translator,groups1),
        offsetof(Translator,groups3),
        offsetof(Translator,groups2),
        offsetof(Translator,groups2_name),
        offsetof(Translator,n_groups2),
        offsetof(Translator,groups2_count),
        offsetof(Translator,groups2_start),
        offsetof(Translator,frequent_pairs),
        offsetof(Translator,expect_verb),
        offsetof(Translator,expect_past),
        offsetof(Translator,expect_verb_s),
        offsetof(Translator,expect_noun),
        offsetof(Translator,prev_last_stress),
        offsetof(Translator,clause_end),
        offsetof(Translator,word_vowel_count),
        offsetof(Translator,word_stressed_count),
        offsetof(Translator,clause_upper_count),
        offsetof(Translator,clause_lower_count),
        offsetof(Translator,prepause_timeout),
        offsetof(Translator,end_stressed_vowel),
        offsetof(Translator,prev_dict_flags),
        offsetof(Translator,clause_terminator),
    };for(unsigned i=0;i<sizeof(offsets)/sizeof(offsets[0]);i++)TEST_ASSERT(espeak_rs_translator_layout(4,i)==offsets[i]);}
    TEST_ASSERT(espeak_rs_translator_layout(4,UINT32_MAX)==SIZE_MAX);
    {const size_t offsets[]={
        offsetof(LANGUAGE_OPTIONS,word_gap),
        offsetof(LANGUAGE_OPTIONS,vowel_pause),
        offsetof(LANGUAGE_OPTIONS,stress_rule),
        offsetof(LANGUAGE_OPTIONS,stress_flags),
        offsetof(LANGUAGE_OPTIONS,unstressed_wd1),
        offsetof(LANGUAGE_OPTIONS,unstressed_wd2),
        offsetof(LANGUAGE_OPTIONS,param),
        offsetof(LANGUAGE_OPTIONS,length_mods),
        offsetof(LANGUAGE_OPTIONS,length_mods0),
        offsetof(LANGUAGE_OPTIONS,numbers),
        offsetof(LANGUAGE_OPTIONS,numbers2),
        offsetof(LANGUAGE_OPTIONS,break_numbers),
        offsetof(LANGUAGE_OPTIONS,max_roman),
        offsetof(LANGUAGE_OPTIONS,min_roman),
        offsetof(LANGUAGE_OPTIONS,thousands_sep),
        offsetof(LANGUAGE_OPTIONS,decimal_sep),
        offsetof(LANGUAGE_OPTIONS,max_digits),
        offsetof(LANGUAGE_OPTIONS,ordinal_indicator),
        offsetof(LANGUAGE_OPTIONS,roman_suffix),
        offsetof(LANGUAGE_OPTIONS,accents),
        offsetof(LANGUAGE_OPTIONS,tone_language),
        offsetof(LANGUAGE_OPTIONS,intonation_group),
        offsetof(LANGUAGE_OPTIONS,tunes),
        offsetof(LANGUAGE_OPTIONS,long_stop),
        offsetof(LANGUAGE_OPTIONS,max_initial_consonants),
        offsetof(LANGUAGE_OPTIONS,spelling_stress),
        offsetof(LANGUAGE_OPTIONS,tone_numbers),
        offsetof(LANGUAGE_OPTIONS,ideographs),
        offsetof(LANGUAGE_OPTIONS,textmode),
        offsetof(LANGUAGE_OPTIONS,dotless_i),
        offsetof(LANGUAGE_OPTIONS,listx),
        offsetof(LANGUAGE_OPTIONS,replace_chars),
        offsetof(LANGUAGE_OPTIONS,our_alphabet),
        offsetof(LANGUAGE_OPTIONS,alt_alphabet),
        offsetof(LANGUAGE_OPTIONS,alt_alphabet_lang),
        offsetof(LANGUAGE_OPTIONS,max_lengthmod),
        offsetof(LANGUAGE_OPTIONS,lengthen_tonic),
        offsetof(LANGUAGE_OPTIONS,suffix_add_e),
        offsetof(LANGUAGE_OPTIONS,lowercase_sentence),
    };for(unsigned i=0;i<sizeof(offsets)/sizeof(offsets[0]);i++)TEST_ASSERT(espeak_rs_translator_layout(5,i)==offsets[i]);}
    TEST_ASSERT(espeak_rs_translator_layout(5,UINT32_MAX)==SIZE_MAX);
    TEST_ASSERT(espeak_rs_translator_layout(99,0)==SIZE_MAX);
}
static void Ownership(void)
{
    TEST_ASSERT(espeak_rs_translator_create(NULL)==NULL);
    char long_name[41];memset(long_name,'a',40);long_name[40]=0;
    TEST_ASSERT(espeak_rs_translator_create(long_name)==NULL);
    espeak_rs_translator_destroy(NULL);
    Translator *first=espeak_rs_translator_create("en"),*second=espeak_rs_translator_create("en");
    void *cache=espeak_rs_dictionary_cache_create();TEST_ASSERT(first&&second&&cache);
    Translator before;memcpy(&before,first,sizeof(before));
    TEST_ASSERT(espeak_rs_translator_load(NULL,cache,"espeak-ng-data/en_dict","en")==2);
    TEST_ASSERT(espeak_rs_translator_load(first,NULL,"espeak-ng-data/en_dict","en")==2);
    TEST_ASSERT(espeak_rs_translator_load(first,cache,NULL,"en")==2);
    TEST_ASSERT(espeak_rs_translator_load(first,cache,"espeak-ng-data/en_dict",NULL)==2);
    TEST_ASSERT(memcmp(first,&before,sizeof(before))==0);
    /* Name aliases the admitted owner's field: capture it before I/O. */
    TEST_ASSERT(espeak_rs_translator_load(first,cache,"espeak-ng-data/en_dict",first->dictionary_name)==0);
    TEST_ASSERT(espeak_rs_translator_load(second,cache,"espeak-ng-data/en_dict","en")==0);
    TEST_ASSERT(first->data_dictlist==second->data_dictlist);
    TEST_ASSERT(first->rust_dictionary_owner!=second->rust_dictionary_owner);
    const unsigned char *bytes;size_t size,offset,buckets[N_HASH_DICT];RustRuleIndex rules;
    TEST_ASSERT(espeak_rs_dictionary_handle_view(first->rust_dictionary_owner,&bytes,&size,&rules,buckets,&offset)==0);
    TEST_ASSERT(bytes==(const unsigned char*)first->data_dictlist&&size==first->data_dict_size);
    TEST_ASSERT(first->data_dictrules==first->data_dictlist+offset);
    memcpy(&before,first,sizeof(before));
    TEST_ASSERT(espeak_rs_translator_load(first,cache,"missing-translator-fixture","fr")==1);
    TEST_ASSERT(memcmp(first,&before,sizeof(before))==0);
    espeak_rs_dictionary_cache_destroy(cache);
    TEST_ASSERT(first->data_dictlist==second->data_dictlist&&size>8);
    espeak_rs_translator_destroy(first);
    TEST_ASSERT(espeak_rs_dictionary_handle_view(second->rust_dictionary_owner,&bytes,&size,&rules,buckets,&offset)==0);
    TEST_ASSERT(bytes==(const unsigned char*)second->data_dictlist);
    espeak_rs_translator_destroy(second);
}
int main(void){Layout();Ownership();printf("Verified 54 translator and 39 language-option offsets, sizes, alignments, alias/ABI guards and pinned ownership\n");return 0;}
