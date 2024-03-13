<!-- markdownlint-disable blanks-around-headings blanks-around-lists no-duplicate-heading -->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->
## [Unreleased] - ReleaseDate
## [0.2.5] - 2024-03-13
### Fixed
- [PR#22](https://github.com/EmbarkStudios/proto-gen/pull/22) Various fixes for the `--prepend-header` option and some cases for escaped doc-tests.
## [0.2.4] - 2024-03-12
### Fixed
- [PR#21](https://github.com/EmbarkStudios/proto-gen/pull/21) Fix for handling multiline code blocks in comments and make them ignored for doc tests.
## [0.2.3] - 2024-03-12
### Added
- [PR#19](https://github.com/EmbarkStudios/proto-gen/pull/19) Added `-p, --prepend-header` option (default false) to prepend header indicating tool version in generated source file.
- [PR#20](https://github.com/EmbarkStudios/proto-gen/pull/20) Added `--toplevel-attribute` option to set toplevel module attribute.
## [0.2.2] - 2024-03-11
### Fixed
- [PR#17](https://github.com/EmbarkStudios/proto-gen/pull/17) Make errors slightly easier to read.
- [PR#18](https://github.com/EmbarkStudios/proto-gen/pull/18) Fix handling of filename for target .rs file when it's a keyword.
## [0.2.1] - 2024-03-01
### Added
- [PR#15](https://github.com/EmbarkStudios/proto-gen/pull/15) added the `-d, --disable-comments <path>` option to code generation, allowing comments to be [disabled](https://docs.rs/prost-build/latest/prost_build/struct.Config.html#method.disable_comments) for one or more proto paths.

## [0.2.0] - 2023-04-12
### Changed
- Tonic build upgraded to 0.10.2
## [0.1.2] - 2023-04-11
### Changed
- Tonic build upgraded to 0.9.1
### Fixed
- A bug where when multiple protos were part of the same chain
of packages they would not be put properly into modules, see <https://github.com/EmbarkStudios/proto-gen/issues/10>
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
[Unreleased]: https://github.com/EmbarkStudios/proto-gen/compare/0.2.5...HEAD
[0.2.5]: https://github.com/EmbarkStudios/proto-gen/compare/0.2.4...0.2.5
[0.2.4]: https://github.com/EmbarkStudios/proto-gen/compare/0.2.3...0.2.4
[0.2.3]: https://github.com/EmbarkStudios/proto-gen/compare/0.2.2...0.2.3
[0.2.2]: https://github.com/EmbarkStudios/proto-gen/compare/0.2.1...0.2.2
[0.2.1]: https://github.com/EmbarkStudios/proto-gen/compare/0.1.1...0.2.1
[0.1.1]: https://github.com/EmbarkStudios/proto-gen/compare/0.1.0...0.1.1
[0.1.0]: https://github.com/EmbarkStudios/proto-gen/releases/tag/0.1.0
