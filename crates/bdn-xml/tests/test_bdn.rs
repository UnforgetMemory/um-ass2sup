use bdn_xml::{BdnEvent, BdnXml, QuantizedFrame, generate_png, generate_xml, ms_to_timecode};

#[test]
fn test_bdn_xml_new_1080p() {
    let bdn = BdnXml::new("Test", 1920, 1080);
    assert_eq!(bdn.version, "0.93");
    assert_eq!(bdn.name, "Test");
    assert_eq!(bdn.display_width, 1920);
    assert_eq!(bdn.display_height, 1080);
    assert_eq!(bdn.format, "NTSC");
    assert_eq!(bdn.frame_rate, "24");
    assert!(bdn.events.is_empty());
}

#[test]
fn test_bdn_xml_new_pal() {
    let bdn = BdnXml::new("PAL", 720, 576);
    assert_eq!(bdn.format, "PAL");
}

#[test]
fn test_bdn_xml_new_ntsc() {
    let bdn = BdnXml::new("NTSC", 1920, 1080);
    assert_eq!(bdn.format, "NTSC");
}

#[test]
fn test_bdn_xml_add_event() {
    let mut bdn = BdnXml::new("Test", 1920, 1080);
    bdn.add_event(BdnEvent {
        index: 0,
        in_tc: "00:00:01:00".into(),
        out_tc: "00:00:05:00".into(),
        graphic: "0000.png".into(),
        x: 0,
        y: 900,
        width: 1920,
        height: 180,
        forced: false,
    });
    assert_eq!(bdn.events.len(), 1);
    assert_eq!(bdn.events[0].graphic, "0000.png");
}

#[test]
fn test_bdn_xml_forced_event() {
    let mut bdn = BdnXml::new("Test", 1920, 1080);
    bdn.add_event(BdnEvent {
        index: 0,
        in_tc: "00:00:01:00".into(),
        out_tc: "00:00:05:00".into(),
        graphic: "0000.png".into(),
        x: 0,
        y: 900,
        width: 1920,
        height: 180,
        forced: true,
    });
    assert!(bdn.events[0].forced);
}

#[test]
fn test_timecode_zero() {
    assert_eq!(ms_to_timecode(0, 24.0), "00:00:00:00");
}

#[test]
fn test_timecode_one_second() {
    assert_eq!(ms_to_timecode(1000, 24.0), "00:00:01:00");
}

#[test]
fn test_timecode_one_frame() {
    assert_eq!(ms_to_timecode(42, 24.0), "00:00:00:01");
}

#[test]
fn test_timecode_complex() {
    let ms = (1 * 3600 + 23 * 60 + 45) * 1000 + 500;
    let tc = ms_to_timecode(ms, 24.0);
    assert!(tc.starts_with("01:23:45:"));
}

#[test]
fn test_generate_xml_empty() {
    let bdn = BdnXml::new("Test", 1920, 1080);
    let xml = generate_xml(&bdn).unwrap();
    assert!(xml.contains("<?xml"));
    assert!(xml.contains("BDN"));
    assert!(xml.contains("Version=\"0.93\""));
    assert!(xml.contains("<Name>Test</Name>"));
    assert!(xml.contains("VideoFormat=\"NTSC\""));
    assert!(xml.contains("<Events>"));
}

#[test]
fn test_generate_xml_with_events() {
    let mut bdn = BdnXml::new("Test", 1920, 1080);
    bdn.add_event(BdnEvent {
        index: 0,
        in_tc: "00:00:01:00".into(),
        out_tc: "00:00:05:00".into(),
        graphic: "0000.png".into(),
        x: 100,
        y: 200,
        width: 300,
        height: 50,
        forced: false,
    });
    let xml = generate_xml(&bdn).unwrap();
    assert!(xml.contains("InTC=\"00:00:01:00\""));
    assert!(xml.contains("OutTC=\"00:00:05:00\""));
    assert!(xml.contains("File=\"0000.png\""));
    assert!(xml.contains("Area=\"100,200,300,50\""));
    assert!(xml.contains("Forced=\"false\""));
}

#[test]
fn test_generate_xml_forced_event() {
    let mut bdn = BdnXml::new("Test", 1920, 1080);
    bdn.add_event(BdnEvent {
        index: 0,
        in_tc: "00:00:01:00".into(),
        out_tc: "00:00:05:00".into(),
        graphic: "0000.png".into(),
        x: 0,
        y: 0,
        width: 100,
        height: 50,
        forced: true,
    });
    let xml = generate_xml(&bdn).unwrap();
    assert!(xml.contains("Forced=\"true\""));
}

#[test]
fn test_generate_png_basic() {
    let palette = vec![[0u8, 0, 0, 0], [255u8, 255, 255, 255]];
    let indices = vec![1u8; 4];
    let png = generate_png(&palette, &indices, 2, 2).unwrap();
    assert!(!png.is_empty());
    assert_eq!(&png[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
}

#[test]
fn test_generate_png_single_pixel() {
    let palette = vec![[128u8, 64, 32, 255]];
    let indices = vec![0u8];
    let png = generate_png(&palette, &indices, 1, 1).unwrap();
    assert!(!png.is_empty());
}

#[test]
fn test_quantized_frame_clone() {
    let frame = QuantizedFrame {
        width: 100,
        height: 50,
        palette: vec![[0, 0, 0, 0]],
        indices: vec![0; 5000],
        transparent_index: 0,
        pts_ms: 1000,
        duration_ms: 4000,
    };
    let cloned = frame.clone();
    assert_eq!(cloned.width, 100);
    assert_eq!(cloned.pts_ms, 1000);
}
