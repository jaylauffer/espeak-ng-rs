/* Frozen test fixture: the C text control projections translate.c had before its
 * callbacks moved to Rust (rust/translate_engine_compat.rs), copied as the
 * test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native text control projections. */
typedef struct { Translator *tr; char *word; size_t length; const char *base; size_t base_length;
    WORD_TAB *rows; int remaining; unsigned char *count; char *pronunciation; int *flags; } RustTextHost;
static int ControlValue(void *opaque,unsigned field)
{
    RustTextHost *h=opaque;switch(field){
    case 0:return embedded_ix;case 1:return embedded_read;case 2:return option_sayas;case 3:return option_sayas2;
    case 4:return count_sayas_digits;case 5:return word_emphasis;case 6:return option_emphasis;case 7:return pre_pause;
    case 8:return h->tr?h->tr->langopts.param[LOPT_COMBINE_WORDS]:0;case 9:return h->remaining;
    case 10:return dictionary_skipwords;case 11:return h->flags?*h->flags:0;case 12:return count_sentences;
    case 13:return count_words;case 14:return end_character_position;case 15:return skip_sentences;
    case 16:return skip_words;case 17:return skip_characters;case 18:return skipping_text;case 19:return new_sentence;
    case 20:return embedded_flag;case 21:return h->length<=840?(int)h->length:-1;default:return -1;}
}
static int ControlStore(void *opaque,unsigned field,int value)
{
    RustTextHost *h=opaque;if((field==0||field==1)&&(value<0||value>N_EMBEDDED_LIST))return -1;
    switch(field){case 0:embedded_ix=value;break;case 1:embedded_read=value;break;case 2:option_sayas=value;break;
    case 3:option_sayas2=value;break;case 4:count_sayas_digits=value;break;case 5:word_emphasis=value;break;
    case 6:option_emphasis=value;break;case 7:pre_pause=value;break;case 10:dictionary_skipwords=value;break;
    case 11:if(h->flags==NULL)return -1;*h->flags=value;break;case 12:count_sentences=value;break;
    case 13:count_words=value;break;case 14:end_character_position=value;break;case 15:skip_sentences=value;break;
    case 16:skip_words=value;break;case 17:skip_characters=value;break;case 18:skipping_text=value!=0;break;
    case 19:new_sentence=value!=0;break;case 20:embedded_flag=value;break;default:return -1;}return 0;
}
static int ControlByte(void *opaque,unsigned kind,size_t index)
{
    RustTextHost *h=opaque;switch(kind){case 0:return index<sizeof(source)?(unsigned char)source[index]:-1;
    case 1:return h->word&&index<h->length?(unsigned char)h->word[index]:-1;
    case 2:return h->pronunciation&&index<N_WORD_PHONEMES?(unsigned char)h->pronunciation[index]:-1;
    case 3:return h->count&&index<N_WORD_PHONEMES?h->count[index]:-1;
    case 4:return index<sizeof(skip_marker)?(unsigned char)skip_marker[index]:-1;default:return -1;}
}
static int ControlWrite(void *opaque,unsigned kind,size_t index,unsigned char value)
{
    RustTextHost *h=opaque;switch(kind){case 0:if(index>=sizeof(source))return -1;source[index]=(char)value;break;
    case 1:if(h->word==NULL||index>=h->length)return -1;h->word[index]=(char)value;break;
    case 2:if(h->pronunciation==NULL||index>=N_WORD_PHONEMES)return -1;h->pronunciation[index]=(char)value;break;
    case 4:if(index>=sizeof(skip_marker))return -1;skip_marker[index]=(char)value;break;default:return -1;}return 0;
}
static int ControlCommand(void *opaque,int index,unsigned *out){(void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;*out=embedded_list[index];return 0;}
static int ControlPutCommand(void *opaque,int index,unsigned value){(void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;embedded_list[index]=value;return 0;}
static int ControlDefault(void *opaque,size_t index,int *out){(void)opaque;if(index>=N_EMBEDDED_VALUES)return -1;*out=embedded_default[index];return 0;}
static int ControlAlpha(void *opaque,unsigned code){(void)opaque;return iswalpha(code)!=0;}
static unsigned char ControlLower(void *opaque,unsigned char code){(void)opaque;return (unsigned char)tolower(code);}
static int ControlRow(void *opaque,size_t index,unsigned *out){RustTextHost *h=opaque;if(h->rows==NULL||h->remaining<0||index>=(size_t)h->remaining)return -1;*out=h->rows[index].flags;return 0;}
static int ControlPutRow(void *opaque,size_t index,unsigned flags){RustTextHost *h=opaque;if(h->rows==NULL||h->remaining<0||index>=(size_t)h->remaining)return -1;h->rows[index].flags=flags;return 0;}
static int ControlTranslate(void *opaque,unsigned main,size_t offset,size_t row,size_t remaining,unsigned *out)
{
    RustTextHost *h=opaque;Translator *tr=main?translator:h->tr;
    if(tr==NULL||h->word==NULL||h->base==NULL||h->base_length==0||h->length>840||h->rows==NULL||h->remaining<0||offset>=h->length||row>=(size_t)h->remaining||remaining>(size_t)h->remaining-row)return -1;
    const char *saved=tr->rule_text_base;size_t saved_length=tr->rule_text_length;
    tr->rule_text_base=h->base;tr->rule_text_length=h->base_length;
    *out=TranslateWordWithBounds(tr,h->word+offset,h->rows+row,(int)remaining,NULL,0);
    tr->rule_text_base=saved;tr->rule_text_length=saved_length;return 0;
}
static int ControlVowel(void *opaque,unsigned char code,int *out){(void)opaque;PHONEME_TAB *ph=phoneme_tab[code];if(ph==NULL)return -1;*out=ph->type==phVOWEL;return 0;}
static int ControlLookup(void *opaque,const char *name,int *out){(void)opaque;if(memchr(name,0,12)==NULL)return -1;*out=LookupPhonemeTable(name);return 0;}
static int ControlSelect(void *opaque,int table){(void)opaque;SelectPhonemeTable(table);return 0;}
static int ControlInit(void *opaque,unsigned which){(void)opaque;if(which==0)InitText2();else if(which==1)InitNamedata();else return -1;return 0;}
static RustTextControl ControlCallbacks(RustTextHost *h)
{
    return (RustTextControl){h,ControlValue,ControlStore,ControlByte,ControlWrite,ControlCommand,ControlPutCommand,
        ControlDefault,ControlAlpha,ControlLower,ControlRow,ControlPutRow,ControlTranslate,ControlVowel,ControlLookup,ControlSelect,ControlInit};
}
static void ControlWordBounds(RustTextHost *h)
{
    const Translator *owners[4]={h->tr,translator,translator2,translator3};
    for(unsigned i=0;i<4;i++){const Translator *tr=owners[i];if(tr==NULL||tr->rule_text_base==NULL)continue;
        uintptr_t base=(uintptr_t)tr->rule_text_base,at=(uintptr_t)h->word;
        if(at>=base&&at-base<tr->rule_text_length){h->base=tr->rule_text_base;h->base_length=tr->rule_text_length;h->length=tr->rule_text_length-(at-base);return;}}
}
static void Word_EmbeddedCmd(void){RustTextHost h={0};RustTextControl c=ControlCallbacks(&h);RustTextResult out;(void)espeak_rs_text_control(&c,1,0,&out);}
static int EmbeddedCommand(unsigned *index){if(index==NULL)return 0;RustTextHost h={0};RustTextControl c=ControlCallbacks(&h);RustTextResult out;
    if(espeak_rs_text_control(&c,0,*index,&out)!=0)return 0;*index=(unsigned)out.index;return (int)out.value;}
static void CombineFlag(Translator *tr,WORD_TAB *rows,int remaining,char *word,int *flags,unsigned char *p,char *pronunciation)
{
    RustTextHost h={.tr=tr,.word=word,.rows=rows,.remaining=remaining,.count=p,.pronunciation=pronunciation,.flags=flags};
    ControlWordBounds(&h);RustTextControl c=ControlCallbacks(&h);RustTextResult out;(void)espeak_rs_text_control(&c,2,0,&out);
}
static void SwitchLanguage(char *word,char *pronunciation){RustTextHost h={.word=word,.pronunciation=pronunciation};
    ControlWordBounds(&h);RustTextControl c=ControlCallbacks(&h);RustTextResult out;(void)espeak_rs_text_control(&c,3,0,&out);}
void InitText(int control){RustTextHost h={0};RustTextControl c=ControlCallbacks(&h);RustTextResult out;(void)espeak_rs_text_control(&c,4,(unsigned)control,&out);}
