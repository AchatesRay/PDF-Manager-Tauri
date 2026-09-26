pub const SCHEMA: &str = r#"
-- 文件夹表
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    parent_id INTEGER,
    storage_path TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE SET NULL
);

-- PDF 文件表
CREATE TABLE IF NOT EXISTS pdfs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id INTEGER,
    filename TEXT NOT NULL,
    original_path TEXT,
    storage_path TEXT NOT NULL,
    file_size INTEGER DEFAULT 0,
    page_count INTEGER DEFAULT 0,
    pdf_type TEXT CHECK(pdf_type IN ('text', 'scanned', 'mixed')) DEFAULT 'scanned',
    status TEXT CHECK(status IN ('pending', 'processing', 'done', 'error')) DEFAULT 'pending',
    error_message TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE SET NULL
);

-- PDF 页面表
CREATE TABLE IF NOT EXISTS pdf_pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pdf_id INTEGER NOT NULL,
    page_number INTEGER NOT NULL,
    ocr_text TEXT,
    ocr_status TEXT CHECK(ocr_status IN ('pending', 'done', 'error')) DEFAULT 'pending',
    thumbnail_path TEXT,
    FOREIGN KEY (pdf_id) REFERENCES pdfs(id) ON DELETE CASCADE
);

-- 全局设置表
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- OCR 持久化任务队列（重启恢复 pending 任务）
-- position = 入队时的顺序号（完成后删行，MAX(position)+1 保证恢复顺序）
CREATE TABLE IF NOT EXISTS ocr_queue (
    pdf_id INTEGER PRIMARY KEY,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_pdfs_folder ON pdfs(folder_id);
CREATE INDEX IF NOT EXISTS idx_pdfs_status ON pdfs(status);
CREATE INDEX IF NOT EXISTS idx_pages_pdf ON pdf_pages(pdf_id);
"#;

pub const MIGRATIONS: &[&str] = &[
    "ALTER TABLE folders ADD COLUMN storage_path TEXT",
    // 去除 (pdf_id, page_number) 重复行（保留最小 id），再建唯一索引
    "DELETE FROM pdf_pages WHERE id NOT IN (SELECT MIN(id) FROM pdf_pages GROUP BY pdf_id, page_number)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_pages_pdf_page ON pdf_pages(pdf_id, page_number)",
];