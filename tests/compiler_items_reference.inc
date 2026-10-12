/* Original compiler keyword/property tables from c8393cc0.
 * Copyright (C) 2005-2014 Jonathan Duddington; 2013-2017 Reece H. Dunn.
 * SPDX-License-Identifier: GPL-3.0-or-later */
typedef struct {
	const char *mnem;
	int type;
	int data;
} keywtab_t;

#define k_AND     1
#define k_OR      2
#define k_THEN    3
#define k_NOT     4

#define kTHISSTRESS 0x800

// keyword types
enum {
	tPHONEME_TYPE = 1,
	tPHONEME_FLAG,
	tTRANSITION,
	tSTATEMENT,
	tINSTRN1,
	tWHICH_PHONEME,
	tTEST,
};

static const keywtab_t k_conditions[] = {
	{ "AND",  0, k_AND },
	{ "OR",   0, k_OR },
	{ "THEN", 0, k_THEN },
	{ "NOT",  0, k_NOT },

	{ "prevPh",    tWHICH_PHONEME,  0 },
	{ "thisPh",    tWHICH_PHONEME,  1 },
	{ "nextPh",    tWHICH_PHONEME,  2 },
	{ "next2Ph",   tWHICH_PHONEME,  3 },
	{ "nextPhW",   tWHICH_PHONEME,  4 },
	{ "prevPhW",   tWHICH_PHONEME,  5 },
	{ "next2PhW",  tWHICH_PHONEME,  6 },
	{ "nextVowel", tWHICH_PHONEME,  7 },
	{ "prevVowel", tWHICH_PHONEME,  8 },
	{ "next3PhW",  tWHICH_PHONEME,  9 },
	{ "prev2PhW",  tWHICH_PHONEME, 10 },

	{ "PreVoicing",  tTEST, 0xf01 },
	{ "KlattSynth",  tTEST, 0xf02 },
	{ "MbrolaSynth", tTEST, 0xf03 },

	{ NULL, 0, 0 }
};

static const keywtab_t k_properties[] = {
	{ "isPause",      0, CONDITION_IS_PHONEME_TYPE | phPAUSE },
	{ "isVowel",      0, CONDITION_IS_PHONEME_TYPE | phVOWEL },
	{ "isNasal",      0, CONDITION_IS_PHONEME_TYPE | phNASAL },
	{ "isLiquid",     0, CONDITION_IS_PHONEME_TYPE | phLIQUID },
	{ "isUStop",      0, CONDITION_IS_PHONEME_TYPE | phSTOP },
	{ "isVStop",      0, CONDITION_IS_PHONEME_TYPE | phVSTOP },
	{ "isVFricative", 0, CONDITION_IS_PHONEME_TYPE | phVFRICATIVE },

	{ "isPalatal",  0, CONDITION_IS_PHFLAG_SET | phFLAGBIT_PALATAL },
	{ "isLong",     0, CONDITION_IS_PHFLAG_SET | phFLAGBIT_LONG },
	{ "isRhotic",   0, CONDITION_IS_PHFLAG_SET | phFLAGBIT_RHOTIC },
	{ "isSibilant", 0, CONDITION_IS_PHFLAG_SET | phFLAGBIT_SIBILANT },
	{ "isFlag1",    0, CONDITION_IS_PHFLAG_SET | phFLAGBIT_FLAG1 },
	{ "isFlag2",    0, CONDITION_IS_PHFLAG_SET | phFLAGBIT_FLAG2 },

	{ "isVelar", 0, CONDITION_IS_PLACE_OF_ARTICULATION | phPLACE_VELAR },

	{ "isDiminished",  0, CONDITION_IS_OTHER | STRESS_IS_DIMINISHED },
	{ "isUnstressed",  0, CONDITION_IS_OTHER | STRESS_IS_UNSTRESSED },
	{ "isNotStressed", 0, CONDITION_IS_OTHER | STRESS_IS_NOT_STRESSED },
	{ "isStressed",    0, CONDITION_IS_OTHER | STRESS_IS_SECONDARY },
	{ "isMaxStress",   0, CONDITION_IS_OTHER | STRESS_IS_PRIMARY },

	{ "isPause2",           0, CONDITION_IS_OTHER | isBreak },
	{ "isWordStart",        0, CONDITION_IS_OTHER | isWordStart },
	{ "isWordEnd",          0, CONDITION_IS_OTHER | isWordEnd },
	{ "isAfterStress",      0, CONDITION_IS_OTHER | isAfterStress },
	{ "isNotVowel",         0, CONDITION_IS_OTHER | isNotVowel },
	{ "isFinalVowel",       0, CONDITION_IS_OTHER | isFinalVowel },
	{ "isVoiced",           0, CONDITION_IS_OTHER | isVoiced },
	{ "isFirstVowel",       0, CONDITION_IS_OTHER | isFirstVowel },
	{ "isSecondVowel",      0, CONDITION_IS_OTHER | isSecondVowel },
	{ "isTranslationGiven", 0, CONDITION_IS_OTHER | isTranslationGiven },

	{ NULL, 0, 0 }
};

enum {
	kPHONEMESTART = 1,
	kUTF8_BOM,
	kPROCEDURE,
	kENDPHONEME,
	kENDPROCEDURE,
	kPHONEMETABLE,
	kINCLUDE,
	kIMPORT_PH,

	kSTARTTYPE,
	kENDTYPE,
	kSTRESSTYPE,
	kVOICINGSWITCH,

	kIF,
	kELSE,
	kELIF,
	kENDIF,
	kCALLPH,

	kSWITCH_PREVVOWEL,
	kSWITCH_NEXTVOWEL,
	kENDSWITCH,

	kFMT,
	kWAV,
	kVOWELSTART,
	kVOWELENDING,
	kANDWAV,

	kVOWELIN,
	kVOWELOUT,
	kTONESPEC,

	kRETURN,
	kCONTINUE,
};

enum {
	kTUNE = 1,
	kENDTUNE,
	kTUNE_PREHEAD,
	kTUNE_ONSET,
	kTUNE_HEAD,
	kTUNE_HEADENV,
	kTUNE_HEADEXTEND,
	kTUNE_HEADLAST,
	kTUNE_NUCLEUS0,
	kTUNE_NUCLEUS1,
	kTUNE_SPLIT,
};

static const unsigned char utf8_bom[] = { 0xef, 0xbb, 0xbf, 0 };

#ifndef USE_RUST_CORE
static const keywtab_t k_intonation[] = {
	{ "tune",       0, kTUNE },
	{ "endtune",    0, kENDTUNE },
	{ "prehead",    0, kTUNE_PREHEAD },
	{ "onset",      0, kTUNE_ONSET },
	{ "head",       0, kTUNE_HEAD },
	{ "headenv",    0, kTUNE_HEADENV },
	{ "headextend", 0, kTUNE_HEADEXTEND },
	{ "headlast",   0, kTUNE_HEADLAST },
	{ "nucleus0",   0, kTUNE_NUCLEUS0 },
	{ "nucleus",    0, kTUNE_NUCLEUS1 },
	{ "split",      0, kTUNE_SPLIT },

	{ NULL, 0, -1 }
};
#endif

static const keywtab_t keywords[] = {
	{ "liquid",  tPHONEME_TYPE, phLIQUID },
	{ "pause",   tPHONEME_TYPE, phPAUSE },
	{ "stress",  tPHONEME_TYPE, phSTRESS },
	{ "virtual", tPHONEME_TYPE, phVIRTUAL },
	{ "delete_phoneme", tPHONEME_TYPE, phDELETED },

	// keywords
	{ "phonemetable",         tSTATEMENT, kPHONEMETABLE },
	{ "include",              tSTATEMENT, kINCLUDE },
	{ (const char *)utf8_bom, tSTATEMENT, kUTF8_BOM },

	{ "phoneme",        tSTATEMENT, kPHONEMESTART },
	{ "procedure",      tSTATEMENT, kPROCEDURE },
	{ "endphoneme",     tSTATEMENT, kENDPHONEME },
	{ "endprocedure",   tSTATEMENT, kENDPROCEDURE },
	{ "import_phoneme", tSTATEMENT, kIMPORT_PH },
	{ "stress_type",    tSTATEMENT, kSTRESSTYPE },
	{ "starttype",      tSTATEMENT, kSTARTTYPE },
	{ "endtype",        tSTATEMENT, kENDTYPE },
	{ "voicingswitch",  tSTATEMENT, kVOICINGSWITCH },

	{ "IF",     tSTATEMENT, kIF },
	{ "ELSE",   tSTATEMENT, kELSE },
	{ "ELIF",   tSTATEMENT, kELIF },
	{ "ELSEIF", tSTATEMENT, kELIF },  // same as ELIF
	{ "ENDIF",  tSTATEMENT, kENDIF },
	{ "CALL",   tSTATEMENT, kCALLPH },
	{ "RETURN", tSTATEMENT, kRETURN },

	{ "PrevVowelEndings", tSTATEMENT, kSWITCH_PREVVOWEL },
	{ "NextVowelStarts",  tSTATEMENT, kSWITCH_NEXTVOWEL },
	{ "EndSwitch",        tSTATEMENT, kENDSWITCH },

	{ "Tone",        tSTATEMENT, kTONESPEC },
	{ "FMT",         tSTATEMENT, kFMT },
	{ "WAV",         tSTATEMENT, kWAV },
	{ "VowelStart",  tSTATEMENT, kVOWELSTART },
	{ "VowelEnding", tSTATEMENT, kVOWELENDING },
	{ "addWav",      tSTATEMENT, kANDWAV },

	{ "Vowelin",  tSTATEMENT, kVOWELIN },
	{ "Vowelout", tSTATEMENT, kVOWELOUT },
	{ "Continue", tSTATEMENT, kCONTINUE },

	{ "ChangePhoneme",       tINSTRN1, i_CHANGE_PHONEME },
	{ "ChangeNextPhoneme",   tINSTRN1, i_REPLACE_NEXT_PHONEME },
	{ "InsertPhoneme",       tINSTRN1, i_INSERT_PHONEME },
	{ "AppendPhoneme",       tINSTRN1, i_APPEND_PHONEME },
	{ "IfNextVowelAppend",   tINSTRN1, i_APPEND_IFNEXTVOWEL },
	{ "ChangeIfDiminished",  tINSTRN1, i_CHANGE_IF | STRESS_IS_DIMINISHED },
	{ "ChangeIfUnstressed",  tINSTRN1, i_CHANGE_IF | STRESS_IS_UNSTRESSED },
	{ "ChangeIfNotStressed", tINSTRN1, i_CHANGE_IF | STRESS_IS_NOT_STRESSED },
	{ "ChangeIfStressed",    tINSTRN1, i_CHANGE_IF | STRESS_IS_SECONDARY },
	{ "ChangeIfStressed",    tINSTRN1, i_CHANGE_IF | STRESS_IS_PRIMARY },

	{ "PauseBefore", tINSTRN1, i_PAUSE_BEFORE },
	{ "PauseAfter",  tINSTRN1, i_PAUSE_AFTER },
	{ "length",      tINSTRN1, i_SET_LENGTH },
	{ "LongLength",  tINSTRN1, i_LONG_LENGTH },
	{ "LengthAdd",   tINSTRN1, i_ADD_LENGTH },
	{ "lengthmod",   tINSTRN1, i_LENGTH_MOD },
	{ "ipa",         tINSTRN1, i_IPA_NAME },

	// flags
	{ "unstressed",   tPHONEME_FLAG, phUNSTRESSED },
	{ "nolink",       tPHONEME_FLAG, phNOLINK },
	{ "brkafter",     tPHONEME_FLAG, phBRKAFTER },
	{ "rhotic",       tPHONEME_FLAG, phRHOTIC },
	{ "lengthenstop", tPHONEME_FLAG, phLENGTHENSTOP },
	{ "nopause",      tPHONEME_FLAG, phNOPAUSE },
	{ "prevoice",     tPHONEME_FLAG, phPREVOICE },

	{ "flag1", tPHONEME_FLAG, phFLAG1 },
	{ "flag2", tPHONEME_FLAG, phFLAG2 },

	// vowel transition attributes
	{ "len=",   tTRANSITION,  1 },
	{ "rms=",   tTRANSITION,  2 },
	{ "f1=",    tTRANSITION,  3 },
	{ "f2=",    tTRANSITION,  4 },
	{ "f3=",    tTRANSITION,  5 },
	{ "brk",    tTRANSITION,  6 },
	{ "rate",   tTRANSITION,  7 },
	{ "glstop", tTRANSITION,  8 },
	{ "lenadd", tTRANSITION,  9 },
	{ "f4",     tTRANSITION, 10 },
	{ "gpaus",  tTRANSITION, 11 },
	{ "colr=",  tTRANSITION, 12 },
	{ "amp=",   tTRANSITION, 13 },  // set rms of 1st frame as fraction of rms of 2nd frame  (1/30ths)

	{ NULL, 0, -1 }
};

static const keywtab_t *keyword_tabs[] = {
	keywords, k_conditions, k_properties,
#ifndef USE_RUST_CORE
	k_intonation
#endif
};

