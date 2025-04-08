use btleplug::api::Peripheral as Device;
use btleplug::api::{Central, Characteristic, Manager as _, ScanFilter, WriteType};
use btleplug::platform::Manager;
use std::env;
use tokio::time::{Duration, sleep};

async fn read_desk_height(device: &impl Device, char: &Characteristic) -> u32 {
    let rs_bytes = device.read(char).await;
    let Ok(bytes) = rs_bytes else { return 0 };
    if bytes.len() < 2 {
        return 0;
    }
    let bytes = [bytes[0], bytes[1]];
    let value = u16::from_le_bytes(bytes);
    (600 + value / 10).into()
}

async fn move_desk_to(device: &impl Device, char: &Characteristic, dir: [u8; 2]) {
    let pin = device.write(char, &dir, WriteType::WithoutResponse);
    pin.await.unwrap_or_default();
}

async fn move_desk_to_target(device: &impl Device, target: u32) {
    const DESK_MARGIN: u32 = 10;
    const MOVE_UP: [u8; 2] = [0x47, 0x00];
    const MOVE_DOWN: [u8; 2] = [0x46, 0x00];
    const MOVE_STOP: [u8; 2] = [0x00, 0x00];
    const READ_CHAR_UUID: &str = "99fa0021-338a-1024-8a49-009c0215f78a";
    const MOVE_CHAR_UUID: &str = "99fa0002-338a-1024-8a49-009c0215f78a";

    let chars = device.characteristics();
    let op_read = chars.iter().find(|p| p.uuid.to_string() == READ_CHAR_UUID);
    let op_move = chars.iter().find(|p| p.uuid.to_string() == MOVE_CHAR_UUID);
    let Some(read_char) = op_read else { return };
    let Some(move_char) = op_move else { return };

    let mut elapsed = 0;
    let mut current = read_desk_height(device, read_char).await;

    println!("Moving...");
    println!("→ {current} → {target}\n");
    if current.abs_diff(target) < DESK_MARGIN {
        return;
    }

    let move_desk = async |dir: [u8; 2], arrow: &str, current: u32| -> (u32, u32) {
        print!("\x1B[1A\x1B[2K");
        println!("{} {current} → {target}", arrow);
        move_desk_to(device, move_char, dir).await;
        let new_current = read_desk_height(device, read_char).await;
        let elapsed = new_current.abs_diff(current);
        (new_current, elapsed)
    };

    if current < target {
        while current + elapsed < target {
            (current, elapsed) = move_desk(MOVE_UP, "↑", current).await;
            sleep(Duration::from_millis(50)).await;
        }
    } else {
        while current - elapsed > target {
            (current, elapsed) = move_desk(MOVE_DOWN, "↓", current).await;
            sleep(Duration::from_millis(50)).await;
        }
    }

    move_desk_to(device, move_char, MOVE_STOP).await;
    println!("→ {}", read_desk_height(device, read_char).await);
}

async fn connect_and_move_desk_to_target(target: u32, mac: &str) {
    println!("Scanning...");
    let rs_manager = Manager::new().await;
    let Ok(manager) = rs_manager else { return };
    let adapters = manager.adapters().await;
    let op_adapter = adapters.unwrap_or_default().into_iter().next();
    let Some(adapter) = op_adapter else { return };
    if adapter.start_scan(ScanFilter::default()).await.is_err() {
        println!("Failed to scan!");
        return;
    }

    for _ in 0..10 {
        let rs_devices = adapter.peripherals().await;
        let Ok(devices) = rs_devices else { return };
        let op_device = devices.iter().find(|p| p.address().to_string() == mac);
        let Some(device) = op_device else {
            sleep(Duration::from_millis(200)).await;
            continue;
        };

        println!("Connecting...");
        if device.connect().await.is_err() {
            println!("Failed to connect!");
            return;
        }
        if device.discover_services().await.is_err() {
            println!("Failed to discover services!");
            return;
        }

        move_desk_to_target(device, target).await;
        println!("Disconnecting...");
        if device.disconnect().await.is_err() {
            println!("Failed to disconnect!");
            return;
        }
        return;
    }
    println!("Could not find device [{}]", mac);
}

async fn scan_and_print_devices() {
    let rs_manager = Manager::new().await;
    let Ok(manager) = rs_manager else { return };
    let adapters = manager.adapters().await;
    let op_adapter = adapters.unwrap_or_default().into_iter().next();
    let Some(adapter) = op_adapter else { return };
    if adapter.start_scan(ScanFilter::default()).await.is_err() {
        println!("Failed to scan!");
        return;
    }
    sleep(Duration::from_millis(2000)).await;
    let rs_devices = adapter.peripherals().await;
    let Ok(devices) = rs_devices else { return };
    for device in devices {
        println!("→ {}", device);
    }
}

#[tokio::main]
async fn main() {
    use std::io::Write;
    let mut input = String::new();

    let args = env::args().collect::<Vec<String>>();
    let mac = if 1 < args.len() {
        &args[1].trim()
    } else {
        println!("No mac address provided, scanning...");
        scan_and_print_devices().await;
        return;
    };

    println!("Target device [{}]", mac);
    print!("Move Desk Height (820 - 1250 mm): ");
    let _ = std::io::stdout().flush();
    let _ = std::io::stdin().read_line(&mut input);

    let rs_target = input.trim().parse::<u32>();
    let Ok(target) = rs_target else { return };

    const DESK_MIN: u32 = 820;
    const DESK_MAX: u32 = 1250;
    if (DESK_MIN..=DESK_MAX).contains(&target) {
        connect_and_move_desk_to_target(target - 20, mac).await;
    } else {
        println!("Expected value between 820 and 1250!");
    }
}
