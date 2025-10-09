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
use crate::config::Config;
use crate::{handle_invalidation, logged_send, rsc};
use anyhow::Result;
use ashpd::WindowIdentifier;
use ashpd::desktop::background::{Background, BackgroundProxy};
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop};
use ashpd::desktop::{PersistMode, Session};
use ashpd::enumflags2::BitFlags;
use futures::StreamExt;
use log::{debug, error, info, warn};
use relm4::adw::gdk;
use relm4::adw::prelude::*;
use relm4::gtk::Adjustment;
use relm4::loading_widgets::LoadingWidgets;
use relm4::prelude::*;
use relm4::view;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;
use tr::tr;
use xkbcommon::xkb::Keysym;

const SHORTCUT_ID_PASTE: &str = "paste";
const SAVE_DELAY: Duration = Duration::from_secs(1);

pub struct App {
    // UI
    remote_desktop_ok: bool,
    shortcut_ok: bool,
    background_ok: bool,

    // Other
    config: Config,
    last_save: SystemTime,
    identifier: Option<WindowIdentifier>,
    sc_opt: Option<(
        GlobalShortcuts<'static>,
        Session<'static, GlobalShortcuts<'static>>,
    )>,
    rd_opt: Option<(
        RemoteDesktop<'static>,
        Session<'static, RemoteDesktop<'static>>,
    )>,
}

pub struct Init {
    pub(crate) silent: bool,
    pub(crate) config: Config,
}

#[derive(Debug)]
pub enum AppMsg {
    TypingPeriodChanged(Duration),
    TypingDelayChanged(Duration),
    CreateShortcuts,
    CreateRemoteDesktop,
    CreateBackground,
    SaveConfig,
}

#[derive(Debug)]
pub enum CmdMsg {
    Paste,
    InvalidateShortcuts,
    InvalidateRemoteDesktop,
}

//noinspection RsSortImplTraitMembers
#[relm4::component(pub async)]
impl AsyncComponent for App {
    type Init = Init;
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = CmdMsg;

    view! {
        adw::ApplicationWindow::builder()
            .title("Type for Me")
            .default_width(540)
            .default_height(540)
            .hide_on_close(true)
            .build()
        {
            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    set_show_title: true,

                    /*
                    pack_end = &gtk::Button {
                        set_icon_name: rsc::MENU_LARGE,

                        //connect_clicked => AppMsg::Test,
                    }
                    */
                },

                #[name = "toast"]
                gtk::CenterBox {
                    set_orientation: gtk::Orientation::Vertical,
                    set_vexpand: true,

                    #[wrap(Some)]
                    set_start_widget = &adw::Banner{
                        #[watch]
                        set_title: &{
                            if !model.remote_desktop_ok {
                                tr!("Remote desktop permission with keyboard access is required")
                            } else if !model.shortcut_ok {
                                tr!("Shortcuts are not configured")
                            } else if !model.background_ok {
                                tr!("Background activity permission is required")
                            } else {
                                "".to_string()
                            }
                        },

                        #[watch]
                        set_revealed: !(model.remote_desktop_ok && model.shortcut_ok && model.background_ok),
                    },

                    #[wrap(Some)]
                    set_center_widget = &adw::Clamp {
                        set_maximum_size: 500,
                        set_tightening_threshold: 600,

                        adw::StatusPage {
                            set_title: &tr!("Paste everywhere"),
                            set_description: Some(&tr!("Convert your clipboard into a auto-typer!")),
                            set_icon_name: Some(rsc::KEYBOARD),

                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 12,

                                adw::PreferencesGroup {
                                    set_title: &tr!("Preferences"),

                                    adw::SpinRow {
                                        set_title: &tr!("Typing period (ms)"),
                                        set_adjustment: Some(&Adjustment::new(model.config.typing_period.as_secs_f64() * 1000f64, 0f64, u64::MAX as f64, 5f64, 0f64, 0f64)),
                                        set_snap_to_ticks: true,

                                        connect_value_notify[sender] => move |elem| {
                                            logged_send!(
                                                sender.input_sender(),
                                                AppMsg::TypingPeriodChanged(Duration::from_millis(elem.value() as u64))
                                            );
                                        },
                                    },

                                    adw::SpinRow {
                                        set_title: &tr!("Typing delay (ms)"),
                                        set_adjustment: Some(&Adjustment::new(model.config.typing_delay.as_secs_f64() * 1000f64, 0f64, u64::MAX as f64, 5f64, 0f64, 0f64)),
                                        set_snap_to_ticks: true,

                                        connect_value_notify[sender] => move |elem| {
                                            logged_send!(
                                                sender.input_sender(),
                                                AppMsg::TypingDelayChanged(Duration::from_millis(elem.value() as u64))
                                            );
                                        },
                                    },
                                },

                                adw::PreferencesGroup {
                                    set_title: &tr!("Permissions"),

                                    adw::ActionRow {
                                        set_title: &tr!("Shortcuts"),
                                        set_subtitle: &tr!("Change the bindings in the the applications settings of your desktop environment."),

                                        add_suffix = &gtk::Box{
                                            set_spacing: 12,

                                            gtk::Image {
                                                set_icon_name: Some(rsc::WARNING_OUTLINE),

                                                #[watch]
                                                set_visible: !model.shortcut_ok
                                            },

                                            gtk::Button {
                                                set_label: &tr!("Configure"),
                                                set_valign: gtk::Align::Center,

                                                #[watch]
                                                set_sensitive: !model.shortcut_ok,

                                                connect_clicked => AppMsg::CreateShortcuts,
                                            }
                                        }
                                    },

                                    adw::ActionRow {
                                        set_title: &tr!("Remote desktop"),
                                        set_subtitle: &tr!("Access to the keyboard to send key strokes."),

                                        add_suffix = &gtk::Box{
                                            set_spacing: 12,

                                            gtk::Image {
                                                set_icon_name: Some(rsc::WARNING_OUTLINE),

                                                #[watch]
                                                set_visible: !model.remote_desktop_ok
                                            },

                                            gtk::Button {
                                                set_label: &tr!("Configure"),
                                                set_valign: gtk::Align::Center,

                                                #[watch]
                                                set_sensitive: !model.remote_desktop_ok,

                                                connect_clicked => AppMsg::CreateRemoteDesktop,
                                            }
                                        }
                                    },

                                    adw::ActionRow {
                                        set_title: &tr!("Background activity"),
                                        set_subtitle: &tr!("Permission to remain opened in the background."),

                                        add_suffix = &gtk::Box{
                                            set_spacing: 12,

                                            gtk::Image {
                                                set_icon_name: Some(rsc::WARNING_OUTLINE),

                                                #[watch]
                                                set_visible: !model.background_ok
                                            },

                                            gtk::Button {
                                                set_label: &tr!("Configure"),
                                                set_valign: gtk::Align::Center,

                                                #[watch]
                                                set_sensitive: !model.background_ok,

                                                connect_clicked => AppMsg::CreateBackground,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<App>,
    ) -> AsyncComponentParts<Self> {
        let model = Self {
            remote_desktop_ok: true,
            shortcut_ok: true,
            background_ok: true,
            config: init.config,
            last_save: SystemTime::now(),
            identifier: WindowIdentifier::from_native(&root).await,
            rd_opt: None,
            sc_opt: None,
        };
        let widgets = view_output!();

        root.set_visible(!init.silent);

        let input_sender = sender.input_sender();
        logged_send!(input_sender, AppMsg::CreateShortcuts);
        logged_send!(input_sender, AppMsg::CreateRemoteDesktop);
        logged_send!(input_sender, AppMsg::CreateBackground);

        AsyncComponentParts { model, widgets }
    }

    fn init_loading_widgets(root: Self::Root) -> Option<LoadingWidgets> {
        view! {
            #[local]
            root {
                #[name(loading)]
                adw::StatusPage {
                    set_title: &tr!("Loading..."),

                    #[wrap(Some)]
                    set_paintable = &adw::SpinnerPaintable::new(Some(&loading)),
                }
            }
        }
        Some(LoadingWidgets::new(root, loading))
    }

    async fn update(
        &mut self,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            AppMsg::CreateShortcuts => match self.create_shortcut(&sender).await {
                Ok(_) => {
                    info!("Shortcuts registered");
                    self.shortcut_ok = true;
                }
                Err(error) => {
                    error!("Error registering shortcuts: {}", error);
                    self.shortcut_ok = false;
                }
            },

            AppMsg::CreateRemoteDesktop => match self.create_remote_desktop(&sender).await {
                Ok(_) => {
                    info!("Remote desktop created");
                    self.remote_desktop_ok = true;
                }
                Err(error) => {
                    error!("Error creating remote desktop: {}", error);
                    self.remote_desktop_ok = false;
                }
            },

            AppMsg::CreateBackground => match self.create_background().await {
                Ok(_) => {
                    info!("Background activity created");
                    self.background_ok = true;
                }
                Err(error) => {
                    error!("Error creating background activity: {}", error);
                    self.background_ok = false;
                }
            },

            AppMsg::TypingPeriodChanged(duration) => {
                self.config.typing_period = duration;
                logged_send!(sender.input_sender(), AppMsg::SaveConfig);
            }

            AppMsg::TypingDelayChanged(duration) => {
                self.config.typing_delay = duration;
                logged_send!(sender.input_sender(), AppMsg::SaveConfig);
            }

            AppMsg::SaveConfig => {
                let elapsed = self.last_save.elapsed().unwrap_or_default();

                if elapsed > SAVE_DELAY {
                    self.config.save();
                    self.last_save = SystemTime::now();
                } else {
                    sleep(SAVE_DELAY - elapsed).await;
                    logged_send!(sender.input_sender(), AppMsg::SaveConfig);
                }
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            CmdMsg::Paste => match self.get_clipboard().await {
                Ok(text) => {
                    if let Err(error) = self.type_text(&text).await {
                        error!("Error during type text {}", error);
                        self.remote_desktop_ok = false;
                    }
                }

                Err(error) => {
                    error!("Error getting clipboard content: {}", error);
                }
            },

            CmdMsg::InvalidateShortcuts => {
                self.shortcut_ok = false;
                info!("Shortcuts session closed");
            }

            CmdMsg::InvalidateRemoteDesktop => {
                self.remote_desktop_ok = false;
                info!("Remote desktop session closed");
            }
        }
    }
}

impl App {
    async fn create_shortcut(&mut self, sender: &AsyncComponentSender<Self>) -> Result<()> {
        let global_shortcuts = GlobalShortcuts::new().await?;
        let session = global_shortcuts.create_session().await?;

        handle_invalidation!(sender, session, CmdMsg::InvalidateShortcuts);

        let request = global_shortcuts
            .bind_shortcuts(
                &session,
                &[NewShortcut::new(SHORTCUT_ID_PASTE, &tr!("Type clipboard"))
                    .preferred_trigger("LOGO+B")],
                self.identifier.as_ref(),
            )
            .await?;

        let shortcuts = global_shortcuts
            .list_shortcuts(&session)
            .await?
            .response()?;

        if shortcuts.shortcuts().is_empty() {
            debug!("No shortcuts registered yet");
            request.response()?;
        }
        debug!("Shortcuts registered");

        // If Ok, plug the stream into the command messages sender
        debug!("Acquiring shortcut event stream");
        let stream = global_shortcuts.receive_deactivated().await?;

        sender.command(move |sender, shutdown| {
            shutdown
                .register(async move {
                    stream
                        .for_each(async |event| match event.shortcut_id() {
                            SHORTCUT_ID_PASTE => logged_send!(sender, CmdMsg::Paste),

                            id => {
                                warn!("Unknown shortcut ID: {id}")
                            }
                        })
                        .await
                })
                .drop_on_shutdown()
        });

        self.sc_opt = Some((global_shortcuts, session));

        Ok(())
    }

    async fn create_remote_desktop(&mut self, sender: &AsyncComponentSender<Self>) -> Result<()> {
        let remote_desktop = RemoteDesktop::new().await?;
        let session = remote_desktop.create_session().await?;

        handle_invalidation!(sender, session, CmdMsg::InvalidateRemoteDesktop);

        remote_desktop
            .select_devices(
                &session,
                BitFlags::from(DeviceType::Keyboard),
                self.config.rd_token.as_ref().map(|x| x.as_str()),
                PersistMode::ExplicitlyRevoked,
            )
            .await?;

        let response = remote_desktop
            .start(&session, self.identifier.as_ref())
            .await?
            .response()?;

        if !response.devices().contains(DeviceType::Keyboard) {
            anyhow::bail!("Refused keyboard access");
        }

        if let Some(token) = response.restore_token() {
            info!("Remote desktop restore token: {token}");
            self.config.rd_token = Some(token.to_owned());
        };

        self.rd_opt = Some((remote_desktop, session));

        Ok(())
    }

    async fn create_background(&mut self) -> Result<()> {
        let command = [
            std::env::current_exe()?.to_string_lossy().to_string(),
            "--silent".to_owned(),
        ];

        info!("Registering background activity command: {command:?}");

        Background::request()
            .reason(Some(tr!("Type the clipboard when prompted.").as_str()))
            .auto_start(true)
            .command(&command)
            .send()
            .await?
            .response()?;

        BackgroundProxy::new()
            .await?
            .set_status(&tr!("Waiting for shortcut to type clipboard."))
            .await?;

        Ok(())
    }

    async fn get_clipboard(&self) -> Result<String> {
        if let Some(gdk) = gdk::Display::default() {
            let result = gdk.clipboard().read_text_future().await;

            match result {
                Ok(Some(text)) => Ok(text.to_string()),
                Ok(None) => anyhow::bail!("Clipboard is empty"),
                Err(error) => Err(error.into()),
            }
        } else {
            anyhow::bail!("GDK display is not available")
        }
    }

    async fn type_text(&self, string: &str) -> Result<()> {
        if let Some((remote_desktop, session)) = self.rd_opt.as_ref() {
            sleep(self.config.typing_delay).await;

            for char in string.chars() {
                let keysym = Keysym::from_char(char);

                // Wait before typing
                sleep(self.config.typing_period).await;

                remote_desktop
                    .notify_keyboard_keysym(session, keysym.raw() as i32, KeyState::Pressed)
                    .await?;
                remote_desktop
                    .notify_keyboard_keysym(session, keysym.raw() as i32, KeyState::Released)
                    .await?;
            }
        } else {
            error!("No remote desktop found");
        }

        Ok(())
    }
}
