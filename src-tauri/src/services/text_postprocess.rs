/// OCR 文本后处理模块
/// 提供常见的错误校正和格式化功能

/// 常见标点符号映射表（英文标点 -> 中文标点）
fn get_punctuation_map() -> std::collections::HashMap<char, char> {
    let mut map = std::collections::HashMap::new();

    // 常见标点符号修正（英文标点 -> 中文标点）
    map.insert(',', '，');
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

/// 检查字符是否可能是被误识别的数字
/// OCR 常见错误：'l' 被识别为 '1'，'O' 被识别为 '0'
fn should_be_digit(ch: char, prev: Option<char>, next: Option<char>) -> Option<char> {
    match ch {
        // 小写 L 可能被误识别为数字 1
        'l' | 'L' | '|' | 'I' => {
            // 如果前后都是数字，或者前一个字符是数字且后一个是数字/标点
            let prev_is_digit = prev.map_or(false, |c| c.is_ascii_digit());
            let next_is_digit = next.map_or(false, |c| c.is_ascii_digit());
            let prev_is_amount = prev.map_or(false, |c| c == '￥' || c == '$' || c == '¥');
            let next_is_unit = next.map_or(false, |c| c == '元' || c == '万' || c == '亿' || c == '年' || c == '月' || c == '日');

            // 数字上下文：前后有数字、金额符号、单位
            if prev_is_digit || next_is_digit || prev_is_amount || next_is_unit {
                return Some('1');
            }
            // 日期模式：年月日前的数字
            if let Some(n) = next {
                if n == '年' || n == '月' || n == '日' {
                    if prev_is_digit || prev.map_or(false, |c| c == '月' || c == '年') {
                        return Some('1');
                    }
                }
            }
            None
        }
        // 大写 O 可能被误识别为数字 0
        'O' | 'o' | 'D' => {
            let prev_is_digit = prev.map_or(false, |c| c.is_ascii_digit());
            let next_is_digit = next.map_or(false, |c| c.is_ascii_digit());
            let prev_is_amount = prev.map_or(false, |c| c == '￥' || c == '$' || c == '¥');

            // 数字上下文：前后有数字、金额符号
            if prev_is_digit || next_is_digit || prev_is_amount {
                return Some('0');
            }
            None
        }
        _ => None,
    }
}

/// 修正 OCR 数字识别错误（上下文感知）
/// OCR 常将数字 1 识别为 l，数字 0 识别为 O
fn fix_ocr_number_errors(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return text.to_string();
    }

    let mut result = Vec::with_capacity(chars.len());

    for (i, &ch) in chars.iter().enumerate() {
        let prev = if i > 0 { Some(chars[i - 1]) } else { None };
        let next = if i + 1 < chars.len() { Some(chars[i + 1]) } else { None };

        // 尝试修正数字识别错误
        if let Some(corrected) = should_be_digit(ch, prev, next) {
            result.push(corrected);
        } else {
            result.push(ch);
        }
    }

    result.into_iter().collect()
}

/// 后处理 OCR 识别文本
/// 包括：数字错误修正、标点符号规范化、空行合并
pub fn postprocess_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    // 先修正数字识别错误
    let text = fix_ocr_number_errors(text);

    let punctuation_map = get_punctuation_map();
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

        // 标点符号修正（英文标点 -> 中文标点）
        let corrected = punctuation_map.get(&ch).copied().unwrap_or(ch);

        // 特殊处理：句号（需要判断是否为小数点）
        let corrected = if ch == '.' {
            // 判断是否为小数点：前后都是数字
            let prev_is_digit = result.chars().last().map_or(false, |c| c.is_ascii_digit());
            // 由于我们已经遍历到当前字符，无法直接查看下一个字符
            // 使用简化逻辑：如果前一个字符是数字，保持为英文句号（可能是小数点）
            if prev_is_digit {
                '.'
            } else {
                '。'
            }
        } else {
            corrected
        };

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
            '，', '。', '、', '；', '：', '？', '！', '"', '"', '\'', '\'', '（', '）', '【', '】',
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
        let input = "Hello,World:";
        let result = normalize_punctuation(input);
        assert!(result.contains('，'));
        assert!(result.contains('：'));
    }

    #[test]
    fn test_fix_ocr_number_errors() {
        // 数字上下文中的 l 应该被修正为 1
        let input = "金额: l00元";
        let result = postprocess_text(input);
        assert!(result.contains("100"), "Expected '100' in '{}'", result);

        // 金额上下文中的 O 应该被修正为 0
        let input2 = "总计: 5O元";
        let result2 = postprocess_text(input2);
        assert!(result2.contains("50"), "Expected '50' in '{}'", result2);

        // 非数字上下文中的 l 不应该被修正
        let input3 = "hello world";
        let result3 = postprocess_text(input3);
        assert!(result3.contains("hello"), "Expected 'hello' in '{}'", result3);
    }

    #[test]
    fn test_date_number_fix() {
        // 日期格式中的数字
        let input = "2026年l月2O日";
        let result = postprocess_text(input);
        assert!(result.contains("1月") || result.contains("20日"), "Result: {}", result);
    }
}