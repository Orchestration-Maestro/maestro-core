//! Local verification windows do not claim a remote change index or snapshot.
use super::full::Mode;
use crate::Refusal;
use maestro_kernel::acquisition::Window;

/// Freeze conservative local bounds using the last fully committed watermark.
/// # Errors
/// A backwards clock or overflowing uncertainty margins cannot prove coverage.
pub fn window(
    watermark: Option<u64>,
    now: u64,
    overlap: u64,
    skew: u64,
) -> Result<Window, Refusal> {
    let margin = overlap.checked_add(skew).ok_or(Refusal::Invalid)?;
    if watermark.is_some_and(|previous| previous > now) {
        return Err(Refusal::Invalid);
    }
    Ok(Window {
        start: watermark.unwrap_or(0).saturating_sub(margin),
        end: now,
        overlap,
        skew,
    })
}

/// Pending and never-verified work is due; covered observations may be reused.
#[must_use]
pub fn due(mode: Mode, watermark: Option<u64>, observed: Option<u64>, window: &Window) -> bool {
    if mode == Mode::Full || watermark.is_none() {
        return true;
    }
    observed.is_none_or(|time| time < window.start)
}

#[cfg(test)]
mod mutation_tests {
    #[test]
    fn s6t_incremental_equal_watermark_is_valid() {
        let window = super::window(Some(100), 100, 10, 5).unwrap();
        assert_eq!((window.start, window.end), (85, 100));
        assert_eq!(
            super::window(Some(101), 100, 10, 5),
            Err(crate::Refusal::Invalid)
        );
    }
}
