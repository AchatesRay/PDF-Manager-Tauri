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