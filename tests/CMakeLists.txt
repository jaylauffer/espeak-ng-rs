include(CTest)

list(APPEND _binary_tests)

macro(compiled_test _test_name)
  add_executable(test_${_test_name}
    $<TARGET_OBJECTS:espeak-ng>
    ${_test_name}.c
  )
  target_link_libraries(test_${_test_name} PRIVATE
    $<TARGET_PROPERTY:espeak-ng,LINK_LIBRARIES>
  )
  target_compile_definitions(test_${_test_name} PRIVATE LIBESPEAK_NG_EXPORT=1)
  target_include_directories(
    test_${_test_name} PRIVATE
    $<TARGET_PROPERTY:espeak-ng,SOURCE_DIR>
    $<TARGET_PROPERTY:espeak-ng,SOURCE_DIR>/include/compat
    $<TARGET_PROPERTY:espeak-ng,INTERFACE_INCLUDE_DIRECTORIES>
    $<TARGET_PROPERTY:espeak-ng-config,INTERFACE_INCLUDE_DIRECTORIES>
  )
  if (MINGW)
    target_link_options(test_${_test_name} PUBLIC "-static" "-static-libstdc++")
  endif()
  add_dependencies(test_${_test_name} data)
  add_test(
    NAME ${_test_name}
    COMMAND ${ESPEAK_RUN_ENV} $<TARGET_FILE:test_${_test_name}>
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/..
  )
  list(APPEND _binary_tests test_${_test_name})
endmacro(compiled_test)

find_program(SHELL bash)

macro(shell_test _test_name)
  add_test(
    NAME ${_test_name}
    COMMAND ${ESPEAK_RUN_ENV} ESPEAK_BIN=$<TARGET_FILE:espeak-ng-bin> ${SHELL} ${CMAKE_CURRENT_SOURCE_DIR}/${_test_name}.test
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/..
  )
endmacro(shell_test)

compiled_test(api)
compiled_test(encoding)
compiled_test(ieee80)
compiled_test(readclause)

if(USE_RUST_CORE)
  # Extract the unchanged C routines; runtime Rust builds do not compile them.
  file(READ "${CMAKE_SOURCE_DIR}/src/libespeak-ng/dictionary.c" _dictionary_source)
  string(FIND "${_dictionary_source}" "static void InitGroups(" _begin)
  string(FIND "${_dictionary_source}" "/* End legacy rule index." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_dictionary_source}" ${_begin} ${_length} _rule_oracle)
  file(READ "${CMAKE_SOURCE_DIR}/src/libespeak-ng/synthdata.c" _phoneme_source)
  string(FIND "${_phoneme_source}" "static void SetUpPhonemeTable(" _begin)
  string(FIND "${_phoneme_source}" "/* End legacy phoneme overlay." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_phoneme_source}" ${_begin} ${_length} _phoneme_oracle)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/data_reference.inc" "${_rule_oracle}\n${_phoneme_oracle}")
  string(FIND "${_phoneme_source}" "static void InvalidInstn(" _begin)
  string(FIND "${_phoneme_source}" "#ifndef USE_RUST_CORE\nstatic bool StressCondition(" _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_phoneme_source}" ${_begin} ${_length} _diagnostic_oracle)
  string(FIND "${_phoneme_source}" "static bool StressCondition(" _begin)
  string(FIND "${_phoneme_source}" "static void SwitchOnVowelType(" _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_phoneme_source}" ${_begin} ${_length} _condition_oracle)
  string(FIND "${_phoneme_source}" "static void SwitchOnVowelType(" _begin)
  string(FIND "${_phoneme_source}" "/* End legacy phoneme program." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_phoneme_source}" ${_begin} ${_length} _program_oracle)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/program_reference.inc" "${_diagnostic_oracle}\n${_condition_oracle}\n${_program_oracle}")
  string(FIND "${_phoneme_source}" "frameref_t *LookupSpect(" _begin)
  string(FIND "${_phoneme_source}" "/* End legacy spectrum lookup." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_phoneme_source}" ${_begin} ${_length} _spectrum_oracle)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/spectrum_reference.inc" "${_spectrum_oracle}")
  file(READ "${CMAKE_SOURCE_DIR}/src/libespeak-ng/synthesize.c" _synthesis_source)
  string(FIND "${_synthesis_source}" "static frame_t *AllocFrame(" _begin)
  string(FIND "${_synthesis_source}" "/* End legacy formant blending." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_synthesis_source}" ${_begin} ${_length} _formant_oracle)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/formant_reference.inc" "${_formant_oracle}")
  string(FIND "${_synthesis_source}" "static void SmoothSpect(void)\n{" _begin)
  string(FIND "${_synthesis_source}" "/* End legacy spectrum smoothing." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_synthesis_source}" ${_begin} ${_length} _smoothing_oracle)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/smoothing_reference.inc" "${_smoothing_oracle}")
  file(READ "${CMAKE_SOURCE_DIR}/src/libespeak-ng/voices.c" _voice_source)
  foreach(_section "metadata" "scoring" "names" "variant" "ordering" "ranking" "selection" "variants_table")
    if(_section STREQUAL "metadata")
      set(_first "static espeak_VOICE *ReadVoiceFile(")
      set(_last "/* End legacy voice metadata.")
    elseif(_section STREQUAL "scoring")
      set(_first "static int ScoreVoice(")
      set(_last "/* End legacy voice scoring.")
    elseif(_section STREQUAL "names")
      set(_first "espeak_VOICE *SelectVoiceByName(")
      set(_last "/* End legacy voice name matching.")
    elseif(_section STREQUAL "variant")
      set(_first "static char *ExtractVoiceVariantName(")
      set(_last "/* End legacy voice variant names.")
    elseif(_section STREQUAL "ordering")
      set(_first "static int __cdecl VoiceNameSorter(")
      set(_last "/* End legacy voice ordering.")
    elseif(_section STREQUAL "ranking")
      set(_first "static int SetVoiceScores(")
      set(_last "/* End legacy voice ranking.")
    elseif(_section STREQUAL "selection")
      set(_first "char const *SelectVoice(")
      set(_last "/* End legacy voice property selection.")
    else()
      set(_first "#define N_VOICE_VARIANTS")
      set(_last "#ifdef _WIN32\n#define MB_PREFIX")
    endif()
    string(FIND "${_voice_source}" "${_first}" _begin)
    string(FIND "${_voice_source}" "${_last}" _end)
    math(EXPR _length "${_end} - ${_begin}")
    string(SUBSTRING "${_voice_source}" ${_begin} ${_length} _part)
    file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/voice_${_section}_reference.inc" "${_part}")
  endforeach()
  foreach(_section "tone" "defaults" "formant" "numbers" "pitch" "acoustic" "fast" "klatt" "strip")
    if(_section STREQUAL "tone")
      set(_first "static void SetToneAdjust(")
      set(_last "/* End legacy tone configuration.")
    elseif(_section STREQUAL "defaults")
      set(_first "void VoiceReset(")
      set(_last "/* End legacy voice defaults.")
    elseif(_section STREQUAL "formant")
      set(_first "static void VoiceFormant(")
      set(_last "/* End legacy voice formant.")
    elseif(_section STREQUAL "numbers")
      set(_first "int Read8Numbers(")
      set(_last "/* End legacy voice number parsing.")
    elseif(_section STREQUAL "pitch")
      set(_first "            case V_FORMANT:")
      set(_last "/* End legacy voice pitch attributes.")
    elseif(_section STREQUAL "acoustic")
      set(_first "            case V_ECHO:")
      set(_last "/* End legacy voice acoustic attributes.")
    elseif(_section STREQUAL "fast")
      set(_first "            case V_FAST:")
      set(_last "/* End legacy voice fast attribute.")
    elseif(_section STREQUAL "klatt")
      set(_first "                voice->klattv[0] = 1;")
      set(_last "#endif\n                break;\n#else\n            case V_KLATT:")
    else()
      set(_first "static char *fgets_strip(")
      set(_last "#ifndef USE_RUST_CORE\nstatic void SetToneAdjust(")
    endif()
    string(FIND "${_voice_source}" "${_first}" _begin)
    string(FIND "${_voice_source}" "${_last}" _end)
    math(EXPR _length "${_end} - ${_begin}")
    string(SUBSTRING "${_voice_source}" ${_begin} ${_length} _part)
    file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/voice_${_section}_reference.inc" "${_part}")
  endforeach()
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_SOURCE_DIR}/src/libespeak-ng/voices.c")
  file(READ "${CMAKE_SOURCE_DIR}/src/libespeak-ng/langopts.c" _language_source)
  string(FIND "${_language_source}" "void LoadLanguageOptions(" _begin)
  string(FIND "${_language_source}" "/* End legacy language options." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_language_source}" ${_begin} ${_length} _part)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/language_options_reference.inc" "${_part}")
  file(READ "${CMAKE_SOURCE_DIR}/src/libespeak-ng/tr_languages.c" _language_source)
  # Include the original branch as a separately named test oracle. Its config
  # include is omitted so the test can select legacy code independently.
  string(REPLACE "#include \"config.h\"" "" _profile_oracle "${_language_source}")
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/language_profile_reference.inc" "${_profile_oracle}")
  string(REGEX MATCHALL "case L[34]?\\([^)]*\\)" _language_cases "${_language_source}")
  set(_language_names "")
  foreach(_case IN LISTS _language_cases)
    string(REGEX MATCHALL "'[^']'" _letters "${_case}")
    string(JOIN "" _name ${_letters})
    string(REPLACE "'" "" _name "${_name}")
    string(APPEND _language_names "language_pair(\"${_name}\");\n")
    string(LENGTH "${_name}" _name_length)
    if(_name_length EQUAL 4)
      string(APPEND _language_names "language_pair(\"prefix${_name}\");\n")
    endif()
  endforeach()
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/language_names_reference.inc" "${_language_names}")
  string(FIND "${_language_source}" "void ProcessLanguageOptions(" _begin)
  string(FIND "${_language_source}" "/* End legacy language separators." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_language_source}" ${_begin} ${_length} _part)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/language_separators_reference.inc" "${_part}")
  string(FIND "${_voice_source}" "void ReadNumbers(" _begin)
  string(FIND "${_voice_source}" "/* End legacy ordinal flags." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_voice_source}" ${_begin} ${_length} _part)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/language_ordinals_reference.inc" "${_part}")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_SOURCE_DIR}/src/libespeak-ng/langopts.c" "${CMAKE_SOURCE_DIR}/src/libespeak-ng/tr_languages.c")
  string(FIND "${_dictionary_source}" "int TransposeAlphabet(" _begin)
  string(FIND "${_dictionary_source}" "/* End legacy alphabet transposition." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_dictionary_source}" ${_begin} ${_length} _transpose_oracle)
  string(FIND "${_dictionary_source}" "static const char *LookupDict2(" _begin)
  string(FIND "${_dictionary_source}" "/* End legacy contextual dictionary lookup." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_dictionary_source}" ${_begin} ${_length} _lookup_oracle)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/lookup_reference.inc" "${_transpose_oracle}\n${_lookup_oracle}")
  set(_matcher_oracle "")
  foreach(_section "accents" "string_groups" "letters" "matcher" "dollar")
    if(_section STREQUAL "accents")
      set(_first "#define N_REMOVE_ACCENT")
      set(_last "#ifndef USE_RUST_CORE\nstatic int Reverse4Bytes")
    elseif(_section STREQUAL "string_groups")
      set(_first "static int LetterGroupNo(")
      set(_last "/* End legacy string letter groups.")
    elseif(_section STREQUAL "letters")
      set(_first "static int IsLetter(")
      set(_last "/* End legacy scalar letter predicate.")
    elseif(_section STREQUAL "matcher")
      set(_first "static void MatchRule(")
      set(_last "/* End legacy rule matcher.")
    else()
      set(_first "static void DollarRule(")
      set(_last "/* End legacy dollar rule.")
    endif()
    string(FIND "${_dictionary_source}" "${_first}" _begin REVERSE)
    string(FIND "${_dictionary_source}" "${_last}" _end)
    math(EXPR _length "${_end} - ${_begin}")
    string(SUBSTRING "${_dictionary_source}" ${_begin} ${_length} _part)
    string(APPEND _matcher_oracle "${_part}\n")
  endforeach()
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/matcher_reference.inc" "${_matcher_oracle}")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/src/libespeak-ng/dictionary.c"
    "${CMAKE_SOURCE_DIR}/src/libespeak-ng/synthdata.c")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_SOURCE_DIR}/src/libespeak-ng/synthesize.c")
  if(UNIX)
    compiled_test(rust_data)
    target_include_directories(test_rust_data PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
    compiled_test(rust_lookup)
    target_include_directories(test_rust_lookup PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
    compiled_test(rust_rulematch)
    target_include_directories(test_rust_rulematch PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
    compiled_test(rust_program)
    target_include_directories(test_rust_program PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
    compiled_test(rust_spectrum)
    target_include_directories(test_rust_spectrum PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
    compiled_test(rust_voice)
    target_include_directories(test_rust_voice PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
    target_compile_definitions(test_rust_voice PRIVATE ESPEAK_VOICE_SOURCE_DIR="${CMAKE_SOURCE_DIR}/espeak-ng-data")
    compiled_test(rust_language)
    target_include_directories(test_rust_language PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
  endif()
  # Keep a separately named C oracle solely for differential testing.
  add_library(espeak-text-reference STATIC
    ../src/libespeak-ng/encoding.c
    ../src/libespeak-ng/ieee80.c
    ../src/libespeak-ng/mnemonics.c
    ../src/libespeak-ng/phoneme.c
    ../src/ucd-tools/src/case.c
    ../src/ucd-tools/src/categories.c
    ../src/ucd-tools/src/ctype.c
    ../src/ucd-tools/src/proplist.c
    ../src/ucd-tools/src/scripts.c
    ../src/ucd-tools/src/tostring.c
  )
  set(_reference_symbols
    create_text_decoder destroy_text_decoder espeak_ng_EncodingFromName
    text_decoder_decode_string text_decoder_decode_string_auto
    text_decoder_decode_wstring text_decoder_decode_string_multibyte
    text_decoder_eof text_decoder_getc text_decoder_peekc text_decoder_get_buffer
    LookupMnem LookupMnemName ieee_extended_to_double
    phoneme_feature_from_string phoneme_add_feature
    ucd_isalnum ucd_isalpha ucd_isblank ucd_iscntrl ucd_isdigit ucd_isgraph
    ucd_islower ucd_isprint ucd_ispunct ucd_isspace ucd_isupper ucd_isxdigit
    ucd_toupper ucd_tolower ucd_totitle ucd_lookup_category ucd_lookup_category_group
    ucd_lookup_script ucd_properties ucd_get_category_group_for_category
    ucd_get_category_string ucd_get_category_group_string ucd_get_script_string
  )
  foreach(_symbol IN LISTS _reference_symbols)
    target_compile_definitions(espeak-text-reference PRIVATE "${_symbol}=reference_${_symbol}")
  endforeach()
  target_link_libraries(espeak-text-reference PRIVATE espeak-include espeak-ng-config)
  target_compile_definitions(espeak-text-reference PRIVATE LIBESPEAK_NG_EXPORT=1)
  add_executable(test_rust_parity rust_parity.c)
  target_include_directories(test_rust_parity PRIVATE ../src/libespeak-ng)
  target_link_libraries(test_rust_parity PRIVATE espeak-text-reference espeak-rust-core espeak-include espeak-ng-config)
  target_compile_definitions(test_rust_parity PRIVATE LIBESPEAK_NG_EXPORT=1)
  add_test(NAME rust-parity COMMAND test_rust_parity)
  set_tests_properties(rust-parity PROPERTIES TIMEOUT 120)
  add_test(NAME rust-dictionaries
    COMMAND ${CMAKE_COMMAND} -E env "ESPEAK_RUST_DATA_PATH=${CMAKE_BINARY_DIR}/espeak-ng-data"
      "${ESPEAK_CARGO_EXECUTABLE}" test --locked --manifest-path "${CMAKE_SOURCE_DIR}/Cargo.toml"
      --target-dir "${CMAKE_BINARY_DIR}/rust-target" --test compiled_dictionary -- --ignored --nocapture
  )
  add_test(NAME rust-resident-assets
    COMMAND ${CMAKE_COMMAND} -E env "ESPEAK_RUST_DATA_PATH=${CMAKE_BINARY_DIR}/espeak-ng-data"
      "${ESPEAK_CARGO_EXECUTABLE}" test --locked --features proactor --manifest-path "${CMAKE_SOURCE_DIR}/Cargo.toml"
      --target-dir "${CMAKE_BINARY_DIR}/rust-target" --test resident_io -- --ignored --nocapture
  )
  list(APPEND _binary_tests test_rust_parity)
endif()

if (SHELL AND UNIX)

shell_test(bom)
shell_test(non-executable-files-with-executable-bit)

shell_test(cmd_options)
shell_test(dictionary)
shell_test(language-numbers-cardinal)
shell_test(language-numbers-ordinal)
shell_test(language-phonemes)
shell_test(language-pronunciation)
shell_test(language-replace)
shell_test(ssml)
shell_test(translate)
shell_test(variants)
shell_test(voices)
shell_test(crash)

# shell_test(windows-data)
# shell_test(windows-installer)

if (USE_KLATT)
  shell_test(klatt)
endif()
if (USE_MBROLA)
  shell_test(mbrola)
endif()

endif()

add_custom_target(tests DEPENDS ${_binary_tests})

if (WITH_FUZZER AND UNIX)
  add_executable(synth_fuzzer fuzzing/synth_fuzzer.c)
  target_link_libraries(synth_fuzzer PRIVATE
    espeak-ng
    espeak-ng-config
    espeak-include
  )
  target_include_directories(
    synth_fuzzer PRIVATE
    $<TARGET_PROPERTY:espeak-ng,SOURCE_DIR>
    $<TARGET_PROPERTY:espeak-ng,SOURCE_DIR>/include/compat
    $<TARGET_PROPERTY:espeak-ng,INTERFACE_INCLUDE_DIRECTORIES>
    $<TARGET_PROPERTY:espeak-ng-config,INTERFACE_INCLUDE_DIRECTORIES>
  )
  target_compile_definitions(synth_fuzzer PRIVATE LIBESPEAK_NG_EXPORT=1)
  target_compile_definitions(synth_fuzzer PRIVATE PATH_ESPEAK_DATA="${CMAKE_BINARY_DIR}/espeak-ng-data")
  target_compile_options(synth_fuzzer PRIVATE "-fsanitize=fuzzer" "-fno-omit-frame-pointer")
  set_property(TARGET synth_fuzzer APPEND_STRING PROPERTY LINK_FLAGS " -fsanitize=fuzzer")
  add_dependencies(synth_fuzzer data)
endif()
