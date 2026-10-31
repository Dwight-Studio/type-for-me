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
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

const DIRNAME: &str = "type-for-me";
const FILENAME: &str = "config.toml";

static CONFIG_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));

    base.join(DIRNAME).join(FILENAME)
});

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub(crate) rd_token: Option<String>,
    pub(crate) typing_period: Duration,
    pub(crate) typing_delay: Duration,
    pub(crate) autostart: bool,
    pub(crate) newlines_as_spaces: bool,
}

impl Config {
    pub fn save(&self) {
        let path = &*CONFIG_PATH;

        info!("Saving config to {}", path.display());

        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            warn!("Failed to create config directory: {}", error);
        }

        match toml::to_string(self) {
            Ok(contents) => match File::create(path) {
                Ok(mut file) => {
                    if let Err(e) = file.write_all(contents.as_bytes()) {
                        warn!("Failed to write config: {}", e);
                    }
                }
                Err(error) => {
                    warn!("Failed to create config file: {}", error);
                }
            },
            Err(error) => {
                warn!("Failed to serialize config: {}", error);
            }
        }
    }
    pub fn load() -> Self {
        let path = &*CONFIG_PATH;

        info!("Loading config from '{}'", path.display());

        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(contents) => match toml::from_str(&contents) {
                    Ok(config) => {
                        return config;
                    }
                    Err(error) => {
                        warn!("Failed to parse config file: {}", error);
                    }
                },
                Err(error) => {
                    warn!("Failed to read config file: {}", error);
                }
            }
        } else {
            info!("Config file doesn't exist");

            if let Some(parent) = path.parent() {
                match std::fs::create_dir_all(parent) {
                    Ok(_) => {
                        info!("Created config directory");
                    }
                    Err(error) => {
                        warn!("Failed to create config directory: {}", error);
                    }
                }
            } else {
                warn!("Failed to get parent directory");
            }
        }

        Config::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rd_token: None,
            typing_period: Duration::from_millis(10),
            typing_delay: Duration::from_millis(250),
            autostart: true,
            newlines_as_spaces: false,
        }
    }
}
