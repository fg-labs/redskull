// Ported over from https://github.com/conda/conda-build/blob/main/conda_build/license_family.py
use regex::Regex;
use std::sync::LazyLock;

const FAMILIES: &[&str] = &[
    "AGPL",
    "LGPL",
    "GPL3",
    "GPL2",
    "GPL",
    "BSD",
    "MIT",
    "APACHE",
    "PSF",
    "CC",
    "MOZILLA",
    "PUBLIC-DOMAIN",
    "PROPRIETARY",
    "OTHER",
    "NONE",
];

static _AGPL_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("[A,L]GPL").unwrap()); // match AGPL and AGPL
static GPL2_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("GPL[^3]*2").unwrap()); // match GPL2
static GPL3_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("GPL[^2]*3").unwrap()); // match GPL3
static GPL23_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("GPL[^2]*>= *2").unwrap()); // match GPL >= 2
static CC_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"CC\w+").unwrap()); // match CC
static PUNK_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("[[:punct:]]").unwrap()); // removes punks
static _GPL_LONG: LazyLock<Regex> = LazyLock::new(|| Regex::new("GENERAL PUBLIC LICENSE").unwrap());
static _LESSER: LazyLock<Regex> = LazyLock::new(|| Regex::new("LESSER *").unwrap());
static _AFFERO: LazyLock<Regex> = LazyLock::new(|| Regex::new("AFFERO *").unwrap());

/// True if family matches GPL3 or GPL >= 2, else False
fn match_gpl3(family: &str) -> bool {
    GPL23_REGEX.is_match(family) || GPL3_REGEX.is_match(family)
}

/// Set to ALL CAPS, replace common GPL patterns, and strip
fn normalize(s: &str) -> String {
    let s = s.to_uppercase();
    let s = _GPL_LONG.replace(&s, "GPL");
    let s = _LESSER.replace(&s, "GPL");
    let s = _AFFERO.replace(&s, "GPL");
    s.trim().to_string()
}

/// Remove punctuation, spaces, tabs, and line feeds
fn remove_special_characters(s: &str) -> String {
    let s = PUNK_REGEX.replace(s, " ");
    let s = s.replace(' ', "");
    s.to_string()
}

/// Return best guess of `license_family` from the conda package index.
/// Note: Logic here is simple, and focuses on existing set of allowed families
pub fn guess_license_family(license_name: &str) -> String {
    let license_name = normalize(license_name);

    // Handle GPL families as special cases
    // Remove AGPL and LGPL before looking for GPL2 and GPL3
    let sans_lgpl = _AGPL_REGEX.replace(&license_name, "");
    if match_gpl3(&sans_lgpl) {
        return "GPL3".to_string();
    } else if GPL2_REGEX.is_match(&sans_lgpl) {
        return "GPL2".to_string();
    } else if CC_REGEX.is_match(&license_name) {
        return "CC".to_string();
    }

    let license_name = remove_special_characters(&license_name);
    for family in FAMILIES {
        let family = remove_special_characters(family);
        if license_name.contains(&family) {
            return family.to_string();
        }
    }
    for family in FAMILIES {
        let family = remove_special_characters(family);
        if family.contains(&license_name) {
            return family.to_string();
        }
    }

    "OTHER".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mit() {
        assert_eq!(guess_license_family("MIT"), "MIT");
    }
    #[test]
    fn test_apache() {
        assert_eq!(guess_license_family("Apache-2.0"), "APACHE");
    }
    #[test]
    fn test_gpl2() {
        assert_eq!(guess_license_family("GPL-2.0"), "GPL2");
    }
    #[test]
    fn test_gpl3() {
        assert_eq!(guess_license_family("GPL-3.0"), "GPL3");
    }
    #[test]
    fn test_gpl3_or_later() {
        assert_eq!(guess_license_family("GPL-3.0-or-later"), "GPL3");
    }
    #[test]
    fn test_bsd() {
        assert_eq!(guess_license_family("BSD-3-Clause"), "BSD");
    }
    #[test]
    fn test_lgpl() {
        assert_eq!(guess_license_family("LGPL-3.0"), "LGPL");
    }
    #[test]
    fn test_agpl() {
        assert_eq!(guess_license_family("AGPL-3.0"), "AGPL");
    }
    #[test]
    fn test_cc() {
        assert_eq!(guess_license_family("CC0-1.0"), "CC");
    }
    #[test]
    fn test_unknown() {
        assert_eq!(guess_license_family("WeirdLicense"), "OTHER");
    }

    #[test]
    fn test_dual_mit_apache() {
        let result = guess_license_family("MIT OR Apache-2.0");
        assert!(
            result == "MIT" || result == "APACHE",
            "Expected MIT or APACHE for dual license, got: {result}"
        );
    }

    #[test]
    fn test_long_form_lgpl() {
        let result = guess_license_family("LGPL-2.1-only");
        assert_eq!(result, "LGPL");
    }
}
