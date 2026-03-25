use sysinfo::System;
use tracing::{debug, info, warn};

/// 内存安全阈值：保留 1GB 给系统
const MEMORY_SAFETY_THRESHOLD: u64 = 1024 * 1024 * 1024; // 1GB

/// OCR 模型预估内存占用（约 800MB）
const OCR_MODEL_MEMORY: u64 = 800 * 1024 * 1024;

/// 内存信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryInfo {
    /// 总内存（字节）
    pub total: u64,
    /// 可用内存（字节）
    pub available: u64,
    /// 已使用百分比
    pub used_percent: f32,
}

/// 获取系统内存信息
pub fn get_system_memory_info() -> MemoryInfo {
    let mut sys = System::new_all();
    sys.refresh_memory();

    let total = sys.total_memory();
    let available = sys.available_memory();
    let used = total.saturating_sub(available);
    let used_percent = if total > 0 {
        (used as f64 / total as f64 * 100.0) as f32
    } else {
        0.0
    };

    debug!(
        "系统内存: 总计={:.2}GB, 可用={:.2}GB, 已用={:.1}%",
        total as f64 / 1024.0 / 1024.0 / 1024.0,
        available as f64 / 1024.0 / 1024.0 / 1024.0,
        used_percent
    );

    MemoryInfo {
        total,
        available,
        used_percent,
    }
}

/// 估算单页处理所需内存（字节）
///
/// 计算公式：
/// - PDF 渲染: max_dimension² × 4 bytes (RGBA)
/// - OCR 处理: max_dimension² × 3 bytes (RGB)
/// - 预留 50% buffer
pub fn estimate_page_memory(max_dimension: u32) -> u64 {
    let pixels = max_dimension as u64 * max_dimension as u64;

    // PDF 渲染 (RGBA)
    let render_memory = pixels * 4;

    // OCR 处理 (RGB)
    let ocr_memory = pixels * 3;

    // 总计 + 50% buffer
    let total = (render_memory + ocr_memory) * 3 / 2;

    debug!(
        "估算单页内存: dimension={}, pixels={}, render={}MB, ocr={}MB, total={}MB",
        max_dimension,
        pixels,
        render_memory / 1024 / 1024,
        ocr_memory / 1024 / 1024,
        total / 1024 / 1024
    );

    total
}

/// 估算整个任务所需内存（字节）
///
/// 包括：
/// - OCR 模型内存（约 800MB）
/// - 图像处理内存（峰值约 2-3 页）
pub fn estimate_task_memory(page_count: u32, max_dimension: u32) -> u64 {
    let per_page = estimate_page_memory(max_dimension);

    // 图像内存峰值：假设同时持有 2 个页面的图像
    let peak_pages = 2.min(page_count as usize);
    let image_memory = per_page * peak_pages as u64;

    // 总内存 = OCR 模型内存 + 图像内存
    let total = OCR_MODEL_MEMORY + image_memory;

    info!(
        "估算任务内存: pages={}, peak_pages={}, per_page={}MB, image={}MB, model={}MB, total={}MB",
        page_count,
        peak_pages,
        per_page / 1024 / 1024,
        image_memory / 1024 / 1024,
        OCR_MODEL_MEMORY / 1024 / 1024,
        total / 1024 / 1024
    );

    total
}

/// 检查是否可以启动新任务
pub fn can_start_task(required_memory: u64) -> bool {
    let mem_info = get_system_memory_info();

    // 检查可用内存是否足够（保留安全阈值）
    let safe_available = mem_info.available.saturating_sub(MEMORY_SAFETY_THRESHOLD);
    let can_start = safe_available >= required_memory;

    if can_start {
        info!(
            "可以启动任务: 需要={}MB, 安全可用={}MB",
            required_memory / 1024 / 1024,
            safe_available / 1024 / 1024
        );
    } else {
        info!(
            "内存不足，无法启动任务: 需要={}MB, 安全可用={}MB",
            required_memory / 1024 / 1024,
            safe_available / 1024 / 1024
        );
    }

    can_start
}

/// 检查系统是否处于低内存状态
pub fn is_low_memory() -> bool {
    let mem_info = get_system_memory_info();
    mem_info.available < MEMORY_SAFETY_THRESHOLD * 2 // 低于 2GB 可用内存视为低内存
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_system_memory_info() {
        let info = get_system_memory_info();
        assert!(info.total > 0);
        assert!(info.available > 0);
        assert!(info.used_percent >= 0.0 && info.used_percent <= 100.0);
    }

    #[test]
    fn test_estimate_page_memory() {
        // 500x500 图像
        let mem = estimate_page_memory(500);
        // 应该大约是 (500*500*4 + 500*500*3) * 1.5 = 5.25MB
        assert!(mem > 5_000_000 && mem < 6_000_000);

        // 2000x2000 图像
        let mem = estimate_page_memory(2000);
        // 应该大约是 (2000*2000*4 + 2000*2000*3) * 1.5 = 84MB
        assert!(mem > 80_000_000 && mem < 90_000_000);
    }

    #[test]
    fn test_can_start_task() {
        // 请求 1MB 内存应该总是可以
        assert!(can_start_task(1024 * 1024));
    }
}