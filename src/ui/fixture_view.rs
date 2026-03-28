use crate::core::{AppEvent, FixtureGroup, FixtureGroupId, StudioState};
use crate::ui::theme;
use iced::mouse;
use iced::widget::canvas::{self, Canvas, Path, Stroke, Text};
use iced::widget::container;
use iced::{Color, Element, Length, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector};

#[derive(Debug, Clone)]
struct FixtureProgram {
    groups: Vec<FixtureGroup>,
    selected: Option<FixtureGroupId>,
}

pub fn view(state: &StudioState) -> Element<'_, AppEvent> {
    container(Canvas::new(FixtureProgram {
        groups: state.fixture_system.groups.clone(),
        selected: state.fixture_system.selected,
    }))
    .height(Length::Fixed(220.0))
    .width(Length::Fill)
    .style(|_| theme::panel_inner())
    .into()
}

impl canvas::Program<AppEvent> for FixtureProgram {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<AppEvent>) {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_in(bounds) else {
                    return (canvas::event::Status::Ignored, None);
                };

                self.group_at_point(position, bounds.size()).map_or(
                    (canvas::event::Status::Ignored, None),
                    |group_id| {
                        (
                            canvas::event::Status::Captured,
                            Some(AppEvent::SelectFixtureGroup(group_id)),
                        )
                    },
                )
            }
            _ => (canvas::event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let hovered = cursor
            .position_in(bounds)
            .and_then(|position| self.group_at_point(position, bounds.size()));
        self.draw_background(&mut frame, bounds.size());
        self.draw_groups(&mut frame, bounds.size(), hovered);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        cursor
            .position_in(bounds)
            .and_then(|position| self.group_at_point(position, bounds.size()))
            .map(|_| mouse::Interaction::Pointer)
            .unwrap_or_default()
    }
}

impl FixtureProgram {
    fn draw_background(&self, frame: &mut canvas::Frame<Renderer>, size: Size) {
        let horizon_y = 42.0;
        let floor_y = size.height - 24.0;

        frame.fill_rectangle(Point::ORIGIN, size, theme::panel_inner_bg());

        let stage = Path::new(|builder| {
            builder.move_to(Point::new(22.0, floor_y));
            builder.line_to(Point::new(size.width * 0.34, horizon_y + 14.0));
            builder.line_to(Point::new(size.width * 0.66, horizon_y + 14.0));
            builder.line_to(Point::new(size.width - 22.0, floor_y));
            builder.close();
        });
        frame.fill(&stage, Color::from_rgba8(22, 32, 44, 0.95));
        frame.stroke(
            &stage,
            Stroke::default()
                .with_color(theme::border_soft())
                .with_width(1.0),
        );

        let vanishing = Point::new(size.width * 0.5, horizon_y);
        for x_factor in [0.18, 0.34, 0.5, 0.66, 0.82] {
            frame.stroke(
                &Path::line(
                    Point::new(size.width * x_factor, floor_y),
                    Point::new(vanishing.x, horizon_y + 14.0),
                ),
                Stroke::default()
                    .with_color(Color::from_rgba8(84, 101, 121, 0.16))
                    .with_width(0.8),
            );
        }

        for row in 0..4 {
            let blend = row as f32 / 4.0;
            let y = floor_y - (row as f32 * 30.0);
            let left = 22.0 + blend * (size.width * 0.28);
            let right = size.width - 22.0 - blend * (size.width * 0.28);
            frame.stroke(
                &Path::line(Point::new(left, y), Point::new(right, y)),
                Stroke::default()
                    .with_color(Color::from_rgba8(84, 101, 121, 0.18))
                    .with_width(0.8),
            );
        }

        frame.fill_text(Text {
            content: "Fixture Space".to_owned(),
            position: Point::new(18.0, 22.0),
            color: theme::text_primary(),
            size: Pixels(14.0),
            ..Text::default()
        });
    }

    fn draw_groups(
        &self,
        frame: &mut canvas::Frame<Renderer>,
        size: Size,
        hovered: Option<FixtureGroupId>,
    ) {
        for group in &self.groups {
            let accent = group.accent.to_iced();
            let is_selected = self.selected == Some(group.id);
            let is_hovered = hovered == Some(group.id);
            let group_nodes = self.project_nodes(group, size);
            let group_bounds = self.group_bounds(group, size);
            let glow_alpha = 0.18
                + ((group.output_level as f32 / 1000.0) * 0.44)
                + if is_hovered { 0.12 } else { 0.0 };

            if is_selected || is_hovered {
                let shell = Path::rounded_rectangle(
                    group_bounds.position(),
                    group_bounds.size(),
                    iced::border::Radius::new(12.0),
                );
                frame.fill(
                    &shell,
                    Color::from_rgba(
                        accent.r,
                        accent.g,
                        accent.b,
                        if is_selected { 0.12 } else { 0.07 },
                    ),
                );
                frame.stroke(
                    &shell,
                    Stroke::default()
                        .with_color(Color::from_rgba(
                            accent.r,
                            accent.g,
                            accent.b,
                            if is_selected { 0.7 } else { 0.4 },
                        ))
                        .with_width(if is_selected { 1.8 } else { 1.0 }),
                );
            }

            for projected in &group_nodes {
                frame.stroke(
                    &Path::line(projected.fixture_point, projected.beam_target),
                    Stroke::default()
                        .with_color(Color {
                            a: glow_alpha,
                            ..accent
                        })
                        .with_width(if is_selected {
                            2.0
                        } else if is_hovered {
                            1.6
                        } else {
                            1.2
                        }),
                );

                let beam_cone = Path::new(|builder| {
                    builder.move_to(projected.fixture_point + Vector::new(-4.0, 0.0));
                    builder.line_to(projected.beam_target + Vector::new(-11.0, 12.0));
                    builder.line_to(projected.beam_target + Vector::new(11.0, 12.0));
                    builder.line_to(projected.fixture_point + Vector::new(4.0, 0.0));
                    builder.close();
                });
                frame.fill(
                    &beam_cone,
                    Color {
                        a: glow_alpha * 0.34,
                        ..accent
                    },
                );

                let glow_radius = 8.0 + (group.output_level as f32 / 1000.0) * 6.0;
                frame.fill(
                    &Path::circle(projected.fixture_point, glow_radius),
                    Color {
                        a: glow_alpha * 0.28,
                        ..accent
                    },
                );
                frame.fill(
                    &Path::circle(
                        projected.fixture_point,
                        if is_selected {
                            4.8
                        } else if is_hovered {
                            4.4
                        } else {
                            4.0
                        },
                    ),
                    if is_selected {
                        theme::text_primary()
                    } else {
                        accent
                    },
                );
            }

            if let Some(projected) = group_nodes.first() {
                frame.fill_text(Text {
                    content: format!(
                        "{}  |  {}%  |  {}  |  {} online",
                        group.name,
                        group.output_level / 10,
                        projected.label,
                        group.online
                    ),
                    position: Point::new(
                        projected.fixture_point.x + 10.0,
                        projected.fixture_point.y - 10.0,
                    ),
                    color: if is_selected {
                        theme::text_primary()
                    } else if is_hovered {
                        Color::from_rgba8(228, 235, 242, 0.86)
                    } else {
                        theme::text_muted()
                    },
                    size: Pixels(if is_selected { 12.5 } else { 11.5 }),
                    ..Text::default()
                });
            }
        }
    }

    fn project_nodes(&self, group: &FixtureGroup, size: Size) -> Vec<ProjectedNode> {
        let floor_y = size.height - 24.0;

        group
            .preview_nodes
            .iter()
            .map(|node| {
                let x = 28.0 + (node.x_permille as f32 / 1000.0) * (size.width - 56.0);
                let depth = node.y_permille as f32 / 1000.0;
                let floor_projection = floor_y - depth * 88.0;
                let elevation = 18.0 + (node.z_permille as f32 / 1000.0) * 72.0;
                let fixture_point = Point::new(x, floor_projection - elevation);
                let beam_target = Point::new(x, floor_projection);

                ProjectedNode {
                    label: node.label.clone(),
                    fixture_point,
                    beam_target,
                }
            })
            .collect()
    }

    fn group_bounds(&self, group: &FixtureGroup, size: Size) -> Rectangle {
        let nodes = self.project_nodes(group, size);
        let Some(first) = nodes.first() else {
            return Rectangle {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            };
        };

        let mut left = first.fixture_point.x;
        let mut right = first.fixture_point.x;
        let mut top = first.fixture_point.y;
        let mut bottom = first.beam_target.y + 12.0;

        for node in &nodes {
            left = left.min(node.fixture_point.x - 16.0);
            right = right.max(node.fixture_point.x + 16.0);
            top = top.min(node.fixture_point.y - 16.0);
            bottom = bottom.max(node.beam_target.y + 18.0);
        }

        Rectangle {
            x: (left - 12.0).max(0.0),
            y: (top - 16.0).max(0.0),
            width: (right - left + 132.0).min(size.width),
            height: (bottom - top + 20.0).min(size.height),
        }
    }

    fn group_at_point(&self, point: Point, size: Size) -> Option<FixtureGroupId> {
        self.groups.iter().find_map(|group| {
            self.group_bounds(group, size)
                .contains(point)
                .then_some(group.id)
        })
    }
}

#[derive(Debug, Clone)]
struct ProjectedNode {
    label: String,
    fixture_point: Point,
    beam_target: Point,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::StudioState;

    #[test]
    fn group_hit_test_selects_fixture_from_projected_node_space() {
        let state = StudioState::default();
        let program = FixtureProgram {
            groups: state.fixture_system.groups.clone(),
            selected: None,
        };
        let size = Size::new(280.0, 220.0);
        let projected = program.project_nodes(&program.groups[0], size);
        let point = projected[0].fixture_point;

        assert_eq!(program.group_at_point(point, size), Some(FixtureGroupId(1)));
    }
}
