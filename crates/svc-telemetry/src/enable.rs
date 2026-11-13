//! Making the game serve the stream: the environment variable it reads, and
//! the permanent, reversible install of it for the Windows user.
//!
//! The game serves telemetry only when the launcher's environment has
//! [`VAR`]`=`[`VALUE`]. Two ways there: start the launcher with it set
//! (the shell does, see `launcher_open`), or install it as a per-user
//! environment variable so the launcher can be started any way.
//!
//! The install writes `HKCU\Environment` (never machine-wide, no elevation)
//! and keeps a **receipt**: what it wrote, and what was there before. The
//! receipt is what makes it reversible. The variable counts as this app's
//! only while a readable receipt exists **and** the variable still holds
//! exactly what the receipt says was written; then
//! [`Installer::uninstall`] puts back the value it found. Anything else — no
//! receipt, an unreadable one, a value someone changed — is not ours:
//! reported as [`InstallState::SetElsewhere`] or
//! [`InstallState::NotInstalled`] and never touched.
//!
//! A receipt that no longer matches is dropped the moment it is seen (any
//! [`Installer::status`]), so "installed, then deleted by hand, then set
//! again by another tool" is not mistaken for ours. The one case this cannot
//! tell apart: the other tool writes the identical value and the app never
//! looked in between.
//!
//! **Where the receipt lives.** Beside the variable, in the user's registry
//! (`HKCU\Software\Starlume\TelemetryEnv`), not in the app's data folder:
//! the variable is one per Windows user while data folders are one per build
//! (debug, release, dev profiles), and a receipt per build would leave every
//! other build unable to see or undo the install.
//!
//! Both sit behind seams ([`UserEnv`], [`ReceiptStore`]), so the logic is
//! tested against fakes and no test writes the real registry.

use std::io;

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

/// Proof that this app set the variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    /// Exactly what the install wrote into the variable.
    pub written: String,
    /// The variable's value before the install; `None` = it did not exist.
    pub previous: Option<String>,
}

/// Where the receipt is kept. One per Windows user, like the variable.
pub trait ReceiptStore {
    /// The receipt, or `None` when there is none **or it cannot be read
    /// whole**: an unreadable receipt proves nothing, so it is not ours.
    fn load(&self) -> Option<Receipt>;
    fn save(&self, receipt: &Receipt) -> io::Result<()>;
    fn clear(&self) -> io::Result<()>;
}

/// Whether the variable is installed, and by whom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallState {
    /// The variable is absent or does not turn the stream on.
    NotInstalled,
    /// This app set it; uninstall removes it.
    Installed,
    /// It turns the stream on, but this app did not set it: left alone.
    SetElsewhere,
}

/// The install / uninstall pair over a [`UserEnv`] and a [`ReceiptStore`].
pub struct Installer<E, R> {
    env: E,
    receipts: R,
}

impl<E: UserEnv, R: ReceiptStore> Installer<E, R> {
    pub fn new(env: E, receipts: R) -> Self {
        Self { env, receipts }
    }

    /// The receipt, if the variable still holds what it says was written.
    /// A receipt that no longer matches is stale and dropped here.
    fn ours(&self, current: Option<&str>) -> Option<Receipt> {
        let receipt = self.receipts.load()?;
        if current == Some(receipt.written.as_str()) {
            Some(receipt)
        } else {
            let _ = self.receipts.clear();
            None
        }
    }

    /// Where the install stands. Drops a stale receipt as a side effect.
    pub fn status(&self) -> io::Result<InstallState> {
        let current = self.env.get(VAR)?;
        let ours = self.ours(current.as_deref()).is_some();
        Ok(match current {
            Some(v) if is_enabling(&v) => {
                if ours {
                    InstallState::Installed
                } else {
                    InstallState::SetElsewhere
                }
            }
            _ => InstallState::NotInstalled,
        })
    }

    /// Set the variable for the user. A variable that already turns the
    /// stream on is left as it is (and stays the user's own when this app
    /// did not set it).
    pub fn install(&self) -> io::Result<InstallState> {
        let state = self.status()?;
        if state != InstallState::NotInstalled {
            return Ok(state);
        }
        // Receipt first: a crash between the two steps leaves a receipt that
        // matches nothing (dropped at the next look), never a variable
        // without one.
        self.receipts.save(&Receipt {
            written: VALUE.into(),
            previous: self.env.get(VAR)?,
        })?;
        if let Err(e) = self.env.set(VAR, VALUE) {
            let _ = self.receipts.clear();
            return Err(e);
        }
        self.status()
    }

    /// Undo [`install`](Self::install): put back what was there before.
    /// Acts only on a variable that is provably this app's (see the module
    /// docs); anything else is left exactly as it is.
    pub fn uninstall(&self) -> io::Result<InstallState> {
        let current = self.env.get(VAR)?;
        if let Some(receipt) = self.ours(current.as_deref()) {
            match &receipt.previous {
                Some(previous) => self.env.set(VAR, previous)?,
                None => self.env.remove(VAR)?,
            }
            self.receipts.clear()?;
        }
        self.status()
    }
}

/// The real environment: `HKCU\Environment`. After each write it tells the
/// running programs (`WM_SETTINGCHANGE`), so Explorer hands the new
/// environment to programs started from then on. The broadcast runs on its
/// own thread and is not waited for: it can take seconds per unresponsive
/// window, and nothing here depends on its outcome. Programs already running
/// (a launcher in the tray) keep the environment they started with.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegistryEnv;

/// The real receipt store: `HKCU\Software\Starlume\TelemetryEnv`, values
/// `Written` and (when there was one) `Previous`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegistryReceipts;

#[cfg(windows)]
mod registry {
    use std::io;

    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

    use super::{Receipt, ReceiptStore, RegistryEnv, RegistryReceipts, UserEnv};

    const ENVIRONMENT: &str = "Environment";
    const RECEIPT_PARENT: &str = r"Software\Starlume";
    const RECEIPT_KEY: &str = "TelemetryEnv";
    const WRITTEN: &str = "Written";
    const PREVIOUS: &str = "Previous";

    fn environment(access: u32) -> io::Result<RegKey> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(ENVIRONMENT, access)
    }

    /// `Ok(None)` for a value that is not there.
    fn read(key: &RegKey, name: &str) -> io::Result<Option<String>> {
        match key.get_value::<String, _>(name) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Tell top-level windows the environment changed (the documented way:
    /// `WM_SETTINGCHANGE` with `lParam` = "Environment"), without waiting.
    fn broadcast() {
        std::thread::spawn(|| {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
            };
            let what: Vec<u16> = ENVIRONMENT
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            // SAFETY: `what` is a NUL-terminated UTF-16 string that outlives
            // the call; the result pointer may be null. The timeout is per
            // window, so the call as a whole has no fixed bound — hence the
            // thread.
            unsafe {
                SendMessageTimeoutW(
                    HWND_BROADCAST,
                    WM_SETTINGCHANGE,
                    0,
                    what.as_ptr() as isize,
                    SMTO_ABORTIFHUNG,
                    1000,
                    std::ptr::null_mut(),
                );
            }
        });
    }

    impl UserEnv for RegistryEnv {
        fn get(&self, name: &str) -> io::Result<Option<String>> {
            read(&environment(KEY_READ)?, name)
        }

        fn set(&self, name: &str, value: &str) -> io::Result<()> {
            environment(KEY_SET_VALUE)?.set_value(name, &value)?;
            broadcast();
            Ok(())
        }

        fn remove(&self, name: &str) -> io::Result<()> {
            match environment(KEY_SET_VALUE)?.delete_value(name) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e),
            }
            broadcast();
            Ok(())
        }
    }

    impl ReceiptStore for RegistryReceipts {
        fn load(&self) -> Option<Receipt> {
            let key = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(format!(r"{RECEIPT_PARENT}\{RECEIPT_KEY}"), KEY_READ)
                .ok()?;
            // Any read error (a value of another type, no access) makes the
            // whole receipt unreadable: not ours.
            Some(Receipt {
                written: read(&key, WRITTEN).ok()??,
                previous: read(&key, PREVIOUS).ok()?,
            })
        }

        fn save(&self, receipt: &Receipt) -> io::Result<()> {
            self.clear()?;
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey(format!(r"{RECEIPT_PARENT}\{RECEIPT_KEY}"))?;
            if let Some(previous) = &receipt.previous {
                key.set_value(PREVIOUS, previous)?;
            }
            // Last: `load` needs it, so a half-written receipt reads as none.
            key.set_value(WRITTEN, &receipt.written)
        }

        fn clear(&self) -> io::Result<()> {
            let parent = match RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(RECEIPT_PARENT, KEY_READ | KEY_SET_VALUE)
            {
                Ok(parent) => parent,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e),
            };
            match parent.delete_subkey(RECEIPT_KEY) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        }
    }
}

#[cfg(not(windows))]
mod registry {
    use std::io;

    use super::{Receipt, ReceiptStore, RegistryEnv, RegistryReceipts, UserEnv};

    fn unsupported() -> io::Error {
        io::Error::new(io::ErrorKind::Unsupported, "the game runs on Windows only")
    }

    impl UserEnv for RegistryEnv {
        fn get(&self, _: &str) -> io::Result<Option<String>> {
            Ok(None)
        }
        fn set(&self, _: &str, _: &str) -> io::Result<()> {
            Err(unsupported())
        }
        fn remove(&self, _: &str) -> io::Result<()> {
            Ok(())
        }
    }

    impl ReceiptStore for RegistryReceipts {
        fn load(&self) -> Option<Receipt> {
            None
        }
        fn save(&self, _: &Receipt) -> io::Result<()> {
            Err(unsupported())
        }
        fn clear(&self) -> io::Result<()> {
            Ok(())
        }
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

    /// An in-memory receipt; `unreadable` stands for a damaged one.
    #[derive(Default)]
    struct FakeReceipts {
        receipt: RefCell<Option<Receipt>>,
        unreadable: RefCell<bool>,
    }

    impl ReceiptStore for &FakeReceipts {
        fn load(&self) -> Option<Receipt> {
            if *self.unreadable.borrow() {
                return None;
            }
            self.receipt.borrow().clone()
        }
        fn save(&self, receipt: &Receipt) -> io::Result<()> {
            *self.receipt.borrow_mut() = Some(receipt.clone());
            Ok(())
        }
        fn clear(&self) -> io::Result<()> {
            *self.receipt.borrow_mut() = None;
            *self.unreadable.borrow_mut() = false;
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

    fn value(env: &FakeEnv) -> Option<String> {
        env.vars.borrow().get(VAR).cloned()
    }

    #[test]
    fn install_then_uninstall_leaves_no_trace() {
        let (env, receipts) = (with(&[("PATH", "C:\\bin")]), FakeReceipts::default());
        let i = Installer::new(&env, &receipts);
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);

        assert_eq!(i.install().unwrap(), InstallState::Installed);
        assert_eq!(value(&env).as_deref(), Some("true"));
        // Twice is once.
        assert_eq!(i.install().unwrap(), InstallState::Installed);

        assert_eq!(i.uninstall().unwrap(), InstallState::NotInstalled);
        assert_eq!(value(&env), None);
        assert_eq!(env.vars.borrow().len(), 1, "other variables are untouched");
        assert_eq!(*receipts.receipt.borrow(), None);
    }

    #[test]
    fn a_variable_the_user_set_is_never_removed() {
        let (env, receipts) = (with(&[(VAR, "TRUE")]), FakeReceipts::default());
        let i = Installer::new(&env, &receipts);
        assert_eq!(i.status().unwrap(), InstallState::SetElsewhere);
        assert_eq!(i.install().unwrap(), InstallState::SetElsewhere);
        assert_eq!(i.uninstall().unwrap(), InstallState::SetElsewhere);
        assert_eq!(value(&env).as_deref(), Some("TRUE"));
    }

    #[test]
    fn uninstall_puts_back_the_value_it_found() {
        let (env, receipts) = (with(&[(VAR, "false")]), FakeReceipts::default());
        let i = Installer::new(&env, &receipts);
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);
        assert_eq!(i.install().unwrap(), InstallState::Installed);
        assert_eq!(value(&env).as_deref(), Some("true"));
        assert_eq!(i.uninstall().unwrap(), InstallState::NotInstalled);
        assert_eq!(value(&env).as_deref(), Some("false"));
    }

    #[test]
    fn uninstall_leaves_a_value_the_user_changed_since() {
        let (env, receipts) = (with(&[]), FakeReceipts::default());
        let i = Installer::new(&env, &receipts);
        i.install().unwrap();
        env.vars.borrow_mut().insert(VAR.into(), "0".into());
        assert_eq!(i.uninstall().unwrap(), InstallState::NotInstalled);
        assert_eq!(value(&env).as_deref(), Some("0"));
        // The receipt is gone: a later uninstall has nothing to act on.
        assert_eq!(*receipts.receipt.borrow(), None);
    }

    #[test]
    fn a_variable_set_again_by_someone_else_is_not_ours() {
        let (env, receipts) = (with(&[]), FakeReceipts::default());
        let i = Installer::new(&env, &receipts);
        i.install().unwrap();
        // The user deletes it by hand; the app sees that once…
        env.vars.borrow_mut().remove(VAR);
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);
        // …then another tool sets it, to the very same value.
        env.vars.borrow_mut().insert(VAR.into(), "true".into());
        assert_eq!(i.status().unwrap(), InstallState::SetElsewhere);
        assert_eq!(i.uninstall().unwrap(), InstallState::SetElsewhere);
        assert_eq!(value(&env).as_deref(), Some("true"));
    }

    #[test]
    fn a_value_that_differs_from_the_write_is_not_ours() {
        let (env, receipts) = (with(&[]), FakeReceipts::default());
        let i = Installer::new(&env, &receipts);
        i.install().unwrap();
        // Still enabling, but not what the receipt says was written.
        env.vars.borrow_mut().insert(VAR.into(), "True".into());
        assert_eq!(i.uninstall().unwrap(), InstallState::SetElsewhere);
        assert_eq!(value(&env).as_deref(), Some("True"));
    }

    #[test]
    fn an_unreadable_receipt_is_not_ours() {
        let (env, receipts) = (with(&[(VAR, "false")]), FakeReceipts::default());
        let i = Installer::new(&env, &receipts);
        i.install().unwrap();
        *receipts.unreadable.borrow_mut() = true;
        assert_eq!(i.status().unwrap(), InstallState::SetElsewhere);
        // Nothing is deleted or "restored" on a guess.
        assert_eq!(i.uninstall().unwrap(), InstallState::SetElsewhere);
        assert_eq!(value(&env).as_deref(), Some("true"));
    }

    #[test]
    fn a_variable_removed_by_hand_can_be_installed_again() {
        let (env, receipts) = (with(&[]), FakeReceipts::default());
        let i = Installer::new(&env, &receipts);
        i.install().unwrap();
        env.vars.borrow_mut().remove(VAR);
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);
        assert_eq!(i.install().unwrap(), InstallState::Installed);
        assert_eq!(i.uninstall().unwrap(), InstallState::NotInstalled);
        assert_eq!(value(&env), None);
    }

    #[test]
    fn a_failed_write_leaves_no_receipt() {
        let env = FakeEnv {
            fail_set: true,
            ..FakeEnv::default()
        };
        let receipts = FakeReceipts::default();
        let i = Installer::new(&env, &receipts);
        assert!(i.install().is_err());
        assert_eq!(i.status().unwrap(), InstallState::NotInstalled);
        assert_eq!(*receipts.receipt.borrow(), None);
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
    }
}
