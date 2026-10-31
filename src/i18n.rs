/*
 *            ____           _       __    __     _____ __            ___
 *           / __ \_      __(_)___ _/ /_  / /_   / ___// /___  ______/ (_)___
 *          / / / / | /| / / / __ `/ __ \/ __/   \__ \/ __/ / / / __  / / __ \
 *         / /_/ /| |/ |/ / / /_/ / / / / /_    ___/ / /_/ /_/ / /_/ / / /_/ /
 *        /_____/ |__/|__/_/\__, /_/ /_/\__/   /____/\__/\__,_/\__,_/_/\____/
 *                         /____/
 *     Copyright (C) 2025 Dwight Studio
 *
 *     This program is free software: you can redistribute it and/or modify
 *     it under the terms of the GNU General Public License as published by
 *     the Free Software Foundation, either version 3 of the License, or
 *     (at your option) any later version.
 *
 *     This program is distributed in the hope that it will be useful,
 *     but WITHOUT ANY WARRANTY; without even the implied warranty of
 *     MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *     GNU General Public License for more details.
 *
 *     You should have received a copy of the GNU General Public License
 *     along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
use crate::rsc;
use log::{info, warn};
use relm4::gtk::glib;
use tr::{MoTranslator, set_translator};

/// Source language of the strings, used when no translation matches
const SOURCE_LANGUAGE: &str = "en";

/// Loads the embedded translation matching the user's languages, if any.
pub fn init() {
    // Ordered by preference, following LANGUAGE, LC_ALL, LC_MESSAGES and LANG,
    // with fallbacks (e.g. "fr_FR.UTF-8", "fr_FR", "fr", ..., "C")
    for name in glib::language_names() {
        let name = name.as_str();

        if name == "C" || name == SOURCE_LANGUAGE || name.starts_with("en_") {
            break;
        }

        let Some((lang, data)) = rsc::TRANSLATIONS.iter().find(|(lang, _)| *lang == name) else {
            continue;
        };

        match MoTranslator::from_vec_u8(data.to_vec()) {
            Ok(translator) => {
                set_translator!(translator);
                info!("Using the '{lang}' translation");
                return;
            }
            Err(error) => warn!("Failed to load the '{lang}' translation: {error}"),
        }
    }

    info!("Using the source language");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tr::{Translator, tr};

    #[test]
    fn french_translation_is_applied() {
        let (_, data) = rsc::TRANSLATIONS
            .iter()
            .find(|(lang, _)| *lang == "fr")
            .expect("French translation is embedded");

        let translator = MoTranslator::from_vec_u8(data.to_vec()).unwrap();

        // Untranslated strings fall back to the source language. The translator is called
        // directly, so that xgettext does not extract this string.
        let untranslated = "Not a translated string";
        assert_eq!(translator.translate(untranslated, None), untranslated);

        set_translator!(translator);
        assert_eq!(tr!("Quit"), "Quitter");
    }
}
