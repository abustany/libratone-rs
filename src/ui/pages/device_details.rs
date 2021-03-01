use std::sync::Arc;

use druid::{EventCtx, Lens, LensExt, Widget, WidgetExt};
use druid::widget::{Button, Either, Flex, Label, LabelText, SizedBox, Slider};

use crate::commands::{Command, PlayControl, PlayControlCommand, PlayInfoData};

use super::super::appstate::{AppState, Device, DeviceMap};
use super::super::commands::{SendCommand, ShowDeviceList};
use super::super::controllers::VolumeController;
use super::super::widgets::Page;

pub struct CurrentDeviceLens;

impl Lens<AppState, Device> for CurrentDeviceLens {
    fn with<V, F: FnOnce(&Device) -> V>(&self, data: &AppState, f: F) -> V {
        let current_device_id = data.current_device_id().unwrap();
        let current_device = data.devices.get(&current_device_id).unwrap();
        f(&current_device)
    }

    fn with_mut<V, F: FnOnce(&mut Device) -> V>(&self, data: &mut AppState, f: F) -> V {
        let current_device_id = data.current_device_id().unwrap();
        let mut current_device = data.devices.get(&current_device_id).unwrap().clone();
        let result = f(&mut current_device);

        data.devices.upsert_device(&current_device);

        result
    }
}

fn control_button(label: impl Into<LabelText<Device>>, action: PlayControlCommand) -> impl Widget<Device> {
     Button::new(label)
        .on_click(move |ctx: &mut EventCtx, device: &mut Device, _env| {
            ctx.submit_command(SendCommand::new(
                &device.id,
                PlayControl::set(action),
                |_| {},
            ))
        })
}

pub fn build_device_details() -> impl Widget<AppState> {
    let back_button = Button::new("←")
        .on_click(|ctx, _app_state, _env| ctx.submit_command(ShowDeviceList::new()));

    let volume_slider = Slider::new()
        .with_range(0.0, 100.0)
        .expand_width()
        .lens(
            Device::volume.map(
                |x| x.unwrap_or(0).into(),
                |x, y: f64| {
                    let y = y as u8;

                    if x != &Some(y) {
                        *x = Some(y as u8);
                    }
                },
            )
        );

    let controls = Flex::row()
        .with_flex_spacer(1.0)
        .with_flex_child(control_button("<< Previous", PlayControlCommand::Previous), 0.0)
        .with_default_spacer()
        .with_flex_child(
            control_button(
                |d: &Device, _env: &_| if d.play_status.as_ref().map(|x| *std::sync::Arc::clone(x)) == Some(PlayControlCommand::Play) {
                    "Pause"
                } else if d.play_status.is_some() {
                    "Play"
                } else {
                    "Play/Pause"
                }.to_owned(),
                PlayControlCommand::Toggle,
                ),
                0.0,
                )
        .with_default_spacer()
        .with_flex_child(control_button("Next >>", PlayControlCommand::Next), 0.0)
        .with_flex_spacer(1.0);

    let now_playing = Either::new(
        |d: &Device, _env: &_| d.play_info.is_some(),
        Flex::column()
            .with_flex_child(Label::new("Now playing:"), 0.0)
            .with_flex_child(Label::dynamic(|d: &Arc<PlayInfoData>, _| {
                d.play_title.as_ref().map(|x| x.to_owned()).unwrap_or_else(|| String::new())
            }), 0.0)
            .with_flex_child(Label::dynamic(|d: &Arc<PlayInfoData>, _| {
                d.play_subtitle.as_ref().map(|x| x.to_owned()).unwrap_or_else(|| String::new())
            }), 0.0)
            .with_default_spacer()
            .expand_width()
            .lens(Device::play_info.map(|x| x.clone().unwrap(), |_x, _y| {})),
        SizedBox::empty(),
    );

    let details = Flex::column()
        .with_flex_child(
            Flex::row()
                .with_flex_child(Label::new("Volume"), 0.0)
                .with_flex_child(volume_slider, 1.0)
                .controller(VolumeController),
            0.0,
        )
        .with_flex_spacer(1.0)
        .with_flex_child(
            Flex::row().with_flex_child(now_playing, 1.0),
            0.0,
        )
        .with_flex_child(controls, 0.0);

    let page = Page::new(
        |data: &Device| data.name.as_ref().unwrap_or_else(|| &data.id).to_owned(),
        details,
        Some(Box::new(back_button)),
    );

    Either::new(
        |data: &AppState, _env: &_| data.current_device_id().is_some(),
        page.lens(CurrentDeviceLens),
        Label::new(""),
    )
}
