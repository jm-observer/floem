use floem::{
    peniko::color::palette,
    peniko::Color,
    style::CursorStyle,
    views::{button, toggle_button, Decorators, ToggleHandleBehavior},
    IntoView,
};

use crate::form::{form, form_item};

pub fn button_view() -> impl IntoView {
    form((
        form_item(
            "Basic Button:",
            button("Click me").action(|| println!("Button clicked")),
        ),
        form_item(
            "Styled Button:",
            button("Click me")
                .action(|| println!("Button clicked"))
                .style(|s| {
                    s.border(1.0)
                        .border_radius(10.0)
                        .padding(10.0)
                        .background(palette::css::YELLOW_GREEN)
                        .color(palette::css::DARK_GREEN)
                        .cursor(CursorStyle::Pointer)
                        .active(|s| s.color(palette::css::WHITE).background(palette::css::RED))
                        .hover(|s| s.background(Color::from_rgb8(244, 67, 54)))
                        .focus_visible(|s| s.border(2.).border_color(palette::css::BLUE))
                }),
        ),
        form_item(
            "Disabled Button:",
            button("Click me")
                .disabled(|| true)
                .action(|| println!("Button clicked")),
        ),
        form_item(
            "Secondary click button:",
            button("Right click me").on_secondary_click_stop(|_| {
                println!("Secondary mouse button click.");
            }),
        ),
        form_item(
            "Toggle button - Snap:",
            toggle_button(|| true)
                .on_toggle(|_| {
                    println!("Button Toggled");
                })
                .toggle_style(|s| s.behavior(ToggleHandleBehavior::Snap)),
        ),
        form_item(
            "Toggle button - Follow:",
            toggle_button(|| true)
                .on_toggle(|_| {
                    println!("Button Toggled");
                })
                .toggle_style(|s| s.behavior(ToggleHandleBehavior::Follow)),
        ),
    ))
}
