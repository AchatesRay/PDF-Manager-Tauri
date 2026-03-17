use image::DynamicImage;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OcrError {
    #[error("Tesseract not found: {0}")]
    TesseractNotFound(String),
    #[error("OCR failed: {0}")]
    OcrFailed(String),
    #[error("Language pack not found: {0}")]
    LanguageNotFound(String),
}

pub struct OcrService {
    tesseract_path: String,
    language: String,
    data_path: String,
}

impl OcrService {
    pub fn new(app_dir: &Path) -> Result<Self, OcrError> {
        let tesseract_path = Self::find_tesseract(app_dir)?;
        let data_path = app_dir.join("tesseract").to_string_lossy().to_string();

        Ok(Self {
            tesseract_path,
            language: "chi_sim+eng".to_string(),
            data_path,
        })
    }

    fn find_tesseract(app_dir: &Path) -> Result<String, OcrError> {
        let local_tesseract = app_dir.join("binaries").join("tesseract.exe");
        if local_tesseract.exists() {
            return Ok(local_tesseract.to_string_lossy().to_string());
        }

        if let Ok(output) = Command::new("tesseract").arg("--version").output() {
            if output.status.success() {
                return Ok("tesseract".to_string());
            }
        }

        Err(OcrError::TesseractNotFound(
            "Tesseract not found in app directory or system PATH".to_string(),
        ))
    }

    pub fn recognize(&self, image: &DynamicImage) -> Result<String, OcrError> {
        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("ocr_input.png");
        let output_path = temp_dir.join("ocr_output");

        image.save(&input_path).map_err(|e| OcrError::OcrFailed(e.to_string()))?;

        let output = Command::new(&self.tesseract_path)
            .env("TESSDATA_PREFIX", &self.data_path)
            .arg(&input_path)
            .arg(&output_path.with_extension(""))
            .arg("-l")
            .arg(&self.language)
            .output()
            .map_err(|e| OcrError::OcrFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OcrError::OcrFailed(stderr.to_string()));
        }

        let result_path = output_path.with_extension("txt");
        let text = std::fs::read_to_string(&result_path)
            .map_err(|e| OcrError::OcrFailed(e.to_string()))?;

        std::fs::remove_file(&input_path).ok();
        std::fs::remove_file(&result_path).ok();

        Ok(text)
    }

    pub fn is_available(&self) -> bool {
        Command::new(&self.tesseract_path)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn available_languages(&self) -> Vec<String> {
        if let Ok(output) = Command::new(&self.tesseract_path)
            .arg("--list-langs")
            .env("TESSDATA_PREFIX", &self.data_path)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout
                    .lines()
                    .skip(1)
                    .map(|s| s.trim().to_string())
                    .collect();
            }
        }
        vec![]
    }
}