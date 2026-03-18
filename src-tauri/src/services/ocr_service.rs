use image::DynamicImage;
use std::path::Path;
use std::process::Command;
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[derive(Error, Debug)]
pub enum OcrError {
    #[error("Tesseract未找到: {0}")]
    TesseractNotFound(String),
    #[error("OCR识别失败: {0}")]
    OcrFailed(String),
    #[error("语言包未找到: {0}")]
    LanguageNotFound(String),
}

pub struct OcrService {
    tesseract_path: String,
    language: String,
    data_path: String,
}

impl OcrService {
    pub fn new(app_dir: &Path) -> Result<Self, OcrError> {
        info!("初始化OCR服务, app_dir={:?}", app_dir);

        let tesseract_path = Self::find_tesseract(app_dir)?;
        let data_path = app_dir.join("tesseract").to_string_lossy().to_string();

        info!("OCR服务初始化成功: tesseract={}, data_path={}", tesseract_path, data_path);

        Ok(Self {
            tesseract_path,
            language: "chi_sim+eng".to_string(),
            data_path,
        })
    }

    fn find_tesseract(app_dir: &Path) -> Result<String, OcrError> {
        debug!("搜索Tesseract...");

        // 先检查应用目录
        let local_tesseract = app_dir.join("binaries").join("tesseract.exe");
        if local_tesseract.exists() {
            info!("在应用目录找到Tesseract: {:?}", local_tesseract);
            return Ok(local_tesseract.to_string_lossy().to_string());
        }
        debug!("应用目录中未找到Tesseract: {:?}", local_tesseract);

        // 检查系统PATH
        match Command::new("tesseract").arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    info!("在系统PATH找到Tesseract: {}", version.lines().next().unwrap_or("unknown version"));
                    return Ok("tesseract".to_string());
                } else {
                    warn!("系统Tesseract版本检查失败");
                }
            }
            Err(e) => {
                debug!("系统PATH中未找到Tesseract: {}", e);
            }
        }

        error!("Tesseract未找到: 应用目录和系统PATH中都不存在");
        Err(OcrError::TesseractNotFound(
            "Tesseract未找到，请安装Tesseract或将其放入应用目录".to_string(),
        ))
    }

    pub fn recognize(&self, image: &DynamicImage) -> Result<String, OcrError> {
        debug!("开始OCR识别, 图像大小: {}x{}", image.width(), image.height());

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("ocr_input.png");
        let output_path = temp_dir.join("ocr_output");

        // 保存临时图像
        match image.save(&input_path) {
            Ok(_) => debug!("临时图像已保存: {:?}", input_path),
            Err(e) => {
                error!("保存临时图像失败: {:?}", input_path);
                return Err(OcrError::OcrFailed(format!("保存图像失败: {}", e)));
            }
        }

        debug!("执行Tesseract: path={}, lang={}, data_path={}",
            self.tesseract_path, self.language, self.data_path);

        let output = Command::new(&self.tesseract_path)
            .env("TESSDATA_PREFIX", &self.data_path)
            .arg(&input_path)
            .arg(&output_path.with_extension(""))
            .arg("-l")
            .arg(&self.language)
            .output()
            .map_err(|e| {
                error!("执行Tesseract失败: {}", e);
                OcrError::OcrFailed(format!("执行Tesseract失败: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            error!("Tesseract执行失败: stderr={}, stdout={}", stderr, stdout);

            // 清理临时文件
            let _ = std::fs::remove_file(&input_path);

            return Err(OcrError::OcrFailed(format!("Tesseract错误: {}", stderr)));
        }

        let result_path = output_path.with_extension("txt");
        let text = match std::fs::read_to_string(&result_path) {
            Ok(t) => t,
            Err(e) => {
                error!("读取OCR结果失败: {:?}, 错误: {}", result_path, e);
                let _ = std::fs::remove_file(&input_path);
                return Err(OcrError::OcrFailed(format!("读取结果失败: {}", e)));
            }
        };

        // 清理临时文件
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&result_path);

        info!("OCR识别成功: {} 字符", text.len());
        Ok(text)
    }

    pub fn is_available(&self) -> bool {
        debug!("检查OCR服务可用性...");

        match Command::new(&self.tesseract_path)
            .arg("--version")
            .output()
        {
            Ok(output) => {
                let available = output.status.success();
                if available {
                    debug!("OCR服务可用");
                } else {
                    warn!("OCR服务不可用: Tesseract版本检查失败");
                }
                available
            }
            Err(e) => {
                warn!("OCR服务不可用: {}", e);
                false
            }
        }
    }

    pub fn available_languages(&self) -> Vec<String> {
        debug!("获取可用语言列表...");

        match Command::new(&self.tesseract_path)
            .arg("--list-langs")
            .env("TESSDATA_PREFIX", &self.data_path)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let langs: Vec<String> = stdout
                        .lines()
                        .skip(1)
                        .map(|s| s.trim().to_string())
                        .collect();
                    debug!("可用语言: {:?}", langs);
                    langs
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!("获取语言列表失败: {}", stderr);
                    vec![]
                }
            }
            Err(e) => {
                warn!("获取语言列表失败: {}", e);
                vec![]
            }
        }
    }
}