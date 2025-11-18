use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Element, Length};
use crate::{Message, ViewMode};

use geist_vpn::profile::VpnProfile;

pub fn view<'a>(
    profiles: &'a [VpnProfile],
    current_view: &ViewMode,
    loading: bool,
) -> Element<'a, Message> {
    let title_text = match current_view {
        ViewMode::All => "VPN Profiles",
        ViewMode::Favorites => "⭐ Favorite VPN Profiles",
        ViewMode::Recent => "🕒 Recently Used VPN Profiles",
    };

    let title = text(title_text)
        .size(18)
        .style(|theme: &iced::Theme| iced::widget::text::Style {
            color: Some(theme.palette().primary),
        });

    let add_button = button(text("Add Profile"))
        .on_press(Message::CreateProfile)
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

    let header_row = row![
        title,
        iced::widget::horizontal_space(),
        add_button
    ]
    .align_y(iced::Alignment::Center);

    let content = if loading {
        column![
            header_row,
            text("Loading profiles...").style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            })
        ]
        .spacing(12)
        .padding(16)
    } else {
        // Filter profiles based on current view
        let filtered_profiles: Vec<&VpnProfile> = match current_view {
            ViewMode::All => profiles.iter().collect(),
            ViewMode::Favorites => profiles.iter().filter(|p| p.metadata.favorite).collect(),
            ViewMode::Recent => {
                // Sort by last_used_at and take first 10
                let mut recent: Vec<&VpnProfile> = profiles.iter()
                    .filter(|p| p.metadata.last_used_at.is_some())
                    .collect();
                recent.sort_by(|a, b| {
                    b.metadata.last_used_at.cmp(&a.metadata.last_used_at)
                });
                recent.into_iter().take(10).collect()
            }
        };

        if filtered_profiles.is_empty() {
            let empty_message = match current_view {
                ViewMode::All => "No VPN profiles configured yet.\nClick \"Add Profile\" to get started.",
                ViewMode::Favorites => "No favorite profiles yet.\nMark profiles as favorites to see them here.",
                ViewMode::Recent => "No recently used profiles yet.\nConnect to profiles to see them here.",
            };

            column![
                header_row,
                text(empty_message).style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                })
            ]
            .spacing(12)
            .padding(16)
        } else {
            let profile_elements: Vec<Element<Message>> = filtered_profiles
                .into_iter()
                .map(|profile| create_profile_element(profile))
                .collect();

            let profiles_column = Column::with_children(profile_elements)
                .spacing(8);

            let scrollable_content = scrollable(profiles_column)
                .height(Length::Fill);

            column![
                header_row,
                scrollable_content
            ]
            .spacing(12)
            .padding(16)
        }
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(400.0))
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

fn create_profile_element<'a>(profile: &'a VpnProfile) -> Element<'a, Message> {
    let favorite_icon = if profile.metadata.favorite {
        "★"
    } else {
        "☆"
    };

    let favorite_button = button(text(favorite_icon))
        .on_press(Message::ToggleFavorite(profile.id.clone()))
        .style(|theme: &iced::Theme, status: iced::widget::button::Status| {
            let palette = theme.palette();
            iced::widget::button::Style {
                background: Some(iced::Background::Color(palette.background)),
                text_color: palette.text,
                border: iced::Border::default().rounded(4.0),
                ..Default::default()
            }
        });

    let edit_button = button(text("Edit"))
        .on_press(Message::EditProfile(profile.id.clone()))
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

    let delete_button = button(text("Delete"))
        .on_press(Message::DeleteProfile(profile.id.clone()))
        .style(|theme: &iced::Theme, status: iced::widget::button::Status| {
            let palette = theme.palette();
            iced::widget::button::Style {
                background: Some(iced::Background::Color(palette.danger)),
                text_color: palette.text,
                border: iced::Border::default().rounded(4.0),
                ..Default::default()
            }
        });

    let profile_info = column![
        text(format!("{} {}", profile.name, if profile.metadata.favorite { "⭐" } else { "" }))
            .size(16)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            }),
        text(format!("{} • {:?}", profile.host, profile.protocol))
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            }),
        text(&profile.description)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            })
    ]
    .spacing(4);

    let metadata_info = if let Some(last_used) = &profile.metadata.last_used_at {
        column![
            text(format!("Last used: {}", last_used.format("%Y-%m-%d %H:%M")))
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                }),
            text(format!("Used {} time{}", profile.metadata.usage_count,
                if profile.metadata.usage_count == 1 { "" } else { "s" }))
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                })
        ]
        .spacing(2)
    } else {
        column![
            text("Never used")
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                })
        ]
    };

    let info_column = column![
        profile_info,
        metadata_info
    ]
    .spacing(8);

    let action_buttons = row![
        favorite_button,
        edit_button,
        delete_button
    ]
    .spacing(8);

    let profile_row = row![
        info_column,
        iced::widget::horizontal_space(),
        action_buttons
    ]
    .spacing(16)
    .align_y(iced::Alignment::Center)
    .padding(12);

    container(profile_row)
        .width(Length::Fill)
        .style(|theme: &iced::Theme| iced::widget::container::Style {
            background: Some(theme.palette().background.into()),
            border: iced::Border {
                color: theme.palette().background,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
}
