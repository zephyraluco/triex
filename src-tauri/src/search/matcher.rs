use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::search::index::{IndexedItem, SearchResultItem};

const WEIGHT_NAME: f64 = 1.00;
const WEIGHT_PINYIN_ABBR: f64 = 0.92;
const WEIGHT_PINYIN_FULL: f64 = 0.86;
const WEIGHT_KEYWORD: f64 = 0.80;
const PREFIX_BONUS: f64 = 60.0;
const EXACT_MATCH_BONUS: f64 = 120.0;

struct CandidateHit {
    score: f64,
    match_indices: Vec<usize>,
}

/// Evaluates a single indexed item against a pre-parsed nucleo pattern across name,
/// Chinese pinyin initials/full spelling, and aliases.
pub fn score_item(
    matcher: &mut Matcher,
    pattern: &Pattern,
    query_lower: &str,
    item: &IndexedItem,
    utf32_buf: &mut Vec<char>,
    indices_buf: &mut Vec<u32>,
) -> Option<SearchResultItem> {
    let mut best: Option<CandidateHit> = None;

    // 1. Direct name match
    if let Some(raw_score) = run_nucleo(matcher, pattern, &item.raw.name, utf32_buf, indices_buf) {
        let mut score = f64::from(raw_score) * WEIGHT_NAME;
        if item.name_lower == query_lower {
            score += EXACT_MATCH_BONUS;
        } else if item.name_lower.starts_with(query_lower) {
            score += PREFIX_BONUS;
        }
        let mapped: Vec<usize> = indices_buf.iter().map(|&idx| idx as usize).collect();
        update_best(&mut best, score, mapped);
    }

    // 2. Pinyin initials / English acronym match (e.g. "wx" -> "微信", "vsc" -> "Visual Studio Code")
    if !item.pinyin.abbr.is_empty()
        && let Some(raw_score) =
            run_nucleo(matcher, pattern, &item.pinyin.abbr, utf32_buf, indices_buf)
    {
        let mut score = f64::from(raw_score) * WEIGHT_PINYIN_ABBR;
        if item.pinyin.abbr == query_lower {
            score += EXACT_MATCH_BONUS * 0.85;
        } else if item.pinyin.abbr.starts_with(query_lower) {
            score += PREFIX_BONUS * 0.85;
        }
        let mapped = map_pinyin_indices(indices_buf, &item.pinyin.abbr_char_map);
        update_best(&mut best, score, mapped);
    }

    // 3. Full pinyin match (e.g. "weixin" -> "微信")
    if !item.pinyin.full.is_empty()
        && let Some(raw_score) =
            run_nucleo(matcher, pattern, &item.pinyin.full, utf32_buf, indices_buf)
    {
        let mut score = f64::from(raw_score) * WEIGHT_PINYIN_FULL;
        if item.pinyin.full.starts_with(query_lower) {
            score += PREFIX_BONUS * 0.7;
        }
        let mapped = map_pinyin_indices(indices_buf, &item.pinyin.full_char_map);
        update_best(&mut best, score, mapped);
    }

    // 4. Keyword / alias match (e.g. "wechat", "cmd", "calc")
    for keyword in &item.raw.keywords {
        if let Some(raw_score) = run_nucleo(matcher, pattern, keyword, utf32_buf, indices_buf) {
            let mut score = f64::from(raw_score) * WEIGHT_KEYWORD;
            if keyword.eq_ignore_ascii_case(query_lower) {
                score += EXACT_MATCH_BONUS * 0.75;
            }
            update_best(&mut best, score, Vec::new());
        }
    }

    best.map(|hit| SearchResultItem {
        id: item.raw.id.clone(),
        kind: item.raw.kind,
        name: item.raw.name.clone(),
        subtitle: item.raw.subtitle.clone(),
        exec_path: item.raw.exec_path.clone(),
        icon_key: item.raw.icon_key.clone(),
        score: hit.score + item.frecency,
        match_indices: hit.match_indices,
    })
}

pub fn create_matcher() -> Matcher {
    Matcher::new(Config::DEFAULT)
}

pub fn build_pattern(query: &str) -> Pattern {
    Pattern::new(
        query,
        CaseMatching::Smart,
        Normalization::Smart,
        AtomKind::Fuzzy,
    )
}

fn run_nucleo(
    matcher: &mut Matcher,
    pattern: &Pattern,
    haystack: &str,
    utf32_buf: &mut Vec<char>,
    indices_buf: &mut Vec<u32>,
) -> Option<u32> {
    utf32_buf.clear();
    indices_buf.clear();
    let haystack_utf32 = Utf32Str::new(haystack, utf32_buf);
    pattern.indices(haystack_utf32, matcher, indices_buf)
}

fn map_pinyin_indices(indices: &[u32], char_map: &[usize]) -> Vec<usize> {
    let mut mapped = Vec::with_capacity(indices.len());
    for &idx in indices {
        if let Some(&orig) = char_map.get(idx as usize)
            && mapped.last().copied() != Some(orig)
        {
            mapped.push(orig);
        }
    }
    mapped
}

fn update_best(best: &mut Option<CandidateHit>, score: f64, match_indices: Vec<usize>) {
    let replace = match best {
        None => true,
        Some(existing) => score > existing.score,
    };
    if replace {
        *best = Some(CandidateHit {
            score,
            match_indices,
        });
    }
}
