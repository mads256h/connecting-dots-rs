use crate::volume_providers::volume_provider::VolumeProvider;

pub struct ConstantVolumeProvider {
    intensity: f32,
}

impl ConstantVolumeProvider {
    pub fn new(intensity: f32) -> Self {
        Self { intensity }
    }
}

impl VolumeProvider for ConstantVolumeProvider {
    fn poll_volume(&self) -> anyhow::Result<Option<f32>> {
        Ok(Some(self.intensity))
    }
}