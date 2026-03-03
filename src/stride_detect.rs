use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use crate::file_handler::MappedFile;

pub struct StrideDetectState {
    rx: Receiver<Vec<StrideCandidate>>,
}

impl StrideDetectState {
    /// Check if results are ready. Returns `Some(candidates)` when the background
    /// thread has finished, `None` while still running.
    pub fn poll(&self) -> Option<Vec<StrideCandidate>> {
        match self.rx.try_recv() {
            Ok(results) => Some(results),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Vec::new()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrideCandidate {
    pub stride: usize,
    pub score: f64,
}

/// Detect likely stride by byte-match or bit-match periodicity.
///
/// In byte mode, counts positions where data[i] == data[i + stride].
/// In bit mode, counts positions where bit[i] == bit[i + stride].
/// When the stride matches a real frame length, sync words line up
/// and produce a sharp spike in the match ratio.
pub fn detect_stride_background(
    data: MappedFile,
    min_stride: usize,
    max_stride: usize,
    num_results: usize,
    bit_mode: bool,
) -> StrideDetectState {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let candidates = if bit_mode {
            detect_stride_bits(data.data(), min_stride, max_stride, num_results)
        } else {
            detect_stride_bytes(data.data(), min_stride, max_stride, num_results)
        };
        let _ = tx.send(candidates);
    });

    StrideDetectState { rx }
}

/// For a given bit lag, get the shifted byte at position `byte_idx`.
/// This extracts 8 bits starting at bit position `byte_idx * 8 + lag`.
#[inline]
fn shifted_byte(data: &[u8], byte_idx: usize, byte_offset: usize, bit_shift: u32) -> u8 {
    let i = byte_idx + byte_offset;
    if bit_shift == 0 {
        data[i]
    } else {
        // Combine two adjacent bytes to extract the shifted 8-bit window
        let hi = u16::from(data[i]) << 8 | u16::from(data[i + 1]);
        #[allow(clippy::cast_possible_truncation)] // intentional: extracting low 8 bits
        let result = (hi >> (8 - bit_shift)) as u8;
        result
    }
}

fn detect_stride_bits(
    data: &[u8],
    min_stride: usize,
    max_stride: usize,
    num_results: usize,
) -> Vec<StrideCandidate> {
    if data.len() < 64 {
        return vec![];
    }

    // Use a larger sample for bit mode — we need more data to
    // distinguish real periodicity from noise at bit-level resolution.
    let sample_bytes = data.len().min(512 * 1024);
    let sample_start = (data.len() - sample_bytes) / 2;
    let sample = &data[sample_start..sample_start + sample_bytes];
    let total_bits = sample.len() * 8;

    let min_lag = min_stride.max(2);
    let max_lag = max_stride.min(total_bits / 4);
    if max_lag < min_lag {
        return vec![];
    }

    // Use as many comparison points as we can afford.
    // Each comparison is just a byte XOR, so we can do a lot.
    let max_byte_offset = max_lag.div_ceil(8) + 1;
    let usable_bytes = sample.len().saturating_sub(max_byte_offset + 1);
    let compare_bytes = 16384usize.min(usable_bytes);
    let byte_step = (usable_bytes / compare_bytes).max(1);

    // First pass: compute raw match counts for each lag
    let raw_scores: Vec<(usize, u32, u32)> = (min_lag..=max_lag)
        .map(|lag| {
            let byte_offset = lag / 8;
            #[allow(clippy::cast_possible_truncation)] // lag % 8 always fits in u32
            let bit_shift = (lag % 8) as u32;
            let mut matching_bytes = 0u32;
            let mut total = 0u32;
            let mut bi = 0usize;
            let limit = sample.len() - byte_offset - if bit_shift > 0 { 2 } else { 1 };
            while bi < limit {
                let a = sample[bi];
                let b = shifted_byte(sample, bi, byte_offset, bit_shift);
                if a == b {
                    matching_bytes += 1;
                }
                total += 1;
                bi += byte_step;
            }
            (lag, matching_bytes, total)
        })
        .collect();

    // Compute baseline expected matches (median) and stddev for z-score.
    // Under null hypothesis (random data), P(byte match) ≈ 1/256.
    let mut match_counts: Vec<u32> = raw_scores.iter().map(|s| s.1).collect();
    match_counts.sort_unstable();
    let median_matches = f64::from(match_counts[match_counts.len() / 2]);
    // Use median as the expected value; estimate stddev from binomial
    let typical_n = f64::from(raw_scores[0].2);
    let p_est = (median_matches / typical_n).max(1.0 / 256.0);
    let stddev = (typical_n * p_est * (1.0 - p_est)).sqrt().max(1.0);

    // Convert to z-scores: how many stddevs above median
    let mut scores: Vec<(usize, f64)> = raw_scores
        .iter()
        .map(|&(lag, matches, _total)| {
            let z = (f64::from(matches) - median_matches) / stddev;
            (lag, z)
        })
        .collect();

    filter_candidates(&mut scores, num_results, 4.0)
}

fn detect_stride_bytes(
    data: &[u8],
    min_stride: usize,
    max_stride: usize,
    num_results: usize,
) -> Vec<StrideCandidate> {
    if data.len() < 64 {
        return vec![];
    }

    let sample_size = data.len().min(512 * 1024);
    let sample_start = (data.len() - sample_size) / 2;
    let sample = &data[sample_start..sample_start + sample_size];

    let min_lag = min_stride.max(2);
    let max_lag = max_stride.min(sample.len() / 4);
    if max_lag < min_lag {
        return vec![];
    }

    let compare_points = 16384usize.min(sample.len() / 2);
    let sub_step = ((sample.len() - max_lag) / compare_points).max(1);

    // First pass: raw match counts
    let raw_scores: Vec<(usize, u32, u32)> = (min_lag..=max_lag)
        .map(|lag| {
            let mut matches = 0u32;
            let mut total = 0u32;
            let mut i = 0;
            while i + lag < sample.len() {
                if sample[i] == sample[i + lag] {
                    matches += 1;
                }
                total += 1;
                i += sub_step;
            }
            (lag, matches, total)
        })
        .collect();

    // Z-score: how many stddevs above the median match count
    let mut match_counts: Vec<u32> = raw_scores.iter().map(|s| s.1).collect();
    match_counts.sort_unstable();
    let median_matches = f64::from(match_counts[match_counts.len() / 2]);
    let typical_n = f64::from(raw_scores[0].2);
    let p_est = (median_matches / typical_n).max(1.0 / 256.0);
    let stddev = (typical_n * p_est * (1.0 - p_est)).sqrt().max(1.0);

    let mut scores: Vec<(usize, f64)> = raw_scores
        .iter()
        .map(|&(lag, matches, _)| {
            let z = (f64::from(matches) - median_matches) / stddev;
            (lag, z)
        })
        .collect();

    filter_candidates(&mut scores, num_results, 4.0)
}

/// Shared filtering: sort by score, prefer fundamentals over harmonics.
/// `min_score` is the minimum score to consider a candidate.
#[allow(clippy::cast_precision_loss)] // stride values are small, f64 is fine
fn filter_candidates(
    scores: &mut [(usize, f64)],
    num_results: usize,
    min_score: f64,
) -> Vec<StrideCandidate> {
    // Sort descending by score, ascending by stride to break ties
    scores.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let mut results: Vec<StrideCandidate> = Vec::new();
    for &(lag, score) in scores.iter() {
        if score < min_score {
            break;
        }

        // Check if this lag is harmonically related to any existing result
        let mut dominated = false;
        let mut is_fundamental_of_any = false;
        for c in &results {
            let (big, small) = if lag >= c.stride {
                (lag, c.stride)
            } else {
                (c.stride, lag)
            };
            if small == 0 {
                continue;
            }
            let ratio = big as f64 / small as f64;
            let nearest_int = ratio.round();
            if nearest_int >= 2.0 && (ratio - nearest_int).abs() < 0.5 {
                if lag < c.stride {
                    is_fundamental_of_any = true;
                } else {
                    dominated = true;
                }
                break;
            }
        }

        if dominated {
            continue;
        }

        if is_fundamental_of_any {
            // This lag is smaller than existing harmonics — it's the fundamental.
            // Remove all harmonics of this lag from results, then add it.
            results.retain(|c| {
                if c.stride <= lag {
                    return true;
                }
                let ratio = c.stride as f64 / lag as f64;
                let nearest_int = ratio.round();
                !(nearest_int >= 2.0 && (ratio - nearest_int).abs() < 0.5)
            });
            results.push(StrideCandidate { stride: lag, score });
        } else {
            results.push(StrideCandidate { stride: lag, score });
        }

        if results.len() >= num_results {
            break;
        }
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}
