//! What the controller needs from the OS: the PC's paired Bose headsets, a
//! control channel to one, and whether Windows offers it as an audio output.

use crate::models::Headphones;
use crate::protocol::BmapError;
use crate::transport::Transport;

pub trait Platform: Send {
    /// Bose headsets paired with this PC, connected ones first.
    fn paired_headphones(&mut self) -> Vec<Headphones>;

    /// Open the BMAP control channel, trying `channels` in order.
    fn open(&mut self, address: &str, channels: &[u8]) -> Result<Box<dyn Transport>, BmapError>;

    /// Whether Windows has an active audio output for the headset. This is
    /// the truth for whether the PC can play to it, as opposed to the
    /// control channel merely being open.
    fn pc_audio_available(&mut self, name_hint: &str) -> bool;
}

/// Is an audio endpoint name the headset's? ("Headphones (Bose QC45)")
pub fn endpoint_matches(endpoint: &str, name_hint: &str) -> bool {
    let name = endpoint.to_lowercase();
    let hint = name_hint.trim().to_lowercase();
    name.contains("bose") || name.contains("quietcomfort") || (!hint.is_empty() && name.contains(&hint))
}

#[cfg(windows)]
pub use win::WindowsPlatform;

#[cfg(windows)]
mod win {
    use windows::core::{BOOL, PCWSTR};
    use windows::Win32::Devices::Bluetooth::{
        BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindNextDevice,
        BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS,
    };
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_Get_Device_ID_ListW, CM_Get_Device_ID_List_SizeW, CM_GETIDLIST_FILTER_ENUMERATOR,
        CR_SUCCESS,
    };
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::{
        eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
    };

    use super::{endpoint_matches, Platform};
    use crate::identity::{friendly_name, is_bose, vendor_for_address};
    use crate::models::Headphones;
    use crate::protocol::{mac_to_u64, u64_to_mac, BmapError};
    use crate::transport::{Transport, WinRfcomm};

    #[derive(Default)]
    pub struct WindowsPlatform;

    /// A device paired with this PC (any kind).
    pub struct PairedDevice {
        pub name: String,
        pub address: u64,
        pub connected: bool,
        pub vendor: Option<(u16, u16)>,
    }

    impl PairedDevice {
        pub fn mac(&self) -> String {
            u64_to_mac(self.address)
        }

        pub fn is_bose(&self) -> bool {
            is_bose(&self.name, self.vendor.map(|v| v.1), self.vendor.map(|v| v.0))
        }
    }

    /// Every Bluetooth device paired with this PC, with its Device ID vendor.
    pub fn paired_devices() -> Vec<PairedDevice> {
        let instance_ids = bthenum_instance_ids();
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
        let blank = || BLUETOOTH_DEVICE_INFO {
            dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
            ..Default::default()
        };
        let mut info = blank();
        let mut devices = Vec::new();
        unsafe {
            let Ok(find) = BluetoothFindFirstDevice(&params, &mut info) else {
                return devices;
            };
            loop {
                let len = info.szName.iter().position(|&c| c == 0).unwrap_or(info.szName.len());
                let address = info.Address.Anonymous.ullLong;
                devices.push(PairedDevice {
                    name: String::from_utf16_lossy(&info.szName[..len]),
                    address,
                    connected: info.fConnected.as_bool(),
                    vendor: vendor_for_address(
                        instance_ids.iter().map(String::as_str),
                        &format!("{address:012X}"),
                    ),
                });
                info = blank();
                if BluetoothFindNextDevice(find, &mut info).is_err() {
                    break;
                }
            }
            let _ = BluetoothFindDeviceClose(find);
        }
        devices
    }

    /// PnP instance IDs under the BTHENUM enumerator (they carry `VID&...`).
    fn bthenum_instance_ids() -> Vec<String> {
        let filter: Vec<u16> = "BTHENUM\0".encode_utf16().collect();
        let filter = PCWSTR(filter.as_ptr());
        unsafe {
            let mut len = 0u32;
            if CM_Get_Device_ID_List_SizeW(&mut len, filter, CM_GETIDLIST_FILTER_ENUMERATOR) != CR_SUCCESS {
                return Vec::new();
            }
            let mut buffer = vec![0u16; len as usize];
            if CM_Get_Device_ID_ListW(filter, &mut buffer, CM_GETIDLIST_FILTER_ENUMERATOR) != CR_SUCCESS {
                return Vec::new();
            }
            buffer
                .split(|&c| c == 0)
                .filter(|s| !s.is_empty())
                .map(String::from_utf16_lossy)
                .collect()
        }
    }

    /// Names of Windows' active audio outputs.
    pub fn active_output_names() -> Vec<String> {
        let mut names = Vec::new();
        unsafe {
            // Fine if COM is already initialised on this thread.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let Ok(enumerator) =
                CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
            else {
                return names;
            };
            let Ok(collection) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else {
                return names;
            };
            let count = collection.GetCount().unwrap_or(0);
            for i in 0..count {
                let Ok(device) = collection.Item(i) else { continue };
                let Ok(store) = device.OpenPropertyStore(STGM_READ) else { continue };
                if let Ok(value) = store.GetValue(&PKEY_Device_FriendlyName) {
                    names.push(value.to_string());
                }
            }
        }
        names
    }

    impl Platform for WindowsPlatform {
        fn paired_headphones(&mut self) -> Vec<Headphones> {
            let mut found: Vec<Headphones> = paired_devices()
                .into_iter()
                .filter(PairedDevice::is_bose)
                .map(|d| Headphones { address: d.mac(), name: friendly_name(&d.name), connected: d.connected })
                .collect();
            found.sort_by(|a, b| {
                b.connected.cmp(&a.connected).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            found
        }

        fn open(&mut self, address: &str, channels: &[u8]) -> Result<Box<dyn Transport>, BmapError> {
            let addr = mac_to_u64(address)
                .ok_or_else(|| BmapError::Transport(format!("Not a Bluetooth address: {address}")))?;
            crate::transport::open_first_answering(channels, |channel| {
                WinRfcomm::open(addr, &[channel]).map(|link| Box::new(link) as Box<dyn Transport>)
            })
        }

        fn pc_audio_available(&mut self, name_hint: &str) -> bool {
            active_output_names().iter().any(|n| endpoint_matches(n, name_hint))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_names() {
        assert!(endpoint_matches("Headphones (Bose QC45)", ""));
        assert!(endpoint_matches("Headphones (Studio Cans)", "Studio Cans"));
        assert!(endpoint_matches("Headset (Studio Cans Hands-Free)", "studio cans"));
        assert!(!endpoint_matches("Speakers (Realtek(R) Audio)", "Studio Cans"));
        assert!(!endpoint_matches("Remote Audio", " "));
    }
}
