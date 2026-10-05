/* Native MBROLA tables/name mapping versus retained C, without backend process.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <dirent.h>
#include <sys/stat.h>
#include <unistd.h>
#include <espeak-ng/espeak_ng.h>
#include "common.h"
#include "mbrola.h"
#include "phoneme.h"
#include "speech.h"
#include "synthesize.h"
#include "rust_data.h"
static MBROLA_TAB *mbrola_tab;
static int mbr_name_prefix;
#include "mbrola_reference.inc"
static unsigned seed=0x2349e15u;
static unsigned next(void){seed=seed*1664525u+1013904223u;return seed;}
static size_t selections,files,rows;
static void compare(void *owner,PHONEME_TAB current,PHONEME_TAB previous,PHONEME_TAB following,PHONEME_TAB pause,RustMbrolaContext context)
{
	PHONEME_LIST list[2]={{0}};list[0].newword=context.word_start;list[1].newword=context.next_word_start;
	list[0].synthflags=context.synth_flags;list[0].stresslevel=context.stress;list[0].wordstress=context.word_stress;
	phoneme_tab[phPAUSE]=&pause;mbr_name_prefix=context.prefix;
	int second,percent,control;int name=GetMbrName(list,&current,&previous,&following,&second,&percent,&control);
	RustMbrolaSelection actual={0};
	TEST_ASSERT(espeak_rs_mbrola_select(owner,&current,&previous,&following,&pause,&context,&actual)==0);
	TEST_ASSERT(actual.name==name && actual.second==second && actual.percent==percent && actual.control==control && actual.prefix==mbr_name_prefix);selections++;
}
static void check_file(void *owner,const char *path)
{
	FILE *file=fopen(path,"rb");TEST_ASSERT(file);TEST_ASSERT(fseek(file,0,SEEK_END)==0);long size=ftell(file);rewind(file);
	TEST_ASSERT(size>=28 && (size-4)%24==0);size_t count=(size-4)/24;
	MBROLA_TAB *expected=calloc(count,sizeof(*expected));TEST_ASSERT(expected);uint32_t control=Read4Bytes(file);
	for(size_t i=0;i<count;i++) {
		expected[i].name=Read4Bytes(file);expected[i].next_phoneme=Read4Bytes(file);expected[i].mbr_name=Read4Bytes(file);
		expected[i].mbr_name2=Read4Bytes(file);expected[i].percent=Read4Bytes(file);expected[i].control=Read4Bytes(file);
	}
	fclose(file);int32_t error=77;uint32_t actual_control=77;TEST_ASSERT(espeak_rs_mbrola_load(owner,path,&actual_control,&error)==0);
	const MBROLA_TAB *actual=NULL;size_t actual_count=0;TEST_ASSERT(espeak_rs_mbrola_view(owner,&actual,&actual_count,&actual_control)==0);
	TEST_ASSERT(actual_control==control && actual_count==count && memcmp(actual,expected,count*24)==0);rows+=count;
	mbrola_tab=expected;
	for(size_t i=0;i<count-1;i++)for(int trial=0;trial<40;trial++) {
		PHONEME_TAB current={.mnemonic=(uint32_t)expected[i].name,.type=phVOWEL};
		PHONEME_TAB previous={.mnemonic=(uint32_t)expected[next()%count].name,.type=next()%10};
		PHONEME_TAB following={.mnemonic=(uint32_t)expected[next()%count].name,.type=next()%10};PHONEME_TAB pause={.mnemonic='_',.type=phPAUSE};
		if(trial%3==0){previous.mnemonic=following.mnemonic=expected[i].next_phoneme;previous.type=following.type=phVOWEL;}
		if(trial%4==0){previous.type=following.type=phPAUSE;}
		RustMbrolaContext context={next()%4,next()%4,next()%16,next()%7,next()%7,(trial%5==0)?'?':0};
		compare(owner,current,previous,following,pause,context);
	}
	/* Fresh reads and both reusable buffers produce identical complete views. */
	for(int reload=0;reload<3;reload++) {TEST_ASSERT(espeak_rs_mbrola_load(owner,path,&actual_control,&error)==0);TEST_ASSERT(espeak_rs_mbrola_view(owner,&actual,&actual_count,&actual_control)==0);TEST_ASSERT(actual_count==count && memcmp(actual,expected,count*24)==0);}
	const MBROLA_TAB *retained=actual;uint32_t retained_control=actual_control;
	TEST_ASSERT(espeak_rs_mbrola_load(owner,"/no-such-espeak-mbrola-table",&actual_control,&error)!=0);
	TEST_ASSERT(espeak_rs_mbrola_view(owner,&actual,&actual_count,&actual_control)==0);TEST_ASSERT(actual==retained && actual_control==retained_control);
	free(expected);mbrola_tab=NULL;files++;
}
static void malformed(void *owner,const char *path)
{
	const MBROLA_TAB *before,*after;size_t count,after_count;uint32_t control,after_control;
	TEST_ASSERT(espeak_rs_mbrola_view(owner,&before,&count,&control)==0);
	unsigned char bad[52]={0};bad[4]='a';bad[28]='b';
	for(int length=0;length<=52;length++) {
		FILE *file=fopen(path,"wb");TEST_ASSERT(file);TEST_ASSERT(fwrite(bad,1,length,file)==(size_t)length);fclose(file);
		int32_t error=0;after_control=77;TEST_ASSERT(espeak_rs_mbrola_load(owner,path,&after_control,&error)!=0);
		TEST_ASSERT(after_control==77);TEST_ASSERT(espeak_rs_mbrola_view(owner,&after,&after_count,&after_control)==0);
		TEST_ASSERT(after==before && after_count==count && after_control==control);
	}
	TEST_ASSERT(unlink(path)==0);
}
int main(void)
{
	_Static_assert(sizeof(MBROLA_TAB)==24,"mapping layout");_Static_assert(sizeof(RustMbrolaContext)==24,"context layout");_Static_assert(sizeof(RustMbrolaSelection)==20,"selection layout");
	char saved[N_PATH_BUF],root[]="/tmp/espeak-mbrola-XXXXXX",directory[N_PATH_BUF];strcpy(saved,path_home);TEST_ASSERT(mkdtemp(root));
	snprintf(directory,sizeof(directory),"%s/mbrola_ph",root);TEST_ASSERT(mkdir(directory,0700)==0);strcpy(path_home,root);
	void *owner=espeak_rs_mbrola_create();TEST_ASSERT(owner);FILE *log=tmpfile();TEST_ASSERT(log);
	DIR *source=opendir(REPO_SOURCE_DIR "/phsource/mbrola");TEST_ASSERT(source);struct dirent *entry;
	while((entry=readdir(source))!=NULL) {
		if(entry->d_name[0]=='.')continue;
		char source_path[N_PATH_BUF],compiled[N_PATH_BUF];snprintf(source_path,sizeof(source_path),"%s/phsource/mbrola/%s",REPO_SOURCE_DIR,entry->d_name);
		struct stat metadata;TEST_ASSERT(stat(source_path,&metadata)==0);if(!S_ISREG(metadata.st_mode))continue;
		TEST_ASSERT(espeak_ng_CompileMbrolaVoice(source_path,log,NULL)==ENS_OK);
		snprintf(compiled,sizeof(compiled),"%s/mbrola_ph/%s_phtrans",root,entry->d_name);check_file(owner,compiled);TEST_ASSERT(unlink(compiled)==0);
	}
	closedir(source);fclose(log);TEST_ASSERT(files>40);
	char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s/bad-table",root);malformed(owner,path);
	espeak_rs_mbrola_destroy(owner);owner=espeak_rs_mbrola_create();TEST_ASSERT(owner);espeak_rs_mbrola_destroy(owner);
	strcpy(path_home,saved);phoneme_tab[phPAUSE]=NULL;TEST_ASSERT(rmdir(directory)==0);TEST_ASSERT(rmdir(root)==0);
	printf("Matched %zu compiled MBROLA tables, %zu records and %zu contextual name selections\n",files,rows,selections);
	return 0;
}
