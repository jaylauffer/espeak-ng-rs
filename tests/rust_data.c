/* Differential checks against the original C dictionary/phoneme routines.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <dirent.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "dictionary.h"
#include "phoneme.h"
#include "speech.h"
#include "synthdata.h"
#include "translate.h"
#include "rust_data.h"

static PHONEME_TAB *reference_phonemes[N_PHONEME_TAB];
static int reference_count;
#define InitGroups reference_InitGroups
#define SetUpPhonemeTable reference_SetUpPhonemeTable
#define phoneme_tab reference_phonemes
#define n_phoneme_tab reference_count
#include "data_reference.inc"
#undef InitGroups
#undef SetUpPhonemeTable
#undef phoneme_tab
#undef n_phoneme_tab

static void check_core_storage(void)
{
    const char *names[]={"phontab","phonindex","phondata","intonations"};
    void *owner=espeak_rs_core_create();TEST_ASSERT(owner!=NULL);
    unsigned char *addresses[4]={0};int32_t lengths[4]={0};size_t total=0;
    for(uint32_t slot=0;slot<4;slot++){
        char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s/%s",path_home,names[slot]);
        int size=GetFileLength(path);TEST_ASSERT(size>0);
        unsigned char *reference=malloc(size);TEST_ASSERT(reference!=NULL);
        FILE *file=fopen(path,"rb");TEST_ASSERT(file!=NULL);TEST_ASSERT(fread(reference,1,size,file)==(size_t)size);fclose(file);
        int32_t error=-1;
        for(unsigned repeat=0;repeat<10;repeat++){
            unsigned char *data=NULL;int32_t length=-1;
            TEST_ASSERT(espeak_rs_core_load(owner,slot,path,&data,&length,&error)==0);
            TEST_ASSERT(data!=NULL&&length==size&&error==0&&(uintptr_t)data%8==0);
            TEST_ASSERT(memcmp(data,reference,size)==0);
            if(repeat==0){addresses[slot]=data;lengths[slot]=length;}else TEST_ASSERT(data==addresses[slot]);
        }
        unsigned char *data=NULL;int32_t length=-1;
        TEST_ASSERT(espeak_rs_core_load(owner,slot,"/missing-espeak-core-fixture",&data,&length,&error)==1);
        TEST_ASSERT(data==addresses[slot]&&length==lengths[slot]&&memcmp(data,reference,size)==0);
        TEST_ASSERT(espeak_rs_core_load(owner,slot,path_home,&data,&length,&error)==4);
        TEST_ASSERT(data==addresses[slot]&&length==lengths[slot]);
        total+=size;free(reference);
    }
    char path[]="/tmp/espeak-rust-core-XXXXXX";int file=mkstemp(path);TEST_ASSERT(file>=0);
    TEST_ASSERT(ftruncate(file,0x8000001)==0);
    unsigned char *data=NULL;int32_t length=-1,error=-1;
    TEST_ASSERT(espeak_rs_core_load(owner,2,path,&data,&length,&error)==5);
    TEST_ASSERT(data==addresses[2]&&length==lengths[2]);
    TEST_ASSERT(ftruncate(file,0)==0);
    TEST_ASSERT(espeak_rs_core_load(owner,2,path,&data,&length,&error)==0);
    TEST_ASSERT(data==NULL&&length==0&&error==0);
    close(file);TEST_ASSERT(unlink(path)==0);
    espeak_rs_core_destroy(owner);espeak_rs_core_destroy(NULL);
    printf("Compared %zu core asset bytes with C across 40 loads, alignment and storage reuse\n",total);
}

static void check_dictionaries(void)
{
	DIR *dir = opendir(path_home);
	TEST_ASSERT(dir != NULL);
	struct dirent *entry;
	int dictionaries = 0;
	Translator *actual = calloc(1, sizeof(*actual));
	Translator *reference = calloc(1, sizeof(*reference));
	TEST_ASSERT(actual && reference);
	while ((entry = readdir(dir)) != NULL) {
		size_t length = strlen(entry->d_name);
		if (length < 5 || strcmp(entry->d_name + length - 5, "_dict") != 0) continue;
		char name[80];
		TEST_ASSERT(length - 5 < sizeof(name));
		memcpy(name, entry->d_name, length - 5);
		name[length - 5] = 0;
		TEST_ASSERT(LoadDictionary(actual, name, 0) == 0);
		memset(reference, 0, sizeof(*reference));
		reference->data_dictrules = actual->data_dictrules;
		reference_InitGroups(reference);
		TEST_ASSERT(actual->n_groups2 == reference->n_groups2);
		TEST_ASSERT(memcmp(actual->groups1, reference->groups1, sizeof(actual->groups1)) == 0);
		TEST_ASSERT(memcmp(actual->groups3, reference->groups3, sizeof(actual->groups3)) == 0);
		TEST_ASSERT(memcmp(actual->letterGroups, reference->letterGroups, sizeof(actual->letterGroups)) == 0);
		TEST_ASSERT(memcmp(actual->groups2_start, reference->groups2_start, sizeof(actual->groups2_start)) == 0);
		TEST_ASSERT(memcmp(actual->groups2_count, reference->groups2_count, sizeof(actual->groups2_count)) == 0);
		TEST_ASSERT(memcmp(actual->groups2, reference->groups2, actual->n_groups2 * sizeof(*actual->groups2)) == 0);
		TEST_ASSERT(memcmp(actual->groups2_name, reference->groups2_name, actual->n_groups2 * sizeof(*actual->groups2_name)) == 0);
		TEST_ASSERT(actual->langopts.replace_chars == reference->langopts.replace_chars);
		char *p = actual->data_dictlist + 8;
		for (int bucket = 0; bucket < N_HASH_DICT; bucket++) {
			TEST_ASSERT(actual->dict_hashtab[bucket] == p);
			while (*(uint8_t *)p != 0) p += *(uint8_t *)p;
			p++;
		}
		TEST_ASSERT(p == actual->data_dictrules);
		dictionaries++;
	}
	closedir(dir);
	/* Failed replacement must keep all indices pointing at the last valid data. */
	char *previous = actual->data_dictlist;
	char previous_name[sizeof(actual->dictionary_name)];
	memcpy(previous_name, actual->dictionary_name, sizeof(previous_name));
	TEST_ASSERT(LoadDictionary(actual, "missing-rust-regression-dictionary", 1) == 1);
	TEST_ASSERT(actual->data_dictlist == previous);
	TEST_ASSERT(memcmp(previous_name, actual->dictionary_name, sizeof(previous_name)) == 0);
	char old_path[sizeof(path_home)], directory[] = "/tmp/espeak-rust-data-XXXXXX", file[256];
	memcpy(old_path, path_home, sizeof(old_path));
	TEST_ASSERT(mkdtemp(directory) != NULL);
	snprintf(file, sizeof(file), "%s/bad_dict", directory);
	FILE *bad = fopen(file, "wb");
	TEST_ASSERT(bad != NULL);
	TEST_ASSERT(fwrite("bad", 1, 3, bad) == 3);
	fclose(bad);
	snprintf(path_home, sizeof(path_home), "%s", directory);
	TEST_ASSERT(LoadDictionary(actual, "bad", 1) == 2);
	TEST_ASSERT(actual->data_dictlist == previous);
	TEST_ASSERT(memcmp(previous_name, actual->dictionary_name, sizeof(previous_name)) == 0);
	memcpy(path_home, old_path, sizeof(path_home));
	TEST_ASSERT(unlink(file) == 0);
	TEST_ASSERT(rmdir(directory) == 0);
	free(actual->data_dictlist);
	free(actual);
	free(reference);
	TEST_ASSERT(dictionaries >= 100);
	printf("Compared native indices with C for %d dictionaries\n", dictionaries);
}

static void check_phonemes(void)
{
	int count = 0;
	/* Reverse order also exercises clearing stale sibling entries. */
	while (count < N_PHONEME_TABS && phoneme_tab_list[count].phoneme_tab_ptr != NULL) count++;
	TEST_ASSERT(count > 100);
	for (int pass = 0; pass < 2; pass++) {
		for (int ix = 0; ix < count; ix++) {
			int number = pass ? count - ix - 1 : ix;
			SelectPhonemeTable(number);
			memset(reference_phonemes, 0, sizeof(reference_phonemes));
			reference_count = 0;
			reference_SetUpPhonemeTable(number);
			TEST_ASSERT(n_phoneme_tab == reference_count + 1);
			TEST_ASSERT(memcmp(phoneme_tab, reference_phonemes, sizeof(reference_phonemes)) == 0);
			int first = 0;
			while (strcmp(phoneme_tab_list[first].name, phoneme_tab_list[number].name) != 0) first++;
			TEST_ASSERT(LookupPhonemeTable(phoneme_tab_list[number].name) == first);
		}
	}
	printf("Compared native inheritance overlays with C for %d phoneme tables\n", count);
	SelectPhonemeTable(N_PHONEME_TABS);
	TEST_ASSERT(n_phoneme_tab == 0);
	for (int ix = 0; ix < N_PHONEME_TAB; ix++) TEST_ASSERT(phoneme_tab[ix] == NULL);
	SelectPhonemeTable(0);
	TEST_ASSERT(n_phoneme_tab > 0);
}

int main(void)
{
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL, 0, NULL, 0) == 22050);
    check_core_storage();
	check_dictionaries();
	check_phonemes();
	TEST_ASSERT(espeak_Terminate() == EE_OK);
	return 0;
}
