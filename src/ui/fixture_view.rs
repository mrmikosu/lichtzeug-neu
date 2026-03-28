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
    patches: Vec<StagePatchChip>,
    selected_patch: Option<u32>,
}

#[derive(Debug, Clone)]
struct StagePatchChip {
    patch_id: u32,
    group_id: FixtureGroupId,
    name: String,
    universe: u16,
    address: u16,
    footprint: u16,
    conflicting: bool,
    enabled: bool,
}

pub fn view(state: &StudioState) -> Element<'_, AppEvent> {
    let mut patches = state
        .fixture_system
        .library
        .patches
        .iter()
        .filter_map(|patch| {
            let group_id = patch.group_id?;
            Some(StagePatchChip {
                patch_id: patch.id,
                group_id,
                name: patch.name.clone(),
                universe: patch.universe,
                address: patch.address,
                footprint: state.fixture_patch_channel_count(patch).unwrap_or(0),
                conflicting: !state.fixture_patch_conflicts(patch.id).is_empty(),
                enabled: patch.enabled,
            })
        })
        .collect::<Vec<_>>();
    patches.sort_by_key(|patch| {
        (
            patch.group_id.0,
            patch.universe,
            patch.address,
            patch.patch_id,
        )
    });

    container(Canvas::new(FixtureProgram {
        groups: state.fixture_system.groups.clone(),
        selected: state.fixture_system.selected,
        patches,
        selected_patch: state.fixture_system.library.selected_patch,
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

                if let Some(patch_id) = self.patch_at_point(position, bounds.size()) {
                    return (
                        canvas::event::Status::Captured,
                        Some(AppEvent::SelectFixturePatch(patch_id)),
                    );
                }

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
        let hovered_patch = cursor
            .position_in(bounds)
            .and_then(|position| self.patch_at_point(position, bounds.size()));
        let hovered = cursor
            .position_in(bounds)
            .and_then(|position| self.group_at_point(position, bounds.size()));
        self.draw_background(&mut frame, bounds.size());
        self.draw_groups(
            &mut frame,
            bounds.size(),
            hovered.or_else(|| hovered_patch.and_then(|patch_id| self.patch_group(patch_id))),
        );
        self.draw_patches(&mut frame, bounds.size(), hovered_patch);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match cursor.position_in(bounds) {
            Some(position) if self.patch_at_point(position, bounds.size()).is_some() => {
                mouse::Interaction::Pointer
            }
            Some(position) if self.group_at_point(position, bounds.size()).is_some() => {
                mouse::Interaction::Pointer
            }
            _ => mouse::Interaction::default(),
        }
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
                let group_patches = self
                    .patches
                    .iter()
                    .filter(|patch| patch.group_id == group.id)
                    .collect::<Vec<_>>();
                let mut patch_universes = group_patches
                    .iter()
                    .map(|patch| format!("U{}", patch.universe))
                    .collect::<Vec<_>>();
                patch_universes.sort();
                patch_universes.dedup();
                frame.fill_text(Text {
                    content: format!(
                        "{}  |  {}%  |  {} online  |  {} patch(es)  |  {}",
                        group.name,
                        group.output_level / 10,
                        group.online,
                        group_patches.len(),
                        if patch_universes.is_empty() {
                            projected.label.clone()
                        } else {
                            patch_universes.join(", ")
                        }
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

    fn draw_patches(
        &self,
        frame: &mut canvas::Frame<Renderer>,
        size: Size,
        hovered_patch: Option<u32>,
    ) {
        for placed in self.layout_patches(size) {
            let is_selected = self.selected_patch == Some(placed.patch_id);
            let is_hovered = hovered_patch == Some(placed.patch_id);
            let fill = if placed.conflicting {
                Color::from_rgba8(110, 45, 38, 0.92)
            } else if placed.enabled {
                Color::from_rgba8(29, 44, 58, 0.94)
            } else {
                Color::from_rgba8(52, 58, 68, 0.9)
            };
            let accent = if placed.conflicting {
                theme::warning()
            } else if is_selected {
                theme::accent_blue()
            } else {
                theme::border_soft()
            };
            let rect = Path::rounded_rectangle(
                placed.bounds.position(),
                placed.bounds.size(),
                iced::border::Radius::new(8.0),
            );

            frame.fill(&rect, fill);
            frame.stroke(
                &rect,
                Stroke::default()
                    .with_color(Color {
                        a: if is_hovered || is_selected {
                            0.95
                        } else {
                            0.62
                        },
                        ..accent
                    })
                    .with_width(if is_selected { 1.6 } else { 1.0 }),
            );

            frame.fill_text(Text {
                content: placed.name.clone(),
                position: Point::new(placed.bounds.x + 8.0, placed.bounds.y + 11.0),
                color: theme::text_primary(),
                size: Pixels(10.5),
                ..Text::default()
            });
            frame.fill_text(Text {
                content: format!(
                    "U{}.{}  |  {}ch{}",
                    placed.universe,
                    placed.address,
                    placed.footprint,
                    if placed.conflicting { "  overlap" } else { "" }
                ),
                position: Point::new(placed.bounds.x + 8.0, placed.bounds.y + 23.0),
                color: if placed.conflicting {
                    theme::warning()
                } else {
                    theme::text_muted()
                },
                size: Pixels(9.0),
                ..Text::default()
            });
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

    fn patch_group(&self, patch_id: u32) -> Option<FixtureGroupId> {
        self.patches
            .iter()
            .find(|patch| patch.patch_id == patch_id)
            .map(|patch| patch.group_id)
    }

    fn patch_at_point(&self, point: Point, size: Size) -> Option<u32> {
        self.layout_patches(size)
            .into_iter()
            .rev()
            .find(|placed| placed.bounds.contains(point))
            .map(|placed| placed.patch_id)
    }

    fn layout_patches(&self, size: Size) -> Vec<PlacedPatchChip> {
        let mut laid_out = Vec::new();

        for group in &self.groups {
            let group_nodes = self.project_nodes(group, size);
            if group_nodes.is_empty() {
                continue;
            }

            let group_patches = self
                .patches
                .iter()
                .filter(|patch| patch.group_id == group.id)
                .collect::<Vec<_>>();
            let node_count = group_nodes.len().max(1);

            for (index, patch) in group_patches.into_iter().enumerate() {
                let node_index = index % node_count;
                let stack = index / node_count;
                let node = &group_nodes[node_index];
                let width = 94.0f32.max(62.0 + patch.name.len() as f32 * 4.2);
                let height = 30.0;
                let x = (node.fixture_point.x - width * 0.5 + ((node_index % 2) as f32 * 10.0)
                    - 5.0)
                    .clamp(8.0, size.width - width - 8.0);
                let y = (node.fixture_point.y + 18.0 + stack as f32 * 34.0)
                    .clamp(44.0, size.height - height - 10.0);

                laid_out.push(PlacedPatchChip {
                    patch_id: patch.patch_id,
                    name: truncate_label(&patch.name, 18),
                    universe: patch.universe,
                    address: patch.address,
                    footprint: patch.footprint,
                    conflicting: patch.conflicting,
                    enabled: patch.enabled,
                    bounds: Rectangle {
                        x,
                        y,
                        width,
                        height,
                    },
                });
            }
        }

        laid_out
    }
}

#[derive(Debug, Clone)]
struct ProjectedNode {
    label: String,
    fixture_point: Point,
    beam_target: Point,
}

#[derive(Debug, Clone)]
struct PlacedPatchChip {
    patch_id: u32,
    name: String,
    universe: u16,
    address: u16,
    footprint: u16,
    conflicting: bool,
    enabled: bool,
    bounds: Rectangle,
}

fn truncate_label(label: &str, max_len: usize) -> String {
    let mut chars = label.chars();
    let preview = chars.by_ref().take(max_len).collect::<String>();
    if chars.next().is_some() {
        format!("{}...", preview)
    } else {
        preview
    }
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
            patches: Vec::new(),
            selected_patch: None,
        };
        let size = Size::new(280.0, 220.0);
        let projected = program.project_nodes(&program.groups[0], size);
        let point = projected[0].fixture_point;

        assert_eq!(program.group_at_point(point, size), Some(FixtureGroupId(1)));
    }

    #[test]
    fn patch_hit_test_prioritizes_fixture_patch_chip() {
        let mut state = StudioState::default();
        let profile = crate::core::import_ofl_fixture(
            r#"{
              "$schema":"https://raw.githubusercontent.com/OpenLightingProject/open-fixture-library/master/schemas/fixture.json",
              "name":"Stage Spot",
              "categories":["Spot"],
              "meta":{"authors":["Tester"],"createDate":"2024-01-01","lastModifyDate":"2024-01-02"},
              "availableChannels":{
                "Dimmer":{"capability":{"type":"Intensity"}},
                "Red":{"capability":{"type":"ColorIntensity","color":"Red"}}
              },
              "modes":[{"name":"2ch","channels":["Dimmer","Red"]}]
            }"#,
            Some("demo"),
            Some("stage-spot"),
        )
        .expect("fixture profile");
        state.fixture_system.library.profiles.push(profile);
        state
            .fixture_system
            .library
            .patches
            .push(crate::core::FixturePatch {
                id: 7,
                profile_id: "demo/stage-spot".to_owned(),
                name: "Stage Spot 1".to_owned(),
                mode_name: "2ch".to_owned(),
                universe: 1,
                address: 12,
                group_id: Some(FixtureGroupId(1)),
                enabled: true,
            });

        let program = FixtureProgram {
            groups: state.fixture_system.groups.clone(),
            selected: Some(FixtureGroupId(1)),
            patches: vec![StagePatchChip {
                patch_id: 7,
                group_id: FixtureGroupId(1),
                name: "Stage Spot 1".to_owned(),
                universe: 1,
                address: 12,
                footprint: 2,
                conflicting: false,
                enabled: true,
            }],
            selected_patch: Some(7),
        };
        let size = Size::new(280.0, 220.0);
        let patch = program
            .layout_patches(size)
            .into_iter()
            .next()
            .expect("patch layout");
        let point = Point::new(patch.bounds.x + 4.0, patch.bounds.y + 4.0);

        assert_eq!(program.patch_at_point(point, size), Some(7));
    }
}
