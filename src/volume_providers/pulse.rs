use std::{cell::RefCell, rc::Rc};

use crate::volume_providers::volume_provider::VolumeProvider;
use anyhow::Result;
use anyhow::anyhow;
use clap::crate_version;
use libpulse_binding::def::BufferAttr;
use libpulse_binding::{
    self,
    context::{Context, FlagSet, State},
    mainloop::standard::Mainloop,
    proplist::{Proplist, properties},
    sample::{Format, Spec},
    stream::{self, Stream},
};
use libpulse_sys::pa_operation_state_t;

pub struct PulseAudioVolumeProvider {
    main_loop: Rc<RefCell<Mainloop>>,
    context: Rc<RefCell<Context>>,
    monitor_stream: Rc<RefCell<Stream>>,
}

impl PulseAudioVolumeProvider {
    pub fn new() -> Result<Self> {
        let main_loop = Rc::new(RefCell::new(
            Mainloop::new().ok_or(anyhow!("Failed to create Mainloop"))?,
        ));
        let mut proplist = Proplist::new().ok_or(anyhow!("Failed to create Proplist"))?;
        proplist
            .set_str(properties::APPLICATION_NAME, "Connecting Dots")
            .unwrap();
        proplist
            .set_str(properties::APPLICATION_ID, "org.mads256h.connectingdots")
            .unwrap();
        proplist
            .set_str(properties::APPLICATION_ICON_NAME, "audio-card")
            .unwrap();
        proplist
            .set_str(properties::APPLICATION_VERSION, crate_version!())
            .unwrap();
        let context = Rc::new(RefCell::new(
            Context::new_with_proplist(&*main_loop.borrow(), "connecting_dots", &proplist)
                .ok_or(anyhow!("Failed to create context"))?,
        ));

        context.borrow_mut().connect(None, FlagSet::NOFLAGS, None)?;

        loop {
            match context.borrow().get_state() {
                State::Ready => break,
                State::Failed | State::Terminated => panic!("Failed to connect to pulseaudio"),
                _ => main_loop.borrow_mut().iterate(false),
            };
        }

        let default_sink_name = Rc::new(RefCell::new(None::<String>));

        {
            let default_sink_name = Rc::clone(&default_sink_name);

            let mut main_loop = main_loop.borrow_mut();

            let operation = context.borrow().introspect().get_server_info(move |info| {
                *default_sink_name.borrow_mut() =
                    info.default_sink_name.as_ref().map(|s| s.to_string());
            });

            loop {
                match operation.get_state() {
                    pa_operation_state_t::Running => main_loop.iterate(false),
                    _ => break,
                };
            }
        }

        let default_sink_name = default_sink_name
            .borrow()
            .clone()
            .ok_or(anyhow!("Failed to get default sink name"))?;

        let monitor_source = format!("{}.monitor", default_sink_name);

        const PEAKS_RATE: u32 = 144;

        let sample_spec = Spec {
            channels: 1,
            format: Format::FLOAT32NE,
            rate: PEAKS_RATE,
        };

        let buffer_attributes = BufferAttr {
            fragsize: size_of::<f32>() as u32,
            maxlength: u32::MAX,
            tlength: 0,
            prebuf: 0,
            minreq: 0,
        };

        let monitor_stream = Rc::new(RefCell::new(
            Stream::new(
                &mut *context.borrow_mut(),
                "Peak detect",
                &sample_spec,
                None,
            )
            .ok_or(anyhow!("Failed to create monitoring stream"))?,
        ));
        monitor_stream.borrow_mut().connect_record(
            Some(&monitor_source),
            Some(&buffer_attributes),
            stream::FlagSet::PEAK_DETECT | stream::FlagSet::ADJUST_LATENCY,
        )?;

        loop {
            match monitor_stream.borrow_mut().get_state() {
                stream::State::Unconnected | stream::State::Creating => {
                    main_loop.borrow_mut().iterate(false)
                }
                stream::State::Ready => break,
                stream::State::Failed | stream::State::Terminated => {
                    panic!("Failed to connect monitor stream")
                }
            };
        }

        Ok(PulseAudioVolumeProvider {
            main_loop,
            context,
            monitor_stream,
        })
    }
}

impl VolumeProvider for PulseAudioVolumeProvider {
    fn poll_volume(&self) -> Result<Option<f32>> {
        self.main_loop.borrow_mut().iterate(false);

        let mut stream = self.monitor_stream.borrow_mut();

        match stream.peek()? {
            stream::PeekResult::Empty => Ok(None),
            stream::PeekResult::Hole(_) => {
                stream.discard()?;
                Ok(None)
            }
            stream::PeekResult::Data(items) => {
                let len = items.len();
                let bytes: [u8; 4] = items[len - 4..len].try_into()?;
                stream.discard()?;
                let peak = f32::from_ne_bytes(bytes);
                Ok(Some(peak))
            }
        }
    }
}
