/* Complete previous-native MBROLA host against the actual Rust host.
 * External process primitives are scripted; frontend/resources/queue are real.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "speech.h"
#include "rust_data.h"
#include "mbrola.h"
#include "mbrowrap.h"
#include "setlengths.h"
#include "wavegen.h"
#include "synthdata.h"
#include "intonation.h"
#include <errno.h>
#include <dirent.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

espeak_ng_STATUS previous_LoadMbrolaTable(const char *,const char *,int *);
void previous_FreeMbrolaTable(void),previous_MbrolaReset(void);
int previous_MbrolaGenerate(PHONEME_LIST *,int *,bool),previous_MbrolaFill(int,bool,int);
extern char previous_mbrola_name[20];extern int previous_mbrola_delay;
typedef struct {int op,value;float volume;char path[N_PATH_BUF];} Effect;
static Effect effects[2][128];static unsigned effect_count[2],side;
static int load_ok=1,init_error,rate=22050,write_limit,write_block,read_limit=7,read_error;
static unsigned read_cursor,read_calls,write_calls,reentry;
static unsigned char text[2][200000],pcm[2][20000];static size_t text_length[2],pcm_length[2];
static PHONEME_LIST source[N_PHONEME_LIST+1],rows[2][N_PHONEME_LIST+1];
static intptr_t queue[2][10000][4];static unsigned queued[2],waits[2];
static SPEED_FACTORS saved_speed;static int saved_embedded[N_EMBEDDED_VALUES];
static Effect *record(int op,int value){TEST_ASSERT(effect_count[side]<128);Effect *e=&effects[side][effect_count[side]++];memset(e,0,sizeof(*e));e->op=op;e->value=value;return e;}
BOOL load_MBR(void){record(1,load_ok);return load_ok;}
void unload_MBR(void){}
static int mock_init(char *path){Effect *e=record(2,init_error);TEST_ASSERT(strlen(path)<sizeof(e->path));strcpy(e->path,path);return init_error;}
static void mock_close(void){record(3,0);}
static void mock_reset(void){record(4,0);}
static int mock_frequency(void){record(5,rate);return rate;}
static void mock_volume(float value){record(6,0)->volume=value;}
static void mock_no_error(int value){record(7,value);}
static char *mock_error(char *output,int size){if(size)output[0]=0;return output;}
static int mock_flush(void){return 1;}
static int mock_write(char *data){
 write_calls++;
 if(reentry){reentry=0;int n=2,r=9;TEST_ASSERT(MbrolaGenerate(rows[0],&n,false)==0);
  TEST_ASSERT(LoadMbrolaTable("test","table",&r)==EAGAIN);FreeMbrolaTable();MbrolaReset();}
 if(write_block&&write_calls%3==1)return 0;
 size_t n=strlen(data);if(write_limit&&n>(size_t)write_limit)n=(size_t)write_limit;
 TEST_ASSERT(text_length[side]+n<sizeof(text[side]));memcpy(text[side]+text_length[side],data,n);text_length[side]+=n;return (int)n;
}
static int mock_read(short *output,int count){
 read_calls++;if(read_error)return read_error;
 int n=count<read_limit?count:read_limit;
 for(int i=0;i<n;i++){unsigned value=(read_cursor++*1731+37)&65535;unsigned char *p=(unsigned char *)&output[i];p[0]=value;p[1]=value>>8;}
 return n;
}
int (*init_MBR)(char *)=mock_init;void (*close_MBR)(void)=mock_close;
void (*reset_MBR)(void)=mock_reset;int (*read_MBR)(short *,int)=mock_read;
int (*write_MBR)(char *)=mock_write;int (*flush_MBR)(void)=mock_flush;
int (*getFreq_MBR)(void)=mock_frequency;void (*setVolumeRatio_MBR)(float)=mock_volume;
char *(*lastErrorStr_MBR)(char *,int)=mock_error;void (*setNoError_MBR)(int)=mock_no_error;

static void clear_effects(unsigned which){side=which;effect_count[side]=0;memset(effects[side],0,sizeof(effects[side]));}
static void compare_effects(void){TEST_ASSERT(effect_count[0]==effect_count[1]);TEST_ASSERT(!memcmp(effects[0],effects[1],effect_count[0]*sizeof(Effect)));}
static void load_pair(const char *name,const char *table,int expected){
 int result[2],rates[2]={-77,-77},voice_types[2];
 for(unsigned which=0;which<2;which++){
  clear_effects(which);samplerate=22050;
  result[which]=which?previous_LoadMbrolaTable(name,table,&rates[which]):LoadMbrolaTable(name,table,&rates[which]);
  voice_types[which]=espeak_GetParameter(espeakVOICETYPE,1);
  if(expected==ENS_OK)TEST_ASSERT(voice_types[which]==(name&&rate!=22050));
 }
 if(result[0]!=expected||result[1]!=expected)fprintf(stderr,"load %s/%s result=%x/%x expected=%x\n",name?name:"NULL",table?table:"NULL",result[0],result[1],expected);
 TEST_ASSERT(result[0]==expected&&result[1]==expected&&rates[0]==rates[1]&&voice_types[0]==voice_types[1]);compare_effects();
 TEST_ASSERT(!strcmp(mbrola_name,previous_mbrola_name)&&mbrola_delay==previous_mbrola_delay);
}
static void collect(void){
 for(int i=wcmdq_head;i!=wcmdq_tail;i=(i+1)%N_WCMDQ){
  TEST_ASSERT(queued[side]<10000);intptr_t *out=queue[side][queued[side]++];memcpy(out,wcmdq[i],sizeof(queue[side][0]));
  int op=(int)(out[0]&255);if(op==15)out[2]=out[3]=0;
  if(out[0]==(10+(7<<8))){unsigned char *name=(unsigned char *)&out[2];for(unsigned j=0;j<8;j++)if(!name[j]){memset(name+j,0,8-j);break;}}
 }
 wcmdq_head=wcmdq_tail;
}
static void generate(unsigned which,int count,int file_mode){
 side=which;memcpy(rows[side],source,sizeof(source));speed=saved_speed;memcpy(embedded_value,saved_embedded,sizeof(saved_embedded));
 memset(&espeak_rs_wave_memory,0,sizeof(espeak_rs_wave_memory));queued[side]=waits[side]=0;text_length[side]=0;write_calls=0;
 FILE *file=file_mode?tmpfile():NULL;TEST_ASSERT(!file_mode||file);f_trans=file;option_phonemes=file_mode?espeakPHONEMES_MBROLA:0;
 wcmdq_tail=N_WCMDQ-2;int n=count;
 int result=which?previous_MbrolaGenerate(rows[side],&n,false):MbrolaGenerate(rows[side],&n,false);
 TEST_ASSERT(result==1&&n==count);wcmdq_head=wcmdq_tail;
 for(unsigned turn=0;;turn++){
  TEST_ASSERT(turn<10000);result=which?previous_MbrolaGenerate(rows[side],&n,true):MbrolaGenerate(rows[side],&n,true);collect();
  if(!result){TEST_ASSERT(n==0);break;}waits[side]++;
 }
 if(file){rewind(file);text_length[side]=fread(text[side],1,sizeof(text[side]),file);TEST_ASSERT(feof(file));fclose(file);f_trans=NULL;}
}
static void fill(unsigned which,int amplitude){
 side=which;read_cursor=read_calls=0;pcm_length[which]=0;samplerate=22050;
 static _Alignas(short) unsigned char output[38];
 for(unsigned turn=0;;turn++){
  TEST_ASSERT(turn<1000);out_ptr=output;out_end=output+sizeof(output);
  int result=which?previous_MbrolaFill(17,turn!=0,amplitude):MbrolaFill(17,turn!=0,amplitude);
  size_t length=(size_t)(out_ptr-output);TEST_ASSERT(length<=sizeof(output)&&pcm_length[which]+length<sizeof(pcm[which]));
  memcpy(pcm[which]+pcm_length[which],output,length);pcm_length[which]+=length;if(!result)break;
 }
}
static void write_table(const char *path){
 FILE *file=fopen(path,"wb");TEST_ASSERT(file);
 static const int words[]={23,'a',0,'A','B',60,0,'b',0,'?',0,0,16,'d',0,'D','_',30,0,0,0,0,0,0,0};
 for(unsigned i=0;i<sizeof(words)/sizeof(words[0]);i++)Write4Bytes(file,words[i]);TEST_ASSERT(!fclose(file));
}
static void path_search(const char *root){
 char path[N_PATH_BUF],dirs[N_PATH_BUF];
 snprintf(dirs,sizeof(dirs),"%s/missing::%s:",root,root);TEST_ASSERT(!setenv("XDG_DATA_DIRS",dirs,1));
 snprintf(path,sizeof(path),"%s/alternate",root);TEST_ASSERT(!mkdir(path,0700));strcpy(path_home,path);
 snprintf(path,sizeof(path),"%s/alternate/mbrola_ph",root);TEST_ASSERT(!mkdir(path,0700));
 snprintf(path,sizeof(path),"%s/alternate/mbrola_ph/table",root);write_table(path);
 snprintf(path,sizeof(path),"%s/mbrola/nested",root);TEST_ASSERT(!mkdir(path,0700));
 snprintf(path,sizeof(path),"%s/mbrola/nested/nested",root);FILE *file=fopen(path,"wb");TEST_ASSERT(file);fputc(1,file);fclose(file);
 snprintf(path,sizeof(path),"%s/mbrola/voices",root);TEST_ASSERT(!mkdir(path,0700));
 snprintf(path,sizeof(path),"%s/mbrola/voices/flat",root);file=fopen(path,"wb");TEST_ASSERT(file);fputc(1,file);fclose(file);
 const char *names[]={"test","nested","flat","absent"};
 const char *suffix[]={"mbrola/test","mbrola/nested/nested","mbrola/voices/flat",NULL};
 for(unsigned i=0;i<4;i++){
  load_pair(names[i],"table",ENS_OK);if(suffix[i])snprintf(path,sizeof(path),"%s/%s",root,suffix[i]);else strcpy(path,"absent");
  TEST_ASSERT(effect_count[0]>=3&&effects[0][2].op==2&&!strcmp(effects[0][2].path,path));
 }
 snprintf(path,sizeof(path),"%s/alternate/mbrola_ph/table",root);file=fopen(path,"wb");TEST_ASSERT(file);Write4Bytes(file,999);fclose(file);
 load_pair("test","table",EINVAL); /* Prior active mapping survives failed replacement. */
 snprintf(path,sizeof(path),"%s/alternate/mbrola_ph/table",root);TEST_ASSERT(!unlink(path));
 snprintf(path,sizeof(path),"%s/alternate/mbrola_ph",root);TEST_ASSERT(!rmdir(path));snprintf(path,sizeof(path),"%s/alternate",root);TEST_ASSERT(!rmdir(path));
 snprintf(path,sizeof(path),"%s/mbrola/nested/nested",root);TEST_ASSERT(!unlink(path));snprintf(path,sizeof(path),"%s/mbrola/nested",root);TEST_ASSERT(!rmdir(path));
 snprintf(path,sizeof(path),"%s/mbrola/voices/flat",root);TEST_ASSERT(!unlink(path));snprintf(path,sizeof(path),"%s/mbrola/voices",root);TEST_ASSERT(!rmdir(path));
 strcpy(path_home,root);
}
int main(void){
 TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)==22050);RustOutput saved_output=espeak_rs_output;
 if(!p_decoder)p_decoder=create_text_decoder();TEST_ASSERT(p_decoder);
 char home[N_PATH_BUF];strcpy(home,path_home);const char *xdg=getenv("XDG_DATA_DIRS");char *saved_xdg=xdg?strdup(xdg):NULL;TEST_ASSERT(!xdg||saved_xdg);
 char temporary[]="/tmp/espeak-mbrhost-XXXXXX";TEST_ASSERT(mkdtemp(temporary));char path[N_PATH_BUF];
 snprintf(path,sizeof(path),"%s/mbrola",temporary);TEST_ASSERT(!mkdir(path,0700));
 snprintf(path,sizeof(path),"%s/mbrola/test",temporary);FILE *file=fopen(path,"wb");TEST_ASSERT(file);fputc(1,file);fclose(file);
 snprintf(path,sizeof(path),"%s/mbrola_ph",temporary);TEST_ASSERT(!mkdir(path,0700));
 snprintf(path,sizeof(path),"%s/mbrola_ph/table",temporary);write_table(path);strcpy(path_home,temporary);
 load_pair(NULL,NULL,ENS_OK);load_ok=0;load_pair("test","table",ENS_MBROLA_NOT_FOUND);load_ok=1;
 init_error=-1;load_pair("test","table",ENS_MBROLA_VOICE_NOT_FOUND);init_error=0;
 load_pair("test","missing",ENOENT);
 load_pair("test",".",EISDIR);
 rate=16000;load_pair("test","table",ENS_OK);rate=22050;load_pair("test","table",ENS_OK);
 path_search(temporary);
 strcpy(path_home,home);snprintf(path,sizeof(path),"%s/mbrola_ph",home);DIR *directory=opendir(path);TEST_ASSERT(directory);
 unsigned tables=0;struct dirent *entry;
 while((entry=readdir(directory))!=NULL)if(strstr(entry->d_name,"_phtrans")){load_pair("test",entry->d_name,ENS_OK);tables++;}
 TEST_ASSERT(!closedir(directory)&&tables>0);strcpy(path_home,temporary);
 /* Actual source lists/resources exercise all host effects and queue admission. */
 static const char *voices[]={"en","de","fr","ru","cmn","vi"};
 static const char *phrases[]={"A brave dad asks for a short phrase.","First \001200Sword \001100Ssecond \00150Pword."};
 unsigned episodes=0;
 for(unsigned trial=0;trial<48;trial++){
  strcpy(path_home,home);TEST_ASSERT(espeak_SetVoiceByName(voices[trial%6])==EE_OK);
  TEST_ASSERT(text_decoder_decode_string(p_decoder,phrases[(trial/6)%2],-1,ESPEAKNG_ENCODING_UTF_8)==ENS_OK);
  int tone;char *change;TranslateClause(translator,&tone,&change);CalcPitches(translator,tone);CalcLengths(translator);
  TEST_ASSERT(n_phoneme_list>2);memcpy(source,phoneme_list,sizeof(source));int count=n_phoneme_list;
  saved_speed=speed;memcpy(saved_embedded,embedded_value,sizeof(saved_embedded));option_phoneme_events=trial%3;
  clause_start_char=13;clause_start_word=7;count_sentences=3;
  strcpy(path_home,temporary);load_pair("test","table",ENS_OK);
  write_limit=(trial%4==0)?3:0;write_block=trial%4==0;
  generate(0,count,trial%2);generate(1,count,trial%2);
  if(text_length[0]!=text_length[1]||memcmp(text[0],text[1],text_length[0]))fprintf(stderr,"text mismatch trial=%u bytes=%zu/%zu\n",trial,text_length[0],text_length[1]);
  TEST_ASSERT(text_length[0]==text_length[1]&&!memcmp(text[0],text[1],text_length[0]));
  TEST_ASSERT(queued[0]==queued[1]&&waits[0]==waits[1]);TEST_ASSERT(!memcmp(queue[0],queue[1],queued[0]*sizeof(queue[0][0])));
  TEST_ASSERT(!memcmp(rows[0],rows[1],sizeof(rows[0])));episodes++;
 }
 for(int i=0;i<24;i++){
  read_error=i%8==0?-2:0;read_limit=1+i%19;fill(0,i*7-21);unsigned calls=read_calls;fill(1,i*7-21);
  TEST_ASSERT(calls==read_calls&&pcm_length[0]==pcm_length[1]&&!memcmp(pcm[0],pcm[1],pcm_length[0]));
 }
 for(unsigned which=0;which<2;which++){clear_effects(which);if(which)previous_MbrolaReset();else MbrolaReset();}compare_effects();
 write_limit=write_block=0;reentry=1;generate(0,n_phoneme_list,0);TEST_ASSERT(reentry==0);generate(1,n_phoneme_list,0);
 TEST_ASSERT(text_length[0]==text_length[1]&&!memcmp(text[0],text[1],text_length[0]));
 int bad=1002;TEST_ASSERT(MbrolaGenerate(rows[0],&bad,false)==0);TEST_ASSERT(MbrolaTranslate(NULL,2,false,NULL)==0);
 /* Preserve prior native empty-pitch fallback on a bounded formatter error.
  * Zero length keeps generator arithmetic defined despite the large split. */
 snprintf(path,sizeof(path),"%s/mbrola_ph/table",temporary);file=fopen(path,"r+b");TEST_ASSERT(file);
 TEST_ASSERT(!fseek(file,20,SEEK_SET));Write4Bytes(file,16777216);TEST_ASSERT(!fclose(file));load_pair("test","table",ENS_OK);
 PHONEME_TAB vowel={0};vowel.mnemonic='a';vowel.type=phVOWEL;vowel.code=42;vowel.std_length=80;
 memset(source,0,sizeof(source));for(unsigned i=0;i<3;i++){source[i].ph=&vowel;source[i].type=phVOWEL;source[i].pitch1=90;source[i].pitch2=100;}
 generate(0,2,1);generate(1,2,1);TEST_ASSERT(text_length[0]>0&&text_length[0]==text_length[1]&&!memcmp(text[0],text[1],text_length[0]));
 for(unsigned which=0;which<2;which++){clear_effects(which);if(which)previous_FreeMbrolaTable();else FreeMbrolaTable();}compare_effects();
 TEST_ASSERT(mbrola_delay==0&&previous_mbrola_delay==0&&mbrola_name[0]==0&&previous_mbrola_name[0]==0);
 snprintf(path,sizeof(path),"%s/mbrola_ph/table",temporary);TEST_ASSERT(!unlink(path));snprintf(path,sizeof(path),"%s/mbrola_ph",temporary);TEST_ASSERT(!rmdir(path));
 snprintf(path,sizeof(path),"%s/mbrola/test",temporary);TEST_ASSERT(!unlink(path));snprintf(path,sizeof(path),"%s/mbrola",temporary);TEST_ASSERT(!rmdir(path));TEST_ASSERT(!rmdir(temporary));
 strcpy(path_home,home);espeak_rs_output=saved_output;option_phonemes=0;espeak_Terminate();
 if(saved_xdg){TEST_ASSERT(!setenv("XDG_DATA_DIRS",saved_xdg,1));free(saved_xdg);}else TEST_ASSERT(!unsetenv("XDG_DATA_DIRS"));
 printf("%u whole previous-native MBROLA host clauses, %u actual tables, startup/error/XDG ordering, partial command writes, queue/row/file bytes and24 PCM cases; native admission/reentry pass\n",episodes,tables);
 return 0;
}
