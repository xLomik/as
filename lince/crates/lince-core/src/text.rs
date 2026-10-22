//! Números y tiempos escritos a la española.

use std::time::Duration;

/// 1234567 → "1.234.567".
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (k, ch) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

/// Número con coma decimal, como se escribe en español.
pub fn decimal(x: f64, places: usize) -> String {
    format!("{x:.places$}").replace('.', ",")
}

pub fn seconds(d: Duration) -> String {
    decimal(d.as_secs_f64(), 3) + " s"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separator() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1.000");
        assert_eq!(thousands(1_234_567), "1.234.567");
    }

    #[test]
    fn spanish_decimals() {
        assert_eq!(decimal(1.26, 1), "1,3");
        assert_eq!(seconds(Duration::from_millis(1500)), "1,500 s");
    }
}
