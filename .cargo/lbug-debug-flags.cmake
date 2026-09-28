# lbug's C++ engine in debug builds, without debug information: cmake-rs
# builds CMake's "Debug" type for any Rust opt-level 0 whatever the profile's
# `debug` says, and its -g made the build 2.6 GB and over 15 minutes on CI's
# runners. Release builds are unchanged. .cargo/config.toml selects this file
# through CMAKE_TOOLCHAIN_FILE; lbug is the workspace's one cmake-rs user.
#
# The fork's reused CMake build (LBUG_REUSE_CMAKE_BUILD) does not see this
# file: after editing it, delete target/*/build/lbug-cmake-* (and CI's cache).
if(CMAKE_HOST_WIN32)
  # A toolchain file stops cmake-rs from naming the compiler; keep MSVC, which
  # lbug's build script assumes, rather than a MinGW g++ found on the PATH.
  set(CMAKE_C_COMPILER cl)
  set(CMAKE_CXX_COMPILER cl)
  set(CMAKE_C_FLAGS_DEBUG "/Ob0 /Od /RTC1" CACHE STRING "")
  set(CMAKE_CXX_FLAGS_DEBUG "/Ob0 /Od /RTC1" CACHE STRING "")
else()
  set(CMAKE_C_FLAGS_DEBUG "-O0" CACHE STRING "")
  set(CMAKE_CXX_FLAGS_DEBUG "-O0" CACHE STRING "")
endif()
