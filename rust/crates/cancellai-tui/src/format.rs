//! Human-readable byte formatting (E09-S02). Mirrors `cancellai.py::format_bytes` exactly
//! (binary 1024 divisor, `B`/`KB`/`MB`/`GB`/`TB` labels, integer bytes but two decimals above
//! that) so the TUI reads the same unit convention as the rest of the product, rather than
//! inventing a second one.

const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

pub fn format_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    for (index, unit) in UNITS.iter().enumerate() {
        let is_last = index == UNITS.len() - 1;
        if value < 1024.0 || is_last {
            return if *unit == "B" {
                format!("{value:.0} {unit}")
            } else {
                format!("{value:.2} {unit}")
            };
        }
        value /= 1024.0;
    }
    unreachable!("UNITS is non-empty, so the loop always returns")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_bytes_is_an_integer_b() {
        assert_eq!(format_bytes(0), "0 B");
    }

    #[test]
    fn sub_kilobyte_stays_in_bytes() {
        assert_eq!(format_bytes(999), "999 B");
    }

    #[test]
    fn exactly_one_kib_rolls_over() {
        assert_eq!(format_bytes(1024), "1.00 KB");
    }

    #[test]
    fn megabytes_and_gigabytes_format_with_two_decimals() {
        assert_eq!(format_bytes(1_048_576), "1.00 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.00 GB");
    }

    #[test]
    fn a_huge_value_caps_at_terabytes_rather_than_an_undefined_unit() {
        assert_eq!(
            format_bytes(u64::MAX),
            format!("{:.2} TB", u64::MAX as f64 / 1024f64.powi(4))
        );
    }
}
