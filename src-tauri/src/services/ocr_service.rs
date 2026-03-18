use image::DynamicImage;
use std::path::Path;
use std::process::Command;
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
    data_path: Option<String>, // None 表示使用系统默认路径
}

impl OcrService {
    pub fn new(app_dir: &Path) -> Result<Self, OcrError> {
        info!("初始化OCR服务, app_dir={:?}", app_dir);

        let tesseract_path = Self::find_tesseract(app_dir)?;

        // 查找 tessdata 目录
        let data_path = Self::find_tessdata(app_dir, &tesseract_path);

        if let Some(ref path) = data_path {
            info!("OCR服务初始化成功: tesseract={}, tessdata={}", tesseract_path, path);
        } else {
            info!("OCR服务初始化成功: tesseract={}, 使用系统默认 tessdata", tesseract_path);
        }

        Ok(Self {
            tesseract_path,
            language: "chi_sim+eng".to_string(),
            data_path,
        })
    }

    /// 查找 tessdata 目录
    fn find_tessdata(app_dir: &Path, tesseract_path: &str) -> Option<String> {
        // 1. 检查应用目录下的 tesseract/tessdata
        let app_tessdata = app_dir.join("tesseract").join("tessdata");
        if app_tessdata.exists() {
            // 检查是否有 chi_sim.traineddata
            if app_tessdata.join("chi_sim.traineddata").exists() {
                info!("找到应用目录下的 tessdata: {:?}", app_tessdata);
                return Some(app_dir.join("tesseract").to_string_lossy().to_string());
            } else {
                warn!("应用目录 tessdata 存在但缺少中文语言包: {:?}", app_tessdata);
            }
        }

        // 2. 检查 Tesseract 安装目录下的 tessdata (Windows 常见路径)
        if cfg!(target_os = "windows") {
            if let Ok(output) = Command::new(tesseract_path)
                .arg("--list-langs")
                .creation_flags(CREATE_NO_WINDOW)
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("chi_sim") {
                    info!("系统 Tesseract 包含中文语言包");
                    return None; // 使用系统默认
                } else {
                    warn!("系统 Tesseract 不包含中文语言包，可用语言: {}", stdout);
                }
            }
        }

        // 3. 检查常见的系统 tessdata 路径
        let common_paths = if cfg!(target_os = "windows") {
            vec![
                "C:\\Program Files\\Tesseract-OCR\\tessdata",
                "C:\\Program Files (x86)\\Tesseract-OCR\\tessdata",
            ]
        } else {
            vec![
                "/usr/share/tessdata",
                "/usr/local/share/tessdata",
                "/usr/share/tesseract-ocr/5/tessdata",
                "/usr/share/tesseract-ocr/4.00/tessdata",
            ]
        };

        for path in common_paths {
            let tessdata_path = Path::new(path);
            if tessdata_path.exists() && tessdata_path.join("chi_sim.traineddata").exists() {
                let parent = tessdata_path.parent().unwrap_or(tessdata_path);
                info!("找到系统 tessdata: {:?}", tessdata_path);
                return Some(parent.to_string_lossy().to_string());
            }
        }

        warn!("未找到包含中文语言包的 tessdata 目录，OCR 可能无法正确识别中文");
        None
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
        #[cfg(target_os = "windows")]
        let result = Command::new("tesseract")
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        #[cfg(not(target_os = "windows"))]
        let result = Command::new("tesseract").arg("--version").output();

        match result {
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

        debug!("执行Tesseract: path={}, lang={}, data_path={:?}",
            self.tesseract_path, self.language, self.data_path);

        #[cfg(target_os = "windows")]
        let output = {
            let mut cmd = Command::new(&self.tesseract_path);
            if let Some(ref path) = self.data_path {
                cmd.env("TESSDATA_PREFIX", path);
            }
            cmd.arg(&input_path)
                .arg(&output_path.with_extension(""))
                .arg("-l")
                .arg(&self.language)
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| {
                    error!("执行Tesseract失败: {}", e);
                    OcrError::OcrFailed(format!("执行Tesseract失败: {}", e))
                })?
        };

        #[cfg(not(target_os = "windows"))]
        let output = {
            let mut cmd = Command::new(&self.tesseract_path);
            if let Some(ref path) = self.data_path {
                cmd.env("TESSDATA_PREFIX", path);
            }
            cmd.arg(&input_path)
                .arg(&output_path.with_extension(""))
                .arg("-l")
                .arg(&self.language)
                .output()
                .map_err(|e| {
                    error!("执行Tesseract失败: {}", e);
                    OcrError::OcrFailed(format!("执行Tesseract失败: {}", e))
                })?
        };

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

        #[cfg(target_os = "windows")]
        let result = Command::new(&self.tesseract_path)
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        #[cfg(not(target_os = "windows"))]
        let result = Command::new(&self.tesseract_path)
            .arg("--version")
            .output();

        match result {
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

        #[cfg(target_os = "windows")]
        let result = {
            let mut cmd = Command::new(&self.tesseract_path);
            cmd.arg("--list-langs");
            if let Some(ref path) = self.data_path {
                cmd.env("TESSDATA_PREFIX", path);
            }
            cmd.creation_flags(CREATE_NO_WINDOW).output()
        };

        #[cfg(not(target_os = "windows"))]
        let result = {
            let mut cmd = Command::new(&self.tesseract_path);
            cmd.arg("--list-langs");
            if let Some(ref path) = self.data_path {
                cmd.env("TESSDATA_PREFIX", path);
            }
            cmd.output()
        };

        match result {
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

    /// 检查中文语言包是否可用
    pub fn check_chinese_support(&self) -> bool {
        let langs = self.available_languages();
        let has_chinese = langs.iter().any(|l| l == "chi_sim" || l == "chi_tra");
        if has_chinese {
            info!("中文语言包已安装");
        } else {
            warn!("中文语言包未安装！可用语言: {:?}", langs);
            warn!("请安装中文语言包: 下载 chi_sim.traineddata 并放入 tessdata 目录");
        }
        has_chinese
    }
}