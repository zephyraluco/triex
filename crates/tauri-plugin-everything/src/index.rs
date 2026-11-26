use std::{cmp::Ordering, path::Path};

use crate::{Error, ItemKind, SearchItem, SearchPage, SearchRequest, Sort};

pub(crate) struct Entry {
    item: SearchItem,
    name: String,
    path: String,
}

impl Entry {
    pub(crate) fn new(item: SearchItem) -> Self {
        Self {
            name: item.name.to_lowercase(),
            path: item.path.to_lowercase(),
            item,
        }
    }
}

enum Filter {
    Name(String),
    Path(String),
    Extension(Vec<String>),
    Kind(ItemKind),
}

impl Filter {
    fn matches(&self, entry: &Entry) -> bool {
        match self {
            Self::Name(value) => entry.name.contains(value),
            Self::Path(value) => entry.path.contains(value),
            Self::Extension(values) => {
                entry.item.kind == ItemKind::File
                    && Path::new(&entry.name)
                        .extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|ext| values.iter().any(|value| value == ext))
            }
            Self::Kind(kind) => *kind == entry.item.kind,
        }
    }
}

fn filters(input: &str) -> Result<Vec<(bool, Filter)>, Error> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    for ch in input.chars() {
        match ch {
            '"' => quoted = !quoted,
            ch if ch.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            _ => word.push(ch),
        }
    }
    if quoted {
        return Err(Error::InvalidRequest);
    }
    if !word.is_empty() {
        words.push(word);
    }

    words
        .into_iter()
        .flat_map(|word| {
            let (negated, term) = word
                .strip_prefix('!')
                .map_or((false, word.as_str()), |word| (true, word));
            let term = term.to_lowercase();
            if let Some(ext) = term.strip_prefix("ext:") {
                let values: Vec<_> = ext
                    .split(';')
                    .map(|ext| ext.trim_start_matches('.').to_owned())
                    .collect();
                return vec![if values.iter().any(String::is_empty) {
                    Err(Error::InvalidRequest)
                } else {
                    Ok((negated, Filter::Extension(values)))
                }];
            }
            if let Some(path) = term.strip_prefix("path:") {
                return vec![if path.is_empty() {
                    Err(Error::InvalidRequest)
                } else {
                    Ok((negated, Filter::Path(path.into())))
                }];
            }
            for (prefix, kind) in [("file:", ItemKind::File), ("folder:", ItemKind::Folder)] {
                if let Some(name) = term.strip_prefix(prefix) {
                    let filter = Ok((negated, Filter::Kind(kind)));
                    return if name.is_empty() {
                        vec![filter]
                    } else if negated {
                        vec![Err(Error::InvalidRequest)]
                    } else {
                        vec![filter, Ok((false, Filter::Name(name.into())))]
                    };
                }
            }
            if term.contains(['\\', '/']) {
                vec![Ok((negated, Filter::Path(term.replace('/', "\\"))))]
            } else if term.is_empty() || term.contains([':', '*', '?', '|']) {
                vec![Err(Error::InvalidRequest)]
            } else {
                vec![Ok((negated, Filter::Name(term)))]
            }
        })
        .collect()
}

fn compare(a: &Entry, b: &Entry, sort: Sort) -> Ordering {
    let order = match sort {
        Sort::NameAscending => a.name.cmp(&b.name),
        Sort::NameDescending => b.name.cmp(&a.name),
        Sort::PathAscending => a.path.cmp(&b.path),
        Sort::PathDescending => b.path.cmp(&a.path),
        Sort::SizeAscending => missing_last(a.item.size_bytes, b.item.size_bytes, false),
        Sort::SizeDescending => missing_last(a.item.size_bytes, b.item.size_bytes, true),
        Sort::ModifiedAscending => missing_last(a.item.modified_at, b.item.modified_at, false),
        Sort::ModifiedDescending => missing_last(a.item.modified_at, b.item.modified_at, true),
    };
    order
        .then_with(|| a.path.cmp(&b.path))
        .then_with(|| a.item.path.cmp(&b.item.path))
}

fn missing_last<T: Ord>(a: Option<T>, b: Option<T>, descending: bool) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) if descending => b.cmp(&a),
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub(crate) fn search(entries: &[Entry], request: SearchRequest) -> Result<SearchPage, Error> {
    let predicates = filters(&request.query)?;
    let mut matches: Vec<_> = entries
        .iter()
        .filter(|entry| {
            !predicates.is_empty()
                && predicates
                    .iter()
                    .all(|(negated, filter)| filter.matches(entry) != *negated)
        })
        .collect();
    let total = u32::try_from(matches.len()).unwrap_or(u32::MAX);
    let start = (request.offset as usize).min(matches.len());
    let end = (start + request.limit.unwrap_or(50) as usize).min(matches.len());
    let ordering = |a: &&Entry, b: &&Entry| compare(a, b, request.sort);
    if start < end {
        if start != 0 {
            matches.select_nth_unstable_by(start, ordering);
        }
        if end < matches.len() {
            matches[start..].select_nth_unstable_by(end - start, ordering);
        }
        matches[start..end].sort_unstable_by(ordering);
    }
    Ok(SearchPage {
        items: matches[start..end]
            .iter()
            .map(|entry| entry.item.clone())
            .collect(),
        total,
        next_offset: (end < total as usize).then_some(end as u32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, size: Option<u64>, kind: ItemKind) -> Entry {
        Entry::new(SearchItem {
            name: name.into(),
            path: format!(r"C:\财务报表\{name}"),
            kind,
            size_bytes: size,
            modified_at: None,
        })
    }

    #[test]
    fn filters_unicode_paths_and_paginates_by_size() {
        let entries = [
            entry("年度 报告.PDF", Some(8), ItemKind::File),
            entry("年度 报告 副本.pdf", Some(4), ItemKind::File),
            entry("年度 报告 草稿.pdf", Some(15), ItemKind::File),
            entry("年度 报告.pdf", None, ItemKind::Folder),
        ];
        let request = |offset| SearchRequest {
            query: r#"path:"财务报表" "年度 报告" ext:pdf !草稿"#.into(),
            offset,
            limit: Some(1),
            sort: Sort::SizeDescending,
        };
        let first = search(&entries, request(0)).unwrap();
        assert_eq!(first.total, 2);
        assert_eq!(first.items[0].name, "年度 报告.PDF");
        assert_eq!(first.next_offset, Some(1));
        let second = search(&entries, request(1)).unwrap();
        assert_eq!(second.items[0].name, "年度 报告 副本.pdf");
        assert_eq!(second.next_offset, None);
        assert!(search(&entries, request(2)).unwrap().items.is_empty());
    }

    #[test]
    fn unsupported_operators_fail_explicitly() {
        for query in [r#""unclosed"#, "content:secret", "report*"] {
            assert!(matches!(filters(query), Err(Error::InvalidRequest)));
        }
    }
}
