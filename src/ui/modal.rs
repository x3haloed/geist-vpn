use iced::widget::{button, column, container, row, text, text_input, pick_list, Column};
use iced::{Element, Length};
use crate::Message;

use geist_vpn::profile::{VpnProfile, VpnProtocol, AuthMethod};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethodType {
    Password,
    Radius,
    NtDomain,
}

impl std::fmt::Display for AuthMethodType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMethodType::Password => write!(f, "Username/Password"),
            AuthMethodType::Radius => write!(f, "RADIUS"),
            AuthMethodType::NtDomain => write!(f, "NT Domain"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileModalState {
    pub name: String,
    pub host: String,
    pub port: String,
    pub protocol: VpnProtocol,
    pub account_name: String,
    pub timeout: String,
    pub auth_method_type: AuthMethodType,
    pub username: String,
    pub password: String,
    pub editing: bool,
}

impl Default for ProfileModalState {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: "443".to_string(),
            protocol: VpnProtocol::SslVpn,
            account_name: String::new(),
            timeout: "30".to_string(),
            auth_method_type: AuthMethodType::Password,
            username: String::new(),
            password: String::new(),
            editing: false,
        }
    }
}

impl ProfileModalState {
    pub fn from_profile(profile: &VpnProfile) -> Self {
        let (auth_method_type, username, password) = match &profile.auth {
            AuthMethod::Password { username, password } => (AuthMethodType::Password, username.clone(), password.clone()),
            AuthMethod::Radius => (AuthMethodType::Radius, String::new(), String::new()),
            AuthMethod::NtDomain { username, password, .. } => (AuthMethodType::NtDomain, username.clone(), password.clone()),
            _ => (AuthMethodType::Password, String::new(), String::new()),
        };

        Self {
            name: profile.name.clone(),
            host: profile.host.clone(),
            port: profile.port.to_string(),
            protocol: profile.protocol.clone(),
            account_name: profile.account_name.clone(),
            timeout: profile.timeout.to_string(),
            auth_method_type,
            username,
            password,
            editing: true,
        }
    }

    pub fn to_profile(&self, id: Option<String>) -> Result<VpnProfile, String> {
        let port: u16 = self.port.parse().map_err(|_| "Invalid port number")?;
        let timeout: u32 = self.timeout.parse().map_err(|_| "Invalid timeout value")?;

        // Create the auth method based on the selected type and credentials
        let auth = match self.auth_method_type {
            AuthMethodType::Password => {
                if self.username.is_empty() {
                    return Err("Username cannot be empty".to_string());
                }
                if self.password.is_empty() {
                    return Err("Password cannot be empty".to_string());
                }
                AuthMethod::Password {
                    username: self.username.clone(),
                    password: self.password.clone(),
                }
            }
            AuthMethodType::Radius => {
                AuthMethod::Radius
            }
            AuthMethodType::NtDomain => {
                if self.username.is_empty() {
                    return Err("Username cannot be empty".to_string());
                }
                if self.password.is_empty() {
                    return Err("Password cannot be empty".to_string());
                }
                AuthMethod::NtDomain {
                    username: self.username.clone(),
                    password: self.password.clone(),
                    domain: "WORKGROUP".to_string(), // Default domain
                }
            }
        };

        let mut profile = VpnProfile::new(
            self.name.clone(),
            self.host.clone(),
            port,
            self.protocol.clone(),
        );

        // Set the authentication method
        profile.auth = auth;

        // Override defaults with our values
        profile.account_name = self.account_name.clone();
        profile.timeout = timeout;

        // If editing, preserve the original ID
        if let Some(existing_id) = id {
            profile.id = existing_id;
        }

        Ok(profile)
    }

    pub fn is_valid(&self) -> bool {
        let basic_valid = !self.name.is_empty() && !self.host.is_empty() && !self.port.is_empty();

        if !basic_valid {
            return false;
        }

        // Check auth-specific requirements
        match self.auth_method_type {
            AuthMethodType::Password => !self.username.is_empty() && !self.password.is_empty(),
            AuthMethodType::Radius => true, // RADIUS doesn't need credentials
            AuthMethodType::NtDomain => !self.username.is_empty() && !self.password.is_empty(),
        }
    }
}

pub fn view<'a>(state: &'a ProfileModalState) -> Element<'a, Message> {
    let title = text(if state.editing { "Edit VPN Profile" } else { "Add VPN Profile" })
        .size(20)
        .style(|theme: &iced::Theme| iced::widget::text::Style {
            color: Some(theme.palette().primary),
        });

    let name_input = text_input("Profile Name", &state.name)
        .on_input(|value| Message::ProfileModalUpdateName(value))
        .padding(8);

    let host_input = text_input("Server Host (e.g., vpn.example.com)", &state.host)
        .on_input(|value| Message::ProfileModalUpdateHost(value))
        .padding(8);

    let port_input = text_input("Port", &state.port)
        .on_input(|value| Message::ProfileModalUpdatePort(value))
        .padding(8);

    let protocol_options = vec![
        VpnProtocol::SslVpn,
        VpnProtocol::L2tpIpsec,
        VpnProtocol::OpenVpn,
        VpnProtocol::Sstp,
    ];

    let protocol_picker = pick_list(
        protocol_options,
        Some(state.protocol.clone()),
        |protocol| Message::ProfileModalUpdateProtocol(protocol),
    )
    .placeholder("Select Protocol");

    let account_input = text_input("Account Name (optional)", &state.account_name)
        .on_input(|value| Message::ProfileModalUpdateAccount(value))
        .padding(8);

    let timeout_input = text_input("Connection Timeout (seconds)", &state.timeout)
        .on_input(|value| Message::ProfileModalUpdateTimeout(value))
        .padding(8);

    let auth_options = vec![
        AuthMethodType::Password,
        AuthMethodType::Radius,
        AuthMethodType::NtDomain,
    ];

    let auth_picker = pick_list(
        auth_options,
        Some(state.auth_method_type),
        |auth_method_type| Message::ProfileModalUpdateAuthMethodType(auth_method_type),
    )
    .placeholder("Select Authentication Method");

    let username_input = text_input("Username", &state.username)
        .on_input(|value| Message::ProfileModalUpdateUsername(value))
        .padding(8);

    let password_input = text_input("Password", &state.password)
        .on_input(|value| Message::ProfileModalUpdatePassword(value))
        .padding(8)
        .secure(true);

    let cancel_button = button(text("Cancel"))
        .on_press(Message::ModalClosed)
        .style(|theme: &iced::Theme, status: iced::widget::button::Status| {
            let palette = theme.palette();
            iced::widget::button::Style {
                background: Some(iced::Background::Color(palette.background)),
                text_color: palette.text,
                border: iced::Border {
                    color: palette.primary,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        });

    let save_button = button(text("Save Profile"))
        .on_press(Message::ProfileModalSave)
        .style(|theme: &iced::Theme, status: iced::widget::button::Status| {
            let palette = theme.palette();
            iced::widget::button::Style {
                background: Some(iced::Background::Color(
                    if state.is_valid() {
                        if status == iced::widget::button::Status::Hovered {
                            palette.primary.scale_alpha(0.8)
                        } else {
                            palette.primary
                        }
                    } else {
                        palette.background
                    }
                )),
                text_color: if state.is_valid() {
                    palette.text
                } else {
                    palette.text
                },
                border: iced::Border::default().rounded(4.0),
                ..Default::default()
            }
        });

    let button_row = row![
        cancel_button,
        save_button
    ]
    .spacing(8);

    let form = column![
        row![text("Profile Name").size(14), name_input].spacing(8).align_y(iced::Alignment::Center),
        row![text("Server Host").size(14), host_input].spacing(8).align_y(iced::Alignment::Center),
        row![text("Port").size(14), port_input].spacing(8).align_y(iced::Alignment::Center),
        row![text("Protocol").size(14), protocol_picker].spacing(8).align_y(iced::Alignment::Center),
        row![text("Auth Method").size(14), auth_picker].spacing(8).align_y(iced::Alignment::Center),
        row![text("Username").size(14), username_input].spacing(8).align_y(iced::Alignment::Center),
        row![text("Password").size(14), password_input].spacing(8).align_y(iced::Alignment::Center),
        row![text("Account Name").size(14), account_input].spacing(8).align_y(iced::Alignment::Center),
        row![text("Timeout").size(14), timeout_input].spacing(8).align_y(iced::Alignment::Center),
    ]
    .spacing(12);

    let content = column![
        title,
        form,
        button_row
    ]
    .spacing(16)
    .padding(24)
    .max_width(500.0);

    container(content)
        .style(|theme: &iced::Theme| iced::widget::container::Style {
            background: Some(theme.palette().background.into()),
            border: iced::Border {
                color: theme.palette().primary,
                width: 2.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        })
        .into()
}
