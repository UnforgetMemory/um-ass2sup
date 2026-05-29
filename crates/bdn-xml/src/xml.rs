use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::error::BdnError;
use crate::types::BdnXml;

pub fn generate_xml(bdn: &BdnXml) -> Result<String, BdnError> {
    let mut buf = Vec::new();
    let mut writer = Writer::new(&mut buf);

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
        .map_err(|e| BdnError::Xml(e.to_string()))?;

    write_element(&mut writer, "BDN", None, Some(&[("Version", bdn.version.as_str())]))?;
    write_element(&mut writer, "Description", None, None)?;
    write_text_element(&mut writer, "Name", &bdn.name)?;
    write_text_element(&mut writer, "Language", "eng")?;
    write_element(&mut writer, "Format", None, Some(&[("VideoFormat", bdn.format.as_str())]))?;
    write_text_element(&mut writer, "Content", "")?;

    write_element(&mut writer, "Events", None, None)?;

    for event in &bdn.events {
        let forced_str = if event.forced { "true" } else { "false" };
        let area_attr = format!(
            "{},{},{},{}",
            event.x, event.y, event.width, event.height
        );

        write_element(&mut writer, "Event", None, Some(&[("InTC", event.in_tc.as_str()), ("OutTC", event.out_tc.as_str()), ("Forced", forced_str)]))?;
        write_element(
            &mut writer,
            "Graphic",
            None,
            Some(&[("File", event.graphic.as_str()), ("Area", area_attr.as_str())]),
        )?;
        writer
            .write_event(Event::End(BytesEnd::new("Event")))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("Events")))
        .map_err(|e| BdnError::Xml(e.to_string()))?;
    writer
        .write_event(Event::End(BytesEnd::new("Description")))
        .map_err(|e| BdnError::Xml(e.to_string()))?;
    writer
        .write_event(Event::End(BytesEnd::new("BDN")))
        .map_err(|e| BdnError::Xml(e.to_string()))?;

    String::from_utf8(buf).map_err(|e| BdnError::Xml(e.to_string()))
}

fn write_element(
    writer: &mut Writer<&mut Vec<u8>>,
    name: &str,
    text: Option<&str>,
    attrs: Option<&[(&str, &str)]>,
) -> Result<(), BdnError> {
    let mut elem = BytesStart::new(name);
    if let Some(attrs) = attrs {
        for (k, v) in attrs {
            elem.push_attribute((*k, *v));
        }
    }

    if let Some(text) = text {
        writer
            .write_event(Event::Start(elem))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
        writer
            .write_event(Event::Text(BytesText::new(text)))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
        writer
            .write_event(Event::End(BytesEnd::new(name)))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
    } else {
        writer
            .write_event(Event::Start(elem))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
    }

    Ok(())
}

fn write_text_element(
    writer: &mut Writer<&mut Vec<u8>>,
    name: &str,
    text: &str,
) -> Result<(), BdnError> {
    write_element(writer, name, Some(text), None)
}

pub fn ms_to_timecode(ms: u64, fps: f64) -> String {
    let total_frames = (ms as f64 * fps / 1000.0).round() as u64;
    let frames = total_frames % fps as u64;
    let total_secs = total_frames / fps as u64;
    let secs = total_secs % 60;
    let total_mins = total_secs / 60;
    let mins = total_mins % 60;
    let hours = total_mins / 60;
    format!("{:02}:{:02}:{:02}:{:02}", hours, mins, secs, frames)
}

pub fn generate_png(palette: &[[u8; 4]], indices: &[u8], width: u32, height: u32) -> Result<Vec<u8>, BdnError> {
    use png::Encoder;

    let mut buf = Vec::new();
    {
        let mut encoder = Encoder::new(&mut buf, width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);

        let mut plte = Vec::with_capacity(palette.len() * 3);
        let mut trns = Vec::with_capacity(palette.len());
        for color in palette {
            plte.extend_from_slice(&color[0..3]);
            trns.push(color[3]);
        }

        encoder.set_palette(&plte);
        encoder.set_trns(&trns);

        let mut writer = encoder.write_header().map_err(|e| BdnError::Png(e.to_string()))?;
        writer
            .write_image_data(indices)
            .map_err(|e| BdnError::Png(e.to_string()))?;
    }

    Ok(buf)
}

pub fn save_frame_png(
    path: &std::path::Path,
    palette: &[[u8; 4]],
    indices: &[u8],
    width: u32,
    height: u32,
) -> Result<(), BdnError> {
    let data = generate_png(palette, indices, width, height)?;
    std::fs::write(path, data)?;
    Ok(())
}
