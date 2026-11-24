use pinyin::ToPinyin;

/// Precomputed Chinese pinyin projections paired with source character index mappings
/// so matched pinyin indices can highlight the original Chinese characters accurately.
#[derive(Debug, Clone)]
pub struct PinyinProjection {
    /// Full pinyin string (e.g. "weixin" for "微信", preserving ASCII letters/digits).
    pub full: String,
    /// Maps each char index in `full` -> original char index in `name`.
    pub full_char_map: Vec<usize>,
    /// Initials abbreviation (e.g. "wx" for "微信", "vsc" for "Visual Studio Code").
    pub abbr: String,
    /// Maps each char index in `abbr` -> original char index in `name`.
    pub abbr_char_map: Vec<usize>,
}

impl PinyinProjection {
    pub fn build(text: &str) -> Self {
        let mut full = String::with_capacity(text.len() * 2);
        let mut full_char_map = Vec::with_capacity(text.len() * 2);
        let mut abbr = String::with_capacity(text.len());
        let mut abbr_char_map = Vec::with_capacity(text.len());

        let mut prev_was_alnum = false;
        let mut prev_was_lower = false;

        for (char_idx, ch) in text.chars().enumerate() {
            if let Some(py) = ch.to_pinyin() {
                let plain = py.plain();
                for py_ch in plain.chars() {
                    full.push(py_ch);
                    full_char_map.push(char_idx);
                }
                if let Some(first_py_ch) = plain.chars().next() {
                    abbr.push(first_py_ch);
                    abbr_char_map.push(char_idx);
                }
                prev_was_alnum = false;
                prev_was_lower = false;
            } else {
                let lower = ch.to_ascii_lowercase();
                if lower.is_alphanumeric() {
                    full.push(lower);
                    full_char_map.push(char_idx);

                    // Record ASCII word boundary initials (space/separator or camelCase transition).
                    let is_word_start = !prev_was_alnum || (prev_was_lower && ch.is_uppercase());
                    if is_word_start {
                        abbr.push(lower);
                        abbr_char_map.push(char_idx);
                    }
                    prev_was_alnum = true;
                    prev_was_lower = ch.is_lowercase();
                } else {
                    prev_was_alnum = false;
                    prev_was_lower = false;
                }
            }
        }

        Self {
            full,
            full_char_map,
            abbr,
            abbr_char_map,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_build_pinyin_and_char_maps_for_chinese_text() {
        let proj = PinyinProjection::build("微信");
        assert_eq!(proj.full, "weixin");
        assert_eq!(proj.abbr, "wx");
        assert_eq!(proj.abbr_char_map, vec![0, 1]);
        assert_eq!(proj.full_char_map, vec![0, 0, 0, 1, 1, 1]);
    }

    #[test]
    fn should_extract_ascii_word_initials_and_camelcase() {
        let proj = PinyinProjection::build("Visual Studio Code");
        assert_eq!(proj.abbr, "vsc");
        assert_eq!(proj.abbr_char_map, vec![0, 7, 14]);
    }

    #[test]
    fn should_handle_mixed_chinese_and_english() {
        let proj = PinyinProjection::build("QQ音乐");
        assert_eq!(proj.full, "qqyinle");
        assert_eq!(proj.abbr, "qyl");
        assert_eq!(proj.abbr_char_map, vec![0, 2, 3]);
    }
}
