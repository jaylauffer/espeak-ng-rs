/* Frozen complete 2b4901a5 public C voice controllers, with its existing
 * native Rust variant extractor. Independent full-C variant tests remain.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static char *ExtractVoiceVariantName(char *name,int number,int directory)
{
    static unsigned char suffix[40];
    size_t base_length;
    if(espeak_rs_voice_variant(name,number,directory,PATHSEP,&base_length,&suffix)!=0){suffix[0]=0;return (char*)suffix;}
    if(name!=NULL)name[base_length]=0;
    return (char*)suffix;
}

voice_t *LoadVoiceVariant(const char *vname, int variant_num)
{
	// Load a voice file.
	// Also apply a voice variant if specified by "variant", or by "+number" or "+name" in the "vname"

	voice_t *v;
	char *variant_name;
	char buf[60];

	strncpy0(buf, vname, sizeof(buf));
	variant_name = ExtractVoiceVariantName(buf, variant_num, 1);

	if ((v = LoadVoice(buf, 0)) == NULL)
		return NULL;

	if (variant_name[0] != 0)
		v = LoadVoice(variant_name, 2);
	return v;
}

ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SetVoiceByFile(const char *filename)
{
	int ix;
	espeak_VOICE voice_selector;
	char *variant_name;
	char buf[60];

	strncpy0(buf, filename, sizeof(buf));

	variant_name = ExtractVoiceVariantName(buf, 0, 1);

	for (ix = 0;; ix++) {
		// convert voice name to lower case  (ascii)
		if ((buf[ix] = tolower(buf[ix])) == 0)
			break;
	}

	memset(&voice_selector, 0, sizeof(voice_selector));
	voice_selector.name = (char *)filename; // include variant name in voice stack ??

	// first check for a voice with this filename
	// This may avoid the need to call espeak_ListVoices().

	if (LoadVoice(buf, 0x10) != NULL) {
		if (variant_name[0] != 0)
			LoadVoice(variant_name, 2);

		DoVoiceChange(voice);
		voice_selector.languages = voice->language_name;
		SetVoiceStack(&voice_selector, variant_name);
		return ENS_OK;
	}

	return ENS_VOICE_NOT_FOUND;
}

ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SetVoiceByName(const char *name)
{
	espeak_VOICE *v;
	int ix;
	espeak_VOICE voice_selector;
	char *variant_name;
	char buf[60];

	strncpy0(buf, name, sizeof(buf));

	variant_name = ExtractVoiceVariantName(buf, 0, 1);

	for (ix = 0;; ix++) {
		// convert voice name to lower case  (ascii)
		if ((buf[ix] = tolower(buf[ix])) == 0)
			break;
	}

	memset(&voice_selector, 0, sizeof(voice_selector));
	voice_selector.name = (char *)name; // include variant name in voice stack ??

	// first check for a voice with this filename
	// This may avoid the need to call espeak_ListVoices().

	if (LoadVoice(buf, 1) != NULL) {
		if (variant_name[0] != 0)
			LoadVoice(variant_name, 2);

		DoVoiceChange(voice);
		voice_selector.languages = voice->language_name;
		SetVoiceStack(&voice_selector, variant_name);
		return ENS_OK;
	}

	if (n_voices_list == 0)
		espeak_ListVoices(NULL); // create the voices list

	if ((v = SelectVoiceByName(voices_list, buf)) != NULL) {
		if (LoadVoice(v->identifier, 0) != NULL) {
			if (variant_name[0] != 0)
				LoadVoice(variant_name, 2);
			DoVoiceChange(voice);
			voice_selector.languages = voice->language_name;
			SetVoiceStack(&voice_selector, variant_name);
			return ENS_OK;
		}
	}
	return ENS_VOICE_NOT_FOUND;
}

ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SetVoiceByProperties(espeak_VOICE *voice_selector)
{
	const char *voice_id;
	int voice_found;

	voice_id = SelectVoice(voice_selector, &voice_found);
	if (voice_found == 0)
		return ENS_VOICE_NOT_FOUND;

	LoadVoiceVariant(voice_id, 0);
	DoVoiceChange(voice);
	SetVoiceStack(voice_selector, "");

	return ENS_OK;
}

/* End frozen public voice control. */
