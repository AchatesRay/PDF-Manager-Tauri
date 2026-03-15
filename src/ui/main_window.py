"""主窗口模块"""

from __future__ import annotations

from typing import TYPE_CHECKING, Optional

from PyQt6.QtWidgets import (
    QMainWindow,
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QSplitter,
    QLabel,
    QLineEdit,
    QStatusBar,
    QMenuBar,
    QMenu,
    QToolBar,
    QMessageBox,
    QFileDialog,
)
from PyQt6.QtCore import Qt, pyqtSignal, QUrl
from PyQt6.QtGui import QAction, QKeySequence, QShortcut, QDesktopServices

from src.ui.widgets import FolderTreeWidget, PDFListWidget, PDFViewerWidget
from src.ui.dialogs import SettingsDialog, ImportDialog

if TYPE_CHECKING:
    from src.main import ApplicationContext


class SearchBarWidget(QWidget):
    """搜索栏组件"""

    # 信号：搜索请求
    search_requested = pyqtSignal(str)  # query

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(4)

        self._search_input = QLineEdit()
        self._search_input.setPlaceholderText("搜索PDF内容...")
        self._search_input.returnPressed.connect(self._on_search)

        layout.addWidget(self._search_input)

    def _on_search(self) -> None:
        """搜索触发"""
        query = self._search_input.text().strip()
        if query:
            self.search_requested.emit(query)


class MainWindow(QMainWindow):
    """主窗口类

    应用程序的主界面，整合所有UI组件。

    布局结构:
    +------------------+------------------------+
    |                  |      搜索栏             |
    |    文件夹树      +------------------------+
    |                  |      PDF列表           |
    |                  +------------------------+
    |                  |      PDF预览           |
    +------------------+------------------------+
    |                  状态栏                   |
    +------------------+------------------------+
    """

    def __init__(self, app_context: ApplicationContext, parent: Optional[QWidget] = None):
        """初始化主窗口

        Args:
            app_context: 应用上下文，包含所有服务和配置
            parent: 父组件
        """
        super().__init__(parent)
        self._app_context = app_context

        # 当前状态
        self._current_folder_id: Optional[int] = None
        self._current_pdf_id: Optional[int] = None

        # 设置窗口
        self.setWindowTitle("PDF Manager")
        self.setMinimumSize(1024, 768)
        self.resize(1280, 800)

        # 初始化UI
        self._setup_ui()
        self._setup_menu()
        self._setup_statusbar()
        self._setup_toolbar()
        self._connect_signals()

    def _setup_ui(self) -> None:
        """设置UI布局"""
        # 创建中央组件
        central_widget = QWidget()
        self.setCentralWidget(central_widget)

        # 主布局
        main_layout = QHBoxLayout(central_widget)
        main_layout.setContentsMargins(4, 4, 4, 4)
        main_layout.setSpacing(4)

        # 创建分割器
        splitter = QSplitter(Qt.Orientation.Horizontal)
        main_layout.addWidget(splitter)

        # 左侧：文件夹树
        self._folder_tree = FolderTreeWidget(
            folder_manager=self._app_context.folder_manager
        )
        splitter.addWidget(self._folder_tree)

        # 右侧：垂直分割器（搜索栏、PDF列表、预览）
        right_splitter = QSplitter(Qt.Orientation.Vertical)
        splitter.addWidget(right_splitter)

        # 右侧上方：搜索栏
        self._search_bar = SearchBarWidget()
        right_splitter.addWidget(self._search_bar)

        # 右侧中间：PDF列表
        self._pdf_list = PDFListWidget(
            pdf_manager=self._app_context.pdf_manager
        )
        right_splitter.addWidget(self._pdf_list)

        # 右侧下方：PDF预览
        self._pdf_viewer = PDFViewerWidget(
            pdf_manager=self._app_context.pdf_manager,
            pdf_service=self._app_context.pdf_service
        )
        right_splitter.addWidget(self._pdf_viewer)

        # 设置分割器比例
        splitter.setSizes([250, 750])  # 左侧宽度 : 右侧宽度
        right_splitter.setSizes([50, 400, 300])  # 搜索栏 : PDF列表 : 预览

    def _setup_menu(self) -> None:
        """设置菜单栏"""
        menubar = self.menuBar()
        if menubar is None:
            return

        # 文件菜单
        file_menu = menubar.addMenu("文件(&F)")
        if file_menu:
            # 添加PDF
            add_pdf_action = QAction("添加PDF(&A)", self)
            add_pdf_action.setShortcut(QKeySequence.StandardKey.Open)
            add_pdf_action.triggered.connect(self._on_add_pdf)
            file_menu.addAction(add_pdf_action)

            # 添加文件夹
            add_folder_action = QAction("添加文件夹(&D)", self)
            add_folder_action.setShortcut(QKeySequence("Ctrl+Shift+N"))
            add_folder_action.triggered.connect(self._on_add_folder)
            file_menu.addAction(add_folder_action)

            file_menu.addSeparator()

            # 导出数据
            export_action = QAction("导出数据(&E)", self)
            export_action.triggered.connect(self._on_export_data)
            file_menu.addAction(export_action)

            # 导入数据
            import_action = QAction("导入数据(&I)", self)
            import_action.triggered.connect(self._on_import_data)
            file_menu.addAction(import_action)

            file_menu.addSeparator()

            # 退出
            exit_action = QAction("退出(&X)", self)
            exit_action.setShortcut(QKeySequence.StandardKey.Quit)
            exit_action.triggered.connect(self.close)
            file_menu.addAction(exit_action)

        # 编辑菜单
        edit_menu = menubar.addMenu("编辑(&E)")
        if edit_menu:
            # 删除
            delete_action = QAction("删除(&D)", self)
            delete_action.setShortcut(QKeySequence.StandardKey.Delete)
            delete_action.triggered.connect(self._on_delete)
            edit_menu.addAction(delete_action)

        # 设置菜单
        settings_menu = menubar.addMenu("设置(&S)")
        if settings_menu:
            # 偏好设置
            preferences_action = QAction("偏好设置(&P)", self)
            preferences_action.setShortcut(QKeySequence("Ctrl+,"))
            preferences_action.triggered.connect(self._on_preferences)
            settings_menu.addAction(preferences_action)

        # 帮助菜单
        help_menu = menubar.addMenu("帮助(&H)")
        if help_menu:
            # 关于
            about_action = QAction("关于(&A)", self)
            about_action.triggered.connect(self._on_about)
            help_menu.addAction(about_action)

    def _setup_statusbar(self) -> None:
        """设置状态栏"""
        statusbar = QStatusBar()
        self.setStatusBar(statusbar)

        # 状态标签
        self._status_label = QLabel("就绪")
        statusbar.addWidget(self._status_label)

        # PDF数量标签
        self._pdf_count_label = QLabel("PDF: 0")
        statusbar.addPermanentWidget(self._pdf_count_label)

    def _setup_toolbar(self) -> None:
        """设置工具栏"""
        toolbar = QToolBar("主工具栏")
        toolbar.setMovable(False)
        self.addToolBar(toolbar)

        # 添加PDF
        add_pdf_action = QAction("添加PDF", self)
        add_pdf_action.triggered.connect(self._on_add_pdf)
        toolbar.addAction(add_pdf_action)

        # 添加文件夹
        add_folder_action = QAction("添加文件夹", self)
        add_folder_action.triggered.connect(self._on_add_folder)
        toolbar.addAction(add_folder_action)

        toolbar.addSeparator()

        # 刷新
        refresh_action = QAction("刷新", self)
        refresh_action.setShortcut(QKeySequence.StandardKey.Refresh)
        refresh_action.triggered.connect(self._on_refresh)
        toolbar.addAction(refresh_action)

    def _connect_signals(self) -> None:
        """连接信号"""
        # 文件夹树选中变化
        self._folder_tree.folder_selected.connect(self._on_folder_selected)

        # PDF列表选中变化
        self._pdf_list.pdf_selected.connect(self._on_pdf_selected)

        # PDF双击
        self._pdf_list.pdf_double_clicked.connect(self._on_pdf_double_clicked)

        # PDF删除
        self._pdf_list.delete_pdf_clicked.connect(self._on_delete_pdf_from_list)

        # 搜索
        self._search_bar.search_requested.connect(self._on_search)

        # PDF预览外部打开
        self._pdf_viewer.open_external_clicked.connect(self._on_open_pdf_external)
        self._pdf_viewer.show_in_folder_clicked.connect(self._on_show_in_folder)

    def _load_initial_data(self) -> None:
        """加载初始数据"""
        self._folder_tree.load_folders()
        self._pdf_list.load_pdfs()
        self._update_status()

    def _update_status(self) -> None:
        """更新状态栏"""
        try:
            stats = self._app_context.pdf_manager.get_statistics()
            total_pdfs = stats.get("total_pdfs", 0)
            total_pages = stats.get("total_pages", 0)
            self._pdf_count_label.setText(f"PDF: {total_pdfs} | 页面: {total_pages}")
        except Exception:
            self._pdf_count_label.setText("PDF: 0")

    # 事件处理方法

    def _on_folder_selected(self, folder_id: int) -> None:
        """文件夹选中变化处理"""
        self._current_folder_id = folder_id if folder_id > 0 else None
        self._pdf_list.load_pdfs(self._current_folder_id)
        self._status_label.setText(f"已选择文件夹 ID: {folder_id}")

    def _on_pdf_selected(self, pdf_id: int) -> None:
        """PDF选中变化处理"""
        self._current_pdf_id = pdf_id
        self._pdf_viewer.load_pdf(pdf_id)

    def _on_pdf_double_clicked(self, pdf_id: int) -> None:
        """PDF双击处理"""
        # TODO: 打开PDF详情或在新窗口中打开
        self._status_label.setText(f"双击PDF: {pdf_id}")

    def _on_search(self, query: str) -> None:
        """搜索处理"""
        self._status_label.setText(f"搜索: {query}")
        # TODO: 执行搜索并更新PDF列表

    def _on_add_pdf(self) -> None:
        """添加PDF处理"""
        file_dialog = QFileDialog(self)
        file_dialog.setWindowTitle("选择PDF文件")
        file_dialog.setNameFilter("PDF文件 (*.pdf)")
        file_dialog.setFileMode(QFileDialog.FileMode.ExistingFiles)

        if file_dialog.exec():
            files = file_dialog.selectedFiles()
            self._status_label.setText(f"已选择 {len(files)} 个文件")

            # 显示导入对话框
            import_dialog = ImportDialog(
                pdf_manager=self._app_context.pdf_manager,
                parent=self
            )
            import_dialog.start_import(files, self._current_folder_id)
            import_dialog.exec()

            # 刷新列表
            self._pdf_list.refresh()
            self._update_status()

    def _on_add_folder(self) -> None:
        """添加文件夹处理"""
        # 触发文件夹树创建根文件夹
        self._folder_tree._on_create_root_folder()

    def _on_delete(self) -> None:
        """删除处理"""
        # 根据当前选中的PDF删除
        if self._current_pdf_id is not None:
            pdf = self._app_context.pdf_manager.get_pdf(self._current_pdf_id)
            if pdf:
                reply = QMessageBox.question(
                    self,
                    "确认删除",
                    f"确定要删除PDF \"{pdf.filename}\" 吗？",
                    QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                    QMessageBox.StandardButton.No
                )
                if reply == QMessageBox.StandardButton.Yes:
                    self._app_context.pdf_manager.delete_pdf(self._current_pdf_id)
                    self._current_pdf_id = None
                    self._pdf_viewer.clear()
                    self._pdf_list.refresh()
                    self._update_status()
                    self._status_label.setText("已删除PDF")
        else:
            self._status_label.setText("请先选择要删除的PDF")

    def _on_export_data(self) -> None:
        """导出数据处理"""
        file_dialog = QFileDialog(self)
        file_dialog.setWindowTitle("导出数据")
        file_dialog.setNameFilter("JSON文件 (*.json)")
        file_dialog.setFileMode(QFileDialog.FileMode.AnyFile)
        file_dialog.setAcceptMode(QFileDialog.AcceptMode.AcceptSave)

        if file_dialog.exec():
            self._status_label.setText("数据已导出")
            # TODO: 实现导出功能

    def _on_import_data(self) -> None:
        """导入数据处理"""
        file_dialog = QFileDialog(self)
        file_dialog.setWindowTitle("导入数据")
        file_dialog.setNameFilter("JSON文件 (*.json)")
        file_dialog.setFileMode(QFileDialog.FileMode.ExistingFile)

        if file_dialog.exec():
            self._status_label.setText("数据已导入")
            # TODO: 实现导入功能

    def _on_preferences(self) -> None:
        """偏好设置处理"""
        dialog = SettingsDialog(config=self._app_context.config, parent=self)
        if dialog.exec():
            self._status_label.setText("设置已保存")

    def _on_about(self) -> None:
        """关于对话框"""
        QMessageBox.about(
            self,
            "关于 PDF Manager",
            "PDF Manager v0.1.0\n\n"
            "一个本地PDF管理工具\n\n"
            "功能：\n"
            "- PDF文件管理\n"
            "- OCR文字识别\n"
            "- 全文搜索\n",
        )

    def _on_open_pdf_external(self, file_path: str) -> None:
        """使用外部程序打开PDF"""
        from PyQt6.QtCore import QUrl
        url = QUrl.fromLocalFile(file_path)
        QDesktopServices.openUrl(url)

    def _on_show_in_folder(self, file_path: str) -> None:
        """在文件夹中显示PDF"""
        from PyQt6.QtCore import QUrl
        import os
        # 使用 file:// URL 并指定 select 参数
        # QDesktopServices 会自动打开文件管理器并选中文件
        url = QUrl.fromLocalFile(os.path.dirname(file_path))
        QDesktopServices.openUrl(url)

    def _on_delete_pdf_from_list(self, pdf_id: int) -> None:
        """从PDF列表删除"""
        pdf = self._app_context.pdf_manager.get_pdf(pdf_id)
        if pdf:
            reply = QMessageBox.question(
                self,
                "确认删除",
                f"确定要删除PDF \"{pdf.filename}\" 吗？",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No
            )
            if reply == QMessageBox.StandardButton.Yes:
                self._app_context.pdf_manager.delete_pdf(pdf_id)
                self._current_pdf_id = None
                self._pdf_viewer.clear()
                self._pdf_list.refresh()
                self._update_status()
                self._status_label.setText("已删除PDF")

    def _on_refresh(self) -> None:
        """刷新处理"""
        self._folder_tree.load_folders()
        self._pdf_list.load_pdfs(self._current_folder_id)
        self._update_status()
        self._status_label.setText("已刷新")

    def showEvent(self, event) -> None:
        """窗口显示事件"""
        super().showEvent(event)
        # 延迟加载初始数据
        self._load_initial_data()

    def closeEvent(self, event) -> None:
        """窗口关闭事件"""
        # TODO: 保存窗口状态、清理资源
        event.accept()