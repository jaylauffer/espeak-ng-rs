/* Frozen test fixture: the C replacement projections translate.c had before its
 * callbacks moved to Rust (rust/translate_engine_compat.rs), copied as the
 * test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native replacement projections. */
typedef struct { Translator *tr; char *word, *text; WORD_TAB *rows; int remaining; } RustReplacementHost;
static int ReplacementValue(void *opaque, unsigned field)
{
    RustReplacementHost *h=opaque;
    switch(field){case 0:return h->tr->langopts.ideographs;case 1:return h->tr->langopts.alt_alphabet;
    case 2:return h->rows!=NULL?(int)h->rows->flags:0;case 3:return dictionary_skipwords;default:return 0;}
}
static int ReplacementStore(void *opaque, unsigned field, int value)
{
    RustReplacementHost *h=opaque;
    if(field==2 && h->rows!=NULL)h->rows->flags=(unsigned)value;
    else if(field==3)dictionary_skipwords=value;else return -1;return 0;
}
static int ReplacementClass(void *opaque, unsigned code, unsigned kind)
{
    RustReplacementHost *h=opaque;
    switch(kind){case 0:return IsAlpha(code);case 1:return IsSpace(code);case 2:return iswupper(code)!=0;
    case 3:return code<128 && isspace((unsigned char)code)!=0;
    case 4:return wcschr(h->tr->punct_within_word,code)!=NULL;default:return 0;}
}
static unsigned ReplacementLower(void *opaque, unsigned code)
{
    RustReplacementHost *h=opaque;return (unsigned)towlower2(code,h->tr);
}
static int ReplacementMain(void *opaque, char *source, size_t initialized, size_t offset, char *text, unsigned *flags)
{
    RustReplacementHost *h=opaque;
    char *word=h->word,*destination=text;
    const char *base=h->tr->rule_text_base;size_t length=h->tr->rule_text_length;
    if(source!=NULL){if(initialized>3+N_WORD_BYTES*2||offset>=initialized)return -1;
        word=source+offset;h->tr->rule_text_base=source;h->tr->rule_text_length=initialized;}
    else if(text!=NULL)destination=h->text;
    *flags=TranslateWord3(h->tr,word,h->rows,h->remaining,destination,&any_stressed_words,
        current_alphabet,word_phonemes,sizeof(word_phonemes));
    if(source!=NULL){h->tr->rule_text_base=base;h->tr->rule_text_length=length;}
    if(source==NULL && text!=NULL && (*flags&FLAG_TEXTMODE)) {
        if(destination==NULL)return -1;
        const char *end=memchr(destination,0,N_WORD_BYTES+1);if(end==NULL)return -1;
        memcpy(text,destination,(size_t)(end-destination)+1);
    }
    return 0;
}
static int ReplacementSnapshot(void *opaque, char *out)
{
    (void)opaque;memcpy(out,word_phonemes,N_WORD_PHONEMES);return 0;
}
static int ReplacementPublish(void *opaque, const char *output)
{
    (void)opaque;const char *end=memchr(output,0,N_WORD_PHONEMES);if(end==NULL)return -1;
    memcpy(word_phonemes,output,(size_t)(end-output)+1);return 0;
}
static RustReplacement ReplacementCallbacks(RustReplacementHost *h)
{
    return (RustReplacement){h,ReplacementValue,ReplacementStore,ReplacementClass,ReplacementLower,
        ReplacementMain,ReplacementSnapshot,ReplacementPublish};
}
#ifdef ESPEAK_RUST_CORE_TEST
static bool ShouldSplitIdeographs(Translator *tr, int c, int previous)
{
    if(tr==NULL)return false;RustReplacementHost h={.tr=tr};RustReplacement callbacks=ReplacementCallbacks(&h);
    return espeak_rs_split_ideographs(&callbacks,(unsigned)c,(unsigned)previous)!=0;
}
#endif
static int TranslateWordWithBounds(Translator *tr, char *word, WORD_TAB *rows, int remaining, char *text, int depth)
{
    if(tr==NULL||word==NULL)return 0;
    RustReplacementHost h={tr,word,text,rows,remaining};RustReplacement callbacks=ReplacementCallbacks(&h);unsigned flags=0;
    return espeak_rs_replacement(&callbacks,text!=NULL,depth,&flags)==0?(int)flags:0;
}
