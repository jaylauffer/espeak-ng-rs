# Native Rust replacements; the legacy C build (-DUSE_RUST_CORE=OFF) remains
# the reference oracle.
find_program(ESPEAK_CARGO_EXECUTABLE cargo)
find_program(ESPEAK_RUSTC_EXECUTABLE rustc)
if(NOT ESPEAK_CARGO_EXECUTABLE OR NOT ESPEAK_RUSTC_EXECUTABLE)
  message(FATAL_ERROR "The Rust core needs cargo and rustc (https://rustup.rs). "
    "To build the C engine instead, configure with -DUSE_RUST_CORE=OFF.")
endif()

# The Rust target is the C compiler's: a cross toolchain's triple, the Android
# ABI, the MSVC architecture, or a 32-bit build on a 64-bit host (-m32).
# --target is passed only when it differs from cargo's host.
set(ESPEAK_RUST_TARGET "" CACHE STRING "Rust target triple (default: the C compiler's target)")
function(espeak_rust_target_for_c out)
  set(triple "")
  if(ANDROID)
    set(abi "${CMAKE_ANDROID_ARCH_ABI}")
    if(NOT abi)
      set(abi "${ANDROID_ABI}")
    endif()
    if(abi STREQUAL "arm64-v8a")
      set(triple aarch64-linux-android)
    elseif(abi STREQUAL "armeabi-v7a")
      set(triple armv7-linux-androideabi)
    elseif(abi STREQUAL "x86")
      set(triple i686-linux-android)
    elseif(abi STREQUAL "x86_64")
      set(triple x86_64-linux-android)
    endif()
  elseif(MSVC)
    set(arch "${CMAKE_C_COMPILER_ARCHITECTURE_ID}")
    if(arch STREQUAL "x64")
      set(triple x86_64-pc-windows-msvc)
    elseif(arch STREQUAL "X86")
      set(triple i686-pc-windows-msvc)
    elseif(arch STREQUAL "ARM64")
      set(triple aarch64-pc-windows-msvc)
    endif()
  elseif(CMAKE_SYSTEM_NAME STREQUAL "iOS")
    if(CMAKE_OSX_SYSROOT MATCHES "[Ss]imulator")
      set(triple aarch64-apple-ios-sim)
    else()
      set(triple aarch64-apple-ios)
    endif()
  else()
    set(machine "${CMAKE_C_COMPILER_TARGET}")
    if(NOT machine)
      separate_arguments(c_args UNIX_COMMAND "${CMAKE_C_COMPILER_ARG1}")
      execute_process(COMMAND ${CMAKE_C_COMPILER} ${c_args} -dumpmachine
        OUTPUT_VARIABLE machine OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
    endif()
    string(REGEX MATCH "^[^-]+" arch "${machine}")
    if(APPLE)
      # CMake passes -arch, which decides over the compiler's own target: the
      # one in CMAKE_OSX_ARCHITECTURES, else the system processor.
      set(machine "apple-darwin")
      set(arch "${CMAKE_SYSTEM_PROCESSOR}")
      if(CMAKE_OSX_ARCHITECTURES)
        list(LENGTH CMAKE_OSX_ARCHITECTURES n)
        if(NOT n EQUAL 1)
          message(FATAL_ERROR "The Rust core is built for one architecture; set "
            "CMAKE_OSX_ARCHITECTURES to one (it is ${CMAKE_OSX_ARCHITECTURES})")
        endif()
        set(arch "${CMAKE_OSX_ARCHITECTURES}")
      endif()
    endif()
    if(arch STREQUAL "arm64")
      set(arch aarch64)
    endif()
    if(arch MATCHES "^(x86_64|i[3-6]86)$")
      if(CMAKE_SIZEOF_VOID_P EQUAL 4)
        set(arch i686)
      else()
        set(arch x86_64)
      endif()
    endif()
    if(machine MATCHES "apple-darwin")
      set(triple ${arch}-apple-darwin)
    elseif(machine MATCHES "linux-gnueabihf$")
      set(triple armv7-unknown-linux-gnueabihf)
    elseif(machine MATCHES "linux-gnueabi$")
      # Debian/Ubuntu armel: ARMv5TE, soft float.
      set(triple armv5te-unknown-linux-gnueabi)
    elseif(machine MATCHES "linux-gnu$" AND arch STREQUAL "riscv64")
      set(triple riscv64gc-unknown-linux-gnu)
    elseif(machine MATCHES "linux-gnu$")
      set(triple ${arch}-unknown-linux-gnu)
    elseif(machine MATCHES "linux-musl$")
      set(triple ${arch}-unknown-linux-musl)
    elseif(machine MATCHES "mingw32$")
      set(triple ${arch}-pc-windows-gnu)
    endif()
  endif()
  set(${out} "${triple}" PARENT_SCOPE)
endfunction()
execute_process(COMMAND "${ESPEAK_RUSTC_EXECUTABLE}" -vV OUTPUT_VARIABLE _rustc_version)
string(REGEX MATCH "host: ([^\n]+)" _ "${_rustc_version}")
set(ESPEAK_RUST_HOST "${CMAKE_MATCH_1}")
set(_rust_target "${ESPEAK_RUST_TARGET}")
if(NOT _rust_target)
  espeak_rust_target_for_c(_rust_target)
endif()
if(NOT _rust_target)
  if(CMAKE_CROSSCOMPILING)
    message(FATAL_ERROR "No Rust target is known for this C compiler; pass -DESPEAK_RUST_TARGET=<triple>")
  endif()
  set(_rust_target "${ESPEAK_RUST_HOST}")
endif()
message(STATUS "Rust core target: ${_rust_target} (host ${ESPEAK_RUST_HOST})")

# Rust frees memory the C code allocated and the other way round, so both use
# one C runtime: on MSVC that is the release DLL runtime (/MD) in every
# configuration, Debug included, because Rust links only that one.
if(MSVC)
  set(CMAKE_MSVC_RUNTIME_LIBRARY MultiThreadedDLL)
endif()

# The engine's file reads go through loadngo's proactor (io_uring, kqueue,
# IOCP, or epoll where io_uring is refused). OFF builds read with std::fs.
option(USE_PROACTOR "Route the Rust core's engine I/O through loadngo's proactor" ON)
set(_rust_features c-abi)
if(USE_PROACTOR)
  string(APPEND _rust_features ",proactor")
endif()

# Playback through the Rust audio sink (loadngo-audio-io: ALSA on Linux,
# CoreAudio on macOS), whose writers wait on the proactor. It replaces
# pcaudio when both are on.
set(_rust_audio_default OFF)
if(NOT USE_PROACTOR)
  # the sink waits on the proactor
elseif(NOT _rust_target STREQUAL ESPEAK_RUST_HOST)
  # loadngo-audio-io's device backends build for the host (iOS and Android
  # have none, and cross builds have no target ALSA); pcaudio plays instead.
elseif(APPLE)
  set(_rust_audio_default ON)
elseif(UNIX)
  find_library(ASOUND_LIB asound)
  if(ASOUND_LIB)
    set(_rust_audio_default ON)
  endif()
endif()
option(USE_RUST_AUDIO "Play audio through the Rust core's proactor-waited sink" ${_rust_audio_default})
if(USE_RUST_AUDIO AND NOT USE_PROACTOR)
  message(FATAL_ERROR "USE_RUST_AUDIO requires USE_PROACTOR")
endif()
if(USE_RUST_AUDIO)
  string(APPEND _rust_features ",audio")
endif()

# A sanitized C build needs the Rust core instrumented the same way (MemorySanitizer
# would otherwise report every byte Rust writes): nightly Rust rebuilds std with
# the sanitizer. Needs `rustup toolchain install nightly --component rust-src`.
set(ESPEAK_RUST_SANITIZER "" CACHE STRING "Build the Rust core with this sanitizer, e.g. memory (nightly Rust)")
set(_rust_toolchain)
set(_rust_env)
set(_rust_target_args)
set(_rust_target_dir "${CMAKE_BINARY_DIR}/rust-target")
set(_rust_profile_dir "${_rust_target_dir}")
if(ESPEAK_RUST_SANITIZER)
  set(_rust_toolchain +nightly)
  set(_rust_flags "-Zsanitizer=${ESPEAK_RUST_SANITIZER}")
  if(ESPEAK_RUST_SANITIZER STREQUAL "memory")
    string(APPEND _rust_flags " -Zsanitizer-memory-track-origins")
  endif()
  list(APPEND _rust_env "RUSTFLAGS=${_rust_flags}")
  list(APPEND _rust_target_args -Zbuild-std=std,panic_abort)
endif()
if(ESPEAK_RUST_SANITIZER OR NOT _rust_target STREQUAL ESPEAK_RUST_HOST)
  list(APPEND _rust_target_args --target "${_rust_target}")
  set(_rust_profile_dir "${_rust_profile_dir}/${_rust_target}")
endif()
if(WIN32 AND NOT MINGW)
  set(_rust_library "${_rust_profile_dir}/release/espeak_ng_rs.lib")
else()
  set(_rust_library "${_rust_profile_dir}/release/libespeak_ng_rs.a")
endif()
file(GLOB _rust_sources CONFIGURE_DEPENDS "${PROJECT_SOURCE_DIR}/rust/*.rs")
add_custom_command(
  OUTPUT "${_rust_library}"
  COMMAND "${CMAKE_COMMAND}" -E env ESPEAK_NATIVE_HOST=1 ${_rust_env}
    "${ESPEAK_CARGO_EXECUTABLE}" ${_rust_toolchain} build --locked --release --features "${_rust_features}"
    --manifest-path "${PROJECT_SOURCE_DIR}/Cargo.toml"
    --target-dir "${_rust_target_dir}" ${_rust_target_args}
  DEPENDS ${_rust_sources} "${PROJECT_SOURCE_DIR}/build.rs"
    "${PROJECT_SOURCE_DIR}/Cargo.toml" "${PROJECT_SOURCE_DIR}/Cargo.lock"
  COMMENT "Building native Rust text, dictionary and phoneme core"
  VERBATIM
)
add_custom_target(espeak-rust-build DEPENDS "${_rust_library}")
add_library(espeak-rust-core STATIC IMPORTED GLOBAL)
set_target_properties(espeak-rust-core PROPERTIES IMPORTED_LOCATION "${_rust_library}")
add_dependencies(espeak-rust-core espeak-rust-build)
if(WIN32)
  set_property(TARGET espeak-rust-core PROPERTY INTERFACE_LINK_LIBRARIES
    "advapi32;bcrypt;kernel32;ntdll;userenv;ws2_32")
else()
  set_property(TARGET espeak-rust-core PROPERTY INTERFACE_LINK_LIBRARIES
    "${CMAKE_DL_LIBS};Threads::Threads;m")
  if(ANDROID)
    # Rust's std there also needs liblog (rustc --print native-static-libs:
    # -ldl -llog -lunwind -lm -lc; the NDK's clang adds libunwind itself).
    set_property(TARGET espeak-rust-core APPEND PROPERTY INTERFACE_LINK_LIBRARIES log)
  endif()
endif()
if(USE_RUST_AUDIO)
  if(APPLE)
    set_property(TARGET espeak-rust-core APPEND PROPERTY INTERFACE_LINK_LIBRARIES
      "-framework CoreAudio" "-framework AudioToolbox" "-framework CoreFoundation")
  elseif(UNIX)
    set_property(TARGET espeak-rust-core APPEND PROPERTY INTERFACE_LINK_LIBRARIES asound)
  else()
    set_property(TARGET espeak-rust-core APPEND PROPERTY INTERFACE_LINK_LIBRARIES
      "ole32;oleaut32;winmm")
  endif()
endif()
