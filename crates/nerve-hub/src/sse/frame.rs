//! Authoritative protocol v2 full snapshots.
use crate::lifecycle::NotifyLease;
use crate::model::Job;
use crate::state::{JobStore, JobView};
use serde_json::{Value, json};
pub struct Frame<'a> {
    store: &'a JobStore,
    departed: Vec<Job>,
    notify: NotifyLease,
}
impl<'a> Frame<'a> {
    pub fn connect(store: &'a JobStore, notify: NotifyLease) -> Self {
        Self {
            store,
            departed: vec![],
            notify,
        }
    }
    pub fn published(store: &'a mut JobStore, notify: NotifyLease) -> Self {
        store.revision += 1;
        let departed = store.drain_departed_deduped();
        Self {
            store,
            departed,
            notify,
        }
    }
    pub fn value(&self) -> Value {
        let jobs = self.store.jobs_json();
        let summary = crate::projection::summarize(&jobs);
        let order: Vec<_> = jobs
            .as_array()
            .into_iter()
            .flatten()
            .map(|j| j["id"].clone())
            .collect();
        json!({"protocolVersion":2,"hubInstanceId":self.store.instance_id,"machineId":self.store.machine_id,
            "revision":self.store.revision,"jobs":jobs,"departed":self.departed.iter().map(JobView::departed).collect::<Vec<_>>(),
            "order":order,"summary":summary,
            "tabIndicators":crate::projection::tab_indicators(&jobs),
            "machines":self.store.machines.values().collect::<Vec<_>>(),"notify":self.notify,"notifications":[]})
    }
    pub fn render(&self) -> String {
        self.value().to_string()
    }
}
