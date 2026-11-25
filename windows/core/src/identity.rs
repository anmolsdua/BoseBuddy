//! Is a paired Bluetooth device a Bose headset? (port of DeviceIdentity.swift)
//!
//! Windows exposes the Device ID profile in the device's PnP instance IDs,
//! e.g. `BTHENUM\{0000110B-...}_VID&0001009E_PID&4039\7&38D62C91&0&782B645B71EE_C00000000`:
//! `0001` is the vendor ID source (1 = Bluetooth SIG company IDs) and `009E`
//! is Bose. That lets renamed headsets be found.

pub const BOSE_VENDOR_ID: u16 = 0x009E;

pub fn is_bose(name: &str, vendor_id: Option<u16>, vendor_source: Option<u16>) -> bool {
    // Source 1 uses Bluetooth SIG company identifiers, not USB IDs.
    if vendor_source == Some(1) && vendor_id == Some(BOSE_VENDOR_ID) {
        return true;
    }
    let lower = name.to_lowercase();
    ["bose", "quietcomfort", "qc35", "qc45", "qc ultra"].iter().any(|n| lower.contains(n))
}

/// `(source, vendor)` from an instance ID containing `VID&SSSSVVVV`.
pub fn vendor_from_instance_id(instance_id: &str) -> Option<(u16, u16)> {
    let upper = instance_id.to_uppercase();
    let start = upper.find("VID&")? + 4;
    let digits = upper.get(start..start + 8)?;
    let source = u16::from_str_radix(&digits[..4], 16).ok()?;
    let vendor = u16::from_str_radix(&digits[4..], 16).ok()?;
    Some((source, vendor))
}

/// The vendor of the device with `address` (12 hex digits, no separators),
/// from a list of PnP instance IDs.
pub fn vendor_for_address<'a>(
    instance_ids: impl IntoIterator<Item = &'a str>,
    address_hex: &str,
) -> Option<(u16, u16)> {
    let needle = format!("&{}_", address_hex.to_uppercase());
    instance_ids
        .into_iter()
        .filter(|id| id.to_uppercase().contains(&needle))
        .find_map(vendor_from_instance_id)
}

/// Display name: trimmed, without the "LE-" prefix some stacks add.
pub fn friendly_name(raw: &str) -> String {
    let trimmed = raw.trim();
    let name = trimmed.strip_prefix("LE-").unwrap_or(trimmed);
    if name.is_empty() {
        "Bose headphones".into()
    } else {
        name.into()
    }
}

/// RFCOMM channels to try, in order. QC Ultra / 700 answer on 2 first.
pub fn channel_candidates(name: &str) -> [u8; 3] {
    let lower = name.to_lowercase();
    if lower.contains("ultra") || lower.contains("700") {
        [2, 8, 9]
    } else {
        [8, 2, 9]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Same cases as Tests/DeviceIdentityTests.swift.
    #[test]
    fn identity() {
        assert!(is_bose("Obsidian", Some(0x009E), Some(1)));
        assert!(is_bose("", Some(0x009E), Some(1)));
        assert!(!is_bose("Other headphones", Some(0x009E), Some(2)));
        assert!(!is_bose("Other headphones", Some(0x009E), None));
        assert!(!is_bose("Other headphones", Some(0x004C), Some(1)));
        assert!(!is_bose("Obsidian", None, None));
        for name in ["Bose QC Headphones", "QuietComfort", "QC35", "QC45", "QC Ultra"] {
            assert!(is_bose(name, None, None));
        }
    }

    #[test]
    fn vendor_from_windows_instance_ids() {
        // Instance IDs of the QC45 on the Windows test PC.
        let ids = [
            r"BTHENUM\DEV_782B645B71EE\7&38D62C91&0&BLUETOOTHDEVICE_782B645B71EE",
            r"BTHENUM\{0000110B-0000-1000-8000-00805F9B34FB}_VID&0001009E_PID&4039\7&38D62C91&0&782B645B71EE_C00000000",
            r"BTHENUM\{0000110B-0000-1000-8000-00805F9B34FB}_VID&0001004C_PID&2014\7&38D62C91&0&447F40A07B56_C00000000",
        ];
        assert_eq!(vendor_for_address(ids, "782B645B71EE"), Some((1, 0x009E)));
        assert_eq!(vendor_for_address(ids, "447f40a07b56"), Some((1, 0x004C)));
        assert_eq!(vendor_for_address(ids, "000000000000"), None);
        assert_eq!(vendor_from_instance_id(r"BTHENUM\DEV_782B645B71EE"), None);
    }

    #[test]
    fn names_and_channels() {
        assert_eq!(friendly_name("  LE-Bose QC45 "), "Bose QC45");
        assert_eq!(friendly_name(""), "Bose headphones");
        assert_eq!(channel_candidates("Bose QC Ultra Headphones"), [2, 8, 9]);
        assert_eq!(channel_candidates("Studio Cans"), [8, 2, 9]);
    }
}
