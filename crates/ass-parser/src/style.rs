use super::color::AssColor;

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub name: String,
    pub font_name: String,
    pub font_size: f64,
    pub primary_color: AssColor,
    pub secondary_color: AssColor,
    pub outline_color: AssColor,
    pub shadow_color: AssColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub scale_x: f64,
    pub scale_y: f64,
    pub spacing: f64,
    pub angle: f64,
    pub border_style: u8,
    pub outline_width: f64,
    pub shadow_depth: f64,
    pub alignment: u8,
    pub margin_l: u32,
    pub margin_r: u32,
    pub margin_v: u32,
    pub encoding: u8,
    pub relative_to: u8,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            font_name: "Arial".to_string(),
            font_size: 20.0,
            primary_color: AssColor::WHITE,
            secondary_color: AssColor::WHITE,
            outline_color: AssColor::BLACK,
            shadow_color: AssColor::BLACK,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            scale_x: 100.0,
            scale_y: 100.0,
            spacing: 0.0,
            angle: 0.0,
            border_style: 1,
            outline_width: 2.0,
            shadow_depth: 2.0,
            alignment: 2,
            margin_l: 10,
            margin_r: 10,
            margin_v: 10,
            encoding: 1,
            relative_to: 0,
        }
    }
}

impl Style {
    pub fn parse_from_line(line: &str) -> Result<Self, super::error::ParseError> {
        let fields: Vec<&str> = line.splitn(24, ',').collect();
        if fields.len() < 23 {
            return Err(super::error::ParseError::InvalidStyle(format!(
                "expected 23 fields, got {}", fields.len()
            )));
        }
        Ok(Self {
            name: fields[0].trim().to_string(),
            font_name: fields[1].trim().to_string(),
            font_size: fields[2].trim().parse().unwrap_or(20.0),
            primary_color: AssColor::from_ass_hex(fields[3].trim()).unwrap_or(AssColor::WHITE),
            secondary_color: AssColor::from_ass_hex(fields[4].trim()).unwrap_or(AssColor::WHITE),
            outline_color: AssColor::from_ass_hex(fields[5].trim()).unwrap_or(AssColor::BLACK),
            shadow_color: AssColor::from_ass_hex(fields[6].trim()).unwrap_or(AssColor::BLACK),
            bold: fields[7].trim() == "-1" || fields[7].trim() == "1",
            italic: fields[8].trim() == "-1" || fields[8].trim() == "1",
            underline: fields[9].trim() == "-1" || fields[9].trim() == "1",
            strikeout: fields[10].trim() == "-1" || fields[10].trim() == "1",
            scale_x: fields[11].trim().parse().unwrap_or(100.0),
            scale_y: fields[12].trim().parse().unwrap_or(100.0),
            spacing: fields[13].trim().parse().unwrap_or(0.0),
            angle: fields[14].trim().parse().unwrap_or(0.0),
            border_style: fields[15].trim().parse().unwrap_or(1),
            outline_width: fields[16].trim().parse().unwrap_or(2.0),
            shadow_depth: fields[17].trim().parse().unwrap_or(2.0),
            alignment: fields[18].trim().parse().unwrap_or(2),
            margin_l: fields[19].trim().parse().unwrap_or(10),
            margin_r: fields[20].trim().parse().unwrap_or(10),
            margin_v: fields[21].trim().parse().unwrap_or(10),
            encoding: fields[22].trim().parse().unwrap_or(1),
            relative_to: if fields.len() > 23 { fields[23].trim().parse().unwrap_or(0) } else { 0 },
        })
    }
}
