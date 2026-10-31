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

use gvdb::gresource::{BundleBuilder, FileData, PreprocessOptions};
use std::ffi::OsStr;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::{env, fs};

// Build script for Type for Me
// The ressource handling has been largely inspired
// by relm4-icons-build (https://github.com/Relm4/icons)

/// GRessource prefix.
pub const PREFIX: &str = "/fr/dwightstudio/TypeForMe";

/// Application ID, used to name the icon and metainfo files.
pub const APP_ID: &str = "fr.dwightstudio.TypeForMe";

/// Ressource directory.
pub const RESOURCES_DIR_NAME: &str = "rsc";

/// Unique identifier added to the icon names.
pub const UNIQUE_IDENTIFIER: &str = "typeforme";

/// Translations directory.
pub const PO_DIR_NAME: &str = "po";

fn main() {
    // Translations, also needed by the resources (translated metainfo)
    let mut source_file = BufWriter::new(
        File::create(
            Path::new(&env::var("OUT_DIR").expect("Missing OUT_DIR env var"))
                .join("translations.rs"),
        )
        .expect("Failed to create translations source file"),
    );
    compile_translations(&mut source_file);
    source_file.flush().expect("Failed to flush source file");

    // Resources
    bundle_resources()
}

fn bundle_resources() {
    let out_dir = env::var("OUT_DIR").expect("Missing OUT_DIR env var");
    let out_dir = Path::new(&out_dir);
    let rsc_dir = Path::new(RESOURCES_DIR_NAME);

    // The resource bundle file data
    let mut resource_data = Vec::new();

    // The rust source file
    let mut source_file = BufWriter::new(
        File::create(out_dir.join("resources.rs")).expect("Failed to create resource source file"),
    );

    // Tell cargo to rerun the build if something changes
    println!("cargo:rerun-if-changed={RESOURCES_DIR_NAME}");

    // Add all icons
    generate_icons(rsc_dir, &mut resource_data, &mut source_file);

    // Add the application icon, so it is available even when the app is not installed
    resource_data.push(
        FileData::from_file(
            format!("{PREFIX}/scalable/apps/{APP_ID}.svg"),
            &rsc_dir.join(format!("{APP_ID}.svg")),
            true,
            &PreprocessOptions::xml_stripblanks(),
        )
        .expect("Failed to create app icon file data"),
    );

    // Add the translated metainfo, used by the about dialog
    let metainfo = out_dir.join(format!("{APP_ID}.metainfo.xml"));
    msgfmt(&[
        "--xml".as_ref(),
        "--template".as_ref(),
        rsc_dir.join(format!("{APP_ID}.metainfo.xml")).as_os_str(),
        "-d".as_ref(),
        PO_DIR_NAME.as_ref(),
        "-o".as_ref(),
        metainfo.as_os_str(),
    ]);
    resource_data.push(
        FileData::from_file(
            format!("{PREFIX}/metainfo.xml"),
            &metainfo,
            true,
            &PreprocessOptions::empty(),
        )
        .expect("Failed to create metainfo file data"),
    );

    // Generate resource bundle
    let data = BundleBuilder::from_file_data(resource_data)
        .build()
        .expect("Failed to build resource bundle");

    // Write ressource bundle file
    fs::write(out_dir.join("resources.gresource"), data)
        .expect("Failed to write resource bundle file");

    write!(
        source_file,
        "/// GResource file contents\n\
        pub const GRESOURCE_BYTES: &[u8] = include_bytes!(\"resources.gresource\");\n\
        /// Resource prefix used in generated `.gresource` file\n\
        pub const RESOURCE_PREFIX: &str = \"{PREFIX}\";\n\
        /// Resource path of the AppStream metainfo\n\
        pub const METAINFO: &str = \"{PREFIX}/metainfo.xml\";"
    )
    .expect("Failed to write");

    source_file.flush().expect("Failed to flush source file");
}

/// Compiles the translations listed in `po/LINGUAS` and embeds them in the binary
fn compile_translations(source_file: &mut BufWriter<File>) {
    let out_dir = env::var("OUT_DIR").expect("Missing OUT_DIR env var");
    let locale_dir = Path::new(&out_dir).join("locale");
    fs::create_dir_all(&locale_dir).expect("Failed to create locale directory");

    println!("cargo:rerun-if-changed={PO_DIR_NAME}");

    let linguas = fs::read_to_string(Path::new(PO_DIR_NAME).join("LINGUAS"))
        .expect("Failed to read po/LINGUAS");
    let languages: Vec<&str> = linguas
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    writeln!(
        source_file,
        "/// Compiled translations (language code, `.mo` file contents)\n\
        pub const TRANSLATIONS: &[(&str, &[u8])] = &["
    )
    .expect("Failed to write");

    for lang in languages {
        let mo_file = locale_dir.join(format!("{lang}.mo"));
        msgfmt(&[
            "--check".as_ref(),
            "-o".as_ref(),
            mo_file.as_os_str(),
            Path::new(PO_DIR_NAME)
                .join(format!("{lang}.po"))
                .as_os_str(),
        ]);
        writeln!(
            source_file,
            "    (\"{lang}\", include_bytes!(\"locale/{lang}.mo\")),"
        )
        .expect("Failed to write");
    }

    writeln!(source_file, "];").expect("Failed to write");
}

/// Runs `msgfmt` from GNU gettext
fn msgfmt(args: &[&OsStr]) {
    let status = Command::new("msgfmt")
        .args(args)
        .status()
        .expect("Failed to run msgfmt: GNU gettext is required to build the translations");

    if !status.success() {
        panic!("msgfmt failed ({status}) with arguments {args:?}");
    }
}

fn generate_icons(
    rsc_dir: &Path,
    file_data: &mut Vec<FileData>,
    source_file: &mut BufWriter<File>,
) {
    // Tell cargo to rerun the build if something changes
    println!("cargo:rerun-if-changed={RESOURCES_DIR_NAME}/icons/");

    let read_dir = fs::read_dir(rsc_dir.join("icons"))
        .expect("Couldn't open the resource path (relative to the manifest)");
    for entry in read_dir.flatten() {
        if let Some(icon) = path_to_icon_name(&entry.file_name()) {
            let path = entry.path();

            // Write the resource bundle file data
            file_data.push(
                FileData::from_file(
                    format!("{PREFIX}/scalable/actions/{UNIQUE_IDENTIFIER}-{icon}-symbolic.svg"),
                    &path,
                    true,
                    &PreprocessOptions::xml_stripblanks(),
                )
                .expect("Failed to create file data"),
            );

            // Add the rust source constants
            let const_name = icon.to_uppercase().replace('-', "_");
            let path = path.display();
            write!(
                source_file,
                "/// Icon name of the icon `{icon}`, found at `{path}`\n\
            pub const {const_name}: &str = \"{UNIQUE_IDENTIFIER}-{icon}\";\n"
            )
            .expect("Failed to write")
        }
    }
}

pub fn path_to_icon_name(string: &OsStr) -> Option<String> {
    match string.to_str() {
        Some(string) => {
            if string.ends_with(".svg") {
                Some(
                    string
                        .trim_end_matches("-symbolic.svg")
                        .trim_end_matches(".svg")
                        .to_owned(),
                )
            } else {
                println!("Found non-icon file `{string}`, ignoring");
                None
            }
        }
        None => panic!("Failed to convert file name `{string:?}` to string"),
    }
}
