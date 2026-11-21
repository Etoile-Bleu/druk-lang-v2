//! The math builtins (`sqrt`, `abs`, `min`, `max`, `random`). Druk numbers
//! are whole numbers, so `sqrt` is the integer square root rounded down.
//! `random` is deterministic: every run starts from the same fixed seed, so a
//! classroom exercise prints the same numbers on every machine.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::{RtValue, TAG_NUMBER};

const RANDOM_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

static RANDOM_STATE: AtomicU64 = AtomicU64::new(RANDOM_SEED);

fn number_of(value: RtValue) -> i64 {
    if value.tag == TAG_NUMBER {
        value.payload
    } else {
        0
    }
}

fn integer_sqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut root = (n as f64).sqrt() as i64;
    while root.checked_mul(root).is_none_or(|square| square > n) {
        root -= 1;
    }
    while (root + 1)
        .checked_mul(root + 1)
        .is_some_and(|square| square <= n)
    {
        root += 1;
    }
    root
}

fn next_random() -> u64 {
    let mut current = RANDOM_STATE.load(Ordering::Relaxed);
    loop {
        let mut next = current;
        next ^= next << 13;
        next ^= next >> 7;
        next ^= next << 17;
        match RANDOM_STATE.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return next,
            Err(actual) => current = actual,
        }
    }
}

fn random_between(low: i64, high: i64) -> i64 {
    let (low, high) = if low <= high {
        (low, high)
    } else {
        (high, low)
    };
    let span = (high as i128 - low as i128 + 1) as u128;
    let offset = (next_random() as u128) % span;
    (low as i128 + offset as i128) as i64
}

/// Rounded-down square root; a negative number gives 0.
///
/// # Safety
/// `out` and `value` must each point to a valid `RtValue` (`out` writable).
#[no_mangle]
pub unsafe extern "C" fn druk_rt_sqrt(out: *mut RtValue, value: *const RtValue) {
    *out = RtValue::number(integer_sqrt(number_of(*value)));
}

/// # Safety
/// `out` and `value` must each point to a valid `RtValue` (`out` writable).
#[no_mangle]
pub unsafe extern "C" fn druk_rt_abs(out: *mut RtValue, value: *const RtValue) {
    *out = RtValue::number(number_of(*value).saturating_abs());
}

/// # Safety
/// `out`, `left` and `right` must each point to a valid `RtValue` (`out` writable).
#[no_mangle]
pub unsafe extern "C" fn druk_rt_min(
    out: *mut RtValue,
    left: *const RtValue,
    right: *const RtValue,
) {
    *out = RtValue::number(number_of(*left).min(number_of(*right)));
}

/// # Safety
/// `out`, `left` and `right` must each point to a valid `RtValue` (`out` writable).
#[no_mangle]
pub unsafe extern "C" fn druk_rt_max(
    out: *mut RtValue,
    left: *const RtValue,
    right: *const RtValue,
) {
    *out = RtValue::number(number_of(*left).max(number_of(*right)));
}

/// A whole number from `low` to `high`, both included (swapped if given in
/// the wrong order).
///
/// # Safety
/// `out`, `low` and `high` must each point to a valid `RtValue` (`out` writable).
#[no_mangle]
pub unsafe extern "C" fn druk_rt_random(
    out: *mut RtValue,
    low: *const RtValue,
    high: *const RtValue,
) {
    *out = RtValue::number(random_between(number_of(*low), number_of(*high)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_sqrt_rounds_down() {
        assert_eq!(integer_sqrt(0), 0);
        assert_eq!(integer_sqrt(1), 1);
        assert_eq!(integer_sqrt(15), 3);
        assert_eq!(integer_sqrt(16), 4);
        assert_eq!(integer_sqrt(-9), 0);
        assert_eq!(integer_sqrt(i64::MAX), 3_037_000_499);
    }

    #[test]
    fn abs_min_max_work_on_numbers() {
        let (a, b) = (RtValue::number(-7), RtValue::number(3));
        let mut out = RtValue::nil();
        unsafe {
            druk_rt_abs(&mut out, &a);
            assert_eq!(out.payload, 7);
            druk_rt_min(&mut out, &a, &b);
            assert_eq!(out.payload, -7);
            druk_rt_max(&mut out, &a, &b);
            assert_eq!(out.payload, 3);
        }
    }

    #[test]
    fn random_stays_inside_the_range() {
        for _ in 0..1000 {
            let value = random_between(5, 8);
            assert!((5..=8).contains(&value));
        }
        assert!((1..=2).contains(&random_between(2, 1)));
        assert_eq!(random_between(4, 4), 4);
    }
}
