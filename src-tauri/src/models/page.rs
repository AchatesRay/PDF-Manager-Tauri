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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_status_display() {
        assert_eq!(OcrStatus::Pending.to_string(), "pending");
        assert_eq!(OcrStatus::Done.to_string(), "done");
        assert_eq!(OcrStatus::Error.to_string(), "error");
    }

    #[test]
    fn test_ocr_status_from_str() {
        assert_eq!("pending".parse::<OcrStatus>(), Ok(OcrStatus::Pending));
        assert_eq!("done".parse::<OcrStatus>(), Ok(OcrStatus::Done));
        assert_eq!("error".parse::<OcrStatus>(), Ok(OcrStatus::Error));
        assert!("invalid".parse::<OcrStatus>().is_err());
    }

    #[test]
    fn test_ocr_status_serialization() {
        let status = OcrStatus::Done;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"done\"");

        let deserialized: OcrStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, OcrStatus::Done);
    }

    #[test]
    fn test_pdf_page_serialization() {
        let page = PdfPage {
            id: 1,
            pdf_id: 100,
            page_number: 5,
            ocr_text: Some("测试文本".to_string()),
            ocr_status: OcrStatus::Done,
            thumbnail_path: None,
        };

        let json = serde_json::to_string(&page).unwrap();
        let deserialized: PdfPage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.pdf_id, 100);
        assert_eq!(deserialized.page_number, 5);
        assert_eq!(deserialized.ocr_text, Some("测试文本".to_string()));
        assert_eq!(deserialized.ocr_status, OcrStatus::Done);
    }
}