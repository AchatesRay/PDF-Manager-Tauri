// 数据库模块集成测试
// 使用内存数据库进行测试

use rusqlite::{Connection, params};
use chrono::Utc;

fn create_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    parent_id INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE SET NULL
);

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

CREATE TABLE IF NOT EXISTS pdf_pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pdf_id INTEGER NOT NULL,
    page_number INTEGER NOT NULL,
    ocr_text TEXT,
    ocr_status TEXT CHECK(ocr_status IN ('pending', 'done', 'error')) DEFAULT 'pending',
    thumbnail_path TEXT,
    FOREIGN KEY (pdf_id) REFERENCES pdfs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_pdfs_folder ON pdfs(folder_id);
CREATE INDEX IF NOT EXISTS idx_pdfs_status ON pdfs(status);
CREATE INDEX IF NOT EXISTS idx_pages_pdf ON pdf_pages(pdf_id);
"#;

#[test]
fn test_database_initialization() {
    let conn = create_test_db();

    // 验证表已创建
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('folders', 'pdfs', 'pdf_pages')",
            [],
            |row| row.get(0)
        )
        .unwrap();

    assert_eq!(table_count, 3);
}

#[test]
fn test_folder_crud() {
    let conn = create_test_db();
    let now = Utc::now().to_rfc3339();

    // Create
    conn.execute(
        "INSERT INTO folders (name, parent_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params!["工作文档", None::<i64>, now, now],
    ).unwrap();

    let folder_id = conn.last_insert_rowid();
    assert!(folder_id > 0);

    // Read
    let name: String = conn
        .query_row(
            "SELECT name FROM folders WHERE id = ?1",
            params![folder_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(name, "工作文档");

    // Update
    conn.execute(
        "UPDATE folders SET name = ?1 WHERE id = ?2",
        params!["工作文档_更新", folder_id],
    ).unwrap();

    let updated_name: String = conn
        .query_row(
            "SELECT name FROM folders WHERE id = ?1",
            params![folder_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(updated_name, "工作文档_更新");

    // Delete
    conn.execute("DELETE FROM folders WHERE id = ?1", params![folder_id]).unwrap();

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders WHERE id = ?1", params![folder_id], |row| row.get(0))
        .unwrap();

    assert_eq!(count, 0);
}

#[test]
fn test_pdf_crud() {
    let conn = create_test_db();
    let now = Utc::now().to_rfc3339();

    // 先创建文件夹
    conn.execute(
        "INSERT INTO folders (name, created_at, updated_at) VALUES (?1, ?2, ?3)",
        params!["测试文件夹", now, now],
    ).unwrap();
    let folder_id = conn.last_insert_rowid();

    // Create PDF
    conn.execute(
        "INSERT INTO pdfs (folder_id, filename, storage_path, file_size, page_count, pdf_type, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![folder_id, "test.pdf", "/storage/test.pdf", 1024, 10, "scanned", "pending", now, now],
    ).unwrap();

    let pdf_id = conn.last_insert_rowid();
    assert!(pdf_id > 0);

    // Read
    let filename: String = conn
        .query_row(
            "SELECT filename FROM pdfs WHERE id = ?1",
            params![pdf_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(filename, "test.pdf");

    // Update status
    conn.execute(
        "UPDATE pdfs SET status = 'done' WHERE id = ?1",
        params![pdf_id],
    ).unwrap();

    let status: String = conn
        .query_row("SELECT status FROM pdfs WHERE id = ?1", params![pdf_id], |row| row.get(0))
        .unwrap();

    assert_eq!(status, "done");
}

#[test]
fn test_pdf_pages() {
    let conn = create_test_db();
    let now = Utc::now().to_rfc3339();

    // 创建 PDF
    conn.execute(
        "INSERT INTO pdfs (filename, storage_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params!["test.pdf", "/storage/test.pdf", now, now],
    ).unwrap();
    let pdf_id = conn.last_insert_rowid();

    // 创建页面
    for i in 1..=5 {
        conn.execute(
            "INSERT INTO pdf_pages (pdf_id, page_number, ocr_status) VALUES (?1, ?2, ?3)",
            params![pdf_id, i, "pending"],
        ).unwrap();
    }

    // 验证页面数量
    let page_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM pdf_pages WHERE pdf_id = ?1", params![pdf_id], |row| row.get(0))
        .unwrap();

    assert_eq!(page_count, 5);

    // 更新 OCR 文本
    conn.execute(
        "UPDATE pdf_pages SET ocr_text = ?1, ocr_status = 'done' WHERE pdf_id = ?2 AND page_number = 1",
        params!["第一页内容", pdf_id],
    ).unwrap();

    let text: String = conn
        .query_row(
            "SELECT ocr_text FROM pdf_pages WHERE pdf_id = ?1 AND page_number = 1",
            params![pdf_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(text, "第一页内容");
}

#[test]
fn test_cascade_delete() {
    let conn = create_test_db();
    let now = Utc::now().to_rfc3339();

    // 创建 PDF 和页面
    conn.execute(
        "INSERT INTO pdfs (filename, storage_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params!["test.pdf", "/storage/test.pdf", now, now],
    ).unwrap();
    let pdf_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO pdf_pages (pdf_id, page_number) VALUES (?1, ?2)",
        params![pdf_id, 1],
    ).unwrap();

    // 删除 PDF
    conn.execute("DELETE FROM pdfs WHERE id = ?1", params![pdf_id]).unwrap();

    // 验证页面也被删除
    let page_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM pdf_pages", [], |row| row.get(0))
        .unwrap();

    assert_eq!(page_count, 0);
}

#[test]
fn test_folder_hierarchy() {
    let conn = create_test_db();
    let now = Utc::now().to_rfc3339();

    // 创建父文件夹
    conn.execute(
        "INSERT INTO folders (name, created_at, updated_at) VALUES (?1, ?2, ?3)",
        params!["父文件夹", now, now],
    ).unwrap();
    let parent_id = conn.last_insert_rowid();

    // 创建子文件夹
    conn.execute(
        "INSERT INTO folders (name, parent_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params!["子文件夹", parent_id, now, now],
    ).unwrap();
    let child_id = conn.last_insert_rowid();

    // 验证层级关系
    let child_parent: i64 = conn
        .query_row("SELECT parent_id FROM folders WHERE id = ?1", params![child_id], |row| row.get(0))
        .unwrap();

    assert_eq!(child_parent, parent_id);
}