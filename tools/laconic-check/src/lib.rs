//! laconic-check's rules as a library, so the fuzz targets in `fuzz/` can call them. The
//! `laconic-check` binary is the supported interface.

pub mod lint;
pub mod preserve;
#[cfg(test)]
mod seeded_faults;
