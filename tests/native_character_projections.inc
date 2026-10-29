/* Frozen test fixture: the C character projections translate.c had before its
 * callbacks moved to Rust (rust/translate_engine_compat.rs), copied as the
 * test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native character projections. */
typedef struct { Translator *tr; } RustCharacterHost;
static int character_ignore;
static int CharacterLength(RustCharacterHost *h)
{
    if(h->tr->langopts.replace_chars==NULL)return 0;
    uintptr_t base=(uintptr_t)h->tr->data_dictlist,begin=(uintptr_t)h->tr->langopts.replace_chars;
    if(base==0||begin<base||begin-base>=h->tr->data_dict_size||h->tr->data_dict_size>134217728)return -1;
    return (int)(h->tr->data_dict_size-(begin-base));
}
static int CharacterValue(void *opaque,unsigned field)
{
    RustCharacterHost *h=opaque;switch(field){case 0:return h->tr->translator_name;
    case 1:return character_ignore;case 2:return CharacterLength(h);default:return -1;}
}
static int CharacterIgnore(void *opaque,int count){(void)opaque;if(count<0)return -1;character_ignore=count;return 0;}
static int CharacterByte(void *opaque,unsigned kind,size_t index)
{
    RustCharacterHost *h=opaque;if(kind==0)return index<sizeof(source)?(unsigned char)source[index]:-1;
    int length=CharacterLength(h);if(kind!=1||length<0||index>=(size_t)length)return -1;
    return h->tr->langopts.replace_chars[index];
}
static int CharacterWrite(void *opaque,size_t index,unsigned char value){(void)opaque;if(index>=sizeof(source))return -1;source[index]=(char)value;return 0;}
static int CharacterClass(void *opaque,unsigned code,unsigned kind)
{
    RustCharacterHost *h=opaque;switch(kind){case 0:return iswupper(code)!=0;case 1:return iswalpha(code)!=0;
    case 2:return IsSpace(code);case 3:return IsVowel(h->tr,code);default:return 0;}
}
static unsigned CharacterLower(void *opaque,unsigned code){RustCharacterHost *h=opaque;return towlower2(code,h->tr);}
static int CharacterTrace(void *opaque,size_t from,size_t to)
{
    RustCharacterHost *h=opaque;int length=CharacterLength(h);
    if(length<0||from>=(size_t)length||to>=(size_t)length)return -1;
    const char *base=(const char *)h->tr->langopts.replace_chars;
    if(memchr(base+from,0,(size_t)length-from)==NULL||memchr(base+to,0,(size_t)length-to)==NULL)return -1;
    if(option_phonemes&espeakPHONEMES_TRACE){if(f_trans==NULL)return -1;fprintf(f_trans,"Replace: %s > %s\n",base+from,base+to);}return 0;
}
static RustCharacterConversion CharacterCallbacks(RustCharacterHost *h)
{
    return (RustCharacterConversion){h,CharacterValue,CharacterIgnore,CharacterByte,CharacterWrite,CharacterClass,CharacterLower,CharacterTrace};
}
static int TranslateChar(Translator *tr,char *ptr,int previous,unsigned code,unsigned next,int *insert,int *flags)
{
    uintptr_t begin=(uintptr_t)source,at=(uintptr_t)ptr;
    if(tr==NULL||insert==NULL||flags==NULL||at<begin||at-begin>=sizeof(source))return (int)code;
    RustCharacterHost h={tr};RustCharacterConversion callbacks=CharacterCallbacks(&h);
    RustCharacterCall call={at-begin,(unsigned)previous,code,next,(unsigned)*insert,(unsigned)*flags};RustCharacterResult result;
    if(espeak_rs_character_translate(&callbacks,&call,&result)!=0)return (int)code;
    *insert=(int)result.insert;*flags=(int)result.flags;return (int)result.code;
}
static int UpperCaseInWord(Translator *tr,char *word,int code)
{
    if(tr==NULL||word==NULL)return 0;unsigned char before[3];memcpy(before,word-3,3);
    RustCharacterHost h={tr};RustCharacterConversion callbacks=CharacterCallbacks(&h);int result=0;
    (void)espeak_rs_character_upper(&callbacks,before,(unsigned)code,&result);return result;
}
