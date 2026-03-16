"""数据库操作模块"""

import sqlite3
from datetime import datetime
from pathlib import Path
from typing import Optional, List, Dict, Any
from contextlib import contextmanager

from .schemas import Folder, PDF, PDFPage, PDFType, PDFStatus, OCRStatus


class Database:
    """SQLite 数据库操作类"""

    def __init__(self, db_path: str | Path):
        """初始化数据库连接

        Args:
            db_path: 数据库文件路径
        """
        self.db_path = Path(db_path)
        self._init_db()

    def _init_db(self) -> None:
        """初始化数据库表结构"""
        # 确保数据库目录存在
        self.db_path.parent.mkdir(parents=True, exist_ok=True)

        with self._get_connection() as conn:
            cursor = conn.cursor()

            # 创建文件夹表
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS folders (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    parent_id INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE SET NULL
                )
            """)

            # 创建PDF文件表
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS pdfs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    folder_id INTEGER,
                    filename TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    file_size INTEGER DEFAULT 0,
                    page_count INTEGER DEFAULT 0,
                    pdf_type TEXT NOT NULL DEFAULT 'scanned',
                    status TEXT NOT NULL DEFAULT 'pending',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE SET NULL
                )
            """)

            # 创建页面表 (带 CASCADE DELETE)
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS pdf_pages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    pdf_id INTEGER NOT NULL,
                    page_number INTEGER NOT NULL,
                    ocr_text TEXT DEFAULT '',
                    ocr_status TEXT NOT NULL DEFAULT 'pending',
                    thumbnail_path TEXT,
                    FOREIGN KEY (pdf_id) REFERENCES pdfs(id) ON DELETE CASCADE
                )
            """)

            # 创建索引
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_folders_parent
                ON folders(parent_id)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_pdfs_folder
                ON pdfs(folder_id)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_pages_pdf
                ON pdf_pages(pdf_id)
            """)

            conn.commit()

    @contextmanager
    def _get_connection(self):
        """获取数据库连接的上下文管理器"""
        conn = sqlite3.connect(str(self.db_path))
        conn.row_factory = sqlite3.Row
        # 启用外键约束
        conn.execute("PRAGMA foreign_keys = ON")
        try:
            yield conn
        finally:
            conn.close()

    # ==================== 文件夹操作 ====================

    def _row_to_folder(self, row: sqlite3.Row) -> Folder:
        """将数据库行转换为 Folder 对象"""
        return Folder(
            id=row["id"],
            name=row["name"],
            parent_id=row["parent_id"],
            created_at=datetime.fromisoformat(row["created_at"]) if row["created_at"] else None,
            updated_at=datetime.fromisoformat(row["updated_at"]) if row["updated_at"] else None,
        )

    def create_folder(self, folder: Folder) -> int:
        """创建文件夹

        Args:
            folder: 文件夹对象

        Returns:
            创建的文件夹ID
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                INSERT INTO folders (name, parent_id, created_at, updated_at)
                VALUES (?, ?, ?, ?)
            """, (
                folder.name,
                folder.parent_id,
                folder.created_at.isoformat() if folder.created_at else datetime.now().isoformat(),
                folder.updated_at.isoformat() if folder.updated_at else datetime.now().isoformat(),
            ))
            conn.commit()
            return cursor.lastrowid

    def get_folder(self, folder_id: int) -> Optional[Folder]:
        """根据ID获取文件夹

        Args:
            folder_id: 文件夹ID

        Returns:
            文件夹对象，不存在则返回None
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM folders WHERE id = ?", (folder_id,))
            row = cursor.fetchone()
            return self._row_to_folder(row) if row else None

    def get_folders_by_parent(self, parent_id: Optional[int]) -> List[Folder]:
        """获取指定父文件夹下的所有子文件夹

        Args:
            parent_id: 父文件夹ID，None表示根目录

        Returns:
            文件夹列表
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            if parent_id is None:
                cursor.execute("SELECT * FROM folders WHERE parent_id IS NULL ORDER BY name")
            else:
                cursor.execute("SELECT * FROM folders WHERE parent_id = ? ORDER BY name", (parent_id,))
            rows = cursor.fetchall()
            return [self._row_to_folder(row) for row in rows]

    def get_all_folders(self) -> List[Folder]:
        """获取所有文件夹

        Returns:
            所有文件夹列表
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM folders ORDER BY name")
            rows = cursor.fetchall()
            folders = [self._row_to_folder(row) for row in rows]
            # 添加日志用于调试
            import logging
            logger = logging.getLogger("database")
            for f in folders:
                logger.debug(f"读取文件夹: ID={f.id}, 名称='{f.name}', parent_id={f.parent_id}")
            return folders

    def update_folder(self, folder: Folder) -> bool:
        """更新文件夹

        Args:
            folder: 文件夹对象（必须包含id）

        Returns:
            是否更新成功
        """
        if folder.id is None:
            return False

        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                UPDATE folders
                SET name = ?, parent_id = ?, updated_at = ?
                WHERE id = ?
            """, (
                folder.name,
                folder.parent_id,
                datetime.now().isoformat(),
                folder.id,
            ))
            conn.commit()
            return cursor.rowcount > 0

    def delete_folder(self, folder_id: int) -> bool:
        """删除文件夹

        Args:
            folder_id: 文件夹ID

        Returns:
            是否删除成功
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("DELETE FROM folders WHERE id = ?", (folder_id,))
            conn.commit()
            return cursor.rowcount > 0

    # ==================== PDF操作 ====================

    def _row_to_pdf(self, row: sqlite3.Row) -> PDF:
        """将数据库行转换为 PDF 对象"""
        return PDF(
            id=row["id"],
            folder_id=row["folder_id"],
            filename=row["filename"],
            file_path=row["file_path"],
            file_size=row["file_size"],
            page_count=row["page_count"],
            pdf_type=PDFType(row["pdf_type"]),
            status=PDFStatus(row["status"]),
            created_at=datetime.fromisoformat(row["created_at"]) if row["created_at"] else None,
            updated_at=datetime.fromisoformat(row["updated_at"]) if row["updated_at"] else None,
        )

    def create_pdf(self, pdf: PDF) -> int:
        """创建PDF记录

        Args:
            pdf: PDF对象

        Returns:
            创建的PDF记录ID
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                INSERT INTO pdfs (folder_id, filename, file_path, file_size, page_count,
                                  pdf_type, status, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                pdf.folder_id,
                pdf.filename,
                pdf.file_path,
                pdf.file_size,
                pdf.page_count,
                pdf.pdf_type.value,
                pdf.status.value,
                pdf.created_at.isoformat() if pdf.created_at else datetime.now().isoformat(),
                pdf.updated_at.isoformat() if pdf.updated_at else datetime.now().isoformat(),
            ))
            conn.commit()
            return cursor.lastrowid

    def get_pdf(self, pdf_id: int) -> Optional[PDF]:
        """根据ID获取PDF

        Args:
            pdf_id: PDF ID

        Returns:
            PDF对象，不存在则返回None
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM pdfs WHERE id = ?", (pdf_id,))
            row = cursor.fetchone()
            return self._row_to_pdf(row) if row else None

    def get_pdfs_by_folder(self, folder_id: Optional[int]) -> List[PDF]:
        """获取指定文件夹下的所有PDF

        Args:
            folder_id: 文件夹ID，None表示根目录

        Returns:
            PDF列表
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            if folder_id is None:
                cursor.execute("SELECT * FROM pdfs WHERE folder_id IS NULL ORDER BY filename")
            else:
                cursor.execute("SELECT * FROM pdfs WHERE folder_id = ? ORDER BY filename", (folder_id,))
            rows = cursor.fetchall()
            return [self._row_to_pdf(row) for row in rows]

    def get_all_pdfs(self) -> List[PDF]:
        """获取所有PDF

        Returns:
            所有PDF列表
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM pdfs ORDER BY filename")
            rows = cursor.fetchall()
            return [self._row_to_pdf(row) for row in rows]

    def update_pdf(self, pdf: PDF) -> bool:
        """更新PDF

        Args:
            pdf: PDF对象（必须包含id）

        Returns:
            是否更新成功
        """
        if pdf.id is None:
            return False

        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                UPDATE pdfs
                SET folder_id = ?, filename = ?, file_path = ?, file_size = ?,
                    page_count = ?, pdf_type = ?, status = ?, updated_at = ?
                WHERE id = ?
            """, (
                pdf.folder_id,
                pdf.filename,
                pdf.file_path,
                pdf.file_size,
                pdf.page_count,
                pdf.pdf_type.value,
                pdf.status.value,
                datetime.now().isoformat(),
                pdf.id,
            ))
            conn.commit()
            return cursor.rowcount > 0

    def update_pdf_status(self, pdf_id: int, status: PDFStatus) -> bool:
        """更新PDF状态

        Args:
            pdf_id: PDF ID
            status: 新状态

        Returns:
            是否更新成功
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                UPDATE pdfs SET status = ?, updated_at = ? WHERE id = ?
            """, (
                status.value,
                datetime.now().isoformat(),
                pdf_id,
            ))
            conn.commit()
            return cursor.rowcount > 0

    def delete_pdf(self, pdf_id: int) -> bool:
        """删除PDF

        Args:
            pdf_id: PDF ID

        Returns:
            是否删除成功
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("DELETE FROM pdfs WHERE id = ?", (pdf_id,))
            conn.commit()
            return cursor.rowcount > 0

    # ==================== 页面操作 ====================

    def _row_to_page(self, row: sqlite3.Row) -> PDFPage:
        """将数据库行转换为 PDFPage 对象"""
        return PDFPage(
            id=row["id"],
            pdf_id=row["pdf_id"],
            page_number=row["page_number"],
            ocr_text=row["ocr_text"] or "",
            ocr_status=OCRStatus(row["ocr_status"]),
            thumbnail_path=row["thumbnail_path"],
        )

    def create_page(self, page: PDFPage) -> int:
        """创建页面记录

        Args:
            page: PDFPage对象

        Returns:
            创建的页面记录ID
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                INSERT INTO pdf_pages (pdf_id, page_number, ocr_text, ocr_status, thumbnail_path)
                VALUES (?, ?, ?, ?, ?)
            """, (
                page.pdf_id,
                page.page_number,
                page.ocr_text,
                page.ocr_status.value,
                page.thumbnail_path,
            ))
            conn.commit()
            return cursor.lastrowid

    def get_page(self, page_id: int) -> Optional[PDFPage]:
        """根据ID获取页面

        Args:
            page_id: 页面ID

        Returns:
            PDFPage对象，不存在则返回None
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM pdf_pages WHERE id = ?", (page_id,))
            row = cursor.fetchone()
            return self._row_to_page(row) if row else None

    def get_pages_by_pdf(self, pdf_id: int) -> List[PDFPage]:
        """获取指定PDF的所有页面

        Args:
            pdf_id: PDF ID

        Returns:
            页面列表（按页码排序）
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM pdf_pages WHERE pdf_id = ? ORDER BY page_number", (pdf_id,))
            rows = cursor.fetchall()
            return [self._row_to_page(row) for row in rows]

    def update_page(self, page: PDFPage) -> bool:
        """更新页面

        Args:
            page: PDFPage对象（必须包含id）

        Returns:
            是否更新成功
        """
        if page.id is None:
            return False

        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                UPDATE pdf_pages
                SET pdf_id = ?, page_number = ?, ocr_text = ?, ocr_status = ?, thumbnail_path = ?
                WHERE id = ?
            """, (
                page.pdf_id,
                page.page_number,
                page.ocr_text,
                page.ocr_status.value,
                page.thumbnail_path,
                page.id,
            ))
            conn.commit()
            return cursor.rowcount > 0

    def update_page_ocr(self, page_id: int, ocr_text: str, ocr_status: OCRStatus) -> bool:
        """更新页面OCR结果

        Args:
            page_id: 页面ID
            ocr_text: OCR文本
            ocr_status: OCR状态

        Returns:
            是否更新成功
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                UPDATE pdf_pages SET ocr_text = ?, ocr_status = ? WHERE id = ?
            """, (
                ocr_text,
                ocr_status.value,
                page_id,
            ))
            conn.commit()
            return cursor.rowcount > 0

    def delete_pages_by_pdf(self, pdf_id: int) -> int:
        """删除指定PDF的所有页面

        Args:
            pdf_id: PDF ID

        Returns:
            删除的页面数量
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("DELETE FROM pdf_pages WHERE pdf_id = ?", (pdf_id,))
            conn.commit()
            return cursor.rowcount

    # ==================== 统计操作 ====================

    def get_pdf_count(self, folder_id: Optional[int] = None) -> int:
        """获取PDF数量

        Args:
            folder_id: 文件夹ID，None表示所有PDF

        Returns:
            PDF数量
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            if folder_id is None:
                cursor.execute("SELECT COUNT(*) FROM pdfs")
            else:
                cursor.execute("SELECT COUNT(*) FROM pdfs WHERE folder_id = ?", (folder_id,))
            result = cursor.fetchone()
            return result[0] if result else 0

    def get_status_counts(self) -> Dict[str, int]:
        """获取各状态的PDF数量统计

        Returns:
            状态统计字典
        """
        with self._get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                SELECT status, COUNT(*) as count FROM pdfs GROUP BY status
            """)
            rows = cursor.fetchall()
            # 确保所有状态都有默认值
            result = {status.value: 0 for status in PDFStatus}
            for row in rows:
                result[row["status"]] = row["count"]
            return result