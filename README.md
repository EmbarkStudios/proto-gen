<!-- Allow this file to not have a first line heading -->
<!-- markdownlint-disable-file MD041 no-emphasis-as-heading -->

<!-- inline html -->
<!-- markdownlint-disable-file MD033 -->

<div align="center">

# `🌻 proto-gen`

**Protobuf to `Rust` code generation using tonic-build**

[![Embark](https://img.shields.io/badge/embark-open%20source-blueviolet.svg)](https://embark.dev)
[![Embark](https://img.shields.io/badge/discord-ark-%237289da.svg?logo=discord)](https://discord.gg/dAuKfZS)
[![Git Docs](https://img.shields.io/badge/git%20main%20docs-published-blue)](https://embarkstudios.github.io/presser/presser/index.html)
[![dependency status](https://deps.rs/repo/github/EmbarkStudios/proto-gen/status.svg)](https://deps.rs/repo/github/EmbarkStudios/proto-gen)
[![Build status](https://github.com/EmbarkStudios/proto-gen/workflows/CI/badge.svg)](https://github.com/EmbarkStudios/proto-gen/actions)
</div>

## What
The repo contains a lib and a cli that uses tonic-build to generate rust-code from protobuf.  
[tonic-build](https://docs.rs/tonic-build/latest/tonic_build/) already does this, the cli is a front-end to 
that with some added code to make sure that the generated files are placed in a valid path, and takes care of the 
module structuring.

## Why
[prost-build](https://docs.rs/prost-build/latest/prost_build/) used to ship with `cmake` which we would like to avoid.  
`cmake` was used to build `protoc` which was then used for the proto-to-rust codegen.  
The final decision from the prost maintainers side is that the user should provide their own protoc and check in the code
instead of building it in a `build.rs`, to make that process simpler, this cli was created.  

## Contributing

[![Contributor Covenant](https://img.shields.io/badge/contributor%20covenant-v1.4-ff69b4.svg)](CODE_OF_CONDUCT.md)

We welcome community contributions to this project.

Please read our [Contributor Guide](CONTRIBUTING.md) for more information on how to get started.
Please also read our [Contributor Terms](CONTRIBUTING.md#contributor-terms) before you make any contributions.

Any contribution intentionally submitted for inclusion in an Embark Studios project, shall comply with the Rust standard licensing model (MIT OR Apache 2.0) and therefore be dual licensed as described below, without any additional terms or conditions:

### License

This contribution is dual licensed under EITHER OF

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

For clarity, "your" refers to Embark or any other licensee/user of the contribution.
