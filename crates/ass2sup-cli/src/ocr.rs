use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct OcrResult {
    pub texts: Vec<OcrText>,
}

#[derive(Debug, Clone)]
pub struct OcrText {
    pub text: String,
    pub confidence: f32,
}

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("Python/paddleocr not found: {0}")]
    NotFound(String),
    #[error("OCR process exited with error: {0}")]
    ProcessError(String),
    #[error("Failed to parse OCR output: {0}")]
    ParseError(String),
}

pub fn run_ocr(png_path: &Path) -> Result<OcrResult, OcrError> {
    let harness = std::env::var("OCR_HARNESS")
        .unwrap_or_else(|_| "python3 scripts/ocr_harness.py".to_string());

    let mut parts = harness.split_whitespace();
    let program = parts.next().unwrap_or("python3");
    let mut cmd = Command::new(program);
    for arg in parts {
        cmd.arg(arg);
    }
    cmd.arg(png_path);

    let output = cmd
        .output()
        .map_err(|e| OcrError::NotFound(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OcrError::ProcessError(stderr.to_string()));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    parse_ocr_json(&json_str)
}

pub fn parse_ocr_json(json_str: &str) -> Result<OcrResult, OcrError> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| OcrError::ParseError(e.to_string()))?;

    let mut texts = Vec::new();

    if let Some(arr) = value.as_array() {
        for item in arr {
            if let Some(arr) = item.as_array() {
                if arr.len() >= 3 {
                    let text = arr[1].as_str().unwrap_or("").to_string();
                    let confidence = arr[2].as_f64().unwrap_or(1.0) as f32;
                    if !text.is_empty() {
                        texts.push(OcrText { text, confidence });
                    }
                }
            }
        }
    }

    Ok(OcrResult { texts })
}

pub fn extract_text(ocr: &OcrResult) -> String {
    ocr.texts
        .iter()
        .map(|t| t.text.clone())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn strip_ass_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            // Handle ASS override tag escapes
            match chars.next() {
                Some('N') => {} // soft line break: skip (lines join in plain text)
                Some('h') => result.push('\u{00A0}'), // non-breaking space
                Some('s') => result.push(' '), // non-breaking space shortcut
                Some('\\') => result.push('\\'),
                Some(c2) => {
                    // Unknown escape: keep original
                    result.push('\\');
                    result.push(c2);
                }
                None => result.push('\\'), // trailing backslash
            }
        } else if c == '{' {
            // Skip until matching '}', handling nested braces
            let mut depth = 1;
            while let Some(&nc) = chars.peek() {
                chars.next();
                if nc == '{' {
                    depth += 1;
                } else if nc == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result.trim().to_string()
}

pub fn normalized_similarity(a: &str, b: &str) -> f64 {
    let a_norm = a.to_lowercase().replace(' ', "");
    let b_norm = b.to_lowercase().replace(' ', "");
    if a_norm.is_empty() && b_norm.is_empty() {
        return 1.0;
    }
    if a_norm.is_empty() || b_norm.is_empty() {
        return 0.0;
    }
    let max_len = a_norm.len().max(b_norm.len()) as f64;
    let dist = strsim::levenshtein(&a_norm, &b_norm) as f64;
    1.0 - (dist / max_len)
}

pub fn is_match(ocr_text: &str, ass_text: &str, threshold: f64) -> bool {
    let cleaned_ocr = strip_ass_tags(ocr_text);
    let cleaned_ass = strip_ass_tags(ass_text);
    let sim = normalized_similarity(&cleaned_ocr, &cleaned_ass);
    sim >= threshold
}
