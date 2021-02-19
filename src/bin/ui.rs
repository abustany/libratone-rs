use druid::{AppLauncher, PlatformError, Target, Widget, WindowDesc};
use druid::im::hashmap;
use druid::widget::ViewSwitcher;

use libratone_rs::device;
use libratone_rs::ui::appstate::{AppState, Device, Route};
use libratone_rs::ui::delegate::Delegate;
use libratone_rs::ui::commands;
use libratone_rs::fake::DeviceManager;
use libratone_rs::ui::pages::device_details::*;
use libratone_rs::ui::pages::device_list::*;


fn build_ui() -> impl Widget<AppState> {
    ViewSwitcher::new(
        |data: &AppState, _env| data.route.clone(),
        |route, _data, _env| match route {
            Route::DeviceList => Box::new(build_device_list()),
            Route::DeviceDetails(_device_id) => Box::new(build_device_details()),
        }
    )
}

fn main() -> Result<(), PlatformError> {
    let mock_state = AppState{
        route: Route::DeviceList,
        devices: hashmap![
            "device-1".to_string() => Device{
                id: "device-1".to_string(),
                ip_addr: "10.0.0.1".to_string(),
                name: None,
                volume: None,
            },
            "device-2".to_string() => Device{
                id: "device-2".to_string(),
                ip_addr: "10.0.0.2".to_string(),
                name: Some("this one has a name".to_string()),
                volume: Some(33),
            },
        ],
    };

    let device_manager = DeviceManager::new()?;
    let device_manager_events = device_manager.listen();

    let window = WindowDesc::new(build_ui)
        .title("Libratone");

    let app = AppLauncher::with_window(window)
        .use_simple_logger()
        .delegate(Delegate{});

    let event_sink = app.get_external_handle();

    std::thread::spawn(move || {
        for event in device_manager_events {
            println!("Device manager event: {:?}", &event);

            match event {
                device::DeviceManagerEvent::DeviceDiscovered(device) => {
                    println!("device {} discovered at {}, fetching info", device.id(), device.addr());

                    if let Err(err) = device_manager.fetch_info(&device.id()) {
                        println!("error fetching device info: {}", err);
                    };

                    event_sink.submit_command(commands::DeviceUpdated::SELECTOR, Box::new(device.into()), Target::Auto).expect("error sending event to sink");
                }
                device::DeviceManagerEvent::DeviceUpdated(device) => {
                    println!("Device update: {:?}", device);
                    event_sink.submit_command(commands::DeviceUpdated::SELECTOR, Box::new(device.into()), Target::Auto).expect("error sending event to sink");
                }
            }
        }
    });

    app.launch(mock_state)?;

    Ok(())
}
