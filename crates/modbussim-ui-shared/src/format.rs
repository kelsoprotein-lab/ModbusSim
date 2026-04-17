//! Value formatting helpers shared by both front-ends.
//!
//! Will host the Rust port of shared-frontend/src/composables/useValueFormat.ts
//! (Float32/UInt32 multi-endian, Hex/Bin/Signed renderers). Stubbed for S0.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum U16Format {
    Unsigned,
    Signed,
    Hex,
    Binary,
}

pub fn format_u16(value: u16, fmt: U16Format) -> String {
    match fmt {
        U16Format::Unsigned => value.to_string(),
        U16Format::Signed => (value as i16).to_string(),
        U16Format::Hex => format!("0x{:04X}", value),
        U16Format::Binary => {
            let b = format!("{:016b}", value);
            format!("{} {} {} {}", &b[0..4], &b[4..8], &b[8..12], &b[12..16])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_and_signed() {
        assert_eq!(format_u16(0xFFFF, U16Format::Unsigned), "65535");
        assert_eq!(format_u16(0xFFFF, U16Format::Signed), "-1");
    }

    #[test]
    fn hex_and_binary() {
        assert_eq!(format_u16(0x00AB, U16Format::Hex), "0x00AB");
        assert_eq!(
            format_u16(0b1010_0101_1100_0011, U16Format::Binary),
            "1010 0101 1100 0011"
        );
    }
}
