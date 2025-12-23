pub mod volume_provider;

mod constant_volume_provider;

#[cfg(feature = "pulseaudio")]
mod pulse;
