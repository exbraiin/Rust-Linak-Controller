use btleplug::api::{Central, Characteristic, Manager as _, Peripheral, ScanFilter, WriteType};
use btleplug::platform::Manager;
use std::env;
use tokio::time::{Duration, sleep};

async fn read_height(peripheral: &impl Peripheral, char: &Characteristic) -> u32 {
    let rs_bytes = peripheral.read(char).await;
    let Ok(bytes) = rs_bytes else { return 0 };
    if bytes.len() < 2 {
        return 0;
    }
    let bytes = [bytes[0], bytes[1]];
    let value = u16::from_le_bytes(bytes);
    (600 + value / 10).into()
}

async fn write_direction(peripheral: &impl Peripheral, char: &Characteristic, dir: [u8; 2]) {
    let pin = peripheral.write(char, &dir, WriteType::WithoutResponse);
    pin.await.unwrap_or_default();
}

async fn move_desk_to(peripheral: &impl Peripheral, target: u32) {
    const DESK_MARGIN: u32 = 10;
    const MOVE_UP: [u8; 2] = [0x47, 0x00];
    const MOVE_DOWN: [u8; 2] = [0x46, 0x00];
    const MOVE_STOP: [u8; 2] = [0x00, 0x00];
    const READ_CHAR_UUID: &str = "99fa0021-338a-1024-8a49-009c0215f78a";
    const MOVE_CHAR_UUID: &str = "99fa0002-338a-1024-8a49-009c0215f78a";

    let chars = peripheral.characteristics();
    let op_read = chars.iter().find(|p| p.uuid.to_string() == READ_CHAR_UUID);
    let op_move = chars.iter().find(|p| p.uuid.to_string() == MOVE_CHAR_UUID);
    let Some(read_char) = op_read else { return };
    let Some(move_char) = op_move else { return };

    let mut elapsed = 0;
    let mut current = read_height(peripheral, read_char).await;

    println!("Moving...");
    println!("→ {current} → {target}\n");
    if current.abs_diff(target) < DESK_MARGIN {
        return;
    }

    let move_desk = async |dir: [u8; 2], arrow: &str, current: u32| -> (u32, u32) {
        print!("\x1B[1A\x1B[2K");
        println!("{} {current} → {target}", arrow);
        write_direction(peripheral, move_char, dir).await;
        let new_current = read_height(peripheral, read_char).await;
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

    write_direction(peripheral, move_char, MOVE_STOP).await;
    println!("→ {}", read_height(peripheral, read_char).await);
}

async fn scan_peripherals() {
    let manager = Manager::new().await.unwrap();
    let adapters = manager.adapters().await;
    let adapter = adapters.unwrap().into_iter().next().unwrap();
    adapter.start_scan(ScanFilter::default()).await.unwrap();
    sleep(Duration::from_millis(2000)).await;
    let rs_periphs = adapter.peripherals().await;
    let Ok(periphs) = rs_periphs else { return };
    for p in periphs {
        println!("→ {}", p);
    }
}

async fn connect_and_move_desk_to(target: u32, mac: &str) {
    println!("Scanning...");
    let manager = Manager::new().await.unwrap();
    let adapters = manager.adapters().await;
    let adapter = adapters.unwrap().into_iter().next().unwrap();
    adapter.start_scan(ScanFilter::default()).await.unwrap();

    for _ in 0..10 {
        let periphs = adapter.peripherals().await.unwrap();
        let op_periph = periphs.iter().find(|p| p.address().to_string() == mac);
        let Some(periph) = op_periph else {
            sleep(Duration::from_millis(200)).await;
            continue;
        };

        println!("Connecting...");
        let _ = periph.connect().await;
        let _ = periph.discover_services().await;

        move_desk_to(periph, target).await;
        println!("Disconnecting...");
        let _ = periph.disconnect().await;
        return;
    }

    println!("Could not find device [{}]", mac);
}

#[tokio::main]
async fn main() {
    use std::io::{Write, stdin, stdout};
    let mut input = String::new();

    let args = env::args().collect::<Vec<String>>();
    let mac = if 1 < args.len() {
        &args[1].trim()
    } else {
        println!("No mac address provided, scanning...");
        scan_peripherals().await;
        return;
    };

    println!("Target device [{}]", mac);
    print!("Move Desk Height (820 - 1250 mm): ");
    let _ = stdout().flush();
    stdin().read_line(&mut input).unwrap();

    let rs_target = input.trim().parse::<u32>();
    let Ok(target) = rs_target else { return };

    if (820..=1250).contains(&target) {
        connect_and_move_desk_to(target - 20, mac).await;
    } else {
        println!("Expected value between 820 and 1250!");
    }
}
