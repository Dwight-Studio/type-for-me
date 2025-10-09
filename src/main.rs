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
mod app;
mod config;
mod util;

use crate::app::{App, Init};
use crate::config::Config;
use fern::colors::ColoredLevelConfig;
use relm4::adw::gio::{Resource, resources_register};
use relm4::adw::glib;
use relm4::{RelmApp, gtk};
use std::env::Args;
use std::time::SystemTime;
use tr::tr_init;

mod rsc {
    include!(concat!(env!("OUT_DIR"), "/resources.rs"));
}
const APP_ID: &str = "fr.dwightstudio.TypeForMe";

fn main() {
    // Initialize logger
    setup_logger().unwrap();

    // Initialize tr
    tr_init!("/usr/share/locale");

    // Load the resources
    let bytes = glib::Bytes::from_static(rsc::GRESOURCE_BYTES);
    let resource = Resource::from_data(&bytes).unwrap();
    resources_register(&resource);

    // Initialize GTK
    gtk::init().unwrap();

    // Add the stylesheet and the icons
    let display = gtk::gdk::Display::default().unwrap();
    gtk::IconTheme::for_display(&display).add_resource_path(rsc::RESOURCE_PREFIX);

    let config = Config::load();
    let silent = std::env::args().any(|arg| arg.contains("--silent"));

    let app = RelmApp::new(APP_ID);
    app.run_async::<App>(Init { silent, config });
}

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339_seconds(SystemTime::now()),
                ColoredLevelConfig::new().color(record.level()),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}
