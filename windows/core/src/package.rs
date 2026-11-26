//! Whether this copy runs from the Microsoft Store's MSIX package. The Store
//! copy is the same build as the GitHub installer's; the Store updates it, so
//! the tray app's updater and `qcb update` send people to the Store instead.

/// The package family name (`Name_publisherhash`), or None outside a package.
#[cfg(windows)]
pub fn family_name() -> Option<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
    use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFamilyName;

    let mut len = 0u32;
    // Outside a package this is APPMODEL_ERROR_NO_PACKAGE.
    if unsafe { GetCurrentPackageFamilyName(&mut len, None) } != ERROR_INSUFFICIENT_BUFFER {
        return None;
    }
    let mut name = vec![0u16; len as usize];
    if unsafe { GetCurrentPackageFamilyName(&mut len, Some(PWSTR(name.as_mut_ptr()))) } != ERROR_SUCCESS {
        return None;
    }
    // `len` counts the terminating null.
    name.truncate((len as usize).saturating_sub(1));
    String::from_utf16(&name).ok()
}

#[cfg(not(windows))]
pub fn family_name() -> Option<String> {
    None
}

pub fn is_packaged() -> bool {
    family_name().is_some()
}

/// QC Buddy's Store page, which offers the update when there is one.
pub fn store_page(family_name: &str) -> String {
    format!("ms-windows-store://pdp/?PFN={family_name}")
}

/// Open QC Buddy's Store page. False outside a package or if it can't open.
pub fn open_store_page() -> bool {
    let Some(name) = family_name() else { return false };
    // explorer.exe hands the ms-windows-store: link to the Store app.
    std::process::Command::new("explorer.exe").arg(store_page(&name)).spawn().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_page_by_family_name() {
        assert_eq!(store_page("QCBuddy_8wekyb3d8bbwe"), "ms-windows-store://pdp/?PFN=QCBuddy_8wekyb3d8bbwe");
    }

    #[test]
    fn tests_run_unpackaged() {
        assert!(!is_packaged());
    }
}
