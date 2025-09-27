## XrealOne Driver

Xreal one imu driver

## Getting Started

Make sure `rust` is installed

Connect glasses and run example with

```sh
cargo run --example run
```

## C consumers: build with CMake (static library + C example)

This repository ships a CMake shim that builds the Rust static library via Cargo and exposes it as an IMPORTED C target, along with a small C example in `examples/run.c`.

What you get:
- Static library: `libxreal_one_driver.a`
- C headers: `include/xreal_one_driver.h`
- Example app: `run_c_example`

### Build and run with CMake

1) Configure (Release by default):

```sh
cmake -S . -B build
```

Optional flags:
- `-DXOD_CARGO_CMD=cross` to use `cross` instead of `cargo`
- `-DXOD_TARGET=aarch64-unknown-linux-gnu` to target a specific Rust triple

Examples:

```sh
# Use cross
cmake -S . -B build -DXOD_CARGO_CMD=cross

# Cross-compile for aarch64 with cross
cmake -S . -B build -DXOD_CARGO_CMD=cross -DXOD_TARGET=aarch64-unknown-linux-gnu
```

2) Build:

```sh
cmake --build build -j
```

3) Run the C example:

```sh
./build/run_c_example
```

Artifacts (Release):
- Static lib: `target/release/libxreal_one_driver.a` (or `target/<TRIPLE>/release/libxreal_one_driver.a` when `XOD_TARGET` is set)
- Header: `include/xreal_one_driver.h`

Behavior:
- The example continuously calls `xo_next` and prints IMU readings until an error occurs or you press Ctrl-C.
- If the glasses aren’t connected, `xo_new` will fail and the example will exit gracefully.

### Use from your own CMake project

You can vendor this repo and link against the IMPORTED target `xreal_one_driver`:

```cmake
add_subdirectory(path/to/xreal_one_driver)

add_executable(my_app main.c)
target_link_libraries(my_app PRIVATE xreal_one_driver)
```

Optionally, propagate the same knobs:

```cmake
set(XOD_CARGO_CMD cross CACHE STRING "")
set(XOD_TARGET aarch64-unknown-linux-gnu CACHE STRING "")
add_subdirectory(path/to/xreal_one_driver)
```
