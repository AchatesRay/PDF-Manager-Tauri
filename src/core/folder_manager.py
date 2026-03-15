"""文件夹管理服务"""

import os
from pathlib import Path
from typing import List, Optional

from src.models.database import Database
from src.models.schemas import Folder
from src.utils.path_utils import get_data_path
from src.utils.logger import get_logger

logger = get_logger("folder_manager")


class FolderManager:
    """文件夹管理器

    提供文件夹的CRUD操作、嵌套管理、循环引用检测等功能。
    """

    def __init__(self, database: Database, storage_path: Optional[Path] = None):
        """初始化文件夹管理器

        Args:
            database: 数据库实例
            storage_path: 存储路径，如果为None则使用默认data路径
        """
        self._db = database
        self._storage_path = storage_path or get_data_path() / "pdfs"
        # 确保存储目录存在
        self._storage_path.mkdir(parents=True, exist_ok=True)

    def create_folder(self, name: str, parent_id: Optional[int] = None) -> Folder:
        """创建文件夹

        Args:
            name: 文件夹名称
            parent_id: 父文件夹ID，None表示根文件夹

        Returns:
            创建的文件夹对象

        Raises:
            ValueError: 父文件夹不存在
        """
        # 验证父文件夹存在
        if parent_id is not None:
            parent = self._db.get_folder(parent_id)
            if parent is None:
                raise ValueError(f"Parent folder with id {parent_id} does not exist")
            logger.info(f"创建子文件夹: 名称='{name}', 父文件夹='{parent.name}' (ID={parent_id})")
        else:
            logger.info(f"创建根文件夹: 名称='{name}'")

        folder = Folder(name=name, parent_id=parent_id)
        folder_id = self._db.create_folder(folder)
        folder.id = folder_id

        # 验证创建结果
        created = self._db.get_folder(folder_id)
        if created:
            logger.info(f"文件夹创建成功: ID={folder_id}, 名称='{created.name}', parent_id={created.parent_id}")

        # 同步创建物理目录
        try:
            folder_path = self._get_folder_physical_path(folder_id)
            folder_path.mkdir(parents=True, exist_ok=True)
            logger.info(f"创建物理目录: {folder_path}")
        except Exception as e:
            logger.warning(f"创建物理目录失败: {e}")

        return folder

    def _get_folder_physical_path(self, folder_id: int) -> Path:
        """获取文件夹的物理存储路径

        Args:
            folder_id: 文件夹ID

        Returns:
            物理路径
        """
        parts = []
        current_id = folder_id

        while current_id is not None:
            folder = self._db.get_folder(current_id)
            if folder is None:
                break
            parts.insert(0, folder.name)
            current_id = folder.parent_id

        return self._storage_path / "/".join(parts)

    def get_folder(self, folder_id: int) -> Optional[Folder]:
        """获取文件夹

        Args:
            folder_id: 文件夹ID

        Returns:
            文件夹对象，不存在则返回None
        """
        return self._db.get_folder(folder_id)

    def get_root_folders(self) -> List[Folder]:
        """获取根文件夹列表

        Returns:
            根文件夹列表（parent_id为None的文件夹）
        """
        return self._db.get_folders_by_parent(None)

    def get_child_folders(self, parent_id: int) -> List[Folder]:
        """获取子文件夹列表

        Args:
            parent_id: 父文件夹ID

        Returns:
            子文件夹列表
        """
        return self._db.get_folders_by_parent(parent_id)

    def get_all_folders(self) -> List[Folder]:
        """获取所有文件夹

        Returns:
            所有文件夹列表
        """
        return self._db.get_all_folders()

    def rename_folder(self, folder_id: int, new_name: str) -> bool:
        """重命名文件夹

        Args:
            folder_id: 文件夹ID
            new_name: 新名称

        Returns:
            是否重命名成功
        """
        folder = self._db.get_folder(folder_id)
        if folder is None:
            return False

        folder.name = new_name
        return self._db.update_folder(folder)

    def move_folder(self, folder_id: int, new_parent_id: Optional[int]) -> bool:
        """移动文件夹到新的父文件夹下

        Args:
            folder_id: 要移动的文件夹ID
            new_parent_id: 新的父文件夹ID，None表示移动到根目录

        Returns:
            是否移动成功

        Raises:
            ValueError: 循环引用或目标文件夹不存在
        """
        folder = self._db.get_folder(folder_id)
        if folder is None:
            return False

        # 移动到同一位置，无需操作
        if folder.parent_id == new_parent_id:
            return True

        # 检查新父文件夹是否存在
        if new_parent_id is not None:
            new_parent = self._db.get_folder(new_parent_id)
            if new_parent is None:
                raise ValueError(f"Target folder with id {new_parent_id} does not exist")

            # 检查循环引用
            if self._would_create_cycle(folder_id, new_parent_id):
                raise ValueError(
                    f"Cannot move folder {folder_id} to {new_parent_id}: "
                    "would create a cycle"
                )

        folder.parent_id = new_parent_id
        return self._db.update_folder(folder)

    def _would_create_cycle(self, folder_id: int, new_parent_id: int) -> bool:
        """检查移动文件夹是否会创建循环引用

        Args:
            folder_id: 要移动的文件夹ID
            new_parent_id: 新的父文件夹ID

        Returns:
            是否会创建循环引用
        """
        # 如果新父文件夹就是自己，则会产生循环
        if folder_id == new_parent_id:
            return True

        # 检查新父文件夹是否是当前文件夹的后代
        current_id = new_parent_id
        while current_id is not None:
            if current_id == folder_id:
                return True
            parent_folder = self._db.get_folder(current_id)
            if parent_folder is None:
                break
            current_id = parent_folder.parent_id

        return False

    def delete_folder(
        self, folder_id: int, delete_contents: bool = False
    ) -> bool:
        """删除文件夹

        Args:
            folder_id: 文件夹ID
            delete_contents: 是否删除子内容（子文件夹和PDF）
                - False: 仅当文件夹为空时删除
                - True: 递归删除所有子内容

        Returns:
            是否删除成功

        Raises:
            ValueError: 文件夹非空且未设置delete_contents
        """
        folder = self._db.get_folder(folder_id)
        if folder is None:
            return False

        # 检查是否有子文件夹
        children = self._db.get_folders_by_parent(folder_id)
        if children:
            if not delete_contents:
                raise ValueError(
                    f"Cannot delete folder {folder_id}: "
                    "folder has subfolders. Set delete_contents=True to force delete."
                )
            # 递归删除子文件夹
            for child in children:
                self.delete_folder(child.id, delete_contents=True)

        # 检查是否有PDF文件
        pdfs = self._db.get_pdfs_by_folder(folder_id)
        if pdfs:
            if not delete_contents:
                raise ValueError(
                    f"Cannot delete folder {folder_id}: "
                    "folder contains PDF files. Set delete_contents=True to force delete."
                )
            # 删除PDF文件
            for pdf in pdfs:
                self._db.delete_pdf(pdf.id)

        return self._db.delete_folder(folder_id)

    def get_folder_path(self, folder_id: int) -> List[Folder]:
        """获取文件夹路径（从根到当前文件夹）

        Args:
            folder_id: 文件夹ID

        Returns:
            文件夹路径列表，从根文件夹开始到当前文件夹
            如果文件夹不存在，返回空列表
        """
        folder = self._db.get_folder(folder_id)
        if folder is None:
            return []

        path = []
        current = folder

        # 从当前文件夹向上遍历到根
        while current is not None:
            path.append(current)
            if current.parent_id is None:
                break
            current = self._db.get_folder(current.parent_id)

        # 反转，使路径从根开始
        path.reverse()
        return path