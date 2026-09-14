//! Timestamp identifiers, the record keys a PDS mints for `tid`-keyed
//! collections. Minting one here lets a record's address be known before
//! it is written (plan 08: a publication's default URL names its own
//! site route, which needs the key).
//!
//! The shape is the protocol's: 64 bits as thirteen characters of
//! base32-sortable, the top bit zero, then 53 bits of microseconds since
//! the Unix epoch, then a 10-bit clock identifier. Keys mint in strictly
//! increasing order within a process, so two minted in the same
//! microsecond still sort as they were made.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use rand_core::RngCore;

/// The base32-sortable alphabet.
const ALPHABET: &[u8; 32] = b"234567abcdefghijklmnopqrstuvwxyz";
/// Characters in a TID.
const LEN: usize = 13;
/// Bits of the clock identifier.
const CLOCK_BITS: u32 = 10;

/// The last microsecond a key was minted at, so keys never repeat or
/// run backwards within a process.
static LAST_MICROS: AtomicU64 = AtomicU64::new(0);

/// A timestamp identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tid(String);

impl Tid {
    /// A fresh key for now. Strictly greater than any this process
    /// minted before.
    pub fn now() -> Self {
        let micros = u64::try_from(jiff::Timestamp::now().as_microsecond()).unwrap_or(0);
        let micros = LAST_MICROS
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |last| {
                Some(micros.max(last + 1))
            })
            .map_or(micros, |last| micros.max(last + 1));
        // The clock id is random so two processes rarely collide.
        let clock = u64::from(rand_core::OsRng.next_u32()) & ((1 << CLOCK_BITS) - 1);
        Self::from_parts(micros, clock)
    }

    /// A key from its parts. The timestamp is truncated to 53 bits.
    pub fn from_parts(micros: u64, clock: u64) -> Self {
        let value = ((micros & ((1 << 53) - 1)) << CLOCK_BITS) | (clock & ((1 << CLOCK_BITS) - 1));
        let mut out = String::with_capacity(LEN);
        for i in 0..LEN {
            // Five bits per character, most significant first.
            let shift = 5 * (LEN - 1 - i);
            #[allow(clippy::cast_possible_truncation)] // masked to five bits
            let index = ((value >> shift) & 0x1f) as usize;
            out.push(char::from(ALPHABET[index]));
        }
        Self(out)
    }

    /// Whether `s` has the shape of a TID: thirteen base32-sortable
    /// characters, the first among `234567ab` (the top bit clear).
    pub fn is_valid(s: &str) -> bool {
        s.len() == LEN && s.bytes().all(|b| ALPHABET.contains(&b)) && s.as_bytes()[0] <= b'b'
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_thirteen_sortable_characters_that_increase() {
        let a = Tid::now();
        let b = Tid::now();
        assert!(Tid::is_valid(a.as_str()), "{a}");
        assert!(Tid::is_valid(b.as_str()), "{b}");
        assert!(a < b, "{a} < {b}");
        assert_eq!(a.to_string().len(), 13);
    }

    #[test]
    fn encoding_matches_the_protocol() {
        // 2026-09-13T12:00:00Z in microseconds, clock id 0: the leading
        // character encodes the top bits of the timestamp.
        let tid = Tid::from_parts(1_789_300_800_000_000, 0);
        assert!(Tid::is_valid(tid.as_str()));
        assert!(tid.as_str().ends_with("22"), "{tid}: a zero clock id");
        assert_eq!(Tid::from_parts(0, 0).as_str(), "2222222222222");
        assert_eq!(Tid::from_parts(0, 1).as_str(), "2222222222223");
        assert_eq!(Tid::from_parts(1, 0).as_str(), "2222222222322");
        assert!(Tid::from_parts(5, 1023) < Tid::from_parts(6, 0));
    }

    #[test]
    fn validity() {
        assert!(Tid::is_valid("3jzfcijpj2z2a"));
        assert!(!Tid::is_valid("3jzfcijpj2z2"), "too short");
        assert!(!Tid::is_valid("0jzfcijpj2z2a"), "0 is not in the alphabet");
        assert!(!Tid::is_valid("zjzfcijpj2z2a"), "top bit set");
        assert!(!Tid::is_valid("cjzfcijpj2z2a"), "top bit set");
    }
}
