use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OcrStatus {
    Pending,
    Done,
    Error,
}

impl std::fmt::Display for OcrStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcrStatus::Pending => write!(f, "pending"),
            OcrStatus::Done => write!(f, "done"),
            OcrStatus::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for OcrStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(OcrStatus::Pending),
            "done" => Ok(OcrStatus::Done),
            "error" => Ok(OcrStatus::Error),
            _ => Err(format!("Unknown OCR status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfPage {
    pub id: i64,
    pub pdf_id: i64,
    pub page_number: i32,
    pub ocr_text: Option<String>,
    pub ocr_status: OcrStatus,
    pub thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPage {
    pub pdf_id: i64,
    pub page_number: i32,
}