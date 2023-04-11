<!-- markdownlint-disable blanks-around-headings blanks-around-lists no-duplicate-heading -->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->
## [Unreleased] - ReleaseDate
## [0.1.2] - 2023-04-11
### Changed 
- Tonic build upgraded to 0.9.1
### Fixed
- A bug where when multiple protos were part of the same chain 
of packages they would not be put properly into modules, see https://github.com/EmbarkStudios/proto-gen/issues/10
### Fixed
- Escape most doc-tests that tonic generates as that is probably not valid Rust code
and will lead to failed cargo test, and if it is rust code, we definitely don't want to run it. 
## [0.1.1] - 2023-04-04
### Added
- Correct cargo metadata
## [0.1.0] - 2023-04-04
### Added 
- Initial creation of the proto-gen lib and proto-gen-cli

<!-- next-url -->
[Unreleased]: https://github.com/EmbarkStudios/proto-gen/compare/0.1.1...HEAD
[0.1.1]: https://github.com/EmbarkStudios/proto-gen/compare/0.1.0...0.1.1
[0.1.0]: https://github.com/EmbarkStudios/proto-gen/releases/tag/0.1.0
