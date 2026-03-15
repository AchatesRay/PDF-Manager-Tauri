"""PDF预览组件"""

import os
import numpy as np
from typing import TYPE_CHECKING, Optional

from PyQt6.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QScrollArea,
    QFileDialog,
    QMessageBox,
)
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtGui import QPixmap, QImage

if TYPE_CHECKING:
    from src.core.pdf_manager import PDFManager
    from src.services.pdf_service import PDFService
    from src.models.schemas import PDF


class PDFViewerWidget(QWidget):
    """PDF预览组件

    显示PDF页面预览，支持：
    - 页面渲染
    - 前后翻页
    - 外部打开
    - 在文件夹中显示
    """

    # 信号：外部打开按钮点击
    open_external_clicked = pyqtSignal(str)  # file_path
    # 信号：在文件夹中显示按钮点击
    show_in_folder_clicked = pyqtSignal(str)  # file_path

    def __init__(
        self,
        pdf_manager: Optional["PDFManager"] = None,
        pdf_service: Optional["PDFService"] = None,
        parent: Optional[QWidget] = None
    ):
        super().__init__(parent)
        self._pdf_manager = pdf_manager
        self._pdf_service = pdf_service
        self._current_pdf: Optional["PDF"] = None
        self._current_page: int = 0
        self._total_pages: int = 0

        self._setup_ui()
        self._connect_signals()

    def _setup_ui(self) -> None:
        """设置UI布局"""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(4)

        # 工具栏
        toolbar = QWidget()
        toolbar_layout = QHBoxLayout(toolbar)
        toolbar_layout.setContentsMargins(0, 0, 0, 0)
        toolbar_layout.setSpacing(4)

        # 上一页按钮
        self._prev_button = QPushButton("<")
        self._prev_button.setFixedSize(28, 28)
        self._prev_button.setToolTip("上一页")
        self._prev_button.setEnabled(False)
        toolbar_layout.addWidget(self._prev_button)

        # 页码显示
        self._page_label = QLabel("0 / 0")
        self._page_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._page_label.setMinimumWidth(80)
        toolbar_layout.addWidget(self._page_label)

        # 下一页按钮
        self._next_button = QPushButton(">")
        self._next_button.setFixedSize(28, 28)
        self._next_button.setToolTip("下一页")
        self._next_button.setEnabled(False)
        toolbar_layout.addWidget(self._next_button)

        toolbar_layout.addStretch()

        # 外部打开按钮
        self._open_external_button = QPushButton("打开")
        self._open_external_button.setToolTip("使用外部程序打开")
        self._open_external_button.setEnabled(False)
        toolbar_layout.addWidget(self._open_external_button)

        # 在文件夹中显示按钮
        self._show_in_folder_button = QPushButton("显示")
        self._show_in_folder_button.setToolTip("在文件夹中显示")
        self._show_in_folder_button.setEnabled(False)
        toolbar_layout.addWidget(self._show_in_folder_button)

        layout.addWidget(toolbar)

        # 预览区域
        scroll_area = QScrollArea()
        scroll_area.setWidgetResizable(True)
        scroll_area.setAlignment(Qt.AlignmentFlag.AlignCenter)

        self._preview_label = QLabel()
        self._preview_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._preview_label.setStyleSheet("background-color: #f0f0f0;")
        self._preview_label.setMinimumSize(200, 300)
        self._preview_label.setText("未选择PDF")

        scroll_area.setWidget(self._preview_label)
        layout.addWidget(scroll_area)

    def _connect_signals(self) -> None:
        """连接信号"""
        self._prev_button.clicked.connect(self._on_prev_page)
        self._next_button.clicked.connect(self._on_next_page)
        self._open_external_button.clicked.connect(self._on_open_external)
        self._show_in_folder_button.clicked.connect(self._on_show_in_folder)

    def set_pdf_manager(self, pdf_manager: "PDFManager") -> None:
        """设置PDF管理器"""
        self._pdf_manager = pdf_manager

    def set_pdf_service(self, pdf_service: "PDFService") -> None:
        """设置PDF服务"""
        self._pdf_service = pdf_service

    def load_pdf(self, pdf_id: int) -> None:
        """加载PDF进行预览"""
        if self._pdf_manager is None:
            return

        self._current_pdf = self._pdf_manager.get_pdf(pdf_id)
        if self._current_pdf is None:
            self.clear()
            return

        self._total_pages = self._current_pdf.page_count
        self._current_page = 0

        # 更新UI状态
        self._update_page_label()
        self._update_navigation_buttons()
        self._open_external_button.setEnabled(True)
        self._show_in_folder_button.setEnabled(True)

        # 渲染第一页
        self._render_current_page()

    def clear(self) -> None:
        """清空预览"""
        self._current_pdf = None
        self._current_page = 0
        self._total_pages = 0
        self._preview_label.clear()
        self._preview_label.setText("未选择PDF")
        self._update_page_label()
        self._update_navigation_buttons()
        self._open_external_button.setEnabled(False)
        self._show_in_folder_button.setEnabled(False)

    def _render_current_page(self) -> None:
        """渲染当前页面"""
        if self._pdf_service is None or self._current_pdf is None:
            return

        if self._current_page < 0 or self._current_page >= self._total_pages:
            return

        try:
            # 渲染页面为图像
            img = self._pdf_service.render_page_to_image(
                self._current_pdf.file_path,
                self._current_page,
                dpi=100
            )

            if img is not None:
                # 确保图像是RGB模式
                if img.mode != "RGB":
                    img = img.convert("RGB")

                # 转换PIL Image为QPixmap
                # 使用正确的方法转换图像数据
                import numpy as np
                img_array = np.array(img)

                height, width, channel = img_array.shape
                bytes_per_line = 3 * width

                qimage = QImage(
                    img_array.data,
                    width,
                    height,
                    bytes_per_line,
                    QImage.Format.Format_RGB888
                )
                pixmap = QPixmap.fromImage(qimage.copy())  # 使用copy()确保数据有效

                # 缩放以适应预览区域
                scaled_pixmap = pixmap.scaled(
                    self._preview_label.size(),
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.SmoothTransformation
                )
                self._preview_label.setPixmap(scaled_pixmap)
            else:
                self._preview_label.setText("无法渲染页面")

        except Exception as e:
            self._preview_label.setText(f"渲染错误: {str(e)}")

    def _update_page_label(self) -> None:
        """更新页码标签"""
        if self._current_pdf is not None:
            self._page_label.setText(f"{self._current_page + 1} / {self._total_pages}")
        else:
            self._page_label.setText("0 / 0")

    def _update_navigation_buttons(self) -> None:
        """更新导航按钮状态"""
        has_pdf = self._current_pdf is not None and self._total_pages > 0

        self._prev_button.setEnabled(has_pdf and self._current_page > 0)
        self._next_button.setEnabled(has_pdf and self._current_page < self._total_pages - 1)

    def _on_prev_page(self) -> None:
        """上一页"""
        if self._current_page > 0:
            self._current_page -= 1
            self._update_page_label()
            self._update_navigation_buttons()
            self._render_current_page()

    def _on_next_page(self) -> None:
        """下一页"""
        if self._current_page < self._total_pages - 1:
            self._current_page += 1
            self._update_page_label()
            self._update_navigation_buttons()
            self._render_current_page()

    def _on_open_external(self) -> None:
        """外部打开"""
        if self._current_pdf is not None:
            file_path = self._current_pdf.file_path
            if os.path.exists(file_path):
                self.open_external_clicked.emit(file_path)

    def _on_show_in_folder(self) -> None:
        """在文件夹中显示"""
        if self._current_pdf is not None:
            file_path = self._current_pdf.file_path
            if os.path.exists(file_path):
                self.show_in_folder_clicked.emit(file_path)

    def resizeEvent(self, event) -> None:
        """窗口大小变化事件"""
        super().resizeEvent(event)
        # 重新渲染当前页面以适应新尺寸
        if self._current_pdf is not None:
            self._render_current_page()