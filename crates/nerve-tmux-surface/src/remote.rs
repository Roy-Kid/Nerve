//! Submit remote Open/Focus to the hub; surfaces never execute SSH.
pub struct RemoteSelector;
impl RemoteSelector {
    pub fn new() -> Self {
        Self
    }
    pub fn request(&self, host: &str, pid: u32) {
        let body = serde_json::json!({"host":host,"pid":pid});
        std::thread::spawn(move || {
            let _ = nerve_surface_core::hub::Hub::loopback().send_json(
                "/v1/focus/remote",
                nerve_surface_core::hub::HttpMethod::POST,
                &body,
            );
        });
    }
}
impl Default for RemoteSelector {
    fn default() -> Self {
        Self::new()
    }
}
