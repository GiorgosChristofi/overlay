use async_trait::async_trait;
use domain::builds::{Build, BuildCatalog};
use domain::champ_select::ChampionPick;
use domain::ports::BuildSource;
use domain::PortError;

/// The curated builds shipped with the app, as a [`BuildSource`]. Used when
/// live statistics are unavailable.
#[derive(Debug)]
pub struct SeedBuilds(pub BuildCatalog);

#[async_trait]
impl BuildSource for SeedBuilds {
    fn name(&self) -> &str {
        "built-in builds"
    }

    async fn build(&self, pick: ChampionPick) -> Result<Option<Build>, PortError> {
        Ok(self.0.select(pick.champion_id, pick.role).cloned())
    }
}
