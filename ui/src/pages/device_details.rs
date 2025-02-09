use std::sync::Arc;

use druid::im::Vector;
use druid::widget::{
    Button, Either, Flex, Label, LabelText, LineBreaking, List, Scroll, SizedBox, Slider,
};
use druid::{lens, EventCtx, Lens, LensExt, Widget, WidgetExt};

use crate::appstate::{AppState, Device, DeviceMap, PreChannel};
use crate::commands::{SendCommand, ShowDeviceList};
use crate::controllers::VolumeController;
use crate::widgets;
use libratone_rs::commands::{Command, PlayControl, PlayControlCommand, PlayInfo, PlayInfoData};

pub struct CurrentDeviceLens;

impl Lens<AppState, Device> for CurrentDeviceLens {
    fn with<V, F: FnOnce(&Device) -> V>(&self, data: &AppState, f: F) -> V {
        let current_device_id = data.current_device_id().unwrap();
        let current_device = data.devices.get(&current_device_id).unwrap();
        f(current_device)
    }

    fn with_mut<V, F: FnOnce(&mut Device) -> V>(&self, data: &mut AppState, f: F) -> V {
        let current_device_id = data.current_device_id().unwrap();
        let mut current_device = data.devices.get(&current_device_id).unwrap().clone();
        let result = f(&mut current_device);

        data.devices.upsert_device(&current_device);

        result
    }
}

fn control_button(
    label: impl Into<LabelText<Device>>,
    action: PlayControlCommand,
) -> impl Widget<Device> {
    Button::new(label).on_click(move |ctx: &mut EventCtx, device: &mut Device, _env| {
        ctx.submit_command(SendCommand::command(
            &device.id,
            PlayControl::set(action),
            |_| {},
        ))
    })
}

fn favorite_item() -> impl Widget<(Device, PreChannel)> {
    Flex::row().with_flex_child(
        Button::new(|(_, ch): &(Device, PreChannel), _env: &_| ch.name.clone()).on_click(
            |ctx, (device, ch), _env| {
                ctx.submit_command(SendCommand::command(
                    &device.id,
                    PlayInfo::set(ch.channel.play_info_data()),
                    |_| {},
                ))
            },
        ),
        1.0,
    )
}

pub fn build_device_details() -> impl Widget<AppState> {
    let back_button = Button::new("←")
        .on_click(|ctx, _app_state, _env| ctx.submit_command(ShowDeviceList::command()));

    let volume_slider =
        Slider::new()
            .with_range(0.0, 100.0)
            .expand_width()
            .lens(Device::volume.map(
                |x| x.unwrap_or(0).into(),
                |x, y: f64| {
                    let y = y as u8;

                    if x != &Some(y) {
                        *x = Some(y as u8);
                    }
                },
            ));

    let pre_channels = Either::new(
        |d: &Device, _env: &_| !d.pre_channels.is_empty(),
        Flex::column()
            .with_child(Label::new("Favorites:").expand_width())
            .with_flex_child(
                Scroll::new(List::new(favorite_item).lens(lens::Identity.map(
                    |d: &Device| (d.clone(), d.pre_channels.clone()),
                    |_d: &mut Device, _x: (Device, Vector<PreChannel>)| {},
                )))
                .vertical(),
                1.0,
            )
            .expand_width(),
        SizedBox::empty(),
    );

    let controls = Flex::row()
        .with_flex_spacer(1.0)
        .with_child(control_button("<< Previous", PlayControlCommand::Previous))
        .with_default_spacer()
        .with_child(control_button(
            |d: &Device, _env: &_| {
                if d.play_status.as_ref().map(|x| *std::sync::Arc::clone(x))
                    == Some(PlayControlCommand::Play)
                {
                    "Pause"
                } else if d.play_status.is_some() {
                    "Play"
                } else {
                    "Play/Pause"
                }
                .to_owned()
            },
            PlayControlCommand::Toggle,
        ))
        .with_default_spacer()
        .with_child(control_button("Next >>", PlayControlCommand::Next))
        .with_flex_spacer(1.0);

    let now_playing = Either::new(
        |d: &Device, _env: &_| {
            d.play_info
                .as_ref()
                .and_then(|x| x.play_title.as_ref())
                .map(|x| x.len())
                .unwrap_or(0)
                > 0
        },
        Flex::column()
            .with_child(Label::new("Now playing:"))
            .with_child(
                Label::dynamic(|d: &Arc<PlayInfoData>, _| {
                    d.play_title
                        .as_ref()
                        .map(|x| x.to_owned())
                        .unwrap_or_default()
                })
                .with_line_break_mode(LineBreaking::WordWrap),
            )
            .with_child(
                Label::dynamic(|d: &Arc<PlayInfoData>, _| {
                    d.play_subtitle
                        .as_ref()
                        .map(|x| x.to_owned())
                        .unwrap_or_default()
                })
                .with_line_break_mode(LineBreaking::WordWrap),
            )
            .with_default_spacer()
            .expand_width()
            .lens(Device::play_info.map(|x| x.clone().unwrap(), |_x, _y| {})),
        SizedBox::empty(),
    );

    let details = Flex::column()
        .with_child(
            Flex::row()
                .with_child(Label::new("Volume"))
                .with_flex_child(volume_slider, 1.0)
                .controller(VolumeController),
        )
        .with_default_spacer()
        .with_flex_child(Flex::row().with_flex_child(pre_channels, 1.0), 1.0)
        .with_default_spacer()
        .with_child(Flex::row().with_flex_child(now_playing, 1.0))
        .with_child(controls);

    let page = widgets::page(
        |data: &Device| data.name.as_ref().unwrap_or(&data.id).to_owned(),
        details,
        Some(Box::new(back_button)),
    );

    Either::new(
        |data: &AppState, _env: &_| data.current_device_id().is_some(),
        page.lens(CurrentDeviceLens),
        Label::new(""),
    )
}
