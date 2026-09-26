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

/// 图像质量分析结果
struct ImageQuality {
    contrast: f32,
    brightness: f32,
    is_clear: bool,
}

/// 分析图像质量
fn analyze_image_quality(image: &DynamicImage) -> ImageQuality {
    let gray = image.to_luma8();
    let (width, height) = gray.dimensions();

    // 采样分析（避免全图计算）
    let step = 10;
    let mut sum: f64 = 0.0;
    let mut sum_sq: f64 = 0.0;
    let mut count: u32 = 0;

    for y in (0..height).step_by(step) {
        for x in (0..width).step_by(step) {
            let pixel = gray.get_pixel(x, y)[0] as f64;
            sum += pixel;
            sum_sq += pixel * pixel;
            count += 1;
        }
    }

    let mean = sum / count as f64;
    let variance = (sum_sq / count as f64) - (mean * mean);
    let std_dev = variance.sqrt();

    ImageQuality {
        contrast: std_dev as f32,
        brightness: mean as f32,
        is_clear: std_dev > 40.0, // 标准差 > 40 认为清晰
    }
}

/// 预处理模式（T6：可配置预处理，设置键 `ocr_preprocess_mode`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PreprocessMode {
    /// 自动：按图像质量决定是否增强（默认，历史行为）
    Auto,
    /// 关闭：原图直出，完全跳过预处理
    Off,
    /// 强制：总是做轻度对比度增强（仍受 P0-7 动态范围保护）
    On,
}

impl Default for PreprocessMode {
    fn default() -> Self {
        PreprocessMode::Auto
    }
}

impl PreprocessMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PreprocessMode::Auto => "auto",
            PreprocessMode::Off => "off",
            PreprocessMode::On => "on",
        }
    }

    /// 严格解析：非法值返回 None（设置层用它校验）
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(PreprocessMode::Auto),
            "off" => Some(PreprocessMode::Off),
            "on" => Some(PreprocessMode::On),
            _ => None,
        }
    }
}

/// 预处理图像以提高 OCR 识别正确率
/// 根据图像质量动态选择预处理方式
fn preprocess_image(image: &DynamicImage) -> DynamicImage {
    let quality = analyze_image_quality(image);

    // 清晰图像直接返回
    if quality.is_clear {
        debug!("图像质量良好，跳过预处理");
        return image.clone();
    }

    debug!("图像质量较差，应用轻度增强 (对比度={:.1}, 亮度={:.1})",
           quality.contrast, quality.brightness);

    // 低对比度：轻度对比度增强
    if quality.contrast < 40.0 {
        let gray = image.to_luma8();
        let enhanced = enhance_contrast(&gray);
        return DynamicImage::ImageLuma8(enhanced);
    }

    // 低光照：亮度调整
    if quality.brightness < 100.0 {
        return enhance_brightness(image, quality.brightness);
    }

    image.clone()
}

/// 亮度提升（把平均亮度拉到 128）
fn enhance_brightness(image: &DynamicImage, brightness: f32) -> DynamicImage {
    let rgb = image.to_rgb8();
    let factor = 128.0 / brightness.max(1.0) as f64;
    let enhanced: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_fn(rgb.width(), rgb.height(), |x, y| {
            let pixel = rgb.get_pixel(x, y);
            image::Rgb([
                ((pixel[0] as f64 * factor).min(255.0)) as u8,
                ((pixel[1] as f64 * factor).min(255.0)) as u8,
                ((pixel[2] as f64 * factor).min(255.0)) as u8,
            ])
        });
    DynamicImage::ImageRgb8(enhanced)
}

/// 按模式应用预处理（纯函数入口，便于单测；T6）
fn apply_preprocess(image: &DynamicImage, mode: PreprocessMode) -> DynamicImage {
    match mode {
        PreprocessMode::Off => {
            debug!("预处理已关闭 (off)，原图直出");
            image.clone()
        }
        PreprocessMode::Auto => preprocess_image(image),
        PreprocessMode::On => {
            let quality = analyze_image_quality(image);
            // 强制增强：先对比度（enhance_contrast 自带 P0-7 动态范围保护），偏暗再提亮
            let out = DynamicImage::ImageLuma8(enhance_contrast(&image.to_luma8()));
            if quality.brightness < 100.0 {
                enhance_brightness(&out, quality.brightness)
            } else {
                out
            }
        }
    }
}

/// 轻度对比度增强
fn enhance_contrast(image: &image::GrayImage) -> image::GrayImage {
    let (width, height) = image.dimensions();

    // 计算直方图
    let mut hist = [0u32; 256];
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y)[0];
            hist[pixel as usize] += 1;
        }
    }

    // 简单的直方图拉伸
    let mut min_val = 0;
    let mut max_val = 255;
    for i in 0..256 {
        if hist[i] > height * width / 100 {
            min_val = i as u8;
            break;
        }
    }
    for i in (0..256).rev() {
        if hist[i] > height * width / 100 {
            max_val = i as u8;
            break;
        }
    }

    if max_val <= min_val {
        return image.clone();
    }

    // 动态范围过小时拉伸无意义且有害：大面积留白的扫描件上，1% 阈值可能选出
    // min=254/max=255 这类"伪范围"，1 级拉伸会把整页压成全黑，导致 OCR 输出为空（P0-7）
    if (max_val as u32) - (min_val as u32) < 16 {
        debug!(
            "对比度增强跳过: 动态范围过小 (min={}, max={})",
            min_val, max_val
        );
        return image.clone();
    }

    // 应用拉伸
    let mut result = image::GrayImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y)[0];
            let new_val = if pixel < min_val {
                0
            } else if pixel > max_val {
                255
            } else {
                ((pixel - min_val) as f32 * 255.0 / (max_val - min_val) as f32) as u8
            };
            result.put_pixel(x, y, image::Luma([new_val]));
        }
    }

    result
}

pub struct OcrService {
    model_manager: Arc<ModelManager>,
    ocr: Option<OAROCR>,
    preprocess_mode: PreprocessMode,
}

/// 计算分块布局：返回 (x0, y0, x1, y1) 列表，**完整覆盖** [0,sw)×[0,sh)。
///
/// 两处根因修复：
/// 1. **只做纵向全宽横带切分（x 恒为 [0,sw)）**：2D 方块分块会把超过分块宽度的
///    横向文本行拦腰裁剪（dim=2000 时 A4 标题行 ~1225px > 1200px tile，
///    `NewTokenBeta` 被切成 `NewTokenBe` —— 历史 Q-2 的真实根因）。改为横带后
///    文本行只可能被上下边界截断，而行高 ≪ 重叠带宽（300px），任何行都完整落在
///    至少一个横带内。
/// 2. **数量向上取整改良**：旧 floor 公式 `((size-tile)/step + 1)` 会把右/下侧
///    不足一个 step 的条带整块丢掉（1414×2000 旧版只覆盖左上 1200×1200）。
fn compute_tiles(sw: u32, sh: u32, tile_size: u32, overlap: u32) -> Vec<(u32, u32, u32, u32)> {
    let step = tile_size.saturating_sub(overlap).max(1);
    let rows = if sh <= tile_size {
        1
    } else {
        (sh - tile_size).div_ceil(step) + 1
    };

    let mut tiles = Vec::new();
    for row in 0..rows {
        // 末带贴边：y0 封顶在 sh - tile（row>0 时 sh>tile 保证不下溢）
        let y0 = if row == 0 { 0 } else { (row * step).min(sh - tile_size) };
        let y1 = (y0 + tile_size).min(sh);
        if y0 < y1 {
            tiles.push((0, y0, sw, y1));
        }
    }
    tiles
}

/// 带整页坐标的识别区域（分块 OCR 结果合并用）
#[derive(Debug, Clone, PartialEq)]
struct OcrRegion {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    text: String,
    conf: f32,
}

impl OcrRegion {
    fn area(&self) -> f32 {
        ((self.x1 - self.x0).max(0.0)) * ((self.y1 - self.y0).max(0.0))
    }

    /// 从识别区域构造：置信度 < MIN_CONFIDENCE 丢弃；分块局部坐标换算为整页坐标
    fn from_text_region(
        region: &TextRegion,
        origin_x: f32,
        origin_y: f32,
        scale: f32,
    ) -> Option<Self> {
        let (text, conf) = region.text_with_confidence()?;
        if conf < MIN_CONFIDENCE {
            debug!("过滤低置信度文本: {} (置信度: {:.2})", text, conf);
            return None;
        }
        let pts = &region.bounding_box.points;
        if pts.is_empty() {
            return None;
        }
        let min_x = pts.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let max_x = pts.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let min_y = pts.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_y = pts.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);

        Some(Self {
            x0: origin_x + min_x * scale,
            y0: origin_y + min_y * scale,
            x1: origin_x + max_x * scale,
            y1: origin_y + max_y * scale,
            text: text.to_string(),
            conf,
        })
    }
}

/// 两区域是否为「同一条被相邻横带重复识别」：交叠面积 / 较小面积 ≥ 0.5
fn is_duplicate_region(a: &OcrRegion, b: &OcrRegion) -> bool {
    let ix = (a.x1.min(b.x1) - a.x0.max(b.x0)).max(0.0);
    let iy = (a.y1.min(b.y1) - a.y0.max(b.y0)).max(0.0);
    let inter = ix * iy;
    if inter <= 0.0 {
        return false;
    }
    let min_area = a.area().min(b.area());
    min_area > 0.0 && inter / min_area >= 0.5
}

/// 合并分块识别结果：按阅读顺序排序 + 去除相邻横带的重复行。
///
/// 重复时**保留包围盒更大的那份**：被横带边界裁掉一半的行识别结果框更小，
/// 完整识别的框覆盖整行——按面积取大即可保证「完整版胜出」。
fn merge_regions(mut regions: Vec<OcrRegion>) -> Vec<OcrRegion> {
    // 阅读顺序：行带（y0 归到 16px 带）优先，同行按 x
    regions.sort_by(|a, b| {
        let band_a = (a.y0 / 16.0).round() as i64;
        let band_b = (b.y0 / 16.0).round() as i64;
        band_a
            .cmp(&band_b)
            .then_with(|| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut kept: Vec<OcrRegion> = Vec::new();
    for region in regions {
        let dup_idx = kept.iter().position(|k| is_duplicate_region(k, &region));
        match dup_idx {
            Some(i) => {
                // 完整（更大包围盒）的识别胜出
                if region.area() > kept[i].area() {
                    kept[i] = region;
                }
            }
            None => kept.push(region),
        }
    }
    kept
}

/// 从一批 predict 结果收集整页坐标区域（origin = 分块在整页中的偏移）
fn collect_regions(results: &[OAROCRResult], origin_x: f32, origin_y: f32, scale: f32) -> Vec<OcrRegion> {
    results
        .iter()
        .flat_map(|r| r.text_regions.iter())
        .filter_map(|region| OcrRegion::from_text_region(region, origin_x, origin_y, scale))
        .collect()
}

impl OcrService {
    /// 创建 OCR 服务（延迟加载模型）
    /// 使用 PP-OCRv5 Mobile 模型（与 ModelType::Balanced 同文件）：速度快、内存约 300MB
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

        info!("OCR 服务初始化成功（模型延迟加载，使用 PP-OCRv5 Mobile 模型）");

        Ok(Self {
            model_manager,
            ocr: None,
            preprocess_mode: PreprocessMode::default(),
        })
    }

    /// 设置预处理模式（T6 可配置预处理）
    pub fn set_preprocess_mode(&mut self, mode: PreprocessMode) {
        if self.preprocess_mode != mode {
            info!("OCR 预处理模式: {} -> {}", self.preprocess_mode.as_str(), mode.as_str());
            self.preprocess_mode = mode;
        }
    }

    /// 当前预处理模式
    pub fn preprocess_mode(&self) -> PreprocessMode {
        self.preprocess_mode
    }

    /// 获取 OCR 状态
    pub fn get_status(&self) -> OcrStatus {
        let status = self.model_manager.check_models(ModelType::Balanced);

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
        self.model_manager.check_models(ModelType::Balanced).ready
    }

    /// 初始化 OCR（加载模型）
    pub fn init_ocr(&mut self) -> Result<(), OcrError> {
        if self.ocr.is_some() {
            debug!("OCR 模型已加载");
            return Ok(());
        }

        let status = self.model_manager.check_models(ModelType::Balanced);

        if !status.ready {
            return Err(OcrError::ModelsMissing(format!(
                "缺少模型文件: {:?}",
                status.missing_files
            )));
        }

        let models_dir = self.model_manager.models_dir();

        // 使用 PP-OCRv5 Mobile 模型
        let (det_name, rec_name, dict_name) = ("pp-ocrv5_mobile_det.onnx", "pp-ocrv5_mobile_rec.onnx", "ppocrv5_dict.txt");

        let det_path = models_dir.join(det_name);
        let rec_path = models_dir.join(rec_name);
        let dict_path = models_dir.join(dict_name);

        info!("加载 OCR 模型: det={:?}, rec={:?}, dict={:?}",
            det_path, rec_path, dict_path);

        let ocr = OAROCRBuilder::new(&det_path, &rec_path, &dict_path)
            .region_batch_size(4)  // 限制识别器批处理大小，避免内存溢出
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

        // 小图像：按配置应用预处理以提高识别正确率（T6：auto/off/on 三模式）
        let preprocessed = apply_preprocess(image, self.preprocess_mode);
        debug!("图像预处理完成 (mode={})", self.preprocess_mode.as_str());

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
    ///
    /// 相对旧实现的两处根因修复：
    /// 1. 分块数量改用 `compute_tiles`（向上取整）——旧公式 floor 会把右/下侧
    ///    不足一个 step 的条带整块丢掉（1414×2000 旧版只处理左上 1200×1200）
    /// 2. 结果按整页坐标合并 + 重叠去重——补齐覆盖后相邻分块的重叠区会重复识别
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
        let tiles = compute_tiles(sw, sh, tile_size, overlap);

        info!("分块处理: {}x{} 图像分为 {} 块 (tile={}, 重叠 {}px)", sw, sh, tiles.len(), tile_size, overlap);

        let mut regions: Vec<OcrRegion> = Vec::new();
        let mut memory_retry_count = 0;

        for (idx, &(x0, y0, x1, y1)) in tiles.iter().enumerate() {
            if x0 >= x1 || y0 >= y1 {
                continue;
            }

            debug!("处理分块 #{}: ({},{}) - ({},{})", idx, x0, y0, x1, y1);

            // 裁剪分块
            let tile = scaled.crop(x0, y0, x1 - x0, y1 - y0);

            // 带重试的识别
            loop {
                let rgb_tile = tile.to_rgb8();

                match ocr.predict(vec![rgb_tile]) {
                    Ok(results) => {
                        regions.extend(collect_regions(&results, x0 as f32, y0 as f32, 1.0));
                        // 重置重试计数
                        memory_retry_count = 0;
                        break;
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        // 检查是否是内存分配错误
                        if error_str.contains("allocate") || error_str.contains("memory") || error_str.contains("Failed to allocate") {
                            warn!("分块 #{} 内存分配失败: {}", idx, e);
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
                            info!("重试分块 #{}，缩小至 {}x{}", idx, retry_w, retry_h);

                            // 创建缩小的分块
                            let smaller_tile = tile.resize(retry_w, retry_h, imageops::FilterType::Lanczos3);
                            let rgb_tile = smaller_tile.to_rgb8();
                            // 缩放后的局部坐标需要乘回比例才能落到整页坐标系
                            let coord_scale = (x1 - x0) as f32 / retry_w.max(1) as f32;

                            // 再次尝试
                            match ocr.predict(vec![rgb_tile]) {
                                Ok(results) => {
                                    regions.extend(collect_regions(&results, x0 as f32, y0 as f32, coord_scale));
                                    memory_retry_count = 0;
                                    break;
                                }
                                Err(_) => {
                                    // 再次失败，跳过此分块
                                    warn!("分块 #{} 重试后仍失败，跳过", idx);
                                    memory_retry_count = 0;
                                    break;
                                }
                            }
                        } else {
                            warn!("分块 #{} 识别失败: {}", idx, e);
                            break;
                        }
                    }
                }
            }
        }

        // 显式释放大图像内存
        drop(scaled);
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);

        // 合并分块结果（阅读顺序 + 重叠去重）并应用后处理
        let merged = merge_regions(regions);
        let raw_text = merged
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let text = if !raw_text.is_empty() {
            let lang = detect_text_language(&raw_text);
            optimize_by_language(&raw_text, lang)
        } else {
            raw_text
        };

        info!("分块 OCR 完成: {} 字符 ({} 个区域)", text.len(), merged.len());
        Ok(text)
    }

    /// 获取模型管理器（用于下载等操作）
    pub fn model_manager(&self) -> Arc<ModelManager> {
        Arc::clone(&self.model_manager)
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    // ===== 测试工具 =====

    fn gray(w: u32, h: u32, v: u8) -> image::GrayImage {
        image::ImageBuffer::from_pixel(w, h, Luma([v]))
    }

    /// 底色 base + 按索引改写部分像素的灰度图
    fn gray_mut(w: u32, h: u32, base: u8, paint: impl Fn(usize) -> Option<u8>) -> image::GrayImage {
        let mut raw = vec![base; (w * h) as usize];
        for (i, v) in raw.iter_mut().enumerate() {
            if let Some(nv) = paint(i) {
                *v = nv;
            }
        }
        image::GrayImage::from_raw(w, h, raw).unwrap()
    }

    fn mean_luma(img: &image::GrayImage) -> f64 {
        let raw = img.as_raw();
        raw.iter().map(|&p| p as f64).sum::<f64>() / raw.len() as f64
    }

    fn region(x0: f32, y0: f32, x1: f32, y1: f32, text: &str, conf: f32) -> OcrRegion {
        OcrRegion { x0, y0, x1, y1, text: text.to_string(), conf }
    }

    // ===== T5：分块几何（覆盖 bug + 行裁剪 bug 回归） =====

    /// 分块必须完整覆盖整页——旧 floor 公式在 1414×2000 下只覆盖左上 1200×1200
    #[test]
    fn test_compute_tiles_full_coverage() {
        let cases: [(u32, u32); 9] = [
            (1414, 2000), // dim=2000 的 A4：旧版只出 1 块，右 214px + 下 800px 丢失
            (999, 1400),  // dim=1400 的 A4：旧版下 200px 丢失
            (2500, 1600), // 旧版右 400px 丢失
            (3000, 4000), // 旧版末行下 100px 丢失
            (816, 1056),  // 样本页
            (1200, 1200), // 恰好一块
            (500, 500),   // 小于分块
            (1415, 2001), // 非对齐余量
            (2100, 2100), // 恰为 step 整数倍边界
        ];

        for (sw, sh) in cases {
            let tiles = compute_tiles(sw, sh, TILE_MAX_DIMENSION, (TILE_MAX_DIMENSION as f32 * OVERLAP_RATIO) as u32);
            assert!(!tiles.is_empty(), "{sw}x{sh} 应至少有 1 块");

            let mut covered = vec![false; (sw * sh) as usize];
            for &(x0, y0, x1, y1) in &tiles {
                assert!(x0 < x1 && y0 < y1, "{sw}x{sh} 出现空块 {:?}", (x0, y0, x1, y1));
                assert!(x1 <= sw && y1 <= sh, "{sw}x{sh} 块越界 {:?}", (x0, y0, x1, y1));
                assert!((y1 - y0) <= TILE_MAX_DIMENSION, "{sw}x{sh} 横带高度超过 TILE_MAX: {:?}", (x0, y0, x1, y1));
                for y in y0..y1 {
                    for x in x0..x1 {
                        covered[(y * sw + x) as usize] = true;
                    }
                }
            }
            assert!(covered.iter().all(|&c| c), "{sw}x{sh} 存在未覆盖像素, tiles={:?}", tiles);
        }
    }

    /// 文本行绝不能被左右裁剪：分块必须是全宽横带（Q-2 根因回归）
    #[test]
    fn test_compute_tiles_never_split_horizontally() {
        for (sw, sh) in [(1414u32, 2000u32), (1545, 1999), (3000, 4000), (816, 1056)] {
            let tiles = compute_tiles(sw, sh, TILE_MAX_DIMENSION, 300);
            for &(x0, _, x1, _) in &tiles {
                assert_eq!(x0, 0, "{sw}x{sh} 横带必须从 x=0 开始");
                assert_eq!(x1, sw, "{sw}x{sh} 横带必须覆盖全宽");
            }
        }
    }

    /// 行容纳性质：任何高度 ≤ overlap 的文本行都完整落在至少一个横带内
    /// （step = tile - overlap ⇒ 行顶距某带起点 < step，行底 < step + overlap ≤ tile）
    #[test]
    fn test_compute_tiles_line_containment() {
        let overlap = (TILE_MAX_DIMENSION as f32 * OVERLAP_RATIO) as u32; // 300
        for (sw, sh) in [(1545u32, 1999u32), (1414, 2000), (999, 1400), (3000, 4000)] {
            let tiles = compute_tiles(sw, sh, TILE_MAX_DIMENSION, overlap);
            for line_h in [40u32, 80, 160, overlap] {
                if line_h >= sh {
                    continue;
                }
                for line_top in (0..sh - line_h).step_by(37) {
                    let line_bottom = line_top + line_h;
                    let contained = tiles.iter().any(|&(_, y0, _, y1)| y0 <= line_top && line_bottom <= y1);
                    assert!(
                        contained,
                        "{sw}x{sh} 行 [{line_top},{line_bottom}) 未被任何横带完整包含, tiles={:?}",
                        tiles
                    );
                }
            }
        }
    }

    /// 相邻横带必须纵向重叠（防文字跨边界被截断）
    #[test]
    fn test_compute_tiles_overlap_between_adjacent() {
        let overlap = (TILE_MAX_DIMENSION as f32 * OVERLAP_RATIO) as u32;
        let tiles = compute_tiles(2500, 1600, TILE_MAX_DIMENSION, overlap);
        assert!(tiles.len() >= 2, "应产生多块: {:?}", tiles);

        for pair in tiles.windows(2) {
            let first = pair[0];
            let second = pair[1];
            assert_eq!(first.0, 0);
            assert!(second.1 < first.3, "相邻横带应纵向重叠: 上={:?} 下={:?}", first, second);
            assert!(first.3 - second.1 >= overlap, "重叠量应 ≥ overlap: 上={:?} 下={:?}", first, second);
        }
    }

    /// 小图不切块
    #[test]
    fn test_compute_tiles_small_image_single() {
        assert_eq!(compute_tiles(816, 1056, TILE_MAX_DIMENSION, 300).len(), 1);
        assert_eq!(compute_tiles(1200, 1200, TILE_MAX_DIMENSION, 300), vec![(0, 0, 1200, 1200)]);
    }

    // ===== T5：区域合并（阅读顺序 + 重叠去重） =====

    #[test]
    fn test_merge_regions_dedup_overlap_and_order() {
        let regions = vec![
            // 相邻分块把同一行识别了两次（整页坐标下高度重叠）
            region(100.0, 10.0, 600.0, 40.0, "同一行", 0.9),
            region(105.0, 12.0, 610.0, 42.0, "同一行", 0.8),
            // 不同行：保留
            region(100.0, 60.0, 500.0, 90.0, "第二行", 0.9),
            // 同行带靠右：保留（x 更大，不算重复）
            region(700.0, 11.0, 1100.0, 41.0, "同行右侧", 0.9),
        ];

        let merged = merge_regions(regions);
        let texts: Vec<&str> = merged.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts.len(), 3, "重叠的重复行应只保留一次: {:?}", texts);
        assert!(texts.contains(&"同一行") && texts.contains(&"第二行") && texts.contains(&"同行右侧"));

        // 阅读顺序：先按行带（y），同行按 x
        assert_eq!(merged[0].text, "同一行");
        assert_eq!(merged[1].text, "同行右侧");
        assert_eq!(merged[2].text, "第二行");
    }

    #[test]
    fn test_merge_regions_no_false_dedup() {
        // 上下相邻、无交叠的两行不得被误删
        let regions = vec![
            region(0.0, 0.0, 500.0, 30.0, "行一", 0.9),
            region(0.0, 35.0, 500.0, 65.0, "行二", 0.9),
        ];
        assert_eq!(merge_regions(regions).len(), 2);
    }

    /// Q-2 回归：被横带边界裁掉一部分的行 vs 完整识别 → 必须保留完整的那份
    #[test]
    fn test_merge_regions_prefers_larger_complete_box() {
        // partial 先进入阅读顺序（y0 更小），但包围盒小（被裁）
        let partial = region(50.0, 1160.0, 600.0, 1200.0, "新词贝塔NewTokenBe", 0.9);
        let full = region(95.0, 1165.0, 1330.0, 1232.0, "新词贝塔 NewTokenBeta", 0.95);

        let merged = merge_regions(vec![partial, full]);
        assert_eq!(merged.len(), 1, "同一行应去重: {:?}", merged);
        assert_eq!(merged[0].text, "新词贝塔 NewTokenBeta", "必须保留完整识别而非裁剪版");
        assert!(merged[0].text.ends_with("Beta"));
    }

    #[test]
    fn test_is_duplicate_region() {
        let a = region(0.0, 0.0, 100.0, 30.0, "a", 0.9);
        let dup = region(5.0, 2.0, 105.0, 32.0, "b", 0.9);
        let far = region(500.0, 0.0, 600.0, 30.0, "c", 0.9);
        let partial = region(60.0, 0.0, 160.0, 30.0, "d", 0.9); // 交叠 40/100 < 0.5

        assert!(is_duplicate_region(&a, &dup));
        assert!(!is_duplicate_region(&a, &far));
        assert!(!is_duplicate_region(&a, &partial));
    }

    #[test]
    fn test_from_text_region_confidence_and_offset() {
        use oar_ocr::processors::BoundingBox;

        let tr = TextRegion::with_recognition(
            BoundingBox::from_coords(10.0, 20.0, 110.0, 50.0),
            Some("识别行".into()),
            Some(0.92),
        );
        let r = OcrRegion::from_text_region(&tr, 900.0, 300.0, 1.0).expect("高置信度应保留");
        assert_eq!(r.text, "识别行");
        assert_eq!(r.x0, 910.0);
        assert_eq!(r.y0, 320.0);
        assert_eq!(r.x1, 1010.0);
        assert_eq!(r.y1, 350.0);
        assert_eq!(r.conf, 0.92);

        // 缩放分块（内存降级重试路径）：局部坐标乘 scale
        let r2 = OcrRegion::from_text_region(&tr, 0.0, 0.0, 1.333).expect("缩放路径应保留");
        assert!((r2.x1 - 110.0 * 1.333).abs() < 0.001);

        // 低置信度丢弃
        let low = TextRegion::with_recognition(
            BoundingBox::from_coords(0.0, 0.0, 10.0, 10.0),
            Some("噪声".into()),
            Some(MIN_CONFIDENCE - 0.1),
        );
        assert!(OcrRegion::from_text_region(&low, 0.0, 0.0, 1.0).is_none());

        // 无文本/无置信度丢弃
        let none = TextRegion::with_recognition(
            BoundingBox::from_coords(0.0, 0.0, 10.0, 10.0),
            None,
            Some(0.9),
        );
        assert!(OcrRegion::from_text_region(&none, 0.0, 0.0, 1.0).is_none());
    }

    // ===== T5：预处理 P0-7 回归 =====

    /// 大面积留白扫描件：直方图拉伸不得把整页压黑（P0-7 根因）
    #[test]
    fn test_enhance_contrast_p07_near_white_not_blackened() {
        // 30000×250 + 6000×254 + 4000×255 → min=250 max=255（范围 5 级 < 16）必须原样返回。
        // 若无动态范围保护（P0-7 旧码），250 会被拉到 0 → 整页压黑（均值会掉到 ~56）。
        let img = gray_mut(200, 200, 250, |i| {
            if i < 30_000 {
                None
            } else if i < 36_000 {
                Some(254)
            } else {
                Some(255)
            }
        });
        let before = mean_luma(&img);
        let out = enhance_contrast(&img);
        assert_eq!(out.as_raw(), img.as_raw(), "动态范围过小时应原样返回（P0-7 回归）");
        assert!(mean_luma(&out) >= before - 0.5);
        assert!(mean_luma(&out) > 200.0, "页面被压黑: mean={:.1}", mean_luma(&out));

        // 纯白页同样安全
        let white = gray(100, 100, 255);
        assert_eq!(enhance_contrast(&white).as_raw(), white.as_raw());
    }

    /// 正常动态范围必须真的被拉伸（保护不能误伤正常增强）
    #[test]
    fn test_enhance_contrast_stretches_normal_range() {
        let img = gray_mut(200, 200, 50, |i| if i % 2 == 0 { Some(200) } else { None });
        let out = enhance_contrast(&img);
        let raw = out.as_raw();
        assert!(raw.contains(&0), "低值应被拉到 0");
        assert!(raw.contains(&255), "高值应被拉到 255");
    }

    #[test]
    fn test_analyze_image_quality() {
        // 纯平图：不清晰
        let flat = gray(200, 200, 255);
        let q = analyze_image_quality(&DynamicImage::ImageLuma8(flat));
        assert!(!q.is_clear);
        assert!(q.contrast < 40.0);

        // 黑白棋盘：清晰
        let board = gray_mut(200, 200, 255, |i| {
            let x = (i % 200) as u32;
            let y = (i / 200) as u32;
            if ((x / 20) + (y / 20)) % 2 == 0 {
                Some(0)
            } else {
                None
            }
        });
        let q2 = analyze_image_quality(&DynamicImage::ImageLuma8(board));
        assert!(q2.is_clear, "对比度={:.1}", q2.contrast);
    }

    // ===== T6：可配置预处理 =====

    #[test]
    fn test_preprocess_mode_parse() {
        assert_eq!(PreprocessMode::parse("auto"), Some(PreprocessMode::Auto));
        assert_eq!(PreprocessMode::parse(" OFF "), Some(PreprocessMode::Off));
        assert_eq!(PreprocessMode::parse("On"), Some(PreprocessMode::On));
        assert_eq!(PreprocessMode::parse("yes"), None);
        assert_eq!(PreprocessMode::default(), PreprocessMode::Auto);
        // as_str ↔ parse 往返
        for m in [PreprocessMode::Auto, PreprocessMode::Off, PreprocessMode::On] {
            assert_eq!(PreprocessMode::parse(m.as_str()), Some(m));
        }
    }

    #[test]
    fn test_apply_preprocess_off_returns_original() {
        let img = gray_mut(120, 120, 100, |i| if i % 3 == 0 { Some(30) } else { None });
        let src = DynamicImage::ImageLuma8(img);
        let out = apply_preprocess(&src, PreprocessMode::Off);
        assert_eq!(out.to_luma8().as_raw(), src.to_luma8().as_raw(), "off 必须原图直出");
    }

    #[test]
    fn test_apply_preprocess_auto_keeps_clear_image() {
        // 黑白棋盘（is_clear）→ auto 不动
        let board = gray_mut(200, 200, 255, |i| {
            let x = (i % 200) as u32;
            let y = (i / 200) as u32;
            if ((x / 20) + (y / 20)) % 2 == 0 {
                Some(0)
            } else {
                None
            }
        });
        let src = DynamicImage::ImageLuma8(board);
        let out = apply_preprocess(&src, PreprocessMode::Auto);
        assert_eq!(out.to_luma8().as_raw(), src.to_luma8().as_raw());
    }

    #[test]
    fn test_apply_preprocess_auto_protects_near_white() {
        // P0-7 场景：近白低对比页走 auto 不得压黑
        let img = gray_mut(200, 200, 250, |i| {
            if i < 30_000 {
                None
            } else if i < 36_000 {
                Some(254)
            } else {
                Some(255)
            }
        });
        let src = DynamicImage::ImageLuma8(img);
        let out = apply_preprocess(&src, PreprocessMode::Auto);
        let mean = mean_luma(&out.to_luma8());
        assert!(mean > 200.0, "auto 模式下近白页被压黑: mean={:.1}", mean);
    }

    #[test]
    fn test_apply_preprocess_on_enhances_low_contrast() {
        // 强制模式：低对比灰图必须被增强（值域拉开）
        let img = gray_mut(200, 200, 100, |i| if i % 2 == 0 { Some(140) } else { None });
        let src = DynamicImage::ImageLuma8(img);
        let out = apply_preprocess(&src, PreprocessMode::On);
        let raw = out.to_luma8().into_raw();
        assert!(raw.contains(&0), "on 模式应把低值拉到 0");
        assert!(raw.contains(&255), "on 模式应把高值拉到 255");
    }

    // ===== 模型集成测试（默认忽略） =====

    /// 端到端模型测试：`PDF_MANAGER_TEST_DATA_DIR=<含 models/ 的目录> `
    /// `PDF_MANAGER_TEST_IMAGE=<图片>` 后运行：
    /// `cargo test --lib -- --ignored ocr_model_integration`
    #[test]
    #[ignore = "需要本地模型与图片：PDF_MANAGER_TEST_DATA_DIR / PDF_MANAGER_TEST_IMAGE"]
    fn ocr_model_integration_recognize() {
        let data_dir = std::env::var("PDF_MANAGER_TEST_DATA_DIR")
            .expect("请设置 PDF_MANAGER_TEST_DATA_DIR（其下需有 models/）");
        let image_path = std::env::var("PDF_MANAGER_TEST_IMAGE")
            .expect("请设置 PDF_MANAGER_TEST_IMAGE（待识别图片路径）");

        let mut svc = OcrService::new(Path::new(&data_dir)).expect("OCR service init failed");
        svc.init_ocr().expect("模型加载失败");
        assert!(svc.is_available());

        let img = image::open(&image_path).expect("读取测试图片失败");
        let text = svc
            .recognize_with_limit(&img, 1000)
            .expect("OCR 识别失败");
        assert!(!text.trim().is_empty(), "OCR 输出不应为空");
    }
}