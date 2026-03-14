"""工具模块"""
from .config import Config
from .path_utils import get_resource_path, get_data_path, get_app_dir, get_ocr_models_path

# logger模块在Task 3中创建，这里使用延迟导入
try:
    from .logger import setup_logger
except ImportError:
    setup_logger = None  # type: ignore

__all__ = ["Config", "setup_logger", "get_resource_path", "get_data_path", "get_app_dir", "get_ocr_models_path"]