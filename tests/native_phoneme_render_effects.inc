/* Frozen test fixture: the C phoneme render effects dictionary.c had before its
 * callbacks moved to Rust (rust/phoneme_render_compat.rs), copied as the test generator
 * extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native phoneme render effects. */
static int32_t PhonemeRenderIpa(const PHONEME_LIST *list,uint32_t code,unsigned char (*out)[18])
{
	PHONEME_DATA data = {0};
	if (list == NULL) InterpretPhoneme2(code,&data);
	else InterpretPhonemeWithLength(NULL,0,(PHONEME_LIST *)list,phoneme_list,&data,NULL,n_phoneme_list);
	memcpy(*out,data.ipa_string,sizeof(*out));return 0;
}
static int32_t PhonemeRenderAlpha(uint32_t code) { return iswalpha(code)!=0; }
static RustPhonemeRender PhonemeRenderEnvironment(void)
{
	RustPhonemeRender env = {phoneme_list,&n_phoneme_list,(const PHONEME_TAB *const *)phoneme_tab,
		phoneme_tab_list,PhonemeRenderIpa,PhonemeRenderAlpha};return env;
}
char *WritePhMnemonicBounded(char *out,size_t capacity,PHONEME_TAB *ph,PHONEME_LIST *list,int ipa,int *flags)
{
	RustPhonemeRender env=PhonemeRenderEnvironment();
	return espeak_rs_write_phoneme(&env,(unsigned char *)out,capacity,ph,list,ipa,0,flags);
}
char *WritePhMnemonicWithStressBounded(char *out,size_t capacity,PHONEME_TAB *ph,PHONEME_LIST *list,int ipa,int *flags)
{
	RustPhonemeRender env=PhonemeRenderEnvironment();
	return espeak_rs_write_phoneme(&env,(unsigned char *)out,capacity,ph,list,ipa,1,flags);
}
char *WritePhMnemonic(char *out,PHONEME_TAB *ph,PHONEME_LIST *list,int ipa,int *flags)
{
	return WritePhMnemonicBounded(out,PTRDIFF_MAX,ph,list,ipa,flags);
}
char *WritePhMnemonicWithStress(char *out,PHONEME_TAB *ph,PHONEME_LIST *list,int ipa,int *flags)
{
	return WritePhMnemonicWithStressBounded(out,PTRDIFF_MAX,ph,list,ipa,flags);
}
const char *GetTranslatedPhonemeString(int mode)
{
	RustPhonemeRender env=PhonemeRenderEnvironment();return espeak_rs_translated_phonemes(&env,mode);
}
