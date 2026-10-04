//! Localized user-facing text.
//!
//! Every message lives in `locales/en.yml` and `locales/zh-CN.yml` under a dotted key
//! (`tui.*`, `report.*`, `waste.*`). Look messages up with [`tr`]; use [`tr_args`] for
//! messages with `%{name}` placeholders.

use std::borrow::Cow;

rust_i18n::i18n!("locales", fallback = "en");

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    En,
    Zh,
}

impl Language {
    pub fn locale(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh-CN",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::En => Self::Zh,
            Self::Zh => Self::En,
        }
    }

    /// Parse a user-supplied language name. Returns `None` for unknown values.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "en" | "english" => Some(Self::En),
            "zh" | "zh-cn" | "zh_cn" | "chinese" => Some(Self::Zh),
            _ => None,
        }
    }
}

/// Look up a message. Unknown keys fall back to English, then to the key itself, so a
/// missing translation is visible instead of silently blank.
pub fn tr(language: Language, key: &'static str) -> &'static str {
    match _rust_i18n_try_translate(language.locale(), key) {
        Some(Cow::Borrowed(text)) => text,
        // Catalog entries are borrowed from static data; this arm is unreachable in practice.
        Some(Cow::Owned(text)) => Box::leak(text.into_boxed_str()),
        None => key,
    }
}

/// Look up a message and substitute `%{name}` placeholders.
pub fn tr_args(
    language: Language,
    key: &'static str,
    args: &[(&str, &dyn std::fmt::Display)],
) -> String {
    let mut text = tr(language, key).to_string();
    for (name, value) in args {
        text = text.replace(&format!("%{{{name}}}"), &value.to_string());
    }
    text
}

/// Keys present in the English catalog, for consistency checks.
pub fn catalog_keys(language: Language) -> Vec<String> {
    let mut keys = _RUST_I18N_BACKEND
        .messages_for_locale(language.locale())
        .unwrap_or_default()
        .into_iter()
        .map(|(key, _)| key.into_owned())
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_catalogs_define_the_same_keys() {
        let en = catalog_keys(Language::En);
        let zh = catalog_keys(Language::Zh);
        let missing_zh = en
            .iter()
            .filter(|key| !zh.contains(key))
            .collect::<Vec<_>>();
        let missing_en = zh
            .iter()
            .filter(|key| !en.contains(key))
            .collect::<Vec<_>>();
        assert!(
            missing_zh.is_empty(),
            "missing in zh-CN.yml: {missing_zh:?}"
        );
        assert!(missing_en.is_empty(), "missing in en.yml: {missing_en:?}");
        assert!(!en.is_empty());
    }

    #[test]
    fn unknown_key_is_visible_and_args_substitute() {
        assert_eq!(tr(Language::Zh, "does.not.exist"), "does.not.exist");
        assert_eq!(Language::parse("ZH-cn"), Some(Language::Zh));
        assert_eq!(Language::parse("fr"), None);
    }
}
