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
  string(FIND "${_dictionary_source}" "int TransposeAlphabet(" _begin)
  string(FIND "${_dictionary_source}" "/* End legacy alphabet transposition." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_dictionary_source}" ${_begin} ${_length} _transpose_oracle)
  string(FIND "${_dictionary_source}" "static const char *LookupDict2(" _begin)
  string(FIND "${_dictionary_source}" "/* End legacy contextual dictionary lookup." _end)
  math(EXPR _length "${_end} - ${_begin}")
  string(SUBSTRING "${_dictionary_source}" ${_begin} ${_length} _lookup_oracle)
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/lookup_reference.inc" "${_transpose_oracle}\n${_lookup_oracle}")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/src/libespeak-ng/dictionary.c"
    "${CMAKE_SOURCE_DIR}/src/libespeak-ng/synthdata.c")
  if(UNIX)
    compiled_test(rust_data)
    target_include_directories(test_rust_data PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
    compiled_test(rust_lookup)
    target_include_directories(test_rust_lookup PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
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
