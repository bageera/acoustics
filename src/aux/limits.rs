//! The G8/N24 memory and render LIMITS for the auxiliary-stream subsystem
//! (docs/neuro-overlay-spec.md §5). Enforced BEFORE any expensive work.

/// The maximum auxiliary stream count per sidecar.
pub const MAX_STREAMS: usize = 8;

/// The legal carrier-frequency bound in millihertz (0.5 Hz ..= 20 kHz).
pub const CARRIER_MHZ_MIN: u32 = 500;
pub const CARRIER_MHZ_MAX: u32 = 20_000_000;

/// The legal modulation-frequency bound in millihertz (0.1 ..= 12 Hz).
pub const MOD_MHZ_MIN: u32 = 100;
pub const MOD_MHZ_MAX: u32 = 12_000;

/// The maximum per-stream render duration in samples (spec §5).
pub const MAX_DURATION_SAMPLES: u64 = 1u64 << 32;

/// The legal primary sample-rate bound (spec §5).
pub const RATE_MIN: u32 = 8_000;
pub const RATE_MAX: u32 = 192_000;

/// The maximum sidecar file length in bytes (16 + 8×38 + 6).
pub const MAX_AUX_FILE_BYTES: usize = 16 + MAX_STREAMS * 38 + 6;
