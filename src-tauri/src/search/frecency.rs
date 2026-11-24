use crate::storage::apps_repo::UsageRecord;

/// Half-life of recency decay in seconds (7 days).
const HALF_LIFE_SECS: f64 = 7.0 * 24.0 * 60.0 * 60.0;

/// Computes a bounded frecency boost combining launch frequency and exponential recency decay.
pub fn compute_frecency(record: UsageRecord, now_secs: i64) -> f64 {
    if record.launch_count == 0 {
        return 0.0;
    }

    let elapsed = (now_secs - record.last_used_at).max(0) as f64;
    let decay = 0.5_f64.powf(elapsed / HALF_LIFE_SECS);
    let freq_weight = (record.launch_count as f64).ln_1p();

    // Scale into a range comparable to nucleo-matcher scores so frecency breaks ties
    // and gently promotes frequent apps without overriding strong exact matches.
    (freq_weight * 40.0 + 30.0) * (0.25 + 0.75 * decay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_launches_should_rank_higher_than_old_launches() {
        let now = 1_700_000_000;
        let recent = UsageRecord {
            launch_count: 5,
            last_used_at: now - 60,
        };
        let old = UsageRecord {
            launch_count: 5,
            last_used_at: now - 30 * 24 * 3600,
        };
        assert!(compute_frecency(recent, now) > compute_frecency(old, now));
    }

    #[test]
    fn zero_launches_should_return_zero() {
        let score = compute_frecency(
            UsageRecord {
                launch_count: 0,
                last_used_at: 1_700_000_000,
            },
            1_700_000_000,
        );
        assert_eq!(score, 0.0);
    }
}
