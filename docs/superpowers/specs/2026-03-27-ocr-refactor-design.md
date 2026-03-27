# OCR 模块重构设计文档

> **日期**: 2026-03-27
> **目标**: 重构 OCR 模块，优化低配电脑使用体验，增加用户可控功能

## 1. 背景

### 1.1 当前问题

- **识别率低**: 扫描合同识别不准确，存在错误
- **内存/性能问题**: 处理速度慢，内存占用高
- **用户无法选择模型**: 模型类型固定，无法根据需求调整
- **缺少模型状态刷新**: 用户无法手动重新检测模型状态
- **无法重新识别**: OCR 完成或失败后无法重新发起识别

### 1.2 目标场景

- **用户设备**: Windows 11，低配电脑（8GB 以下内存）
- **文档类型**: 扫描版合同 PDF，文字清晰、排版整齐、黑白或灰度扫描

---

## 2. 设计方案

### 2.1 模型选择功能

#### 默认模型

- 默认使用 **Mobile** 模型（~200MB 内存），适合低配电脑

#### 用户可选模型

| 模型 | 内存占用 | 识别速度 | 推荐场景 |
|------|---------|---------|---------|
| **Mobile** (默认) | ~200MB | 快 | 低配电脑、快速识别 |
| Balanced | ~300MB | 中 | 平衡场景 |
| Server | ~1.5GB | 慢 | 高精度需求 |
| Lite (v4) | ~200MB | 快 | 成熟稳定版 |

#### 数据存储

使用现有 `settings` 表存储用户偏好：

```sql
INSERT OR REPLACE INTO settings (key, value) VALUES ('ocr_model_type', 'mobile')
```

### 2.2 预处理优化

#### 策略：根据图像特征动态选择预处理方式

```
图像输入
    ↓
分析图像特征（对比度、清晰度）
    ↓
┌─────────────────────────────────────┐
│ 对比度高 + 清晰 → 直接识别          │
│ 对比度低 → 轻度对比度增强           │
│ 模糊 → 轻度锐化                     │
│ 低光照 → 自动亮度调整               │
└─────────────────────────────────────┘
    ↓
OCR 识别
```

#### 关键变更

- **移除 Sauvola 二值化**：之前导致识别失败（字符识别为 0）
- **简化预处理流程**：仅保留轻度的对比度/亮度调整
- **按需启用**：检测到图像质量问题时才启用预处理

### 2.3 内存管理优化

- 识别完成后立即释放中间图像数据
- 无排队任务时立即卸载模型
- 内存紧张时自动降低处理尺寸

---

## 3. UI 改动

### 3.1 OcrModelSetup 组件重构

#### 默认显示（模型已安装）

```
┌─────────────────────────────────────────────────────────────┐
│ OCR 模型                                                    │
│ ┌─────────────────────────┐  ┌───────────────────────────┐  │
│ │ 模型选择下拉框           │  │ [🔄 重新检测] 按钮        │  │
│ │ Mobile (推荐) ▼         │  │                           │  │
│ └─────────────────────────┘  └───────────────────────────┘  │
│ 状态: ✓ 模型已就绪                                          │
└─────────────────────────────────────────────────────────────┘
```

#### 模型未安装时的交互

点击「重新检测」→ 检测失败 → 弹出下载对话框：

```
┌─────────────────────────────────────────────────────────────┐
│ ⚠️ OCR 模型未安装                                           │
│                                                             │
│ 请下载以下模型文件：                                         │
│ • pp-ocrv5_mobile_det.onnx (4.6MB)                         │
│ • pp-ocrv5_mobile_rec.onnx (15.8MB)                        │
│ • ppocrv5_dict.txt (5KB)                                   │
│                                                             │
│ 模型目录: C:\Users\xxx\AppData\...                          │
│                                                             │
│ ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│ │ 在线下载    │  │ 手动下载    │  │ 取消                │  │
│ └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

#### 模型选择下拉框选项

| 值 | 显示文本 | 说明 |
|---|---------|------|
| mobile | Mobile (推荐) | 快速、低内存 |
| balanced | Balanced | 平衡精度与速度 |
| server | Server (高精度) | 高精度、高内存 |
| lite | Lite (v4稳定版) | 成熟稳定 |

### 3.2 PdfList 组件改动

#### OCR 按钮显示逻辑（复用现有按钮）

| 状态 | 显示内容 | 点击行为 |
|------|---------|---------|
| pending | 「OCR」按钮 | 发起 OCR 任务 |
| pending (排队中) | 「取消」按钮 | 取消排队 |
| processing | 加载动画 | - |
| done | 「OCR」按钮 | 清除结果，重新识别 |
| error | 「OCR」按钮 | 重新识别 |

#### 重新识别逻辑

1. 检查当前状态
2. 如果正在处理，先取消任务
3. 清除该 PDF 的已有 OCR 结果（`pdf_pages` 表）
4. 重置状态为 `pending`
5. 发起新的 OCR 任务

---

## 4. 后端 API 变更

### 4.1 新增命令

#### `refresh_ocr_status`

重新检测模型状态并尝试加载模型。

```rust
#[tauri::command]
pub fn refresh_ocr_status(
    ocr_service: State<'_, Mutex<OcrService>>,
) -> Result<OcrStatus, String>
```

**逻辑**：
1. 检查模型文件是否存在
2. 如果存在且模型未加载，尝试加载
3. 返回最新状态

#### `get_ocr_model_types`

获取可选模型类型列表。

```rust
#[tauri::command]
pub fn get_ocr_model_types() -> Vec<ModelTypeInfo>
```

**返回**：
```json
[
  {"value": "mobile", "label": "Mobile (推荐)", "memory": "~200MB"},
  {"value": "balanced", "label": "Balanced", "memory": "~300MB"},
  {"value": "server", "label": "Server (高精度)", "memory": "~1.5GB"},
  {"value": "lite", "label": "Lite (v4稳定版)", "memory": "~200MB"}
]
```

### 4.2 修改命令

#### `set_ocr_model_type`

修改为持久化保存模型选择。

```rust
#[tauri::command]
pub fn set_ocr_model_type(
    model_type: String,
    db: State<'_, Db>,
    ocr_service: State<'_, Mutex<OcrService>>,
) -> Result<(), String>
```

**逻辑**：
1. 验证模型类型有效性
2. 保存到数据库设置表
3. 切换 OCR 服务的模型类型
4. 重新加载模型（如果需要）

### 4.3 修改 `start_ocr`

支持重新识别场景：

```rust
#[tauri::command]
pub async fn start_ocr(
    pdf_id: i64,
    force: bool,  // 新增参数：强制重新识别
    // ... 其他参数
) -> Result<(), String>
```

**逻辑**：
- 如果 `force=true`，先清除已有 OCR 结果
- 重置状态为 `pending`
- 继续正常 OCR 流程

---

## 5. 前端 API 变更

### 5.1 新增函数

```typescript
// 重新检测模型状态
export async function refreshOcrStatus(): Promise<OcrStatus>

// 获取可选模型类型
export async function getOcrModelTypes(): Promise<ModelTypeInfo[]>
```

### 5.2 修改函数

```typescript
// 设置模型类型（已有，行为变更：会持久化保存）
export async function setOcrModelType(modelType: string): Promise<void>

// 开始 OCR（增加 force 参数）
export async function startOcr(pdfId: number, force?: boolean): Promise<void>
```

---

## 6. 数据库变更

### 6.1 设置表

使用现有 `settings` 表，新增键：

| key | value | 说明 |
|-----|-------|------|
| ocr_model_type | mobile/balanced/server/lite | 用户选择的模型类型 |

---

## 7. 文件变更清单

### 7.1 后端 (Rust)

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `ocr_service.rs` | 修改 | 默认模型改为 Mobile，简化预处理 |
| `commands/ocr.rs` | 修改 | 新增/修改命令 |
| `commands/settings.rs` | 修改 | 模型类型持久化 |
| `db/schema.rs` | 修改 | 确认设置表存在 |

### 7.2 前端 (Svelte/TypeScript)

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `OcrModelSetup.svelte` | 重构 | 模型选择、重新检测按钮 |
| `PdfList.svelte` | 修改 | OCR 按钮支持重新识别 |
| `api/index.ts` | 修改 | 新增/修改 API 函数 |
| `stores/index.ts` | 修改 | 状态管理更新 |

---

## 8. 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| Mobile 模型识别率低于 Balanced | 中 | 用户可选择切换到 Balanced/Server |
| 预处理简化后某些图像识别率下降 | 低 | 保留轻度增强，标准合同影响小 |
| 模型切换失败 | 低 | 保留原模型，显示错误提示 |

---

## 9. 验收标准

1. ✅ 默认使用 Mobile 模型，内存占用 ~200MB
2. ✅ 用户可在 OcrModelSetup 中选择模型类型
3. ✅ 点击「重新检测」可刷新模型状态
4. ✅ 模型未安装时弹出下载对话框
5. ✅ done/error 状态的 PDF 可点击 OCR 按钮重新识别
6. ✅ 重新识别会清除旧结果
7. ✅ 模型选择持久化保存