"""应用程序主入口模块"""

import sys
from pathlib import Path
from typing import Optional

from PyQt6.QtWidgets import QApplication, QMessageBox
from PyQt6.QtCore import Qt, QTimer

from src.utils.config import Config
from src.models.database import Database
from src.services.pdf_service import PDFService
from src.services.ocr_service import OCRService
from src.services.index_service import IndexService
from src.core.folder_manager import FolderManager
from src.core.pdf_manager import PDFManager
from src.core.search_service import SearchService
from src.ui.main_window import MainWindow
from src.ui.dialogs import OCRSetupDialog


class ApplicationContext:
    """应用上下文，负责初始化和管理所有服务

    这是应用程序的核心类，负责创建和管理所有服务的生命周期。
    服务之间的依赖关系在此类中自动处理。

    初始化顺序:
    1. Config - 配置管理
    2. Database - 数据库
    3. PDFService - PDF处理服务
    4. OCRService - OCR服务
    5. IndexService - 索引服务
    6. FolderManager - 文件夹管理
    7. PDFManager - PDF管理
    8. SearchService - 搜索服务
    """

    def __init__(self, config_path: Optional[str] = None):
        """初始化应用上下文

        Args:
            config_path: 配置文件路径，如果为None则使用默认路径
        """
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
            use_gpu=False  # 默认不使用GPU
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

    def _get_default_config_path(self) -> str:
        """获取默认配置文件路径

        Returns:
            配置文件路径
        """
        # 使用用户主目录下的应用数据目录
        if sys.platform == "win32":
            # Windows: %APPDATA%/PdfOCR/config.json
            base_path = Path.home() / "AppData" / "Roaming" / "PdfOCR"
        elif sys.platform == "darwin":
            # macOS: ~/Library/Application Support/PdfOCR/config.json
            base_path = Path.home() / "Library" / "Application Support" / "PdfOCR"
        else:
            # Linux: ~/.config/PdfOCR/config.json
            base_path = Path.home() / ".config" / "PdfOCR"

        return str(base_path / "config.json")

    def cleanup(self) -> None:
        """清理资源

        在应用退出时调用，确保所有资源正确释放。
        """
        # 保存配置
        try:
            self.config.save()
        except Exception:
            pass

        # 关闭索引
        try:
            if self.index_service._index is not None:
                self.index_service._index.close()
        except Exception:
            pass

    def check_ocr_available(self) -> bool:
        """检查 OCR 模型是否可用

        Returns:
            True 如果 OCR 模型已安装并可用，否则 False
        """
        status = self.ocr_service.check_model_status()
        return status.get("installed", False)


def main() -> int:
    """应用程序主入口函数

    设置高DPI支持，创建QApplication，初始化应用上下文，
    创建主窗口并运行应用程序。

    Returns:
        应用程序退出码
    """
    # 打包后设置全局异常处理器
    if getattr(sys, 'frozen', False):
        def exception_handler(exc_type, exc_value, exc_tb):
            """全局异常处理器，打包后显示错误对话框"""
            import traceback
            error_msg = ''.join(traceback.format_exception(exc_type, exc_value, exc_tb))
            print(f"Unhandled exception:\n{error_msg}")
            QMessageBox.critical(
                None,
                "程序错误",
                f"程序发生错误：\n{exc_value}\n\n详细信息：\n{error_msg}"
            )
        sys.excepthook = exception_handler

    # 设置高DPI支持
    # PyQt6 默认启用高DPI缩放，无需手动设置
    # QApplication.setAttribute(Qt.AA_EnableHighDpiScaling)  # PyQt5 方式
    # QApplication.setAttribute(Qt.AA_UseHighDpiPixmaps)  # PyQt5 方式

    # 创建QApplication实例
    app = QApplication(sys.argv)
    app.setApplicationName("PdfOCR")
    app.setApplicationVersion("0.1.0")
    app.setOrganizationName("PdfOCR")

    # 创建应用上下文
    try:
        app_context = ApplicationContext()
    except Exception as e:
        print(f"Failed to initialize application: {e}")
        return 1

    # 创建主窗口
    main_window = MainWindow(app_context)
    main_window.setWindowTitle("PDF Manager")

    # 显示窗口
    main_window.show()

    # 延迟检查 OCR 状态
    def check_ocr():
        """检查 OCR 模型状态"""
        if not app_context.check_ocr_available():
            dialog = OCRSetupDialog(main_window)
            dialog.exec()

    QTimer.singleShot(500, check_ocr)

    # 运行应用程序
    result = app.exec()

    # 清理资源
    app_context.cleanup()

    return result


if __name__ == "__main__":
    sys.exit(main())