use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tracing::{debug, info, warn};

/// OCR 任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrTask {
    pub pdf_id: i64,
    pub created_at: DateTime<Utc>,
}

/// 任务队列状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    /// 当前正在处理的 PDF ID
    pub current: Option<i64>,
    /// 等待中的任务队列
    pub pending: Vec<OcrTask>,
}

/// OCR 任务队列
///
/// 设计决策：
/// - 单任务串行：一次只处理一个 PDF
/// - 持久化：pending 任务落库 `ocr_queue` 表（见 db/schema.rs），重启由
///   `restore` 装载后自动续跑；本结构本身仍保持纯内存（DB 访问在 commands 层）
pub struct TaskQueue {
    /// 等待中的任务
    pending: VecDeque<OcrTask>,
    /// 当前正在运行的任务
    running: Option<i64>,
}

impl TaskQueue {
    pub fn new() -> Self {
        info!("初始化任务队列");
        Self {
            pending: VecDeque::new(),
            running: None,
        }
    }

    /// 启动时装载持久化任务（按给定顺序全部进入 pending，running 保持 None）
    ///
    /// 返回实际装载的任务数（去重后）。消费方随后应取队首任务开始执行。
    pub fn restore(&mut self, tasks: Vec<OcrTask>) -> usize {
        if self.running.is_some() {
            warn!("队列已有运行中任务，restore 跳过: running={:?}", self.running);
            return 0;
        }

        let mut restored = 0;
        for task in tasks {
            if self.pending.iter().any(|t| t.pdf_id == task.pdf_id) {
                warn!("恢复任务已在队列中，跳过: pdf_id={}", task.pdf_id);
                continue;
            }
            self.pending.push_back(task);
            restored += 1;
        }

        if restored > 0 {
            info!("恢复持久化任务 {} 个", restored);
        }
        restored
    }

    /// 将任务加入队列
    ///
    /// 返回：
    /// - Ok(position) - 成功加入队列，返回队列位置（0 表示立即执行）
    /// - Err - 任务已在队列中或正在运行
    pub fn enqueue(&mut self, pdf_id: i64) -> Result<usize, String> {
        // 检查是否已在队列中
        if self.running == Some(pdf_id) {
            warn!("任务正在运行，无法重复添加: pdf_id={}", pdf_id);
            return Err("任务正在运行中".to_string());
        }

        if self.pending.iter().any(|t| t.pdf_id == pdf_id) {
            warn!("任务已在队列中: pdf_id={}", pdf_id);
            return Err("任务已在队列中".to_string());
        }

        let task = OcrTask {
            pdf_id,
            created_at: Utc::now(),
        };

        // 如果没有正在运行的任务，立即开始
        if self.running.is_none() {
            self.running = Some(pdf_id);
            info!("任务立即开始: pdf_id={}", pdf_id);
            return Ok(0);
        }

        // 否则加入队列
        self.pending.push_back(task);
        let position = self.pending.len();
        info!("任务加入队列: pdf_id={}, 位置={}", pdf_id, position);
        Ok(position)
    }

    /// 获取下一个待处理任务
    ///
    /// 当当前任务完成时调用此方法
    pub fn get_next(&mut self) -> Option<OcrTask> {
        if let Some(task) = self.pending.pop_front() {
            self.running = Some(task.pdf_id);
            info!("开始下一个任务: pdf_id={}", task.pdf_id);
            return Some(task);
        }

        self.running = None;
        debug!("队列为空，无待处理任务");
        None
    }

    /// 标记任务完成
    pub fn complete(&mut self, pdf_id: i64) {
        if self.running == Some(pdf_id) {
            info!("任务完成: pdf_id={}", pdf_id);
            self.running = None;
        } else {
            warn!("尝试完成不在运行中的任务: pdf_id={}", pdf_id);
        }
    }

    /// 取消任务
    ///
    /// 返回是否成功取消
    pub fn cancel(&mut self, pdf_id: i64) -> bool {
        // 如果是正在运行的任务，不能取消
        if self.running == Some(pdf_id) {
            warn!("无法取消正在运行的任务: pdf_id={}", pdf_id);
            return false;
        }

        // 从队列中移除
        let initial_len = self.pending.len();
        self.pending.retain(|t| t.pdf_id != pdf_id);

        if self.pending.len() < initial_len {
            info!("任务已取消: pdf_id={}", pdf_id);
            return true;
        }

        warn!("任务不存在: pdf_id={}", pdf_id);
        false
    }

    /// 获取任务在队列中的位置
    ///
    /// 返回 None 表示任务不在队列中或正在运行
    pub fn get_position(&self, pdf_id: i64) -> Option<usize> {
        if self.running == Some(pdf_id) {
            return Some(0); // 正在运行，位置为 0
        }

        self.pending
            .iter()
            .position(|t| t.pdf_id == pdf_id)
            .map(|p| p + 1) // 队列位置从 1 开始
    }

    /// 获取队列状态
    pub fn get_status(&self) -> QueueStatus {
        QueueStatus {
            current: self.running,
            pending: self.pending.iter().cloned().collect(),
        }
    }

    /// 检查是否有任务正在运行
    pub fn is_busy(&self) -> bool {
        self.running.is_some()
    }

    /// 获取队列长度
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.running.is_none()
    }

    /// 检查指定 PDF 是否在队列中或正在运行
    pub fn contains(&self, pdf_id: i64) -> bool {
        self.running == Some(pdf_id) || self.pending.iter().any(|t| t.pdf_id == pdf_id)
    }

    /// 清空队列（工作线程致命错误时的兜底：放弃本次全部任务，避免重启后反复崩溃）
    pub fn clear(&mut self) {
        if let Some(cur) = self.running {
            warn!("清空队列：放弃运行中任务 pdf_id={}", cur);
        }
        if !self.pending.is_empty() {
            warn!("清空队列：放弃 {} 个排队任务", self.pending.len());
        }
        self.running = None;
        self.pending.clear();
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enqueue_first_task() {
        let mut queue = TaskQueue::new();
        let result = queue.enqueue(1);
        assert_eq!(result, Ok(0)); // 第一个任务立即开始
        assert!(queue.is_busy());
    }

    #[test]
    fn test_enqueue_second_task() {
        let mut queue = TaskQueue::new();
        queue.enqueue(1).unwrap();
        let result = queue.enqueue(2);
        assert_eq!(result, Ok(1)); // 第二个任务排在位置 1
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_duplicate_task() {
        let mut queue = TaskQueue::new();
        queue.enqueue(1).unwrap();
        let result = queue.enqueue(1);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_next() {
        let mut queue = TaskQueue::new();
        queue.enqueue(1).unwrap();
        queue.enqueue(2).unwrap();

        let next = queue.get_next();
        assert_eq!(next.map(|t| t.pdf_id), Some(2)); // 第一个任务"完成"后获取下一个
    }

    #[test]
    fn test_cancel() {
        let mut queue = TaskQueue::new();
        queue.enqueue(1).unwrap();
        queue.enqueue(2).unwrap();

        // 不能取消正在运行的任务
        assert!(!queue.cancel(1));

        // 可以取消队列中的任务
        assert!(queue.cancel(2));
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_get_position() {
        let mut queue = TaskQueue::new();
        queue.enqueue(1).unwrap();
        queue.enqueue(2).unwrap();
        queue.enqueue(3).unwrap();

        assert_eq!(queue.get_position(1), Some(0)); // 正在运行
        assert_eq!(queue.get_position(2), Some(1)); // 队列位置 1
        assert_eq!(queue.get_position(3), Some(2)); // 队列位置 2
        assert_eq!(queue.get_position(99), None); // 不存在
    }

    #[test]
    fn test_restore_pending_tasks() {
        let mut queue = TaskQueue::new();
        let tasks = vec![
            OcrTask { pdf_id: 7, created_at: Utc::now() },
            OcrTask { pdf_id: 8, created_at: Utc::now() },
        ];

        // 恢复：全部进入 pending，running 仍为空（等待消费方取队首）
        let restored = queue.restore(tasks);
        assert_eq!(restored, 2);
        assert!(!queue.is_busy());
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.get_position(7), Some(1));
        assert_eq!(queue.get_position(8), Some(2));

        // 去重：重复 pdf_id 不会二次入队
        let again = queue.restore(vec![OcrTask { pdf_id: 7, created_at: Utc::now() }]);
        assert_eq!(again, 0);
        assert_eq!(queue.len(), 2);

        // 恢复后按顺序消费：先 7 后 8
        let first = queue.get_next().unwrap();
        assert_eq!(first.pdf_id, 7);
        assert!(queue.is_busy());
        queue.complete(7);
        let second = queue.get_next().unwrap();
        assert_eq!(second.pdf_id, 8);
    }

    /// 致命错误兜底：clear 必须同时清空 running 与 pending
    #[test]
    fn test_clear() {
        let mut queue = TaskQueue::new();
        queue.enqueue(1).unwrap();
        queue.enqueue(2).unwrap();
        assert!(queue.is_busy());
        assert_eq!(queue.len(), 1);

        queue.clear();
        assert!(queue.is_empty());
        assert!(!queue.is_busy());
        assert_eq!(queue.get_position(1), None);
        assert_eq!(queue.get_position(2), None);

        // clear 后可正常复用
        assert_eq!(queue.enqueue(3), Ok(0));
    }
}