use iced::widget::{button, column, container, row, scrollable, stack, text, Column};
use iced::{Alignment, Element, Length, Subscription, Task};
use iced::{Color, Theme};
use iced_futures::subscription::from_recipe;
mod ui;
mod vpn_manager;

use geist_vpn::cert_prompt;
use geist_vpn::cert_prompt as certificate_prompt;
use geist_vpn::profile::{ProfileManager, VpnProfile};
use geist_vpn::{cleanup, init};
use std::sync::Arc;
use vpn_manager::VpnManager;

#[derive(Debug, Clone)]
pub enum Message {
    // Connection messages
    Connect(String),
    Disconnect,
    ConnectionResult(Result<(), String>),
    PollStatus,

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
    ProfileModalUpdateHubName(String),
    ProfileModalUpdateAccount(String),
    ProfileModalUpdateTimeout(String),
    ProfileModalUpdateAuthMethodType(crate::ui::modal::AuthMethodType),
    ProfileModalUpdateUsername(String),
    ProfileModalUpdatePassword(String),
    ProfileModalUpdateCertificatePath(String),
    ProfileModalFetchHubs,
    ProfileModalHubListFetched(Result<Vec<String>, String>),
    ProfileModalHubSelected(String),
    ProfileModalSave,

    // UI state messages
    ProfileSelected(String),
    ViewChanged(ViewMode),
    ModalClosed,

    // Status updates
    StatusUpdated(ConnectionStatus),
    CertificatePromptReceived(cert_prompt::CertificatePrompt),
    CertificatePromptDecision(cert_prompt::CertificateDecision),
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
    vpn_manager: Option<VpnManager>,
    loading_profiles: bool,
    connecting: bool,

    // Modal state
    show_profile_modal: bool,
    profile_modal_state: ui::modal::ProfileModalState,

    certificate_prompt_receiver: Option<Arc<flume::Receiver<cert_prompt::CertificatePrompt>>>,
    pending_certificate_prompt: Option<cert_prompt::CertificatePrompt>,
}

impl Default for GeistApp {
    fn default() -> Self {
        Self {
            connection_status: ConnectionStatus::default(),
            selected_profile: None,
            profiles: Vec::new(),
            current_view: ViewMode::All,
            profile_manager: None,
            vpn_manager: None,
            loading_profiles: false,
            connecting: false,
            show_profile_modal: false,
            profile_modal_state: ui::modal::ProfileModalState::default(),
            certificate_prompt_receiver: None,
            pending_certificate_prompt: None,
        }
    }
}

impl GeistApp {
    fn new() -> (Self, Task<Message>) {
        tracing::info!("GeistApp: Starting app initialization");
        let mut app = Self::default();

        let (certificate_prompt_sender, certificate_prompt_receiver) = flume::bounded(8);
        if cert_prompt::register_sender(certificate_prompt_sender).is_err() {
            tracing::warn!("Failed to register certificate prompt sender");
        }
        app.certificate_prompt_receiver = Some(Arc::new(certificate_prompt_receiver));

        tracing::info!("GeistApp: Initializing profile manager");
        // Initialize profile manager
        match ProfileManager::new() {
            Ok(manager) => {
                app.profile_manager = Some(Arc::new(manager));
                tracing::info!("GeistApp: Profile manager initialized successfully");
            }
            Err(e) => {
                tracing::error!("GeistApp: Failed to initialize profile manager: {}", e);
            }
        }

        tracing::info!("GeistApp: Initializing VPN manager");
        // Initialize VPN manager
        match VpnManager::new() {
            Ok(manager) => {
                app.vpn_manager = Some(manager);
                tracing::info!("GeistApp: VPN manager initialized successfully");
            }
            Err(e) => {
                tracing::error!("GeistApp: Failed to initialize VPN manager: {}", e);
            }
        }

        tracing::info!("GeistApp: App initialization complete");

        (
            app,
            iced::Task::perform(async { Message::LoadProfiles }, |_| Message::LoadProfiles),
        )
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
                        return iced::Task::perform(
                            async {
                                Message::ConnectionResult(Err("Profile not found".to_string()))
                            },
                            |msg| msg,
                        );
                    }
                };

                // Use VPN manager for connection (runs in separate thread, non-blocking)
                let result = if let Some(vpn_manager) = &self.vpn_manager {
                    vpn_manager
                        .connect(profile)
                        .map_err(|e| format!("Connection failed: {}", e))
                } else {
                    Err("VPN manager not initialized".to_string())
                };

                iced::Task::perform(async { result }, |result| Message::ConnectionResult(result))
            }

            Message::Disconnect => {
                self.connecting = true;

                // Use VPN manager for disconnection (runs in separate thread, non-blocking)
                let result = if let Some(vpn_manager) = &self.vpn_manager {
                    vpn_manager
                        .disconnect()
                        .map_err(|e| format!("Disconnection failed: {}", e))
                } else {
                    Err("VPN manager not initialized".to_string())
                };

                iced::Task::perform(async { result }, |result| Message::ConnectionResult(result))
            }

            Message::ConnectionResult(result) => {
                self.connecting = false;
                match result {
                    Ok(_) => {
                        // Status will be updated via subscription polling
                        // For immediate feedback, we could trigger a status update here
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

            Message::PollStatus => {
                // Get status from VPN manager
                if let Some(vpn_manager) = &self.vpn_manager {
                    match vpn_manager.get_status() {
                        Ok(status) => {
                            self.connection_status = status;
                        }
                        Err(e) => {
                            tracing::error!("Failed to get VPN status: {}", e);
                            self.connection_status = ConnectionStatus {
                                connected: false,
                                profile_name: None,
                                status_message: format!("Status check failed: {}", e),
                            };
                        }
                    }
                } else {
                    self.connection_status = ConnectionStatus {
                        connected: false,
                        profile_name: None,
                        status_message: "VPN manager not available".to_string(),
                    };
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
                self.profile_modal_state.available_hubs.clear();
                self.profile_modal_state.hub_fetch_error = None;
                iced::Task::none()
            }

            Message::ProfileModalUpdatePort(port) => {
                self.profile_modal_state.port = port;
                self.profile_modal_state.available_hubs.clear();
                self.profile_modal_state.hub_fetch_error = None;
                iced::Task::none()
            }

            Message::ProfileModalUpdateHubName(hub) => {
                self.profile_modal_state.hub_name = hub;
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

            Message::ProfileModalUpdateAuthMethodType(auth_method_type) => {
                self.profile_modal_state.auth_method_type = auth_method_type;
                iced::Task::none()
            }

            Message::ProfileModalUpdateUsername(username) => {
                self.profile_modal_state.username = username;
                iced::Task::none()
            }

            Message::ProfileModalUpdatePassword(password) => {
                self.profile_modal_state.password = password;
                iced::Task::none()
            }

            Message::ProfileModalUpdateCertificatePath(certificate_path) => {
                self.profile_modal_state.certificate_path = certificate_path;
                iced::Task::none()
            }

            Message::ProfileModalFetchHubs => {
                if self.profile_modal_state.fetching_hubs {
                    return iced::Task::none();
                }

                if self.profile_modal_state.host.trim().is_empty() {
                    self.profile_modal_state.hub_fetch_error =
                        Some("Server host is required before fetching Virtual Hubs".into());
                    return iced::Task::none();
                }

                let port_parse = self.profile_modal_state.port.parse::<u16>();
                let port = match port_parse {
                    Ok(p) if p > 0 => p,
                    _ => {
                        self.profile_modal_state.hub_fetch_error =
                            Some("Enter a valid server port before fetching Virtual Hubs".into());
                        return iced::Task::none();
                    }
                };

                let host = self.profile_modal_state.host.clone();
                self.profile_modal_state.fetching_hubs = true;
                self.profile_modal_state.hub_fetch_error = None;

                iced::Task::perform(
                    async move {
                        geist_vpn::hub::enumerate_virtual_hubs(&host, port)
                            .map_err(|e| e.to_string())
                    },
                    |result| Message::ProfileModalHubListFetched(result),
                )
            }

            Message::ProfileModalHubListFetched(result) => {
                self.profile_modal_state.fetching_hubs = false;
                match result {
                    Ok(hubs) => {
                        self.profile_modal_state.available_hubs = hubs;
                        if self.profile_modal_state.available_hubs.is_empty() {
                            self.profile_modal_state.hub_fetch_error =
                                Some("Server returned no Virtual Hubs.".into());
                        } else {
                            if self.profile_modal_state.hub_name.is_empty() {
                                self.profile_modal_state.hub_name =
                                    self.profile_modal_state.available_hubs[0].clone();
                            }
                            self.profile_modal_state.hub_fetch_error = None;
                        }
                    }
                    Err(error) => {
                        self.profile_modal_state.hub_fetch_error = Some(error);
                        self.profile_modal_state.available_hubs.clear();
                    }
                }
                iced::Task::none()
            }

            Message::ProfileModalHubSelected(hub) => {
                self.profile_modal_state.hub_name = hub;
                iced::Task::none()
            }

            Message::ProfileModalSave => {
                if self.profile_modal_state.is_valid() {
                    match self
                        .profile_modal_state
                        .to_profile(if self.profile_modal_state.editing {
                            // Find existing profile ID
                            self.profiles
                                .iter()
                                .find(|p| p.name == self.profile_modal_state.name)
                                .map(|p| p.id.clone())
                        } else {
                            None
                        }) {
                        Ok(profile) => {
                            return iced::Task::perform(
                                async move { Message::SaveProfile(profile) },
                                |msg| msg,
                            );
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
            Message::CertificatePromptReceived(prompt) => {
                self.pending_certificate_prompt = Some(prompt);
                iced::Task::none()
            }
            Message::CertificatePromptDecision(decision) => {
                if let Some(prompt) = self.pending_certificate_prompt.take() {
                    let _ = prompt.response_tx.send(decision);
                    if decision == cert_prompt::CertificateDecision::TrustPermanently {
                        if let Some(profile_id) = prompt.profile_id.as_deref() {
                            if let Some(manager) = &self.profile_manager {
                                match manager.get_profile(profile_id) {
                                    Ok(mut profile) => {
                                        profile
                                            .options
                                            .insert("server_cert".to_string(), prompt.pem.clone());

                                        if let Err(err) = manager.save_profile(&profile) {
                                            tracing::error!(
                                                "Failed to save trusted certificate: {}",
                                                err
                                            );
                                        }

                                        if let Some(local_profile) =
                                            self.profiles.iter_mut().find(|p| p.id == profile_id)
                                        {
                                            local_profile.options.insert(
                                                "server_cert".to_string(),
                                                prompt.pem.clone(),
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        tracing::error!(
                                            "Failed to load profile for certificate trust: {}",
                                            err
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
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
            ui::profiles::view(&self.profiles, &self.current_view, self.loading_profiles),
        ]
        .spacing(20)
        .padding(20);

        let container = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &Theme| iced::widget::container::Style {
                background: Some(theme.palette().background.into()),
                ..Default::default()
            });

        // Add modal if needed
        let base_view = if self.show_profile_modal {
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
            ]
        } else {
            iced::widget::stack![container]
        };

        if let Some(prompt) = &self.pending_certificate_prompt {
            let header = iced::widget::text("Untrusted Server Certificate").size(22);
            let server_info =
                iced::widget::text(format!("Server: {}:{}", prompt.host, prompt.port)).size(16);
            let subject_info = iced::widget::text(format!("Subject: {}", prompt.subject)).size(14);
            let issuer_info = iced::widget::text(format!("Issuer: {}", prompt.issuer)).size(14);
            let fingerprint_info =
                iced::widget::text(format!("Fingerprint (SHA1): {}", prompt.fingerprint)).size(12);

            let trust_temp_button =
                button(text("Trust Temporarily")).on_press(Message::CertificatePromptDecision(
                    cert_prompt::CertificateDecision::TrustTemporarily,
                ));
            let trust_perm_button =
                button(text("Trust Permanently")).on_press(Message::CertificatePromptDecision(
                    cert_prompt::CertificateDecision::TrustPermanently,
                ));
            let cancel_button = button(text("Cancel")).on_press(
                Message::CertificatePromptDecision(cert_prompt::CertificateDecision::Reject),
            );

            let button_row = row![trust_temp_button, trust_perm_button, cancel_button]
                .spacing(8)
                .align_y(iced::Alignment::Center);

            let modal_content = column![
                header,
                server_info,
                subject_info,
                issuer_info,
                fingerprint_info,
                button_row
            ]
            .spacing(12)
            .padding(24)
            .max_width(480.0);

            let prompt_modal = iced::widget::container(modal_content)
                .width(Length::Fill)
                .height(Length::Shrink)
                .style(|theme: &Theme| iced::widget::container::Style {
                    background: Some(theme.palette().background.into()),
                    border: iced::Border {
                        color: theme.palette().primary,
                        width: 2.0,
                        radius: 12.0.into(),
                    },
                    ..Default::default()
                });

            return iced::widget::stack![
                base_view,
                iced::widget::opaque(
                    iced::widget::container(prompt_modal)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                        .padding(20)
                        .style(|theme: &Theme| iced::widget::container::Style {
                            background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6).into()),
                            ..Default::default()
                        })
                )
            ]
            .into();
        }

        base_view.into()
    }

    fn theme(&self) -> Theme {
        Theme::default()
    }

    fn subscription(&self) -> Subscription<Message> {
        let status_sub = if self.vpn_manager.is_some() {
            iced::time::every(std::time::Duration::from_secs(2)).map(|_| Message::PollStatus)
        } else {
            Subscription::none()
        };

        let cert_sub = if let Some(receiver) = &self.certificate_prompt_receiver {
            from_recipe(certificate_prompt::subscription(receiver.clone()))
                .map(Message::CertificatePromptReceived)
        } else {
            Subscription::none()
        };

        Subscription::batch(vec![status_sub, cert_sub])
    }
}

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application("Geist VPN", GeistApp::update, GeistApp::view)
        .subscription(GeistApp::subscription)
        .theme(GeistApp::theme)
        .run_with(GeistApp::new)
}
