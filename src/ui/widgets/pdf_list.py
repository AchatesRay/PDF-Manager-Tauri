"""PDF列表组件"""

from typing import TYPE_CHECKING, Optional, List

from PyQt6.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QTableView,
    QLineEdit,
    QPushButton,
    QHeaderView,
    QAbstractItemView,
    QMenu,
    QFileDialog,
)
from PyQt6.QtCore import Qt, pyqtSignal, QAbstractTableModel, QModelIndex
from PyQt6.QtGui import QAction

from src.models.schemas import PDF, PDFStatus

if TYPE_CHECKING:
    from src.core.pdf_manager import PDFManager


class PDFTableModel(QAbstractTableModel):
    """PDF表格数据模型"""

    COLUMNS = ["文件名", "页数", "状态", "大小"]

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._pdfs: List[PDF] = []
        self._filtered_pdfs: List[PDF] = []
        self._search_filter: str = ""

    def setPDFs(self, pdfs: List[PDF]) -> None:
        """设置PDF数据"""
        self.beginResetModel()
        self._pdfs = pdfs
        self._apply_filter()
        self.endResetModel()

    def setSearchFilter(self, filter_text: str) -> None:
        """设置搜索过滤"""
        self.beginResetModel()
        self._search_filter = filter_text.lower()
        self._apply_filter()
        self.endResetModel()

    def _apply_filter(self) -> None:
        """应用搜索过滤"""
        if not self._search_filter:
            self._filtered_pdfs = self._pdfs.copy()
        else:
            self._filtered_pdfs = [
                pdf for pdf in self._pdfs
                if self._search_filter in pdf.filename.lower()
            ]

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:
        return len(self._filtered_pdfs)

    def columnCount(self, parent: QModelIndex = QModelIndex()) -> int:
        return len(self.COLUMNS)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or role != Qt.ItemDataRole.DisplayRole:
            return None

        pdf = self._filtered_pdfs[index.row()]
        col = index.column()

        if col == 0:
            return pdf.filename
        elif col == 1:
            return str(pdf.page_count)
        elif col == 2:
            return self._status_to_string(pdf.status)
        elif col == 3:
            return self._format_file_size(pdf.file_size)

        return None

    def headerData(self, section: int, orientation: Qt.Orientation, role: int = Qt.ItemDataRole.DisplayRole):
        if role != Qt.ItemDataRole.DisplayRole:
            return None

        if orientation == Qt.Orientation.Horizontal:
            return self.COLUMNS[section]

        return None

    def getPdf(self, row: int) -> Optional[PDF]:
        """获取指定行的PDF"""
        if 0 <= row < len(self._filtered_pdfs):
            return self._filtered_pdfs[row]
        return None

    def getPdfId(self, row: int) -> Optional[int]:
        """获取指定行的PDF ID"""
        pdf = self.getPdf(row)
        return pdf.id if pdf else None

    def _status_to_string(self, status: PDFStatus) -> str:
        """状态枚举转字符串"""
        status_map = {
            PDFStatus.PENDING: "待处理",
            PDFStatus.PROCESSING: "处理中",
            PDFStatus.DONE: "已完成",
            PDFStatus.ERROR: "错误",
        }
        return status_map.get(status, "未知")

    def _format_file_size(self, size: int) -> str:
        """格式化文件大小"""
        if size < 1024:
            return f"{size} B"
        elif size < 1024 * 1024:
            return f"{size / 1024:.1f} KB"
        elif size < 1024 * 1024 * 1024:
            return f"{size / (1024 * 1024):.1f} MB"
        else:
            return f"{size / (1024 * 1024 * 1024):.1f} GB"


class PDFListWidget(QWidget):
    """PDF列表组件

    显示PDF列表，支持：
    - 显示PDF列表（文件名、页数、状态、大小）
    - 选择PDF
    - 双击打开
    - 删除PDF

    注意：搜索过滤功能通过 setSearchFilter 方法提供，由外部搜索框调用。
    """

    # 信号：选中PDF变化
    pdf_selected = pyqtSignal(int)  # pdf_id
    # 信号：双击PDF
    pdf_double_clicked = pyqtSignal(int)  # pdf_id
    # 信号：添加PDF按钮点击
    add_pdf_clicked = pyqtSignal()
    # 信号：删除PDF按钮点击
    delete_pdf_clicked = pyqtSignal(int)  # pdf_id

    def __init__(self, pdf_manager: Optional["PDFManager"] = None, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._pdf_manager = pdf_manager
        self._current_folder_id: Optional[int] = None
        self._current_pdf_id: Optional[int] = None

        self._setup_ui()
        self._connect_signals()

    def _setup_ui(self) -> None:
        """设置UI布局"""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(4)

        # 表格视图
        self._table_view = QTableView()
        self._table_view.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self._table_view.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection)
        self._table_view.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self._table_view.setAlternatingRowColors(True)
        self._table_view.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)

        # 设置模型
        self._model = PDFTableModel(self)
        self._table_view.setModel(self._model)

        # 设置列宽
        header = self._table_view.horizontalHeader()
        header.setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        header.setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(3, QHeaderView.ResizeMode.ResizeToContents)

        layout.addWidget(self._table_view)

        # 设置上下文菜单
        self._setup_context_menu()

    def _setup_context_menu(self) -> None:
        """设置右键菜单"""
        self._context_menu = QMenu(self)

        self._open_action = QAction("打开", self)
        self._open_action.triggered.connect(self._on_open_pdf)
        self._context_menu.addAction(self._open_action)

        self._delete_action = QAction("删除", self)
        self._delete_action.triggered.connect(self._on_delete_pdf)
        self._context_menu.addAction(self._delete_action)

    def _connect_signals(self) -> None:
        """连接信号"""
        self._table_view.clicked.connect(self._on_selection_changed)
        self._table_view.doubleClicked.connect(self._on_double_click)
        self._table_view.customContextMenuRequested.connect(self._show_context_menu)

    def set_pdf_manager(self, pdf_manager: "PDFManager") -> None:
        """设置PDF管理器"""
        self._pdf_manager = pdf_manager

    def load_pdfs(self, folder_id: Optional[int] = None) -> None:
        """加载PDF列表"""
        self._current_folder_id = folder_id

        if self._pdf_manager is None:
            return

        if folder_id is None:
            pdfs = self._pdf_manager.get_all_pdfs()
        else:
            pdfs = self._pdf_manager.get_pdfs_by_folder(folder_id)

        self._model.setPDFs(pdfs)

    def refresh(self) -> None:
        """刷新列表"""
        self.load_pdfs(self._current_folder_id)

    def get_selected_pdf_id(self) -> Optional[int]:
        """获取当前选中的PDF ID"""
        return self._current_pdf_id

    def setSearchFilter(self, filter_text: str) -> None:
        """设置搜索过滤（供外部调用）"""
        self._model.setSearchFilter(filter_text)

    def _on_selection_changed(self, index: QModelIndex) -> None:
        """选择变化处理"""
        pdf_id = self._model.getPdfId(index.row())
        if pdf_id is not None:
            self._current_pdf_id = pdf_id
            self.pdf_selected.emit(pdf_id)

    def _on_double_click(self, index: QModelIndex) -> None:
        """双击处理"""
        pdf_id = self._model.getPdfId(index.row())
        if pdf_id is not None:
            self.pdf_double_clicked.emit(pdf_id)

    def _on_open_pdf(self) -> None:
        """打开PDF"""
        if self._current_pdf_id is not None:
            self.pdf_double_clicked.emit(self._current_pdf_id)

    def _on_delete_pdf(self) -> None:
        """删除PDF"""
        if self._current_pdf_id is not None:
            self.delete_pdf_clicked.emit(self._current_pdf_id)

    def _show_context_menu(self, pos) -> None:
        """显示右键菜单"""
        index = self._table_view.indexAt(pos)
        if index.isValid():
            self._table_view.selectRow(index.row())
            self._on_selection_changed(index)
            self._context_menu.exec(self._table_view.viewport().mapToGlobal(pos))