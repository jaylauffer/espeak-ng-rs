/* Frozen full parameter/embedded/table controllers and production projections.
 * GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include "readclause.h"
#include "setlengths.h"
#include "synthesize.h"
#include "translate.h"
#include "ssml.h"
#include "rust_parameter_control.h"
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
extern const int param_defaults[N_SPEECH_PARAM];
extern int saved_parameters[N_SPEECH_PARAM];
_Static_assert(sizeof(PARAM_STACK)==64,"owned parameter frame");
#if UINTPTR_MAX==UINT64_MAX
_Static_assert(sizeof(RustParameterControl)==80&&sizeof(RustLengthEmbedded)==48,"descriptors");
#endif
typedef struct {
 PARAM_STACK stack[20];int saved[15],embedded[15],line,gap,tone;
 int cursor,speeds[3];uint32_t words[250];int trace[64][7];unsigned used;
} State;
static State ref,native;
static Translator ref_translator,native_translator;
static Translator *reference_translator=&ref_translator,*live_translator=&native_translator;
static const int oracle_defaults[15]={0,175,100,50,50,0,0,0,0,0,100,0,0,0,0};
static uint32_t random_state=17;
static uint32_t Next(void){random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state;}
static void Trace(State *s,int kind,int command,int value)
{
 TEST_ASSERT(s->used<64);int *t=s->trace[s->used++];t[0]=kind;t[1]=command;t[2]=value;t[3]=s->cursor;
 t[4]=s->stack[0].parameter[1];t[5]=s->saved[1];t[6]=s->embedded[kind==3?3:2];
}
static void ReferenceSpeed(int control){Trace(&ref,1,control,0);for(int i=0;i<3;i++){ref.speeds[i]=ref.embedded[2]+i+control;}}
static int ReferenceAmplitude(void){Trace(&ref,3,0,0);return 0;}
static void ReferenceSet(int command,int value){Trace(&ref,2,command,value);ref.embedded[2]=value+command;}
#define param_defaults oracle_defaults
#define param_stack ref.stack
#define saved_parameters ref.saved
#define embedded_value ref.embedded
#define embedded_list ref.words
#define option_linelength ref.line
#define option_wordgap ref.gap
#define option_tone_flags ref.tone
#define translator reference_translator
#define SetSpeed ReferenceSpeed
#define GetAmplitude ReferenceAmplitude
#define SetEmbedded ReferenceSet
#define SetParameter ReferenceParameter
#define DoEmbedded2 ReferenceEmbedded
#define SetLengthMods ReferenceMods
#include "parameter_control_reference.inc"
#undef param_defaults
#undef param_stack
#undef saved_parameters
#undef embedded_value
#undef embedded_list
#undef option_linelength
#undef option_wordgap
#undef option_tone_flags
#undef translator
#undef SetSpeed
#undef GetAmplitude
#undef SetEmbedded
#undef SetParameter
#undef DoEmbedded2
#undef SetLengthMods
static int *observed_cursor;
static int len_speeds[3];
static void ReadNative(void){memcpy(native.stack,param_stack,sizeof(native.stack));memcpy(native.saved,saved_parameters,sizeof(native.saved));memcpy(native.embedded,embedded_value,sizeof(native.embedded));if(observed_cursor){native.cursor=*observed_cursor;}}
static RustLengthEmbedded *mutable_d;
static int mutate;
static void NativeSpeed(int control){ReadNative();Trace(&native,1,control,0);for(int i=0;i<3;i++){len_speeds[i]=native.speeds[i]=native.embedded[2]+i+control;}}
static int NativeAmplitude(void){ReadNative();Trace(&native,3,0,0);return 0;}
static void NativeSet(int command,int value){ReadNative();Trace(&native,2,command,value);embedded_value[2]=value+command;if(mutate){memset(native.words,0,sizeof(native.words));mutable_d->speed=NULL;mutable_d->words=NULL;mutate=0;}}
typedef struct {int embedded_ix;} LengthHost;
#define translator live_translator
#define option_linelength native.line
#define option_wordgap native.gap
#define option_tone_flags native.tone
#define SetSpeed NativeSpeed
#define GetAmplitude NativeAmplitude
#define SetEmbedded NativeSet
#define embedded_list native.words
#define SetParameter NativeParameter
#define SetLengthMods NativeMods
#include "parameter_control_projections.inc"
#undef translator
#undef option_linelength
#undef option_wordgap
#undef option_tone_flags
#undef SetSpeed
#undef GetAmplitude
#undef SetEmbedded
#undef embedded_list
#undef SetParameter
#undef SetLengthMods
static void Seed(void)
{
 memset(&ref,0,sizeof(ref));for(unsigned i=0;i<15;i++){ref.stack[0].parameter[i]=(int)(Next()%500)-100;ref.saved[i]=(int)(Next()%500)-100;ref.embedded[i]=(int)(Next()%500)-100;}
 ref.line=17;ref.gap=19;ref.tone=21;ref.speeds[0]=71;ref.speeds[1]=72;ref.speeds[2]=73;
 native=ref;memcpy(param_stack,ref.stack,sizeof(ref.stack));memcpy(saved_parameters,ref.saved,sizeof(ref.saved));memcpy(embedded_value,ref.embedded,sizeof(ref.embedded));
 native_translator.langopts.intonation_group=ref_translator.langopts.intonation_group=42;
}
static unsigned parameter_cases,embedded_cases,mod_cases;
static void ParameterPair(unsigned n)
{
 Seed();int parameter=(int)(n%15),value=(int)(Next()%4001)-2000,relative=(int)(n%3);
 int a=ReferenceParameter(parameter,value,relative),b=NativeParameter(parameter,value,relative);ReadNative();
 TEST_ASSERT(a==b&&!memcmp(ref.stack,native.stack,sizeof(ref.stack))&&!memcmp(ref.saved,native.saved,sizeof(ref.saved))&&!memcmp(ref.embedded,native.embedded,sizeof(ref.embedded)));
 TEST_ASSERT(ref.line==native.line&&ref.gap==native.gap&&ref.tone==native.tone&&ref.used==native.used&&!memcmp(ref.trace,native.trace,ref.used*sizeof(ref.trace[0])));
 TEST_ASSERT(ref_translator.langopts.intonation_group==native_translator.langopts.intonation_group);parameter_cases++;
}
static void EmbeddedPair(unsigned n)
{
 Seed();size_t start=n%230,end=start+1+Next()%20;ref.cursor=native.cursor=(int)start;
 for(size_t i=start;i<end;i++){ref.words[i]=(Next()&~128u);if(i%3==0){ref.words[i]=(175u<<8)|((i%2)?66u:2u);}if(i%7==0){ref.words[i]=0;}}
 ref.words[end-1]|=128;memcpy(native.words,ref.words,sizeof(ref.words));memcpy(len_speeds,ref.speeds,sizeof(len_speeds));
 ReferenceEmbedded(&ref.cursor);
 /* The production projection uses the same cursor cell callbacks observe. */
 LengthHost h={native.cursor};observed_cursor=&h.embedded_ix;int speeds[3]={-1,-1,-1};
 TEST_ASSERT(LengthEmbedded(&h,speeds)==0);ReadNative();observed_cursor=NULL;
 TEST_ASSERT(ref.cursor==native.cursor&&ref.used==native.used&&!memcmp(ref.trace,native.trace,ref.used*sizeof(ref.trace[0]))&&!memcmp(ref.embedded,native.embedded,sizeof(ref.embedded)));
 /* NativeSpeed commits fresh speed values to the production speed view. */
 TEST_ASSERT(!memcmp(ref.speeds,native.speeds,sizeof(ref.speeds))&&!memcmp(ref.speeds,speeds,sizeof(ref.speeds)));
 embedded_cases++;
}
static void Mods(void)
{
 for(int a=0;a<6;a++){for(int b=0;b<6;b++){
  int value=100*b+a;ReferenceMods(&ref_translator,value);NativeMods(&native_translator,value);
  TEST_ASSERT(!memcmp(ref_translator.langopts.length_mods,native_translator.langopts.length_mods,100)&&!memcmp(ref_translator.langopts.length_mods0,native_translator.langopts.length_mods0,100));
  TEST_ASSERT((ref_translator.langopts.length_mods==ref_translator.langopts.length_mods0)==(native_translator.langopts.length_mods==native_translator.langopts.length_mods0));mod_cases++;
 }}
 const unsigned char *a=native_translator.langopts.length_mods,*b=native_translator.langopts.length_mods0;
 TEST_ASSERT(espeak_rs_length_mods(-1,&a,&b)==EINVAL&&a==native_translator.langopts.length_mods&&b==native_translator.langopts.length_mods0);
 TEST_ASSERT(espeak_rs_length_mods(600,&a,&b)==EINVAL);NativeMods(NULL,0);
}
static void Guards(void)
{
 Seed();State before=native;TEST_ASSERT(NativeParameter(-1,1,0)==EINVAL&&NativeParameter(15,1,0)==EINVAL&&NativeParameter(1,INT_MAX,1)==EINVAL);ReadNative();TEST_ASSERT(!memcmp(&before,&native,sizeof(native)));
 live_translator=NULL;TEST_ASSERT(NativeParameter(9,1,0)==EINVAL&&NativeParameter(9,256,0)==0);live_translator=&native_translator;
 Seed();param_stack[0].parameter[1]=100;
 RustParameterControl alias={param_stack[0].parameter,param_stack[0].parameter,saved_parameters,embedded_value,
  &native.line,&native.gap,&native.tone,&native_translator.langopts.intonation_group,NativeSpeed,NativeAmplitude};
 TEST_ASSERT(espeak_rs_set_parameter(&alias,1,200,1)==0&&param_stack[0].parameter[1]==300&&saved_parameters[1]==300&&embedded_value[2]==300);
 Seed();before=native;alias.amplitude=NULL;TEST_ASSERT(espeak_rs_set_parameter(&alias,2,42,0)==EINVAL);ReadNative();TEST_ASSERT(!memcmp(&before,&native,sizeof(native)));
 Seed();memset(native.words,0,sizeof(native.words));int speeds[3]={11,12,13};native.cursor=0;
 RustLengthEmbedded d={native.words,250,&native.cursor,native.speeds,NativeSet,NativeSpeed};
 TEST_ASSERT(espeak_rs_length_embedded(&d,speeds)==EINVAL&&native.cursor==0&&native.used==0&&speeds[0]==11);
 d.length=251;TEST_ASSERT(espeak_rs_length_embedded(&d,speeds)==EINVAL);d.length=250;native.cursor=249;native.words[249]=130;
 TEST_ASSERT(espeak_rs_length_embedded(&d,speeds)==0&&native.cursor==250);TEST_ASSERT(espeak_rs_length_embedded(&d,speeds)==EINVAL);
 Seed();native.words[0]=(100u<<8)|2;native.words[1]=(200u<<8)|130;d.words=native.words;d.cursor=&native.cursor;d.speeds=native.speeds;mutable_d=&d;mutate=1;
 TEST_ASSERT(espeak_rs_length_embedded(&d,speeds)==0&&native.cursor==2&&native.used==4&&embedded_value[2]==202&&speeds[0]==203);
}
int main(void)
{
 /* The Rust core defines these arrays; C compilers may assume the x86-64
  * psABI's 16-byte alignment for global arrays (clang copies them with
  * aligned SSE moves), so the Rust definitions keep it on every target. */
 TEST_ASSERT(((uintptr_t)param_stack&15)==0&&((uintptr_t)saved_parameters&15)==0&&((uintptr_t)param_defaults&15)==0&&((uintptr_t)embedded_value&15)==0);
 TEST_ASSERT(!memcmp(param_defaults,oracle_defaults,sizeof(oracle_defaults)));
 for(int i=0;i<20;i++){TEST_ASSERT(param_stack[i].type==0);for(int j=0;j<15;j++){TEST_ASSERT(param_stack[i].parameter[j]==0);}}
 for(unsigned i=0;i<200000;i++){ParameterPair(i);EmbeddedPair(i);}
 Mods();Guards();printf("%u complete C/native parameter episodes, %u embedded speed spans and %u complete table selections; owned storage and native bounds/copied-input/effect guards pass\n",parameter_cases,embedded_cases,mod_cases);return 0;
}
