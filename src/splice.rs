//! Byte-exact, data-preserving `Header::update_file`: splice only the edited
//! `<FITSKeyword>`/`<Property>` elements (and the attachment offset, if the
//! header's length changed) into the original file's raw bytes, leaving
//! every other byte — unmodeled XML, whitespace, the attached data block —
//! untouched.

use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;

use crate::error::{Error, Result};
use crate::header::Header;
use crate::keyword::FitsKeyword;
use crate::property::Property;
use crate::reader::{self, ImageInfo};
use crate::value::Value;

const INFALLIBLE: &str = "writing XML to an in-memory buffer cannot fail";

/// A byte range in the original XML replaced by `bytes` (empty to delete the
/// range, or a zero-width `start == end` to insert without deleting).
struct Region {
    start: usize,
    end: usize,
    bytes: Vec<u8>,
}

/// Read `path`, apply `edit` to its parsed [`Header`], and splice the result
/// back in place. See [`crate::Header::update_file`] for the public contract.
pub(crate) fn update_file<P: AsRef<Path>>(
    path: P,
    edit: impl FnOnce(&mut Header) -> Result<()>,
) -> Result<()> {
    let path = path.as_ref();
    let original = std::fs::read(path)?;
    let (xml_start, xml_end) = reader::split_preamble(&original)?;
    let xml = std::str::from_utf8(&original[xml_start..xml_end])?;
    let (before, index) = reader::parse_xml_with_index(xml)?;
    let reader::XmlIndex {
        keyword_spans,
        property_spans,
        image,
    } = index;
    let image = image.map_err(Error::Unsupported)?;

    let mut after = before.clone();
    edit(&mut after)?;

    let xml_bytes = xml.as_bytes();
    let mut regions = content_regions(&before, &after, &keyword_spans, &property_spans, &image)?;
    regions.sort_by_key(|r| r.start);

    let content_delta: isize = regions
        .iter()
        .map(|r| r.bytes.len() as isize - (r.end - r.start) as isize)
        .sum();

    if content_delta != 0 {
        regions.push(relocate_attachment(xml_bytes.len(), content_delta, &image)?);
        regions.sort_by_key(|r| r.start);
    }

    let new_xml = splice(xml_bytes, &regions);
    write_container(path, &original, xml_start, xml_end, &image, &new_xml)
}

/// Build every region except the (possibly unnecessary) attachment-offset
/// rewrite: edited/removed keyword and property spans, plus one insertion
/// region for anything newly added.
fn content_regions(
    before: &Header,
    after: &Header,
    keyword_spans: &[(usize, usize)],
    property_spans: &[(String, usize, usize)],
    image: &ImageInfo,
) -> Result<Vec<Region>> {
    let mut regions = Vec::new();

    let (kept, new_keywords) = match_keywords(&before.keywords, &after.keywords);
    for (i, &(start, end)) in keyword_spans.iter().enumerate() {
        match kept[i] {
            Some(j) if after.keywords[j] == before.keywords[i] => {} // unchanged: leave verbatim
            Some(j) => regions.push(Region {
                start,
                end,
                bytes: render_keyword(&after.keywords[j]),
            }),
            None => regions.push(Region {
                start,
                end,
                bytes: Vec::new(),
            }), // removed
        }
    }

    for (id, start, end) in property_spans {
        match after.properties.get(id) {
            None => regions.push(Region {
                start: *start,
                end: *end,
                bytes: Vec::new(),
            }),
            Some(p) if before.properties.get(id) == Some(p) => {} // unchanged: leave verbatim
            Some(p) => regions.push(Region {
                start: *start,
                end: *end,
                bytes: render_property(id, p),
            }),
        }
    }
    let existing_ids: std::collections::HashSet<&str> = property_spans
        .iter()
        .map(|(id, _, _)| id.as_str())
        .collect();
    let new_properties: Vec<&String> = after
        .properties
        .keys()
        .filter(|id| !existing_ids.contains(id.as_str()))
        .collect();

    if !new_keywords.is_empty() || !new_properties.is_empty() {
        let insertion_point = image.insertion_point.ok_or_else(|| {
            Error::Unsupported(
                "cannot add keywords or properties: <Image> is self-closing and has no \
                 children to insert into"
                    .to_owned(),
            )
        })?;
        let mut block = Vec::new();
        for kw in &new_keywords {
            block.extend_from_slice(b"\n    ");
            block.extend_from_slice(&render_keyword(kw));
        }
        for id in &new_properties {
            block.extend_from_slice(b"\n    ");
            block.extend_from_slice(&render_property(id, &after.properties[id.as_str()]));
        }
        regions.push(Region {
            start: insertion_point,
            end: insertion_point,
            bytes: block,
        });
    }

    Ok(regions)
}

/// Match each original keyword slot to its surviving position in `after`, by
/// name, preserving relative order; anything left over in `after` is newly
/// appended. Relies on the public `Header` API invariant that edits only
/// update an existing slot in place, delete a slot, or push new slots to the
/// tail — never reorder or insert mid-vector — so a name match, walked
/// left-to-right, is enough to tell "kept" from "removed" from "new".
///
/// With duplicate names (e.g. repeated `HISTORY`), removing one occurrence
/// while editing another in the same `edit` closure can misattribute the
/// edit to the wrong original span: the resulting `Header` value is still
/// correct, but the surviving occurrence may be re-rendered (losing its
/// original byte formatting) instead of a removed one. This is a narrow,
/// documented limitation, not a data-loss risk.
fn match_keywords<'a>(
    before: &[FitsKeyword],
    after: &'a [FitsKeyword],
) -> (Vec<Option<usize>>, Vec<&'a FitsKeyword>) {
    let mut kept = vec![None; before.len()];
    let mut j = 0;
    for (i, kw) in before.iter().enumerate() {
        if j < after.len() && after[j].name == kw.name {
            kept[i] = Some(j);
            j += 1;
        }
    }
    (kept, after[j..].iter().collect())
}

/// Recompute the `attachment:OFFSET:SIZE` location text so it reflects the
/// new total header length, converging on the offset's own digit width
/// (which itself contributes to that length) by fixed-point iteration —
/// mirroring the two-pass render in [`crate::writer`].
fn relocate_attachment(xml_len: usize, content_delta: isize, image: &ImageInfo) -> Result<Region> {
    let (loc_start, loc_end) = image.location_span;
    let old_loc_len = loc_end - loc_start;

    let mut offset = 16 + xml_len; // initial guess: ignore the offset-text width delta
    for _ in 0..16 {
        let text = format!("attachment:{offset}:{}", image.size);
        let loc_delta = text.len() as isize - old_loc_len as isize;
        let new_xml_len: usize = (xml_len as isize + content_delta + loc_delta)
            .try_into()
            .map_err(|_| {
                Error::Unsupported("edit shrank the header below zero bytes".to_owned())
            })?;
        let needed_offset: usize = 16 + new_xml_len;
        if needed_offset == offset {
            return Ok(Region {
                start: loc_start,
                end: loc_end,
                bytes: text.into_bytes(),
            });
        }
        offset = needed_offset;
    }
    Err(Error::Unsupported(
        "attachment offset did not converge while recomputing the header length".to_owned(),
    ))
}

/// Apply non-overlapping `regions` (sorted by `start`) to `xml`, copying
/// everything between and around them verbatim.
fn splice(xml: &[u8], regions: &[Region]) -> Vec<u8> {
    let mut out = Vec::with_capacity(xml.len());
    let mut cursor = 0;
    for r in regions {
        out.extend_from_slice(&xml[cursor..r.start]);
        out.extend_from_slice(&r.bytes);
        cursor = r.end;
    }
    out.extend_from_slice(&xml[cursor..]);
    out
}

/// Assemble the final container and write it atomically: preamble (with the
/// new XML length; signature and reserved bytes preserved verbatim) + the
/// spliced XML + the original gap/data/trailing bytes, moved verbatim to sit
/// right after the (possibly relocated) header.
fn write_container(
    path: &Path,
    original: &[u8],
    xml_start: usize,
    xml_end: usize,
    image: &ImageInfo,
    new_xml: &[u8],
) -> Result<()> {
    let gap_start = xml_end;
    let data_start = image.offset;
    let data_end = data_start
        .checked_add(image.size)
        .filter(|&e| e <= original.len())
        .ok_or_else(|| {
            Error::Unsupported("attachment location/size is out of bounds".to_owned())
        })?;
    if data_start < gap_start {
        return Err(Error::Unsupported(
            "attachment offset overlaps the XML header".to_owned(),
        ));
    }
    let gap = &original[gap_start..data_start];
    let data = &original[data_start..data_end];
    let trailing = &original[data_end..];

    let mut out = Vec::with_capacity(16 + new_xml.len() + gap.len() + data.len() + trailing.len());
    out.extend_from_slice(&original[0..8]); // signature, unchanged
    out.extend_from_slice(
        &u32::try_from(new_xml.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    out.extend_from_slice(&original[12..xml_start]); // reserved bytes, preserved verbatim
    out.extend_from_slice(new_xml);
    out.extend_from_slice(gap);
    out.extend_from_slice(data);
    out.extend_from_slice(trailing);

    let tmp_path = tmp_path_for(path);
    if let Err(e) = std::fs::write(&tmp_path, &out) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// A sibling temp path (`.<name>.tmp-<pid>`) for an atomic write-then-rename.
fn tmp_path_for(path: &Path) -> std::path::PathBuf {
    let file_name = path.file_name().unwrap_or_default();
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(file_name);
    tmp_name.push(format!(".tmp-{}", std::process::id()));
    path.with_file_name(tmp_name)
}

/// Render a single `<FITSKeyword .../>` element.
fn render_keyword(kw: &FitsKeyword) -> Vec<u8> {
    let mut w = Writer::new(Vec::new());
    let mut e = BytesStart::new("FITSKeyword");
    e.push_attribute(("name", kw.name.as_str()));
    let value = match &kw.value {
        Value::Str(s) => format!("'{s}'"),
        Value::Literal(s) => s.clone(),
    };
    e.push_attribute(("value", value.as_str()));
    e.push_attribute(("comment", kw.comment.as_str()));
    w.write_event(Event::Empty(e)).expect(INFALLIBLE);
    w.into_inner()
}

/// Render a single `<Property .../>` element.
fn render_property(id: &str, p: &Property) -> Vec<u8> {
    let mut w = Writer::new(Vec::new());
    let mut e = BytesStart::new("Property");
    e.push_attribute(("id", id));
    e.push_attribute(("type", p.type_.as_str()));
    e.push_attribute(("value", p.value.as_str()));
    if !p.format.is_empty() {
        e.push_attribute(("format", p.format.as_str()));
    }
    if !p.comment.is_empty() {
        e.push_attribute(("comment", p.comment.as_str()));
    }
    w.write_event(Event::Empty(e)).expect(INFALLIBLE);
    w.into_inner()
}
