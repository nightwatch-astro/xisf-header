# Changelog

## [0.4.0](https://github.com/nightwatch-astro/xisf-header/compare/v0.3.5...v0.4.0) (2026-07-17)


### ⚠ BREAKING CHANGES

* relicense from Apache-2.0 to MPL-2.0 ([#18](https://github.com/nightwatch-astro/xisf-header/issues/18))

### Bug Fixes

* store CLA signatures on unprotected branch, allowlist owner ([#22](https://github.com/nightwatch-astro/xisf-header/issues/22)) ([0a959ee](https://github.com/nightwatch-astro/xisf-header/commit/0a959ee49f2c33d254ec7954903284ee63cbe5a1))
* use GitHub App token for CLA bot instead of PAT ([#20](https://github.com/nightwatch-astro/xisf-header/issues/20)) ([3c61395](https://github.com/nightwatch-astro/xisf-header/commit/3c6139502a7b257fe982d6daad5e43736a26ab3c))


### Miscellaneous Chores

* relicense from Apache-2.0 to MPL-2.0 ([#18](https://github.com/nightwatch-astro/xisf-header/issues/18)) ([9dd9dc7](https://github.com/nightwatch-astro/xisf-header/commit/9dd9dc78d2915e2b9142e55bd2a02f56dda4a222))

## [0.3.5](https://github.com/nightwatch-astro/xisf-header/compare/v0.3.4...v0.3.5) (2026-07-13)


### Documentation

* use absolute URLs for guide and example links so they resolve on docs.rs ([#16](https://github.com/nightwatch-astro/xisf-header/issues/16)) ([2b345ca](https://github.com/nightwatch-astro/xisf-header/commit/2b345ca01ca4142da520818e6affb0390cad42e8))

## [0.3.4](https://github.com/nightwatch-astro/xisf-header/compare/v0.3.3...v0.3.4) (2026-07-13)


### Features

* add Header::write_to_file for safe, race-free new-file creation ([#14](https://github.com/nightwatch-astro/xisf-header/issues/14)) ([c2fcc81](https://github.com/nightwatch-astro/xisf-header/commit/c2fcc81586acc5f4da9542d0429a5f566ba3838a))

## [0.3.3](https://github.com/nightwatch-astro/xisf-header/compare/v0.3.2...v0.3.3) (2026-07-13)


### Bug Fixes

* serialize HISTORY/COMMENT as commentary keywords, not quoted values ([#12](https://github.com/nightwatch-astro/xisf-header/issues/12)) ([6d2ac69](https://github.com/nightwatch-astro/xisf-header/commit/6d2ac69c5d402b0d3c650f1d432c4bdf221777ba))

## [0.3.2](https://github.com/nightwatch-astro/xisf-header/compare/v0.3.1...v0.3.2) (2026-07-13)


### Documentation

* show updating and removing repeated keywords (HISTORY/COMMENT) ([#10](https://github.com/nightwatch-astro/xisf-header/issues/10)) ([3312c39](https://github.com/nightwatch-astro/xisf-header/commit/3312c39f87559f5a6bb9783840c166bc42b5192e))

## [0.3.1](https://github.com/nightwatch-astro/xisf-header/compare/v0.3.0...v0.3.1) (2026-07-13)


### Documentation

* render guide.md on docs.rs and close remaining example gaps ([#8](https://github.com/nightwatch-astro/xisf-header/issues/8)) ([460e049](https://github.com/nightwatch-astro/xisf-header/commit/460e04934498c4b89483e0d197dc59684c24bc53))

## [0.3.0](https://github.com/nightwatch-astro/xisf-header/compare/v0.2.1...v0.3.0) (2026-07-13)


### ⚠ BREAKING CHANGES

* Header::to_bytes and Header::write_to_file are removed (they only ever produced a zero-filled, unmodeled-XML-dropping container). Assemble a new file with to_header_bytes(&hints) plus your own data. Header::update_file(path, edit) drops the &StructuralHints argument and its edit closure now returns Result<()>.

### Features

* byte-exact in-place XISF header editing ([#6](https://github.com/nightwatch-astro/xisf-header/issues/6)) ([de01604](https://github.com/nightwatch-astro/xisf-header/commit/de016048a4a3ab70c3a0d6d0d2e5f353a6a8f82f))

## [0.2.1](https://github.com/nightwatch-astro/xisf-header/compare/v0.2.0...v0.2.1) (2026-07-11)


### Bug Fixes

* skip FITSKeyword elements without a name attribute ([#3](https://github.com/nightwatch-astro/xisf-header/issues/3)) ([07bd826](https://github.com/nightwatch-astro/xisf-header/commit/07bd82615001291bd5bdbf78eded46a0e86e2c21))

## [0.2.0](https://github.com/nightwatch-astro/xisf-header/compare/v0.1.0...v0.2.0) (2026-07-11)


### ⚠ BREAKING CHANGES

* faithful typed <Property> model
* 0.2 faithful-editor API — strict keyword access, typed values, two outputs

### Features

* 0.2 faithful-editor API — strict keyword access, typed values, two outputs ([c4d8897](https://github.com/nightwatch-astro/xisf-header/commit/c4d8897d9f1b6418816e39c3159018099cb8946f))
* faithful typed &lt;Property&gt; model ([1b44aeb](https://github.com/nightwatch-astro/xisf-header/commit/1b44aeb164f5bdc3dbed66d944f91f62db70e191))


### Bug Fixes

* NaN f64 values no longer serialize as `NaN.0` ([b3cba6a](https://github.com/nightwatch-astro/xisf-header/commit/b3cba6aaeec4ea994c969c73cd0353d4918127a3))
