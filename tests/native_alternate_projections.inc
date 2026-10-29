/* Frozen test fixture: the C alternate projections translate.c had before its
 * callbacks moved to Rust (rust/translate_engine_compat.rs), copied as the
 * test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native alternate projections. */
static Translator **AlternateSlot(unsigned slot)
{
    if(slot==0)return &translator2;
    if(slot==1)return &translator3;
    return NULL;
}
static char *AlternateCache(unsigned slot)
{
    if(slot==0)return translator2_language;
    if(slot==1)return translator3_language;
    return NULL;
}
static int AlternatePresent(void *opaque,unsigned slot)
{
    (void)opaque;Translator **p=AlternateSlot(slot);return p&&*p!=NULL;
}
static int AlternateCached(void *opaque,unsigned slot,size_t index)
{
    (void)opaque;char *p=AlternateCache(slot);return p&&index<20?(unsigned char)p[index]:-1;
}
static int AlternateWrite(void *opaque,unsigned slot,size_t index,unsigned char byte)
{
    (void)opaque;char *p=AlternateCache(slot);if(!p||index>=20)return -1;p[index]=(char)byte;return 0;
}
static int AlternateLookup(void *opaque,const char *name)
{
    (void)opaque;return SelectPhonemeTableName(name);
}
static int AlternateCreate(void *opaque,unsigned slot,const char *name)
{
    (void)opaque;Translator **p=AlternateSlot(slot);if(!p||*p)return -1;
    *p=SelectTranslator(name);return *p?0:-1;
}
static int AlternateLoad(void *opaque,unsigned slot)
{
    (void)opaque;Translator **p=AlternateSlot(slot);
    return p&&*p?LoadDictionary(*p,(*p)->dictionary_name,0):-1;
}
static int AlternateMainTable(void *opaque)
{
    (void)opaque;return voice?voice->phoneme_tab_ix:-1;
}
static int AlternateEffect(void *opaque,unsigned slot,unsigned effect,int value)
{
    (void)opaque;Translator **p=AlternateSlot(slot);if(!p)return -1;
    switch(effect){
    case 0:DeleteTranslator(*p);return 0;
    case 1:*p=NULL;return 0;
    case 2:if(!*p)return -1;(*p)->phoneme_tab_ix=value;return 0;
    case 3:if(!*p)return -1;(*p)->phonemes_repeat[0]=0;return 0;
    case 4:SelectPhonemeTable(value);return 0;
    default:return -1;
    }
}
static RustAlternateTranslator AlternateCallbacks(void *context)
{
    RustAlternateTranslator h={context,AlternatePresent,AlternateCached,AlternateWrite,
        AlternateLookup,AlternateCreate,AlternateLoad,AlternateMainTable,AlternateEffect};return h;
}
static int AlternateSelect(unsigned slot,const char *name)
{
    if(!name)return -1;
    size_t length=0;while(length<20&&name[length])length++;
    if(length>=20)return -1;
    unsigned context=slot;RustAlternateTranslator h=AlternateCallbacks(&context);int out=-1;
    (void)espeak_rs_alternate_translator(&h,slot,name,length,&out);return out;
}
int SetTranslator2(const char *name){return AlternateSelect(0,name);}
int SetTranslator3(const char *name){return AlternateSelect(1,name);}
void FreeAlternateTranslators(void)
{
    unsigned context=0;RustAlternateTranslator h=AlternateCallbacks(&context);int out;
    (void)espeak_rs_alternate_translator(&h,2,NULL,0,&out);
}
