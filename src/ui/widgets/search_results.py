"""搜索结果组件"""

from typing import TYPE_CHECKING, Optional, List

from PyQt6.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QListWidget,
    QListWidgetItem,
    QLabel,
    QHBoxLayout,
)
from PyQt6.QtCore import Qt, pyqtSignal

from src.services.index_service import SearchResult

if TYPE_CHECKING:
    from src.core.search_service import GroupedSearchResult


class SearchResultItem(QWidget):
    """单个搜索结果项组件"""

    clicked = pyqtSignal(int, int)  # pdf_id, page_number

    def __init__(self, result: SearchResult, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._result = result
        self._setup_ui()

    def _setup_ui(self) -> None:
        layout = QVBoxLayout(self)
        layout.setContentsMargins(8, 4, 8, 4)
        layout.setSpacing(2)

        # 文件名和页码
        header_layout = QHBoxLayout()
        self._filename_label = QLabel(self._result.filename)
        self._filename_label.setStyleSheet("font-weight: bold;")
        header_layout.addWidget(self._filename_label)

        self._page_label = QLabel(f"第{self._result.page_number}页")
        self._page_label.setStyleSheet("color: #666;")
        header_layout.addWidget(self._page_label)
        header_layout.addStretch()

        layout.addLayout(header_layout)

        # 摘要
        self._snippet_label = QLabel(self._result.snippet[:100] + "..." if len(self._result.snippet) > 100 else self._result.snippet)
        self._snippet_label.setWordWrap(True)
        self._snippet_label.setStyleSheet("color: #444; font-size: 11px;")
        layout.addWidget(self._snippet_label)

    def mousePressEvent(self, event) -> None:
        """点击事件"""
        self.clicked.emit(self._result.pdf_id, self._result.page_number)


class SearchResultsWidget(QWidget):
    """搜索结果列表组件

    显示内容搜索结果，每项包含文件名、页码和内容摘要。
    """

    # 信号：点击搜索结果
    result_clicked = pyqtSignal(int, int)  # pdf_id, page_number

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._results: List[SearchResult] = []
        self._setup_ui()

    def _setup_ui(self) -> None:
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        # 结果数量标签
        self._count_label = QLabel("搜索结果: 0 项")
        self._count_label.setStyleSheet("padding: 4px; background: #f0f0f0;")
        layout.addWidget(self._count_label)

        # 结果列表
        self._list_widget = QListWidget()
        self._list_widget.setSelectionMode(QListWidget.SelectionMode.SingleSelection)
        layout.addWidget(self._list_widget)

        # 空状态提示
        self._empty_label = QLabel("输入关键词搜索PDF内容")
        self._empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._empty_label.setStyleSheet("color: #999; padding: 20px;")
        self._list_widget.setVisible(False)
        layout.addWidget(self._empty_label)

    def set_results(self, results: List[SearchResult]) -> None:
        """设置搜索结果"""
        self._results = results
        self._list_widget.clear()

        if results:
            self._count_label.setText(f"搜索结果: {len(results)} 项")
            self._list_widget.setVisible(True)
            self._empty_label.setVisible(False)

            for result in results:
                item = QListWidgetItem(self._list_widget)
                widget = SearchResultItem(result)
                item.setSizeHint(widget.sizeHint())
                self._list_widget.addItem(item)
                self._list_widget.setItemWidget(item, widget)
                # 连接点击信号
                widget.clicked.connect(self._on_result_clicked)
        else:
            self._count_label.setText("搜索结果: 0 项")
            self._list_widget.setVisible(False)
            self._empty_label.setVisible(True)
            self._empty_label.setText("未找到匹配结果")

    def clear_results(self) -> None:
        """清空搜索结果"""
        self._results = []
        self._list_widget.clear()
        self._count_label.setText("搜索结果: 0 项")
        self._list_widget.setVisible(False)
        self._empty_label.setVisible(True)
        self._empty_label.setText("输入关键词搜索PDF内容")

    def _on_result_clicked(self, pdf_id: int, page_number: int) -> None:
        """处理结果点击"""
        self.result_clicked.emit(pdf_id, page_number)