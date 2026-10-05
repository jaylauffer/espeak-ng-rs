/* Configuration and WAV ownership compared with retained C, plus bounds.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include "soundicon.h"
#include "synthesize.h"
#include "speech.h"
#include "voice.h"
#include "common.h"
#include "error.h"
#include "langopts.h"
#include "rust_data.h"
static SOUND_ICON reference_icons[80];
static int reference_count, reference_points[12];
static void ReferenceTone(char *text,int *points)
{
	for (int index=0;index<12;index++) points[index]=-1;
	sscanf(text,"%d %d %d %d %d %d %d %d %d %d",
	       points,points+1,points+2,points+3,points+4,points+5,points+6,points+7,points+8,points+9);
}
#define soundicon_tab reference_icons
#define n_soundicon_tab reference_count
#define tone_points reference_points
#define ReadTonePoints ReferenceTone
#define LoadConfig ReferenceConfig
#include "config_reference.inc"
#undef LoadConfig
#define LoadSoundFile ReferenceLoad
#define LookupSoundicon ReferenceLookup
#define LoadSoundFile2 ReferenceFile
#include "sound_reference.inc"
#undef LoadSoundFile
#undef LookupSoundicon
#undef LoadSoundFile2
#undef soundicon_tab
#undef n_soundicon_tab
#undef tone_points
#undef ReadTonePoints
static void reset_reference(void)
{
	for (int index=0;index<reference_count;index++) {
		free(reference_icons[index].data);free(reference_icons[index].filename);
	}
	memset(reference_icons,0,sizeof(reference_icons));reference_count=0;
}
static void compare(SOUND_ICON icons[80],int count)
{
	TEST_ASSERT(count==reference_count);
	for (int index=0;index<count;index++) {
		TEST_ASSERT(icons[index].name==reference_icons[index].name);
		TEST_ASSERT(icons[index].length==reference_icons[index].length);
		TEST_ASSERT(strcmp(icons[index].filename,reference_icons[index].filename)==0);
		if (icons[index].length>0) {
			TEST_ASSERT(((uintptr_t)icons[index].data+44)%2==0);
			TEST_ASSERT(memcmp(icons[index].data,reference_icons[index].data,44+icons[index].length*2)==0);
		}
	}
}
static void word(unsigned char *bytes,unsigned value)
{
	for(int index=0;index<4;index++)bytes[index]=(value>>(8*index))&255;
}
static void wave(const char *path,int length,int format,int declared)
{
	unsigned char bytes[2048]={0};TEST_ASSERT(length>=0&&length<=2000);
	word(bytes+20,format);word(bytes+24,22050);word(bytes+28,44100);word(bytes+40,declared);
	for(int index=0;index<length;index++)bytes[44+index]=(unsigned char)(index*17+length);
	FILE *file=fopen(path,"wb");TEST_ASSERT(file!=NULL);
	TEST_ASSERT(fwrite(bytes,1,44+length,file)==(size_t)(44+length));fclose(file);
}
int main(void)
{
	char previous[sizeof(path_home)],root[]="/tmp/espeak-soundicons-XXXXXX",config[4096],directory[4096],path[4096];
	memcpy(previous,path_home,sizeof(previous));TEST_ASSERT(mkdtemp(root)!=NULL);
	snprintf(path_home,sizeof(path_home),"%s",root);
	snprintf(directory,sizeof(directory),"%s/soundicons",root);TEST_ASSERT(mkdir(directory,0700)==0);
	snprintf(config,sizeof(config),"%s/config",root);samplerate=22050;
	size_t configurations=0,loads=0,bytes=0;
	for(int pass=0;pass<200;pass++) {
		FILE *file=fopen(config,"wb");TEST_ASSERT(file!=NULL);
		fprintf(file,"/tone 9 9\n ignored\ntoneX%d %d %d %d\nsoundicon _! one.wav\nsoundicon _? one.wav\n",pass,pass+1,pass+2,pass+3);
		for(int icon=0;icon<18;icon++)fprintf(file,"soundiconX_%c icon%d.wav ignored\n",'A'+icon,icon);
		fprintf(file,"tone %d %d\n",pass*2,100+pass);fclose(file);
		void *owner=espeak_rs_soundicons_create();TEST_ASSERT(owner!=NULL);
		SOUND_ICON icons[80]={0};int count=0;int32_t points[12]={0};
		TEST_ASSERT(espeak_rs_soundicons_configure(owner,config,&points,N_PATH_BUF,(char)0xff<0,icons,&count)==0);
		ReferenceConfig();compare(icons,count);TEST_ASSERT(memcmp(points,reference_points,sizeof(points))==0);
		configurations+=count;
		espeak_rs_soundicons_destroy(owner);reset_reference();
	}
	/* The final configuration uses duplicate filenames for distinct characters. */
	void *owner=espeak_rs_soundicons_create();TEST_ASSERT(owner!=NULL);
	SOUND_ICON icons[80]={0};int count=0;int32_t points[12]={0};
	TEST_ASSERT(espeak_rs_soundicons_configure(owner,config,&points,N_PATH_BUF,(char)0xff<0,icons,&count)==0);
	ReferenceConfig();
	snprintf(path,sizeof(path),"%s/one.wav",directory);wave(path,510,0x10001,510);
	for(int pass=0;pass<5;pass++) for(int character=0;character<2;character++) {
		int name=character?'?':'!';
		TEST_ASSERT(espeak_rs_soundicons_lookup(owner,root,NULL,name,22050,PATHSEP,N_PATH_BUF,icons,&count)==ReferenceLookup(name));
		TEST_ASSERT(icons[character].length==255);compare(icons,count);loads++;bytes+=510;
	}
	for(int index=0;index<18;index++) {
		char filename[80];snprintf(filename,sizeof(filename),"icon%d.wav",index);
		snprintf(path,sizeof(path),"%s/%s",directory,filename);wave(path,index*10,0x10001,index*10);
		char *address=NULL;
		for(int pass=0;pass<5;pass++) {
			int expected_index=ReferenceFile(filename);
			TEST_ASSERT(espeak_rs_soundicons_lookup(owner,root,filename,0,22050,PATHSEP,N_PATH_BUF,icons,&count)==expected_index);
			compare(icons,count);
			if(address!=NULL)TEST_ASSERT(icons[expected_index].data==address);
			address=icons[expected_index].data;loads++;bytes+=index*10;
		}
	}
	/* Absolute files, warm lookup after deletion, missing and invalid formats. */
	snprintf(path,sizeof(path),"%s/absolute.wav",root);wave(path,120,0x10001,120);
	int absolute=ReferenceFile(path);
	TEST_ASSERT(espeak_rs_soundicons_lookup(owner,root,path,0,22050,PATHSEP,N_PATH_BUF,icons,&count)==absolute);
	compare(icons,count);TEST_ASSERT(unlink(path)==0);
	char *address=icons[absolute].data;
	TEST_ASSERT(espeak_rs_soundicons_lookup(owner,root,path,0,22050,PATHSEP,N_PATH_BUF,icons,&count)==absolute);
	TEST_ASSERT(icons[absolute].data==address);
	TEST_ASSERT(espeak_rs_soundicons_lookup(owner,root,"missing.wav",0,22050,PATHSEP,N_PATH_BUF,icons,&count)==-1);
	snprintf(path,sizeof(path),"%s/bad.wav",directory);wave(path,10,0x10002,10);
	TEST_ASSERT(espeak_rs_soundicons_lookup(owner,root,"bad.wav",0,22050,PATHSEP,N_PATH_BUF,icons,&count)==-1);
	wave(path,10,0x10001,9999);
	TEST_ASSERT(espeak_rs_soundicons_lookup(owner,root,"bad.wav",0,22050,PATHSEP,N_PATH_BUF,icons,&count)==-1);
	TEST_ASSERT(count==21);TEST_ASSERT(unlink(path)==0);
	/* Public compatibility wrappers bind and clear the same owned views. */
	LoadConfig();TEST_ASSERT(n_soundicon_tab==20);
	TEST_ASSERT(LookupSoundicon('!')==0&&LookupSoundicon('?')==1);
	TEST_ASSERT(LoadSoundFile2(soundicon_tab[0].filename)==0);
	FreeSoundIcons();TEST_ASSERT(n_soundicon_tab==0&&soundicon_tab[0].data==NULL);
	LoadConfig();TEST_ASSERT(n_soundicon_tab==20);FreeSoundIcons();
	/* Table admission remains bounded; oversized names are ignored. */
	FILE *file=fopen(config,"wb");TEST_ASSERT(file!=NULL);
	for(int index=0;index<81;index++)fprintf(file,"soundicon _! name%d.wav\n",index);
	fclose(file);void *full=espeak_rs_soundicons_create();TEST_ASSERT(full!=NULL);
	TEST_ASSERT(espeak_rs_soundicons_configure(full,config,&points,N_PATH_BUF,1,icons,&count)==1);
	TEST_ASSERT(count==80);espeak_rs_soundicons_destroy(full);
	espeak_rs_soundicons_destroy(owner);reset_reference();
	snprintf(path,sizeof(path),"%s/one.wav",directory);TEST_ASSERT(unlink(path)==0);
	for(int index=0;index<18;index++){snprintf(path,sizeof(path),"%s/icon%d.wav",directory,index);TEST_ASSERT(unlink(path)==0);}
	TEST_ASSERT(unlink(config)==0);TEST_ASSERT(rmdir(directory)==0);TEST_ASSERT(rmdir(root)==0);
	memcpy(path_home,previous,sizeof(path_home));
	printf("Compared %zu config entries and %zu WAV loads (%zu PCM bytes) with C; ownership/bounds pass\n",configurations,loads,bytes);
	return 0;
}
