# OCR 引擎检查与打包修复实现计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 PDF Manager 添加 OCR 引擎检查和下载功能，并修复打包后功能异常的问题。

**Architecture:**
- 新增 `OCRSetupDialog` 对话框处理 OCR 模型检测和下载
- 新增 `path_utils.py` 处理打包后路径问题
- 修改 `OCRService` 添加模型检测和下载方法
- 创建 `.spec` 文件替代命令行打包参数

**Tech Stack:** PyQt6, PaddleOCR, PyInstaller

---

## 文件结构

### 新增文件
- `src/utils/path_utils.py` - 路径工具函数，处理打包后路径
- `src/ui/dialogs/ocr_setup_dialog.py` - OCR 设置对话框
- `tests/test_path_utils.py` - 路径工具测试
- `tests/test_ocr_setup_dialog.py` - OCR 对话框测试
- `pdf_manager.spec` - PyInstaller 打包配置

### 修改文件
- `src/services/ocr_service.py` - 添加模型检测和下载方法
- `src/utils/config.py` - 使用新的路径工具
- `src/main.py` - 添加 OCR 检查逻辑和异常处理
- `src/ui/dialogs/__init__.py` - 导出新对话框
- `requirements.txt` - 添加打包相关依赖

---

## Chunk 1: 路径工具模块

### Task 1: 创建 path_utils.py

**Files:**
- Create: `src/utils/path_utils.py`
- Test: `tests/test_path_utils.py`

- [ ] **Step 1: 编写路径工具测试**

```python
# tests/test_path_utils.py
"""路径工具测试"""

import sys
import os
from pathlib import Path
from unittest import mock

import pytest


class TestGetResourcePath:
    """测试 get_resource_path 函数"""

    def test_development_mode(self):
        """开发模式下返回正确路径"""
        from src.utils.path_utils import get_resource_path

        # 确保不在打包模式
        with mock.patch.object(sys, 'frozen', False):
            path = get_resource_path("test.txt")
            assert "test.txt" in path
            assert os.path.isabs(path)

    def test_frozen_mode(self):
        """打包模式下返回正确路径"""
        from src.utils.path_utils import get_resource_path

        with mock.patch.object(sys, 'frozen', True):
            with mock.patch.object(sys, '_MEIPASS', '/tmp/meipass'):
                path = get_resource_path("test.txt")
                assert path == "/tmp/meipass/test.txt"


class TestGetDataPath:
    """测试 get_data_path 函数"""

    def test_development_mode(self):
        """开发模式返回 ./data"""
        from src.utils.path_utils import get_data_path

        with mock.patch.object(sys, 'frozen', False):
            path = get_data_path()
            assert path == Path("./data")

    def test_frozen_mode_windows(self):
        """打包模式 Windows 返回 AppData 路径"""
        from src.utils.path_utils import get_data_path

        with mock.patch.object(sys, 'frozen', True):
            with mock.patch('sys.platform', 'win32'):
                with mock.patch.dict(os.environ, {'USERPROFILE': 'C:\\Users\\test'}):
                    path = get_data_path()
                    assert "PdfOCR" in str(path)

    def test_frozen_mode_linux(self):
        """打包模式 Linux 返回 .config 路径"""
        from src.utils.path_utils import get_data_path

        with mock.patch.object(sys, 'frozen', True):
            with mock.patch('sys.platform', 'linux'):
                with mock.patch.object(Path, 'home', return_value=Path('/home/test')):
                    path = get_data_path()
                    assert ".config/PdfOCR" in str(path)


class TestGetAppDir:
    """测试 get_app_dir 函数"""

    def test_development_mode(self):
        """开发模式返回项目根目录"""
        from src.utils.path_utils import get_app_dir

        with mock.patch.object(sys, 'frozen', False):
            path = get_app_dir()
            assert path.is_absolute()

    def test_frozen_mode(self):
        """打包模式返回可执行文件目录"""
        from src.utils.path_utils import get_app_dir

        with mock.patch.object(sys, 'frozen', True):
            with mock.patch.object(sys, 'executable', '/app/PDF Manager.exe'):
                path = get_app_dir()
                assert path == Path('/app')
```

- [ ] **Step 2: 运行测试确认失败**

```bash
python -m pytest tests/test_path_utils.py -v
```

Expected: FAIL - 模块不存在

- [ ] **Step 3: 实现路径工具模块**

```python
# src/utils/path_utils.py
"""路径工具模块

处理开发环境和打包环境的路径差异。
"""

import sys
import os
from pathlib import Path
from typing import Union


def get_resource_path(relative_path: Union[str, Path]) -> str:
    """获取资源文件路径，兼容开发环境和打包环境

    Args:
        relative_path: 相对路径

    Returns:
        绝对路径字符串
    """
    if getattr(sys, 'frozen', False):
        # PyInstaller 打包后的路径
        base_path = Path(sys._MEIPASS)
    else:
        # 开发环境路径 - 项目根目录
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
            # Windows: %APPDATA%/PdfOCR
            base_path = Path.home() / "AppData" / "Roaming" / "PdfOCR"
        elif sys.platform == "darwin":
            # macOS: ~/Library/Application Support/PdfOCR
            base_path = Path.home() / "Library" / "Application Support" / "PdfOCR"
        else:
            # Linux: ~/.config/PdfOCR
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
        # 打包后返回可执行文件所在目录
        return Path(sys.executable).parent
    else:
        # 开发环境返回项目根目录
        return Path(__file__).parent.parent.parent


def get_ocr_models_path() -> Path:
    """获取 OCR 模型存储路径

    打包后优先使用程序目录下的 ocr_models，
    如果不存在则使用用户数据目录。

    Returns:
        OCR 模型目录路径
    """
    if getattr(sys, 'frozen', False):
        # 打包后检查程序目录
        app_models = get_app_dir() / "ocr_models"
        if app_models.exists():
            return app_models
        # 回退到用户数据目录
        return get_data_path() / "ocr_models"
    else:
        # 开发环境使用默认 PaddleOCR 路径
        return Path.home() / ".paddleocr"
```

- [ ] **Step 4: 运行测试确认通过**

```bash
python -m pytest tests/test_path_utils.py -v
```

Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/utils/path_utils.py tests/test_path_utils.py
git commit -m "feat: 添加路径工具模块处理打包后路径问题

- get_resource_path: 获取资源文件路径
- get_data_path: 获取数据存储路径
- get_app_dir: 获取程序目录
- get_ocr_models_path: 获取 OCR 模型路径

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 2: OCR 服务扩展

### Task 2: 扩展 OCRService 添加模型检测和下载功能

**Files:**
- Modify: `src/services/ocr_service.py`
- Modify: `tests/test_ocr_service.py` (如存在)

- [ ] **Step 1: 编写模型状态检测测试**

```python
# 在 tests/test_ocr_service.py 添加

class TestOCRServiceModelStatus:
    """测试 OCR 模型状态检测"""

    def test_check_model_status_not_installed(self, tmp_path):
        """模型未安装时返回正确状态"""
        from src.services.ocr_service import OCRService

        service = OCRService(lang="ch")
        # 模拟模型不存在的情况
        status = service.check_model_status()

        assert "installed" in status
        assert "missing_models" in status

    def test_check_model_status_installed(self):
        """模型已安装时返回正确状态"""
        from src.services.ocr_service import OCRService

        service = OCRService(lang="ch")
        status = service.check_model_status()

        # 如果模型已安装
        if status["installed"]:
            assert status["model_path"] is not None
            assert len(status["missing_models"]) == 0
```

- [ ] **Step 2: 运行测试确认失败**

```bash
python -m pytest tests/test_ocr_service.py -v -k "model_status"
```

Expected: FAIL - 方法不存在

- [ ] **Step 3: 实现模型状态检测方法**

在 `src/services/ocr_service.py` 添加：

```python
# 在 imports 部分添加
import os
from pathlib import Path
from typing import Callable, Dict, List, Optional

# 在 OCRService 类中添加方法

def check_model_status(self) -> Dict:
    """检查 OCR 模型状态

    Returns:
        {
            "installed": bool,      # 是否已安装
            "model_path": str,      # 模型路径
            "model_size": int,      # 模型大小（字节）
            "missing_models": list  # 缺失的模型列表
        }
    """
    result = {
        "installed": False,
        "model_path": None,
        "model_size": 0,
        "missing_models": []
    }

    # PaddleOCR 模型名称列表
    required_models = [
        "ch_ppocr_mobile_v2.0_det_infer",
        "ch_ppocr_mobile_v2.0_cls_infer",
        "ch_ppocr_mobile_v2.0_rec_infer"
    ]

    try:
        # 检查 PaddleOCR 默认模型目录
        model_dir = Path.home() / ".paddleocr"

        if not model_dir.exists():
            result["missing_models"] = required_models
            return result

        # 检查每个模型是否存在
        total_size = 0
        missing = []

        for model_name in required_models:
            model_path = model_dir / model_name
            if model_path.exists():
                # 计算模型大小
                for file in model_path.glob("*"):
                    if file.is_file():
                        total_size += file.stat().st_size
            else:
                missing.append(model_name)

        result["model_path"] = str(model_dir)
        result["model_size"] = total_size
        result["missing_models"] = missing
        result["installed"] = len(missing) == 0

    except Exception:
        result["missing_models"] = required_models

    return result

def get_manual_download_info(self) -> Dict:
    """获取手动下载信息

    Returns:
        {
            "models": [
                {"name": str, "url": str, "size": str, "mirror_urls": list}
            ],
            "target_dir": str
        }
    """
    from src.utils.path_utils import get_ocr_models_path

    models = [
        {
            "name": "ch_ppocr_mobile_v2.0_det_infer",
            "description": "文字检测模型",
            "url": "https://paddleocr.bj.bcebos.com/dygraph_v2.0/ch/ch_ppocr_mobile_v2.0_det_infer.tar",
            "size": "~3MB",
            "mirror_urls": [
                "https://mirror.baidu.com/paddlepaddle/models/ocr/ch_ppocr_mobile_v2.0_det_infer.tar"
            ]
        },
        {
            "name": "ch_ppocr_mobile_v2.0_cls_infer",
            "description": "文字方向分类模型",
            "url": "https://paddleocr.bj.bcebos.com/dygraph_v2.0/ch/ch_ppocr_mobile_v2.0_cls_infer.tar",
            "size": "~2MB",
            "mirror_urls": [
                "https://mirror.baidu.com/paddlepaddle/models/ocr/ch_ppocr_mobile_v2.0_cls_infer.tar"
            ]
        },
        {
            "name": "ch_ppocr_mobile_v2.0_rec_infer",
            "description": "文字识别模型",
            "url": "https://paddleocr.bj.bcebos.com/dygraph_v2.0/ch/ch_ppocr_mobile_v2.0_rec_infer.tar",
            "size": "~5MB",
            "mirror_urls": [
                "https://mirror.baidu.com/paddlepaddle/models/ocr/ch_ppocr_mobile_v2.0_rec_infer.tar"
            ]
        }
    ]

    return {
        "models": models,
        "target_dir": str(get_ocr_models_path()),
        "instructions": [
            "1. 下载上述三个模型文件（.tar 格式）",
            "2. 解压 tar 文件",
            "3. 将解压后的文件夹放到目标目录",
            "4. 点击「重新检查」按钮验证安装"
        ]
    }

def download_models(
    self,
    progress_callback: Optional[Callable[[int, int, str], None]] = None
) -> bool:
    """下载 OCR 模型

    Args:
        progress_callback: 进度回调 (current, total, message)

    Returns:
        是否下载成功
    """
    import urllib.request
    import tarfile
    from src.utils.path_utils import get_ocr_models_path

    models_info = self.get_manual_download_info()["models"]
    model_dir = Path(get_ocr_models_path())
    model_dir.mkdir(parents=True, exist_ok=True)

    total_models = len(models_info)

    try:
        for i, model in enumerate(models_info):
            if progress_callback:
                progress_callback(
                    i, total_models,
                    f"正在下载 {model['description']}..."
                )

            model_path = model_dir / model["name"]

            # 如果模型已存在，跳过
            if model_path.exists():
                continue

            # 下载 tar 文件
            tar_path = model_dir / f"{model['name']}.tar"

            def report_progress(block_num, block_size, total_size):
                if progress_callback:
                    downloaded = block_num * block_size
                    percent = min(100, int(downloaded * 100 / total_size)) if total_size > 0 else 0
                    progress_callback(
                        i, total_models,
                        f"下载 {model['description']}: {percent}%"
                    )

            urllib.request.urlretrieve(
                model["url"],
                tar_path,
                reporthook=report_progress
            )

            # 解压
            if progress_callback:
                progress_callback(
                    i, total_models,
                    f"解压 {model['description']}..."
                )

            with tarfile.open(tar_path, 'r') as tar:
                tar.extractall(path=model_dir)

            # 删除 tar 文件
            tar_path.unlink()

        if progress_callback:
            progress_callback(
                total_models, total_models,
                "下载完成！"
            )

        # 重置 OCR 引擎
        self._ocr_engine = None
        self._available = None

        return True

    except Exception as e:
        if progress_callback:
            progress_callback(0, total_models, f"下载失败: {str(e)}")
        return False
```

- [ ] **Step 4: 运行测试确认通过**

```bash
python -m pytest tests/test_ocr_service.py -v -k "model_status"
```

Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/services/ocr_service.py tests/test_ocr_service.py
git commit -m "feat: 为 OCRService 添加模型检测和下载方法

- check_model_status: 检查模型是否已安装
- get_manual_download_info: 获取手动下载信息
- download_models: 自动下载模型

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 3: OCR 设置对话框

### Task 3: 创建 OCRSetupDialog 对话框

**Files:**
- Create: `src/ui/dialogs/ocr_setup_dialog.py`
- Modify: `src/ui/dialogs/__init__.py`
- Create: `tests/test_ocr_setup_dialog.py`

- [ ] **Step 1: 编写对话框测试**

```python
# tests/test_ocr_setup_dialog.py
"""OCR 设置对话框测试"""

import pytest
from PyQt6.QtWidgets import QApplication
from unittest import mock


@pytest.fixture
def app(qtbot):
    """创建 QApplication"""
    return QApplication.instance() or QApplication([])


class TestOCRSetupDialog:
    """测试 OCRSetupDialog"""

    def test_dialog_creation(self, app, qtbot):
        """测试对话框可以创建"""
        from src.ui.dialogs.ocr_setup_dialog import OCRSetupDialog

        dialog = OCRSetupDialog()
        qtbot.addWidget(dialog)

        assert dialog is not None
        assert dialog.windowTitle() == "OCR 引擎设置"

    def test_status_label_exists(self, app, qtbot):
        """测试状态标签存在"""
        from src.ui.dialogs.ocr_setup_dialog import OCRSetupDialog

        dialog = OCRSetupDialog()
        qtbot.addWidget(dialog)

        assert hasattr(dialog, '_status_label')
        assert dialog._status_label is not None

    def test_progress_bar_exists(self, app, qtbot):
        """测试进度条存在"""
        from src.ui.dialogs.ocr_setup_dialog import OCRSetupDialog

        dialog = OCRSetupDialog()
        qtbot.addWidget(dialog)

        assert hasattr(dialog, '_progress_bar')
        assert dialog._progress_bar is not None

    def test_set_status(self, app, qtbot):
        """测试设置状态"""
        from src.ui.dialogs.ocr_setup_dialog import OCRSetupDialog

        dialog = OCRSetupDialog()
        qtbot.addWidget(dialog)

        dialog.set_status("测试状态")
        assert "测试状态" in dialog._status_label.text()
```

- [ ] **Step 2: 运行测试确认失败**

```bash
python -m pytest tests/test_ocr_setup_dialog.py -v
```

Expected: FAIL - 模块不存在

- [ ] **Step 3: 实现 OCRSetupDialog**

```python
# src/ui/dialogs/ocr_setup_dialog.py
"""OCR 设置对话框"""

from typing import TYPE_CHECKING, Optional, Callable

from PyQt6.QtWidgets import (
    QDialog,
    QVBoxLayout,
    QHBoxLayout,
    QLabel,
    QProgressBar,
    QPushButton,
    QGroupBox,
    QTextEdit,
    QMessageBox,
    QStackedWidget,
    QWidget,
)
from PyQt6.QtCore import Qt, pyqtSignal, QThread

if TYPE_CHECKING:
    from src.services.ocr_service import OCRService


class DownloadWorker(QThread):
    """模型下载工作线程"""

    progress = pyqtSignal(int, int, str)  # current, total, message
    finished = pyqtSignal(bool)  # success

    def __init__(self, ocr_service: "OCRService"):
        super().__init__()
        self._ocr_service = ocr_service

    def run(self) -> None:
        """执行下载"""
        try:
            success = self._ocr_service.download_models(
                progress_callback=lambda c, t, m: self.progress.emit(c, t, m)
            )
            self.finished.emit(success)
        except Exception as e:
            self.progress.emit(0, 1, f"下载失败: {str(e)}")
            self.finished.emit(False)


class OCRSetupDialog(QDialog):
    """OCR 设置对话框

    显示 OCR 模型状态，支持自动下载和手动下载。
    """

    # 信号：OCR 准备就绪
    ocr_ready = pyqtSignal()

    def __init__(
        self,
        ocr_service: Optional["OCRService"] = None,
        parent: Optional[QWidget] = None
    ):
        super().__init__(parent)
        self._ocr_service = ocr_service
        self._download_worker: Optional[DownloadWorker] = None

        self.setWindowTitle("OCR 引擎设置")
        self.setMinimumSize(500, 400)
        self.setModal(True)

        self._setup_ui()
        self._check_status()

    def _setup_ui(self) -> None:
        """设置 UI 布局"""
        layout = QVBoxLayout(self)
        layout.setSpacing(16)

        # 状态标签
        self._status_label = QLabel("正在检查 OCR 模型状态...")
        self._status_label.setStyleSheet("font-size: 14px; font-weight: bold;")
        layout.addWidget(self._status_label)

        # 堆叠组件（切换不同视图）
        self._stack = QStackedWidget()
        layout.addWidget(self._stack)

        # 视图1：操作按钮（自动下载/手动下载）
        self._action_view = self._create_action_view()
        self._stack.addWidget(self._action_view)

        # 视图2：下载进度
        self._download_view = self._create_download_view()
        self._stack.addWidget(self._download_view)

        # 视图3：手动下载说明
        self._manual_view = self._create_manual_view()
        self._stack.addWidget(self._manual_view)

        # 默认显示操作视图
        self._stack.setCurrentWidget(self._action_view)

    def _create_action_view(self) -> QWidget:
        """创建操作按钮视图"""
        widget = QWidget()
        layout = QVBoxLayout(widget)

        # 说明文字
        info_label = QLabel(
            "PDF Manager 使用 PaddleOCR 识别扫描版 PDF 中的文字。\n"
            "首次使用需要下载 OCR 模型（约 10MB）。"
        )
        info_label.setWordWrap(True)
        layout.addWidget(info_label)

        layout.addSpacing(16)

        # 自动下载按钮
        self._auto_download_btn = QPushButton("自动下载（推荐）")
        self._auto_download_btn.setMinimumHeight(40)
        self._auto_download_btn.clicked.connect(self._on_auto_download)
        layout.addWidget(self._auto_download_btn)

        # 手动下载按钮
        self._manual_download_btn = QPushButton("手动下载")
        self._manual_download_btn.setMinimumHeight(40)
        self._manual_download_btn.clicked.connect(self._on_manual_download)
        layout.addWidget(self._manual_download_btn)

        # 以后再说按钮
        self._later_btn = QPushButton("以后再说")
        self._later_btn.setMinimumHeight(40)
        self._later_btn.clicked.connect(self._on_later)
        layout.addWidget(self._later_btn)

        layout.addStretch()

        return widget

    def _create_download_view(self) -> QWidget:
        """创建下载进度视图"""
        widget = QWidget()
        layout = QVBoxLayout(widget)

        # 进度条
        self._progress_bar = QProgressBar()
        self._progress_bar.setMinimum(0)
        self._progress_bar.setMaximum(100)
        self._progress_bar.setValue(0)
        layout.addWidget(self._progress_bar)

        # 进度文本
        self._progress_label = QLabel("准备下载...")
        layout.addWidget(self._progress_label)

        layout.addStretch()

        # 取消按钮
        cancel_btn = QPushButton("取消")
        cancel_btn.clicked.connect(self._on_cancel_download)
        layout.addWidget(cancel_btn)

        return widget

    def _create_manual_view(self) -> QWidget:
        """创建手动下载说明视图"""
        widget = QWidget()
        layout = QVBoxLayout(widget)

        # 说明
        title = QLabel("手动下载 OCR 模型")
        title.setStyleSheet("font-weight: bold;")
        layout.addWidget(title)

        # 目标目录
        self._target_dir_label = QLabel()
        self._target_dir_label.setWordWrap(True)
        layout.addWidget(self._target_dir_label)

        # 下载说明
        self._manual_text = QTextEdit()
        self._manual_text.setReadOnly(True)
        self._manual_text.setMinimumHeight(150)
        layout.addWidget(self._manual_text)

        # 按钮行
        btn_layout = QHBoxLayout()

        back_btn = QPushButton("返回")
        back_btn.clicked.connect(lambda: self._stack.setCurrentWidget(self._action_view))
        btn_layout.addWidget(back_btn)

        check_btn = QPushButton("重新检查")
        check_btn.clicked.connect(self._recheck_models)
        btn_layout.addWidget(check_btn)

        layout.addLayout(btn_layout)

        return widget

    def _check_status(self) -> None:
        """检查 OCR 模型状态"""
        if self._ocr_service is None:
            self.set_status("OCR 服务不可用")
            return

        status = self._ocr_service.check_model_status()

        if status["installed"]:
            self.set_status("OCR 模型已安装")
            self._auto_download_btn.setText("已安装 ✓")
            self._auto_download_btn.setEnabled(False)
        else:
            missing_count = len(status["missing_models"])
            self.set_status(f"OCR 模型未安装（缺少 {missing_count} 个模型）")

    def set_status(self, status: str) -> None:
        """设置状态文本"""
        self._status_label.setText(status)

    def _on_auto_download(self) -> None:
        """自动下载按钮点击"""
        if self._ocr_service is None:
            QMessageBox.warning(self, "错误", "OCR 服务不可用")
            return

        # 切换到下载视图
        self._stack.setCurrentWidget(self._download_view)
        self._status_label.setText("正在下载 OCR 模型...")

        # 启动下载线程
        self._download_worker = DownloadWorker(self._ocr_service)
        self._download_worker.progress.connect(self._on_download_progress)
        self._download_worker.finished.connect(self._on_download_finished)
        self._download_worker.start()

    def _on_download_progress(self, current: int, total: int, message: str) -> None:
        """下载进度更新"""
        self._progress_label.setText(message)
        if total > 0:
            progress = int((current + 1) / total * 100)
            self._progress_bar.setValue(min(100, progress))

    def _on_download_finished(self, success: bool) -> None:
        """下载完成"""
        if success:
            self._status_label.setText("OCR 模型安装完成！")
            QMessageBox.information(self, "成功", "OCR 模型下载完成！")
            self.ocr_ready.emit()
            self.accept()
        else:
            self._status_label.setText("下载失败")
            QMessageBox.warning(self, "失败", "OCR 模型下载失败，请尝试手动下载。")
            self._stack.setCurrentWidget(self._action_view)

    def _on_manual_download(self) -> None:
        """手动下载按钮点击"""
        if self._ocr_service is None:
            QMessageBox.warning(self, "错误", "OCR 服务不可用")
            return

        # 切换到手动下载视图
        self._stack.setCurrentWidget(self._manual_view)

        # 获取下载信息
        info = self._ocr_service.get_manual_download_info()

        # 显示目标目录
        self._target_dir_label.setText(f"目标目录: {info['target_dir']}")

        # 构建说明文本
        text_parts = ["下载以下模型文件:\n"]

        for model in info["models"]:
            text_parts.append(f"• {model['name']} ({model['description']}) - {model['size']}")
            text_parts.append(f"  官方: {model['url']}")
            if model.get('mirror_urls'):
                text_parts.append(f"  镜像: {model['mirror_urls'][0]}")
            text_parts.append("")

        text_parts.append("\n操作步骤:")
        for step in info.get("instructions", []):
            text_parts.append(step)

        self._manual_text.setText("\n".join(text_parts))

    def _on_later(self) -> None:
        """以后再说按钮点击"""
        reply = QMessageBox.question(
            self,
            "确认",
            "不安装 OCR 模型将无法识别扫描版 PDF。确定要稍后安装吗？",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.No
        )

        if reply == QMessageBox.StandardButton.Yes:
            self.reject()

    def _on_cancel_download(self) -> None:
        """取消下载"""
        if self._download_worker is not None and self._download_worker.isRunning():
            self._download_worker.terminate()
            self._download_worker.wait()

        self._stack.setCurrentWidget(self._action_view)
        self._status_label.setText("下载已取消")

    def _recheck_models(self) -> None:
        """重新检查模型"""
        self._check_status()

        status = self._ocr_service.check_model_status()
        if status["installed"]:
            QMessageBox.information(self, "成功", "OCR 模型已检测到！")
            self.ocr_ready.emit()
            self.accept()
        else:
            QMessageBox.warning(
                self,
                "未检测到",
                f"未检测到模型，请确保文件放置正确。\n缺少: {', '.join(status['missing_models'])}"
            )

    def set_ocr_service(self, ocr_service: "OCRService") -> None:
        """设置 OCR 服务"""
        self._ocr_service = ocr_service
        self._check_status()

    def closeEvent(self, event) -> None:
        """关闭事件"""
        if self._download_worker is not None and self._download_worker.isRunning():
            self._download_worker.terminate()
            self._download_worker.wait()
        event.accept()
```

- [ ] **Step 4: 更新 dialogs/__init__.py**

```python
# src/ui/dialogs/__init__.py
"""UI对话框模块"""

from .settings_dialog import SettingsDialog
from .import_dialog import ImportDialog
from .ocr_setup_dialog import OCRSetupDialog

__all__ = ["SettingsDialog", "ImportDialog", "OCRSetupDialog"]
```

- [ ] **Step 5: 运行测试确认通过**

```bash
python -m pytest tests/test_ocr_setup_dialog.py -v
```

Expected: PASS

- [ ] **Step 6: 提交**

```bash
git add src/ui/dialogs/ocr_setup_dialog.py src/ui/dialogs/__init__.py tests/test_ocr_setup_dialog.py
git commit -m "feat: 添加 OCR 设置对话框

- 显示 OCR 模型状态
- 支持自动下载模型
- 支持手动下载（显示下载链接和说明）
- 支持重新检查模型

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 4: 主程序集成

### Task 4: 集成 OCR 检查到主程序

**Files:**
- Modify: `src/main.py`
- Modify: `src/utils/config.py`

- [ ] **Step 1: 修改 Config 使用新的路径工具**

在 `src/utils/config.py` 中：

```python
# 修改 imports
from pathlib import Path
from typing import Any

from src.utils.path_utils import get_data_path

# 修改 storage_path 属性
@property
def storage_path(self) -> Path:
    """数据存储路径"""
    # 如果配置中有指定路径，使用配置的路径
    configured = self._config.get("storage_path")
    if configured and configured != "./data":
        return Path(configured)
    # 否则使用智能路径
    return get_data_path()

# 修改 __init__ 方法
def __init__(self, config_path: str | Path | None = None):
    if config_path is None:
        # 使用智能数据路径
        config_path = get_data_path() / "config.json"
    self._config_path = Path(config_path)
    self._config: dict[str, Any] = DEFAULT_CONFIG.copy()
    if self._config_path and self._config_path.exists():
        self.load()
```

- [ ] **Step 2: 修改 main.py 添加 OCR 检查逻辑**

在 `src/main.py` 中：

```python
# 修改 imports
import sys
from pathlib import Path
from typing import Optional

from PyQt6.QtWidgets import QApplication, QMessageBox
from PyQt6.QtCore import Qt

from src.utils.config import Config
from src.utils.path_utils import get_data_path
from src.models.database import Database
from src.services.pdf_service import PDFService
from src.services.ocr_service import OCRService
from src.services.index_service import IndexService
from src.core.folder_manager import FolderManager
from src.core.pdf_manager import PDFManager
from src.core.search_service import SearchService
from src.ui.main_window import MainWindow
from src.ui.dialogs import OCRSetupDialog

# 在 ApplicationContext 类中添加
class ApplicationContext:
    """应用上下文"""

    def __init__(self, config_path: Optional[str] = None):
        """初始化应用上下文"""
        # 确定配置文件路径
        if config_path is None:
            config_path = self._get_default_config_path()

        # 初始化配置
        self.config = Config(config_path)

        # 确保所有必要目录存在
        self.config.ensure_directories()

        # 初始化数据库
        self.database = Database(self.config.database_path)

        # 初始化服务
        self.pdf_service = PDFService(
            thumbnail_size=self.config.get("thumbnail_size", 200)
        )

        self.ocr_service = OCRService(
            lang=self.config.get("ocr_language", "ch"),
            use_gpu=False
        )

        self.index_service = IndexService(
            index_path=str(self.config.index_path)
        )

        # 初始化管理器
        self.folder_manager = FolderManager(database=self.database)

        self.pdf_manager = PDFManager(
            database=self.database,
            pdf_service=self.pdf_service,
            ocr_service=self.ocr_service,
            index_service=self.index_service,
            storage_path=str(self.config.pdfs_path)
        )

        self.search_service = SearchService(
            database=self.database,
            index_service=self.index_service
        )

        # OCR 状态
        self._ocr_checked = False

    def check_ocr_available(self, parent=None) -> bool:
        """检查 OCR 是否可用，如果不可用则显示设置对话框

        Args:
            parent: 父窗口

        Returns:
            OCR 是否可用
        """
        if self._ocr_checked:
            return self.ocr_service.is_available()

        status = self.ocr_service.check_model_status()

        if status["installed"]:
            self._ocr_checked = True
            return True

        # 显示 OCR 设置对话框
        dialog = OCRSetupDialog(self.ocr_service, parent)
        result = dialog.exec()

        self._ocr_checked = self.ocr_service.is_available()
        return self._ocr_checked

    # ... 其余方法保持不变 ...


# 修改 main 函数
def main() -> int:
    """应用程序主入口函数"""
    # 全局异常处理
    def handle_exception(exc_type, exc_value, exc_traceback):
        if getattr(sys, 'frozen', False):
            # 打包后显示错误对话框
            QMessageBox.critical(
                None,
                "错误",
                f"程序发生错误:\n{exc_value}"
            )
        else:
            # 开发环境打印到控制台
            print(f"Error: {exc_value}")
        sys.__excepthook__(exc_type, exc_value, exc_traceback)

    sys.excepthook = handle_exception

    # 创建 QApplication 实例
    app = QApplication(sys.argv)
    app.setApplicationName("PdfOCR")
    app.setApplicationVersion("0.1.0")
    app.setOrganizationName("PdfOCR")

    # 创建应用上下文
    try:
        app_context = ApplicationContext()
    except Exception as e:
        if getattr(sys, 'frozen', False):
            QMessageBox.critical(
                None,
                "初始化错误",
                f"程序初始化失败:\n{e}"
            )
        else:
            print(f"Failed to initialize application: {e}")
        return 1

    # 检查 OCR 可用性（但不阻塞启动）
    # 实际检查在用户使用 OCR 功能时进行

    # 创建主窗口
    main_window = MainWindow(app_context)
    main_window.setWindowTitle("PDF Manager")

    # 检查 OCR（在窗口显示后）
    def check_ocr():
        status = app_context.ocr_service.check_model_status()
        if not status["installed"]:
            app_context.check_ocr_available(main_window)

    # 延迟检查 OCR
    from PyQt6.QtCore import QTimer
    QTimer.singleShot(500, check_ocr)

    # 显示窗口
    main_window.show()

    # 运行应用程序
    result = app.exec()

    # 清理资源
    app_context.cleanup()

    return result


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 3: 运行测试确认主程序可以正常启动**

```bash
python -m src.main &
```

Expected: 程序启动，显示 OCR 检查对话框

- [ ] **Step 4: 提交**

```bash
git add src/main.py src/utils/config.py
git commit -m "feat: 集成 OCR 检查到主程序启动流程

- 使用新的路径工具处理数据路径
- 添加全局异常处理
- 启动时检查 OCR 模型状态
- OCR 不可用时显示设置对话框

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 5: 打包配置

### Task 5: 创建 PyInstaller .spec 文件

**Files:**
- Create: `pdf_manager.spec`
- Modify: `requirements.txt`

- [ ] **Step 1: 更新 requirements.txt**

```diff
PyQt6>=6.4.0
paddlepaddle>=2.5.0
paddleocr>=2.7.0
PyMuPDF>=1.23.0
whoosh>=2.7.4
jieba>=0.42.1
Pillow>=10.0.0
numpy>=1.24.0
pytest>=7.0.0
pytest-qt>=4.2.0
+pyinstaller>=6.0.0
```

- [ ] **Step 2: 创建 pdf_manager.spec**

```python
# -*- mode: python ; coding: utf-8 -*-
"""
PDF Manager PyInstaller 打包配置

使用方法:
    pyinstaller pdf_manager.spec

输出:
    dist/PDF Manager.exe (Windows)
"""

import sys
from pathlib import Path

block_cipher = None

# 所有需要包含的隐藏导入
HIDDEN_IMPORTS = [
    # PaddleOCR 相关
    'paddle',
    'paddleocr',
    'paddle.dataset.image',
    'paddle.fluid',
    'paddle.framework',
    'paddle.nn',
    'paddle.nn.functional',
    'paddle.optimizer',
    'paddle.io',

    # PyMuPDF
    'fitz',
    'pymupdf',

    # Whoosh 搜索
    'whoosh',
    'whoosh.fields',
    'whoosh.index',
    'whoosh.qparser',
    'whoosh.analysis',
    'whoosh.lang',
    'whoosh.lang.chinese',

    # jieba 分词
    'jieba',
    'jieba.analyse',
    'jieba.posseg',

    # 图像处理
    'PIL',
    'PIL.Image',
    'PIL.ImageDraw',
    'PIL.ImageFont',

    # 数值计算
    'numpy',

    # PyQt6
    'PyQt6',
    'PyQt6.QtCore',
    'PyQt6.QtGui',
    'PyQt6.QtWidgets',
    'PyQt6.sip',

    # 项目模块
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
]

# 排除的模块
EXCLUDES = [
    'tkinter',
    'matplotlib',
    'scipy',
    'pandas',
    'IPython',
    'jupyter',
    'notebook',
]

a = Analysis(
    ['src/main.py'],
    pathex=[],
    binaries=[],
    datas=[
        # 如果有资源文件，在这里添加
        # ('resources', 'resources'),
    ],
    hiddenimports=HIDDEN_IMPORTS,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=EXCLUDES,
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
    icon=None,  # 如果有图标，设置图标路径
)
```

- [ ] **Step 3: 更新 GitHub Actions 工作流**

修改 `.github/workflows/build.yml`:

```yaml
name: Build PDF Manager

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:

permissions:
  contents: write

jobs:
  build-windows:
    runs-on: windows-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.10'

      - name: Install dependencies
        run: |
          python -m pip install --upgrade pip
          pip install -r requirements.txt

      - name: Build executable
        env:
          PYTHONPATH: ${{ github.workspace }}
        run: |
          pyinstaller pdf_manager.spec --clean

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: PDF-Manager-Windows
          path: dist/PDF Manager.exe
          retention-days: 30

      - name: Create Release
        if: startsWith(github.ref, 'refs/tags/')
        uses: softprops/action-gh-release@v1
        with:
          files: dist/PDF Manager.exe
          generate_release_notes: true
```

- [ ] **Step 4: 提交**

```bash
git add pdf_manager.spec requirements.txt .github/workflows/build.yml
git commit -m "feat: 添加 PyInstaller .spec 打包配置

- 使用 .spec 文件替代命令行参数
- 添加所有必要的隐藏导入
- 排除不必要的模块减小体积
- 更新 GitHub Actions 使用 .spec 文件

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 6: 完整测试

### Task 6: 执行完整测试计划

**Files:**
- 无新文件，执行测试

- [ ] **Step 1: 运行所有单元测试**

```bash
python -m pytest tests/ -v
```

Expected: 所有测试通过

- [ ] **Step 2: 手动测试 OCR 检查功能**

1. 删除 `~/.paddleocr` 目录（模拟未安装状态）
2. 启动程序：`python -m src.main`
3. 验证显示 OCR 设置对话框
4. 测试「手动下载」按钮显示下载说明
5. 测试「以后再说」按钮

Expected: 功能正常

- [ ] **Step 3: 手动测试核心功能**

1. 添加 PDF 文件
2. 查看 PDF 预览
3. 使用搜索功能
4. 创建/删除文件夹
5. 删除 PDF

Expected: 所有功能正常

- [ ] **Step 4: 打包测试（可选）**

```bash
pyinstaller pdf_manager.spec --clean
```

Expected: 打包成功，生成 `dist/PDF Manager.exe`

- [ ] **Step 5: 最终提交**

```bash
git add -A
git commit -m "test: 完成功能测试

- OCR 检查功能正常
- 所有核心功能正常
- 打包配置正常

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## 测试计划总结

| 序号 | 测试项 | 状态 |
|------|--------|------|
| 1 | OCR 检测-无模型 | [ ] |
| 2 | OCR 检测-有模型 | [ ] |
| 3 | OCR 自动下载 | [ ] |
| 4 | OCR 手动下载 | [ ] |
| 5 | OCR 重新检查 | [ ] |
| 6 | 添加 PDF | [ ] |
| 7 | PDF 预览 | [ ] |
| 8 | 搜索功能 | [ ] |
| 9 | 文件夹管理 | [ ] |
| 10 | 删除 PDF | [ ] |
| 11 | 打包测试 | [ ] |

---

## 注意事项

1. **打包后路径问题**：确保所有文件操作使用 `path_utils.py` 中的函数
2. **OCR 模型位置**：打包后模型可以放在程序目录或用户数据目录
3. **错误处理**：打包后无控制台，所有错误需通过对话框显示
4. **依赖大小**：PaddleOCR 和 PaddlePaddle 较大，打包后可能超过 500MB