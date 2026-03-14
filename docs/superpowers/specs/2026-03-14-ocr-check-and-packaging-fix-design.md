# OCR 引擎检查与打包修复设计文档

## 概述

本文档描述了 PDF Manager 应用的两个重要改进：
1. OCR 引擎检查与下载功能
2. 修复打包后功能异常的问题

## 第一部分：OCR 引擎检查功能

### 1.1 需求说明

- 程序启动时检查 OCR 引擎是否已下载并可用
- 显示检查状态
- 如果未下载，询问用户是否开始下载
- 显示下载进度
- 支持手动下载（提供下载链接和目录说明）

### 1.2 架构设计

```
程序启动
    ↓
ApplicationContext 初始化
    ↓
检查 OCR 模型状态
    ↓
┌─────────────────────┐
│ 模型已存在？        │
└─────────────────────┘
    ↓ 是              ↓ 否
继续初始化        显示 OCRSetupDialog
                        ↓
                   用户选择：
                   ├─ 自动下载 → 后台下载 → 显示进度 → 完成
                   ├─ 手动下载 → 显示链接和说明
                   └─ 以后再说 → 提示功能受限 → 继续初始化
```

### 1.3 新增组件

#### 1.3.1 OCRSetupDialog (`src/ui/dialogs/ocr_setup_dialog.py`)

职责：显示 OCR 状态，引导用户完成模型下载

界面元素：
- 状态标签：显示当前 OCR 状态（未安装/已安装/下载中）
- 进度条：显示下载进度
- 操作按钮：
  - 自动下载（推荐）
  - 手动下载
  - 以后再说
- 手动下载区域：
  - 下载链接列表
  - 目标目录路径
  - 重新检查按钮

#### 1.3.2 修改 OCRService (`src/services/ocr_service.py`)

新增方法：

```python
def check_model_status(self) -> dict:
    """检查 OCR 模型状态

    Returns:
        {
            "installed": bool,      # 是否已安装
            "model_path": str,      # 模型路径
            "model_size": int,      # 模型大小（字节）
            "missing_models": list  # 缺失的模型列表
        }
    """

def download_model(
    self,
    progress_callback: Callable[[int, int, str], None] = None
) -> bool:
    """下载 OCR 模型

    Args:
        progress_callback: 进度回调 (current, total, message)

    Returns:
        是否下载成功
    """

def get_manual_download_info(self) -> dict:
    """获取手动下载信息

    Returns:
        {
            "models": [
                {"name": str, "url": str, "size": int, "mirror_urls": list}
            ],
            "target_dir": str
        }
    """
```

#### 1.3.3 修改 ApplicationContext (`src/main.py`)

在 `__init__` 中添加 OCR 检查逻辑：

```python
def __init__(self, config_path: Optional[str] = None):
    # ... 现有初始化代码 ...

    # 检查 OCR 状态
    self._ocr_status = self._check_ocr_status()

def _check_ocr_status(self) -> dict:
    """检查 OCR 模型状态"""
    return self.ocr_service.check_model_status()

def ensure_ocr_available(self, parent_widget=None) -> bool:
    """确保 OCR 可用，如果不可用则引导用户下载

    Returns:
        OCR 是否可用
    """
```

### 1.4 数据流程

1. 程序启动 → ApplicationContext 初始化
2. 调用 `ocr_service.check_model_status()`
3. 如果模型不存在，返回状态中包含缺失模型列表
4. 在 `main()` 中检查状态，如果需要则显示 `OCRSetupDialog`
5. 用户选择操作：
   - 自动下载：调用 `ocr_service.download_model()`
   - 手动下载：显示下载链接和目录说明
   - 以后再说：记录状态，继续运行

### 1.5 手动下载支持

OCR 模型文件列表：
- `ch_ppocr_mobile_v2.0_cls_infer` - 文字方向分类模型
- `ch_ppocr_mobile_v2.0_det_infer` - 文字检测模型
- `ch_ppocr_mobile_v2.0_rec_infer` - 文字识别模型

下载地址：
- 官方：`https://paddleocr.bj.bcebos.com/...`
- 百度镜像：`https://mirror.baidu.com/paddlepaddle/models/...`

目标目录：
- 开发环境：`~/.paddleocr/`
- 打包环境：`程序目录/ocr_models/` 或 `~/.paddleocr/`

---

## 第二部分：修复打包问题

### 2.1 问题分析

打包后大部分功能不正常，可能原因：

1. **路径问题**
   - 相对路径在打包后失效
   - 数据存储路径指向错误位置

2. **资源缺失**
   - 配置文件未被打包
   - 数据文件路径问题

3. **模块导入**
   - 动态导入的模块未被 PyInstaller 发现
   - paddle/paddleocr 内部模块依赖

### 2.2 解决方案

#### 2.2.1 创建 path_utils.py (`src/utils/path_utils.py`)

```python
import sys
import os
from pathlib import Path

def get_resource_path(relative_path: str) -> str:
    """获取资源文件路径，兼容开发环境和打包环境

    Args:
        relative_path: 相对路径

    Returns:
        绝对路径
    """
    if getattr(sys, 'frozen', False):
        # PyInstaller 打包后的路径
        base_path = Path(sys._MEIPASS)
    else:
        # 开发环境路径
        base_path = Path(__file__).parent.parent.parent
    return str(base_path / relative_path)

def get_data_path() -> Path:
    """获取数据存储路径

    打包后使用用户 AppData 目录，开发环境使用 ./data

    Returns:
        数据目录路径
    """
    if getattr(sys, 'frozen', False):
        # 打包后使用用户数据目录
        if sys.platform == "win32":
            base_path = Path.home() / "AppData" / "Roaming" / "PdfOCR"
        elif sys.platform == "darwin":
            base_path = Path.home() / "Library" / "Application Support" / "PdfOCR"
        else:
            base_path = Path.home() / ".config" / "PdfOCR"
    else:
        # 开发环境
        base_path = Path("./data")
    return base_path

def get_app_dir() -> Path:
    """获取应用程序所在目录

    Returns:
        程序目录路径
    """
    if getattr(sys, 'frozen', False):
        return Path(sys.executable).parent
    else:
        return Path(__file__).parent.parent.parent
```

#### 2.2.2 修改 Config (`src/utils/config.py`)

使用 `get_data_path()` 替代硬编码路径：

```python
from src.utils.path_utils import get_data_path

class Config:
    def __init__(self, config_path: str | Path | None = None):
        if config_path is None:
            config_path = get_data_path() / "config.json"
        self._config_path = Path(config_path)
        # ...

    @property
    def storage_path(self) -> Path:
        return get_data_path()
```

#### 2.2.3 创建 .spec 文件 (`pdf_manager.spec`)

```python
# -*- mode: python ; coding: utf-8 -*-
import sys
from pathlib import Path

block_cipher = None

a = Analysis(
    ['src/main.py'],
    pathex=[],
    binaries=[],
    datas=[
        # 添加需要打包的数据文件
    ],
    hiddenimports=[
        'paddle',
        'paddleocr',
        'paddle.dataset.image',
        'paddle.fluid',
        'paddle.framework',
        'fitz',
        'pymupdf',
        'whoosh',
        'whoosh.fields',
        'whoosh.index',
        'whoosh.qparser',
        'whoosh.analysis',
        'whoosh.lang',
        'jieba',
        'jieba.analyse',
        'PIL',
        'PIL.Image',
        'numpy',
        'PyQt6',
        'PyQt6.QtCore',
        'PyQt6.QtGui',
        'PyQt6.QtWidgets',
        'PyQt6.sip',
        'src',
        'src.utils',
        'src.utils.config',
        'src.utils.logger',
        'src.utils.path_utils',
        'src.models',
        'src.models.schemas',
        'src.models.database',
        'src.services',
        'src.services.pdf_service',
        'src.services.ocr_service',
        'src.services.index_service',
        'src.core',
        'src.core.folder_manager',
        'src.core.pdf_manager',
        'src.core.search_service',
        'src.ui',
        'src.ui.main_window',
        'src.ui.widgets',
        'src.ui.widgets.folder_tree',
        'src.ui.widgets.pdf_list',
        'src.ui.widgets.pdf_viewer',
        'src.ui.dialogs',
        'src.ui.dialogs.settings_dialog',
        'src.ui.dialogs.import_dialog',
        'src.ui.dialogs.ocr_setup_dialog',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'tkinter',
        'matplotlib',
        'scipy',
        'pandas',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='PDF Manager',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,  # 不显示控制台窗口
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
```

#### 2.2.4 修改 main.py

添加异常处理和错误显示：

```python
def main() -> int:
    # 设置高DPI支持
    app = QApplication(sys.argv)
    app.setApplicationName("PdfOCR")
    app.setApplicationVersion("0.1.0")
    app.setOrganizationName("PdfOCR")

    # 全局异常处理
    def handle_exception(exc_type, exc_value, exc_traceback):
        if sys.frozen:
            # 打包后显示错误对话框
            from PyQt6.QtWidgets import QMessageBox
            QMessageBox.critical(
                None,
                "错误",
                f"程序发生错误:\n{exc_value}"
            )
        sys.__excepthook__(exc_type, exc_value, exc_traceback)

    sys.excepthook = handle_exception

    try:
        app_context = ApplicationContext()
    except Exception as e:
        if sys.frozen:
            from PyQt6.QtWidgets import QMessageBox
            QMessageBox.critical(
                None,
                "初始化错误",
                f"程序初始化失败:\n{e}"
            )
        return 1

    # ... 其余代码 ...
```

---

## 第三部分：测试计划

### 3.1 测试环境

- Windows 10/11
- Python 3.10+
- 干净系统（无 OCR 模型）
- 已安装 OCR 模型的系统

### 3.2 功能测试清单

| 序号 | 测试项 | 测试步骤 | 预期结果 |
|------|--------|----------|----------|
| 1 | OCR 检测-无模型 | 删除模型目录后启动程序 | 显示下载对话框 |
| 2 | OCR 检测-有模型 | 正常启动程序 | 直接进入主界面 |
| 3 | OCR 自动下载 | 点击自动下载按钮 | 显示进度，下载完成 |
| 4 | OCR 手动下载 | 点击手动下载按钮 | 显示链接和目录说明 |
| 5 | OCR 重新检查 | 手动放置模型后重新检查 | 检测到模型 |
| 6 | 添加 PDF | 添加测试 PDF | 成功导入 |
| 7 | PDF 预览 | 选中 PDF | 显示预览 |
| 8 | 搜索功能 | 输入关键词 | 显示结果 |
| 9 | 文件夹管理 | 创建/删除文件夹 | 正常操作 |
| 10 | 删除 PDF | 选中并删除 | 成功删除 |
| 11 | 打包测试 | 运行打包后程序 | 所有功能正常 |

### 3.3 打包后专项测试

1. **路径测试**
   - 验证数据文件存储在正确位置（AppData）
   - 验证配置文件正确保存和读取

2. **资源加载测试**
   - 验证所有 UI 组件正确加载
   - 验证资源文件（如有）正确加载

3. **模块导入测试**
   - 验证 paddle/paddleocr 正确导入
   - 验证 whoosh/jieba 正确导入

4. **异常处理测试**
   - 验证错误能通过对话框显示
   - 验证程序不会静默失败

---

## 实现计划

详见实现计划文档。