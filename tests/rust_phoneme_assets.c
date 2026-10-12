/* Complete frozen native C loader/control versus owned Rust assets.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "speech.h"
#include "rust_data.h"
#include "rust_phoneme_assets.h"
#include <ctype.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <sys/stat.h>
#ifdef _WIN32
#include <direct.h>
#include <process.h>
#define mkdir_path(p) _mkdir(p)
#define rmdir_path(p) _rmdir(p)
#define process_id _getpid
#else
#include <unistd.h>
#define mkdir_path(p) mkdir(p,0700)
#define rmdir_path(p) rmdir(p)
#define process_id getpid
#endif
_Static_assert(sizeof(RustPhonemeAssetsResult)==8,"outcome");
_Static_assert(sizeof(RustPhonemeTableView)==sizeof(PHONEME_TAB_LIST),"table view");
#if UINTPTR_MAX==UINT64_MAX
_Static_assert(sizeof(RustPhonemeAssets)==72,"request");
_Static_assert(sizeof(RustPhonemeAssetsView)==9328,"projection");
#endif
#define version_phdata FixtureVersion
static const int FixtureVersion=0x014801;
typedef struct {
 unsigned char *table_data,*wave;char *data;unsigned short *programs;TUNE *tune_data;
 void *core,*index;int table_length,program_length,data_length,table_count,current,active,number,tune_count;
 PHONEME_TAB_LIST tables[150];PHONEME_TAB *selected[256];
 char home[N_PATH_BUF],error_path[4096];int error_status,error_count,version,expected,projects;
} State;
static State *Current;
#define path_home Current->home
#define phoneme_tab_data Current->table_data
#define wavefile_data Current->wave
#define phondata_ptr Current->data
#define phoneme_index Current->programs
#define tunes Current->tune_data
#define rust_core_storage Current->core
#define rust_phontab_index Current->index
#define rust_phontab_length Current->table_length
#define rust_phonindex_length Current->program_length
#define rust_phondata_length Current->data_length
#define n_phoneme_tables Current->table_count
#define current_phoneme_table Current->current
#define n_phoneme_tab Current->active
#define phoneme_tab_number Current->number
#define n_tunes Current->tune_count
#define phoneme_tab_list Current->tables
#define phoneme_tab Current->selected
static int FileError(void *opaque,int status,const char *path)
{(void)opaque;Current->error_status=status;Current->error_count++;snprintf(Current->error_path,sizeof(Current->error_path),"%s",path);return status;}
static int VersionError(void *opaque,const char *path,int actual,int expected)
{(void)opaque;Current->error_status=ENS_VERSION_MISMATCH;Current->error_count++;Current->version=actual;Current->expected=expected;snprintf(Current->error_path,sizeof(Current->error_path),"%s",path);return ENS_VERSION_MISMATCH;}
#define create_file_error_context FileError
#define create_version_mismatch_error_context VersionError
#define LoadPhData OracleLoad
#define FreePhData OracleFree
#define SelectPhonemeTable OracleSelect
#define LookupPhonemeTable OracleLookup
#define SelectPhonemeTableName OracleSelectName
#define SetUpPhonemeTable OracleOverlay
#include "phoneme_assets_reference.inc"
#undef SetUpPhonemeTable
#undef LoadPhData
#undef FreePhData
#undef SelectPhonemeTable
#undef LookupPhonemeTable
#undef SelectPhonemeTableName
static unsigned char **CaptureTable(const RustPhonemeAssetsView *view){Current->table_length=view->lengths[0];Current->projects++;return &Current->table_data;}
#undef phoneme_tab_data
#define phoneme_tab_data (*CaptureTable(view))
#include "phoneme_assets_projections.inc"
#undef phoneme_tab_data
#define phoneme_tab_data Current->table_data
#undef create_file_error_context
#undef create_version_mismatch_error_context
static char directory[N_PATH_BUF];static unsigned comparisons;
static void Path(char out[4096],const char *name){snprintf(out,4096,"%s/%s",directory,name);}
static void Bytes(const char *name,const void *bytes,size_t length)
{char path[4096];Path(path,name);FILE *f=fopen(path,"wb");TEST_ASSERT(f&&fwrite(bytes,1,length,f)==length&&fclose(f)==0);}
static void Remove(const char *name){char path[4096];Path(path,name);remove(path);}
static void AssetsFixtureGenerate(unsigned seed,int warm)
{
 unsigned char bytes[8192];size_t length=0;unsigned count=warm?8:seed%8+1;bytes[length++]=(unsigned char)count;memset(bytes+length,0,3);length+=3;
 for(unsigned i=0;i<count;i++){
  unsigned n=warm?8:(seed+i)%9;bytes[length++]=(unsigned char)n;bytes[length++]=(unsigned char)(i?(seed+i)%i+1:0);bytes[length++]=0;bytes[length++]=0;
  char name[32]={0};snprintf(name,sizeof(name),"t%u",i);memcpy(bytes+length,name,32);length+=32;
  for(unsigned j=0;j<n;j++){PHONEME_TAB row={0};row.mnemonic=seed+i+j+1;row.phflags=seed*3+i;row.code=(unsigned char)((seed+i+j*13)%256);row.type=(unsigned char)((j%7)+1);memcpy(bytes+length,&row,16);length+=16;}
 }
 Bytes("phontab",bytes,length);
 length=warm?8192:(seed*17)%4096;for(size_t i=0;i<length;i++){bytes[i]=(unsigned char)(i*7+seed);}Bytes("phonindex",bytes,length);
 length=warm?8192:8+(seed*23)%4096;for(size_t i=0;i<length;i++){bytes[i]=(unsigned char)(i*13+seed);}uint32_t fields[2]={0x014801,seed%2?22050:44100};memcpy(bytes,fields,8);Bytes("phondata",bytes,length);
 length=warm?8192:(seed*7)%3000;for(size_t i=0;i<length;i++){bytes[i]=(unsigned char)(i*19+seed);}Bytes("intonations",bytes,length);
}
static void Failure(unsigned kind)
{
 if(kind>=1&&kind<=4){Remove((const char *[]) {"phontab","phonindex","phondata","intonations"}[kind-1]);}
 if(kind==5){Bytes("phontab","",0);}
 if(kind==6){Bytes("phondata","bad",3);}
 if(kind==7||kind==8||kind==9){uint32_t data[2]={kind==7?0x014802:0x014801,kind==8?0:(kind==9?UINT32_MAX:22050)};Bytes("phondata",data,8);}
 if(kind==10){unsigned char table[40]={1};table[4]=0;table[5]=1;memcpy(table+8,"cycle",6);Bytes("phontab",table,40);}
 if(kind==11){unsigned char table[40]={1};table[4]=1;memcpy(table+8,"short",6);Bytes("phontab",table,40);}
 if(kind==12){Bytes("phontab","\0\0\0\0",4);}
 if(kind==13){unsigned char tunes_bytes[69]={0};Bytes("intonations",tunes_bytes,69);}
}
static void Compare(State *a,State *b,int full)
{
 TEST_ASSERT(a->table_count==b->table_count&&a->active==b->active&&a->number==b->number&&a->current==b->current);comparisons++;
 TEST_ASSERT(a->error_status==b->error_status&&a->error_count==b->error_count&&a->version==b->version&&a->expected==b->expected&&strcmp(a->error_path,b->error_path)==0);comparisons++;
 unsigned char *ab[]={a->table_data,(unsigned char *)a->programs,(unsigned char *)a->data,(unsigned char *)a->tune_data};unsigned char *bb[]={b->table_data,(unsigned char *)b->programs,(unsigned char *)b->data,(unsigned char *)b->tune_data};
 int sizes[]={a->table_length,a->program_length,a->data_length};int sizes_b[]={b->table_length,b->program_length,b->data_length};
 for(unsigned i=0;i<3;i++){TEST_ASSERT(sizes[i]==sizes_b[i]&&(ab[i]!=NULL)==(bb[i]!=NULL));if(sizes[i]){TEST_ASSERT(memcmp(ab[i],bb[i],sizes[i])==0);}comparisons++;}
 for(int i=0;i<a->table_count;i++){TEST_ASSERT(a->tables[i].n_phonemes==b->tables[i].n_phonemes&&a->tables[i].includes==b->tables[i].includes&&memcmp(a->tables[i].name,b->tables[i].name,32)==0);TEST_ASSERT((unsigned char *)a->tables[i].phoneme_tab_ptr-a->table_data==(unsigned char *)b->tables[i].phoneme_tab_ptr-b->table_data);comparisons++;}
 for(unsigned i=0;i<256;i++){TEST_ASSERT((a->selected[i]!=NULL)==(b->selected[i]!=NULL));if(a->selected[i]){TEST_ASSERT((unsigned char *)a->selected[i]-a->table_data==(unsigned char *)b->selected[i]-b->table_data);TEST_ASSERT(memcmp(a->selected[i],b->selected[i],16)==0);}comparisons++;}
 if(full){TEST_ASSERT(a->tune_count==b->tune_count&&(a->wave!=NULL)==(b->wave!=NULL)&&(a->tune_data!=NULL)==(b->tune_data!=NULL));if(a->tune_count){TEST_ASSERT(memcmp(a->tune_data,b->tune_data,a->tune_count*sizeof(TUNE))==0);}comparisons++;}
}
static void Begin(State *s){memset(s,0,sizeof(*s));snprintf(s->home,sizeof(s->home),"%s",directory);s->current=-1;}
static void Pair(unsigned seed)
{
 State a,b;Begin(&a);Begin(&b);AssetsFixtureGenerate(seed,1);int ra=777,rb=777;
 Current=&a;TEST_ASSERT(OracleLoad(&ra,NULL)==0);Current=&b;TEST_ASSERT(AssetsControl(RUST_ASSETS_LOAD,0,NULL,&rb,NULL)==0&&ra==rb);
 Current=&a;OracleSelect(0);Current=&b;AssetsControl(RUST_ASSETS_SELECT,0,NULL,NULL,NULL);Compare(&a,&b,1);
 unsigned char *addresses[4]={b.table_data,(unsigned char *)b.programs,(unsigned char *)b.data,(unsigned char *)b.tune_data};
 AssetsFixtureGenerate(seed,0);Failure(seed%14);a.number=b.number=(int)(seed%12)-2;a.error_count=b.error_count=0;ra=rb=777;
 Current=&a;int sa=OracleLoad(&ra,NULL);Current=&b;int sb=AssetsControl(RUST_ASSETS_LOAD,0,NULL,&rb,NULL);
 TEST_ASSERT(sa==sb&&ra==rb);Compare(&a,&b,seed%14==0||seed%14>=5);comparisons++;
 TEST_ASSERT(b.table_data==addresses[0]||b.table_data==NULL);TEST_ASSERT((unsigned char *)b.programs==addresses[1]||b.programs==NULL);TEST_ASSERT((unsigned char *)b.data==addresses[2]||b.data==NULL);TEST_ASSERT((unsigned char *)b.tune_data==addresses[3]||b.tune_data==NULL);
 if(seed%14>=1&&seed%14<=4){TEST_ASSERT(b.wave==NULL&&b.tune_count==0);}
 for(unsigned i=0;i<16;i++){
  int number=(int)((seed+i)%12)-2;Current=&a;OracleSelect(number);Current=&b;AssetsControl(RUST_ASSETS_SELECT,number,NULL,NULL,NULL);Compare(&a,&b,0);
  char name[40];snprintf(name,sizeof(name),"t%u",(seed+i)%10);Current=&a;int ia=OracleSelectName(name);Current=&b;int ib=AssetsControl(RUST_ASSETS_SELECT_NAME,0,name,NULL,NULL);TEST_ASSERT(ia==ib);Compare(&a,&b,0);comparisons++;
  if(ib>=0){State expected=a;Current=&expected;memset(expected.selected,0,sizeof(expected.selected));expected.active=0;OracleOverlay(ib);expected.active++;Compare(&expected,&b,0);}
 }
 Current=&a;OracleFree();Current=&b;AssetsControl(RUST_ASSETS_RELEASE,0,NULL,NULL,NULL);Compare(&a,&b,1);
}

static RustPhonemeAssets *guard_descriptor;static int inspect,mutate;
static void GuardProject(void *context,const RustPhonemeAssetsView *view)
{
 if(inspect){RustPhonemeAssets nested=*guard_descriptor;nested.project=GuardProject;RustPhonemeAssetsResult sentinel={42,43};TEST_ASSERT(espeak_rs_phoneme_assets(&nested,&sentinel)==11&&sentinel.rate==42&&sentinel.number==43);}
 if(mutate){mutate=0;guard_descriptor->project=NULL;Current->home[0]='X';}
 AssetsProject(context,view);
}
static void Guards(void)
{
 State b;Begin(&b);Current=&b;AssetsFixtureGenerate(1,0);RustPhonemeAssets d={NULL,b.home,NULL,N_PATH_BUF,PATHSEP,0,0,RUST_ASSETS_LOAD,GuardProject,AssetsFileError,AssetsVersionError};
 guard_descriptor=&d;inspect=mutate=1;RustPhonemeAssetsResult out={42,43};TEST_ASSERT(espeak_rs_phoneme_assets(&d,&out)==0&&out.rate==22050&&b.table_count==2&&b.wave!=NULL);
 inspect=0;snprintf(b.home,sizeof(b.home),"%s",directory);d.project=GuardProject;unsigned projects=b.projects;
 /* Queries and cached selection preserve the existing resident-view fast path. */
 TEST_ASSERT(AssetsControl(RUST_ASSETS_SELECT,0,NULL,NULL,NULL)==0&&b.current==0);
 projects=b.projects;PHONEME_TAB *selected=b.selected[1];
 TEST_ASSERT(selected!=NULL);unsigned mnemonic=selected->mnemonic;selected->mnemonic^=0x1234;
 TEST_ASSERT(AssetsControl(RUST_ASSETS_SELECT,0,NULL,NULL,NULL)==0);
 TEST_ASSERT(AssetsControl(RUST_ASSETS_LOOKUP,0,"t1",NULL,NULL)==1&&b.number==1&&b.current==0);
 TEST_ASSERT(AssetsControl(RUST_ASSETS_SELECT_NAME,0,"t0",NULL,NULL)==0&&b.number==0&&b.current==0);
 TEST_ASSERT(AssetsControl(RUST_ASSETS_LOOKUP,0,"missing",NULL,NULL)==-1&&b.number==0);
 TEST_ASSERT(AssetsControl(RUST_ASSETS_SELECT_NAME,0,"missing",NULL,NULL)==-1&&b.number==0);
 TEST_ASSERT(b.projects==(int)projects&&b.selected[1]==selected);
 TEST_ASSERT(b.selected[1]->mnemonic==(mnemonic^0x1234));selected->mnemonic=mnemonic;
 d.width=1;out.rate=42;TEST_ASSERT(espeak_rs_phoneme_assets(&d,&out)==22&&out.rate==42&&b.projects==(int)projects);
 d.width=N_PATH_BUF;char unterminated[N_PATH_BUF];memset(unterminated,'x',sizeof(unterminated));d.home=unterminated;TEST_ASSERT(espeak_rs_phoneme_assets(&d,&out)==ENAMETOOLONG&&out.rate==42&&b.projects==(int)projects);
 d.home=b.home;d.operation=5;TEST_ASSERT(espeak_rs_phoneme_assets(&d,&out)==22&&out.rate==42);TEST_ASSERT(espeak_rs_phoneme_assets(NULL,&out)==22&&espeak_rs_phoneme_assets(&d,NULL)==22);
 d.operation=RUST_ASSETS_SELECT;d.argument=99;TEST_ASSERT(espeak_rs_phoneme_assets(&d,&out)==22&&out.rate==42&&b.active==0&&b.current==-1);for(unsigned i=0;i<256;i++){TEST_ASSERT(b.selected[i]==NULL);}
 /* Input/result overlap: owned descriptor/name is complete before effects. */
 union {RustPhonemeAssets d;RustPhonemeAssetsResult out;} overlap;overlap.d=d;overlap.d.operation=RUST_ASSETS_SELECT_NAME;overlap.d.name="t1";overlap.d.argument=0;guard_descriptor=&overlap.d;TEST_ASSERT(espeak_rs_phoneme_assets(&overlap.d,&overlap.out)==0&&overlap.out.number==1&&b.number==1&&b.current==1);
 /* Revoke wave aliases when the data allocation grows and a later file fails. */
 unsigned char enlarged[16384]={0};uint32_t header[2]={0x014801,22050};memcpy(enlarged,header,8);Bytes("phondata",enlarged,sizeof(enlarged));Remove("intonations");d.operation=RUST_ASSETS_LOAD;d.argument=0;d.number=b.number;out.rate=42;
 TEST_ASSERT(espeak_rs_phoneme_assets(&d,&out)==ENOENT&&out.rate==42&&b.wave==NULL&&b.tune_count==0&&b.table_count==0&&b.data_length==16384);
 for(unsigned i=0;i<256;i++){TEST_ASSERT(b.selected[i]==NULL);}
 d.operation=RUST_ASSETS_RELEASE;TEST_ASSERT(espeak_rs_phoneme_assets(&d,&out)==0&&b.table_data==NULL&&b.data==NULL&&b.programs==NULL&&b.tune_data==NULL&&b.wave==NULL&&b.active==0&&b.current==-1);
}
int main(void)
{
 const char *temporary=getenv("TMPDIR");
#ifdef _WIN32
 if(!temporary){temporary=getenv("TEMP");}
#endif
 if(!temporary){temporary="/tmp";}
 snprintf(directory,sizeof(directory),"%s/espeak-phoneme-assets-%ld",temporary,(long)process_id());TEST_ASSERT(mkdir_path(directory)==0);
 for(unsigned i=0;i<1000;i++){Pair(i);}Guards();
 for(unsigned i=0;i<4;i++){Remove((const char *[]) {"phontab","phonindex","phondata","intonations"}[i]);}TEST_ASSERT(rmdir_path(directory)==0);
 printf("1000 complete frozen-C resident load/control episodes: %u comparisons; independent C inheritance, live production projections, retained alignment/reuse and snapshot/reentry/overlap/revocation guards passed\n",comparisons);return 0;
}
