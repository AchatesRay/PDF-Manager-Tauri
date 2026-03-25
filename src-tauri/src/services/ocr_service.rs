use crate::services::model_manager::ModelManager;
use image::{DynamicImage, GenericImageView, imageops};
use oar_ocr::prelude::*;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[derive(Error, Debug)]
pub enum OcrError {
    #[error("模型文件缺失: {0}")]
    ModelsMissing(String),
    #[error("OCR 初始化失败: {0}")]
    InitFailed(String),
    #[error("OCR 识别失败: {0}")]
    OcrFailed(String),
    #[error("模型目录不存在")]
    ModelDirNotFound,
}

/// OCR 服务状态
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OcrStatus {
    pub available: bool,
    pub models_ready: bool,
    pub missing_files: Vec<String>,
    pub models_dir: String,
}

/// 默认最大图像尺寸
const DEFAULT_MAX_IMAGE_DIMENSION: u32 = 2000;

/// 分块处理的最大尺寸（每个分块）
const TILE_MAX_DIMENSION: u32 = 800;

pub struct OcrService {
    model_manager: Arc<ModelManager>,
    ocr: Option<OAROCR>,
}

impl OcrService {
    /// 创建 OCR 服务（延迟加载模型）
    pub fn new(data_dir: &Path) -> Result<Self, OcrError> {
        info!("初始化 OCR 服务, data_dir={:?}", data_dir);

        let models_dir = data_dir.join("models");

        // 确保模型目录存在
        if !models_dir.exists() {
            if let Err(_e) = std::fs::create_dir_all(&models_dir) {
                warn!("创建模型目录失败: {:?}", models_dir);
            }
        }

        let model_manager = Arc::new(ModelManager::new(models_dir));

        info!("OCR 服务初始化成功（模型延迟加载）");

        Ok(Self {
            model_manager,
            ocr: None,
        })
    }

    /// 获取 OCR 状态
    pub fn get_status(&self) -> OcrStatus {
        let status = self.model_manager.check_models();

        OcrStatus {
            available: self.ocr.is_some(),
            models_ready: status.ready,
            missing_files: status.missing_files,
            models_dir: status.models_dir,
        }
    }

    /// 检查模型是否可用
    pub fn is_available(&self) -> bool {
        self.ocr.is_some()
    }

    /// 检查模型文件是否存在
    pub fn check_models(&self) -> bool {
        self.model_manager.check_models().ready
    }

    /// 初始化 OCR（加载模型）
    pub fn init_ocr(&mut self) -> Result<(), OcrError> {
        if self.ocr.is_some() {
            debug!("OCR 模型已加载");
            return Ok(());
        }

        let status = self.model_manager.check_models();

        if !status.ready {
            return Err(OcrError::ModelsMissing(format!(
                "缺少模型文件: {:?}",
                status.missing_files
            )));
        }

        let models_dir = self.model_manager.models_dir();

        let det_path = models_dir.join("pp-ocrv5_mobile_det.onnx");
        let rec_path = models_dir.join("pp-ocrv5_mobile_rec.onnx");
        let dict_path = models_dir.join("ppocrv5_dict.txt");

        info!("加载 OCR 模型: det={:?}, rec={:?}, dict={:?}", det_path, rec_path, dict_path);

        let ocr = OAROCRBuilder::new(&det_path, &rec_path, &dict_path)
            .build()
            .map_err(|e| {
                error!("OCR 模型加载失败: {}", e);
                OcrError::InitFailed(format!("模型加载失败: {}", e))
            })?;

        self.ocr = Some(ocr);
        info!("OCR 模型加载成功");

        Ok(())
    }

    /// 执行 OCR 识别（使用默认尺寸限制）
    pub fn recognize(&mut self, image: &DynamicImage) -> Result<String, OcrError> {
        self.recognize_with_limit(image, DEFAULT_MAX_IMAGE_DIMENSION)
    }

    /// 执行 OCR 识别（指定最大尺寸限制）
    pub fn recognize_with_limit(&mut self, image: &DynamicImage, max_dimension: u32) -> Result<String, OcrError> {
        // 如果模型未加载，尝试加载
        if self.ocr.is_none() {
            self.init_ocr()?;
        }

        let ocr = self.ocr.as_ref().ok_or_else(|| {
            OcrError::OcrFailed("OCR 模型未初始化".to_string())
        })?;

        debug!("开始 OCR 识别, 图像大小: {}x{}, 最大尺寸限制: {}", image.width(), image.height(), max_dimension);

        let (width, height) = image.dimensions();

        // 如果图像较大，使用分块处理以减少内存峰值
        if width > TILE_MAX_DIMENSION || height > TILE_MAX_DIMENSION {
            info!("图像较大，使用分块处理: {}x{}", width, height);
            return self.recognize_with_tiling(image, max_dimension);
        }

        // 小图像直接处理
        let rgb_image = Self::resize_image_if_needed(image, max_dimension);
        debug!("处理后图像大小: {}x{}", rgb_image.width(), rgb_image.height());

        let results = ocr.predict(vec![rgb_image]).map_err(|e| {
            error!("OCR 识别失败: {}", e);
            OcrError::OcrFailed(format!("识别失败: {}", e))
        })?;

        let text = results
            .first()
            .map(|r| {
                r.text_regions
                    .iter()
                    .filter_map(|region| region.text_with_confidence())
                    .map(|(t, _)| t)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        info!("OCR 识别完成: {} 字符", text.len());
        Ok(text)
    }

    /// 分块处理大图像
    fn recognize_with_tiling(&mut self, image: &DynamicImage, max_dimension: u32) -> Result<String, OcrError> {
        let ocr = self.ocr.as_ref().ok_or_else(|| {
            OcrError::OcrFailed("OCR 模型未初始化".to_string())
        })?;

        let (width, height) = image.dimensions();

        // 先缩放到目标尺寸
        let scaled = if width > max_dimension || height > max_dimension {
            let scale = max_dimension as f64 / width.max(height) as f64;
            let new_width = (width as f64 * scale) as u32;
            let new_height = (height as f64 * scale) as u32;
            info!("缩放图像: {}x{} -> {}x{}", width, height, new_width, new_height);
            image.resize(new_width, new_height, imageops::FilterType::Lanczos3)
        } else {
            image.clone()
        };

        let (sw, sh) = scaled.dimensions();

        // 计算分块数量
        let tile_size = TILE_MAX_DIMENSION;
        let cols = ((sw + tile_size - 1) / tile_size) as usize;
        let rows = ((sh + tile_size - 1) / tile_size) as usize;

        info!("分块处理: {}x{} 图像分为 {}x{} = {} 块", sw, sh, cols, rows, cols * rows);

        let mut all_texts: Vec<String> = Vec::new();

        for row in 0..rows {
            for col in 0..cols {
                let x0 = (col as u32 * tile_size).min(sw);
                let y0 = (row as u32 * tile_size).min(sh);
                let x1 = ((col as u32 + 1) * tile_size).min(sw);
                let y1 = ((row as u32 + 1) * tile_size).min(sh);

                if x0 >= x1 || y0 >= y1 {
                    continue;
                }

                debug!("处理分块 [{},{}]: ({},{}) - ({},{})", row, col, x0, y0, x1, y1);

                // 裁剪分块
                let tile = scaled.crop(x0, y0, x1 - x0, y1 - y0);
                let rgb_tile = tile.to_rgb8();

                // 识别分块
                match ocr.predict(vec![rgb_tile]) {
                    Ok(results) => {
                        if let Some(result) = results.first() {
                            let tile_text: String = result.text_regions
                                .iter()
                                .filter_map(|region| region.text_with_confidence())
                                .map(|(t, _)| t)
                                .collect::<Vec<_>>()
                                .join("\n");

                            if !tile_text.is_empty() {
                                all_texts.push(tile_text);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("分块 [{},{}] 识别失败: {}", row, col, e);
                    }
                }
            }
        }

        let text = all_texts.join("\n\n");
        info!("分块 OCR 完成: {} 字符", text.len());
        Ok(text)
    }

    /// 获取模型管理器（用于下载等操作）
    pub fn model_manager(&self) -> Arc<ModelManager> {
        Arc::clone(&self.model_manager)
    }

    /// 兼容旧 API：获取可用语言列表
    pub fn available_languages(&self) -> Vec<String> {
        if self.ocr.is_some() {
            vec!["chi_sim".to_string(), "eng".to_string()]
        } else {
            vec![]
        }
    }

    /// 兼容旧 API：检查中文支持
    pub fn check_chinese_support(&self) -> bool {
        self.check_models()
    }
}