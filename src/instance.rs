//! One Almanac at a time (per data folder): starting it again while it runs
//! brings the running one's window forward instead of opening a second copy
//! that would sync and write the same mail at the same time.

use std::sync::Mutex;

#[cfg(target_os = "linux")]
static CONNECTION: Mutex<Option<zbus::blocking::Connection>> = Mutex::new(None);

/// The bus name of Almanac for this data folder (test copies with their own
/// folders don't count as "already running").
#[cfg(target_os = "linux")]
fn bus_name() -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::data::data_dir().hash(&mut h);
    format!("org.craft.Almanac.Instance.d{:016x}", h.finish())
}

/// Takes the name for this run. False when another Almanac already has it:
/// that one has been asked to show its window, and this one should stop.
#[cfg(target_os = "linux")]
pub fn claim() -> bool {
    use zbus::fdo::{RequestNameFlags, RequestNameReply};
    let Ok(conn) = zbus::blocking::Connection::session() else { return true };
    let name = bus_name();
    let flags = RequestNameFlags::DoNotQueue.into();
    match conn.request_name_with_flags(name.as_str(), flags) {
        Ok(RequestNameReply::PrimaryOwner) | Ok(RequestNameReply::AlreadyOwner) => {
            *CONNECTION.lock().unwrap() = Some(conn);
            true
        }
        Ok(_) | Err(zbus::Error::NameTaken) => {
            // Running already: ask it to come forward (it may still be starting).
            eprintln!("Almanac is already running: bringing its window forward.");
            let req = crate::cli::launch_request().map(|r| r.to_string());
            for _ in 0..20 {
                let call = match &req {
                    Some(r) => conn.call_method(Some(name.as_str()), "/Instance", Some("org.craft.Almanac.Instance"), "Open", &(r.as_str(),)),
                    None => conn.call_method(Some(name.as_str()), "/Instance", Some("org.craft.Almanac.Instance"), "Show", &()),
                };
                match call {
                    Ok(_) => break,
                    Err(e) => eprintln!("  (not answering yet: {e})"),
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            false
        }
        Err(e) => {
            eprintln!("single instance check: {e}");
            true
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn claim() -> bool {
    true
}

/// Hands an open request (`openRequest` JSON) to the Almanac already running
/// for this data folder. False when none is (start one with it instead).
#[cfg(target_os = "linux")]
pub fn forward(req: &str) -> bool {
    let Ok(conn) = zbus::blocking::Connection::session() else { return false };
    conn.call_method(Some(bus_name().as_str()), "/Instance", Some("org.craft.Almanac.Instance"), "Open", &(req,)).is_ok()
}

#[cfg(not(target_os = "linux"))]
pub fn forward(_req: &str) -> bool {
    false
}

#[cfg(target_os = "linux")]
struct Instance {
    show: Box<dyn Fn() + Send + Sync>,
    open: Box<dyn Fn(String) + Send + Sync>,
}

#[cfg(target_os = "linux")]
#[zbus::interface(name = "org.craft.Almanac.Instance")]
impl Instance {
    #[zbus(name = "Show")]
    fn show(&self) {
        (self.show)();
    }

    /// Shows something (an event, the new-event editor): `openRequest` JSON.
    #[zbus(name = "Open")]
    fn open(&self, request: String) {
        (self.open)(request);
    }
}

/// Answers "Show" from a second start by calling `show`, and "Open" (an
/// event, a new event) by calling `open` (from the running app).
#[cfg(target_os = "linux")]
pub fn serve(show: impl Fn() + Send + Sync + 'static, open: impl Fn(String) + Send + Sync + 'static) {
    if let Some(conn) = CONNECTION.lock().unwrap().as_ref() {
        let _ = conn.object_server().at("/Instance", Instance { show: Box::new(show), open: Box::new(open) });
    }
}

#[cfg(not(target_os = "linux"))]
pub fn serve(_show: impl Fn() + Send + Sync + 'static, _open: impl Fn(String) + Send + Sync + 'static) {}
