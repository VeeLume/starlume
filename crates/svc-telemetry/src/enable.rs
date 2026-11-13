//! Making the game serve the stream: the environment variable it reads, and
//! the permanent, reversible install of it for the Windows user.
//!
//! The game serves telemetry only when the launcher's environment has
//! [`VAR`]`=`[`VALUE`]. Two ways there: start the launcher with it set
//! (the shell does, see `launcher_open`), or install it as a per-user
//! environment variable so the launcher can be started any way.
//!
//! The install writes `HKCU\Environment` (never machine-wide, no elevation)
//! and keeps a **receipt** beside the app's data. The receipt is what makes
//! it reversible: [`Installer::uninstall`] acts only when a receipt says this
//! app set the variable, and puts back the value it found. A variable the
//! user set themselves is reported as [`InstallState::SetElsewhere`] and
//! never touched.
//!
//! The registry sits behind [`UserEnv`], so the logic is tested against a
//! fake and no test writes the real environment.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The variable the game checks.
pub const VAR: &str = "GRPC_CLIENT_SERVER_ENABLED";
/// The value that turns the stream on.
pub const VALUE: &str = "true";

/// Does this value of [`VAR`] turn the stream on?
pub fn is_enabling(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case(VALUE)
}

/// The user's persistent environment variables.
pub trait UserEnv {
    fn get(&self, name: &str) -> io::Result<Option<String>>;
    fn set(&self, name: &str, value: &str) -> io::Result<()>;
    fn remove(&self, name: &str) -> io::Result<()>;
}

/// Whether the variable is installed, and by whom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum InstallState {
    /// The variable is absent or does not turn the stream on.
    NotInstalled,
    /// This app set it; uninstall removes it.
    Installed,
    /// It turns the stream on, but this app did not set it: left alone.
    SetElsewhere,
}

/// Proof that this app set the variable, and what was there before.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Receipt {
    /// The variable's value before the install; `None` = it did not exist.
    previous: Option<String>,
}

/// The install / uninstall pair over a [`UserEnv`].
pub struct Installer<E> {
    env: E,
    receipt: PathBuf,
}

impl<E: UserEnv> Installer<E> {
    /// `receipt` is the file that records an install (the shell passes a
    /// path under `app_kit::app_data_root()`).
    pub fn new(env: E, receipt: PathBuf) -> Self {
        Self { env, receipt }
    }

    fn enabled(&self) -> io::Result<bool> {
        Ok(self.env.get(VAR)?.is_some_and(|v| is_enabling(&v)))
    }

    pub fn status(&self) -> io::Result<InstallState> {
        Ok(match (self.enabled()?, self.receipt.is_file()) {
            (true, true) => InstallState::Installed,
            (true, false) => InstallState::SetElsewhere,
            // A receipt without the variable: someone removed it since.
            (false, _) => InstallState::NotInstalled,
        })
    }

    /// Set the variable for the user. A variable that already turns the
    /// stream on is left as it is (and stays the user's own when this app
    /// did not set it).
    pub fn install(&self) -> io::Result<InstallState> {
        if self.enabled()? {
            return self.status();
        }
        // Receipt first: a crash between the two steps leaves a receipt
        // without the variable (harmless), never a variable without one.
        let receipt = Receipt {
            previous: self.env.get(VAR)?,
        };
        app_kit::save_json(&self.receipt, &receipt)?;
        if let Err(e) = self.env.set(VAR, VALUE) {
            let _ = std::fs::remove_file(&self.receipt);
            return Err(e);
        }
        self.status()
    }

    /// Undo [`install`](Self::install): put back what was there before. Does
    /// nothing without a receipt, and leaves a variable the user changed
    /// since (it no longer turns the stream on) as they set it.
    pub fn uninstall(&self) -> io::Result<InstallState> {
        if !self.receipt.is_file() {
            return self.status();
        }
        let receipt: Receipt = app_kit::load_json(&self.receipt);
        if self.enabled()? {
            match &receipt.previous {
                Some(previous) => self.env.set(VAR, previous)?,
                None => self.env.remove(VAR)?,
            }
        }
        std::fs::remove_file(&self.receipt)?;
        self.status()
    }
}

/// The real thing: `HKCU\Environment`, with a `WM_SETTINGCHANGE` broadcast
/// after each write so Explorer hands the new environment to programs
/// started from then on. Programs already running (a launcher in the tray)
/// keep the environment they started with.
///
/// Blocking — a write waits on the broadcast (hung windows are skipped, five
/// seconds at the very most): call from `spawn_blocking`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegistryEnv;

#[cfg(windows)]
mod registry {
    use std::io;

    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

    use super::{RegistryEnv, UserEnv};

    const KEY: &str = "Environment";

    fn open(access: u32) -> io::Result<RegKey> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(KEY, access)
    }

    /// Tell top-level windows the environment changed (the documented way:
    /// `WM_SETTINGCHANGE` with `lParam` = "Environment").
    fn broadcast() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
        };
        let what: Vec<u16> = KEY.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `what` is a NUL-terminated UTF-16 string that outlives the
        // call; the result pointer may be null.
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                what.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                5000,
                std::ptr::null_mut(),
            );
        }
    }

    impl UserEnv for RegistryEnv {
        fn get(&self, name: &str) -> io::Result<Option<String>> {
            match open(KEY_READ)?.get_value::<String, _>(name) {
                Ok(v) => Ok(Some(v)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e),
            }
        }

        fn set(&self, name: &str, value: &str) -> io::Result<()> {
            open(KEY_SET_VALUE)?.set_value(name, &value)?;
            broadcast();
            Ok(())
        }

        fn remove(&self, name: &str) -> io::Result<()> {
            match open(KEY_SET_VALUE)?.delete_value(name) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e),
            }
            broadcast();
            Ok(())
        }
    }
}

#[cfg(not(windows))]
impl UserEnv for RegistryEnv {
    fn get(&self, _: &str) -> io::Result<Option<String>> {
        Ok(None)
    }
    fn set(&self, _: &str, _: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the game runs on Windows only",
        ))
    }
    fn remove(&self, _: &str) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    /// An in-memory environment; `fail_set` makes writes fail.
    #[derive(Default)]
    struct FakeEnv {
        vars: RefCell<BTreeMap<String, String>>,
        fail_set: bool,
    }

    impl UserEnv for &FakeEnv {
        fn get(&self, name: &str) -> io::Result<Option<String>> {
            Ok(self.vars.borrow().get(name).cloned())
        }
        fn set(&self, name: &str, value: &str) -> io::Result<()> {
            if self.fail_set {
                return Err(io::Error::other("denied"));
            }
            self.vars.borrow_mut().insert(name.into(), value.into());
            Ok(())
        }
        fn remove(&self, name: &str) -> io::Result<()> {
            self.vars.borrow_mut().remove(name);
            Ok(())
        }
    }

    fn with(vars: &[(&str, &str)]) -> FakeEnv {
        FakeEnv {
            vars: RefCell::new(
                vars.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
            fail_set: false,
        }
    }

    fn installer<'a>(env: &'a FakeEnv, dir: &tempfile::TempDir) -> Installer<&'a FakeEnv> {
        Installer::new(env, dir.path().join("telemetry-env.json"))
    }

    fn value(env: &FakeEnv) -> Option<String> {
        env.vars.borrow().get(VAR).cloned()
    }

    #[test]
    fn install_then_uninstall_leaves_no_trace() {
        let dir = tempfile::tempdir().unwrap();
        let env = with(&[("PATH", "C:\\bin")]);
        let i = installer(&env, &dir);
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);

        assert_eq!(i.install().unwrap(), InstallState::Installed);
        assert_eq!(value(&env).as_deref(), Some("true"));
        // Twice is once.
        assert_eq!(i.install().unwrap(), InstallState::Installed);

        assert_eq!(i.uninstall().unwrap(), InstallState::NotInstalled);
        assert_eq!(value(&env), None);
        assert_eq!(env.vars.borrow().len(), 1, "other variables are untouched");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_variable_the_user_set_is_never_removed() {
        let dir = tempfile::tempdir().unwrap();
        let env = with(&[(VAR, "TRUE")]);
        let i = installer(&env, &dir);
        assert_eq!(i.status().unwrap(), InstallState::SetElsewhere);
        assert_eq!(i.install().unwrap(), InstallState::SetElsewhere);
        assert_eq!(i.uninstall().unwrap(), InstallState::SetElsewhere);
        assert_eq!(value(&env).as_deref(), Some("TRUE"));
    }

    #[test]
    fn uninstall_puts_back_the_value_it_found() {
        let dir = tempfile::tempdir().unwrap();
        let env = with(&[(VAR, "false")]);
        let i = installer(&env, &dir);
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);
        assert_eq!(i.install().unwrap(), InstallState::Installed);
        assert_eq!(value(&env).as_deref(), Some("true"));
        assert_eq!(i.uninstall().unwrap(), InstallState::NotInstalled);
        assert_eq!(value(&env).as_deref(), Some("false"));
    }

    #[test]
    fn uninstall_leaves_a_value_the_user_changed_since() {
        let dir = tempfile::tempdir().unwrap();
        let env = with(&[]);
        let i = installer(&env, &dir);
        i.install().unwrap();
        env.vars.borrow_mut().insert(VAR.into(), "0".into());
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);
        assert_eq!(i.uninstall().unwrap(), InstallState::NotInstalled);
        assert_eq!(value(&env).as_deref(), Some("0"));
        // The receipt is gone: a later uninstall has nothing to act on.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_variable_removed_by_hand_can_be_installed_again() {
        let dir = tempfile::tempdir().unwrap();
        let env = with(&[]);
        let i = installer(&env, &dir);
        i.install().unwrap();
        env.vars.borrow_mut().remove(VAR);
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);
        assert_eq!(i.install().unwrap(), InstallState::Installed);
        assert_eq!(i.uninstall().unwrap(), InstallState::NotInstalled);
        assert_eq!(value(&env), None);
    }

    /// The one test against the real registry, and it only reads: a missing
    /// variable is `None`, not an error.
    #[cfg(windows)]
    #[test]
    fn the_registry_reads_a_missing_variable_as_none() {
        assert_eq!(
            RegistryEnv.get("STARLUME_TEST_NO_SUCH_VARIABLE").unwrap(),
            None
        );
        // TEMP is a per-user variable on every Windows install.
        assert!(RegistryEnv.get("TEMP").unwrap().is_some());
    }

    #[test]
    fn a_failed_write_leaves_no_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let env = FakeEnv {
            fail_set: true,
            ..FakeEnv::default()
        };
        let i = installer(&env, &dir);
        assert!(i.install().is_err());
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
