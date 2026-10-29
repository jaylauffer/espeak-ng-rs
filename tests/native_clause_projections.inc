/* Frozen test fixture: the C clause projections translate.c had before its
 * callbacks moved to Rust (rust/translate_engine_compat.rs), copied as the
 * test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native clause projections. */
typedef struct { Translator *tr; int *tone,*terminator; char **voice; } RustClauseHost;
static char clause_voice_name[40];
static int ClauseValue(void *opaque,unsigned field)
{
    RustClauseHost *h=opaque;switch(field){
    case 0:return (int)(embedded_ix);
    case 1:return (int)(embedded_read);
    case 2:return (int)(pre_pause);
    case 3:return (int)(any_stressed_words);
    case 4:return (int)(count_characters);
    case 5:return (int)(clause_start_char);
    case 6:return (int)(count_words);
    case 7:return (int)(clause_start_word);
    case 8:return (int)(max_clause_pause);
    case 9:return (int)(new_sentence);
    case 10:return (int)(count_sentences);
    case 11:return (int)(skip_sentences);
    case 12:return (int)(skipping_text);
    case 13:return (int)(n_ph_list2);
    case 14:return (int)(h->tr->prev_last_stress);
    case 15:return (int)(h->tr->prepause_timeout);
    case 16:return (int)(h->tr->expect_verb);
    case 17:return (int)(h->tr->expect_noun);
    case 18:return (int)(h->tr->expect_past);
    case 19:return (int)(h->tr->expect_verb_s);
    case 20:return (int)(h->tr->phonemes_repeat_count);
    case 21:return (int)(h->tr->end_stressed_vowel);
    case 22:return (int)(h->tr->prev_dict_flags[0]);
    case 23:return (int)(h->tr->prev_dict_flags[1]);
    case 24:return (int)(h->tr->clause_terminator);
    case 25:return (int)(h->tr->langopts.tone_numbers);
    case 26:return (int)(option_sayas2);
    case 27:return (int)(count_sayas_digits);
    case 28:return (int)(option_phoneme_input);
    case 29:return (int)(h->tr->letter_bits_offset);
    case 30:return (int)(h->tr->langopts.param[LOPT_CAPS_IN_WORD]);
    case 31:return (int)(h->tr->translator_name);
    case 32:return (int)(h->tr->langopts.param[LOPT_APOSTROPHE]);
    case 33:return (int)(word_emphasis);
    case 34:return (int)(h->tr->langopts.break_numbers);
    case 35:return (int)(h->tr->langopts.thousands_sep);
    case 36:return (int)(h->tr->langopts.decimal_sep);
    case 37:return (int)(h->tr->langopts.max_digits);
    case 38:return (int)(skip_words);
    case 39:return (int)(dictionary_skipwords);
    case 40:return (int)(option_endpause);
    case 41:return (int)(n_phoneme_list);
    case 42:return (int)(h->tr->langopts.ideographs);
    case 43:return (int)(h->tr->langopts.alt_alphabet);
    case 44:return (int)(0);
    default:return 0;}
}
static int ClauseStore(void *opaque,unsigned field,int value)
{
    RustClauseHost *h=opaque;
    if((field==0||field==1)&&(value<0||value>N_EMBEDDED_LIST))return -1;
    if(field==13&&(value<0||value>N_PHONEME_LIST))return -1;
    switch(field){
    case 0:embedded_ix=value;break;
    case 1:embedded_read=value;break;
    case 2:pre_pause=value;break;
    case 3:any_stressed_words=value;break;
    case 4:count_characters=value;break;
    case 5:clause_start_char=value;break;
    case 6:count_words=value;break;
    case 7:clause_start_word=value;break;
    case 8:max_clause_pause=value;break;
    case 9:new_sentence=value;break;
    case 10:count_sentences=value;break;
    case 11:skip_sentences=value;break;
    case 12:skipping_text=value;break;
    case 13:n_ph_list2=value;break;
    case 14:h->tr->prev_last_stress=value;break;
    case 15:h->tr->prepause_timeout=value;break;
    case 16:h->tr->expect_verb=value;break;
    case 17:h->tr->expect_noun=value;break;
    case 18:h->tr->expect_past=value;break;
    case 19:h->tr->expect_verb_s=value;break;
    case 20:h->tr->phonemes_repeat_count=value;break;
    case 21:h->tr->end_stressed_vowel=value;break;
    case 22:h->tr->prev_dict_flags[0]=value;break;
    case 23:h->tr->prev_dict_flags[1]=value;break;
    case 24:h->tr->clause_terminator=value;break;
    case 26:option_sayas2=value;break;
    case 27:count_sayas_digits=value;break;
    case 38:skip_words=value;break;
    case 39:dictionary_skipwords=value;break;
    case 44:if(value!=0)return -1;current_alphabet=NULL;break;
    default:return -1;}return 0;
}
static int ClauseByte(void *opaque,size_t index){(void)opaque;return index<sizeof(source)?(unsigned char)source[index]:-1;}
static int ClauseWrite(void *opaque,size_t index,unsigned char value){(void)opaque;if(index>=sizeof(source))return -1;source[index]=(char)value;return 0;}
static int ClauseClass(void *opaque,unsigned code,unsigned kind)
{
    RustClauseHost *h=opaque;switch(kind){case 0:return IsAlpha(code);case 1:return IsSpace(code);
    case 2:return iswupper(code)!=0;case 3:return iswlower(code)!=0;case 4:return iswdigit(code)!=0;
    case 5:return iswalnum(code)!=0;case 6:return IsBracket(code);case 7:return IsEmoji(code);
    case 8:return IsEmojiModifier(code);case 9:return IsRegionalIndicator(code);case 10:return IsEmojiTag(code);
    case 11:return wcschr(h->tr->punct_within_word,code)!=NULL;
    case 12:return wcschr(h->tr->char_plus_apostrophe,code)!=NULL;
    case 13:return lookupwchar(breaks,code)!=0;case 14:return isspace2(code);case 15:return IsDigit(code);
    default:return 0;}
}
static unsigned ClauseLower(void *opaque,unsigned code){RustClauseHost *h=opaque;return towlower2(code,h->tr);}
static int ClauseRead(void *opaque,int16_t *positions,char *name,RustClauseRead *out)
{
    RustClauseHost *h=opaque;int top=0,tone=0;
    int term=ReadClause(h->tr,source,positions,&top,N_TR_SOURCE,&tone,clause_voice_name);
    if(top<0||top>=N_TR_SOURCE)return -1;
    const char *end=memchr(clause_voice_name,0,sizeof(clause_voice_name));if(end==NULL)return -1;
    memcpy(name,clause_voice_name,(size_t)(end-clause_voice_name)+1);
    *out=(RustClauseRead){term,tone,(size_t)top};return 0;
}
static int ClauseEmbeddedCommand(void *opaque,size_t *index,int *result)
{
    (void)opaque;if(*index>=sizeof(source))return -1;unsigned ix=(unsigned)*index;
    *result=EmbeddedCommand(&ix);*index=ix;return ix<=sizeof(source)?0:-1;
}
static int ClauseChar(void *opaque,size_t index,unsigned previous,unsigned code,unsigned next,unsigned *insert,unsigned *flags,unsigned *result)
{
    RustClauseHost *h=opaque;if(index>=sizeof(source))return -1;int in=(int)*insert,wf=(int)*flags;
    *result=TranslateChar(h->tr,source+index,(int)previous,code,next,&in,&wf);*insert=(unsigned)in;*flags=(unsigned)wf;return 0;
}
static int ClauseUpper(void *opaque,char *text,size_t index,unsigned code,int *result)
{
    RustClauseHost *h=opaque;if(index<3||index>=N_TR_SOURCE)return -1;
    *result=UpperCaseInWord(h->tr,text+index,(int)code);return 0;
}
static int ClauseEntry(void *opaque,int index,void *out){(void)opaque;if(index<0||index>=N_PHONEME_LIST)return -1;memcpy(out,&ph_list2[index],sizeof(PHONEME_LIST2));return 0;}
static int ClausePut(void *opaque,int index,const void *in){(void)opaque;if(index<0||index>=N_PHONEME_LIST)return -1;memcpy(&ph_list2[index],in,sizeof(PHONEME_LIST2));return 0;}
static int ClauseCommand(void *opaque,int index,unsigned *out){(void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;*out=embedded_list[index];return 0;}
static int ClausePutCommand(void *opaque,int index,unsigned value){(void)opaque;if(index<0||index>=N_EMBEDDED_LIST)return -1;embedded_list[index]=value;return 0;}
static int ClauseTranslate(void *opaque,char *clause,size_t end,char *number,size_t initialized,size_t offset,RustClauseRow *rows,size_t row,size_t remaining,int pause,unsigned *flags)
{
    RustClauseHost *h=opaque;
    if(end>=N_TR_SOURCE-1||row>=N_CLAUSE_WORDS||remaining>N_CLAUSE_WORDS-row)return -1;
    char *base=number!=NULL?number:clause;size_t length=number!=NULL?initialized:end+2;
    if((number!=NULL&&(initialized==0||initialized>150))||offset>=length)return -1;
    char *saved_end=h->tr->clause_end;h->tr->clause_end=clause+end;
    *flags=TranslateWord2WithContext(h->tr,base+offset,(WORD_TAB *)(rows+row+1),(int)remaining,pause,base,length);
    h->tr->clause_end=saved_end;return 0;
}
static int ClauseWordEmbedded(void *opaque){(void)opaque;Word_EmbeddedCmd();return 0;}
static int ClauseEof(void *opaque){(void)opaque;return Eof();}
static int ClauseMakeList(void *opaque,int pause,int sentence){RustClauseHost *h=opaque;MakePhonemeList(h->tr,pause,sentence!=0);return 0;}
static int ClauseOutput(void *opaque,size_t index,unsigned field,unsigned value)
{
    (void)opaque;if(index>N_PHONEME_LIST)return -1;
    switch(field){case 0:if(value!=0)return -1;phoneme_list[index].ph=NULL;break;
    case 1:phoneme_list[index].sourceix=value;break;case 2:phoneme_list[index].synthflags=value;break;default:return -1;}return 0;
}
static int ClauseResult(void *opaque,unsigned kind,int value,const char *name)
{
    RustClauseHost *h=opaque;
    if(kind==0){if(h->terminator!=NULL)*h->terminator=value;}
    else if(kind==1){if(h->tone!=NULL)*h->tone=value;}
    else if(kind==2){if(value!=0){const char *end=memchr(name,0,40);if(end==NULL)return -1;
        memcpy(clause_voice_name,name,(size_t)(end-name)+1);if(h->voice!=NULL)*h->voice=clause_voice_name;}else if(h->voice!=NULL)*h->voice=NULL;}
    else if(kind!=2)return -1;return 0;
}
_Static_assert(sizeof(RustClauseRow)==sizeof(WORD_TAB),"clause row size");
_Static_assert(offsetof(RustClauseRow,start)==offsetof(WORD_TAB,start),"clause row start");
_Static_assert(offsetof(RustClauseRow,pause)==offsetof(WORD_TAB,pre_pause),"clause row pause");
_Static_assert(offsetof(RustClauseRow,source_index)==offsetof(WORD_TAB,sourceix),"clause row source");
_Static_assert(offsetof(RustClauseRow,length)==offsetof(WORD_TAB,length),"clause row length");
void TranslateClauseWithTerminator(Translator *tr,int *tone,char **voice,int *terminator)
{
    if(tr==NULL)return;RustClauseHost h={tr,tone,terminator,voice};
    RustTranslateClause callbacks={&h,ClauseValue,ClauseStore,ClauseByte,ClauseWrite,ClauseClass,ClauseLower,
        ClauseRead,ClauseEmbeddedCommand,ClauseChar,ClauseUpper,ClauseEntry,ClausePut,ClauseCommand,
        ClausePutCommand,ClauseTranslate,ClauseWordEmbedded,ClauseEof,ClauseMakeList,ClauseOutput,ClauseResult};
    (void)espeak_rs_translate_clause(&callbacks);
}
