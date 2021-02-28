use std::thread;

use anyhow::Result;

use libratone_rs::device;

fn main() -> Result<()> {
    let device_manager = device::DeviceManager::new(device::DeviceManagerConfig::default()?)?;
    let device_manager_events = device_manager.listen();

    let events_thread = thread::spawn(move || {
        for event in device_manager_events {
            println!("Device manager event: {:?}", &event);

            match event {
                device::DeviceManagerEvent::DeviceDiscovered(device) => {
                    if let Err(err) = device_manager.fetch_info(&device.id()) {
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
