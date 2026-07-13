# Changelog

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
