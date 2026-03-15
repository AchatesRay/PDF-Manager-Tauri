"""导入对话框"""

from typing import TYPE_CHECKING, Optional, List

from PyQt6.QtWidgets import (
    QDialog,
    QVBoxLayout,
    QHBoxLayout,
    QLabel,
    QProgressBar,
    QListWidget,
    QPushButton,
    QAbstractItemView,
)
from PyQt6.QtCore import Qt, pyqtSignal, QThread

if TYPE_CHECKING:
    from src.core.pdf_manager import PDFManager
    from src.models.schemas import PDF


class ImportWorker(QThread):
    """导入工作线程"""

    progress = pyqtSignal(int, int, str)  # current, total, message
    file_progress = pyqtSignal(int, int, str)  # current_file, total_files, filename
    finished = pyqtSignal(list)  # list of (pdf_id, success, message)

    def __init__(
        self,
        pdf_manager: "PDFManager",
        file_paths: List[str],
        folder_id: Optional[int] = None
    ):
        super().__init__()
        self._pdf_manager = pdf_manager
        self._file_paths = file_paths
        self._folder_id = folder_id

    def run(self) -> None:
        """执行导入"""
        results = []
        total_files = len(self._file_paths)

        for i, file_path in enumerate(self._file_paths):
            # 发送文件进度
            self.file_progress.emit(i + 1, total_files, file_path)

            try:
                status = self._pdf_manager.import_pdf(
                    source_path=file_path,
                    folder_id=self._folder_id,
                    process_ocr=True,
                    progress_callback=lambda c, t, m: self.progress.emit(c, t, m)
                )
                results.append((status.pdf_id, status.result.value == "success", status.message))
            except Exception as e:
                results.append((None, False, str(e)))

        self.finished.emit(results)


class ImportDialog(QDialog):
    """导入对话框

    显示PDF导入进度，支持：
    - 显示导入进度条
    - 显示导入文件列表
    - 显示导入结果
    """

    def __init__(
        self,
        pdf_manager: Optional["PDFManager"] = None,
        parent: Optional[QDialog] = None
    ):
        super().__init__(parent)
        self._pdf_manager = pdf_manager
        self._worker: Optional[ImportWorker] = None
        self._import_results: List = []
        self._total_files: int = 0

        self.setWindowTitle("导入PDF")
        self.setMinimumSize(500, 400)
        self.setModal(True)

        self._setup_ui()

    def _setup_ui(self) -> None:
        """设置UI布局"""
        layout = QVBoxLayout(self)
        layout.setSpacing(12)

        # 文件进度标签
        self._file_progress_label = QLabel("准备导入...")
        layout.addWidget(self._file_progress_label)

        # 文件进度条
        self._file_progress_bar = QProgressBar()
        self._file_progress_bar.setMinimum(0)
        self._file_progress_bar.setMaximum(100)
        self._file_progress_bar.setValue(0)
        layout.addWidget(self._file_progress_bar)

        # OCR进度标签
        self._ocr_progress_label = QLabel("")
        layout.addWidget(self._ocr_progress_label)

        # OCR进度条
        self._ocr_progress_bar = QProgressBar()
        self._ocr_progress_bar.setMinimum(0)
        self._ocr_progress_bar.setMaximum(100)
        self._ocr_progress_bar.setValue(0)
        layout.addWidget(self._ocr_progress_bar)

        # 文件列表
        self._file_list = QListWidget()
        self._file_list.setSelectionMode(QAbstractItemView.SelectionMode.NoSelection)
        layout.addWidget(self._file_list)

        # 统计标签
        self._stats_label = QLabel("")
        layout.addWidget(self._stats_label)

        # 按钮
        button_layout = QHBoxLayout()
        button_layout.addStretch()

        self._close_button = QPushButton("关闭")
        self._close_button.clicked.connect(self._on_close)
        button_layout.addWidget(self._close_button)

        layout.addLayout(button_layout)

    def set_pdf_manager(self, pdf_manager: "PDFManager") -> None:
        """设置PDF管理器"""
        self._pdf_manager = pdf_manager

    def start_import(self, file_paths: List[str], folder_id: Optional[int] = None) -> None:
        """开始导入"""
        if self._pdf_manager is None:
            self._file_progress_label.setText("错误: PDF管理器未设置")
            return

        # 清空列表
        self._file_list.clear()
        self._import_results = []
        self._total_files = len(file_paths)

        # 添加文件到列表
        for file_path in file_paths:
            self._file_list.addItem(file_path)

        # 设置进度条
        self._file_progress_bar.setMaximum(len(file_paths))
        self._file_progress_bar.setValue(0)
        self._ocr_progress_bar.setValue(0)
        self._ocr_progress_label.setText("")

        self._file_progress_label.setText(f"准备导入 {len(file_paths)} 个文件...")

        # 创建并启动工作线程
        self._worker = ImportWorker(self._pdf_manager, file_paths, folder_id)
        self._worker.file_progress.connect(self._on_file_progress)
        self._worker.progress.connect(self._on_ocr_progress)
        self._worker.finished.connect(self._on_finished)
        self._worker.start()

    def _on_file_progress(self, current: int, total: int, filename: str) -> None:
        """文件进度更新"""
        self._file_progress_bar.setValue(current)
        self._file_progress_label.setText(f"正在处理 ({current}/{total}): {filename}")
        # 重置OCR进度条
        self._ocr_progress_bar.setValue(0)
        self._ocr_progress_label.setText("准备OCR处理...")

    def _on_ocr_progress(self, current: int, total: int, message: str) -> None:
        """OCR进度更新"""
        self._ocr_progress_bar.setMaximum(total)
        self._ocr_progress_bar.setValue(current)
        self._ocr_progress_label.setText(message)

    def _on_finished(self, results: List) -> None:
        """导入完成"""
        self._import_results = results

        # 统计结果
        success_count = sum(1 for r in results if r[1])
        error_count = len(results) - success_count

        self._file_progress_label.setText("导入完成")
        self._ocr_progress_label.setText("")
        self._stats_label.setText(f"成功: {success_count} | 失败: {error_count}")

        # 更新列表显示结果
        self._file_list.clear()
        for i, (pdf_id, success, message) in enumerate(results):
            status = "成功" if success else "失败"
            item_text = f"[{status}] {results[i][2] if i < len(results) else '未知'}"
            self._file_list.addItem(item_text)

    def _on_close(self) -> None:
        """关闭对话框"""
        if self._worker is not None and self._worker.isRunning():
            self._worker.terminate()
            self._worker.wait()

        self.reject()

    def closeEvent(self, event) -> None:
        """关闭事件"""
        if self._worker is not None and self._worker.isRunning():
            self._worker.terminate()
            self._worker.wait()
        event.accept()