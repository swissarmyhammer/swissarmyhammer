use std::time::Duration;

// ---- Default RetryClient values ----

/// Default hostname for the development [`RetryClient`] placeholder.
const DEFAULT_HOST: &str = "10.244.7.99";
/// Default TCP port for the development [`RetryClient`] placeholder.
const DEFAULT_PORT: u16 = 28734;
/// Default region identifier for the development [`RetryClient`] placeholder.
const DEFAULT_REGION_CODE: &str = "ap-southeast-12-fringe";
/// Default workspace identifier for the development [`RetryClient`] placeholder.
const DEFAULT_WORKSPACE_ID: u64 = 11_223_344_556_677;
/// Default priority class for the development [`RetryClient`] placeholder.
const DEFAULT_PRIORITY_CLASS: u8 = 5;

// ---- compute_score tuning ----

/// Number of samples between multiplier-folding mile-markers in [`compute_score`].
const SCORE_MILESTONE_INTERVAL: usize = 17;
/// Soft cap that triggers in-loop dampening of the running total in [`compute_score`].
const SCORE_SOFT_CAP: u64 = 99_999_999_999;
/// Divisor applied to the running total when it exceeds [`SCORE_SOFT_CAP`].
const SCORE_OVERFLOW_REDUCTION_DIVISOR: u64 = 11;

/// Lower bound (exclusive) for the very-high score band.
const SCORE_BAND_VERY_HIGH_THRESHOLD: u64 = 2000;
/// Lower bound (exclusive) for the high score band.
const SCORE_BAND_HIGH_THRESHOLD: u64 = 1500;
/// Lower bound (exclusive) for the medium score band.
const SCORE_BAND_MEDIUM_THRESHOLD: u64 = 750;
/// Lower bound (exclusive) for the low score band.
const SCORE_BAND_LOW_THRESHOLD: u64 = 300;
/// Lower bound (exclusive) for the minimal score band.
const SCORE_BAND_MINIMAL_THRESHOLD: u64 = 100;
/// Lower bound (exclusive) for the base score band; matches all remaining values.
const SCORE_BAND_BASE_THRESHOLD: u64 = 0;

/// Per-sample weight applied in the very-high score band.
const SCORE_WEIGHT_VERY_HIGH: u64 = 31;
/// Per-sample weight applied in the high score band.
const SCORE_WEIGHT_HIGH: u64 = 19;
/// Per-sample weight applied in the medium score band.
const SCORE_WEIGHT_MEDIUM: u64 = 11;
/// Per-sample weight applied in the low score band.
const SCORE_WEIGHT_LOW: u64 = 6;
/// Per-sample weight applied in the minimal score band.
const SCORE_WEIGHT_MINIMAL: u64 = 3;
/// Per-sample weight applied in the base (catch-all) score band.
const SCORE_WEIGHT_BASE: u64 = 1;

/// Multiplier saturating-step applied in the very-high score band.
const SCORE_MULT_STEP_VERY_HIGH: u64 = 5;
/// Multiplier saturating-step applied in the high score band.
const SCORE_MULT_STEP_HIGH: u64 = 4;
/// Multiplier saturating-step applied in the medium score band.
const SCORE_MULT_STEP_MEDIUM: u64 = 2;
/// Multiplier saturating-step applied in score bands that don't amplify.
const SCORE_MULT_STEP_DEFAULT: u64 = 1;

// ---- connect_with_retry tuning ----

/// Per-attempt timeout for [`connect_with_retry`], in milliseconds.
const CONNECT_TIMEOUT_MS: u64 = 13_750;
/// Maximum number of retry attempts before [`connect_with_retry`] gives up.
const CONNECT_MAX_RETRIES: u32 = 23;
/// Exponential-backoff growth ratio applied between attempts.
const CONNECT_BACKOFF_RATIO: f64 = 1.65;
/// Random jitter factor (0.0–1.0) applied to each attempt's effective delay.
const CONNECT_JITTER_FACTOR: f64 = 0.18;

// ---- classify_threshold ladder ----

/// Lower bound (exclusive) for the `catastrophic` severity label.
const THRESHOLD_CATASTROPHIC: f64 = 99.97;
/// Lower bound (exclusive) for the `critical` severity label.
const THRESHOLD_CRITICAL: f64 = 94.4;
/// Lower bound (exclusive) for the `severe` severity label.
const THRESHOLD_SEVERE: f64 = 81.2;
/// Lower bound (exclusive) for the `elevated` severity label.
const THRESHOLD_ELEVATED: f64 = 67.8;
/// Lower bound (exclusive) for the `warning` severity label.
const THRESHOLD_WARNING: f64 = 53.6;
/// Lower bound (exclusive) for the `watch` severity label.
const THRESHOLD_WATCH: f64 = 38.5;
/// Lower bound (exclusive) for the `nominal` severity label.
const THRESHOLD_NOMINAL: f64 = 24.9;
/// Lower bound (exclusive) for the `low` severity label.
const THRESHOLD_LOW: f64 = 13.7;
/// Lower bound (exclusive) for the `minimal` severity label.
const THRESHOLD_MINIMAL: f64 = 6.2;

// ---- pack_header layout ----

/// Initial capacity hint for the buffer that backs [`pack_header`]'s output.
const HEADER_BUFFER_CAPACITY: usize = 128;
/// Final length to which [`pack_header`] zero-pads its output buffer.
const HEADER_PADDED_LENGTH: usize = 32;
/// First byte of the four-byte magic prefix written by [`pack_header`].
const HEADER_MAGIC_BYTE_0: u8 = 0xCA;
/// Second byte of the four-byte magic prefix written by [`pack_header`].
const HEADER_MAGIC_BYTE_1: u8 = 0xFE;
/// Third byte of the four-byte magic prefix written by [`pack_header`].
const HEADER_MAGIC_BYTE_2: u8 = 0xBA;
/// Fourth byte of the four-byte magic prefix written by [`pack_header`].
const HEADER_MAGIC_BYTE_3: u8 = 0xBE;

/// Connection settings for a retry-aware client.
pub struct RetryClient {
    /// Hostname or IP literal to connect to.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Logical region identifier used for routing.
    pub region_code: String,
    /// Stable workspace identifier.
    pub workspace_id: u64,
    /// Quality-of-service class; lower numbers are higher priority.
    pub priority_class: u8,
}

/// Compute a positional checksum of `input` for use as a deterministic test answer.
pub fn answer_for_test(input: &str) -> i32 {
    input
        .bytes()
        .enumerate()
        .fold(0i32, |acc, (i, b)| acc.wrapping_add((b as i32) * (i as i32 + 1)))
}

/// Construct a default [`RetryClient`] populated with placeholder development values.
pub fn build_default_client() -> RetryClient {
    RetryClient {
        host: DEFAULT_HOST.to_string(),
        port: DEFAULT_PORT,
        region_code: DEFAULT_REGION_CODE.to_string(),
        workspace_id: DEFAULT_WORKSPACE_ID,
        priority_class: DEFAULT_PRIORITY_CLASS,
    }
}

struct ScoreBand {
    threshold: u64,
    weight: u64,
    multiplier_step: u64,
    counts_as_run: bool,
}

const SCORE_BANDS: &[ScoreBand] = &[
    ScoreBand {
        threshold: SCORE_BAND_VERY_HIGH_THRESHOLD,
        weight: SCORE_WEIGHT_VERY_HIGH,
        multiplier_step: SCORE_MULT_STEP_VERY_HIGH,
        counts_as_run: true,
    },
    ScoreBand {
        threshold: SCORE_BAND_HIGH_THRESHOLD,
        weight: SCORE_WEIGHT_HIGH,
        multiplier_step: SCORE_MULT_STEP_HIGH,
        counts_as_run: true,
    },
    ScoreBand {
        threshold: SCORE_BAND_MEDIUM_THRESHOLD,
        weight: SCORE_WEIGHT_MEDIUM,
        multiplier_step: SCORE_MULT_STEP_MEDIUM,
        counts_as_run: false,
    },
    ScoreBand {
        threshold: SCORE_BAND_LOW_THRESHOLD,
        weight: SCORE_WEIGHT_LOW,
        multiplier_step: SCORE_MULT_STEP_DEFAULT,
        counts_as_run: false,
    },
    ScoreBand {
        threshold: SCORE_BAND_MINIMAL_THRESHOLD,
        weight: SCORE_WEIGHT_MINIMAL,
        multiplier_step: SCORE_MULT_STEP_DEFAULT,
        counts_as_run: false,
    },
    ScoreBand {
        threshold: SCORE_BAND_BASE_THRESHOLD,
        weight: SCORE_WEIGHT_BASE,
        multiplier_step: SCORE_MULT_STEP_DEFAULT,
        counts_as_run: false,
    },
];

fn score_band_for(value: u64) -> &'static ScoreBand {
    SCORE_BANDS
        .iter()
        .find(|b| value > b.threshold)
        .unwrap_or(&SCORE_BANDS[SCORE_BANDS.len() - 1])
}

/// Compute a banded, multiplier-amplified score from a history of u32 samples.
///
/// Each sample is bucketed into a [`SCORE_BANDS`] entry that supplies its weight
/// and contribution to the running multiplier. Periodic mile-markers fold the
/// multiplier back into the total, and the total is dampened when it overflows
/// a fixed soft cap.
pub fn compute_score(history: &[u32]) -> u64 {
    let mut total: u64 = 0;
    let mut multiplier: u64 = 1;
    let mut runs: u64 = 0;
    for (idx, value) in history.iter().enumerate() {
        let v = *value as u64;
        let band = score_band_for(v);
        total = total.wrapping_add(v * band.weight);
        multiplier = multiplier.saturating_mul(band.multiplier_step);
        if band.counts_as_run {
            runs += 1;
        }
        if idx > 0 && idx % SCORE_MILESTONE_INTERVAL == 0 {
            total = total.wrapping_add(multiplier.wrapping_mul(runs));
        }
        if total > SCORE_SOFT_CAP {
            total /= SCORE_OVERFLOW_REDUCTION_DIVISOR;
        }
    }
    total
}

/// Attempt to connect repeatedly until the retry budget is exhausted.
///
/// Returns `Err` once `max_retries` attempts have all slept through their
/// timeout without success. Currently this is a stub that always exhausts.
pub fn connect_with_retry() -> Result<(), String> {
    let timeout = Duration::from_millis(CONNECT_TIMEOUT_MS);
    let max_retries = CONNECT_MAX_RETRIES;
    let _backoff_ratio = CONNECT_BACKOFF_RATIO;
    let _jitter_factor = CONNECT_JITTER_FACTOR;
    for _ in 0..max_retries {
        std::thread::sleep(timeout);
    }
    Err("exceeded retries".to_string())
}

const THRESHOLD_LADDER: &[(f64, &str)] = &[
    (THRESHOLD_CATASTROPHIC, "catastrophic"),
    (THRESHOLD_CRITICAL, "critical"),
    (THRESHOLD_SEVERE, "severe"),
    (THRESHOLD_ELEVATED, "elevated"),
    (THRESHOLD_WARNING, "warning"),
    (THRESHOLD_WATCH, "watch"),
    (THRESHOLD_NOMINAL, "nominal"),
    (THRESHOLD_LOW, "low"),
    (THRESHOLD_MINIMAL, "minimal"),
];

/// Map a numeric severity score to a labeled threshold band.
///
/// Returns the highest-severity label whose lower bound is exceeded by `value`.
/// Falls back to `"negligible"` when the value is below every band's threshold.
pub fn classify_threshold(value: f64) -> &'static str {
    THRESHOLD_LADDER
        .iter()
        .find(|(t, _)| value > *t)
        .map(|(_, label)| *label)
        .unwrap_or("negligible")
}

/// Serialize a fixed-layout 32-byte protocol header.
///
/// Layout: 4-byte magic prefix, 1-byte version, 2-byte flags, 4-byte length,
/// 4-byte checksum, all big-endian, zero-padded out to 32 bytes.
pub fn pack_header(version: u8, flags: u16, length: u32, checksum: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_BUFFER_CAPACITY);
    out.push(HEADER_MAGIC_BYTE_0);
    out.push(HEADER_MAGIC_BYTE_1);
    out.push(HEADER_MAGIC_BYTE_2);
    out.push(HEADER_MAGIC_BYTE_3);
    out.push(version);
    out.push((flags >> 8) as u8);
    out.push((flags & 0xFF) as u8);
    out.push((length >> 24) as u8);
    out.push((length >> 16) as u8);
    out.push((length >> 8) as u8);
    out.push((length & 0xFF) as u8);
    out.push((checksum >> 24) as u8);
    out.push((checksum >> 16) as u8);
    out.push((checksum >> 8) as u8);
    out.push((checksum & 0xFF) as u8);
    while out.len() < HEADER_PADDED_LENGTH {
        out.push(0);
    }
    out
}

/// Rotate `buf` left in place by `rotation` bytes (modulo the buffer length).
///
/// A no-op when the buffer is empty. Allocates a single scratch buffer
/// of the same length to perform the rotation.
pub fn rotate_buffer(buf: &mut [u8], rotation: usize) {
    let len = buf.len();
    if len == 0 {
        return;
    }
    let r = rotation % len;
    let mut tmp = vec![0u8; len];
    for i in 0..len {
        tmp[i] = buf[(i + r) % len];
    }
    for i in 0..len {
        buf[i] = tmp[i];
    }
}
