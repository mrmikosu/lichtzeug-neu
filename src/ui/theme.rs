use iced::widget::{button, container};
use iced::{Background, Border, Color, Shadow, Vector, border};

pub fn app_background() -> Color {
    Color::from_rgb8(8, 11, 16)
}

pub fn panel_bg() -> Color {
    Color::from_rgb8(15, 19, 26)
}

pub fn panel_bg_alt() -> Color {
    Color::from_rgb8(21, 27, 35)
}

pub fn panel_inner_bg() -> Color {
    Color::from_rgb8(24, 31, 40)
}

pub fn timeline_background() -> Color {
    Color::from_rgb8(11, 15, 22)
}

pub fn timeline_header() -> Color {
    Color::from_rgb8(19, 24, 32)
}

pub fn text_primary() -> Color {
    Color::from_rgb8(234, 238, 245)
}

pub fn text_muted() -> Color {
    Color::from_rgb8(127, 138, 152)
}

pub fn border_strong() -> Color {
    Color::from_rgba8(99, 112, 130, 0.72)
}

pub fn border_soft() -> Color {
    Color::from_rgba8(71, 82, 97, 0.52)
}

pub fn accent_playhead() -> Color {
    Color::from_rgb8(255, 174, 92)
}

pub fn accent_snap() -> Color {
    Color::from_rgb8(99, 212, 255)
}

pub fn accent_blue() -> Color {
    Color::from_rgb8(96, 146, 255)
}

pub fn success() -> Color {
    Color::from_rgb8(71, 215, 141)
}

pub fn warning() -> Color {
    Color::from_rgb8(236, 182, 84)
}

pub fn muted_chip() -> Color {
    Color::from_rgb8(110, 121, 139)
}

pub fn grid_bar() -> Color {
    Color::from_rgba8(115, 137, 165, 0.34)
}

pub fn grid_beat() -> Color {
    Color::from_rgba8(84, 101, 121, 0.24)
}

pub fn grid_subdivision() -> Color {
    Color::from_rgba8(63, 74, 87, 0.16)
}

pub fn panel() -> container::Style {
    container::Style::default()
        .background(panel_bg())
        .color(text_primary())
        .border(
            Border::default()
                .rounded(18)
                .width(1)
                .color(Color::from_rgba8(112, 126, 145, 0.34)),
        )
        .shadow(Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.28),
            offset: Vector::new(0.0, 18.0),
            blur_radius: 34.0,
        })
}

pub fn panel_subtle() -> container::Style {
    container::Style::default()
        .background(panel_bg_alt())
        .color(text_primary())
        .border(
            Border::default()
                .rounded(14)
                .width(1)
                .color(Color::from_rgba8(112, 126, 145, 0.28)),
        )
}

pub fn panel_inner() -> container::Style {
    container::Style::default()
        .background(panel_inner_bg())
        .color(text_primary())
        .border(Border::default().rounded(12).width(1).color(border_soft()))
}

pub fn timeline_shell() -> container::Style {
    container::Style::default()
        .background(timeline_background())
        .color(text_primary())
        .border(
            Border::default()
                .rounded(18)
                .width(1)
                .color(Color::from_rgba8(122, 138, 160, 0.52)),
        )
        .shadow(Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.36),
            offset: Vector::new(0.0, 20.0),
            blur_radius: 34.0,
        })
}

pub fn status_bar() -> container::Style {
    container::Style::default()
        .background(panel_bg_alt())
        .color(text_primary())
        .border(
            Border::default()
                .rounded(14)
                .width(1)
                .color(Color::from_rgba8(112, 126, 145, 0.32)),
        )
}

pub fn panel_tinted(accent: Color) -> container::Style {
    let background = mix(panel_bg_alt(), accent, 0.1);

    container::Style::default()
        .background(background)
        .color(text_primary())
        .border(
            Border::default()
                .rounded(18)
                .width(1)
                .color(Color::from_rgba(accent.r, accent.g, accent.b, 0.34)),
        )
        .shadow(Shadow {
            color: Color::from_rgba(accent.r, accent.g, accent.b, 0.08),
            offset: Vector::new(0.0, 16.0),
            blur_radius: 30.0,
        })
}

pub fn track_card(track_color: Color, selected: bool) -> container::Style {
    let border_color = if selected { track_color } else { border_soft() };

    let background = if selected {
        mix(panel_inner_bg(), track_color, 0.12)
    } else {
        panel_inner_bg()
    };

    container::Style::default()
        .background(background)
        .color(text_primary())
        .border(
            Border::default()
                .rounded(14)
                .width(if selected { 2 } else { 1 })
                .color(border_color),
        )
        .shadow(if selected {
            Shadow {
                color: Color::from_rgba(track_color.r, track_color.g, track_color.b, 0.1),
                offset: Vector::new(0.0, 10.0),
                blur_radius: 22.0,
            }
        } else {
            Shadow::default()
        })
}

pub fn color_bar(color: Color) -> container::Style {
    container::Style::default()
        .background(color)
        .border(border::rounded(10))
}

pub fn transport_button(status: button::Status, active: bool) -> button::Style {
    let base = if active {
        accent_playhead()
    } else {
        accent_blue()
    };

    let mut style = button::Style::default().with_background(base);
    style.border = Border::default().rounded(12).width(1).color(base);
    style.text_color = Color::from_rgb8(17, 22, 28);

    match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(lighten(base, 0.08))),
            ..style
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(darken(base, 0.08))),
            ..style
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(Color::from_rgba8(80, 90, 101, 0.45))),
            text_color: text_muted(),
            ..style
        },
        button::Status::Active => style,
    }
}

pub fn toggle_button(status: button::Status, active: bool, accent: Color) -> button::Style {
    let fill = if active { accent } else { panel_inner_bg() };

    let text = if active {
        Color::from_rgb8(14, 18, 24)
    } else {
        text_primary()
    };

    let style = button::Style {
        background: Some(Background::Color(fill)),
        text_color: text,
        border: Border::default().rounded(10).width(1).color(if active {
            accent
        } else {
            border_soft()
        }),
        shadow: Shadow::default(),
    };

    match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(lighten(fill, 0.08))),
            ..style
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(darken(fill, 0.08))),
            ..style
        },
        button::Status::Disabled | button::Status::Active => style,
    }
}

fn lighten(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r + amount).min(1.0),
        g: (color.g + amount).min(1.0),
        b: (color.b + amount).min(1.0),
        a: color.a,
    }
}

fn darken(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r - amount).max(0.0),
        g: (color.g - amount).max(0.0),
        b: (color.b - amount).max(0.0),
        a: color.a,
    }
}

fn mix(base: Color, accent: Color, amount: f32) -> Color {
    Color {
        r: base.r + (accent.r - base.r) * amount,
        g: base.g + (accent.g - base.g) * amount,
        b: base.b + (accent.b - base.b) * amount,
        a: 1.0,
    }
}
