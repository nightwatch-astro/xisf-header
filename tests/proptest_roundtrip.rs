// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Property-based round-trip tests: arbitrary headers must survive
//! serialization and parsing unchanged.

use proptest::prelude::*;
use xisf_header::{Header, Literal, StructuralHints};

/// Valid keyword names: 1–8 ASCII alphanumerics, `-`, or `_`. Excludes the
/// FITS commentary keywords (`HISTORY`/`COMMENT`): those are always
/// serialized value="" + text-in-comment (see `is_commentary`), which this
/// generic literal/comment fuzz strategy doesn't model — they get dedicated
/// tests instead.
fn keyword_name() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9_-]{1,8}")
        .unwrap()
        .prop_filter("excludes commentary keywords", |n| {
            n != "HISTORY" && n != "COMMENT"
        })
}

/// String values: printable ASCII (attribute-value normalization folds tabs
/// and newlines, so those are out of scope for a round-trip guarantee).
fn string_value() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[ -~]{0,40}").unwrap()
}

/// Bare literals as they occur in practice: integers, floats, logicals.
fn literal_value() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<i64>().prop_map(|n| n.to_string()),
        (-1.0e15..1.0e15_f64).prop_map(|f| format!("{f}")),
        Just("T".to_owned()),
        Just("F".to_owned()),
    ]
}

/// A keyword: name, value (string or literal), comment.
#[derive(Debug, Clone)]
enum Kind {
    Str(String),
    Lit(String),
}

fn keyword() -> impl Strategy<Value = (String, Kind, String)> {
    (
        keyword_name(),
        prop_oneof![
            string_value().prop_map(Kind::Str),
            literal_value().prop_map(Kind::Lit),
        ],
        string_value(),
    )
}

/// Property ids: ASCII alphanumerics, `_`, `:`.
fn property() -> impl Strategy<Value = (String, String, String)> {
    (
        proptest::string::string_regex("[A-Za-z0-9_:]{1,30}").unwrap(),
        string_value(),
        prop_oneof![
            Just("String".to_owned()),
            Just("Float32".to_owned()),
            Just("Int32".to_owned()),
            Just("TimePoint".to_owned()),
        ],
    )
}

fn build_header(
    keywords: &[(String, Kind, String)],
    properties: &[(String, String, String)],
) -> Header {
    let mut h = Header::new();
    for (name, kind, comment) in keywords {
        let occurrence = h.count(name);
        match kind {
            Kind::Str(s) => h.append(name, s.as_str()).unwrap(),
            Kind::Lit(l) => h.append(name, Literal(l.clone())).unwrap(),
        }
        if !comment.is_empty() {
            h.set_comment((name.as_str(), occurrence), comment.clone())
                .unwrap();
        }
    }
    for (id, value, type_) in properties {
        h.set_property_with_type(id, value, type_).unwrap();
    }
    h
}

proptest! {
    #[test]
    fn arbitrary_headers_round_trip(
        keywords in proptest::collection::vec(keyword(), 0..16),
        properties in proptest::collection::vec(property(), 0..8),
    ) {
        let h = build_header(&keywords, &properties);
        let hints = StructuralHints::default();

        let header_only = Header::parse(&h.to_header_bytes(&hints)).unwrap();
        prop_assert_eq!(&header_only, &h);
    }
}
