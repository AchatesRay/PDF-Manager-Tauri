"""OCR 设置对话框

用于检测和下载 OCR 模型的对话框。
"""

from typing import Optional

from PyQt6.QtWidgets import (
    QDialog,
    QVBoxLayout,
    QHBoxLayout,
    QLabel,
    QProgressBar,
    QPushButton,
    QStackedWidget,
    QWidget,
    QTextEdit,
    QMessageBox,
)
from PyQt6.QtCore import Qt, pyqtSignal, QThread

from src.services.ocr_service import OCRService


class DownloadWorker(QThread):
    """后台下载模型的工作线程"""

    progress = pyqtSignal(int, int, str)  # current, total, message
    finished = pyqtSignal(dict)  # result dict with success, error, downloaded

    def __init__(self, ocr_service: OCRService, parent: Optional[QThread] = None):
        super().__init__(parent)
        self._ocr_service = ocr_service

    def run(self) -> None:
        """执行下载"""
        try:
            # 发送开始信号
            self.progress.emit(0, 3, "正在检查模型状态...")

            # 获取模型状态
            status = self._ocr_service.check_model_status()
            missing_count = len(status.get("missing_models", []))

            if missing_count == 0:
                # 所有模型已存在
                self.progress.emit(3, 3, "所有模型已存在，无需下载")
                self.finished.emit({
                    "success": True,
                    "error": None,
                    "downloaded": status.get("installed_models", []),
                })
                return

            # 开始下载
            self.progress.emit(0, missing_count, "正在下载模型...")

            # 调用 OCRService 的下载方法
            result = self._ocr_service.download_models()

            # 发送进度更新
            if result.get("success"):
                self.progress.emit(missing_count, missing_count, "下载完成")
            else:
                self.progress.emit(0, missing_count, "下载失败")

            self.finished.emit(result)

        except Exception as e:
            self.finished.emit({
                "success": False,
                "error": str(e),
                "downloaded": [],
            })


class OCRSetupDialog(QDialog):
    """OCR 设置对话框

    显示 OCR 模型状态并支持：
    - 自动下载（推荐）
    - 手动下载
    - 以后再说
    """

    # 视图索引
    VIEW_ACTION = 0    # 操作按钮视图
    VIEW_PROGRESS = 1  # 下载进度视图
    VIEW_MANUAL = 2    # 手动下载说明视图

    def __init__(self, parent: Optional[QDialog] = None):
        super().__init__(parent)
        self._ocr_service = OCRService()
        self._worker: Optional[DownloadWorker] = None

        self.setWindowTitle("OCR 模型设置")
        self.setMinimumSize(500, 350)
        self.setModal(True)

        self._setup_ui()
        self._check_status()

    def _setup_ui(self) -> None:
        """设置 UI 布局"""
        layout = QVBoxLayout(self)
        layout.setSpacing(16)

        # 状态标签
        self._status_label = QLabel("正在检查模型状态...")
        self._status_label.setWordWrap(True)
        layout.addWidget(self._status_label)

        # 堆叠视图
        self._stacked_widget = QStackedWidget()
        layout.addWidget(self._stacked_widget, 1)

        # 创建三个视图
        self._stacked_widget.addWidget(self._create_action_view())
        self._stacked_widget.addWidget(self._create_progress_view())
        self._stacked_widget.addWidget(self._create_manual_view())

    def _create_action_view(self) -> QWidget:
        """创建操作按钮视图"""
        widget = QWidget()
        layout = QVBoxLayout(widget)
        layout.setSpacing(12)

        # 说明文本
        info_label = QLabel(
            "OCR 模型用于识别 PDF 文档中的文字。"
            "首次使用需要下载模型文件（约 50MB）。"
        )
        info_label.setWordWrap(True)
        layout.addWidget(info_label)

        layout.addStretch()

        # 自动下载按钮（推荐）
        self._auto_download_button = QPushButton("自动下载（推荐）")
        self._auto_download_button.setDefault(True)
        self._auto_download_button.clicked.connect(self._on_auto_download)
        layout.addWidget(self._auto_download_button)

        # 手动下载按钮
        self._manual_download_button = QPushButton("手动下载")
        self._manual_download_button.clicked.connect(self._on_manual_download)
        layout.addWidget(self._manual_download_button)

        # 以后再说按钮
        self._later_button = QPushButton("以后再说")
        self._later_button.clicked.connect(self._on_later)
        layout.addWidget(self._later_button)

        layout.addStretch()

        # 重新检查按钮
        self._recheck_button = QPushButton("重新检查")
        self._recheck_button.clicked.connect(self._recheck_models)
        layout.addWidget(self._recheck_button)

        return widget

    def _create_progress_view(self) -> QWidget:
        """创建下载进度视图"""
        widget = QWidget()
        layout = QVBoxLayout(widget)
        layout.setSpacing(12)

        # 进度信息
        self._progress_label = QLabel("正在下载...")
        layout.addWidget(self._progress_label)

        # 进度条
        self._progress_bar = QProgressBar()
        self._progress_bar.setMinimum(0)
        self._progress_bar.setMaximum(100)
        self._progress_bar.setValue(0)
        layout.addWidget(self._progress_bar)

        layout.addStretch()

        # 取消按钮
        self._cancel_button = QPushButton("取消")
        self._cancel_button.clicked.connect(self._on_cancel_download)
        layout.addWidget(self._cancel_button)

        return widget

    def _create_manual_view(self) -> QWidget:
        """创建手动下载说明视图"""
        widget = QWidget()
        layout = QVBoxLayout(widget)
        layout.setSpacing(12)

        # 说明标题
        title_label = QLabel("手动下载说明")
        title_label.setStyleSheet("font-weight: bold;")
        layout.addWidget(title_label)

        # 说明文本框
        self._manual_text = QTextEdit()
        self._manual_text.setReadOnly(True)
        self._manual_text.setPlaceholderText("正在获取下载信息...")
        layout.addWidget(self._manual_text)

        # 返回按钮
        button_layout = QHBoxLayout()
        button_layout.addStretch()

        self._back_button = QPushButton("返回")
        self._back_button.clicked.connect(self._on_back_to_action)
        button_layout.addWidget(self._back_button)

        layout.addLayout(button_layout)

        return widget

    def _check_status(self) -> None:
        """检查 OCR 模型状态"""
        try:
            status = self._ocr_service.check_model_status()

            if status.get("installed"):
                self.set_status("OCR 模型已安装，可以正常使用。")
                self._update_ui_for_installed()
            else:
                missing = status.get("missing_models", [])
                missing_text = ", ".join(missing) if missing else "未知"
                self.set_status(f"检测到缺失模型：{missing_text}")
                self._update_ui_for_not_installed()
        except Exception as e:
            self.set_status(f"检查模型状态时出错：{str(e)}")

    def set_status(self, status: str) -> None:
        """设置状态文本"""
        self._status_label.setText(status)

    def _update_ui_for_installed(self) -> None:
        """更新 UI 为已安装状态"""
        self._auto_download_button.setText("重新下载")
        self._manual_download_button.setEnabled(True)
        self._later_button.setText("关闭")

    def _update_ui_for_not_installed(self) -> None:
        """更新 UI 为未安装状态"""
        self._auto_download_button.setText("自动下载（推荐）")
        self._manual_download_button.setEnabled(True)
        self._later_button.setText("以后再说")

    def _on_auto_download(self) -> None:
        """开始自动下载"""
        # 切换到进度视图
        self._stacked_widget.setCurrentIndex(self.VIEW_PROGRESS)
        self._progress_label.setText("准备下载...")
        self._progress_bar.setValue(0)

        # 创建并启动工作线程
        self._worker = DownloadWorker(self._ocr_service)
        self._worker.progress.connect(self._on_download_progress)
        self._worker.finished.connect(self._on_download_finished)
        self._worker.start()

    def _on_download_progress(self, current: int, total: int, message: str) -> None:
        """下载进度更新"""
        self._progress_label.setText(message)
        if total > 0:
            self._progress_bar.setMaximum(total)
            self._progress_bar.setValue(current)

    def _on_download_finished(self, result: dict) -> None:
        """下载完成"""
        self._worker = None

        if result.get("success"):
            downloaded = result.get("downloaded", [])
            self.set_status(f"下载完成！已下载 {len(downloaded)} 个模型。")
            self._update_ui_for_installed()
            self._stacked_widget.setCurrentIndex(self.VIEW_ACTION)
            QMessageBox.information(
                self,
                "下载完成",
                "OCR 模型已成功下载，现在可以正常使用 OCR 功能。"
            )
        else:
            error = result.get("error", "未知错误")
            self.set_status(f"下载失败：{error}")
            self._stacked_widget.setCurrentIndex(self.VIEW_ACTION)
            QMessageBox.warning(
                self,
                "下载失败",
                f"下载模型时发生错误：{error}\n\n您可以尝试手动下载或稍后重试。"
            )

    def _on_cancel_download(self) -> None:
        """取消下载"""
        if self._worker is not None and self._worker.isRunning():
            self._worker.terminate()
            self._worker.wait()
            self._worker = None

        self.set_status("下载已取消")
        self._stacked_widget.setCurrentIndex(self.VIEW_ACTION)

    def _on_manual_download(self) -> None:
        """显示手动下载说明"""
        # 获取手动下载信息
        try:
            info = self._ocr_service.get_manual_download_info()
            models = info.get("models", [])
            model_dir = info.get("model_dir", "")

            # 构建说明文本
            text_parts = [
                "手动下载 OCR 模型：\n",
                "请从以下链接下载模型文件：\n"
            ]

            for model in models:
                name = model.get("name", "未知模型")
                url = model.get("url", "")
                text_parts.append(f"\n{name}:")
                text_parts.append(f"  {url}")

            text_parts.append(f"\n\n下载完成后，请将解压后的文件夹放置到以下目录：")
            text_parts.append(f"\n{model_dir}")
            text_parts.append("\n\n注意：下载的文件为 .tar 格式，需要解压后使用。")

            self._manual_text.setText("".join(text_parts))

        except Exception as e:
            self._manual_text.setText(f"获取下载信息失败：{str(e)}")

        # 切换到手动下载视图
        self._stacked_widget.setCurrentIndex(self.VIEW_MANUAL)

    def _on_later(self) -> None:
        """以后再说 - 关闭对话框"""
        # 检查模型状态，如果未安装则提示
        status = self._ocr_service.check_model_status()
        if not status.get("installed"):
            result = QMessageBox.question(
                self,
                "确认",
                "OCR 模型尚未安装，OCR 功能将不可用。\n\n确定要跳过安装吗？",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No
            )
            if result == QMessageBox.StandardButton.No:
                return

        self.reject()

    def _recheck_models(self) -> None:
        """重新检查模型"""
        self.set_status("正在检查模型状态...")
        self._check_status()

    def _on_back_to_action(self) -> None:
        """返回操作视图"""
        self._stacked_widget.setCurrentIndex(self.VIEW_ACTION)

    def closeEvent(self, event) -> None:
        """关闭事件"""
        # 确保下载线程已停止
        if self._worker is not None and self._worker.isRunning():
            self._worker.terminate()
            self._worker.wait()
        event.accept()

    def is_model_installed(self) -> bool:
        """检查模型是否已安装"""
        status = self._ocr_service.check_model_status()
        return status.get("installed", False)