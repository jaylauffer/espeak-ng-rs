/* Frozen test fixture: the C word-to-list projections translate.c had before its
 * callbacks moved to Rust (rust/translate_engine_compat.rs), copied as the
 * test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native word-to-list projections. */
typedef struct { Translator *tr; char *word; WORD_TAB *rows; int remaining; size_t source_length; } RustWord2Host;
static int Word2Value(void *opaque, unsigned field)
{
    RustWord2Host *h=opaque;
    switch(field) {
    case 0:return h->rows->length; case 1:return h->rows->sourceix; case 2:return h->rows->flags;
    case 3:return h->rows[-1].flags; case 4:return h->remaining>1?h->rows[1].flags:0; case 5:return h->remaining;
    case 6:return n_ph_list2; case 7:return embedded_flag; case 8:return embedded_ix;
    case 9:return option_sayas; case 10:return option_capitals; case 11:return option_emphasis;
    case 12:return h->tr->prepause_timeout; case 13:return h->tr->end_stressed_vowel;
    case 14:return h->tr->prev_dict_flags[0]; case 15:return h->tr->prev_dict_flags[1];
    case 16:return any_stressed_words; case 17:return voice->phoneme_tab_ix;
    default:return 0;
    }
}
static int Word2Store(void *opaque, unsigned field, int value)
{
    RustWord2Host *h=opaque;
    switch(field) {
    case 2:h->rows->flags=value;break;
    case 6:if(value<0||value>=N_PHONEME_LIST)return -1;n_ph_list2=value;break;
    case 7:embedded_flag=value;break;
    case 8:if(value<0||value>N_EMBEDDED_LIST)return -1;embedded_ix=value;break;
    case 12:h->tr->prepause_timeout=value;break;case 13:h->tr->end_stressed_vowel=value;break;
    case 14:h->tr->prev_dict_flags[0]=value;break;case 15:h->tr->prev_dict_flags[1]=value;break;
    case 16:any_stressed_words=value!=0;break;default:return -1;
    }
    return 0;
}
static int Word2Byte(void *opaque, size_t offset, unsigned pronunciation)
{
    RustWord2Host *h=opaque;
    if(offset >= (pronunciation?N_WORD_PHONEMES:h->source_length))return -1;
    return (unsigned char)(pronunciation?word_phonemes:h->word)[offset];
}
static int Word2Write(void *opaque, size_t offset, unsigned char byte, unsigned pronunciation)
{
    RustWord2Host *h=opaque;
    if(offset >= (pronunciation?N_WORD_PHONEMES:h->source_length))return -1;
    (pronunciation?word_phonemes:h->word)[offset]=(char)byte;return 0;
}
static int Word2Entry(void *opaque, int index, PHONEME_LIST2 *out)
{
    (void)opaque;if(index<0||index>=N_PHONEME_LIST)return -1;*out=ph_list2[index];return 0;
}
static int Word2Put(void *opaque, int index, const PHONEME_LIST2 *entry)
{
    (void)opaque;if(index<0||index>=N_PHONEME_LIST)return -1;ph_list2[index]=*entry;return 0;
}
static int Word2Command(void *opaque, int index, unsigned *out)
{
    (void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;*out=embedded_list[index];return 0;
}
static int Word2PutCommand(void *opaque, int index, unsigned value)
{
    (void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;embedded_list[index]=value;return 0;
}
static int Word2Embedded(void *opaque){(void)opaque;Word_EmbeddedCmd();return 0;}
static int Word2Encode(void *opaque, unsigned language)
{
    RustWord2Host *h=opaque;int bad;
    if(language)SwitchLanguage(h->word,word_phonemes);
    else if(EncodePhonemesBounded(h->word,word_phonemes,sizeof(word_phonemes),&bad)==NULL)return -1;
    return 0;
}
static int Word2Translate(void *opaque, unsigned alternate, unsigned mode, char *replacement, unsigned *flags)
{
    RustWord2Host *h=opaque;Translator *tr=alternate?translator2:translator;
    if(tr==NULL)return -1;
    if(mode) {
        const char *base=tr->rule_text_base;size_t length=tr->rule_text_length;
        tr->rule_text_base=replacement;tr->rule_text_length=N_WORD_BYTES+3;
        *flags=TranslateWordWithBounds(tr,replacement+1,h->rows,h->remaining,NULL,0);
        tr->rule_text_base=base;tr->rule_text_length=length;
    } else *flags=TranslateWordWithBounds(tr,h->word,h->rows,h->remaining,replacement+2,0);
    return 0;
}
static int Word2Combine(void *opaque, unsigned *flags)
{
    RustWord2Host *h=opaque;int result=(int)*flags;
    CombineFlag(h->tr,h->rows,h->remaining,h->word,&result,(unsigned char *)word_phonemes,word_phonemes);
    *flags=(unsigned)result;return 0;
}
static int Word2Dictionary(void *opaque, char *out)
{
    (void)opaque;const char *end=memchr(dictionary_name,0,40);if(end==NULL)return -1;
    memcpy(out,dictionary_name,(size_t)(end-dictionary_name)+1);return 0;
}
static int Word2RestoreDictionary(void *opaque, const char *name)
{
    (void)opaque;const char *end=memchr(name,0,40);if(end==NULL)return -1;
    memcpy(dictionary_name,name,(size_t)(end-name)+1);return 0;
}
static int Word2Alternate(void *opaque, const char *name, int *table)
{
    (void)opaque;*table=SetTranslator2(name[0]?name:ESPEAKNG_DEFAULT_VOICE);return 0;
}
static int Word2Select(void *opaque, int table){(void)opaque;SelectPhonemeTable(table);return 0;}
static int Word2Phoneme(void *opaque, unsigned char code, RustWord2Phoneme *out)
{
    (void)opaque;PHONEME_TAB *ph=phoneme_tab[code];if(ph==NULL)return -1;
    *out=(RustWord2Phoneme){ph->type,ph->std_length,ph->program};return 0;
}
static int Word2Alpha(void *opaque, unsigned char code){(void)opaque;return IsAlpha(code);}
static void Word2Invalid(void *opaque, unsigned char code){(void)opaque;printf("Invalid phoneme code %d\n",code);}
static int TranslateWord2(Translator *tr, char *word, WORD_TAB *rows, int remaining, int pause)
{
    if(tr==NULL||word==NULL||rows==NULL)return 0;
    RustWord2Host h={tr,word,rows,remaining,0};
    const Translator *owners[4]={tr,translator,translator2,translator3};
    for(unsigned i=0;i<4;i++) {
        const Translator *owner=owners[i];if(owner==NULL||owner->rule_text_base==NULL)continue;
        uintptr_t address=(uintptr_t)word,base=(uintptr_t)owner->rule_text_base;
        if(address>=base && address-base<owner->rule_text_length){h.source_length=owner->rule_text_length-(address-base);break;}
    }
    if(h.source_length==0)h.source_length=strlen(word)+1;
    RustTranslateWord2 callbacks={&h,Word2Value,Word2Store,Word2Byte,Word2Write,Word2Entry,Word2Put,
        Word2Command,Word2PutCommand,Word2Embedded,Word2Encode,Word2Translate,Word2Combine,
        Word2Dictionary,Word2RestoreDictionary,Word2Alternate,Word2Select,Word2Phoneme,Word2Alpha,Word2Invalid};
    unsigned flags=0;
    return espeak_rs_translate_word2(&callbacks,pause,&flags)==0?(int)flags:0;
}
