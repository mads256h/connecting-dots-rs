use anyhow::Result;

pub trait VolumeProvider {
    fn poll_volume(&self) -> Result<Option<f32>>;
}
