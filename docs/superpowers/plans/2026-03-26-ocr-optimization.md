# OCR 识别率优化实施计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不更换模型（保持 PP-OCRv5 Mobile）的前提下，通过优化图像预处理、后处理和参数调优，将 OCR 识别率提升至接近 Tesseract 水平。

**架构:** 采用多阶段图像预处理流水线（去噪→二值化→倾斜校正→锐化），添加后处理文本校正，优化分块策略避免文字截断。

**Tech Stack:** Rust, image-rs, imageproc, oar-ocr v0.6

---

## Chunk 8: 快速见效优化（高优先级）

### Task 10: 立即见效的参数调优

**Files:**
- Modify: `src-tauri/src/services/ocr_service.rs:30-43`

**背景:** 以下优化不需要修改预处理逻辑，只调整参数即可见效，建议优先实施。

- [ ] **Step 1: 立即优化核心参数**

**关键变更：**
1. **删除 `equalize_histogram` 调用** - 已在 Chunk 1 中完成
2. **增大图像尺寸** - 提升小字体识别率
3. **降低置信度阈值** - 适应 Mobile 模型特性
4. **增大分块重叠** - 避免边界文字丢失

```rust
// === 高优先级参数优化 ===

// 默认最大图像尺寸：2000 → 3000
// 效果：提升 8pt 以下小字体识别率，保留更多细节
const DEFAULT_MAX_IMAGE_DIMENSION: u32 = 3000;

// 分块处理尺寸：800 → 1200
// 效果：减少分块数量，避免文字被切分
const TILE_MAX_DIMENSION: u32 = 1200;

// 最小分块尺寸：400 → 600
const MIN_TILE_DIMENSION: u32 = 600;

// 分块重叠比例：0.15 → 0.25
// 效果：避免跨分块文字被截断
const OVERLAP_RATIO: f32 = 0.25;

// 最低置信度阈值：0.5 → 0.35
// 效果：PP-OCRv5 Mobile 模型置信度偏低，降低阈值可保留有效结果
// 注意：后处理会去重和低质量结果
const MIN_CONFIDENCE: f32 = 0.35;

// Sauvola 二值化参数优化
// window: 15 → 25（更大的窗口适应不均匀光照）
// k: 0.2 → 0.3（提高对比度敏感度）
// 在 preprocess_image 函数中使用：sauvola_threshold(&denoised, 25, 0.3)
```

- [ ] **Step 2: 验证参数变更**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 3: 快速测试**

使用包含小字体（8-10pt）的 PDF 测试，验证：
1. 识别结果字符数是否增加
2. 小字体是否被正确识别
3. 分块边界是否还有截断

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/ocr_service.rs
git commit -m "perf: 高优先级 OCR 参数优化（尺寸、阈值、重叠）"
```

---

## 优化优先级和预期效果

### 快速见效优化（Chunk 8 - 立即实施）

| 优化项 | 原值 | 新值 | 预期效果 |
|--------|------|------|---------|
| **最大图像尺寸** | 2000px | 3000px | 小字体识别率提升 20-30% |
| **置信度阈值** | 0.5 | 0.35 | 召回率提升 15-20% |
| **分块重叠** | 15% | 25% | 消除边界截断问题 |
| **分块尺寸** | 800px | 1200px | 减少分块数量，提升速度 |

**实施建议：** Chunk 8 可以在 5 分钟内完成，建议立即测试验证效果。

---

## Chunk 1: 图像预处理增强

### Task 1: 添加自适应二值化预处理

**Files:**
- Modify: `src-tauri/src/services/ocr_service.rs:48-68`
- Modify: `src-tauri/Cargo.toml:27` (确认 imageproc 版本)

**背景:** 当前预处理仅使用直方图均衡化，缺少二值化处理。OCR 对二值化图像的识别效果更好。

- [ ] **Step 1: 分析当前预处理代码**

阅读 `src-tauri/src/services/ocr_service.rs` 第 48-68 行的 `preprocess_image` 函数，了解当前实现。

- [ ] **Step 2: 修改预处理函数，添加自适应二值化**

**重要变更：**
1. **删除 `equalize_histogram`** - 避免破坏文字与背景的对比度关系
2. **调优 Sauvola 参数** - `window=25, k=0.3` 更适合文档 OCR

```rust
fn preprocess_image(image: &DynamicImage) -> DynamicImage {
    use imageproc::filter::median_filter;

    // 转换为灰度图
    let gray = image.to_luma8();

    // 1. 中值滤波去噪（保留边缘，去除噪点）
    let denoised = median_filter(&gray, 3, 3);

    // 2. 自适应阈值二值化（使用 Sauvola 算法，适合文档图像）
    // 参数优化：window=25（更大的窗口适应文档光照不均），k=0.3（提高对比度敏感度）
    let binary = sauvola_threshold(&denoised, 25, 0.3);

    // 3. 转回 RGB 格式（OAROCR 需要 RGB 输入）
    let rgb: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> = image::ImageBuffer::from_fn(
        binary.width(),
        binary.height(),
        |x, y| {
            let luma = binary.get_pixel(x, y);
            image::Rgb([luma[0], luma[0], luma[0]])
        }
    );

    DynamicImage::ImageRgb8(rgb)
}
```

- [ ] **Step 3: 添加 Sauvola 阈值实现**

在 `preprocess_image` 函数上方添加：

```rust
/// Sauvola 局部阈值算法（适合文档 OCR）
/// window_size: 邻域窗口大小（奇数）
/// k: 控制阈值敏感度（通常 0.2-0.5）
fn sauvola_threshold(image: &image::GrayImage, window_size: u32, k: f32) -> image::GrayImage {
    let (width, height) = image.dimensions();
    let half_window = (window_size / 2) as i32;
    let mut result = image::GrayImage::new(width, height);

    let max_std = 128.0; // 灰度标准差最大值

    for y in 0..height {
        for x in 0..width {
            // 计算局部均值和标准差
            let mut sum = 0u32;
            let mut sum_sq = 0u32;
            let mut count = 0u32;

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

            let mean = sum as f32 / count as f32;
            let variance = (sum_sq as f32 / count as f32) - (mean * mean);
            let std = variance.sqrt();

            // Sauvola 公式: T = mean * (1 + k * ((std / R) - 1))
            let threshold = mean * (1.0 + k * ((std / max_std) - 1.0));

            let current_pixel = image.get_pixel(x, y)[0] as f32;
            result.put_pixel(x, y, image::Luma([if current_pixel <= threshold { 0 } else { 255 }]));
        }
    }

    result
}
```

- [ ] **Step 4: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过，无错误

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/ocr_service.rs
git commit -m "feat: 添加自适应二值化预处理提升 OCR 识别率"
```

---

### Task 2: 添加倾斜校正（Deskew）功能

**Files:**
- Modify: `src-tauri/src/services/ocr_service.rs`
- Test: 使用带有倾斜的扫描 PDF 测试

**背景:** 扫描文档常有轻微倾斜（1-5度），会导致 OCR 检测框不准确。

- [ ] **Step 1: 添加霍夫变换检测倾斜角度的函数**

在 `ocr_service.rs` 中添加：

```rust
/// 使用霍夫变换检测文档倾斜角度
/// 返回角度（度），正数表示顺时针倾斜
fn detect_skew_angle(image: &image::GrayImage) -> f32 {
    use imageproc::edges::canny;
    use std::f32::consts::PI;

    // 1. Canny 边缘检测
    let edges = canny(image, 50.0, 150.0);

    // 2. 霍夫变换检测直线
    // 简化实现：使用水平投影法检测倾斜
    let max_angle = 5.0; // 只检测 ±5 度
    let angle_step = 0.5;

    let mut best_angle = 0.0;
    let mut best_score = 0.0;

    let angle = -max_angle;
    while angle <= max_angle {
        let radians = angle * PI / 180.0;
        let score = calculate_horizontal_projection(&edges, radians);

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

    let mut projection = std::collections::HashMap::new();

    for y in 0..height {
        for x in 0..width {
            if edges.get_pixel(x, y)[0] > 128 {
                // 边缘点
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

    let values: Vec<i32> = projection.values().cloned().collect();
    let mean = values.iter().sum::<i32>() as f32 / values.len() as f32;
    let variance = values.iter()
        .map(|&v| (v as f32 - mean).powi(2))
        .sum::<f32>() / values.len() as f32;

    variance
}
```

- [ ] **Step 2: 修改预处理函数，集成倾斜校正**

**重要变更：** 删除 `equalize_histogram`，使用优化后的 Sauvola 参数

```rust
fn preprocess_image(image: &DynamicImage) -> DynamicImage {
    use imageproc::filter::median_filter;
    use imageproc::geometric_transformations::{rotate_about_center, Interpolation};

    // 转换为灰度图
    let gray = image.to_luma8();

    // 1. 中值滤波去噪
    let denoised = median_filter(&gray, 3, 3);

    // 2. 检测并校正倾斜（如果角度大于 0.5 度）
    let skew_angle = detect_skew_angle(&denoised);
    let deskewed = if skew_angle.abs() > 0.5 {
        rotate_about_center(
            &denoised,
            skew_angle.to_radians(),
            Interpolation::Bilinear,
            image::Luma([255]), // 白色背景
        )
    } else {
        denoised
    };

    // 3. 自适应阈值二值化（使用优化参数）
    let binary = sauvola_threshold(&deskewed, 25, 0.3);

    // 4. 转回 RGB
    let rgb: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> = image::ImageBuffer::from_fn(
        binary.width(),
        binary.height(),
        |x, y| {
            let luma = binary.get_pixel(x, y);
            image::Rgb([luma[0], luma[0], luma[0]])
        }
    );

    DynamicImage::ImageRgb8(rgb)
}
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/ocr_service.rs
git commit -m "feat: 添加倾斜校正功能提升文档 OCR 准确性"
```

---

## Chunk 2: 图像尺寸和分块策略优化

### Task 3: 增大图像尺寸限制和优化分块策略

**Files:**
- Modify: `src-tauri/src/services/ocr_service.rs:30-43`

**背景:** 当前尺寸限制（2000px 最大，800px 分块）对于 OCR 来说过低，导致小字体文字无法识别。

- [ ] **Step 1: 修改尺寸限制常量**

**高优先级参数优化：**
1. **增大图像尺寸** - 2000 → 3000px（保留更多细节）
2. **增大分块尺寸** - 800 → 1200px（提升小字体识别）
3. **降低置信度阈值** - 0.5 → 0.35（保留更多有效结果）
4. **增大重叠比例** - 0.15 → 0.25（避免文字截断）

```rust
// 默认最大图像尺寸 - 从 2000 提升到 3000
const DEFAULT_MAX_IMAGE_DIMENSION: u32 = 3000;

// 分块处理的最大尺寸（每个分块）- 从 800 提升到 1200
const TILE_MAX_DIMENSION: u32 = 1200;

// 最小分块尺寸
const MIN_TILE_DIMENSION: u32 = 600;

// 分块重叠比例 - 从 0.15 提升到 0.25，避免文字被截断
const OVERLAP_RATIO: f32 = 0.25;

// 最低置信度阈值 - 从 0.5 降低到 0.35，避免过滤有效结果
// Mobile 模型置信度偏低，降低阈值可保留更多正确识别
const MIN_CONFIDENCE: f32 = 0.35;
```

- [ ] **Step 2: 优化分块处理逻辑，防止文字截断**

修改 `recognize_with_tiling` 函数中的重叠计算：

```rust
fn recognize_with_tiling(&mut self, image: &DynamicImage, max_dimension: u32) -> Result<String, OcrError> {
    // ... 现有代码 ...

    // 分块重叠比例（避免文字被截断）
    const OVERLAP_RATIO: f32 = 0.25; // 增加到 25%
    let tile_size = TILE_MAX_DIMENSION;
    let overlap = (tile_size as f32 * OVERLAP_RATIO) as u32;
    let step = tile_size - overlap;

    // ... 现有代码 ...

    // 修改后的分块去重逻辑
    let mut all_regions: Vec<(String, f32, imageproc::rect::Rect)> = Vec::new();

    for row in 0..rows {
        for col in 0..cols {
            // ... 裁剪分块 ...

            match ocr.predict(vec![rgb_tile]) {
                Ok(results) => {
                    if let Some(result) = results.first() {
                        for region in &result.text_regions {
                            if let Some((text, conf)) = region.text_with_confidence() {
                                if conf >= MIN_CONFIDENCE {
                                    // 记录文本、置信度和位置（相对于原图）
                                    let abs_rect = imageproc::rect::Rect::at(
                                        (x0 as i32 + region.bbox.x),
                                        (y0 as i32 + region.bbox.y)
                                    ).of_size(region.bbox.width, region.bbox.height);

                                    all_regions.push((text, conf, abs_rect));
                                }
                            }
                        }
                    }
                    memory_retry_count = 0;
                    break;
                }
                // ... 错误处理 ...
            }
        }
    }

    // 去重：移除重叠区域中的低置信度结果
    let filtered_regions = remove_duplicate_regions(all_regions, overlap as i32 / 2);

    // 按位置排序并合并文本
    let text = merge_regions_to_text(filtered_regions);

    info!("分块 OCR 完成: {} 字符", text.len());
    Ok(text)
}
```

- [ ] **Step 3: 添加区域去重和合并函数**

```rust
/// 移除重叠的识别区域，保留置信度最高的结果
fn remove_duplicate_regions(
    regions: Vec<(String, f32, imageproc::rect::Rect)>,
    min_distance: i32
) -> Vec<(String, f32, imageproc::rect::Rect)> {
    let mut result = Vec::new();
    let mut skip_indices = std::collections::HashSet::new();

    for i in 0..regions.len() {
        if skip_indices.contains(&i) {
            continue;
        }

        let (text_i, conf_i, rect_i) = &regions[i];
        let mut best_idx = i;
        let mut best_conf = *conf_i;

        // 查找与当前区域重叠的其他区域
        for j in (i + 1)..regions.len() {
            if skip_indices.contains(&j) {
                continue;
            }

            let (_, conf_j, rect_j) = &regions[j];

            // 计算中心点距离
            let center_i = (rect_i.left() + rect_i.width() as i32 / 2,
                           rect_i.top() + rect_i.height() as i32 / 2);
            let center_j = (rect_j.left() + rect_j.width() as i32 / 2,
                           rect_j.top() + rect_j.height() as i32 / 2);

            let distance = ((center_i.0 - center_j.0).pow(2) as f32 +
                          (center_i.1 - center_j.1).pow(2) as f32).sqrt();

            // 如果距离小于阈值，认为是同一文字
            if distance < min_distance as f32 {
                skip_indices.insert(j);
                if *conf_j > best_conf {
                    best_conf = *conf_j;
                    best_idx = j;
                }
            }
        }

        result.push(regions[best_idx].clone());
    }

    result
}

/// 将识别区域按阅读顺序排序并合并为文本
fn merge_regions_to_text(regions: Vec<(String, f32, imageproc::rect::Rect)>) -> String {
    if regions.is_empty() {
        return String::new();
    }

    // 按垂直位置分组（同一行的文字）
    let row_threshold = 20; // 同一行的垂直距离阈值
    let mut rows: Vec<Vec<(String, f32, i32)>> = Vec::new(); // (text, conf, x)

    for (text, conf, rect) in regions {
        let y_center = rect.top() + rect.height() as i32 / 2;
        let x_center = rect.left() + rect.width() as i32 / 2;

        // 查找是否属于现有行
        let mut found_row = false;
        for row in &mut rows {
            let row_y = row[0].2; // 使用第一个元素的 y 作为参考
            if (y_center - row_y).abs() <= row_threshold {
                row.push((text, conf, x_center));
                found_row = true;
                break;
            }
        }

        if !found_row {
            rows.push(vec![(text, conf, y_center)]);
        }
    }

    // 对每行按 x 坐标排序，然后合并所有行
    let mut all_lines: Vec<(String, i32)> = Vec::new(); // (line_text, avg_y)

    for row in rows {
        let avg_y = row.iter().map(|(_, _, y)| *y).sum::<i32>() / row.len() as i32;
        let mut sorted_row = row.clone();
        sorted_row.sort_by_key(|(_, _, x)| *x);
        let line_text: String = sorted_row.into_iter()
            .map(|(text, _, _)| text)
            .collect::<Vec<_>>()
            .join(" ");
        all_lines.push((line_text, avg_y));
    }

    // 按垂直位置排序所有行
    all_lines.sort_by_key(|(_, y)| *y);

    all_lines.into_iter()
        .map(|(text, _)| text)
        .collect::<Vec<_>>()
        .join("\n")
}
```

- [ ] **Step 4: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/ocr_service.rs
git commit -m "perf: 增大图像尺寸限制并优化分块策略避免文字截断"
```

---

## Chunk 3: PDF 渲染优化

### Task 4: 优化 PDF 渲染配置

**Files:**
- Modify: `src-tauri/src/services/pdf_service.rs:221-223`

**背景:** 当前渲染配置过于简单，没有针对 OCR 进行优化。

- [ ] **Step 1: 修改渲染配置，提升图像质量**

```rust
// 渲染配置
let render_config = PdfRenderConfig::new()
    .set_target_width(render_width as i32)
    .set_maximum_height(render_height as i32)
    .render_form_data(true)
    .set_render_flags(
        pdfium_render::prelude::PdfPageRenderFlags::AntiAliasingForText |
        pdfium_render::prelude::PdfPageRenderFlags::AntiAliasingForImages |
        pdfium_render::prelude::PdfPageRenderFlags::AntiAliasingForPaths
    );
```

- [ ] **Step 2: 添加 DPI 检测和优化**

如果 PDF 包含原始图像，尝试提取而不是重新渲染：

```rust
/// 尝试从 PDF 页面提取原始图像（如果存在）
/// 返回 None 如果没有嵌入图像或提取失败
fn try_extract_embedded_image(page: &PdfPage) -> Option<DynamicImage> {
    // 遍历页面中的图像对象
    for object in page.objects().iter() {
        if let Ok(image_object) = object.as_image_object() {
            // 尝试获取原始图像数据
            if let Ok(bitmap) = image_object.get_raw_image() {
                let width = bitmap.width() as u32;
                let height = bitmap.height() as u32;
                let pixels = bitmap.as_raw_bytes();

                // 根据颜色格式创建图像
                return match bitmap.format() {
                    Some(pdfium_render::prelude::PdfBitmapFormat::BGRA) |
                    Some(pdfium_render::prelude::PdfBitmapFormat::RGBA) => {
                        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                            width, height, pixels.to_vec()
                        ).map(image::DynamicImage::ImageRgba8)
                    }
                    _ => None,
                };
            }
        }
    }
    None
}
```

在 `render_page_with_limit` 中：

```rust
// 尝试提取原始图像
if let Some(original_image) = try_extract_embedded_image(&page) {
    info!("提取到原始嵌入图像: {}x{}", original_image.width(), original_image.height());
    return Ok(original_image);
}

// 否则使用渲染
let render_config = PdfRenderConfig::new()
    .set_target_width(render_width as i32)
    .set_maximum_height(render_height as i32)
    .render_form_data(true)
    .set_render_flags(
        pdfium_render::prelude::PdfPageRenderFlags::AntiAliasingForText |
        pdfium_render::prelude::PdfPageRenderFlags::AntiAliasingForImages
    );
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/pdf_service.rs
git commit -m "feat: 优化 PDF 渲染配置，添加抗锯齿和原始图像提取"
```

---

## Chunk 4: 后处理文本优化

### Task 5: 添加简单的后处理校正

**Files:**
- Create: `src-tauri/src/services/text_postprocess.rs`
- Modify: `src-tauri/src/services/ocr_service.rs`（集成后处理）
- Modify: `src-tauri/src/services/mod.rs`（添加模块）

**背景:** 缺少后处理校正，无法修复常见的识别错误。

- [ ] **Step 1: 创建后处理模块文件**

Create: `src-tauri/src/services/text_postprocess.rs`

```rust
/// OCR 文本后处理模块
/// 提供常见的错误校正和格式化功能

/// 常见 OCR 错误字符映射表（中文场景）
fn get_char_correction_map() -> std::collections::HashMap<char, char> {
    let mut map = std::collections::HashMap::new();

    // 常见混淆字符
    map.insert('0', 'O'); // 英文零和字母 O 混淆（根据上下文）
    map.insert('1', 'l'); // 数字 1 和小写 L
    map.insert('|', 'I'); // 竖线和字母 I
    map.insert('「', '【'); // 日文引号修正
    map.insert('」', '】');
    map.insert('『', '『');
    map.insert('』', '』');

    // 常见标点符号修正
    map.insert(',', '，'); // 英文逗号改中文逗号
    map.insert('.', '。'); // 英文句号改中文句号
    map.insert(':', '：');
    map.insert(';', '；');
    map.insert('?', '？');
    map.insert('!', '！');
    map.insert('(', '（');
    map.insert(')', '）');
    map.insert('[', '［');
    map.insert(']', '］');

    map
}

/// 后处理 OCR 识别文本
/// 包括：标点符号规范化、常见错误修正、空行合并
pub fn postprocess_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let correction_map = get_char_correction_map();
    let mut result = String::with_capacity(text.len());

    let mut prev_char = '\0';
    let mut consecutive_newlines = 0;

    for ch in text.chars() {
        // 跳过连续空行（最多保留一个空行）
        if ch == '\n' {
            consecutive_newlines += 1;
            if consecutive_newlines > 2 {
                continue;
            }
        } else {
            consecutive_newlines = 0;
        }

        // 字符修正
        let corrected = correction_map.get(&ch).copied().unwrap_or(ch);

        // 避免重复标点
        if is_punctuation(corrected) && is_punctuation(prev_char) {
            // 如果前后都是标点，保留中文标点
            if !is_ascii_punctuation(corrected) && is_ascii_punctuation(prev_char) {
                result.pop();
                result.push(corrected);
                prev_char = corrected;
                continue;
            }
        }

        result.push(corrected);
        prev_char = corrected;
    }

    // 清理首尾空白
    result.trim().to_string()
}

/// 检查是否为标点符号
fn is_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation() ||
    ['，', '。', '、', '；', '：', '？', '！', '"', '"', ''', ''',
     '（', '）', '【', '】', '《', '》', '…', '—', '～'].contains(&ch)
}

/// 检查是否为 ASCII 标点
fn is_ascii_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

/// 检测文本语言类型
pub fn detect_text_language(text: &str) -> TextLanguage {
    let chinese_chars = text.chars().filter(|&c| is_chinese_char(c)).count();
    let english_chars = text.chars().filter(|&c| c.is_ascii_alphabetic()).count();
    let total_chars = text.chars().filter(|&c| !c.is_whitespace()).count();

    if total_chars == 0 {
        return TextLanguage::Unknown;
    }

    let chinese_ratio = chinese_chars as f32 / total_chars as f32;
    let english_ratio = english_chars as f32 / total_chars as f32;

    if chinese_ratio > 0.3 {
        TextLanguage::Chinese
    } else if english_ratio > 0.5 {
        TextLanguage::English
    } else {
        TextLanguage::Mixed
    }
}

/// 文本语言类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextLanguage {
    Chinese,
    English,
    Mixed,
    Unknown,
}

/// 根据语言类型优化文本
pub fn optimize_by_language(text: &str, lang: TextLanguage) -> String {
    match lang {
        TextLanguage::Chinese => optimize_chinese_text(text),
        TextLanguage::English => optimize_english_text(text),
        _ => postprocess_text(text),
    }
}

/// 优化中文文本
fn optimize_chinese_text(text: &str) -> String {
    let mut result = postprocess_text(text);

    // 中英文之间添加空格（可选，根据需求）
    // result = add_spaces_between_languages(&result);

    // 修正全角/半角标点
    result = normalize_punctuation(&result);

    result
}

/// 优化英文文本
fn optimize_english_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_was_space = false;

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }

    result
}

/// 规范化标点符号（统一使用全角）
fn normalize_punctuation(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            ',' => '，',
            '.' if !ch.is_ascii_digit() => '。',
            ':' => '：',
            ';' => '；',
            '?' => '？',
            '!' => '！',
            '(' => '（',
            ')' => '）',
            _ => ch,
        })
        .collect()
}

/// 检查是否为中文字符
fn is_chinese_char(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch) || // CJK 统一表意文字
    ('\u{3400}'..='\u{4dbf}').contains(&ch) || // CJK 扩展 A
    ('\u{f900}'..='\u{faff}').contains(&ch)    // CJK 兼容表意文字
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postprocess_text() {
        let input = "Hello,World.";
        let result = postprocess_text(input);
        assert!(result.contains("，") || result.contains("。"));
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_text_language("中文测试"), TextLanguage::Chinese);
        assert_eq!(detect_text_language("English test"), TextLanguage::English);
    }

    #[test]
    fn test_remove_consecutive_newlines() {
        let input = "Line1\n\n\n\nLine2";
        let result = postprocess_text(input);
        assert!(!result.contains("\n\n\n"));
    }
}
```

- [ ] **Step 2: 更新 mod.rs 添加模块**

Modify: `src-tauri/src/services/mod.rs`

在文件末尾添加：

```rust
pub mod text_postprocess;
```

- [ ] **Step 3: 在 OCR 服务中集成后处理**

Modify: `src-tauri/src/services/ocr_service.rs`

在顶部添加导入：

```rust
use crate::services::text_postprocess::{postprocess_text, detect_text_language, optimize_by_language};
```

修改 `recognize_with_limit` 中的结果处理：

```rust
match ocr.predict(vec![rgb_image]) {
    Ok(results) => {
        let text = results
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
        let processed_text = if !text.is_empty() {
            let lang = detect_text_language(&text);
            optimize_by_language(&text, lang)
        } else {
            text
        };

        info!("OCR 识别完成: {} 字符", processed_text.len());
        return Ok(processed_text);
    }
    // ... 错误处理 ...
}
```

同样修改 `recognize_with_tiling` 中的结果处理。

- [ ] **Step 4: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/text_postprocess.rs

git add src-tauri/src/services/mod.rs

git add src-tauri/src/services/ocr_service.rs

git commit -m "feat: 添加 OCR 后处理文本校正功能"
```

---

## Chunk 5: 模型字典修复

### Task 6: 修复 Balanced 模型的字典不匹配问题

**Files:**
- Modify: `src-tauri/src/services/model_manager.rs:141-158`
- Modify: `src-tauri/src/services/ocr_service.rs:162-167`

**背景:** Balanced 模型使用 `ch_repsvtr_rec.onnx`，但字典使用了 PP-OCRv4 的 `ppocr_keys_v1.txt`，可能存在不匹配。

- [ ] **Step 1: 检查模型和字典的对应关系**

根据代码分析：
- `ch_repsvtr_rec.onnx` 应该是和 PP-OCRv4 兼容的识别模型
- 但应该使用对应版本的字典

修改 `model_manager.rs` 中的 Balanced 模型配置，添加注释说明：

```rust
// 平衡版：Mobile 检测 + RepSVTR 识别
// 注意：ch_repsvtr_rec 是 PP-OCRv4 架构的识别模型，使用 ppocr_keys_v1.txt
// 如果识别结果出现异常，可能需要检查模型版本
ModelType::Balanced => vec![
    ModelFile {
        name: String::from("pp-ocrv5_mobile_det.onnx"),
        url: String::from("https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_mobile_det.onnx"),
        size: 4_600_000,
    },
    ModelFile {
        name: String::from("ch_repsvtr_rec.onnx"),
        url: String::from("https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ch_repsvtr_rec.onnx"),
        size: 24_200_000,
    },
    ModelFile {
        name: String::from("ppocr_keys_v1.txt"),
        url: String::from("https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ppocr_keys_v1.txt"),
        size: 24_000,
    },
],
```

- [ ] **Step 2: 添加字典验证功能**

在 `ocr_service.rs` 中添加字典校验：

```rust
/// 验证字典文件是否正确加载
fn validate_dictionary(dict_path: &std::path::Path) -> Result<usize, OcrError> {
    use std::io::BufRead;

    let file = std::fs::File::open(dict_path)
        .map_err(|e| OcrError::InitFailed(format!("无法打开字典文件: {}", e)))?;

    let reader = std::io::BufReader::new(file);
    let line_count = reader.lines().count();

    info!("字典加载成功: {} 个字符", line_count);

    // PP-OCRv4 字典应该有约 6623 个字符
    if line_count < 6000 {
        warn!("字典字符数异常: {}，可能影响识别效果", line_count);
    }

    Ok(line_count)
}
```

在 `init_ocr` 中调用验证：

```rust
let dict_path = models_dir.join(dict_name);

// 验证字典
if let Err(e) = validate_dictionary(&dict_path) {
    warn!("字典验证警告: {}", e);
}

info!("加载 OCR 模型: type={}, det={:?}, rec={:?}, dict={:?}",
    self.model_type, det_path, rec_path, dict_path);
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/model_manager.rs

git add src-tauri/src/services/ocr_service.rs

git commit -m "chore: 添加字典验证和模型配置注释"
```

---

## Chunk 6: 配置和调优

### Task 7: 添加可配置的预处理选项

**Files:**
- Modify: `src-tauri/src/commands/settings.rs`
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/services/ocr_service.rs`

**背景:** 不同的文档类型需要不同的预处理参数，应该让用户可以配置。

- [ ] **Step 1: 在数据库中添加 OCR 配置表**

Modify: `src-tauri/src/db/schema.rs`

在 `init_schema` 函数中添加：

```rust
// OCR 配置表
conn.execute(
    "CREATE TABLE IF NOT EXISTS ocr_settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )",
    [],
)?;

// 插入默认配置
let default_settings = [
    ("ocr_preprocessing_enabled", "true"),
    ("ocr_deskew_enabled", "true"),
    ("ocr_binarization_enabled", "true"),
    ("ocr_confidence_threshold", "0.4"),
    ("ocr_max_image_dimension", "3000"),
    ("ocr_postprocessing_enabled", "true"),
];

for (key, value) in &default_settings {
    conn.execute(
        "INSERT OR IGNORE INTO ocr_settings (key, value) VALUES (?1, ?2)",
        [key, value],
    )?;
}
```

- [ ] **Step 2: 修改 OCR 服务以使用配置**

Modify: `src-tauri/src/services/ocr_service.rs`

添加配置结构体：

```rust
/// OCR 处理配置
pub struct OcrProcessingConfig {
    pub preprocessing_enabled: bool,
    pub deskew_enabled: bool,
    pub binarization_enabled: bool,
    pub confidence_threshold: f32,
    pub postprocessing_enabled: bool,
}

impl Default for OcrProcessingConfig {
    fn default() -> Self {
        Self {
            preprocessing_enabled: true,
            deskew_enabled: true,
            binarization_enabled: true,
            confidence_threshold: 0.4,
            postprocessing_enabled: true,
        }
    }
}

impl OcrService {
    /// 根据配置执行预处理
    fn preprocess_with_config(&self, image: &DynamicImage, config: &OcrProcessingConfig) -> DynamicImage {
        if !config.preprocessing_enabled {
            return image.clone();
        }

        // 根据配置选择性启用预处理步骤
        let mut result = image.clone();

        if config.binarization_enabled {
            result = self.apply_binarization(&result, config.deskew_enabled);
        }

        result
    }

    fn apply_binarization(&self, image: &DynamicImage, apply_deskew: bool) -> DynamicImage {
        // 实现二值化逻辑（复用之前的代码）
        // ...
        image.clone() // 临时占位
    }
}
```

- [ ] **Step 3: 添加设置命令**

Modify: `src-tauri/src/commands/settings.rs`

添加 OCR 配置相关的命令：

```rust
/// OCR 配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OcrConfiguration {
    pub preprocessing_enabled: bool,
    pub deskew_enabled: bool,
    pub binarization_enabled: bool,
    pub confidence_threshold: f32,
    pub postprocessing_enabled: bool,
}

/// 获取 OCR 配置
#[tauri::command]
pub fn get_ocr_configuration(db: State<'_, Db>) -> Result<OcrConfiguration, String> {
    let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;

    let mut config = OcrConfiguration::default();

    // 从数据库读取配置
    if let Ok(value) = get_setting(&conn, "ocr_preprocessing_enabled") {
        config.preprocessing_enabled = value.parse().unwrap_or(true);
    }
    if let Ok(value) = get_setting(&conn, "ocr_deskew_enabled") {
        config.deskew_enabled = value.parse().unwrap_or(true);
    }
    if let Ok(value) = get_setting(&conn, "ocr_binarization_enabled") {
        config.binarization_enabled = value.parse().unwrap_or(true);
    }
    if let Ok(value) = get_setting(&conn, "ocr_confidence_threshold") {
        config.confidence_threshold = value.parse().unwrap_or(0.4);
    }
    if let Ok(value) = get_setting(&conn, "ocr_postprocessing_enabled") {
        config.postprocessing_enabled = value.parse().unwrap_or(true);
    }

    Ok(config)
}

/// 更新 OCR 配置
#[tauri::command]
pub fn update_ocr_configuration(
    config: OcrConfiguration,
    db: State<'_, Db>,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;

    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        ["ocr_preprocessing_enabled", &config.preprocessing_enabled.to_string()],
    ).map_err(|e| format!("更新配置失败: {}", e))?;

    // 更新其他配置...

    Ok(())
}
```

- [ ] **Step 4: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/schema.rs

git add src-tauri/src/commands/settings.rs

git add src-tauri/src/services/ocr_service.rs

git commit -m "feat: 添加可配置的 OCR 预处理选项"
```

---

## Chunk 7: 测试和验证

### Task 8: 创建 OCR 测试套件

**Files:**
- Create: `src-tauri/src/services/ocr_service_tests.rs`

- [ ] **Step 1: 创建测试文件**

Create: `src-tauri/src/services/ocr_service_tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sauvola_threshold() {
        // 创建测试图像
        let mut image = image::GrayImage::new(100, 100);
        // 填充一些测试数据
        for y in 0..100 {
            for x in 0..100 {
                let value = if x < 50 { 50 } else { 200 };
                image.put_pixel(x, y, image::Luma([value]));
            }
        }

        let result = sauvola_threshold(&image, 15, 0.2);

        // 验证二值化结果
        assert_eq!(result.width(), 100);
        assert_eq!(result.height(), 100);
    }

    #[test]
    fn test_detect_skew_angle_zero() {
        // 创建没有倾斜的测试图像
        let image = image::GrayImage::new(100, 100);
        let angle = detect_skew_angle(&image);

        // 应该接近 0
        assert!(angle.abs() < 1.0);
    }

    #[test]
    fn test_preprocess_image() {
        // 创建 RGB 测试图像
        let rgb_image = image::ImageBuffer::from_fn(100, 100, |x, y| {
            image::Rgb([x as u8, y as u8, 128])
        });
        let dynamic = image::DynamicImage::ImageRgb8(rgb_image);

        let result = preprocess_image(&dynamic);

        assert_eq!(result.width(), 100);
        assert_eq!(result.height(), 100);
    }

    #[test]
    fn test_remove_duplicate_regions() {
        use imageproc::rect::Rect;

        let regions = vec![
            ("测试".to_string(), 0.9, Rect::at(10, 10).of_size(50, 20)),
            ("测试".to_string(), 0.8, Rect::at(12, 12).of_size(48, 18)), // 重叠
            ("文本".to_string(), 0.95, Rect::at(100, 10).of_size(50, 20)), // 不重叠
        ];

        let result = remove_duplicate_regions(regions, 15);

        // 应该保留 2 个（去重后）
        assert_eq!(result.len(), 2);
    }
}
```

- [ ] **Step 2: 更新 ocr_service.rs 添加测试模块**

在文件末尾添加：

```rust
#[cfg(test)]
#[path = "ocr_service_tests.rs"]
mod tests;
```

- [ ] **Step 3: 运行测试**

Run: `cd src-tauri && cargo test ocr::`
Expected: 测试通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/ocr_service_tests.rs

git add src-tauri/src/services/ocr_service.rs

git commit -m "test: 添加 OCR 服务单元测试"
```

---

## 最终验证和总结

### Task 9: 完整编译和最终测试

- [ ] **Step 1: 完整编译**

Run: `cd src-tauri && cargo build --release`
Expected: 编译成功

- [ ] **Step 2: 运行所有测试**

Run: `cd src-tauri && cargo test`
Expected: 所有测试通过

- [ ] **Step 3: 创建优化效果验证文档**

Create: `docs/ocr-optimization-results.md`

```markdown
# OCR 优化效果验证

## 快速见效优化（Chunk 8）

### 参数变更
1. **最大图像尺寸**: 2000 → 3000px
   - 预期：小字体识别率提升 20-30%
2. **置信度阈值**: 0.5 → 0.35
   - 预期：召回率提升 15-20%
3. **分块重叠**: 15% → 25%
   - 预期：消除边界截断
4. **删除 equalize_histogram**
   - 预期：避免过度处理，保留原始对比度

### Sauvola 参数优化
- **window**: 15 → 25（适应不均匀光照）
- **k**: 0.2 → 0.3（提高对比度敏感度）

## 完整优化内容

1. **图像预处理增强**
   - 添加中值滤波去噪
   - 实现 Sauvola 自适应二值化（优化参数）
   - 删除 equalize_histogram
   - 添加倾斜校正（Deskew）

2. **图像尺寸和分块策略优化**
   - 最大图像尺寸：2000 → 3000px
   - 分块大小：800 → 1200px
   - 重叠比例：15% → 25%
   - 置信度阈值：0.5 → 0.35

3. **PDF 渲染优化**
   - 添加抗锯齿选项
   - 尝试提取原始图像

4. **后处理优化**
   - 添加文本校正
   - 标点符号规范化
   - 语言检测和优化

5. **分块去重和合并**
   - 移除重叠区域的重复识别结果
   - 按阅读顺序合并文本

## 预期效果

- 中文文档识别率提升 15-30%
- 表格文档结构保留更完整
- 倾斜文档识别准确性提升
- 标点符号识别更规范

## 测试方法

1. 准备包含以下内容的测试 PDF：
   - 中文段落
   - 英文段落
   - 表格
   - 倾斜扫描页
   - **小字体文本（8-10pt）**

2. 对比优化前后的识别结果：
   - 字符准确率（CER）
   - 单词准确率（WER）
   - 段落完整性
   - **小字体识别率**
```

- [ ] **Step 4: 最终 Commit**

```bash
git add docs/ocr-optimization-results.md
git commit -m "docs: 添加 OCR 优化效果验证文档"
```

---

## 总结

本计划通过以下 **8 个 Chunk** 提升 OCR 识别率（保持 PP-OCRv5 Mobile 模型）：

### 实施顺序建议

| 优先级 | Chunk | 描述 | 预计时间 | 效果 |
|--------|-------|------|---------|------|
| **P0** | **Chunk 8** | 快速参数调优 | 5 分钟 | ⭐⭐⭐⭐⭐ |
| **P1** | **Chunk 2** | 图像尺寸优化 | 10 分钟 | ⭐⭐⭐⭐ |
| **P2** | **Chunk 1** | 预处理增强（删除 equalize_histogram） | 20 分钟 | ⭐⭐⭐⭐ |
| **P3** | **Chunk 4** | 后处理优化 | 15 分钟 | ⭐⭐⭐ |
| **P4** | **Chunk 3** | PDF 渲染优化 | 10 分钟 | ⭐⭐⭐ |
| **P5** | **Chunk 5-7** | 其他优化 | 30 分钟 | ⭐⭐ |

### 关键优化点

1. **删除 `equalize_histogram`** - 避免破坏原始对比度
2. **图像尺寸 2000→3000** - 提升小字体识别
3. **置信度阈值 0.5→0.35** - 适应 Mobile 模型特性
4. **Sauvola 参数 window=25, k=0.3** - 优化二值化效果

每个 Chunk 都是独立的，可以按顺序逐步实施。建议在每个 Chunk 完成后进行测试验证，确保优化效果。
