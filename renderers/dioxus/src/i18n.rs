//! The renderer's `t('…')`: the web's `createTranslate(messages, locale)`. The program's catalog for the page's
//! locale first, then the UI catalog (`orbital_core::i18n::ui`: that locale, then English), then the key.

use std::cell::RefCell;
use std::collections::BTreeMap;

std::thread_local! {
    static LOCALE: RefCell<Option<String>> = const { RefCell::new(None) };
    static MESSAGES: RefCell<BTreeMap<String, String>> = const { RefCell::new(BTreeMap::new()) };
}

/// Paint in `locale` with the program's `messages` for it; `None` is English.
pub fn set_locale(locale: Option<String>, messages: BTreeMap<String, String>) {
    LOCALE.with(|slot| *slot.borrow_mut() = locale);
    MESSAGES.with(|slot| *slot.borrow_mut() = messages);
}

/// The locale being painted (`en` when none was set).
pub fn locale() -> String {
    LOCALE
        .with(|slot| slot.borrow().clone())
        .unwrap_or_else(|| "en".to_string())
}

/// The message for `key`: the program's catalog, then the UI catalog for the locale, then English, then the key.
pub fn t(key: &str) -> String {
    match MESSAGES.with(|slot| slot.borrow().get(key).cloned()) {
        Some(message) => message,
        None => orbital_core::i18n::ui::ui_message(&locale(), key),
    }
}

/// [`t`] with each `{{name}}` replaced by its value.
pub fn t_with(key: &str, params: &[(&str, String)]) -> String {
    orbital_core::i18n::ui::interpolate(&t(key), params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_catalog_wins_then_the_ui_catalog_then_english_then_the_key() {
        set_locale(
            Some("ar".into()),
            BTreeMap::from([("common.cancel".to_string(), "program".to_string())]),
        );
        assert_eq!(t("common.cancel"), "program");
        assert_eq!(
            t("common.save"),
            orbital_core::i18n::ui::ui_message("ar", "common.save")
        );
        assert_ne!(t("common.save"), "Save", "control: ar is not English");
        set_locale(Some("fr".into()), BTreeMap::new());
        assert_eq!(t("common.save"), "Save", "an unshipped locale falls to en");
        assert_eq!(t("no.such.key"), "no.such.key");
        set_locale(None, BTreeMap::new());
        assert_eq!(t("common.save"), "Save");
        assert_eq!(locale(), "en");
    }
}
