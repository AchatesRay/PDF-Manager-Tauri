"""配置管理模块"""

import json
from pathlib import Path
from typing import Any

from src.utils.path_utils import get_data_path


DEFAULT_CONFIG = {
    "storage_path": "./data",
    "ocr_language": "ch",
    "ocr_workers": 2,
    "thumbnail_size": 200,
}


class Config:
    """配置管理类"""

    def __init__(self, config_path: str | Path | None = None):
        if config_path is None:
            # 使用 get_data_path() 获取默认配置路径
            self._config_path = get_data_path() / "config.json"
        else:
            self._config_path = Path(config_path)
        self._config: dict[str, Any] = DEFAULT_CONFIG.copy()
        if self._config_path and self._config_path.exists():
            self.load()

    def load(self) -> None:
        """从文件加载配置"""
        if self._config_path and self._config_path.exists():
            with open(self._config_path, "r", encoding="utf-8") as f:
                loaded = json.load(f)
                self._config.update(loaded)

    def save(self) -> None:
        """保存配置到文件"""
        if self._config_path:
            self._config_path.parent.mkdir(parents=True, exist_ok=True)
            with open(self._config_path, "w", encoding="utf-8") as f:
                json.dump(self._config, f, indent=2, ensure_ascii=False)

    def get(self, key: str, default: Any = None) -> Any:
        """获取配置项"""
        return self._config.get(key, default)

    def set(self, key: str, value: Any) -> None:
        """设置配置项"""
        self._config[key] = value

    @property
    def storage_path(self) -> Path:
        """数据存储路径"""
        configured_path = self._config["storage_path"]
        # 如果配置的路径是 "./data"，使用 get_data_path() 获取正确的路径
        if configured_path == "./data":
            return get_data_path()
        return Path(configured_path)

    @storage_path.setter
    def storage_path(self, path: str | Path) -> None:
        self._config["storage_path"] = str(path)

    @property
    def database_path(self) -> Path:
        """数据库文件路径"""
        return self.storage_path / "pdf_manager.db"

    @property
    def pdfs_path(self) -> Path:
        """PDF文件存储路径"""
        return self.storage_path / "pdfs"

    @property
    def thumbnails_path(self) -> Path:
        """缩略图存储路径"""
        return self.storage_path / "thumbnails"

    @property
    def index_path(self) -> Path:
        """索引存储路径"""
        return self.storage_path / "index"

    def ensure_directories(self) -> None:
        """确保所有必要目录存在"""
        self.storage_path.mkdir(parents=True, exist_ok=True)
        self.pdfs_path.mkdir(parents=True, exist_ok=True)
        self.thumbnails_path.mkdir(parents=True, exist_ok=True)
        self.index_path.mkdir(parents=True, exist_ok=True)