use std::sync::Arc;

use druid::im::HashMap;
use druid::widget::ViewSwitcher;
use druid::{AppLauncher, PlatformError, Target, Widget, WindowDesc};

use libratone_rs::device;
use libratone_rs::device::{DeviceManager, DeviceManagerConfig};
use libratone_rs_ui::appstate::{AppState, Route};
use libratone_rs_ui::commands;
use libratone_rs_ui::delegate::Delegate;
use libratone_rs_ui::pages::device_details::*;
use libratone_rs_ui::pages::device_list::*;

fn build_ui() -> impl Widget<AppState> {
    ViewSwitcher::new(
        |data: &AppState, _env| data.route.clone(),
        |route, _data, _env| match route {
            Route::DeviceList => Box::new(build_device_list()),
            Route::DeviceDetails(_device_id) => Box::new(build_device_details()),
        },
    )
}

fn main() -> Result<(), PlatformError> {
    let mock_state = AppState {
        route: Route::DeviceList,
        devices: HashMap::new(),
    };

    let device_manager = Arc::new(DeviceManager::new(DeviceManagerConfig::default()?)?);
    let device_manager_events = device_manager.listen();

    let window = WindowDesc::new(build_ui()).title("Libratone");

    let app = AppLauncher::with_window(window)
        .log_to_console()
        .delegate(Delegate {
            device_manager: Arc::clone(&device_manager),
        });

    let event_sink = app.get_external_handle();

    std::thread::spawn(move || {
        for event in device_manager_events {
            println!("Device manager event: {:?}", &event);

            match event {
                device::DeviceManagerEvent::DeviceDiscovered(device) => {
                    println!(
                        "device {} discovered at {}, fetching info",
                        device.id(),
                        device.addr()
                    );

                    if let Err(err) = device_manager.fetch_info(&device.id()) {
                        println!("error fetching device info: {}", err);
                    };

                    event_sink
                        .submit_command(
                            commands::DeviceUpdated::SELECTOR,
                            Box::new(device.into()),
                            Target::Auto,
                        )
                        .expect("error sending event to sink");
                }
                device::DeviceManagerEvent::DeviceUpdated(device) => {
                    println!("Device update: {:?}", device);
                    event_sink
                        .submit_command(
                            commands::DeviceUpdated::SELECTOR,
                            Box::new(device.into()),
                            Target::Auto,
                        )
                        .expect("error sending event to sink");
                }
            }
        }
    });

    app.launch(mock_state)?;

    Ok(())
}
