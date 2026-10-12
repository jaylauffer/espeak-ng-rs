/* Frozen complete native C asset controllers from 20feb182.
 * Copyright (C) 2005-2014 Jonathan Duddington, 2015-2017 Reece H. Dunn.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static espeak_ng_STATUS ReadPhFile(void **ptr,const char *name,int *size,espeak_ng_ERROR_CONTEXT *context)
{
    if(ptr==NULL)return EINVAL;
    char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s%c%s",path_home,PATHSEP,name);
    uint32_t slot;
    if(strcmp(name,"phontab")==0)slot=0;
    else if(strcmp(name,"phonindex")==0)slot=1;
    else if(strcmp(name,"phondata")==0)slot=2;
    else if(strcmp(name,"intonations")==0)slot=3;
    else return EINVAL;
    if(rust_core_storage==NULL)rust_core_storage=espeak_rs_core_create();
    unsigned char *data=NULL;int32_t length=0,error=0;
    uint32_t status=espeak_rs_core_load(rust_core_storage,slot,path,&data,&length,&error);
    *ptr=data;if(size!=NULL)*size=length;
    if(status==0)return ENS_OK;
    if(status==3)return ENOMEM;
    if(error==0){
        switch(status){case 1:error=ENOENT;break;case 2:error=EACCES;break;case 4:error=EISDIR;break;case 5:error=EINVAL;break;default:error=EIO;break;}
    }
    return create_file_error_context(context,error,path);
}

espeak_ng_STATUS LoadPhData(int *srate, espeak_ng_ERROR_CONTEXT *context)
{
	int ix;
	int version;
	int length = 0;
	int rate;
	espeak_ng_STATUS status;
#ifndef USE_RUST_CORE
	unsigned char *p;
#else
	RustTableMeta tables[N_PHONEME_TABS];
	uint32_t header[2];
	espeak_rs_phontab_destroy(rust_phontab_index);
	rust_phontab_index = NULL;
	current_phoneme_table = -1;
	n_phoneme_tables = 0;
	memset(phoneme_tab, 0, sizeof(phoneme_tab));
	memset(phoneme_tab_list, 0, sizeof(phoneme_tab_list));
	if ((status = ReadPhFile((void **)&phoneme_tab_data, "phontab", &rust_phontab_length, context)) != ENS_OK)
		return status;
#endif

#ifndef USE_RUST_CORE
	if ((status = ReadPhFile((void **)&phoneme_tab_data, "phontab", NULL, context)) != ENS_OK)
		return status;
	if ((status = ReadPhFile((void **)&phoneme_index, "phonindex", NULL, context)) != ENS_OK)
		return status;
	if ((status = ReadPhFile((void **)&phondata_ptr, "phondata", NULL, context)) != ENS_OK)
		return status;
#else
	if ((status = ReadPhFile((void **)&phoneme_index, "phonindex", &rust_phonindex_length, context)) != ENS_OK)
		return status;
	if ((status = ReadPhFile((void **)&phondata_ptr, "phondata", &rust_phondata_length, context)) != ENS_OK)
		return status;
#endif
	if ((status = ReadPhFile((void **)&tunes, "intonations", &length, context)) != ENS_OK)
		return status;
	wavefile_data = (unsigned char *)phondata_ptr;
	n_tunes = length / sizeof(TUNE);

	// read the version number and sample rate from the first 8 bytes of phondata
#ifdef USE_RUST_CORE
	if (espeak_rs_phondata_header(wavefile_data, rust_phondata_length, header) != 0)
		return ENS_UNSUPPORTED_PHON_FORMAT;
	version = (int)header[0];
#else
	version = 0; // bytes 0-3, version number
	rate = 0;    // bytes 4-7, sample rate
	if (wavefile_data) {
		for (ix = 0; ix < 4; ix++) {
			version += (wavefile_data[ix] << (ix*8));
			rate += (wavefile_data[ix+4] << (ix*8));
		}
	}
#endif

	if (version != version_phdata)
		return create_version_mismatch_error_context(context, path_home, version, version_phdata);

#ifdef USE_RUST_CORE
	rate = espeak_rs_sample_rate(wavefile_data, rust_phondata_length);
	if (rate < 0) return ENS_UNSUPPORTED_PHON_FORMAT;
	rust_phontab_index = espeak_rs_phontab_create(phoneme_tab_data, rust_phontab_length, tables, &n_phoneme_tables);
	if (rust_phontab_index == NULL) return ENS_UNSUPPORTED_PHON_FORMAT;
	for (ix = 0; ix < n_phoneme_tables; ix++) {
		memcpy(phoneme_tab_list[ix].name, tables[ix].name, N_PHONEME_TAB_NAME);
		phoneme_tab_list[ix].n_phonemes = tables[ix].count;
		phoneme_tab_list[ix].includes = tables[ix].includes;
		phoneme_tab_list[ix].phoneme_tab_ptr = (PHONEME_TAB *)(phoneme_tab_data + tables[ix].records_offset);
	}
#else
	// set up phoneme tables
	p = phoneme_tab_data;
	n_phoneme_tables = p[0];
	p += 4;

	for (ix = 0; ix < n_phoneme_tables; ix++) {
		int n_phonemes = p[0];
		phoneme_tab_list[ix].n_phonemes = p[0];
		phoneme_tab_list[ix].includes = p[1];
		p += 4;
		memcpy(phoneme_tab_list[ix].name, p, N_PHONEME_TAB_NAME);
		p += N_PHONEME_TAB_NAME;
		phoneme_tab_list[ix].phoneme_tab_ptr = (PHONEME_TAB *)p;
		p += (n_phonemes * sizeof(PHONEME_TAB));
	}
#endif

	if (phoneme_tab_number >= n_phoneme_tables)
		phoneme_tab_number = 0;

	if (srate != NULL)
		*srate = rate;
	return ENS_OK;
}

void FreePhData(void)
{
#ifdef USE_RUST_CORE
	espeak_rs_phontab_destroy(rust_phontab_index);
	rust_phontab_index = NULL;
	rust_phontab_length = 0;
	rust_phonindex_length = 0;
	rust_phondata_length = 0;
	n_phoneme_tables = 0;
	n_phoneme_tab = 0;
	memset(phoneme_tab, 0, sizeof(phoneme_tab));
	memset(phoneme_tab_list, 0, sizeof(phoneme_tab_list));
    espeak_rs_core_destroy(rust_core_storage);rust_core_storage=NULL;
    wavefile_data=NULL;n_tunes=0;
#else
	free(phoneme_tab_data);
	free(phoneme_index);
	free(phondata_ptr);
	free(tunes);
#endif
	phoneme_tab_data = NULL;
	phoneme_index = NULL;
	phondata_ptr = NULL;
	tunes = NULL;
	current_phoneme_table = -1;
}

void SelectPhonemeTable(int number)
{
	if (current_phoneme_table == number) return;
	n_phoneme_tab = 0;
	memset(phoneme_tab, 0, sizeof(phoneme_tab)); // clear so a code absent from the new table looks up as NULL, not a stale pointer from the previous table
#ifdef USE_RUST_CORE
	size_t slots[N_PHONEME_TAB];
	if (espeak_rs_phontab_select(rust_phontab_index, phoneme_tab_data, rust_phontab_length, number, slots) != 0) {
		current_phoneme_table = -1;
		return;
	}
	for (int ix = 0; ix < N_PHONEME_TAB; ix++) {
		if (slots[ix] == SIZE_MAX) continue;
		phoneme_tab[ix] = (PHONEME_TAB *)(phoneme_tab_data + slots[ix]);
		n_phoneme_tab = ix;
	}
#else
	SetUpPhonemeTable(number); // recursively for included phoneme tables
#endif
	n_phoneme_tab++;
	current_phoneme_table = number;
}

int LookupPhonemeTable(const char *name)
{
#ifdef USE_RUST_CORE
	int ix = espeak_rs_phontab_lookup(rust_phontab_index, name);
	if (ix >= 0) phoneme_tab_number = ix;
	return ix;
#else
	int ix;

	for (ix = 0; ix < n_phoneme_tables; ix++) {
		if (strcmp(name, phoneme_tab_list[ix].name) == 0) {
			phoneme_tab_number = ix;
			break;
		}
	}
	if (ix == n_phoneme_tables)
		return -1;

	return ix;
#endif
}

int SelectPhonemeTableName(const char *name)
{
	// Look up a phoneme set by name, and select it if it exists
	// Returns the phoneme table number
	int ix;

	if ((ix = LookupPhonemeTable(name)) == -1)
		return -1;

	SelectPhonemeTable(ix);
	return ix;
}

static void SetUpPhonemeTable(int number)
{
	int ix;
	int includes;
	PHONEME_TAB *phtab;

	if ((includes = phoneme_tab_list[number].includes) > 0) {
		// recursively include base phoneme tables
		SetUpPhonemeTable(includes - 1);
	}

	// now add the phonemes from this table
	phtab = phoneme_tab_list[number].phoneme_tab_ptr;
	for (ix = 0; ix < phoneme_tab_list[number].n_phonemes; ix++) {
		int ph_code = phtab[ix].code;
		phoneme_tab[ph_code] = &phtab[ix];
		if (ph_code > n_phoneme_tab) {
			memset(&phoneme_tab[n_phoneme_tab+1], 0, (ph_code - (n_phoneme_tab+1)) * sizeof(*phoneme_tab));
			n_phoneme_tab = ph_code;
		}
	}
}
