//! Helpers shared by the integration-test binaries; each test crate
//! compiles this module separately.

/// Today's UTC date, `YYYY-MM-DD`, for grows whose forecast must
/// start now.
#[must_use]
pub fn started_today() -> String {
    let today = time::OffsetDateTime::now_utc().date();
    format!(
        "{:04}-{:02}-{:02}",
        today.year(),
        u8::from(today.month()),
        today.day()
    )
}
