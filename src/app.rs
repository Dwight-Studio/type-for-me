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
use crate::{APP_ID, handle_invalidation, logged_send, rsc};
use anyhow::Result;
use ashpd::desktop::background::{Background, BackgroundProxy, SetStatusOptions};
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::{PersistMode, Session};
use ashpd::enumflags2::BitFlags;
use ashpd::{AppID, WindowIdentifier};
use futures::StreamExt;
use log::{debug, error, info, warn};
use relm4::actions::{AccelsPlus, ActionName, RelmAction, RelmActionGroup};
use relm4::adw::prelude::*;
use relm4::adw::{gdk, gio};
use relm4::gtk::Adjustment;
use relm4::loading_widgets::LoadingWidgets;
use relm4::prelude::*;
use relm4::view;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tr::tr;
use xkbcommon::xkb::Keysym;

const SHORTCUT_ID_PASTE: &str = "paste";
const SAVE_DELAY: Duration = Duration::from_secs(1);
const MAX_DELAY_MS: f64 = 5000f64;

relm4::new_action_group!(WindowActionGroup, "win");
relm4::new_stateful_action!(AutostartAction, WindowActionGroup, "autostart", (), bool);
relm4::new_stateless_action!(AboutAction, WindowActionGroup, "about");
relm4::new_stateless_action!(QuitAction, WindowActionGroup, "quit");

pub struct App {
    // UI
    remote_desktop_ok: bool,
    shortcut_ok: bool,
    background_ok: bool,
    shortcut_trigger: Option<String>,
    typing: bool,
    toast_overlay: Option<adw::ToastOverlay>,
    test_entry: Option<adw::EntryRow>,
    autostart_action: gio::SimpleAction,

    // Other
    config: Config,
    save_scheduled: bool,
    identifier: Option<WindowIdentifier>,
    sc_opt: Option<(GlobalShortcuts, Arc<Session<GlobalShortcuts>>)>,
    rd_opt: Option<Arc<RemoteDesktopSession>>,
}

type RemoteDesktopSession = (RemoteDesktop, Arc<Session<RemoteDesktop>>);

pub struct Init {
    pub(crate) silent: bool,
    pub(crate) config: Config,
}

#[derive(Debug)]
pub enum AppMsg {
    TypingPeriodChanged(Duration),
    TypingDelayChanged(Duration),
    NewlinesAsSpacesChanged(bool),
    CreateShortcuts,
    ConfigureShortcuts,
    CreateRemoteDesktop,
    CreateBackground,
    FixPermissions,
    SetAutostart(bool),
    TestTyping,
    ShowAbout,
    Quit,
    SaveConfig,
}

#[derive(Debug)]
pub enum CmdMsg {
    Paste,
    TypingDone(Result<()>),
    ShortcutTriggerChanged(Option<String>),
    SaveConfig,
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
            .default_width(520)
            .default_height(720)
            .width_request(360)
            .hide_on_close(true)
            .build()
        {
            #[name = "toast_overlay"]
            adw::ToastOverlay {
                #[wrap(Some)]
                set_child = &adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {
                        pack_start = &adw::Spinner {
                            set_tooltip_text: Some(&tr!("Typing…")),

                            #[watch]
                            set_visible: model.typing,
                        },

                        pack_end = &gtk::MenuButton {
                            set_icon_name: rsc::MENU_LARGE,
                            set_menu_model: Some(&main_menu),
                            set_primary: true,
                            set_tooltip_text: Some(&tr!("Main Menu")),
                        },
                    },

                    add_top_bar = &adw::Banner {
                        set_button_label: Some(&tr!("Allow")),
                        connect_button_clicked => AppMsg::FixPermissions,

                        #[watch]
                        set_title: &{
                            if !model.remote_desktop_ok {
                                tr!("Keyboard access is required to type the clipboard")
                            } else if !model.shortcut_ok {
                                tr!("The shortcut is not registered")
                            } else if !model.background_ok {
                                tr!("Background activity is required for the shortcut to work")
                            } else {
                                "".to_string()
                            }
                        },

                        #[watch]
                        set_revealed: !(model.remote_desktop_ok && model.shortcut_ok && model.background_ok),
                    },

                    #[wrap(Some)]
                    set_content = &adw::PreferencesPage {
                        add = &adw::PreferencesGroup {
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 12,
                                set_halign: gtk::Align::Center,

                                gtk::Image {
                                    set_icon_name: Some(APP_ID),
                                    set_pixel_size: 128,
                                    add_css_class: "icon-dropshadow",
                                },

                                gtk::Label {
                                    set_label: "Type for Me",
                                    add_css_class: "title-1",
                                },

                                gtk::Label {
                                    set_label: &tr!("Types your clipboard instead of pasting it, even where pasting is not possible, such as virtual machines."),
                                    set_wrap: true,
                                    set_justify: gtk::Justification::Center,
                                    add_css_class: "dimmed",
                                },
                            },
                        },

                        add = &adw::PreferencesGroup {
                            set_title: &tr!("Shortcut"),

                            adw::ActionRow {
                                set_title: &tr!("Type the clipboard"),
                                // The trigger description contains "<Super>"-like text, not markup
                                set_use_markup: false,

                                #[watch]
                                set_subtitle: &model.shortcut_trigger.clone().unwrap_or_else(|| {
                                    if model.shortcut_ok {
                                        tr!("No shortcut assigned")
                                    } else {
                                        tr!("The shortcut is not registered")
                                    }
                                }),

                                add_suffix = &gtk::Image {
                                    set_icon_name: Some(rsc::WARNING_OUTLINE),
                                    set_tooltip_text: Some(&tr!("Not registered")),
                                    add_css_class: "warning",

                                    #[watch]
                                    set_visible: !model.shortcut_ok,
                                },

                                add_suffix = &gtk::Button {
                                    set_valign: gtk::Align::Center,

                                    #[watch]
                                    set_label: &if model.shortcut_ok { tr!("Change…") } else { tr!("Allow") },

                                    #[watch]
                                    set_css_classes: if model.shortcut_ok { &["flat"] } else { &["suggested-action"] },

                                    connect_clicked => AppMsg::ConfigureShortcuts,
                                },
                            },
                        },

                        add = &adw::PreferencesGroup {
                            set_title: &tr!("Try It"),
                            set_description: Some(&tr!("Copy some text, then press the button: it will be typed in the field below.")),

                            #[name = "test_entry"]
                            adw::EntryRow {
                                set_title: &tr!("Test area"),

                                add_suffix = &gtk::Button {
                                    set_icon_name: rsc::KEYBOARD,
                                    set_tooltip_text: Some(&tr!("Type the clipboard here")),
                                    set_valign: gtk::Align::Center,
                                    add_css_class: "flat",

                                    #[watch]
                                    set_sensitive: model.remote_desktop_ok && !model.typing,

                                    connect_clicked => AppMsg::TestTyping,
                                },
                            },
                        },

                        add = &adw::PreferencesGroup {
                            set_title: &tr!("Typing"),

                            adw::SpinRow {
                                set_title: &tr!("Delay before typing"),
                                set_subtitle: &tr!("Time to release the shortcut and focus the target window, in milliseconds."),
                                set_adjustment: Some(&Adjustment::new(model.config.typing_delay.as_secs_f64() * 1000f64, 0f64, MAX_DELAY_MS, 5f64, 0f64, 0f64)),
                                set_snap_to_ticks: true,

                                connect_value_notify[sender] => move |elem| {
                                    logged_send!(
                                        sender.input_sender(),
                                        AppMsg::TypingDelayChanged(Duration::from_millis(elem.value() as u64))
                                    );
                                },
                            },

                            adw::SpinRow {
                                set_title: &tr!("Delay between keys"),
                                set_subtitle: &tr!("Increase it if some characters are lost, in milliseconds."),
                                set_adjustment: Some(&Adjustment::new(model.config.typing_period.as_secs_f64() * 1000f64, 0f64, MAX_DELAY_MS, 5f64, 0f64, 0f64)),
                                set_snap_to_ticks: true,

                                connect_value_notify[sender] => move |elem| {
                                    logged_send!(
                                        sender.input_sender(),
                                        AppMsg::TypingPeriodChanged(Duration::from_millis(elem.value() as u64))
                                    );
                                },
                            },

                            adw::SwitchRow {
                                set_title: &tr!("Replace line breaks with spaces"),
                                set_subtitle: &tr!("Type a space instead of pressing Enter, so that pasting several lines does not validate a form or run a command."),
                                set_active: model.config.newlines_as_spaces,

                                connect_active_notify[sender] => move |elem| {
                                    logged_send!(
                                        sender.input_sender(),
                                        AppMsg::NewlinesAsSpacesChanged(elem.is_active())
                                    );
                                },
                            },
                        },

                        add = &adw::PreferencesGroup {
                            set_title: &tr!("Permissions"),

                            adw::ActionRow {
                                set_title: &tr!("Keyboard access"),
                                set_subtitle: &tr!("Send key strokes through the remote desktop portal."),

                                add_suffix = &gtk::Image {
                                    set_icon_name: Some("object-select-symbolic"),
                                    set_tooltip_text: Some(&tr!("Allowed")),
                                    add_css_class: "success",

                                    #[watch]
                                    set_visible: model.remote_desktop_ok,
                                },

                                add_suffix = &gtk::Button {
                                    set_label: &tr!("Allow"),
                                    set_valign: gtk::Align::Center,
                                    add_css_class: "suggested-action",

                                    #[watch]
                                    set_visible: !model.remote_desktop_ok,

                                    connect_clicked => AppMsg::CreateRemoteDesktop,
                                },
                            },

                            adw::ActionRow {
                                set_title: &tr!("Background activity"),
                                set_subtitle: &tr!("Keep running when the window is closed, so the shortcut always works."),

                                add_suffix = &gtk::Image {
                                    set_icon_name: Some("object-select-symbolic"),
                                    set_tooltip_text: Some(&tr!("Allowed")),
                                    add_css_class: "success",

                                    #[watch]
                                    set_visible: model.background_ok,
                                },

                                add_suffix = &gtk::Button {
                                    set_label: &tr!("Allow"),
                                    set_valign: gtk::Align::Center,
                                    add_css_class: "suggested-action",

                                    #[watch]
                                    set_visible: !model.background_ok,

                                    connect_clicked => AppMsg::CreateBackground,
                                },
                            },
                        },
                    },
                },
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<App>,
    ) -> AsyncComponentParts<Self> {
        // Outside a sandbox, the portals guess the app ID from the parent process (e.g. the
        // terminal or the IDE), which breaks the shortcuts configuration: register it explicitly.
        // This must happen before any other portal call.
        match AppID::from_str(APP_ID) {
            Ok(app_id) => {
                if let Err(error) = ashpd::register_host_app(app_id).await {
                    warn!("Error registering the app ID to the portals: {}", error);
                }
            }
            Err(error) => error!("Invalid app ID {APP_ID}: {}", error),
        }

        // Window actions, used by the main menu
        let autostart_action: RelmAction<AutostartAction> = {
            let sender = sender.clone();
            RelmAction::new_stateful(&init.config.autostart, move |_, state: &mut bool| {
                *state = !*state;
                sender.input(AppMsg::SetAutostart(*state));
            })
        };
        let about_action: RelmAction<AboutAction> = {
            let sender = sender.clone();
            RelmAction::new_stateless(move |_| sender.input(AppMsg::ShowAbout))
        };
        let quit_action: RelmAction<QuitAction> = {
            let sender = sender.clone();
            RelmAction::new_stateless(move |_| sender.input(AppMsg::Quit))
        };

        let autostart_simple_action: gio::SimpleAction = autostart_action.clone().into();

        let mut actions = RelmActionGroup::<WindowActionGroup>::new();
        actions.add_action(autostart_action);
        actions.add_action(about_action);
        actions.add_action(quit_action);
        actions.register_for_widget(&root);

        relm4::main_application().set_accelerators_for_action::<QuitAction>(&["<Control>q"]);

        let main_menu = gio::Menu::new();
        let settings_section = gio::Menu::new();
        settings_section.append(
            Some(&tr!("Launch at Login")),
            Some(&AutostartAction::action_name()),
        );
        main_menu.append_section(None, &settings_section);
        let app_section = gio::Menu::new();
        app_section.append(
            Some(&tr!("About Type for Me")),
            Some(&AboutAction::action_name()),
        );
        app_section.append(Some(&tr!("Quit")), Some(&QuitAction::action_name()));
        main_menu.append_section(None, &app_section);

        let mut model = Self {
            remote_desktop_ok: true,
            shortcut_ok: true,
            background_ok: true,
            shortcut_trigger: None,
            typing: false,
            toast_overlay: None,
            test_entry: None,
            autostart_action: autostart_simple_action,
            config: init.config,
            save_scheduled: false,
            identifier: WindowIdentifier::from_native(&root).await,
            rd_opt: None,
            sc_opt: None,
        };
        let widgets = view_output!();

        model.toast_overlay = Some(widgets.toast_overlay.clone());
        model.test_entry = Some(widgets.test_entry.clone());

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
                    set_title: &tr!("Loading…"),

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
        root: &Self::Root,
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
                    self.toast(tr!("The shortcut could not be registered"));
                }
            },

            AppMsg::ConfigureShortcuts => match &self.sc_opt {
                Some((global_shortcuts, session)) => {
                    if let Err(error) = global_shortcuts
                        .configure_shortcuts(session, self.identifier.as_ref(), Default::default())
                        .await
                    {
                        warn!("Error opening the shortcuts configuration: {}", error);
                        self.toast(tr!(
                            "Change the shortcut in the keyboard settings of your desktop"
                        ));
                    }
                }
                None => logged_send!(sender.input_sender(), AppMsg::CreateShortcuts),
            },

            AppMsg::CreateRemoteDesktop => match self.create_remote_desktop(&sender).await {
                Ok(_) => {
                    info!("Remote desktop created");
                    self.remote_desktop_ok = true;
                }
                Err(error) => {
                    error!("Error creating remote desktop: {}", error);
                    self.remote_desktop_ok = false;
                    self.toast(tr!("Keyboard access was not granted"));
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
                    self.toast(tr!("Background activity was not granted"));
                }
            },

            AppMsg::FixPermissions => {
                let message = if !self.remote_desktop_ok {
                    AppMsg::CreateRemoteDesktop
                } else if !self.shortcut_ok {
                    AppMsg::CreateShortcuts
                } else {
                    AppMsg::CreateBackground
                };
                logged_send!(sender.input_sender(), message);
            }

            AppMsg::SetAutostart(enabled) => {
                self.config.autostart = enabled;
                logged_send!(sender.input_sender(), AppMsg::SaveConfig);
                logged_send!(sender.input_sender(), AppMsg::CreateBackground);
            }

            AppMsg::TestTyping => {
                if let Some(entry) = &self.test_entry {
                    entry.set_text("");
                    entry.grab_focus();
                }
                self.paste(&sender).await;
            }

            AppMsg::ShowAbout => {
                let dialog =
                    adw::AboutDialog::from_appdata(rsc::METAINFO, Some(env!("CARGO_PKG_VERSION")));
                dialog.set_version(env!("CARGO_PKG_VERSION"));
                dialog.set_developers(&env!("CARGO_PKG_AUTHORS").split(':').collect::<Vec<_>>());
                dialog.set_copyright("© 2025 Dwight Studio");
                dialog.present(Some(root));
            }

            AppMsg::Quit => relm4::main_application().quit(),

            AppMsg::TypingPeriodChanged(duration) => {
                self.config.typing_period = duration;
                logged_send!(sender.input_sender(), AppMsg::SaveConfig);
            }

            AppMsg::TypingDelayChanged(duration) => {
                self.config.typing_delay = duration;
                logged_send!(sender.input_sender(), AppMsg::SaveConfig);
            }

            AppMsg::NewlinesAsSpacesChanged(enabled) => {
                self.config.newlines_as_spaces = enabled;
                logged_send!(sender.input_sender(), AppMsg::SaveConfig);
            }

            // Debounce: the save happens once SAVE_DELAY has elapsed, without blocking the UI
            AppMsg::SaveConfig => {
                if !self.save_scheduled {
                    self.save_scheduled = true;
                    sender.oneshot_command(async {
                        sleep(SAVE_DELAY).await;
                        CmdMsg::SaveConfig
                    });
                }
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            CmdMsg::Paste => self.paste(&sender).await,

            CmdMsg::TypingDone(result) => {
                self.typing = false;

                if let Err(error) = result {
                    error!("Error during type text {}", error);
                    self.remote_desktop_ok = false;
                    self.toast(tr!("Typing failed: keyboard access was lost"));

                    // The session is unusable: close it instead of keeping a dead one
                    self.close_remote_desktop().await;
                }
            }

            CmdMsg::ShortcutTriggerChanged(trigger) => {
                debug!("Shortcut trigger changed: {trigger:?}");
                self.shortcut_trigger = trigger;
            }

            CmdMsg::SaveConfig => {
                self.save_scheduled = false;
                self.config.save();
            }

            CmdMsg::InvalidateShortcuts => {
                self.shortcut_ok = false;
                self.shortcut_trigger = None;
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
    fn toast(&self, message: String) {
        if let Some(overlay) = &self.toast_overlay {
            overlay.add_toast(adw::Toast::new(&message));
        }
    }

    async fn paste(&mut self, sender: &AsyncComponentSender<Self>) {
        if self.typing {
            warn!("Already typing, ignoring request");
            return;
        }

        match self.get_clipboard().await {
            Ok(text) => {
                // Type in a separate task so the component stays responsive
                self.typing = true;
                let rd = self.rd_opt.clone();
                let delay = self.config.typing_delay;
                let period = self.config.typing_period;
                let newlines_as_spaces = self.config.newlines_as_spaces;

                sender.command(move |sender, shutdown| {
                    shutdown
                        .register(async move {
                            let result =
                                type_text(rd.as_deref(), &text, delay, period, newlines_as_spaces)
                                    .await;
                            logged_send!(sender, CmdMsg::TypingDone(result));
                        })
                        .drop_on_shutdown()
                });
            }

            Err(error) => {
                error!("Error getting clipboard content: {}", error);
                self.toast(tr!("The clipboard does not contain text"));
            }
        }
    }

    async fn create_shortcut(&mut self, sender: &AsyncComponentSender<Self>) -> Result<()> {
        if let Some((_, session)) = self.sc_opt.take()
            && let Err(error) = session.close().await
        {
            debug!("Error closing previous shortcuts session: {}", error);
        }

        let global_shortcuts = GlobalShortcuts::new().await?;
        let session = Arc::new(global_shortcuts.create_session(Default::default()).await?);

        handle_invalidation!(sender, session, CmdMsg::InvalidateShortcuts);

        let request = global_shortcuts
            .bind_shortcuts(
                &session,
                &[NewShortcut::new(SHORTCUT_ID_PASTE, tr!("Type clipboard"))
                    .preferred_trigger("LOGO+B")],
                self.identifier.as_ref(),
                Default::default(),
            )
            .await?;

        let listed = global_shortcuts
            .list_shortcuts(&session, Default::default())
            .await?
            .response()?;

        self.shortcut_trigger = if listed.shortcuts().is_empty() {
            debug!("No shortcuts registered yet");
            paste_trigger(request.response()?.shortcuts())
        } else {
            paste_trigger(listed.shortcuts())
        };
        debug!("Shortcuts registered, trigger: {:?}", self.shortcut_trigger);

        // If Ok, plug the streams into the command messages sender
        debug!("Acquiring shortcut event streams");
        let stream = global_shortcuts.receive_deactivated().await?;
        let changes = global_shortcuts.receive_shortcuts_changed().await?;

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

        sender.command(move |sender, shutdown| {
            shutdown
                .register(async move {
                    changes
                        .for_each(async |event| {
                            logged_send!(
                                sender,
                                CmdMsg::ShortcutTriggerChanged(paste_trigger(event.shortcuts()))
                            )
                        })
                        .await
                })
                .drop_on_shutdown()
        });

        self.sc_opt = Some((global_shortcuts, session));

        Ok(())
    }

    async fn close_remote_desktop(&mut self) {
        if let Some(rd) = self.rd_opt.take()
            && let Err(error) = rd.1.close().await
        {
            debug!("Error closing remote desktop session: {}", error);
        }
    }

    async fn create_remote_desktop(&mut self, sender: &AsyncComponentSender<Self>) -> Result<()> {
        self.close_remote_desktop().await;

        let remote_desktop = RemoteDesktop::new().await?;
        let session = Arc::new(remote_desktop.create_session(Default::default()).await?);

        handle_invalidation!(sender, session, CmdMsg::InvalidateRemoteDesktop);

        remote_desktop
            .select_devices(
                &session,
                SelectDevicesOptions::default()
                    .set_devices(BitFlags::from(DeviceType::Keyboard))
                    .set_restore_token(self.config.rd_token.as_deref())
                    .set_persist_mode(PersistMode::ExplicitlyRevoked),
            )
            .await?;

        let response = remote_desktop
            .start(&session, self.identifier.as_ref(), Default::default())
            .await?
            .response()?;

        if !response.devices().contains(DeviceType::Keyboard) {
            anyhow::bail!("Refused keyboard access");
        }

        if let Some(token) = response.restore_token() {
            debug!("Remote desktop restore token received");
            self.config.rd_token = Some(token.to_owned());
            self.config.save();
        };

        self.rd_opt = Some(Arc::new((remote_desktop, session)));

        Ok(())
    }

    async fn create_background(&mut self) -> Result<()> {
        let command = [
            std::env::current_exe()?.to_string_lossy().to_string(),
            "--silent".to_owned(),
        ];

        info!(
            "Registering background activity command: {command:?}, autostart: {}",
            self.config.autostart
        );

        let response = Background::request()
            .reason(Some(tr!("Type the clipboard when prompted.").as_str()))
            .auto_start(self.config.autostart)
            .command(&command)
            .send()
            .await?
            .response()?;

        // The user may refuse the autostart: reflect the actual state in the menu
        if response.auto_start() != self.config.autostart {
            warn!("Autostart was not granted as requested");
            self.config.autostart = response.auto_start();
            self.config.save();
            self.autostart_action
                .set_state(&self.config.autostart.to_variant());
            self.toast(tr!("Launch at login was not allowed"));
        }

        if !response.run_in_background() {
            anyhow::bail!("Refused background activity");
        }

        // Only sandboxed applications can set a status: failing here is not an error
        let status = SetStatusOptions::default()
            .set_message(&tr!("Waiting for shortcut to type clipboard."));
        if let Err(error) = BackgroundProxy::new().await?.set_status(status).await {
            debug!("Could not set background status: {}", error);
        }

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
}

fn paste_trigger(shortcuts: &[Shortcut]) -> Option<String> {
    shortcuts
        .iter()
        .find(|shortcut| shortcut.id() == SHORTCUT_ID_PASTE)
        .map(|shortcut| shortcut.trigger_description().to_owned())
        .filter(|trigger| !trigger.is_empty())
}

async fn type_text(
    rd: Option<&RemoteDesktopSession>,
    string: &str,
    delay: Duration,
    period: Duration,
    newlines_as_spaces: bool,
) -> Result<()> {
    if let Some((remote_desktop, session)) = rd {
        sleep(delay).await;

        for char in string.chars() {
            let Some(keysym) = keysym_for(char, newlines_as_spaces) else {
                continue;
            };

            // Wait before typing
            sleep(period).await;

            remote_desktop
                .notify_keyboard_keysym(
                    session,
                    keysym.raw() as i32,
                    KeyState::Pressed,
                    Default::default(),
                )
                .await?;
            remote_desktop
                .notify_keyboard_keysym(
                    session,
                    keysym.raw() as i32,
                    KeyState::Released,
                    Default::default(),
                )
                .await?;
        }
    } else {
        anyhow::bail!("No remote desktop session");
    }

    Ok(())
}

/// Returns the key to press for a character, or `None` if it must be skipped
fn keysym_for(char: char, newlines_as_spaces: bool) -> Option<Keysym> {
    let keysym = match char {
        // Windows line endings: the '\n' that follows produces the line break
        '\r' => return None,
        '\n' if newlines_as_spaces => Keysym::space,
        '\n' => Keysym::Return,
        '\t' => Keysym::Tab,
        _ => Keysym::from_char(char),
    };

    if keysym.raw() == 0 {
        warn!("No keysym for character {char:?}, skipping");
        return None;
    }

    Some(keysym)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keysyms(text: &str, newlines_as_spaces: bool) -> Vec<Keysym> {
        text.chars()
            .filter_map(|char| keysym_for(char, newlines_as_spaces))
            .collect()
    }

    #[test]
    fn newlines_are_typed_as_return_by_default() {
        assert_eq!(
            keysyms("a\r\nb\n", false),
            [Keysym::a, Keysym::Return, Keysym::b, Keysym::Return]
        );
    }

    #[test]
    fn newlines_can_be_replaced_by_spaces() {
        assert_eq!(
            keysyms("a\r\nb\n", true),
            [Keysym::a, Keysym::space, Keysym::b, Keysym::space]
        );
    }

    #[test]
    fn tabs_are_kept() {
        assert_eq!(keysyms("\t", true), [Keysym::Tab]);
    }
}
