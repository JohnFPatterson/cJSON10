# Rust port of cJSON

Safe, bug-for-bug port of cJSON 1.7.19 with a C ABI drop-in shim.

## Crates

- `cjson-core` — parse, print, DOM, and utils. `#![forbid(unsafe_code)]`.
- `cjson-cabi` — the only crate that may use `unsafe`. Exports stock `cJSON_*` / `cJSONUtils_*` symbols and a `#[repr(C)]` `cJSON` layout matching `cJSON.h`.

## Build

```
cargo test --manifest-path rust/Cargo.toml
```

CMake can build the Rust libraries instead of `cJSON.c`:

```
cmake -DENABLE_CJSON_RUST=ON -DENABLE_CJSON_UTILS=ON -S . -B build-rust
cmake --build build-rust
ctest --test-dir build-rust --output-on-failure
```

Unity tests that include `cJSON.c` to reach static helpers are skipped in that configuration. Public-API C tests and the core unit tests cover the port.
