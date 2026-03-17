use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PdfType {
    Text,
    Scanned,
    Mixed,
}

impl std::fmt::Display for PdfType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfType::Text => write!(f, "text"),
            PdfType::Scanned => write!(f, "scanned"),
            PdfType::Mixed => write!(f, "mixed"),
        }
    }
}

impl std::str::FromStr for PdfType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text" => Ok(PdfType::Text),
            "scanned" => Ok(PdfType::Scanned),
            "mixed" => Ok(PdfType::Mixed),
            _ => Err(format!("Unknown PDF type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PdfStatus {
    Pending,
    Processing,
    Done,
    Error,
}

impl std::fmt::Display for PdfStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfStatus::Pending => write!(f, "pending"),
            PdfStatus::Processing => write!(f, "processing"),
            PdfStatus::Done => write!(f, "done"),
            PdfStatus::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for PdfStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(PdfStatus::Pending),
            "processing" => Ok(PdfStatus::Processing),
            "done" => Ok(PdfStatus::Done),
            "error" => Ok(PdfStatus::Error),
            _ => Err(format!("Unknown PDF status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pdf {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub filename: String,
    pub original_path: Option<String>,
    pub storage_path: String,
    pub file_size: i64,
    pub page_count: i32,
    pub pdf_type: PdfType,
    pub status: PdfStatus,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPdf {
    pub folder_id: Option<i64>,
    pub filename: String,
    pub original_path: Option<String>,
    pub storage_path: String,
    pub file_size: i64,
    pub page_count: i32,
    pub pdf_type: PdfType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfInfo {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub filename: String,
    pub page_count: i32,
    pub pdf_type: PdfType,
    pub status: PdfStatus,
    pub progress: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_type_display() {
        assert_eq!(PdfType::Text.to_string(), "text");
        assert_eq!(PdfType::Scanned.to_string(), "scanned");
        assert_eq!(PdfType::Mixed.to_string(), "mixed");
    }

    #[test]
    fn test_pdf_type_from_str() {
        assert_eq!("text".parse::<PdfType>(), Ok(PdfType::Text));
        assert_eq!("scanned".parse::<PdfType>(), Ok(PdfType::Scanned));
        assert_eq!("mixed".parse::<PdfType>(), Ok(PdfType::Mixed));
        assert!("invalid".parse::<PdfType>().is_err());
    }

    #[test]
    fn test_pdf_status_display() {
        assert_eq!(PdfStatus::Pending.to_string(), "pending");
        assert_eq!(PdfStatus::Processing.to_string(), "processing");
        assert_eq!(PdfStatus::Done.to_string(), "done");
        assert_eq!(PdfStatus::Error.to_string(), "error");
    }

    #[test]
    fn test_pdf_status_from_str() {
        assert_eq!("pending".parse::<PdfStatus>(), Ok(PdfStatus::Pending));
        assert_eq!("processing".parse::<PdfStatus>(), Ok(PdfStatus::Processing));
        assert_eq!("done".parse::<PdfStatus>(), Ok(PdfStatus::Done));
        assert_eq!("error".parse::<PdfStatus>(), Ok(PdfStatus::Error));
        assert!("invalid".parse::<PdfStatus>().is_err());
    }

    #[test]
    fn test_pdf_type_serialization() {
        let pdf_type = PdfType::Scanned;
        let json = serde_json::to_string(&pdf_type).unwrap();
        assert_eq!(json, "\"scanned\"");

        let deserialized: PdfType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PdfType::Scanned);
    }

    #[test]
    fn test_pdf_status_serialization() {
        let status = PdfStatus::Done;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"done\"");

        let deserialized: PdfStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PdfStatus::Done);
    }

    #[test]
    fn test_pdf_info_serialization() {
        let info = PdfInfo {
            id: 1,
            folder_id: Some(2),
            filename: "test.pdf".to_string(),
            page_count: 10,
            pdf_type: PdfType::Text,
            status: PdfStatus::Done,
            progress: Some(1.0),
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: PdfInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.filename, "test.pdf");
        assert_eq!(deserialized.page_count, 10);
        assert_eq!(deserialized.pdf_type, PdfType::Text);
        assert_eq!(deserialized.status, PdfStatus::Done);
    }
}