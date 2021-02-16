use std::thread;

use anyhow::Result;

mod commands;
mod device;
mod discovery_reply;
mod protocol;

fn main() -> Result<()> {
    let device_manager = device::DeviceManager::new()?;
    let device_manager_events = device_manager.listen();

    let events_thread = thread::spawn(move || {
        for event in device_manager_events {
            println!("Device manager event: {:?}", &event);

            match event {
                device::DeviceManagerEvent::DeviceDiscovered(device_id) => {
                    if let Err(err) = device_manager.fetch_info(&device_id) {
                        println!("error fetching device info: {}", err);
                    };
                }
                device::DeviceManagerEvent::DeviceUpdated(device) => {
                    println!("Device update: {:?}", device);
                }
            }
        }
    });

    events_thread.join().unwrap();
    Ok(())
}
