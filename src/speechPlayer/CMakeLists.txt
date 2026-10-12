if(USE_RUST_CORE)
  # The backend and public C symbols come from espeak-rust-core. Keep only
  # its public header dependency here; retained C++ is a test-only oracle.
  add_library(speechPlayer INTERFACE)
  target_include_directories(speechPlayer INTERFACE include)
else()
add_library(speechPlayer STATIC
  src/frame.cpp
  src/speechPlayer.cpp
  src/speechWaveGenerator.cpp
)
target_include_directories(speechPlayer PUBLIC include)
if(NOT MSVC)
  target_compile_options(speechPlayer PRIVATE "-fPIC")
endif()
endif()
