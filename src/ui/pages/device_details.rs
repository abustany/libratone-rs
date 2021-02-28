use druid::{Lens, LensExt, Widget, WidgetExt};
use druid::widget::{Button, Either, Flex, Label, Slider};

use super::super::appstate::{AppState, Device, DeviceMap};
use super::super::commands::ShowDeviceList;
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

    let details = Flex::column()
        .with_flex_child(
            Flex::row()
                .with_flex_child(Label::new("Volume"), 0.0)
                .with_flex_child(volume_slider, 1.0)
                .controller(VolumeController),
            0.0,
        );

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
