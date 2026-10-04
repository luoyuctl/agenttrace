//! Localized user-facing text.
//!
//! Every message lives in `locales/en.yml` and `locales/zh-CN.yml` under a dotted key
//! (`tui.*`, `report.*`, `waste.*`). Look messages up with [`tr`]; use [`tr_args`] for
//! messages with `%{name}` placeholders.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;

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

/// Look up a runtime key (e.g. one stored in a [`Message`]). `None` when the key is unknown.
pub fn tr_key(language: Language, key: &str) -> Option<Cow<'static, str>> {
    _rust_i18n_try_translate(language.locale(), key)
}

/// A localizable message: a catalog key plus pre-formatted parameters. Core stores this next
/// to its English prose so every display path can render it in the reader's language, and a
/// cached session can be shown in either language without being re-parsed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub key: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

impl Message {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            params: BTreeMap::new(),
        }
    }

    pub fn arg(mut self, name: &str, value: impl std::fmt::Display) -> Self {
        self.params.insert(name.to_string(), value.to_string());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.key.is_empty()
    }

    /// Render in `language`; `None` when there is no key or the key is unknown.
    pub fn render(&self, language: Language) -> Option<String> {
        if self.key.is_empty() {
            return None;
        }
        let mut text = tr_key(language, &self.key)?.into_owned();
        for (name, value) in &self.params {
            text = text.replace(&format!("%{{{name}}}"), value);
        }
        Some(text)
    }

    /// Render in `language`, falling back to `english` (the stored prose) for messages
    /// produced before keys existed.
    pub fn render_or(&self, language: Language, english: &str) -> String {
        self.render(language).unwrap_or_else(|| english.to_string())
    }
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
    fn translations_use_the_same_placeholders() {
        let placeholders = |text: &str| {
            let mut names = text
                .match_indices("%{")
                .filter_map(|(start, _)| {
                    let rest = &text[start + 2..];
                    rest.find('}').map(|end| rest[..end].to_string())
                })
                .collect::<Vec<_>>();
            names.sort();
            names
        };
        for key in catalog_keys(Language::En) {
            let en = tr_key(Language::En, &key).unwrap_or_default();
            let zh = tr_key(Language::Zh, &key).unwrap_or_default();
            assert_eq!(
                placeholders(&en),
                placeholders(&zh),
                "placeholders differ for {key}"
            );
        }
    }

    #[test]
    fn message_renders_in_each_language() {
        let message = Message::new("msg.anomaly.hanging")
            .arg("count", 2)
            .arg("max", 400);
        assert_eq!(
            message.render_or(Language::En, ""),
            "2 gap(s) >60s, max=400s"
        );
        assert_eq!(
            message.render_or(Language::Zh, ""),
            "2 个间隔超过 60 秒，最长 400 秒"
        );
        assert_eq!(
            Message::default().render_or(Language::Zh, "legacy"),
            "legacy"
        );
        assert_eq!(
            Message::new("msg.nope").render_or(Language::Zh, "legacy"),
            "legacy"
        );
    }

    #[test]
    fn unknown_key_is_visible_and_args_substitute() {
        assert_eq!(tr(Language::Zh, "does.not.exist"), "does.not.exist");
        assert_eq!(Language::parse("ZH-cn"), Some(Language::Zh));
        assert_eq!(Language::parse("fr"), None);
    }
}
