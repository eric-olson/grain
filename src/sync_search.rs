use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
pub enum Variation {
    Exact,
    ByteSwap16,
    ByteSwap32,
    BitReversed,
    BitInverted,
}

impl std::fmt::Display for Variation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variation::Exact => write!(f, "Exact"),
            Variation::ByteSwap16 => write!(f, "Byte-swap 16"),
            Variation::ByteSwap32 => write!(f, "Byte-swap 32"),
            Variation::BitReversed => write!(f, "Bit-reversed"),
            Variation::BitInverted => write!(f, "Bit-inverted"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub offset: usize,
    pub variation: Variation,
}

pub struct SearchState {
    pub results: Arc<Mutex<Vec<SearchMatch>>>,
    pub done: Arc<Mutex<bool>>,
}

/// Parse a hex string like "1ACFFC1D" into bytes.
pub fn parse_hex_pattern(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim().replace(' ', "");
    if s.len() % 2 != 0 {
        return Err("Hex string must have even length".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn reverse_bits(b: u8) -> u8 {
    b.reverse_bits()
}

fn generate_variations(pattern: &[u8]) -> Vec<(Vec<u8>, Variation)> {
    let mut variations = vec![(pattern.to_vec(), Variation::Exact)];

    // Bit-inverted
    let inverted: Vec<u8> = pattern.iter().map(|b| !b).collect();
    variations.push((inverted, Variation::BitInverted));

    // Bit-reversed (each byte)
    let reversed: Vec<u8> = pattern.iter().map(|b| reverse_bits(*b)).collect();
    variations.push((reversed, Variation::BitReversed));

    // 16-bit byte swap
    if pattern.len() >= 2 && pattern.len() % 2 == 0 {
        let mut swapped = pattern.to_vec();
        for chunk in swapped.chunks_exact_mut(2) {
            chunk.swap(0, 1);
        }
        variations.push((swapped, Variation::ByteSwap16));
    }

    // 32-bit byte swap
    if pattern.len() >= 4 && pattern.len() % 4 == 0 {
        let mut swapped = pattern.to_vec();
        for chunk in swapped.chunks_exact_mut(4) {
            chunk.reverse();
        }
        variations.push((swapped, Variation::ByteSwap32));
    }

    variations
}

fn find_all(data: &[u8], pattern: &[u8]) -> Vec<usize> {
    if pattern.is_empty() || data.len() < pattern.len() {
        return vec![];
    }
    let mut offsets = Vec::new();
    let end = data.len() - pattern.len() + 1;
    for i in 0..end {
        if data[i..i + pattern.len()] == *pattern {
            offsets.push(i);
        }
    }
    offsets
}

/// Launch a background search. Returns a SearchState to poll for results.
pub fn search_background(data: Arc<Vec<u8>>, pattern: Vec<u8>) -> SearchState {
    let results = Arc::new(Mutex::new(Vec::new()));
    let done = Arc::new(Mutex::new(false));

    let results_clone = results.clone();
    let done_clone = done.clone();

    thread::spawn(move || {
        let variations = generate_variations(&pattern);
        for (var_pattern, var_type) in variations {
            let offsets = find_all(&data, &var_pattern);
            let mut res = results_clone.lock().unwrap();
            for offset in offsets {
                res.push(SearchMatch {
                    offset,
                    variation: var_type.clone(),
                });
            }
        }
        // Sort by offset
        let mut res = results_clone.lock().unwrap();
        res.sort_by_key(|m| m.offset);
        *done_clone.lock().unwrap() = true;
    });

    SearchState { results, done }
}
