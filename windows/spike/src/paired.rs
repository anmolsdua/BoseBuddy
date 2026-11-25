//! Bluetooth devices paired with this PC (BluetoothFindFirstDevice).

use windows::Win32::Devices::Bluetooth::{
    BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindNextDevice,
    BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS,
};
use windows::core::BOOL;

pub struct PairedDevice {
    pub name: String,
    pub address: u64,
    pub connected: bool,
}

impl PairedDevice {
    pub fn mac(&self) -> String {
        let b = self.address.to_be_bytes();
        crate::bmap::format_mac(&b[2..8])
    }

    pub fn looks_like_bose(&self) -> bool {
        let lower = self.name.to_lowercase();
        ["bose", "quietcomfort", "qc35", "qc45", "qc ultra"]
            .iter()
            .any(|needle| lower.contains(needle))
    }
}

pub fn list() -> Vec<PairedDevice> {
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: BOOL(1),
        fReturnRemembered: BOOL(1),
        fReturnUnknown: BOOL(0),
        fReturnConnected: BOOL(1),
        fIssueInquiry: BOOL(0),
        cTimeoutMultiplier: 0,
        hRadio: Default::default(),
    };
    let mut info = BLUETOOTH_DEVICE_INFO {
        dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    let mut devices = Vec::new();
    unsafe {
        let Ok(find) = BluetoothFindFirstDevice(&params, &mut info) else {
            return devices;
        };
        loop {
            let len = info.szName.iter().position(|&c| c == 0).unwrap_or(info.szName.len());
            devices.push(PairedDevice {
                name: String::from_utf16_lossy(&info.szName[..len]),
                address: info.Address.Anonymous.ullLong,
                connected: info.fConnected.as_bool(),
            });
            info = BLUETOOTH_DEVICE_INFO {
                dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
                ..Default::default()
            };
            if BluetoothFindNextDevice(find, &mut info).is_err() {
                break;
            }
        }
        let _ = BluetoothFindDeviceClose(find);
    }
    devices
}
