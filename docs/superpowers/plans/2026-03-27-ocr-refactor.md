# OCR 模块重构实施计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重构 OCR 模块，默认使用 Mobile 模型，增加用户模型选择、重新检测按钮，支持重新识别功能。

**Architecture:** 后端增加模型类型持久化和状态刷新命令；前端 OcrModelSetup 组件重构为模型选择+状态显示，PdfList 组件支持所有状态重新识别。

**Tech Stack:** Tauri 2.0, Rust, Svelte, SQLite, oar-ocr

---

## Chunk 1: 后端 - 数据库设置与模型类型持久化

### Task 1: 添加模型类型设置常量

**Files:**
- Modify: `src-tauri/src/db/mod.rs:112`

- [ ] **Step 1: 添加模型类型设置常量**

在 `src-tauri/src/db/mod.rs` 第 112 行后添加：

```rust
pub const SETTING_OCR_MODEL_TYPE: &str = "ocr_model_type";
```

- [ ] **Step 2: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/mod.rs
git commit -m "feat: 添加 OCR 模型类型设置常量

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 2: 修改默认模型为 Mobile

**Files:**
- Modify: `src-tauri/src/services/ocr_service.rs:194-197`

- [ ] **Step 1: 修改默认模型类型**

修改 `src-tauri/src/services/ocr_service.rs` 中的 `new` 方法：

```rust
/// 创建 OCR 服务（延迟加载模型）
pub fn new(data_dir: &Path) -> Result<Self, OcrError> {
    // 默认使用 Mobile 模型，适合低配电脑（内存 ~200MB）
    Self::with_model_type(data_dir, ModelType::Mobile)
}
```

- [ ] **Step 2: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/services/ocr_service.rs
git commit -m "feat: 默认使用 Mobile 模型以降低内存占用

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 3: 简化图像预处理

**Files:**
- Modify: `src-tauri/src/services/ocr_service.rs:175-184`

- [ ] **Step 1: 添加图像质量分析函数**

在 `preprocess_image` 函数前添加：

```rust
/// 图像质量分析结果
struct ImageQuality {
    contrast: f32,
    brightness: f32,
    is_clear: bool,
}

/// 分析图像质量
fn analyze_image_quality(image: &image::DynamicImage) -> ImageQuality {
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
```

- [ ] **Step 2: 简化预处理函数**

修改 `preprocess_image` 函数：

```rust
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
        // 简单的亮度调整
        let rgb = image.to_rgb8();
        let factor = 128.0 / quality.brightness as f64;
        let enhanced: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_fn(rgb.width(), rgb.height(), |x, y| {
                let pixel = rgb.get_pixel(x, y);
                image::Rgb([
                    (pixel[0] as f64 * factor).min(255.0) as u8,
                    (pixel[1] as f64 * factor).min(255.0) as u8,
                    (pixel[2] as f64 * factor).min(255.0) as u8,
                ])
            });
        return DynamicImage::ImageRgb8(enhanced);
    }

    image.clone()
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
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/ocr_service.rs
git commit -m "refactor: 简化图像预处理，根据质量动态选择处理方式

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 2: 后端 - 新增和修改 Tauri 命令

### Task 4: 添加获取模型类型列表命令

**Files:**
- Modify: `src-tauri/src/commands/ocr.rs`

- [ ] **Step 1: 添加 ModelTypeInfo 结构体**

在 `src-tauri/src/commands/ocr.rs` 文件顶部，`OcrStatus` 结构体后添加：

```rust
/// 模型类型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTypeInfo {
    pub value: String,
    pub label: String,
    pub memory: String,
}
```

- [ ] **Step 2: 添加获取模型类型列表命令**

在文件末尾添加：

```rust
/// 获取可选模型类型列表
#[tauri::command]
pub fn get_ocr_model_types() -> Vec<ModelTypeInfo> {
    vec![
        ModelTypeInfo {
            value: "mobile".to_string(),
            label: "Mobile (推荐)".to_string(),
            memory: "~200MB".to_string(),
        },
        ModelTypeInfo {
            value: "balanced".to_string(),
            label: "Balanced".to_string(),
            memory: "~300MB".to_string(),
        },
        ModelTypeInfo {
            value: "server".to_string(),
            label: "Server (高精度)".to_string(),
            memory: "~1.5GB".to_string(),
        },
        ModelTypeInfo {
            value: "lite".to_string(),
            label: "Lite (v4稳定版)".to_string(),
            memory: "~200MB".to_string(),
        },
    ]
}
```

- [ ] **Step 3: 在 lib.rs 中注册命令**

修改 `src-tauri/src/lib.rs`，在 `invoke_handler` 中添加新命令：

```rust
// 找到 invoke_handler 块，添加命令
get_ocr_model_types,
```

- [ ] **Step 4: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/ocr.rs src-tauri/src/lib.rs
git commit -m "feat: 添加获取 OCR 模型类型列表命令

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 5: 修改模型类型设置命令（持久化）

**Files:**
- Modify: `src-tauri/src/commands/ocr.rs:74-91`

- [ ] **Step 1: 修改 set_ocr_model_type 命令**

替换现有的 `set_ocr_model_type` 函数：

```rust
/// 设置模型类型（持久化保存）
#[tauri::command]
pub fn set_ocr_model_type(
    model_type: String,
    db: State<'_, Db>,
    ocr_service: State<'_, Mutex<OcrService>>,
) -> Result<(), String> {
    info!("设置 OCR 模型类型: {}", model_type);

    // 验证模型类型
    let model_type: ModelType = model_type.parse()
        .map_err(|e| format!("无效的模型类型: {}", e))?;

    // 保存到数据库
    {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;

        crate::db::set_setting(&conn, crate::db::SETTING_OCR_MODEL_TYPE, &model_type.to_string())
            .map_err(|e| {
                error!("保存模型类型设置失败: {}", e);
                format!("保存设置失败: {}", e)
            })?;
    }

    // 切换服务中的模型类型
    let mut svc = ocr_service.lock().map_err(|e| {
        error!("获取OCR服务锁失败: {}", e);
        format!("OCR服务锁定失败: {}", e)
    })?;

    svc.set_model_type(model_type);
    info!("模型类型已切换并保存: {}", model_type);
    Ok(())
}
```

- [ ] **Step 2: 添加获取当前模型类型命令**

```rust
/// 获取当前模型类型（从数据库读取）
#[tauri::command]
pub fn get_ocr_model_type(
    db: State<'_, Db>,
) -> Result<String, String> {
    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    let model_type = crate::db::get_setting(&conn, crate::db::SETTING_OCR_MODEL_TYPE)
        .unwrap_or_else(|| "mobile".to_string());

    info!("当前模型类型: {}", model_type);
    Ok(model_type)
}
```

- [ ] **Step 3: 在 lib.rs 中注册新命令**

```rust
get_ocr_model_type,
```

- [ ] **Step 4: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/ocr.rs src-tauri/src/lib.rs
git commit -m "feat: 模型类型设置持久化到数据库

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 6: 添加重新检测模型状态命令

**Files:**
- Modify: `src-tauri/src/commands/ocr.rs`

- [ ] **Step 1: 添加 refresh_ocr_status 命令**

在 `get_ocr_model_type` 函数后添加：

```rust
/// 重新检测模型状态并尝试加载
#[tauri::command]
pub fn refresh_ocr_status(
    db: State<'_, Db>,
    ocr_service: State<'_, Mutex<OcrService>>,
) -> Result<OcrStatus, String> {
    info!("重新检测 OCR 模型状态");

    // 从数据库读取模型类型
    let model_type_str = {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;

        crate::db::get_setting(&conn, crate::db::SETTING_OCR_MODEL_TYPE)
            .unwrap_or_else(|| "mobile".to_string())
    };

    let model_type: ModelType = model_type_str.parse()
        .unwrap_or(ModelType::Mobile);

    // 检查并更新服务
    let mut svc = ocr_service.lock().map_err(|e| {
        error!("获取OCR服务锁失败: {}", e);
        format!("OCR服务锁定失败: {}", e)
    })?;

    // 确保模型类型一致
    if svc.model_type() != model_type {
        svc.set_model_type(model_type);
    }

    // 如果模型文件存在但未加载，尝试加载
    let status = svc.get_status();
    if status.models_ready && !svc.is_available() {
        info!("模型文件存在但未加载，尝试加载");
        if let Err(e) = svc.init_ocr() {
            warn!("模型加载失败: {}", e);
        }
    }

    let final_status = svc.get_status();
    info!("OCR 状态检测完成: models_ready={}, available={}",
          final_status.models_ready, final_status.available);

    Ok(OcrStatus {
        available: final_status.available,
        models_ready: final_status.models_ready,
        missing_files: final_status.missing_files,
        models_dir: final_status.models_dir,
        model_type: svc.model_type().to_string(),
    })
}
```

- [ ] **Step 2: 在 lib.rs 中注册命令**

```rust
refresh_ocr_status,
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/ocr.rs src-tauri/src/lib.rs
git commit -m "feat: 添加重新检测 OCR 模型状态命令

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 7: 修改 start_ocr 支持强制重新识别

**Files:**
- Modify: `src-tauri/src/commands/ocr.rs:170-180`

- [ ] **Step 1: 修改 start_ocr 函数签名**

找到 `start_ocr` 函数，修改签名添加 `force` 参数：

```rust
/// 开始 OCR 处理
#[tauri::command]
pub async fn start_ocr(
    pdf_id: i64,
    force: Option<bool>,  // 新增：强制重新识别
    db: State<'_, Db>,
    // ... 其他参数保持不变
```

- [ ] **Step 2: 添加清除已有结果的逻辑**

在函数开头，获取 PDF 信息后添加：

```rust
    // 如果是强制重新识别，清除已有结果
    if force.unwrap_or(false) {
        info!("强制重新识别，清除已有 OCR 结果: pdf_id={}", pdf_id);

        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;

        // 清除 OCR 结果
        conn.execute(
            "DELETE FROM pdf_pages WHERE pdf_id = ?1",
            rusqlite::params![pdf_id],
        ).map_err(|e| {
            error!("清除 OCR 结果失败: {}", e);
            format!("清除结果失败: {}", e)
        })?;

        // 重置状态为 pending
        conn.execute(
            "UPDATE pdfs SET status = 'pending', error_message = NULL, updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![pdf_id],
        ).map_err(|e| {
            error!("重置状态失败: {}", e);
            format!("重置状态失败: {}", e)
        })?;
    }
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/ocr.rs
git commit -m "feat: start_ocr 支持 force 参数强制重新识别

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 3: 前端 - API 层更新

### Task 8: 更新前端 API 函数

**Files:**
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: 添加新的类型定义**

在 `DownloadProgress` 接口后添加：

```typescript
export interface ModelTypeInfo {
  value: string;
  label: string;
  memory: string;
}
```

修改 `OcrStatus` 接口，添加 `model_type` 字段：

```typescript
export interface OcrStatus {
  available: boolean;
  models_ready: boolean;
  missing_files: string[];
  models_dir: string;
  model_type: string;  // 新增
}
```

- [ ] **Step 2: 添加新的 API 函数**

在文件末尾添加：

```typescript
// 获取可选模型类型列表
export async function getOcrModelTypes(): Promise<ModelTypeInfo[]> {
  return invoke('get_ocr_model_types');
}

// 获取当前模型类型
export async function getOcrModelType(): Promise<string> {
  return invoke('get_ocr_model_type');
}

// 设置模型类型（持久化）
export async function setOcrModelType(modelType: string): Promise<void> {
  return invoke('set_ocr_model_type', { modelType });
}

// 重新检测模型状态
export async function refreshOcrStatus(): Promise<OcrStatus> {
  return invoke('refresh_ocr_status');
}
```

- [ ] **Step 3: 修改 startOcr 函数支持 force 参数**

```typescript
export async function startOcr(pdfId: number, force: boolean = false): Promise<void> {
  return invoke('start_ocr', { pdfId, force });
}
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/api/index.ts
git commit -m "feat: 前端 API 增加模型选择和重新检测功能

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 9: 更新前端 Store

**Files:**
- Modify: `src/lib/stores/index.ts`

- [ ] **Step 1: 添加模型类型相关 Store**

在文件末尾添加：

```typescript
// OCR 模型类型
export const selectedModelType = writable<string>('mobile');

// 模型类型列表
export const modelTypes = writable<{value: string, label: string, memory: string}[]>([]);

// 是否显示下载对话框
export const showDownloadDialog = writable(false);
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/stores/index.ts
git commit -m "feat: 添加模型类型相关 Store

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 4: 前端 - UI 组件重构

### Task 10: 重构 OcrModelSetup 组件

**Files:**
- Modify: `src/lib/components/OcrModelSetup.svelte`

- [ ] **Step 1: 重写整个组件**

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import {
    getOcrStatus,
    getOcrModelTypes,
    getOcrModelType,
    setOcrModelType,
    refreshOcrStatus,
    getOcrDownloadGuide,
    downloadOcrModels,
    cancelOcrDownload,
    type OcrStatus,
    type ModelTypeInfo,
    type DownloadGuide,
    type DownloadProgress,
  } from '../api';
  import {
    ocrModelStatus,
    ocrDownloadProgress,
    isDownloading,
    selectedModelType,
    modelTypes,
    showDownloadDialog,
  } from '../stores';

  let error: string | null = null;
  let downloadGuides: DownloadGuide[] = [];
  let isRefreshing = false;

  onMount(async () => {
    await loadModelTypes();
    await loadCurrentModelType();
    await checkStatus();
    downloadGuides = await getOcrDownloadGuide();

    // 监听下载进度
    const unlistenProgress = await listen<DownloadProgress>('model-download-progress', (event) => {
      ocrDownloadProgress.set(event.payload);
    });

    // 监听下载完成
    const unlistenComplete = await listen<void>('model-download-complete', async () => {
      isDownloading.set(false);
      ocrDownloadProgress.set(null);
      showDownloadDialog.set(false);
      await checkStatus();
    });

    // 监听下载错误
    const unlistenError = await listen<{ error: string }>('model-download-error', (event) => {
      isDownloading.set(false);
      error = event.payload.error;
    });

    return () => {
      unlistenProgress();
      unlistenComplete();
      unlistenError();
    };
  });

  async function loadModelTypes() {
    try {
      const types = await getOcrModelTypes();
      modelTypes.set(types);
    } catch (e) {
      console.error('Failed to load model types:', e);
    }
  }

  async function loadCurrentModelType() {
    try {
      const modelType = await getOcrModelType();
      selectedModelType.set(modelType);
    } catch (e) {
      console.error('Failed to get current model type:', e);
    }
  }

  async function checkStatus() {
    try {
      const status = await getOcrStatus();
      ocrModelStatus.set(status);
    } catch (e) {
      console.error('Failed to get OCR status:', e);
    }
  }

  async function handleRefresh() {
    isRefreshing = true;
    error = null;
    try {
      const status = await refreshOcrStatus();
      ocrModelStatus.set(status);
      if (!status.models_ready) {
        showDownloadDialog.set(true);
      }
    } catch (e) {
      error = String(e);
    } finally {
      isRefreshing = false;
    }
  }

  async function handleModelTypeChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    const newType = target.value;
    try {
      await setOcrModelType(newType);
      selectedModelType.set(newType);
      // 切换模型后刷新状态
      await checkStatus();
    } catch (e) {
      error = String(e);
    }
  }

  async function handleDownload() {
    error = null;
    isDownloading.set(true);
    ocrDownloadProgress.set(null);

    try {
      await downloadOcrModels();
    } catch (e) {
      isDownloading.set(false);
      error = String(e);
    }
  }

  function handleCancel() {
    cancelOcrDownload();
    isDownloading.set(false);
    ocrDownloadProgress.set(null);
  }

  function formatProgress(current: number, total: number): string {
    if (total === 0) return '0%';
    const percent = Math.round((current / total) * 100);
    return `${percent}%`;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  $: statusText = $ocrModelStatus?.models_ready
    ? '✓ 模型已就绪'
    : '✗ 模型未安装';
</script>

<div class="ocr-setup">
  <div class="setup-header">
    <h3>OCR 模型</h3>
  </div>

  <div class="setup-content">
    <!-- 模型选择和重新检测按钮 -->
    <div class="model-controls">
      <div class="model-select">
        <label for="model-type">模型选择</label>
        <select id="model-type" value={$selectedModelType} on:change={handleModelTypeChange}>
          {#each $modelTypes as type}
            <option value={type.value}>{type.label} ({type.memory})</option>
          {/each}
        </select>
      </div>

      <button
        class="refresh-btn"
        on:click={handleRefresh}
        disabled={isRefreshing}
      >
        <svg class:spinning={isRefreshing} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M23 4v6h-6M1 20v-6h6"/>
          <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/>
        </svg>
        重新检测
      </button>
    </div>

    <!-- 状态显示 -->
    <div class="status-text" class:ready={$ocrModelStatus?.models_ready}>
      {statusText}
    </div>

    <!-- 错误提示 -->
    {#if error}
      <div class="error-message">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/>
          <line x1="15" y1="9" x2="9" y2="15"/>
          <line x1="9" y1="9" x2="15" y2="15"/>
        </svg>
        <span>{error}</span>
      </div>
    {/if}
  </div>
</div>

<!-- 下载对话框 -->
{#if $showDownloadDialog && !$ocrModelStatus?.models_ready}
  <div class="download-dialog-overlay" on:click={() => showDownloadDialog.set(false)}>
    <div class="download-dialog" on:click|stopPropagation>
      <div class="dialog-header">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
        </svg>
        <h4>OCR 模型未安装</h4>
      </div>

      <p class="dialog-desc">请下载以下模型文件：</p>

      <ul class="file-list">
        {#each downloadGuides as guide}
          <li>
            <span class="file-name">{guide.name}</span>
            <span class="file-size">{guide.size}</span>
          </li>
        {/each}
      </ul>

      <p class="models-dir">模型目录: {$ocrModelStatus?.models_dir || 'models'}</p>

      {#if $isDownloading && $ocrDownloadProgress}
        <div class="download-progress">
          <div class="progress-info">
            <span class="file-name">{$ocrDownloadProgress.file}</span>
            <span class="progress-percent">
              {formatProgress($ocrDownloadProgress.current, $ocrDownloadProgress.total)}
            </span>
          </div>
          <div class="progress-bar">
            <div
              class="progress-fill"
              style="width: {($ocrDownloadProgress.current / ($ocrDownloadProgress.total || 1)) * 100}%"
            ></div>
          </div>
          <div class="progress-bytes">
            {formatBytes($ocrDownloadProgress.current)} / {formatBytes($ocrDownloadProgress.total)}
          </div>
          <button class="cancel-btn" on:click={handleCancel}>
            取消下载
          </button>
        </div>
      {:else}
        <div class="dialog-actions">
          <button class="download-btn" on:click={handleDownload}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
              <polyline points="7 10 12 15 17 10"/>
              <line x1="12" y1="15" x2="12" y2="3"/>
            </svg>
            在线下载
          </button>
          <button class="manual-btn" on:click={() => window.open('https://github.com/GreatV/oar-ocr/releases/tag/v0.3.0', '_blank')}>
            手动下载
          </button>
          <button class="close-btn" on:click={() => showDownloadDialog.set(false)}>
            取消
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .ocr-setup {
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 8px;
    padding: 12px;
    margin: 8px;
  }

  .setup-header h3 {
    margin: 0 0 12px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
  }

  .model-controls {
    display: flex;
    gap: 10px;
    align-items: flex-end;
    margin-bottom: 10px;
  }

  .model-select {
    flex: 1;
  }

  .model-select label {
    display: block;
    font-size: 11px;
    color: var(--text-secondary, #6b7280);
    margin-bottom: 4px;
  }

  .model-select select {
    width: 100%;
    padding: 6px 8px;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 4px;
    font-size: 12px;
    background: var(--bg-primary, #f9fafb);
    color: var(--text-primary, #1f2937);
    cursor: pointer;
  }

  .refresh-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 10px;
    background: var(--bg-primary, #f9fafb);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 4px;
    font-size: 11px;
    color: var(--text-primary, #1f2937);
    cursor: pointer;
    white-space: nowrap;
  }

  .refresh-btn:hover:not(:disabled) {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .refresh-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .refresh-btn svg {
    width: 14px;
    height: 14px;
  }

  .refresh-btn svg.spinning {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .status-text {
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
  }

  .status-text.ready {
    color: var(--success, #10b981);
  }

  .error-message {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px;
    background: var(--error-soft, #fef2f2);
    border: 1px solid var(--error, #ef4444);
    border-radius: 4px;
    margin-top: 8px;
    font-size: 11px;
    color: var(--error, #ef4444);
  }

  .error-message svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  /* 下载对话框 */
  .download-dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .download-dialog {
    background: var(--bg-secondary, #ffffff);
    border-radius: 8px;
    padding: 16px;
    max-width: 400px;
    width: 90%;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  }

  .dialog-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }

  .dialog-header svg {
    width: 20px;
    height: 20px;
    color: var(--warning, #f59e0b);
  }

  .dialog-header h4 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
  }

  .dialog-desc {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--text-secondary, #6b7280);
  }

  .file-list {
    margin: 0;
    padding: 0;
    list-style: none;
    background: var(--bg-tertiary, #f3f4f6);
    border-radius: 4px;
    padding: 8px;
    margin-bottom: 8px;
  }

  .file-list li {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    padding: 4px 0;
  }

  .file-name {
    color: var(--text-primary, #1f2937);
  }

  .file-size {
    color: var(--text-muted, #9ca3af);
  }

  .models-dir {
    margin: 0 0 12px;
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
  }

  .dialog-actions {
    display: flex;
    gap: 8px;
  }

  .download-btn, .manual-btn, .close-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 8px;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }

  .download-btn {
    background: var(--accent, #3b82f6);
    color: white;
    border: none;
  }

  .download-btn:hover {
    background: var(--accent-dark, #2563eb);
  }

  .download-btn svg, .manual-btn svg {
    width: 14px;
    height: 14px;
  }

  .manual-btn {
    background: var(--bg-primary, #f9fafb);
    color: var(--text-primary, #1f2937);
    border: 1px solid var(--border, #e5e7eb);
  }

  .close-btn {
    background: transparent;
    color: var(--text-secondary, #6b7280);
    border: 1px solid var(--border, #e5e7eb);
  }

  .download-progress {
    background: var(--bg-tertiary, #f3f4f6);
    border-radius: 4px;
    padding: 10px;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    margin-bottom: 6px;
  }

  .progress-percent {
    font-weight: 600;
    color: var(--accent, #3b82f6);
  }

  .progress-bar {
    height: 6px;
    background: var(--border, #e5e7eb);
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 6px;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent, #3b82f6);
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .progress-bytes {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
    margin-bottom: 8px;
  }

  .cancel-btn {
    width: 100%;
    padding: 6px;
    background: none;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 4px;
    font-size: 11px;
    color: var(--text-secondary, #6b7280);
    cursor: pointer;
  }

  .cancel-btn:hover {
    border-color: var(--error, #ef4444);
    color: var(--error, #ef4444);
  }
</style>
```

- [ ] **Step 2: 编译检查**

Run: `cd /root/PdfOCR && npm run check`
Expected: 无 TypeScript 错误

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/OcrModelSetup.svelte
git commit -m "refactor: 重构 OcrModelSetup 组件，增加模型选择和重新检测

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 11: 修改 PdfList 组件支持重新识别

**Files:**
- Modify: `src/lib/components/PdfList.svelte`

- [ ] **Step 1: 修改 OCR 按钮显示逻辑**

找到 `{#if pdf.status === 'pending'}` 块，修改为：

```svelte
          <div class="actions">
            <!-- pending 且不在队列中：显示 OCR 按钮 -->
            {#if pdf.status === 'pending' && getQueuePosition(pdf.id) === null}
              <button class="ocr-btn" on:click|stopPropagation={() => handleStartOcr(pdf.id)}>OCR</button>
            {/if}

            <!-- pending 在队列中：显示取消按钮 -->
            {#if pdf.status === 'pending' && getQueuePosition(pdf.id) !== null}
              <span class="queue-position">排队中 (#{getQueuePosition(pdf.id)})</span>
              <button class="cancel-btn" on:click|stopPropagation={() => handleCancelTask(pdf.id)} title="取消排队">取消</button>
            {/if}

            <!-- processing：显示加载动画 -->
            {#if pdf.status === 'processing'}
              <span class="processing-indicator">
                <svg class="spinner-small" viewBox="0 0 24 24">
                  <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" fill="none" stroke-dasharray="31.4 31.4"/>
                </svg>
              </span>
            {/if}

            <!-- done 或 error：显示 OCR 按钮重新识别 -->
            {#if pdf.status === 'done' || pdf.status === 'error'}
              <button class="ocr-btn re-ocr" on:click|stopPropagation={() => handleStartOcr(pdf.id, true)}>OCR</button>
            {/if}

            <button class="delete-btn" on:click|stopPropagation={() => handleDelete(pdf.id)} title="删除">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="3 6 5 6 21 6"/>
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
              </svg>
            </button>
          </div>
```

- [ ] **Step 2: 修改 handleStartOcr 函数**

```typescript
  async function handleStartOcr(id: number, force: boolean = false) {
    try {
      await startOcr(id, force);
      await refreshQueueStatus();
    } catch (e) {
      alert('启动OCR失败: ' + e);
    }
  }
```

- [ ] **Step 3: 添加重新识别按钮样式**

在 `<style>` 块中添加：

```css
  .ocr-btn.re-ocr {
    background: var(--accent, #3b82f6);
  }

  .ocr-btn.re-ocr:hover {
    background: var(--accent-dark, #2563eb);
  }
```

- [ ] **Step 4: 编译检查**

Run: `cd /root/PdfOCR && npm run check`
Expected: 无 TypeScript 错误

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/PdfList.svelte
git commit -m "feat: PdfList 支持 done/error 状态重新 OCR 识别

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 5: 最终验证

### Task 12: 完整编译和测试

- [ ] **Step 1: 后端编译**

Run: `cd src-tauri && cargo build --release`
Expected: 编译成功

- [ ] **Step 2: 前端类型检查**

Run: `npm run check`
Expected: 无错误

- [ ] **Step 3: 最终 Commit**

```bash
git add -A
git commit -m "chore: OCR 模块重构完成，准备测试

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## 验收清单

- [ ] 默认使用 Mobile 模型
- [ ] 用户可在 OcrModelSetup 中选择模型类型
- [ ] 点击「重新检测」可刷新模型状态
- [ ] 模型未安装时弹出下载对话框
- [ ] done/error 状态的 PDF 可点击 OCR 按钮重新识别
- [ ] 重新识别会清除旧结果
- [ ] 模型选择持久化保存