use iced::widget::{button, column, container, row, scrollable, text, stack, Column};
use iced::{Alignment, Element, Length, Subscription, Task};
use iced::{Color, Theme};
use tracing_subscriber;

mod ui;

use geist_vpn::profile::{ProfileManager, VpnProfile};
use std::sync::Arc;
use geist_vpn::{init, cleanup, client::SoftEtherClient};

#[derive(Debug, Clone)]
pub enum Message {
    // Connection messages
    Connect(String),
    Disconnect,
    ConnectionResult(Result<(), String>),

    // Profile management messages
    LoadProfiles,
    ProfilesLoaded(Result<Vec<VpnProfile>, String>),
    CreateProfile,
    EditProfile(String),
    DeleteProfile(String),
    ToggleFavorite(String),
    SaveProfile(VpnProfile),
    ProfileSaved(Result<(), String>),

    // Profile modal messages
    ProfileModalUpdateName(String),
    ProfileModalUpdateHost(String),
    ProfileModalUpdatePort(String),
    ProfileModalUpdateProtocol(geist_vpn::profile::VpnProtocol),
    ProfileModalUpdateAccount(String),
    ProfileModalUpdateTimeout(String),
    ProfileModalSave,

    // UI state messages
    ProfileSelected(String),
    ViewChanged(ViewMode),
    ModalClosed,

    // Status updates
    StatusUpdated(ConnectionStatus),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    All,
    Favorites,
    Recent,
}

#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub profile_name: Option<String>,
    pub status_message: String,
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self {
            connected: false,
            profile_name: None,
            status_message: "Disconnected".to_string(),
        }
    }
}

pub struct GeistApp {
    // Connection state
    connection_status: ConnectionStatus,
    selected_profile: Option<String>,

    // Profile management
    profiles: Vec<VpnProfile>,
    current_view: ViewMode,

    // UI state
    profile_manager: Option<Arc<ProfileManager>>,
    vpn_client: Option<Box<SoftEtherClient>>,
    loading_profiles: bool,
    connecting: bool,

    // Modal state
    show_profile_modal: bool,
    profile_modal_state: ui::modal::ProfileModalState,
}

impl Default for GeistApp {
    fn default() -> Self {
        Self {
            connection_status: ConnectionStatus::default(),
            selected_profile: None,
            profiles: Vec::new(),
            current_view: ViewMode::All,
            profile_manager: None,
            vpn_client: None,
            loading_profiles: false,
            connecting: false,
            show_profile_modal: false,
            profile_modal_state: ui::modal::ProfileModalState::default(),
        }
    }
}

impl GeistApp {
    fn new() -> (Self, Task<Message>) {
        let mut app = Self::default();

        // Initialize SoftEther in the main thread - DISABLED FOR TESTING
        // if let Err(e) = init() {
        //     tracing::error!("Failed to initialize SoftEther: {}", e);
        // }

        // Initialize profile manager
        match ProfileManager::new() {
            Ok(manager) => {
                app.profile_manager = Some(Arc::new(manager));
            }
            Err(e) => {
                tracing::error!("Failed to initialize profile manager: {}", e);
            }
        }

        (app, iced::Task::perform(async { Message::LoadProfiles }, |_| Message::LoadProfiles))
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Connect(profile_id) => {
                self.connecting = true;

                // Find the profile to connect to
                let profile = match self.profiles.iter().find(|p| p.id == profile_id) {
                    Some(profile) => profile.clone(),
                    None => {
                        self.connecting = false;
                        return iced::Task::perform(async { Message::ConnectionResult(Err("Profile not found".to_string())) }, |msg| msg);
                    }
                };

                let result = if let Some(client) = &mut self.vpn_client {
                    // Perform the connection operation synchronously in the main thread
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            client.connect(&profile).await
                        })
                    }).map_err(|e| format!("Connection failed: {}", e))
                } else {
                    // Lazy initialization of VPN client
                    match SoftEtherClient::new() {
                        Ok(new_client) => {
                            self.vpn_client = Some(Box::new(new_client));
                            if let Some(client) = &mut self.vpn_client {
                                tokio::task::block_in_place(|| {
                                    tokio::runtime::Handle::current().block_on(async {
                                        client.connect(&profile).await
                                    })
                                }).map_err(|e| format!("Connection failed: {}", e))
                            } else {
                                Err("Failed to initialize VPN client".to_string())
                            }
                        }
                        Err(e) => Err(format!("Failed to create VPN client: {}", e)),
                    }
                };

                iced::Task::perform(async { result }, |result| Message::ConnectionResult(result))
            }

            Message::Disconnect => {
                self.connecting = true;

                let result = if let Some(client) = &mut self.vpn_client {
                    // Perform the disconnection operation synchronously in the main thread
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            client.disconnect().await
                        })
                    }).map_err(|e| format!("Disconnection failed: {}", e))
                } else {
                    Err("VPN client not initialized".to_string())
                };

                iced::Task::perform(async { result }, |result| Message::ConnectionResult(result))
            }

            Message::ConnectionResult(result) => {
                self.connecting = false;
                match result {
                    Ok(_) => {
                        // Get the real connection status from VPN client
                        let status = if let Some(client) = &self.vpn_client {
                            // Convert client status to UI status
                            match client.get_status() {
                                geist_vpn::client::ConnectionStatus::Disconnected => {
                                    ConnectionStatus {
                                        connected: false,
                                        profile_name: None,
                                        status_message: "Disconnected".to_string(),
                                    }
                                }
                                geist_vpn::client::ConnectionStatus::Connecting => {
                                    ConnectionStatus {
                                        connected: false,
                                        profile_name: client.active_profile().map(|p| p.name.clone()),
                                        status_message: "Connecting...".to_string(),
                                    }
                                }
                                geist_vpn::client::ConnectionStatus::Connected => {
                                    ConnectionStatus {
                                        connected: true,
                                        profile_name: client.active_profile().map(|p| p.name.clone()),
                                        status_message: "Connected".to_string(),
                                    }
                                }
                                geist_vpn::client::ConnectionStatus::Disconnecting => {
                                    ConnectionStatus {
                                        connected: true,
                                        profile_name: client.active_profile().map(|p| p.name.clone()),
                                        status_message: "Disconnecting...".to_string(),
                                    }
                                }
                                geist_vpn::client::ConnectionStatus::Error(msg) => {
                                    ConnectionStatus {
                                        connected: false,
                                        profile_name: None,
                                        status_message: format!("Error: {}", msg),
                                    }
                                }
                            }
                        } else {
                            ConnectionStatus {
                                connected: false,
                                profile_name: None,
                                status_message: "VPN client not initialized".to_string(),
                            }
                        };

                        return iced::Task::perform(async { Message::StatusUpdated(status) }, |msg| msg);
                    }
                    Err(error) => {
                        self.connection_status = ConnectionStatus {
                            connected: false,
                            profile_name: None,
                            status_message: format!("Operation failed: {}", error),
                        };
                    }
                }
                iced::Task::none()
            }

            Message::LoadProfiles => {
                self.loading_profiles = true;
                let manager = self.profile_manager.as_ref().map(Arc::clone);
                iced::Task::perform(
                    async move {
                        if let Some(manager) = manager {
                            manager.load_profiles().map_err(|e| e.to_string())
                        } else {
                            Err("Profile manager not initialized".to_string())
                        }
                    },
                    |result| Message::ProfilesLoaded(result),
                )
            }

            Message::ProfilesLoaded(result) => {
                self.loading_profiles = false;
                match result {
                    Ok(profiles) => {
                        self.profiles = profiles;
                    }
                    Err(error) => {
                        tracing::error!("Failed to load profiles: {}", error);
                    }
                }
                iced::Task::none()
            }

            Message::CreateProfile => {
                self.show_profile_modal = true;
                self.profile_modal_state = ui::modal::ProfileModalState::default();
                iced::Task::none()
            }

            Message::EditProfile(profile_id) => {
                if let Some(profile) = self.profiles.iter().find(|p| p.id == profile_id) {
                    self.show_profile_modal = true;
                    self.profile_modal_state = ui::modal::ProfileModalState::from_profile(profile);
                }
                iced::Task::none()
            }

            Message::DeleteProfile(profile_id) => {
                if let Some(manager) = &self.profile_manager {
                    if let Err(e) = manager.delete_profile(&profile_id) {
                        tracing::error!("Failed to delete profile: {}", e);
                    }
                }
                iced::Task::perform(async { Message::LoadProfiles }, |_| Message::LoadProfiles)
            }

            Message::ToggleFavorite(profile_id) => {
                if let Some(manager) = &self.profile_manager {
                    if let Some(profile) = self.profiles.iter_mut().find(|p| p.id == profile_id) {
                        profile.toggle_favorite();
                        if let Err(e) = manager.save_profile(profile) {
                            tracing::error!("Failed to save profile: {}", e);
                        }
                    }
                }
                iced::Task::none()
            }

            Message::SaveProfile(profile) => {
                let manager = self.profile_manager.as_ref().map(Arc::clone);
                iced::Task::perform(
                    async move {
                        if let Some(manager) = manager {
                            manager.save_profile(&profile).map_err(|e| e.to_string())
                        } else {
                            Err("Profile manager not initialized".to_string())
                        }
                    },
                    |result| Message::ProfileSaved(result),
                )
            }

            Message::ProfileSaved(result) => {
                match result {
                    Ok(_) => {
                        self.show_profile_modal = false;
// No editing_profile field anymore
                    }
                    Err(error) => {
                        tracing::error!("Failed to save profile: {}", error);
                    }
                }
                iced::Task::perform(async { Message::LoadProfiles }, |_| Message::LoadProfiles)
            }

            Message::ProfileSelected(profile_id) => {
                self.selected_profile = Some(profile_id);
                iced::Task::none()
            }

            Message::ViewChanged(view) => {
                self.current_view = view;
                iced::Task::none()
            }

            Message::ProfileModalUpdateName(name) => {
                self.profile_modal_state.name = name;
                iced::Task::none()
            }

            Message::ProfileModalUpdateHost(host) => {
                self.profile_modal_state.host = host;
                iced::Task::none()
            }

            Message::ProfileModalUpdatePort(port) => {
                self.profile_modal_state.port = port;
                iced::Task::none()
            }

            Message::ProfileModalUpdateProtocol(protocol) => {
                self.profile_modal_state.protocol = protocol;
                iced::Task::none()
            }

            Message::ProfileModalUpdateAccount(account) => {
                self.profile_modal_state.account_name = account;
                iced::Task::none()
            }

            Message::ProfileModalUpdateTimeout(timeout) => {
                self.profile_modal_state.timeout = timeout;
                iced::Task::none()
            }

            Message::ProfileModalSave => {
                if self.profile_modal_state.is_valid() {
                    match self.profile_modal_state.to_profile(
                        if self.profile_modal_state.editing {
                            // Find existing profile ID
                            self.profiles.iter()
                                .find(|p| p.name == self.profile_modal_state.name)
                                .map(|p| p.id.clone())
                        } else {
                            None
                        }
                    ) {
                        Ok(profile) => {
                            return iced::Task::perform(async move { Message::SaveProfile(profile) }, |msg| msg);
                        }
                        Err(error) => {
                            tracing::error!("Failed to create profile: {}", error);
                        }
                    }
                }
                iced::Task::none()
            }

            Message::ModalClosed => {
                self.show_profile_modal = false;
                self.profile_modal_state = ui::modal::ProfileModalState::default();
                iced::Task::none()
            }

            Message::StatusUpdated(status) => {
                self.connection_status = status;
                iced::Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let content = column![
            ui::header::view(&self.connection_status),
            ui::quick_connect::view(
                &self.profiles,
                self.selected_profile.as_deref(),
                self.connecting,
                &self.connection_status
            ),
            ui::quick_access::view(&self.current_view),
            ui::profiles::view(
                &self.profiles,
                &self.current_view,
                self.loading_profiles
            ),
        ]
        .spacing(20)
        .padding(20);

        let container = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &Theme| {
                iced::widget::container::Style {
                    background: Some(theme.palette().background.into()),
                    ..Default::default()
                }
            });

        // Add modal if needed
        if self.show_profile_modal {
            let modal = ui::modal::view(&self.profile_modal_state);

            // Overlay the modal on top of the main content
            iced::widget::stack![
                container,
                iced::widget::opaque(
                    iced::widget::container(modal)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(40)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                        .style(|theme: &iced::Theme| iced::widget::container::Style {
                            background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
                            ..Default::default()
                        })
                )
            ].into()
        } else {
            container.into()
        }
    }

    fn theme(&self) -> Theme {
        Theme::default()
    }

    fn subscription(&self) -> Subscription<Message> {
        // Poll VPN status every 2 seconds for real-time updates
        if self.vpn_client.is_some() {
            iced::time::every(std::time::Duration::from_secs(2))
                .map(|_| {
                    // In a real implementation, we'd get the actual status
                    // For now, just indicate polling is active
                    Message::StatusUpdated(ConnectionStatus {
                        connected: false,
                        profile_name: None,
                        status_message: "Status monitoring active".to_string(),
                    })
                })
        } else {
            Subscription::none()
        }
    }
}

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application("Geist VPN", GeistApp::update, GeistApp::view)
        .subscription(GeistApp::subscription)
        .theme(GeistApp::theme)
        .run_with(GeistApp::new)
}