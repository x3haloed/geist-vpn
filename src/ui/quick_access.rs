use crate::{Message, ViewMode};
use iced::widget::{button, container, row, text};
use iced::{Element, Length};

pub fn view(current_view: &ViewMode) -> Element<Message> {
    let title =
        text("Quick Access")
            .size(18)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().primary),
            });

    let all_button = button(text("📋 All Profiles"))
        .on_press(Message::ViewChanged(ViewMode::All))
        .style(
            |theme: &iced::Theme, status: iced::widget::button::Status| {
                let palette = theme.palette();
                if *current_view == ViewMode::All {
                    iced::widget::button::Style {
                        background: Some(iced::Background::Color(palette.primary)),
                        text_color: palette.text,
                        border: iced::Border::default().rounded(4.0),
                        ..Default::default()
                    }
                } else {
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
                }
            },
        );

    let favorites_button = button(text("⭐ Favorites"))
        .on_press(Message::ViewChanged(ViewMode::Favorites))
        .style(
            |theme: &iced::Theme, status: iced::widget::button::Status| {
                let palette = theme.palette();
                if *current_view == ViewMode::Favorites {
                    iced::widget::button::Style {
                        background: Some(iced::Background::Color(palette.primary)),
                        text_color: palette.text,
                        border: iced::Border::default().rounded(4.0),
                        ..Default::default()
                    }
                } else {
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
                }
            },
        );

    let recent_button = button(text("🕒 Recent"))
        .on_press(Message::ViewChanged(ViewMode::Recent))
        .style(
            |theme: &iced::Theme, status: iced::widget::button::Status| {
                let palette = theme.palette();
                if *current_view == ViewMode::Recent {
                    iced::widget::button::Style {
                        background: Some(iced::Background::Color(palette.primary)),
                        text_color: palette.text,
                        border: iced::Border::default().rounded(4.0),
                        ..Default::default()
                    }
                } else {
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
                }
            },
        );

    let button_row = row![all_button, favorites_button, recent_button].spacing(8);

    let content = iced::widget::column![title, button_row]
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
