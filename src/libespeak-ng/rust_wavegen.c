/* Native backend configuration/ABI declarations; no waveform engine logic. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "rust_data.h"
#include "rust_klatt.h"
#include "mbrola.h"
#include "speech.h"
#if USE_LIBSONIC
#include "sonic.h"
#else
typedef void *sonicStream;
#endif
typedef struct {
 unsigned int features;
 const char *mbrola_name;
 int (*mbrola_fill)(int,bool,int);
 void (*mbrola_reset)(void);
 sonicStream (*create)(int,int);
 void (*destroy)(sonicStream);
 int (*flush)(sonicStream);
 int (*read)(sonicStream,short *,int);
 int (*write)(sonicStream,const short *,int);
 float (*get_speed)(sonicStream);
 void (*set_speed)(sonicStream,float);
 void (*rescale)(int,int);
} NativeWavegenConfig;
const NativeWavegenConfig espeak_rs_wavegen_config = {
 .features=USE_KLATT|(USE_MBROLA<<1)|(USE_LIBSONIC<<2)|(USE_SPEECHPLAYER<<4)
#ifdef ARCH_BIG
 |8
#endif
 ,
#if USE_MBROLA
 .mbrola_name=mbrola_name,.mbrola_fill=MbrolaFill,.mbrola_reset=MbrolaReset,
#endif
#if USE_LIBSONIC
 .create=sonicCreateStream,.destroy=sonicDestroyStream,.flush=sonicFlushStream,
 .read=sonicReadShortFromStream,.write=sonicWriteShortToStream,
 .get_speed=sonicGetSpeed,.set_speed=sonicSetSpeed,.rescale=RescaleEventSamples,
#endif
};
_Static_assert(sizeof(NativeWavegenConfig)==96 && sizeof(RustWavegenHost)==184 &&
 sizeof(RustWavegenEffect)==56 && sizeof(RustWavegenShared)==32 && sizeof(RustWavegenOptions)==20 &&
 sizeof(RustKlattShared)==32 && sizeof(voice_t)==1344 && sizeof(WGEN_DATA)==80 &&
 sizeof(wavegen_peaks_t)==80 && sizeof(RustOutput)==40 && sizeof(RustWaveMemory)==16464,
 "native backend layout");
_Static_assert(EMBED_P==1 && EMBED_T==6 && EMBED_R==4 && N_PEAKS==9 &&
 N_WCMDQ==170 && N_EMBEDDED_VALUES==15,"native backend constants");
