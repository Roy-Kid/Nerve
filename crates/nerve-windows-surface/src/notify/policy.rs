//! The notification executor's OS payload; policy is owned by nerve-hub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub title: String,
    pub body: String,
    pub job_id: String,
    pub tag: String,
    pub sound: bool,
}
