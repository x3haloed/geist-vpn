use iced::widget::{button, column, container, pick_list, row, text, Column};
use iced::{Element, Length};
use crate::{ConnectionStatus, Message, ViewMode};

use geist_vpn::profile::VpnProfile;

pub fn view<'a>(
    profiles: &'a [VpnProfile],
    selected_profile: Option<&'a str>,
    connecting: bool,
    connection_status: &'a ConnectionStatus,
) -> Element<'a, Message> {
    let title = text("Quick Connect")
        .size(18)
        .style(|theme: &iced::Theme| iced::widget::text::Style {
            color: Some(theme.palette().primary),
        });

    // Create profile options for pick list
    let profile_options: Vec<String> = profiles
        .iter()
        .map(|p| format!("{} ({})", p.name, p.host))
        .collect();

    let profile_selector: iced::Element<Message> = if profile_options.is_empty() {
        iced::widget::text("No profiles available")
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            })
            .into()
    } else {
        let current_selection = selected_profile
            .and_then(|selected_id| {
                profiles.iter().find(|p| p.id == selected_id)
            })
            .map(|p| format!("{} ({})", p.name, p.host));

        pick_list(
            profile_options,
            current_selection,
            |selected_text| {
                // Find the profile ID from the selected text
                if let Some(profile) = profiles.iter().find(|p| {
                    format!("{} ({})", p.name, p.host) == selected_text
                }) {
                    Message::ProfileSelected(profile.id.clone())
                } else {
                    Message::ProfileSelected(String::new())
                }
            }
        )
        .placeholder("Select a profile...")
        .width(Length::Fill)
        .into()
    };

    let connect_button = if connection_status.connected {
        button(
            if connecting {
                text("Disconnecting...")
            } else {
                text("Disconnect")
            }
        )
        .on_press(Message::Disconnect)
        .style(|theme: &iced::Theme, status: iced::widget::button::Status| {
            let palette = theme.palette();
            iced::widget::button::Style {
                background: Some(iced::Background::Color(
                    if status == iced::widget::button::Status::Hovered {
                        palette.danger
                    } else {
                        palette.danger
                    }
                )),
                text_color: palette.text,
                border: iced::Border::default().rounded(4.0),
                ..Default::default()
            }
        })
    } else {
        let button_text = if connecting {
            "Connecting..."
        } else {
            "Connect"
        };

        let button_widget = button(text(button_text))
            .style(|theme: &iced::Theme, status: iced::widget::button::Status| {
                let palette = theme.palette();
                iced::widget::button::Style {
                    background: Some(iced::Background::Color(
                        if status == iced::widget::button::Status::Hovered {
                            palette.primary.scale_alpha(0.8)
                        } else {
                            palette.primary
                        }
                    )),
                    text_color: palette.text,
                    border: iced::Border::default().rounded(4.0),
                    ..Default::default()
                }
            });

        if connecting || selected_profile.is_none() {
            button_widget
        } else {
            button_widget.on_press(Message::Connect(selected_profile.unwrap().to_string()))
        }
    };

    let button_row = row![
        connect_button
    ]
    .spacing(8);

    let connection_info = if connection_status.connected {
        column![
            text(format!("Connected to: {}", connection_status.profile_name.as_deref().unwrap_or("Unknown"))),
            text(format!("Status: {}", connection_status.status_message))
        ]
        .spacing(4)
    } else if connecting {
        column![
            text("Status: Connecting...").style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().primary),
            })
        ]
    } else if connection_status.status_message != "Disconnected" {
        column![
            text(format!("Status: {}", connection_status.status_message)).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().danger),
            })
        ]
    } else {
        column![
            text("Status: Disconnected")
        ]
    };

    let content = column![
        title,
        profile_selector,
        button_row,
        connection_info
    ]
    .spacing(12)
    .padding(16);

    container(content)
        .width(Length::Fill)
        .style(|theme: &iced::Theme| iced::widget::container::Style {
            background: Some(theme.palette().background.into()),
            border: iced::Border {
                color: theme.palette().primary,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}
