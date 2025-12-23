use std::rc::Rc;
use anyhow::Result;
use crate::volume_providers::constant_volume_provider::ConstantVolumeProvider;
#[cfg(feature = "pulseaudio")]
use crate::volume_providers::pulse::PulseAudioVolumeProvider;

pub trait VolumeProvider {
    fn poll_volume(&self) -> Result<Option<f32>>;
}


pub fn get_volume_provider() -> Rc<dyn VolumeProvider> {
    #[cfg(feature = "pulseaudio")]
    {
        if let Ok(pulse_volume_provider) = PulseAudioVolumeProvider::new() {
            return Rc::new(pulse_volume_provider);
        }

        Rc::new(ConstantVolumeProvider::new(0.8))
    }
}