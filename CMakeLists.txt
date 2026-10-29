cmake_minimum_required(VERSION 3.8...3.10)
# CMAKE_MSVC_RUNTIME_LIBRARY (cmake/rust.cmake): one C runtime for C and Rust.
if(POLICY CMP0091)
  cmake_policy(SET CMP0091 NEW)
endif()

project(espeak-ng
  VERSION 1.53.0
  DESCRIPTION "open source speech synthesizer that supports more than hundred languages and accents"
  HOMEPAGE_URL "https://github.com/espeak-ng/espeak-ng"
)

option(BUILD_SHARED_LIBS "Build shared libraries" OFF)
option(COMPILE_INTONATIONS "Intonation compilation" ON)
option(ENABLE_TESTS "Enable tests" ON)
option(WITH_COVERAGE "Enable clang source-based coverage instrumentation" OFF)
option(WITH_FUZZER "Build libFuzzer targets (requires Clang)" OFF)
option(USE_RUST_CORE "Use the native Rust speech engine core (OFF builds the original C engine)" ON)

if(MSVC)
  # The C sources check struct layouts with C11 _Static_assert, which MSVC
  # compiles only from /std:c11 (gcc and clang default to C17).
  add_compile_options($<$<COMPILE_LANGUAGE:C>:/std:c11>)
endif()

include(CTest)

if (WITH_COVERAGE OR WITH_FUZZER)
  if (NOT CMAKE_C_COMPILER_ID STREQUAL "Clang")
    message(FATAL_ERROR "WITH_COVERAGE and WITH_FUZZER require Clang")
  endif()
endif()

if (WITH_COVERAGE)
  add_compile_options(-fprofile-instr-generate -fcoverage-mapping)
  set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -fprofile-instr-generate -fcoverage-mapping")
  set(CMAKE_SHARED_LINKER_FLAGS "${CMAKE_SHARED_LINKER_FLAGS} -fprofile-instr-generate -fcoverage-mapping")
endif()

include(GNUInstallDirs)
include(cmake/deps.cmake)
include(cmake/config.cmake)
if(USE_RUST_CORE)
  include(cmake/rust.cmake)
endif()
add_subdirectory(src)

if (COMPILE_INTONATIONS)
  if (CMAKE_CROSSCOMPILING)
    find_program(NATIVEBUILD NAMES espeak-ng PATHS ${NativeBuild_DIR} NO_DEFAULT_PATH)
    if(NATIVEBUILD)
      message(STATUS "Native build of espeak-ng is located at: ${NATIVEBUILD}")
      include(cmake/data.cmake)
    else()
      message(STATUS "Not building intonations as the build is a cross compile.")
      message(STATUS "Call CMake with the location of the native build of espeak-ng")
      message(STATUS "  e.g. -DNativeBuild=~/src/build-native")
      message(STATUS "to build the intonations.")
    endif()
  else()
    include(cmake/data.cmake)
  endif()
endif()
include(cmake/docs.cmake)

include(cmake/package.cmake)
include(CPack)

if(ENABLE_TESTS AND NOT CMAKE_CROSSCOMPILING)
  add_subdirectory(tests)
endif()

message(STATUS "Configuration:")
message(STATUS "  shared: ${BUILD_SHARED_LIBS}")
message(STATUS "  mbrola: ${USE_MBROLA} (${MBROLA_BIN})")
message(STATUS "  libsonic: ${USE_LIBSONIC} (${SONIC_LIB} ${SONIC_INC})")
message(STATUS "  libpcaudio: ${USE_LIBPCAUDIO} (${PCAUDIO_LIB} ${PCAUDIO_INC})")
message(STATUS "  klatt: ${USE_KLATT}")
message(STATUS "  rust core: ${USE_RUST_CORE} (proactor I/O: ${USE_PROACTOR}, audio: ${USE_RUST_AUDIO})")
message(STATUS "  speech-player: ${USE_SPEECHPLAYER}")
message(STATUS "  async: ${USE_ASYNC}")

install(
  DIRECTORY vim/ftdetect vim/syntax DESTINATION share/vim/vimfiles
)
