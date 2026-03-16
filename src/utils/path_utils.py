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

    统一使用程序所在目录下的 data 目录。
    开发环境使用 ./data。

    Returns:
        数据目录路径
    """
    if getattr(sys, 'frozen', False):
        # 打包后使用程序所在目录
        return get_app_dir() / "data"
    else:
        # 开发环境
        return Path("./data")


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

    统一使用程序所在目录下的 ocr_models 目录。

    Returns:
        OCR 模型目录路径
    """
    if getattr(sys, 'frozen', False):
        # 打包后使用程序所在目录
        return get_app_dir() / "ocr_models"
    else:
        # 开发环境也使用程序目录下的 ocr_models
        return get_app_dir() / "ocr_models"


def get_log_path() -> Path:
    """获取日志文件路径

    统一使用程序所在目录下的 logs 目录。
    开发环境使用 ./logs。

    Returns:
        日志目录路径
    """
    if getattr(sys, 'frozen', False):
        # 打包后使用程序所在目录
        return get_app_dir() / "logs"
    else:
        # 开发环境
        return Path("./logs")