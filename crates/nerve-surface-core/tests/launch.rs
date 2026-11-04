use nerve_surface_core::launch::*;
use std::path::Path;
struct Probe;
impl HealthProbe for Probe {
    fn is_serving(&self) -> bool {
        false
    }
}
struct Spawner;
impl ProcessSpawner for Spawner {
    fn spawn(&mut self, _: &Path, args: &[&str]) -> Result<(), SpawnError> {
        assert_eq!(args, ["ensure"]);
        Ok(())
    }
}
#[test]
fn repeated_bootstrap_always_delegates_to_ensure() {
    let mut launcher = HubLauncher::new(Spawner, Probe);
    for _ in 0..3 {
        assert_eq!(
            launcher.ensure(Some(Path::new("nerve-hub"))),
            Launch::Spawned
        );
    }
    assert_eq!(launcher.ensure(None), Launch::Missing);
}
