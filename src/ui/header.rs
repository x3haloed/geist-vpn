use iced::widget::{container, row, text};
use iced::{Element, Length, Color};
use crate::{ConnectionStatus, Message};

pub fn view(connection_status: &ConnectionStatus) -> Element<Message> {
    let title = text("Geist VPN")
        .size(24)
        .style(|theme: &iced::Theme| text::Style {
            color: Some(theme.palette().primary),
        });

    let status_indicator = container(
        row![
            text("●")
                .style(move |theme: &iced::Theme| text::Style {
                    color: Some(if connection_status.connected {
                        Color::from_rgb(0.0, 0.8, 0.0) // Green
                    } else if connection_status.status_message.contains("Connecting") {
                        Color::from_rgb(1.0, 0.8, 0.0) // Orange
                    } else if connection_status.status_message.contains("Error") {
                        Color::from_rgb(0.8, 0.0, 0.0) // Red
                    } else {
                        Color::from_rgb(0.5, 0.5, 0.5) // Gray
                    }),
                }),
            text(&connection_status.status_message)
                .size(16)
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
    )
    .padding(8);

    let header_row = row![
        title,
        iced::widget::horizontal_space(),
        status_indicator
    ]
    .align_y(iced::Alignment::Center);

    container(header_row)
        .width(Length::Fill)
        .padding(16)
        .style(|theme: &iced::Theme| iced::widget::container::Style {
            background: Some(theme.palette().background.into()),
            border: iced::Border::default().color(theme.palette().primary).width(1.0).rounded(8.0),
            ..Default::default()
        })
        .into()
}
