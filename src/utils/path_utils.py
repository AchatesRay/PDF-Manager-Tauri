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