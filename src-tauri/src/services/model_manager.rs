use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

/// 模型文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// 模型状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub ready: bool,
    pub missing_files: Vec<String>,
    pub models_dir: String,
}

/// 下载进度
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub file: String,
    pub current: u64,
    pub total: u64,
}

/// OCR 模型配置
pub struct ModelManager {
    models_dir: PathBuf,
    cancel_flag: Arc<AtomicBool>,
}

/// 模型文件列表
const MODEL_FILES: &[ModelFile] = &[
    ModelFile {
        name: "pp-ocrv5_mobile_det.onnx".to_string(),
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_mobile_det.onnx".to_string(),
        size: 4_828_087, // ~4.6MB
    },
    ModelFile {
        name: "pp-ocrv5_mobile_rec.onnx".to_string(),
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_mobile_rec.onnx".to_string(),
        size: 16_556_181, // ~15.8MB
    },
    ModelFile {
        name: "ppocrv5_dict.txt".to_string(),
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ppocrv5_dict.txt".to_string(),
        size: 5_682, // ~5KB
    },
];

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Self {
        // 确保模型目录存在
        if !models_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&models_dir) {
                warn!("创建模型目录失败: {:?}, {}", models_dir, e);
            }
        }

        Self {
            models_dir,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 获取模型目录路径
    pub fn models_dir(&self) -> &PathBuf {
        &self.models_dir
    }

    /// 检查所有模型文件是否存在
    pub fn check_models(&self) -> ModelStatus {
        debug!("检查模型文件, 目录: {:?}", self.models_dir);

        let missing_files: Vec<String> = MODEL_FILES
            .iter()
            .filter(|model| !self.models_dir.join(&model.name).exists())
            .map(|model| model.name.clone())
            .collect();

        let ready = missing_files.is_empty();

        info!(
            "模型检查完成: ready={}, missing={:?}",
            ready, missing_files
        );

        ModelStatus {
            ready,
            missing_files,
            models_dir: self.models_dir.to_string_lossy().to_string(),
        }
    }

    /// 获取所有模型文件信息
    pub fn get_model_files() -> &'static [ModelFile] {
        MODEL_FILES
    }

    /// 获取手动下载指导
    pub fn get_download_guide() -> Vec<DownloadGuide> {
        MODEL_FILES
            .iter()
            .map(|model| DownloadGuide {
                name: model.name.clone(),
                url: model.url.clone(),
                size: format_size(model.size),
            })
            .collect()
    }

    /// 取消下载
    pub fn cancel_download(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        info!("已请求取消下载");
    }

    /// 下载所有缺失的模型文件
    pub async fn download_models(&self, app_handle: AppHandle) -> Result<(), String> {
        info!("开始下载模型文件");

        // 重置取消标志
        self.cancel_flag.store(false, Ordering::SeqCst);

        let status = self.check_models();

        if status.ready {
            info!("所有模型文件已存在，无需下载");
            let _ = app_handle.emit("model-download-complete", ());
            return Ok(());
        }

        // 创建 HTTP 客户端
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        // 下载缺失的文件
        for model in MODEL_FILES {
            // 检查是否取消
            if self.cancel_flag.load(Ordering::SeqCst) {
                warn!("下载已取消");
                return Err("下载已取消".to_string());
            }

            let file_path = self.models_dir.join(&model.name);

            // 跳过已存在的文件
            if file_path.exists() {
                info!("模型文件已存在，跳过: {}", model.name);
                continue;
            }

            info!("开始下载: {}", model.name);

            // 发送下载开始事件
            let _ = app_handle.emit(
                "model-download-progress",
                DownloadProgress {
                    file: model.name.clone(),
                    current: 0,
                    total: model.size,
                },
            );

            // 下载文件
            match self.download_file(&client, &model.url, &file_path, &app_handle).await {
                Ok(_) => {
                    info!("下载完成: {}", model.name);
                }
                Err(e) => {
                    error!("下载失败: {}, 错误: {}", model.name, e);

                    // 删除部分下载的文件
                    let _ = std::fs::remove_file(&file_path);

                    let _ = app_handle.emit(
                        "model-download-error",
                        serde_json::json!({ "error": format!("下载 {} 失败: {}", model.name, e) }),
                    );

                    return Err(format!("下载 {} 失败: {}", model.name, e));
                }
            }
        }

        // 发送下载完成事件
        let _ = app_handle.emit("model-download-complete", ());

        info!("所有模型文件下载完成");
        Ok(())
    }

    /// 下载单个文件
    async fn download_file(
        &self,
        client: &reqwest::Client,
        url: &str,
        path: &PathBuf,
        app_handle: &AppHandle,
    ) -> Result<(), String> {
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP 错误: {}", response.status()));
        }

        let total_size = response.content_length().unwrap_or(0);

        // 创建临时文件
        let temp_path = path.with_extension("tmp");
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| format!("创建文件失败: {}", e))?;

        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            // 检查是否取消
            if self.cancel_flag.load(Ordering::SeqCst) {
                let _ = std::fs::remove_file(&temp_path);
                return Err("下载已取消".to_string());
            }

            let chunk = chunk.map_err(|e| format!("读取数据失败: {}", e))?;

            use std::io::Write;
            file.write_all(&chunk)
                .map_err(|e| format!("写入文件失败: {}", e))?;

            downloaded += chunk.len() as u64;

            // 发送进度事件 (每 100KB 更新一次)
            if downloaded % 102400 < chunk.len() as u64 || downloaded == total_size {
                let _ = app_handle.emit(
                    "model-download-progress",
                    DownloadProgress {
                        file: path.file_name().unwrap().to_string_lossy().to_string(),
                        current: downloaded,
                        total: total_size,
                    },
                );
            }
        }

        // 重命名临时文件为最终文件
        std::fs::rename(&temp_path, path)
            .map_err(|e| format!("重命名文件失败: {}", e))?;

        Ok(())
    }
}

/// 手动下载指导
#[derive(Debug, Clone, Serialize)]
pub struct DownloadGuide {
    pub name: String,
    pub url: String,
    pub size: String,
}

/// 格式化文件大小
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}