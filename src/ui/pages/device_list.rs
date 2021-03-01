use druid::im::Vector;
use druid::widget::{Flex, Label, List, Scroll, WidgetExt};
use druid::{LensExt, Widget};

use crate::ui::appstate::{AppState, Device};
use crate::ui::commands::ShowDeviceDetails;
use crate::ui::widgets::Page;

fn device_item() -> impl Widget<Device> {
    let device_name =
        Label::new(|d: &Device, _env: &_| d.name.as_ref().unwrap_or_else(|| &d.id).to_owned())
            .on_click(|ctx, device, _env| {
                ctx.submit_command(ShowDeviceDetails::new(&device.id));
            });
    Flex::row().with_flex_child(device_name, 1.0)
}

pub fn build_device_list() -> impl Widget<AppState> {
    Page::new(
        |_data| "Device list".to_owned(),
        Flex::column().with_flex_child(
            Scroll::new(List::new(device_item).lens(AppState::devices.map(
                |x| {
                    let res: Vector<Device> = x.values().map(|x| x.clone()).collect();
                    res
                },
                |_x, _y| {},
            )))
            .vertical(),
            1.0,
        ),
        None,
    )
}
