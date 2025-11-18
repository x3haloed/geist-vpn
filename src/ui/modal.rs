use iced::widget::{button, column, container, row, text, text_input, pick_list, Column};
use iced::{Element, Length};
use crate::Message;

use geist_vpn::profile::{VpnProfile, VpnProtocol, AuthMethod};

#[derive(Debug, Clone)]
pub struct ProfileModalState {
    pub name: String,
    pub host: String,
    pub port: String,
    pub protocol: VpnProtocol,
    pub account_name: String,
    pub timeout: String,
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
            editing: false,
        }
    }
}

impl ProfileModalState {
    pub fn from_profile(profile: &VpnProfile) -> Self {
        Self {
            name: profile.name.clone(),
            host: profile.host.clone(),
            port: profile.port.to_string(),
            protocol: profile.protocol.clone(),
            account_name: profile.account_name.clone(),
            timeout: profile.timeout.to_string(),
            editing: true,
        }
    }

    pub fn to_profile(&self, id: Option<String>) -> Result<VpnProfile, String> {
        let port: u16 = self.port.parse().map_err(|_| "Invalid port number")?;
        let timeout: u32 = self.timeout.parse().map_err(|_| "Invalid timeout value")?;

        let mut profile = VpnProfile::new(
            self.name.clone(),
            self.host.clone(),
            port,
            self.protocol.clone(),
        );

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
        !self.name.is_empty() && !self.host.is_empty() && !self.port.is_empty()
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
