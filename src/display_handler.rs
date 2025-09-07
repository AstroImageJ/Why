use std::io::Error;

fn print_message(msg: &str) -> Result<(), Error> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::{
            core::*, Win32::UI::WindowsAndMessaging::*,
        };
        use std::ffi::OsStr;
        use std::iter::once;
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = OsStr::new(msg).encode_wide().chain(once(0)).collect();
        let ret = unsafe {
            MessageBoxW(0 as _, wide.as_ptr(), w!("Launcher"), MB_OK)
        };
        if ret == 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    // Mac code from IntelliJ IDEA, licensed under Apache 2.0
    // https://github.com/JetBrains/intellij-community/blob/master/native/XPlatLauncher/src/ui.rs
    #[cfg(target_os = "macos")]
    #[allow(non_snake_case, unused_variables, unused_results)]
    {
        use {
            core_foundation::base::{CFOptionFlags, SInt32, TCFType},
            core_foundation::date::CFTimeInterval,
            core_foundation::string::{CFString, CFStringRef},
            core_foundation::url::CFURLRef
        };

        unsafe extern "C" {
            fn CFUserNotificationDisplayAlert(
                timeout: CFTimeInterval,
                flags: CFOptionFlags,
                iconURL: CFURLRef, soundURL: CFURLRef, localizationURL: CFURLRef,
                alertHeader: CFStringRef, alertMessage: CFStringRef,
                defaultButtonTitle: CFStringRef, alternateButtonTitle: CFStringRef, otherButtonTitle: CFStringRef,
                responseFlags: *mut CFOptionFlags,
            ) -> SInt32;
        }

        let header = CFString::new("Launcher");
        let message = CFString::new(msg);
        let ret = unsafe {
            CFUserNotificationDisplayAlert(
                0.0,
                0,
                std::ptr::null(), std::ptr::null(), std::ptr::null(),
                header.as_concrete_TypeRef(), message.as_concrete_TypeRef(),
                std::ptr::null(), std::ptr::null(), std::ptr::null(),
                std::ptr::null_mut())
        };

        if ret == 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(target_os = "linux")]
    {
        eprintln!("{}", msg);
        Ok(())
    }
}

/// Display a native message dialog.<br>
/// Logs any errors that occur.
pub fn message(msg: &str) {
    if let Err(err) = print_message(msg) {
        eprintln!("Failed to display message due to error: {:?}", err);
        eprintln!("Message: {}", msg);
    }
}
