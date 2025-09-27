cmake_minimum_required(VERSION 3.16)

# CMake shim to build the Rust static library with Cargo and expose it as an IMPORTED target.
# Usage from a parent project:
#   add_subdirectory(xreal_one_driver)
#   target_link_libraries(your_target PRIVATE xreal_one_driver)
# Options:
#   -DXOD_CARGO_CMD=cross   # use cross instead of cargo
#   -DXOD_TARGET=aarch64-unknown-linux-gnu  # pass a Rust target triple to cargo/cross

project(xreal_one_driver_cmake LANGUAGES C)

# Configurable cargo command and rust target triple
set(XOD_CARGO_CMD "cargo" CACHE STRING "Cargo command to use (cargo or cross)")
set(XOD_TARGET "" CACHE STRING "Rust target triple to build for (e.g., aarch64-unknown-linux-gnu)")

# Map CMake build type to Cargo profile
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
string(TOLOWER "${CMAKE_BUILD_TYPE}" XOD_BUILD_TYPE_LOWER)
if(XOD_BUILD_TYPE_LOWER STREQUAL "debug")
  set(XOD_CARGO_PROFILE_FLAG "--profile debug")
  set(XOD_PROFILE_DIR "debug")
else()
  # Treat everything else as release
  set(XOD_CARGO_PROFILE_FLAG "--release")
  set(XOD_PROFILE_DIR "release")
endif()

# Compose cargo build command
set(XOD_MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml")
set(XOD_TARGET_FLAG "")
if(XOD_TARGET)
  set(XOD_TARGET_FLAG "--target ${XOD_TARGET}")
endif()

# Custom target to invoke cargo build
add_custom_target(xreal_one_driver_cargo_build ALL
  COMMAND ${XOD_CARGO_CMD} build --lib ${XOD_CARGO_PROFILE_FLAG} --manifest-path "${XOD_MANIFEST_PATH}" ${XOD_TARGET_FLAG}
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
  COMMENT "Building Rust static library with ${XOD_CARGO_CMD} (${CMAKE_BUILD_TYPE}) ${XOD_TARGET}"
  VERBATIM)

# Compute path to produced static library
if(XOD_TARGET)
  set(XOD_LIB_PATH "${CMAKE_CURRENT_SOURCE_DIR}/target/${XOD_TARGET}/${XOD_PROFILE_DIR}/libxreal_one_driver.a")
else()
  set(XOD_LIB_PATH "${CMAKE_CURRENT_SOURCE_DIR}/target/${XOD_PROFILE_DIR}/libxreal_one_driver.a")
endif()

# Expose an IMPORTED STATIC target to link against from C/C++
add_library(xreal_one_driver STATIC IMPORTED GLOBAL)
set_target_properties(xreal_one_driver PROPERTIES
  IMPORTED_LOCATION "${XOD_LIB_PATH}"
  INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/include"
)
add_dependencies(xreal_one_driver xreal_one_driver_cargo_build)

# Optional: print a message where the library will be located
message(STATUS "xreal_one_driver: IMPORTED_LOCATION=${XOD_LIB_PATH}")

# Build C example executable
add_executable(run_c_example examples/run.c)
target_link_libraries(run_c_example PRIVATE xreal_one_driver)
