use crate::services::model_manager::{ModelManager, ModelType};
use crate::services::text_postprocess::{detect_text_language, optimize_by_language};
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

/// 默认最大图像尺寸 - 优化：2000 -> 3000，提升小字体识别率
const DEFAULT_MAX_IMAGE_DIMENSION: u32 = 3000;

/// 分块处理的最大尺寸（每个分块）- 优化：800 -> 1200，减少文字截断
const TILE_MAX_DIMENSION: u32 = 1200;

/// 最小分块尺寸（用于内存不足时降级处理）- 优化：400 -> 600
const MIN_TILE_DIMENSION: u32 = 600;

/// 内存不足时的最大重试次数
const MAX_MEMORY_RETRIES: u32 = 3;

/// 最低置信度阈值（0.0-1.0）- 优化：0.5 -> 0.35
/// PP-OCRv5 Mobile 模型置信度偏低，降低阈值以保留更多有效识别结果
const MIN_CONFIDENCE: f32 = 0.35;

/// 分块重叠比例 - 优化：避免文字被分块边界截断
const OVERLAP_RATIO: f32 = 0.25;

/// 倾斜检测阈值 - 超过此角度才进行校正（度）
const SKEW_THRESHOLD_DEGREES: f32 = 0.5;

/// 检测文档倾斜角度
/// 返回角度（度），正数表示顺时针倾斜
/// 使用简化投影法检测（适合小幅倾斜的文档）
fn detect_skew_angle(image: &image::GrayImage) -> f32 {
    use imageproc::edges::canny;

    // Canny 边缘检测
    let edges = canny(image, 50.0, 150.0);

    // 使用水平投影法检测倾斜（±5度范围）
    let max_angle = 5.0_f32;
    let angle_step = 0.5_f32;

    let mut best_angle = 0.0_f32;
    let mut best_score = 0.0_f32;

    let mut angle = -max_angle;
    while angle <= max_angle {
        let score = calculate_horizontal_projection(&edges, angle);

        if score > best_score {
            best_score = score;
            best_angle = angle;
        }

        angle += angle_step;
    }

    best_angle
}

/// 计算水平投影分数（用于检测最佳倾斜角度）
fn calculate_horizontal_projection(edges: &image::GrayImage, angle: f32) -> f32 {
    use std::f32::consts::PI;

    let (width, height) = edges.dimensions();
    let radians = angle * PI / 180.0;
    let cos_a = radians.cos();
    let sin_a = radians.sin();

    let mut projection: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();

    for y in 0..height {
        for x in 0..width {
            if edges.get_pixel(x, y)[0] > 128 {
                // 边缘点 - 计算旋转后的 y 坐标
                let rotated_y = y as f32 * cos_a - x as f32 * sin_a;
                let bucket = rotated_y.round() as i32;
                *projection.entry(bucket).or_insert(0) += 1;
            }
        }
    }

    // 返回投影的方差（越大的方差表示越明显的水平线）
    if projection.is_empty() {
        return 0.0;
    }

    let values: Vec<i32> = projection.values().copied().collect();
    let mean = values.iter().sum::<i32>() as f32 / values.len() as f32;
    let variance = values
        .iter()
        .map(|&v| (v as f32 - mean).powi(2))
        .sum::<f32>()
        / values.len() as f32;

    variance
}

/// Sauvola 局部阈值算法（适合文档 OCR）
/// window_size: 邻域窗口大小（奇数）
/// k: 控制阈值敏感度（通常 0.2-0.5）
/// 优化参数：window=25（更大窗口适应不均匀光照），k=0.3（提高对比度敏感度）
fn sauvola_threshold(image: &image::GrayImage, window_size: u32, k: f32) -> image::GrayImage {
    let (width, height) = image.dimensions();
    let half_window = (window_size / 2) as i32;
    let mut result = image::GrayImage::new(width, height);

    const MAX_STD: f32 = 128.0; // 灰度标准差最大值

    for y in 0..height {
        for x in 0..width {
            // 计算局部均值和标准差
            let mut sum: u32 = 0;
            let mut sum_sq: u32 = 0;
            let mut count: u32 = 0;

            for dy in -half_window..=half_window {
                for dx in -half_window..=half_window {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let pixel = image.get_pixel(nx as u32, ny as u32)[0] as u32;
                        sum += pixel;
                        sum_sq += pixel * pixel;
                        count += 1;
                    }
                }
            }

            if count == 0 {
                result.put_pixel(x, y, image::Luma([255]));
                continue;
            }

            let mean = sum as f32 / count as f32;
            let variance = (sum_sq as f32 / count as f32) - (mean * mean);
            let std = variance.sqrt().max(0.0);

            // Sauvola 公式: T = mean * (1 + k * ((std / R) - 1))
            let threshold = mean * (1.0 + k * ((std / MAX_STD) - 1.0));

            let current_pixel = image.get_pixel(x, y)[0] as f32;
            let binary_value = if current_pixel <= threshold { 0 } else { 255 };
            result.put_pixel(x, y, image::Luma([binary_value]));
        }
    }

    result
}

/// 预处理图像以提高 OCR 识别正确率
///
/// 注意：当前简化处理，直接返回原图
/// 因为 Sauvola 二值化可能导致 OCR 模型识别失败
/// 如需启用预处理，请确保测试验证效果
fn preprocess_image(image: &DynamicImage) -> DynamicImage {
    // 直接返回原图，避免二值化处理破坏识别效果
    // 后续可以根据需要添加轻度的对比度增强
    image.clone()
}

pub struct OcrService {
    model_manager: Arc<ModelManager>,
    ocr: Option<OAROCR>,
    model_type: ModelType,
}

impl OcrService {
    /// 创建 OCR 服务（延迟加载模型）
    pub fn new(data_dir: &Path) -> Result<Self, OcrError> {
        // 默认使用 Mobile 模型，适合低配电脑（内存 ~200MB）
        Self::with_model_type(data_dir, ModelType::Mobile)
    }

    /// 创建 OCR 服务（指定模型类型）
    pub fn with_model_type(data_dir: &Path, model_type: ModelType) -> Result<Self, OcrError> {
        info!("初始化 OCR 服务, data_dir={:?}, model_type={}", data_dir, model_type);

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
            model_type,
        })
    }

    /// 设置模型类型（需要重新加载模型）
    pub fn set_model_type(&mut self, model_type: ModelType) {
        if self.model_type != model_type {
            info!("切换模型类型: {} -> {}", self.model_type, model_type);
            // 卸载当前模型
            self.unload_ocr();
            self.model_type = model_type;
        }
    }

    /// 获取当前模型类型
    pub fn model_type(&self) -> ModelType {
        self.model_type
    }

    /// 获取 OCR 状态
    pub fn get_status(&self) -> OcrStatus {
        let status = self.model_manager.check_models(self.model_type);

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
        self.model_manager.check_models(self.model_type).ready
    }

    /// 初始化 OCR（加载模型）
    pub fn init_ocr(&mut self) -> Result<(), OcrError> {
        if self.ocr.is_some() {
            debug!("OCR 模型已加载");
            return Ok(());
        }

        let status = self.model_manager.check_models(self.model_type);

        if !status.ready {
            return Err(OcrError::ModelsMissing(format!(
                "缺少模型文件: {:?}",
                status.missing_files
            )));
        }

        let models_dir = self.model_manager.models_dir();

        let (det_name, rec_name, dict_name) = match self.model_type {
            ModelType::Mobile => ("pp-ocrv5_mobile_det.onnx", "pp-ocrv5_mobile_rec.onnx", "ppocrv5_dict.txt"),
            ModelType::Server => ("pp-ocrv5_server_det.onnx", "pp-ocrv5_server_rec.onnx", "ppocrv5_dict.txt"),
            ModelType::Lite => ("pp-ocrv4_mobile_det.onnx", "pp-ocrv4_mobile_rec.onnx", "ppocr_keys_v1.txt"),
            ModelType::Balanced => ("pp-ocrv5_mobile_det.onnx", "ch_repsvtr_rec.onnx", "ppocr_keys_v1.txt"),
        };

        let det_path = models_dir.join(det_name);
        let rec_path = models_dir.join(rec_name);
        let dict_path = models_dir.join(dict_name);

        info!("加载 OCR 模型: type={}, det={:?}, rec={:?}, dict={:?}",
            self.model_type, det_path, rec_path, dict_path);

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

        // 小图像：应用预处理以提高识别正确率
        let preprocessed = preprocess_image(image);
        debug!("图像预处理完成");

        // 小图像直接处理，带有内存不足重试
        let mut current_tile_size = TILE_MAX_DIMENSION;
        let mut retry_count = 0;

        loop {
            // 如果需要缩小处理
            let process_image = if current_tile_size < width.max(height) {
                let scale = current_tile_size as f64 / width.max(height) as f64;
                let new_width = (width as f64 * scale) as u32;
                let new_height = (height as f64 * scale) as u32;
                info!("内存不足重试 #{}: 缩小图像 {}x{} -> {}x{}",
                    retry_count, width, height, new_width, new_height);
                preprocessed.resize(new_width, new_height, imageops::FilterType::Lanczos3)
            } else {
                preprocessed.clone()
            };

            let rgb_image = process_image.to_rgb8();
            debug!("图像尺寸: {}x{}", rgb_image.width(), rgb_image.height());

            match ocr.predict(vec![rgb_image]) {
                Ok(results) => {
                    let raw_text = results
                        .first()
                        .map(|r| {
                            r.text_regions
                                .iter()
                                .filter_map(|region| region.text_with_confidence())
                                .filter_map(|(t, conf)| {
                                    if conf >= MIN_CONFIDENCE {
                                        Some(t)
                                    } else {
                                        debug!("过滤低置信度文本: {} (置信度: {:.2})", t, conf);
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();

                    // 应用后处理
                    let text = if !raw_text.is_empty() {
                        let lang = detect_text_language(&raw_text);
                        optimize_by_language(&raw_text, lang)
                    } else {
                        raw_text
                    };

                    info!("OCR 识别完成: {} 字符", text.len());
                    return Ok(text);
                }
                Err(e) => {
                    let error_str = e.to_string();
                    // 检查是否是内存分配错误
                    if error_str.contains("allocate") || error_str.contains("memory") || error_str.contains("Failed to allocate") {
                        warn!("OCR 内存分配失败: {}", e);
                        retry_count += 1;

                        if retry_count >= MAX_MEMORY_RETRIES || current_tile_size <= MIN_TILE_DIMENSION {
                            error!("OCR 识别失败: 达到最大重试次数或最小尺寸");
                            return Err(OcrError::OcrFailed(format!("内存不足，无法完成识别: {}", e)));
                        }

                        // 缩小尺寸重试
                        current_tile_size = (current_tile_size * 3 / 4).max(MIN_TILE_DIMENSION);
                        info!("重试 OCR，减小处理尺寸至: {}", current_tile_size);
                        continue;
                    } else {
                        error!("OCR 识别失败: {}", e);
                        return Err(OcrError::OcrFailed(format!("识别失败: {}", e)));
                    }
                }
            }
        }
    }

    /// 分块处理大图像
    fn recognize_with_tiling(&mut self, image: &DynamicImage, max_dimension: u32) -> Result<String, OcrError> {
        let ocr = self.ocr.as_ref().ok_or_else(|| {
            OcrError::OcrFailed("OCR 模型未初始化".to_string())
        })?;

        let (width, height) = image.dimensions();

        // 先缩放到目标尺寸
        let mut scaled = if width > max_dimension || height > max_dimension {
            let scale = max_dimension as f64 / width.max(height) as f64;
            let new_width = (width as f64 * scale) as u32;
            let new_height = (height as f64 * scale) as u32;
            info!("缩放图像: {}x{} -> {}x{}", width, height, new_width, new_height);
            image.resize(new_width, new_height, imageops::FilterType::Lanczos3)
        } else {
            image.clone()
        };

        let (sw, sh) = scaled.dimensions();

        // 分块重叠比例（避免文字被截断）- 使用常量 OVERLAP_RATIO = 0.25
        let tile_size = TILE_MAX_DIMENSION;
        let overlap = (tile_size as f32 * OVERLAP_RATIO) as u32;
        let step = tile_size - overlap; // 实际步进距离

        // 计算分块数量（考虑重叠）
        let cols = if sw > tile_size { ((sw - tile_size) / step + 1) as usize } else { 1 };
        let rows = if sh > tile_size { ((sh - tile_size) / step + 1) as usize } else { 1 };

        info!("分块处理: {}x{} 图像分为 {}x{} = {} 块 (重叠 {}px)", sw, sh, cols, rows, cols * rows, overlap);

        let mut all_texts: Vec<String> = Vec::new();
        let mut memory_retry_count = 0;

        for row in 0..rows {
            for col in 0..cols {
                // 计算分块位置（带重叠）
                let x0 = if col == 0 { 0 } else { (col as u32 * step).min(sw.saturating_sub(tile_size)) };
                let y0 = if row == 0 { 0 } else { (row as u32 * step).min(sh.saturating_sub(tile_size)) };
                let x1 = (x0 + tile_size).min(sw);
                let y1 = (y0 + tile_size).min(sh);

                if x0 >= x1 || y0 >= y1 {
                    continue;
                }

                debug!("处理分块 [{},{}]: ({},{}) - ({},{})", row, col, x0, y0, x1, y1);

                // 裁剪分块
                let tile = scaled.crop(x0, y0, x1 - x0, y1 - y0);

                // 带重试的识别
                loop {
                    let rgb_tile = tile.to_rgb8();

                    match ocr.predict(vec![rgb_tile]) {
                        Ok(results) => {
                            if let Some(result) = results.first() {
                                let tile_text: String = result.text_regions
                                    .iter()
                                    .filter_map(|region| region.text_with_confidence())
                                    .filter_map(|(t, conf)| {
                                        if conf >= MIN_CONFIDENCE {
                                            Some(t)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");

                                if !tile_text.is_empty() {
                                    all_texts.push(tile_text);
                                }
                            }
                            // 重置重试计数
                            memory_retry_count = 0;
                            break;
                        }
                        Err(e) => {
                            let error_str = e.to_string();
                            // 检查是否是内存分配错误
                            if error_str.contains("allocate") || error_str.contains("memory") || error_str.contains("Failed to allocate") {
                                warn!("分块 [{},{}] 内存分配失败: {}", row, col, e);
                                memory_retry_count += 1;

                                if memory_retry_count >= MAX_MEMORY_RETRIES {
                                    error!("分块处理达到最大重试次数，跳过此分块");
                                    memory_retry_count = 0;
                                    break;
                                }

                                // 缩小图像后重试此分块
                                let retry_scale = 0.75f64;
                                let retry_w = ((x1 - x0) as f64 * retry_scale) as u32;
                                let retry_h = ((y1 - y0) as f64 * retry_scale) as u32;
                                info!("重试分块 [{},{}]，缩小至 {}x{}", row, col, retry_w, retry_h);

                                // 创建缩小的分块
                                let smaller_tile = tile.resize(retry_w, retry_h, imageops::FilterType::Lanczos3);
                                let rgb_tile = smaller_tile.to_rgb8();

                                // 再次尝试
                                match ocr.predict(vec![rgb_tile]) {
                                    Ok(results) => {
                                        if let Some(result) = results.first() {
                                            let tile_text: String = result.text_regions
                                                .iter()
                                                .filter_map(|region| region.text_with_confidence())
                                                .filter_map(|(t, conf)| {
                                                    if conf >= MIN_CONFIDENCE {
                                                        Some(t)
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect::<Vec<_>>()
                                                .join("\n");

                                            if !tile_text.is_empty() {
                                                all_texts.push(tile_text);
                                            }
                                        }
                                        memory_retry_count = 0;
                                        break;
                                    }
                                    Err(_) => {
                                        // 再次失败，跳过此分块
                                        warn!("分块 [{},{}] 重试后仍失败，跳过", row, col);
                                        memory_retry_count = 0;
                                        break;
                                    }
                                }
                            } else {
                                warn!("分块 [{},{}] 识别失败: {}", row, col, e);
                                break;
                            }
                        }
                    }
                }
            }
        }

        // 显式释放大图像内存
        drop(scaled);
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);

        // 合并分块结果并应用后处理
        let raw_text = all_texts.join("\n\n");
        let text = if !raw_text.is_empty() {
            let lang = detect_text_language(&raw_text);
            optimize_by_language(&raw_text, lang)
        } else {
            raw_text
        };

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

    /// 释放 OCR 模型，释放内存
    ///
    /// 当内存紧张或不再需要 OCR 功能时调用此方法
    /// 下次使用时会自动重新加载模型
    pub fn unload_ocr(&mut self) {
        if self.ocr.take().is_some() {
            info!("OCR 模型已卸载，释放内存");
            // 强制触发内存回收
            std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// 检查模型是否已加载
    pub fn is_model_loaded(&self) -> bool {
        self.ocr.is_some()
    }
}