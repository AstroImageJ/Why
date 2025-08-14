Cross compiling is provided by cargo-zigbuild
Zig must be version 0.13.0 due to bugs in newer versions.
    https://github.com/rust-cross/cargo-zigbuild/discussions/309
    https://github.com/rust-cross/cargo-zigbuild/issues/324
    https://github.com/ziglang/zig/issues/23179
    https://github.com/rust-lang/rust/issues/112501
To compile for macos, the mac sdk is required.
    this can be provided natively on mac by installing xcode, 
    or on other platforms by downloading an sdk and setting the SDKROOT env var.
        https://github.com/joseluisq/macosx-sdks

Known Issues:
- can't build windows-gnu target on windows with zigbuild
- can't build windows-msvc target with zigbuild