"""PDF管理服务"""

import os
import shutil
import hashlib
import logging
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Callable, List, Optional

from src.models.database import Database
from src.models.schemas import PDF, PDFPage, PDFStatus, PDFType, OCRStatus
from src.utils.logger import get_logger

if TYPE_CHECKING:
    from src.services.pdf_service import PDFService
    from src.services.ocr_service import OCRService
    from src.services.index_service import IndexService

logger = get_logger("pdf_manager")


class ImportResult(Enum):
    """PDF导入结果枚举"""
    SUCCESS = "success"
    DUPLICATE = "duplicate"
    INVALID = "invalid"
    ERROR = "error"


@dataclass
class ImportStatus:
    """PDF导入状态"""
    result: ImportResult
    pdf_id: Optional[int]
    message: str


class PDFManager:
    """PDF管理器

    负责PDF文件的导入、删除、移动等操作，协调OCR处理和索引更新。
    """

    def __init__(
        self,
        database: Database,
        pdf_service: "PDFService",
        ocr_service: "OCRService",
        index_service: "IndexService",
        storage_path: str
    ):
        """初始化PDF管理器

        Args:
            database: 数据库实例
            pdf_service: PDF服务实例
            ocr_service: OCR服务实例
            index_service: 索引服务实例
            storage_path: PDF文件存储路径
        """
        self._db = database
        self._pdf_service = pdf_service
        self._ocr_service = ocr_service
        self._index_service = index_service
        self._storage_path = Path(storage_path)

        # 确保存储目录存在
        self._storage_path.mkdir(parents=True, exist_ok=True)

        # 缩略图存储目录
        self._thumbnails_path = self._storage_path / "thumbnails"
        self._thumbnails_path.mkdir(parents=True, exist_ok=True)

    def _calculate_file_hash(self, file_path: str) -> str:
        """计算文件MD5哈希值

        Args:
            file_path: 文件路径

        Returns:
            MD5哈希字符串
        """
        hash_md5 = hashlib.md5()
        with open(file_path, "rb") as f:
            for chunk in iter(lambda: f.read(8192), b""):
                hash_md5.update(chunk)
        return hash_md5.hexdigest()

    def _check_duplicate(self, source_path: str) -> Optional[PDF]:
        """检查是否存在重复的PDF文件

        通过文件名和文件大小进行初步检查。

        Args:
            source_path: 源文件路径

        Returns:
            如果存在重复，返回已存在的PDF对象；否则返回None
        """
        filename = Path(source_path).name
        file_size = self._pdf_service.get_file_size(source_path)

        # 检查同文件名同大小的文件
        all_pdfs = self._db.get_all_pdfs()
        for pdf in all_pdfs:
            if pdf.filename == filename and pdf.file_size == file_size:
                return pdf

        return None

    def _get_storage_filename(self, source_path: str) -> str:
        """生成存储文件名

        使用时间戳和原文件名组合，避免文件名冲突。

        Args:
            source_path: 源文件路径

        Returns:
            存储文件名
        """
        original_name = Path(source_path).name
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        return f"{timestamp}_{original_name}"

    def _get_folder_path_parts(self, folder_id: int) -> List[str]:
        """获取文件夹路径部分（从根到当前文件夹的名称列表）

        Args:
            folder_id: 文件夹ID

        Returns:
            文件夹名称列表，从根文件夹开始
        """
        parts = []
        current_id = folder_id

        while current_id is not None:
            folder = self._db.get_folder(current_id)
            if folder is None:
                break
            parts.insert(0, folder.name)
            current_id = folder.parent_id

        return parts

    def import_pdf(
        self,
        source_path: str,
        folder_id: Optional[int] = None,
        process_ocr: bool = True,
        progress_callback: Optional[Callable[[int, int, str], None]] = None
    ) -> ImportStatus:
        """导入PDF文件

        执行流程：验证 → 检查重复 → 复制文件 → 创建记录 → OCR处理 → 建立索引

        Args:
            source_path: 源PDF文件路径
            folder_id: 目标文件夹ID，None表示根目录
            process_ocr: 是否执行OCR处理
            progress_callback: 进度回调函数，参数为(current, total, message)

        Returns:
            ImportStatus: 导入状态
        """
        # 验证文件存在
        if not os.path.exists(source_path):
            logger.warning(f"导入失败：文件不存在 - {source_path}")
            return ImportStatus(
                result=ImportResult.INVALID,
                pdf_id=None,
                message=f"File not found: {source_path}"
            )

        # 验证PDF有效性
        if not self._pdf_service.validate_pdf(source_path):
            logger.warning(f"导入失败：无效的PDF文件 - {source_path}")
            return ImportStatus(
                result=ImportResult.INVALID,
                pdf_id=None,
                message=f"Invalid PDF file: {source_path}"
            )

        # 检查重复
        duplicate = self._check_duplicate(source_path)
        if duplicate:
            logger.info(f"导入跳过：重复文件 - {duplicate.filename} (ID: {duplicate.id})")
            return ImportStatus(
                result=ImportResult.DUPLICATE,
                pdf_id=duplicate.id,
                message=f"Duplicate PDF: {duplicate.filename} (ID: {duplicate.id})"
            )

        try:
            # 确定存储路径：按文件夹结构保存
            target_folder_path = self._storage_path
            if folder_id is not None:
                # 获取文件夹路径
                folder = self._db.get_folder(folder_id)
                if folder:
                    # 构建文件夹路径
                    folder_path_parts = self._get_folder_path_parts(folder_id)
                    if folder_path_parts:
                        target_folder_path = self._storage_path / "pdfs" / "/".join(folder_path_parts)
                        target_folder_path.mkdir(parents=True, exist_ok=True)
                        logger.debug(f"创建文件夹路径: {target_folder_path}")
            else:
                target_folder_path = self._storage_path / "pdfs"

            # 确保存储目录存在
            target_folder_path.mkdir(parents=True, exist_ok=True)

            # 复制文件到存储目录
            storage_filename = self._get_storage_filename(source_path)
            storage_file_path = target_folder_path / storage_filename
            shutil.copy2(source_path, storage_file_path)
            logger.debug(f"文件复制完成: {source_path} -> {storage_file_path}")

            # 获取PDF信息
            page_count = self._pdf_service.get_page_count(str(storage_file_path))
            file_size = self._pdf_service.get_file_size(str(storage_file_path))
            pdf_type = self._pdf_service.detect_pdf_type(str(storage_file_path))

            # 创建PDF记录
            pdf = PDF(
                filename=Path(source_path).name,
                file_path=str(storage_file_path),
                folder_id=folder_id,
                file_size=file_size,
                page_count=page_count,
                pdf_type=pdf_type,
                status=PDFStatus.PENDING
            )
            pdf_id = self._db.create_pdf(pdf)
            pdf.id = pdf_id
            logger.info(f"创建PDF记录: ID={pdf_id}, 文件名={pdf.filename}, 页数={page_count}")

            # 更新状态为处理中
            self._db.update_pdf_status(pdf_id, PDFStatus.PROCESSING)

            # 创建页面记录
            for page_num in range(page_count):
                page = PDFPage(
                    pdf_id=pdf_id,
                    page_number=page_num,
                    ocr_status=OCRStatus.PENDING
                )
                self._db.create_page(page)

            # OCR处理
            if process_ocr:
                logger.info(f"开始OCR处理: PDF ID={pdf_id}")
                self._process_ocr(pdf, progress_callback)

            # 更新状态为完成
            self._db.update_pdf_status(pdf_id, PDFStatus.DONE)
            logger.info(f"PDF导入完成: ID={pdf_id}, 文件名={pdf.filename}")

            return ImportStatus(
                result=ImportResult.SUCCESS,
                pdf_id=pdf_id,
                message=f"Successfully imported: {pdf.filename}"
            )

        except Exception as e:
            logger.error(f"PDF导入失败: {source_path} - {str(e)}")
            # 清理可能已创建的文件
            if 'storage_file_path' in locals() and storage_file_path.exists():
                storage_file_path.unlink()

            # 如果已创建记录，更新状态为错误
            if 'pdf_id' in locals():
                self._db.update_pdf_status(pdf_id, PDFStatus.ERROR)

            return ImportStatus(
                result=ImportResult.ERROR,
                pdf_id=None,
                message=f"Error importing PDF: {str(e)}"
            )

    def _process_ocr(
        self,
        pdf: PDF,
        progress_callback: Optional[Callable[[int, int, str], None]] = None
    ) -> None:
        """执行OCR处理

        Args:
            pdf: PDF对象
            progress_callback: 进度回调函数
        """
        pages = self._db.get_pages_by_pdf(pdf.id)
        total_pages = len(pages)
        logger.info(f"开始OCR处理: PDF ID={pdf.id}, 文件名={pdf.filename}, 总页数={total_pages}")

        processed_pages = 0
        failed_pages = 0

        for i, page in enumerate(pages):
            if progress_callback:
                progress_callback(i + 1, total_pages, f"OCR处理中: 第 {i + 1}/{total_pages} 页")

            try:
                # 根据PDF类型选择处理方式
                if pdf.pdf_type == PDFType.TEXT:
                    # 文字型PDF直接提取文本
                    text = self._pdf_service.extract_text_from_page(pdf.file_path, page.page_number)
                    logger.debug(f"页面 {page.page_number}: 直接提取文本，长度={len(text)}")
                else:
                    # 扫描型或混合型使用OCR
                    text = self._ocr_service.recognize_pdf_page(
                        self._pdf_service,
                        pdf.file_path,
                        page.page_number
                    )
                    logger.debug(f"页面 {page.page_number}: OCR识别完成，文本长度={len(text)}")

                # 更新页面OCR结果
                self._db.update_page_ocr(page.id, text, OCRStatus.DONE)

                # 添加到索引
                self._index_service.add_page(
                    page_id=f"{pdf.id}_{page.page_number}",
                    pdf_id=pdf.id,
                    page_number=page.page_number,
                    folder_id=pdf.folder_id,
                    filename=pdf.filename,
                    content=text
                )

                processed_pages += 1

            except Exception as e:
                # OCR处理失败，标记为错误但继续处理其他页面
                failed_pages += 1
                logger.warning(f"OCR处理失败: PDF ID={pdf.id}, 页码={page.page_number} - {str(e)}")
                self._db.update_page_ocr(page.id, "", OCRStatus.ERROR)

        logger.info(f"OCR处理完成: PDF ID={pdf.id}, 成功={processed_pages}, 失败={failed_pages}")

    def delete_pdf(self, pdf_id: int) -> bool:
        """删除PDF及所有相关数据

        删除内容包括：
        - 索引中的页面记录
        - 缩略图文件
        - PDF文件
        - 数据库记录（页面记录通过CASCADE自动删除）

        Args:
            pdf_id: PDF ID

        Returns:
            是否删除成功
        """
        pdf = self._db.get_pdf(pdf_id)
        if pdf is None:
            logger.warning(f"删除失败：PDF不存在 - ID={pdf_id}")
            return False

        try:
            logger.info(f"开始删除PDF: ID={pdf_id}, 文件名={pdf.filename}")

            # 删除索引
            self._index_service.delete_pdf(pdf_id)

            # 删除缩略图
            pages = self._db.get_pages_by_pdf(pdf_id)
            for page in pages:
                if page.thumbnail_path and os.path.exists(page.thumbnail_path):
                    os.unlink(page.thumbnail_path)

            # 删除PDF文件
            if os.path.exists(pdf.file_path):
                os.unlink(pdf.file_path)

            # 删除数据库记录（页面会通过CASCADE自动删除）
            self._db.delete_pdf(pdf_id)

            logger.info(f"PDF删除完成: ID={pdf_id}")
            return True

        except Exception as e:
            logger.error(f"PDF删除失败: ID={pdf_id} - {str(e)}")
            return False

    def get_pdf(self, pdf_id: int) -> Optional[PDF]:
        """获取PDF

        Args:
            pdf_id: PDF ID

        Returns:
            PDF对象，不存在则返回None
        """
        return self._db.get_pdf(pdf_id)

    def get_pdfs_by_folder(self, folder_id: Optional[int]) -> List[PDF]:
        """获取文件夹下的PDF列表

        Args:
            folder_id: 文件夹ID，None表示根目录

        Returns:
            PDF列表
        """
        return self._db.get_pdfs_by_folder(folder_id)

    def get_all_pdfs(self) -> List[PDF]:
        """获取所有PDF

        Returns:
            所有PDF列表
        """
        return self._db.get_all_pdfs()

    def get_page(self, page_id: int) -> Optional[PDFPage]:
        """获取页面

        Args:
            page_id: 页面ID

        Returns:
            PDFPage对象，不存在则返回None
        """
        return self._db.get_page(page_id)

    def get_pages_by_pdf(self, pdf_id: int) -> List[PDFPage]:
        """获取PDF的所有页面

        Args:
            pdf_id: PDF ID

        Returns:
            页面列表（按页码排序）
        """
        return self._db.get_pages_by_pdf(pdf_id)

    def move_pdf(self, pdf_id: int, folder_id: Optional[int]) -> bool:
        """移动PDF到其他文件夹

        Args:
            pdf_id: PDF ID
            folder_id: 目标文件夹ID，None表示根目录

        Returns:
            是否移动成功
        """
        pdf = self._db.get_pdf(pdf_id)
        if pdf is None:
            return False

        # 如果目标文件夹不是None，验证文件夹存在
        if folder_id is not None:
            folder = self._db.get_folder(folder_id)
            if folder is None:
                return False

        # 更新文件夹ID
        pdf.folder_id = folder_id
        result = self._db.update_pdf(pdf)

        if result:
            # 更新索引中的folder_id
            pages = self._db.get_pages_by_pdf(pdf_id)
            for page in pages:
                text = page.ocr_text or ""
                self._index_service.add_page(
                    page_id=f"{pdf_id}_{page.page_number}",
                    pdf_id=pdf_id,
                    page_number=page.page_number,
                    folder_id=folder_id,
                    filename=pdf.filename,
                    content=text
                )

        return result

    def reprocess_pdf(self, pdf_id: int) -> bool:
        """重新处理PDF的OCR

        清除现有OCR结果和索引，重新执行OCR处理。

        Args:
            pdf_id: PDF ID

        Returns:
            是否成功启动重新处理
        """
        pdf = self._db.get_pdf(pdf_id)
        if pdf is None:
            return False

        # 删除现有索引
        self._index_service.delete_pdf(pdf_id)

        # 重置页面OCR状态
        pages = self._db.get_pages_by_pdf(pdf_id)
        for page in pages:
            self._db.update_page_ocr(page.id, "", OCRStatus.PENDING)

        # 更新PDF状态
        self._db.update_pdf_status(pdf_id, PDFStatus.PROCESSING)

        try:
            # 重新处理OCR
            self._process_ocr(pdf)

            # 更新状态为完成
            self._db.update_pdf_status(pdf_id, PDFStatus.DONE)

            return True

        except Exception:
            self._db.update_pdf_status(pdf_id, PDFStatus.ERROR)
            return False

    def get_statistics(self) -> dict:
        """获取统计信息

        Returns:
            统计信息字典，包含：
            - total_pdfs: PDF总数
            - total_pages: 页面总数
            - total_size: 总文件大小（字节）
            - status_counts: 各状态的PDF数量
            - ocr_pending: 待OCR处理的页面数
            - ocr_done: 已完成OCR的页面数
            - ocr_error: OCR错误的页面数
        """
        all_pdfs = self._db.get_all_pdfs()

        total_pdfs = len(all_pdfs)
        total_pages = sum(pdf.page_count for pdf in all_pdfs)
        total_size = sum(pdf.file_size for pdf in all_pdfs)

        # 状态统计
        status_counts = self._db.get_status_counts()

        # OCR状态统计
        ocr_pending = 0
        ocr_done = 0
        ocr_error = 0

        for pdf in all_pdfs:
            pages = self._db.get_pages_by_pdf(pdf.id)
            for page in pages:
                if page.ocr_status == OCRStatus.PENDING:
                    ocr_pending += 1
                elif page.ocr_status == OCRStatus.DONE:
                    ocr_done += 1
                elif page.ocr_status == OCRStatus.ERROR:
                    ocr_error += 1

        return {
            "total_pdfs": total_pdfs,
            "total_pages": total_pages,
            "total_size": total_size,
            "status_counts": status_counts,
            "ocr_pending": ocr_pending,
            "ocr_done": ocr_done,
            "ocr_error": ocr_error,
        }