"""文件夹树组件"""

from typing import TYPE_CHECKING, Optional, List

from PyQt6.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QTreeView,
    QLineEdit,
    QPushButton,
    QMenu,
    QMessageBox,
    QInputDialog,
    QAbstractItemView,
)
from PyQt6.QtCore import Qt, pyqtSignal, QAbstractItemModel, QModelIndex
from PyQt6.QtGui import QAction, QIcon

from src.models.schemas import Folder

if TYPE_CHECKING:
    from src.core.folder_manager import FolderManager


class FolderTreeItem:
    """文件夹树项目"""

    def __init__(self, folder: Optional[Folder] = None, parent: Optional["FolderTreeItem"] = None):
        self._folder = folder
        self._parent = parent
        self._children: List["FolderTreeItem"] = []

    def appendChild(self, child: "FolderTreeItem") -> None:
        self._children.append(child)

    def child(self, row: int) -> Optional["FolderTreeItem"]:
        if 0 <= row < len(self._children):
            return self._children[row]
        return None

    def childCount(self) -> int:
        return len(self._children)

    def row(self) -> int:
        if self._parent is not None:
            return self._parent._children.index(self)
        return 0

    def parentItem(self) -> Optional["FolderTreeItem"]:
        return self._parent

    def folder(self) -> Optional[Folder]:
        return self._folder

    def folderId(self) -> int:
        return self._folder.id if self._folder else -1


class FolderTreeModel(QAbstractItemModel):
    """文件夹树数据模型"""

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._root_item = FolderTreeItem()
        self._folder_items: dict[int, FolderTreeItem] = {}

    def setFolders(self, folders: List[Folder]) -> None:
        """设置文件夹数据"""
        from src.utils.logger import get_logger
        logger = get_logger("folder_tree")

        self.beginResetModel()
        self._root_item = FolderTreeItem()
        self._folder_items = {}

        # 创建所有项目
        for folder in folders:
            item = FolderTreeItem(folder)
            self._folder_items[folder.id] = item
            logger.debug(f"创建文件夹项: ID={folder.id}, 名称='{folder.name}', parent_id={folder.parent_id}")

        # 构建树结构
        for folder in folders:
            item = self._folder_items[folder.id]
            if folder.parent_id is None:
                self._root_item.appendChild(item)
                logger.debug(f"添加根节点: ID={folder.id}, 名称='{folder.name}'")
            elif folder.parent_id in self._folder_items:
                parent_item = self._folder_items[folder.parent_id]
                parent_item.appendChild(item)

        self.endResetModel()

    def index(self, row: int, column: int, parent: QModelIndex = QModelIndex()) -> QModelIndex:
        if not self.hasIndex(row, column, parent):
            return QModelIndex()

        parent_item = self._root_item if not parent.isValid() else parent.internalPointer()

        child_item = parent_item.child(row)
        if child_item is not None:
            return self.createIndex(row, column, child_item)
        return QModelIndex()

    def parent(self, child: QModelIndex) -> QModelIndex:
        if not child.isValid():
            return QModelIndex()

        child_item = child.internalPointer()
        parent_item = child_item.parentItem()

        if parent_item == self._root_item or parent_item is None:
            return QModelIndex()

        return self.createIndex(parent_item.row(), 0, parent_item)

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:
        if parent.column() > 0:
            return 0

        parent_item = self._root_item if not parent.isValid() else parent.internalPointer()
        return parent_item.childCount()

    def columnCount(self, parent: QModelIndex = QModelIndex()) -> int:
        return 1

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole):
        if not index.isValid():
            return None

        item: FolderTreeItem = index.internalPointer()
        folder = item.folder()

        if role == Qt.ItemDataRole.DisplayRole and folder:
            return folder.name

        return None

    def getFolderId(self, index: QModelIndex) -> int:
        """获取索引对应的文件夹ID"""
        if not index.isValid():
            return -1
        item: FolderTreeItem = index.internalPointer()
        return item.folderId()


class FolderTreeWidget(QWidget):
    """文件夹树组件

    显示文件夹树结构，支持：
    - 文件夹选择
    - 文件夹创建
    - 文件夹重命名
    - 文件夹删除
    """

    # 信号：选中文件夹变化
    folder_selected = pyqtSignal(int)  # folder_id
    # 信号：文件夹创建完成
    folder_created = pyqtSignal(int)  # folder_id
    # 信号：文件夹删除完成
    folder_deleted = pyqtSignal(int)  # folder_id

    def __init__(self, folder_manager: Optional["FolderManager"] = None, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._folder_manager = folder_manager
        self._current_folder_id: Optional[int] = None

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

        # 新建文件夹按钮
        self._add_button = QPushButton("+")
        self._add_button.setFixedSize(28, 28)
        self._add_button.setToolTip("新建文件夹")
        toolbar_layout.addWidget(self._add_button)

        toolbar_layout.addStretch()

        layout.addWidget(toolbar)

        # 树视图
        self._tree_view = QTreeView()
        self._tree_view.setHeaderHidden(True)
        self._tree_view.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self._tree_view.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection)
        self._tree_view.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)

        # 设置模型
        self._model = FolderTreeModel(self)
        self._tree_view.setModel(self._model)

        layout.addWidget(self._tree_view)

        # 设置上下文菜单
        self._setup_context_menu()

    def _setup_context_menu(self) -> None:
        """设置右键菜单"""
        self._context_menu = QMenu(self)

        self._create_action = QAction("新建子文件夹", self)
        self._create_action.triggered.connect(self._on_create_folder)
        self._context_menu.addAction(self._create_action)

        self._rename_action = QAction("重命名", self)
        self._rename_action.triggered.connect(self._on_rename_folder)
        self._context_menu.addAction(self._rename_action)

        self._context_menu.addSeparator()

        self._delete_action = QAction("删除", self)
        self._delete_action.triggered.connect(self._on_delete_folder)
        self._context_menu.addAction(self._delete_action)

    def _connect_signals(self) -> None:
        """连接信号"""
        self._tree_view.clicked.connect(self._on_selection_changed)
        self._tree_view.customContextMenuRequested.connect(self._show_context_menu)
        self._add_button.clicked.connect(self._on_create_root_folder)

    def set_folder_manager(self, folder_manager: "FolderManager") -> None:
        """设置文件夹管理器"""
        self._folder_manager = folder_manager

    def load_folders(self) -> None:
        """加载文件夹数据"""
        if self._folder_manager is None:
            return

        folders = self._folder_manager.get_all_folders()
        self._model.setFolders(folders)
        self._tree_view.expandAll()

    def get_selected_folder_id(self) -> Optional[int]:
        """获取当前选中的文件夹ID"""
        return self._current_folder_id

    def _on_selection_changed(self, index: QModelIndex) -> None:
        """选择变化处理"""
        folder_id = self._model.getFolderId(index)
        if folder_id > 0:
            self._current_folder_id = folder_id
            self.folder_selected.emit(folder_id)
        else:
            self._current_folder_id = None
            self.folder_selected.emit(0)  # 0 表示根目录

    def _on_create_root_folder(self) -> None:
        """创建根文件夹"""
        self._create_folder_internal(None)

    def _on_create_folder(self) -> None:
        """创建子文件夹"""
        # 使用当前选中的文件夹作为父文件夹
        # _show_context_menu 已经在显示菜单前更新了 _current_folder_id
        if self._current_folder_id is None:
            QMessageBox.warning(self, "提示", "请先选择一个父文件夹")
            return
        self._create_folder_internal(self._current_folder_id)

    def _create_folder_internal(self, parent_id: Optional[int]) -> None:
        """内部创建文件夹方法"""
        if self._folder_manager is None:
            return

        # 获取父文件夹名称用于提示
        parent_name = "根目录"
        if parent_id is not None:
            parent_folder = self._folder_manager.get_folder(parent_id)
            if parent_folder:
                parent_name = parent_folder.name

        name, ok = QInputDialog.getText(
            self,
            "新建文件夹",
            f"在 '{parent_name}' 下创建文件夹，名称:",
            QLineEdit.EchoMode.Normal,
            ""
        )

        if ok and name.strip():
            try:
                folder = self._folder_manager.create_folder(name.strip(), parent_id)
                self.load_folders()
                self.folder_created.emit(folder.id)
            except ValueError as e:
                QMessageBox.warning(self, "错误", str(e))

    def _on_rename_folder(self) -> None:
        """重命名文件夹"""
        if self._folder_manager is None or self._current_folder_id is None:
            return

        folder = self._folder_manager.get_folder(self._current_folder_id)
        if folder is None:
            return

        name, ok = QInputDialog.getText(
            self,
            "重命名文件夹",
            "新名称:",
            QLineEdit.EchoMode.Normal,
            folder.name
        )

        if ok and name.strip():
            self._folder_manager.rename_folder(self._current_folder_id, name.strip())
            self.load_folders()

    def _on_delete_folder(self) -> None:
        """删除文件夹"""
        if self._folder_manager is None or self._current_folder_id is None:
            return

        folder = self._folder_manager.get_folder(self._current_folder_id)
        if folder is None:
            return

        reply = QMessageBox.question(
            self,
            "确认删除",
            f"确定要删除文件夹 \"{folder.name}\" 吗？\n这将同时删除其中的所有PDF文件。",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.No
        )

        if reply == QMessageBox.StandardButton.Yes:
            try:
                folder_id = self._current_folder_id
                self._folder_manager.delete_folder(folder_id, delete_contents=True)
                self._current_folder_id = None
                self.load_folders()
                self.folder_deleted.emit(folder_id)
            except ValueError as e:
                QMessageBox.warning(self, "错误", str(e))

    def _show_context_menu(self, pos) -> None:
        """显示右键菜单"""
        index = self._tree_view.indexAt(pos)
        if index.isValid():
            # 获取右键点击位置的文件夹ID
            folder_id = self._model.getFolderId(index)
            if folder_id > 0:
                # 更新当前操作的文件夹ID（用于创建子文件夹等操作）
                # 但不改变选中状态，保持用户之前的选中
                self._current_folder_id = folder_id
                # 显示菜单
                self._context_menu.exec(self._tree_view.viewport().mapToGlobal(pos))