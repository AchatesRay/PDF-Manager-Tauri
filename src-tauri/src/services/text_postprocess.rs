/// OCR 文本后处理模块
/// 提供常见的错误校正和格式化功能

/// 常见 OCR 错误字符映射表（中文场景）
fn get_char_correction_map() -> std::collections::HashMap<char, char> {
    let mut map = std::collections::HashMap::new();

    // 常见混淆字符
    map.insert('0', 'O'); // 英文零和字母 O 混淆（根据上下文）
    map.insert('1', 'l'); // 数字 1 和小写 L
    map.insert('|', 'I'); // 竖线和字母 I
    map.insert('「', '【'); // 日文引号修正为中文引号
    map.insert('」', '】');
    map.insert('『', '『');
    map.insert('』', '』');

    // 常见标点符号修正（英文标点 -> 中文标点）
    map.insert(',', '，');
    map.insert('.', '。');
    map.insert(':', '：');
    map.insert(';', '；');
    map.insert('?', '？');
    map.insert('!', '！');
    map.insert('(', '（');
    map.insert(')', '）');
    map.insert('[', '［');
    map.insert(']', '］');

    map
}

/// 后处理 OCR 识别文本
/// 包括：标点符号规范化、常见错误修正、空行合并
pub fn postprocess_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let correction_map = get_char_correction_map();
    let mut result = String::with_capacity(text.len());

    let mut prev_char = '\0';
    let mut consecutive_newlines: u32 = 0;

    for ch in text.chars() {
        // 跳过连续空行（最多保留一个空行）
        if ch == '\n' {
            consecutive_newlines += 1;
            if consecutive_newlines > 2 {
                continue;
            }
        } else {
            consecutive_newlines = 0;
        }

        // 字符修正（英文标点 -> 中文标点）
        let corrected = correction_map.get(&ch).copied().unwrap_or(ch);

        // 避免重复标点（保留中文标点）
        if is_punctuation(corrected) && is_punctuation(prev_char) {
            if !is_ascii_punctuation(corrected) && is_ascii_punctuation(prev_char) {
                result.pop();
                result.push(corrected);
                prev_char = corrected;
                continue;
            }
        }

        result.push(corrected);
        prev_char = corrected;
    }

    // 清理首尾空白
    result.trim().to_string()
}

/// 检查是否为标点符号
fn is_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || [
            '，', '。', '、', '；', '：', '？', '！', '"', '"', ''', ''', '（', '）', '【', '】',
            '《', '》', '…', '—', '～',
        ]
        .contains(&ch)
}

/// 检查是否为 ASCII 标点
fn is_ascii_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

/// 检测文本语言类型
pub fn detect_text_language(text: &str) -> TextLanguage {
    let chinese_chars = text.chars().filter(|&c| is_chinese_char(c)).count();
    let english_chars = text.chars().filter(|&c| c.is_ascii_alphabetic()).count();
    let total_chars = text.chars().filter(|&c| !c.is_whitespace()).count();

    if total_chars == 0 {
        return TextLanguage::Unknown;
    }

    let chinese_ratio = chinese_chars as f32 / total_chars as f32;
    let english_ratio = english_chars as f32 / total_chars as f32;

    if chinese_ratio > 0.3 {
        TextLanguage::Chinese
    } else if english_ratio > 0.5 {
        TextLanguage::English
    } else {
        TextLanguage::Mixed
    }
}

/// 文本语言类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextLanguage {
    Chinese,
    English,
    Mixed,
    Unknown,
}

/// 根据语言类型优化文本
pub fn optimize_by_language(text: &str, lang: TextLanguage) -> String {
    match lang {
        TextLanguage::Chinese => optimize_chinese_text(text),
        TextLanguage::English => optimize_english_text(text),
        _ => postprocess_text(text),
    }
}

/// 优化中文文本
fn optimize_chinese_text(text: &str) -> String {
    let mut result = postprocess_text(text);

    // 规范化标点符号（统一使用全角）
    result = normalize_punctuation(&result);

    result
}

/// 优化英文文本
fn optimize_english_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_was_space = false;

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }

    result
}

/// 规范化标点符号（统一使用全角）
fn normalize_punctuation(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            ',' => '，',
            '.' if !ch.is_ascii_digit() => '。',
            ':' => '：',
            ';' => '；',
            '?' => '？',
            '!' => '！',
            '(' => '（',
            ')' => '）',
            _ => ch,
        })
        .collect()
}

/// 检查是否为中文字符
fn is_chinese_char(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch) ||    // CJK 统一表意文字
    ('\u{3400}'..='\u{4dbf}').contains(&ch) ||    // CJK 扩展 A
    ('\u{f900}'..='\u{faff}').contains(&ch)       // CJK 兼容表意文字
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postprocess_text() {
        let input = "Hello,World.";
        let result = postprocess_text(input);
        assert!(result.contains("，") || result.contains("。"));
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_text_language("中文测试"), TextLanguage::Chinese);
        assert_eq!(detect_text_language("English test"), TextLanguage::English);
    }

    #[test]
    fn test_remove_consecutive_newlines() {
        let input = "Line1\n\n\n\nLine2";
        let result = postprocess_text(input);
        assert!(!result.contains("\n\n\n"));
    }

    #[test]
    fn test_normalize_punctuation() {
        let input = "Hello,World.";
        let result = normalize_punctuation(input);
        assert!(result.contains('，'));
        assert!(result.contains('。'));
    }
}
