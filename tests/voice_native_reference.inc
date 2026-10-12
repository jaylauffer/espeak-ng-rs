/* Complete frozen 5edcdd95 native host and whole LoadVoice controller.
 * Its inactive C branch remains intact; USE_RUST_CORE selects native policy.
 * SPDX-License-Identifier: GPL-3.0-or-later */
typedef struct {char *name,*languages;} RustVoiceLoadContext;
static int32_t RustActiveVoiceHost(void *opaque,uint32_t kind,const RustVoiceSetup *setup,const char *key,const char *value,const RustVoiceAction *action,voice_t *snapshot,int32_t *fast)
{
    RustVoiceLoadContext *context=opaque;
    if(kind==2){fprintf(stderr,"Invalid voice attribute: %s\n",key);return 1;}
    if(kind==3){if(translator==NULL)translator=SelectTranslator((const char*)setup->translator);return 1;}
    if(kind==4)return SelectPhonemeTableName(key);
    if(kind==5){fprintf(stderr,"Unknown phoneme table: '%s'\n",key);return 1;}
    if(kind==6){voice->phoneme_tab_ix=action->argument;translator->phoneme_tab_ix=action->argument;return 1;}
    if(kind==7){LoadDictionary(translator,key,action->argument?4:0);if(dictionary_name[0]==0){DeleteTranslator(translator);return 0;}return 1;}
    if(kind!=1)return 0;
    *voice=*snapshot;speed.fast_settings=*fast;
    switch(action->action){
    case 1:LoadLanguageOptions(translator,action->argument,(char*)value);break;
    case 2:if(action->argument)SetSpeed(3);break;
    case 3:
    case 4:
        memcpy(context->name,setup->name,sizeof(setup->name));memcpy(context->languages,setup->languages,sizeof(setup->languages));
        current_voice_selected.gender=setup->gender;current_voice_selected.age=setup->age;
        if(action->argument==1){
            SelectPhonemeTableName((const char*)setup->phonemes);translator=SelectTranslator((const char*)setup->translator);
            memcpy(voice->language_name,setup->language,sizeof(voice->language_name));
        }
        if(action->action==4){
            if(action->argument==2)SelectPhonemeTableName((const char*)setup->phonemes);
            PhonemeReplacement((char*)value);
        }
        break;
    case 5:
#if USE_MBROLA
    {
        int rate=action->backend.sample_rate;
        espeak_ng_STATUS status=LoadMbrolaTable((char*)action->backend.voice,(char*)action->backend.table,&rate);
        if(status!=ENS_OK){espeak_ng_PrintStatusCodeMessage(status,stderr,NULL);return 0;}
        voice->samplerate=rate;
    }
#endif
        break;
    case 6:fprintf(stderr,"espeak-ng was built without mbrola support\n");break;
    case 7:fprintf(stderr,"espeak-ng was built without klatt support\n");break;
    default:fprintf(stderr,"Bad voice attribute: %s\n",key);break;
    }
    *snapshot=*voice;*fast=speed.fast_settings;return 1;
}

voice_t *LoadVoice(const char *vname, int control)
{
	// control, bit 0  1= no_default
	//          bit 1  1 = change tone only, not language
	//          bit 2  1 = don't report error on LoadDictionary
	//          bit 4  1 = vname = full path
        //          bit 8  1 = INTERNAL: compiling phonemes; do not try to
        //                     load the phoneme table
        //          bit 16 1 = UNDOCUMENTED

#ifdef USE_RUST_CORE
    void *f_voice = NULL;
#define CloseVoiceFile espeak_rs_voice_file_close
#else
	FILE *f_voice = NULL;
#define CloseVoiceFile fclose
#endif
#ifndef USE_RUST_CORE
	char *p;
	int key;
#endif
#ifndef USE_RUST_CORE
	int ix;
	int value;
	int langix = 0;
#endif
	int tone_only = control & 2;
#ifndef USE_RUST_CORE
	bool language_set = false;
#endif
#ifndef USE_RUST_CORE
	bool phonemes_set = false;
#endif

	char voicename[40];
#ifndef USE_RUST_CORE
	char language_name[40];
#endif
#ifndef USE_RUST_CORE
	char translator_name[40];
	char new_dictionary[40];
	char phonemes_name[40] = "";
#endif
	const char *language_type;
	char buf[N_PATH_BUF];
#if USE_MBROLA && !defined(USE_RUST_CORE)
	char name1[40];
	char name2[80];
#endif

#ifndef USE_RUST_CORE
	int pitch1;
	int pitch2;
#endif

#ifdef USE_RUST_CORE
    char *voice_identifier,*voice_name,*voice_languages;
#else
	static char voice_identifier[40]; // file name for  current_voice_selected
	static char voice_name[40];       // voice name for current_voice_selected
	static char voice_languages[100]; // list of languages and priorities for current_voice_selected

	if (!tone_only) {
		MAKE_MEM_UNDEFINED(&voice_identifier, sizeof(voice_identifier));
		MAKE_MEM_UNDEFINED(&voice_name, sizeof(voice_name));
		MAKE_MEM_UNDEFINED(&voice_languages, sizeof(voice_languages));
	}
#endif

#ifndef USE_RUST_CORE
	if ((vname == NULL || vname[0] == 0) && !(control & 8)) {
		return NULL;
	}

	strncpy0(voicename, vname, sizeof(voicename));
	if (control & 0x10) {
		strcpy(buf, vname);
		if (GetFileLength(buf) <= 0)
			return NULL;
	} else {
		if (voicename[0] == 0 && !(control & 8)/*compiling phonemes*/)
			strcpy(voicename, ESPEAKNG_DEFAULT_VOICE);

		char path_voices[N_PATH_BUF];
		snprintf(path_voices, sizeof(path_voices), "%s%cvoices%c", path_home, PATHSEP, PATHSEP);
		snprintf(buf, sizeof(buf), "%s%s", path_voices, voicename); // look in the main voices directory

		if (GetFileLength(buf) <= 0) {
			snprintf(path_voices, sizeof(path_voices), "%s%clang%c", path_home, PATHSEP, PATHSEP);
			snprintf(buf, sizeof(buf), "%s%s", path_voices, voicename); // look in the main languages directory
		}
	}
/* End legacy voice request paths. Kept as a differential oracle. */
#else
    RustVoiceRequest request;
    if(espeak_rs_voice_request(path_home,vname,control,PATHSEP,sizeof(buf),NULL,RustVoiceLength,&request)!=0)return NULL;
    memcpy(voicename,request.name,sizeof(voicename));memcpy(buf,request.path,sizeof(buf));
#endif

#ifdef USE_RUST_CORE
    f_voice=espeak_rs_voice_file_open(buf,sizeof(buf));
#else
	f_voice = fopen(buf, "r");
#endif

#ifndef USE_RUST_CORE
        if (!(control & 8)/*compiling phonemes*/)
            language_type = ESPEAKNG_DEFAULT_VOICE; // default
        else
            language_type = "";

	if (f_voice == NULL) {
		if (control & 3)
			return NULL; // can't open file

		if (SelectPhonemeTableName(voicename) >= 0)
			language_type = voicename;
	}
/* End legacy voice fallback. Kept as a differential oracle. */
#else
    unsigned char fallback[40];
    if(espeak_rs_voice_fallback(&request,f_voice!=NULL,0,ESPEAKNG_DEFAULT_VOICE,&fallback)!=0)return NULL;
    if(f_voice==NULL&&SelectPhonemeTableName(voicename)>=0)
        espeak_rs_voice_fallback(&request,0,1,ESPEAKNG_DEFAULT_VOICE,&fallback);
    language_type=(const char*)fallback;
    if(rust_current_voice==NULL)rust_current_voice=espeak_rs_current_voice_create();
    RustVoiceSetup setup;
    if(espeak_rs_current_voice_prepare(rust_current_voice,vname,language_type,tone_only,current_voice_selected.gender,current_voice_selected.age,(const unsigned char(*)[20])&voice->language_name,&setup)!=0){if(f_voice)CloseVoiceFile(f_voice);return NULL;}
    voice_identifier=(char*)rust_current_voice->identifier;voice_name=(char*)rust_current_voice->name;voice_languages=(char*)rust_current_voice->languages;
#endif

	if (!tone_only && (translator != NULL)) {
		DeleteTranslator(translator);
		translator = NULL;
	}

#ifndef USE_RUST_CORE
	strcpy(translator_name, language_type);
	strcpy(new_dictionary, language_type);
#endif

	if (!tone_only) {
		voice = &voicedata;
#ifndef USE_RUST_CORE
		strncpy0(voice_identifier, vname, sizeof(voice_identifier));
		voice_name[0] = 0;
		voice_languages[0] = 0;
#endif

		current_voice_selected.identifier = voice_identifier;
		current_voice_selected.name = voice_name;
		current_voice_selected.languages = voice_languages;
	} else {
#ifndef USE_RUST_CORE
		// append the variant file name to the voice identifier
		if ((p = strchr(voice_identifier, '+')) != NULL)
			*p = 0;    // remove previous variant name
		else
			p = voice_identifier + strlen(voice_identifier);
		snprintf(p, sizeof(voice_identifier) - (p - voice_identifier), "+%s", &vname[3]);    // omit  !v/  from the variant filename
/* End legacy current voice variant identifier. Kept as a differential oracle. */
#endif
	}
	VoiceReset(tone_only);

#ifdef USE_RUST_CORE
    voice_t snapshot=*voice;int32_t fast=speed.fast_settings;
    RustVoiceLoadContext context={voice_name,voice_languages};
    int status=espeak_rs_voice_configure(f_voice,&setup,&snapshot,&fast,USE_KLATT|(USE_MBROLA<<1),control,&context,RustActiveVoiceHost);
    if(f_voice!=NULL)CloseVoiceFile(f_voice);
#undef CloseVoiceFile
    if(status!=0)return NULL;
    memcpy(voice_languages,setup.languages,sizeof(setup.languages));
    return voice;
#else
	while ((f_voice != NULL) && (fgets_strip(buf, sizeof(buf), f_voice) != NULL)) {
		// isolate the attribute name
		for (p = buf; (*p != 0) && !isspace(*p); p++) ;
		*p++ = 0;

		if (buf[0] == 0) continue;
		key = LookupMnem(langopts_tab, buf);

        if (key != 0) {
            LoadLanguageOptions(translator, key, p);
        } else {
            key = LookupMnem(keyword_tab, buf);
#ifdef USE_RUST_CORE
            uint32_t update_speed = 0;
            int acoustic = espeak_rs_voice_attribute(voice, buf, p, USE_KLATT, &speed.fast_settings, &update_speed);
            if (acoustic != 1) {
                if (acoustic == 2) fprintf(stderr, "Invalid voice attribute: %s\n", buf);
                else if (update_speed) SetSpeed(3);
                continue;
            }
            uint32_t effect=0;
            int metadata=espeak_rs_voice_setup_attribute(&setup,buf,p,&effect);
            if(metadata!=1){
                if(metadata==2){fprintf(stderr,"Invalid voice setup attribute: %s\n",buf);continue;}
                memcpy(translator_name,setup.translator,sizeof(translator_name));memcpy(new_dictionary,setup.dictionary,sizeof(new_dictionary));
                memcpy(phonemes_name,setup.phonemes,sizeof(phonemes_name));memcpy(voice_name,setup.name,sizeof(voice_name));
                memcpy(voice_languages,setup.languages,sizeof(voice_languages));langix=setup.language_length;
                current_voice_selected.gender=setup.gender;current_voice_selected.age=setup.age;
                if(effect==1){
                    SelectPhonemeTableName(phonemes_name);translator=SelectTranslator(translator_name);
                    memcpy(voice->language_name,setup.language,sizeof(voice->language_name));
                }
                if(strcmp(buf,"replace")==0){
                    if(effect==2)SelectPhonemeTableName(phonemes_name);
                    PhonemeReplacement(p);
                }
                continue;
            }
#endif
            switch (key)
            {
#ifndef USE_RUST_CORE
            case V_LANGUAGE:
            {
                unsigned int len;
                int priority;

                if (tone_only)
                    break;

                priority = DEFAULT_LANGUAGE_PRIORITY;
                language_name[0] = 0;

                sscanf(p, "%s %d", language_name, &priority);
                if (strcmp(language_name, "variant") == 0)
                    break;

                len = strlen(language_name) + 2;
                // check for space in languages[]
                if (len < (sizeof(voice_languages)-langix-1)) {
                    voice_languages[langix] = priority;

                    strcpy(&voice_languages[langix+1], language_name);
                    langix += len;
                }

                // only act on the first language line
                if (language_set == false) {
                    language_type = strtok(language_name, "-");
                    language_set = true;
                    strcpy(translator_name, language_type);
                    strcpy(new_dictionary, language_type);
                    strcpy(phonemes_name, language_type);
                    SelectPhonemeTableName(phonemes_name);

                    translator = SelectTranslator(translator_name);
                    strncpy0(voice->language_name, language_name, sizeof(voice->language_name));
                }
            }
                break;
            case V_NAME:
                if (tone_only == 0) {
                    while (isspace(*p)) p++;
                    strncpy0(voice_name, p, sizeof(voice_name));
                }
                break;
            case V_GENDER:
            {
                int age = 0;
                char vgender[80];
                sscanf(p, "%s %d", vgender, &age);
                current_voice_selected.gender = LookupMnem(genders, vgender);
                current_voice_selected.age = age;
            }
                break;
            case V_DICTIONARY: // dictionary
                sscanf(p, "%s", new_dictionary);
                break;
            case V_PHONEMES: // phoneme table
                sscanf(p, "%s", phonemes_name);
                break;
/* End legacy active voice metadata. Kept as a differential oracle. */
#endif
#ifndef USE_RUST_CORE
            case V_FORMANT:
                VoiceFormant(p);
                break;
            case V_PITCH:
                // default is  pitch 82 118
                if (sscanf(p, "%d %d", &pitch1, &pitch2) == 2) {
                    voice->pitch_base = (pitch1 - 9) << 12;
                    voice->pitch_range = (pitch2 - pitch1) * 108;
                    double factor = (double)(pitch1 - 82)/82;
                    voice->formant_factor = (int)((1+factor/4) * 256); // nominal formant shift for a different voice pitch
                }
                break;





/* End legacy voice pitch attributes. Kept as a differential oracle. */
#endif
            case V_REPLACE:
#ifndef USE_RUST_CORE
                if (phonemes_set == false) {
                    // must set up a phoneme table before we can lookup phoneme mnemonics
                    SelectPhonemeTableName(phonemes_name);
                    phonemes_set = true;
                }
                PhonemeReplacement(p);
#endif
                break;

#ifndef USE_RUST_CORE
            case V_ECHO:
                // echo.  suggest: 135mS  11%
                value = 0;
                voice->echo_amp = 0;
                sscanf(p, "%d %d", &voice->echo_delay, &voice->echo_amp);
                break;
            case V_FLUTTER: // flutter
                if (sscanf(p, "%d", &value) == 1)
                    voice->flutter = value * 32;
                break;
            case V_ROUGHNESS: // roughness
                if (sscanf(p, "%d", &value) == 1)
                    voice->roughness = value;
                break;
            case V_CLARITY: // formantshape
                if (sscanf(p, "%d", &value) == 1) {
                    if (value > 4) {
                        voice->peak_shape = 1; // squarer formant peaks
                        value = 4;
                    }
                    voice->n_harmonic_peaks = 1+value;
                }
                break;
            case V_TONE:
            {
                int tone_data[12];
                ReadTonePoints(p, tone_data);
                SetToneAdjust(voice, tone_data);
            }
                break;
            case V_VOICING:
                if (sscanf(p, "%d", &value) == 1)
                    voice->voicing = (value * 64)/100;
                break;
            case V_BREATH:
                voice->breath[0] = Read8Numbers(p, &voice->breath[1]);
                for (ix = 1; ix < 8; ix++) {
                    if (ix % 2)
                        voice->breath[ix] = -voice->breath[ix];
                }
                break;
            case V_BREATHW:
                voice->breathw[0] = Read8Numbers(p, &voice->breathw[1]);
                break;
            case V_CONSONANTS:
                value = sscanf(p, "%d %d", &voice->consonant_amp, &voice->consonant_ampv);
                break;
            case V_SPEED:
                sscanf(p, "%d", &voice->speed_percent);
                SetSpeed(3);
                break;
/* End legacy voice acoustic attributes. Kept as a differential oracle. */
#endif
#if USE_MBROLA
            case V_MBROLA:
            {
#ifndef USE_RUST_CORE
                int srate = 16000;

                name2[0] = 0;
                sscanf(p, "%s %s %d", name1, name2, &srate);
/* End legacy MBROLA directive parsing. Kept as a differential oracle. */
#else
                RustMbrolaRequest request;
                if(espeak_rs_mbrola_request(p,&request)!=0){fprintf(stderr,"Invalid MBROLA directive\n");continue;}
                int srate=request.sample_rate;
                const char *name1=(const char*)request.voice,*name2=(const char*)request.table;
#endif
                espeak_ng_STATUS status = LoadMbrolaTable(name1, name2, &srate);
                if (status != ENS_OK) {
                    espeak_ng_PrintStatusCodeMessage(status, stderr, NULL);
                    fclose(f_voice);
                    return NULL;
                }
                else
                    voice->samplerate = srate;
            }
                break;
#else
            case V_MBROLA:
                fprintf(stderr, "espeak-ng was built without mbrola support\n");
                break;
#endif
#if USE_KLATT
            case V_KLATT:
#ifndef USE_RUST_CORE
                voice->klattv[0] = 1; // default source: IMPULSIVE
                Read8Numbers(p, voice->klattv);
                voice->klattv[KLATT_Kopen] -= 40;
#endif
                break;
#else
            case V_KLATT:
                fprintf(stderr, "espeak-ng was built without klatt support\n");
                break;
#endif
#ifndef USE_RUST_CORE
            case V_FAST:
                sscanf(p, "%d", &speed.fast_settings);
                SetSpeed(3);
                break;
/* End legacy voice fast attribute. Kept as a differential oracle. */
#endif

            case V_MAINTAINER:
            case V_STATUS:
                break;
            default:
                fprintf(stderr, "Bad voice attribute: %s\n", buf);
                break;
            }
        }
	}
	if (f_voice != NULL)
		CloseVoiceFile(f_voice);
#undef CloseVoiceFile

	if ((translator == NULL) && (!tone_only)) {
		// not set by language attribute
		translator = SelectTranslator(translator_name);
	}

	if (!tone_only) {
		if (!!(control & 8/*compiling phonemes*/)) {
			/* Set by espeak_ng_CompilePhonemeDataPath when it
				* calls LoadVoice("", 8) to set up a dummy(?) voice.
				* As phontab may not yet exist this avoids the spurious
				* error message and guarantees consistent results by
				* not actually reading a potentially bogus phontab...
				*/
			ix = 0;
		} else if ((ix = SelectPhonemeTableName(phonemes_name)) < 0) {
			fprintf(stderr, "Unknown phoneme table: '%s'\n", phonemes_name);
			ix = 0;
		}

		voice->phoneme_tab_ix = ix;
		translator->phoneme_tab_ix = ix;

		if (!(control & 8/*compiling phonemes*/)) {
			LoadDictionary(translator, new_dictionary, control & 4);
			if (dictionary_name[0] == 0) {
				DeleteTranslator(translator);
				return NULL; // no dictionary loaded
			}
		}

		/* Terminate languages list with a zero-priority entry */
		voice_languages[langix] = 0;
	}

	return voice;
#endif
}
/* End frozen native voice admission and host. */
