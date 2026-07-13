//! Quickstart: build a master-dark calibration header, edit it, and
//! round-trip it through a real XISF container.
//!
//! This is the canonical example mirrored in `README.md` and `docs/guide.md`.
//! Run it with `cargo run --example quickstart`.

use xisf_header::{Fixed, Header, StructuralHints};

fn main() -> Result<(), xisf_header::Error> {
    // 1. Create a header for a master dark calibration frame.
    let mut header = Header::new();
    header.set("IMAGETYP", "Master Dark")?;
    header.set_comment("IMAGETYP", "Type of image")?;
    header.set("EXPTIME", Fixed(300.0, 2))?; // fixed-point, 2 decimals
    header.set("GAIN", 100_i64)?;

    // 2. Track processing steps with repeated HISTORY keywords.
    header.append("HISTORY", "reduced with siril")?;
    header.append("HISTORY", "stacked 20x300s")?;

    // 3. Attach XISF <Property> metadata alongside the FITS keywords.
    header.set_property("Observation:Object:Name", "NGC 7000")?;
    header.set_property_with_type("Instrument:Telescope:FocalLength", "0.53", "Float32")?;

    // 4. Serialize to a self-contained container and confirm it round-trips.
    let hints = StructuralHints::default();
    let bytes = header.to_bytes(&hints);
    assert_eq!(Header::parse(&bytes)?, header);

    // 5. Round-trip through a real file on disk.
    let path = std::env::temp_dir().join("xisf-header-quickstart.xisf");
    header.write_to_file(&path, &hints)?;
    let reloaded = Header::read_from_file(&path)?;
    assert_eq!(reloaded, header);

    // 6. Read values back out, typed.
    assert_eq!(reloaded.get_str("IMAGETYP")?, Some("Master Dark"));
    assert_eq!(reloaded.get_str("EXPTIME")?, Some("300.00"));
    assert_eq!(reloaded.get_i64("GAIN")?, Some(100));
    assert_eq!(reloaded.count("HISTORY"), 2);
    assert_eq!(
        reloaded.property("Observation:Object:Name"),
        Some("NGC 7000")
    );
    assert_eq!(
        reloaded.property_get::<f64>("Instrument:Telescope:FocalLength"),
        Some(0.53)
    );

    // 7. Edit the file's header in place, then clean up.
    Header::update_file(&path, &hints, |h| {
        h.set("OBJECT", "NGC 7000").unwrap();
    })?;
    std::fs::remove_file(&path).ok();

    println!("quickstart header round-tripped successfully");
    Ok(())
}
