/* Native MBROLA platform configuration; runtime primitives remain external. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "speech.h"
#include "mbrola.h"
#include "rust_data.h"
#include <stddef.h>
#include <stdio.h>
#if defined(_WIN32) || defined(_WIN64)
#include <windows.h>
#else
#define WINAPI
#endif
#if USE_MBROLA
#include "mbrowrap.h"
#endif
typedef struct {
 unsigned enabled,posix;size_t width;
 int (*load)(void);
 int (WINAPI **init)(char *);
 void (WINAPI **close)(void);
 void (WINAPI **reset)(void);
 int (WINAPI **read)(short *,int);
 int (WINAPI **write)(char *);
 int (WINAPI **flush)(void);
 int (WINAPI **frequency)(void);
 void (WINAPI **volume)(float);
 void (WINAPI **no_error)(int);
 FILE *(*standard_error)(void);
} NativeMbrolaConfig;
_Static_assert(sizeof(NativeMbrolaConfig)==8+12*sizeof(void *) &&
 offsetof(NativeMbrolaConfig,standard_error)==8+11*sizeof(void *),"native MBROLA layout");
static FILE *standard_error(void){return stderr;}
const NativeMbrolaConfig espeak_rs_mbrola_config={USE_MBROLA,
#if PLATFORM_POSIX
 1,
#else
 0,
#endif
 N_PATH_BUF,
#if USE_MBROLA
 load_MBR,&init_MBR,&close_MBR,&reset_MBR,&read_MBR,&write_MBR,&flush_MBR,
 &getFreq_MBR,&setVolumeRatio_MBR,&setNoError_MBR,
#else
 NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,
#endif
 standard_error};
_Static_assert(N_PHONEME_LIST==1000 && phonLENGTHEN==12 && phPAUSE==0 &&
 espeakVOICETYPE==14 && espeakPHONEMES_MBROLA==16 && N_ENVELOPE_DATA==20 &&
 espeakINITIALIZE_PHONEME_IPA==2,"native MBROLA constants");
