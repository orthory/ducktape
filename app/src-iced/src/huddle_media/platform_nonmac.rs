use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
#[cfg(target_os = "linux")]
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(target_os = "windows")]
use cpal::{SampleFormat, Stream, StreamConfig, SupportedStreamConfig};
use tokio::sync::mpsc::error::TryRecvError;

use super::*;
use crate::huddle_session::{DriverIncoming, DriverOutgoing, FRAME_SAMPLES, SAMPLE_RATE};

const AUDIO_PLAYBACK_QUEUE: usize = 8;
#[cfg(target_os = "windows")]
const SCREEN_FRAME_QUEUE: usize = 2;
const LOOP_INTERVAL: Duration = Duration::from_millis(5);
const VIDEO_INTERVAL: Duration = Duration::from_millis(1000 / VIDEO_FPS as u64);
const MAX_CAPTURE_PIXELS: u64 = 7680 * 4320;

pub struct Backend;

impl MediaBackend for Backend {
    fn run(
        mut port: MediaDriverPort,
        commands: Receiver<Command>,
        events: SyncSender<Event>,
        running: Arc<AtomicBool>,
        level: Arc<AtomicU8>,
    ) {
        // Match the privacy contract on every desktop: joining starts muted.
        let muted = Arc::new(AtomicBool::new(true));
        let mut microphone = None;
        let mut speaker = None;
        let mut camera_device = None;
        let (audio, playback_tx) = match open_audio(
            Arc::clone(&muted),
            Arc::clone(&level),
            port.outgoing.clone(),
            microphone,
            speaker,
        ) {
            Ok(audio) => audio,
            Err(error) => {
                emit_failure(
                    &events,
                    error,
                    FailureKind::MicrophoneDenied,
                    FailureKind::MicrophoneUnavailable,
                );
                emit(&events, Event::Stopped);
                return;
            }
        };
        let mut audio = Some(audio);
        let mut playback_tx = playback_tx;
        let mut screen_sources = Vec::new();
        let mut screen_source = None;
        emit(&events, Event::Ready);
        emit(
            &events,
            Event::Devices(enumerate_devices(
                microphone,
                camera_device,
                speaker,
                &screen_sources,
                screen_source,
            )),
        );

        let mut source = VideoSource::Off;
        let mut camera_on = false;
        let mut sharing = false;
        let mut resume_camera_after_share = false;
        let mut codec = VideoCodec::new(800);
        let mut force_keyframe = true;
        let mut last_video = Instant::now() - VIDEO_INTERVAL;
        let started = Instant::now();
        let mut keyframe_requests = HashMap::<[u8; 32], Instant>::new();

        while running.load(Ordering::Acquire) {
            match commands.recv_timeout(LOOP_INTERVAL) {
                Ok(Command::SetMuted(value)) => muted.store(value, Ordering::Release),
                Ok(Command::SetCamera(value)) => {
                    if value == camera_on && !sharing {
                        continue;
                    }
                    resume_camera_after_share = false;
                    screen_sources.clear();
                    screen_source = None;
                    stop_source(&mut source);
                    camera_on = false;
                    sharing = false;
                    if value {
                        match CameraSource::open(camera_device) {
                            Ok(camera) => {
                                source = VideoSource::Camera(camera);
                                camera_on = true;
                                force_keyframe = true;
                            }
                            Err(error) => emit_failure(
                                &events,
                                error,
                                FailureKind::CameraDenied,
                                FailureKind::CameraUnavailable,
                            ),
                        }
                    }
                    emit(&events, Event::VideoState { camera_on, sharing });
                    emit_devices(
                        &events,
                        microphone,
                        camera_device,
                        speaker,
                        &screen_sources,
                        screen_source,
                    );
                }
                Ok(Command::SetScreenShare(value)) => {
                    if value == sharing && !camera_on {
                        continue;
                    }
                    if value {
                        match selectable_screen_sources() {
                            Ok(Some(sources)) => {
                                screen_sources = sources;
                                screen_source = None;
                                emit_devices(
                                    &events,
                                    microphone,
                                    camera_device,
                                    speaker,
                                    &screen_sources,
                                    screen_source,
                                );
                                continue;
                            }
                            Ok(None) => {}
                            Err(error) => {
                                emit_failure(
                                    &events,
                                    error,
                                    FailureKind::ScreenDenied,
                                    FailureKind::ScreenUnavailable,
                                );
                                continue;
                            }
                        }
                        resume_camera_after_share = camera_on;
                    }
                    stop_source(&mut source);
                    camera_on = false;
                    sharing = false;
                    if value {
                        match ScreenSource::open_picker() {
                            Ok(screen) => {
                                source = VideoSource::Screen(screen);
                                sharing = true;
                                force_keyframe = true;
                            }
                            Err(error) => {
                                emit_failure(
                                    &events,
                                    error,
                                    FailureKind::ScreenDenied,
                                    FailureKind::ScreenUnavailable,
                                );
                                restore_camera(
                                    &mut source,
                                    &mut camera_on,
                                    &mut force_keyframe,
                                    resume_camera_after_share,
                                    camera_device,
                                );
                                resume_camera_after_share = false;
                            }
                        }
                    } else {
                        restore_camera(
                            &mut source,
                            &mut camera_on,
                            &mut force_keyframe,
                            resume_camera_after_share,
                            camera_device,
                        );
                        resume_camera_after_share = false;
                    }
                    screen_sources.clear();
                    screen_source = None;
                    emit(&events, Event::VideoState { camera_on, sharing });
                    emit_devices(
                        &events,
                        microphone,
                        camera_device,
                        speaker,
                        &screen_sources,
                        screen_source,
                    );
                }
                Ok(Command::RefreshDevices) => emit_devices(
                    &events,
                    microphone,
                    camera_device,
                    speaker,
                    &screen_sources,
                    screen_source,
                ),
                Ok(Command::SetMicrophone(selection)) => {
                    let old = microphone;
                    drop(audio.take());
                    match open_audio(
                        Arc::clone(&muted),
                        Arc::clone(&level),
                        port.outgoing.clone(),
                        selection,
                        speaker,
                    ) {
                        Ok((device, sender)) => {
                            audio = Some(device);
                            playback_tx = sender;
                            microphone = selection;
                        }
                        Err(error) => {
                            emit_failure(
                                &events,
                                error,
                                FailureKind::DeviceSelection,
                                FailureKind::DeviceSelection,
                            );
                            if let Ok((device, sender)) = open_audio(
                                Arc::clone(&muted),
                                Arc::clone(&level),
                                port.outgoing.clone(),
                                old,
                                speaker,
                            ) {
                                audio = Some(device);
                                playback_tx = sender;
                            }
                        }
                    }
                    emit_devices(
                        &events,
                        microphone,
                        camera_device,
                        speaker,
                        &screen_sources,
                        screen_source,
                    );
                }
                Ok(Command::SetSpeaker(selection)) => {
                    let old = speaker;
                    drop(audio.take());
                    match open_audio(
                        Arc::clone(&muted),
                        Arc::clone(&level),
                        port.outgoing.clone(),
                        microphone,
                        selection,
                    ) {
                        Ok((device, sender)) => {
                            audio = Some(device);
                            playback_tx = sender;
                            speaker = selection;
                        }
                        Err(error) => {
                            emit_failure(
                                &events,
                                error,
                                FailureKind::DeviceSelection,
                                FailureKind::DeviceSelection,
                            );
                            if let Ok((device, sender)) = open_audio(
                                Arc::clone(&muted),
                                Arc::clone(&level),
                                port.outgoing.clone(),
                                microphone,
                                old,
                            ) {
                                audio = Some(device);
                                playback_tx = sender;
                            }
                        }
                    }
                    emit_devices(
                        &events,
                        microphone,
                        camera_device,
                        speaker,
                        &screen_sources,
                        screen_source,
                    );
                }
                Ok(Command::SetCameraDevice(selection)) => {
                    let old = camera_device;
                    camera_device = selection;
                    if camera_on {
                        stop_source(&mut source);
                        match CameraSource::open(camera_device) {
                            Ok(camera) => {
                                source = VideoSource::Camera(camera);
                                force_keyframe = true;
                            }
                            Err(error) => {
                                camera_device = old;
                                camera_on = false;
                                emit_failure(
                                    &events,
                                    error,
                                    FailureKind::DeviceSelection,
                                    FailureKind::DeviceSelection,
                                );
                                emit(&events, Event::VideoState { camera_on, sharing });
                            }
                        }
                    }
                    emit_devices(
                        &events,
                        microphone,
                        camera_device,
                        speaker,
                        &screen_sources,
                        screen_source,
                    );
                }
                Ok(Command::SetScreenSource(selection)) => {
                    let Some(index) = selection else {
                        continue;
                    };
                    let Some(choice) = screen_sources.get(index) else {
                        emit_failure(
                            &events,
                            "the selected screen source is no longer available".into(),
                            FailureKind::DeviceSelection,
                            FailureKind::DeviceSelection,
                        );
                        continue;
                    };
                    resume_camera_after_share = camera_on;
                    stop_source(&mut source);
                    camera_on = false;
                    sharing = false;
                    match ScreenSource::open_source(choice) {
                        Ok(screen) => {
                            source = VideoSource::Screen(screen);
                            sharing = true;
                            screen_source = Some(index);
                            force_keyframe = true;
                        }
                        Err(error) => {
                            emit_failure(
                                &events,
                                error,
                                FailureKind::ScreenDenied,
                                FailureKind::ScreenUnavailable,
                            );
                            restore_camera(
                                &mut source,
                                &mut camera_on,
                                &mut force_keyframe,
                                resume_camera_after_share,
                                camera_device,
                            );
                            resume_camera_after_share = false;
                        }
                    }
                    emit(&events, Event::VideoState { camera_on, sharing });
                    emit_devices(
                        &events,
                        microphone,
                        camera_device,
                        speaker,
                        &screen_sources,
                        screen_source,
                    );
                }
                Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }

            for _ in 0..8 {
                match port.incoming.try_recv() {
                    Ok(DriverIncoming::Audio(frame)) => {
                        let _ = playback_tx.try_send(frame);
                    }
                    Ok(DriverIncoming::Video {
                        peer,
                        keyframe,
                        vp8,
                        ..
                    }) => match codec.decode(peer, keyframe, &vp8) {
                        Ok(frame) => emit(
                            &events,
                            Event::PeerFrame {
                                peer: hex_key(peer),
                                frame,
                            },
                        ),
                        Err(error) => {
                            let now = Instant::now();
                            if keyframe_requests.get(&peer).is_none_or(|last| {
                                now.duration_since(*last) >= Duration::from_secs(2)
                            }) {
                                keyframe_requests.insert(peer, now);
                                emit(&events, Event::RequestKeyframe(hex_key(peer)));
                            }
                            tracing::debug!(
                                target: "ducktape::huddle",
                                event = "video_decode_failed",
                                reason = "invalid_vp8_frame",
                                detail = %error
                            );
                        }
                    },
                    Ok(DriverIncoming::KeyframeRequested) => force_keyframe = true,
                    Ok(DriverIncoming::RateHint { max_kbps }) => {
                        codec.set_rate(max_kbps.clamp(300, 1_200));
                        force_keyframe = true;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        running.store(false, Ordering::Release);
                        break;
                    }
                }
            }

            if !matches!(source, VideoSource::Off) && last_video.elapsed() >= VIDEO_INTERVAL {
                last_video = Instant::now();
                match source.frame() {
                    Ok(Some(frame)) => {
                        match rgb_to_i420(&frame.rgba, frame.width, frame.height, 4, sharing) {
                            Ok(frame) => match codec.encode(&frame, force_keyframe) {
                                Ok((keyframe, vp8)) => {
                                    force_keyframe = false;
                                    let timestamp_ms =
                                        started.elapsed().as_millis().min(u128::from(u32::MAX))
                                            as u32;
                                    let _ = port.outgoing.try_send(DriverOutgoing::Video {
                                        keyframe,
                                        timestamp_ms,
                                        vp8,
                                    });
                                    emit(&events, Event::LocalFrame(preview(&frame)));
                                }
                                Err(detail) => emit(
                                    &events,
                                    Event::Failed {
                                        kind: FailureKind::Codec,
                                        detail,
                                    },
                                ),
                            },
                            Err(detail) => emit(
                                &events,
                                Event::Failed {
                                    kind: FailureKind::Codec,
                                    detail,
                                },
                            ),
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let was_sharing = sharing;
                        let (denied, unavailable) = if was_sharing {
                            (FailureKind::ScreenDenied, FailureKind::ScreenUnavailable)
                        } else {
                            (FailureKind::CameraDenied, FailureKind::CameraUnavailable)
                        };
                        emit_failure(&events, error, denied, unavailable);
                        stop_source(&mut source);
                        camera_on = false;
                        sharing = false;
                        restore_camera(
                            &mut source,
                            &mut camera_on,
                            &mut force_keyframe,
                            was_sharing && resume_camera_after_share,
                            camera_device,
                        );
                        resume_camera_after_share = false;
                        emit(&events, Event::VideoState { camera_on, sharing });
                    }
                }
            }
        }

        stop_source(&mut source);
        drop(audio);
        emit(&events, Event::Stopped);
    }
}

fn restore_camera(
    source: &mut VideoSource,
    camera_on: &mut bool,
    force_keyframe: &mut bool,
    resume: bool,
    selection: Option<usize>,
) {
    if resume && let Ok(camera) = CameraSource::open(selection) {
        *source = VideoSource::Camera(camera);
        *camera_on = true;
        *force_keyframe = true;
    }
}

#[cfg(target_os = "windows")]
fn open_audio(
    muted: Arc<AtomicBool>,
    level: Arc<AtomicU8>,
    outgoing: tokio::sync::mpsc::Sender<DriverOutgoing>,
    microphone: Option<usize>,
    speaker: Option<usize>,
) -> Result<(AudioStreams, SyncSender<Box<[i16; FRAME_SAMPLES]>>), String> {
    let host = cpal::default_host();
    let input = if let Some(index) = microphone {
        host.input_devices()
            .map_err(|error| error.to_string())?
            .nth(index)
            .ok_or_else(|| "the selected microphone is no longer available".to_string())?
    } else {
        host.default_input_device()
            .ok_or_else(|| "no default microphone is available".to_string())?
    };
    let output = if let Some(index) = speaker {
        host.output_devices()
            .map_err(|error| error.to_string())?
            .nth(index)
            .ok_or_else(|| "the selected speaker is no longer available".to_string())?
    } else {
        host.default_output_device()
            .ok_or_else(|| "no default speaker is available".to_string())?
    };
    let input_config = audio_config(&input, true)?;
    let output_config = audio_config(&output, false)?;
    let (playback_tx, playback_rx) = sync_channel(AUDIO_PLAYBACK_QUEUE);
    let input_stream = build_input_stream(&input, &input_config, muted, level, outgoing)?;
    let output_stream = build_output_stream(&output, &output_config, playback_rx)?;
    output_stream.play().map_err(|error| error.to_string())?;
    input_stream.play().map_err(|error| error.to_string())?;
    Ok((
        AudioStreams {
            _input: input_stream,
            _output: output_stream,
        },
        playback_tx,
    ))
}

#[cfg(target_os = "windows")]
struct AudioStreams {
    _input: Stream,
    _output: Stream,
}

#[cfg(target_os = "windows")]
fn audio_config(device: &cpal::Device, input: bool) -> Result<SupportedStreamConfig, String> {
    let configs = if input {
        device
            .supported_input_configs()
            .map_err(|error| error.to_string())?
            .collect::<Vec<_>>()
    } else {
        device
            .supported_output_configs()
            .map_err(|error| error.to_string())?
            .collect::<Vec<_>>()
    };
    configs
        .into_iter()
        .filter(|config| {
            config.min_sample_rate() <= SAMPLE_RATE && config.max_sample_rate() >= SAMPLE_RATE
        })
        .min_by_key(|config| {
            let format = match config.sample_format() {
                SampleFormat::I16 => 0,
                SampleFormat::F32 => 1,
                SampleFormat::U16 => 2,
                _ => 3,
            };
            (format, config.channels())
        })
        .map(|config| config.with_sample_rate(SAMPLE_RATE))
        .ok_or_else(|| "the audio device does not support 48 kHz PCM".into())
}

#[cfg(target_os = "windows")]
fn build_input_stream(
    device: &cpal::Device,
    supported: &SupportedStreamConfig,
    muted: Arc<AtomicBool>,
    level: Arc<AtomicU8>,
    outgoing: tokio::sync::mpsc::Sender<DriverOutgoing>,
) -> Result<Stream, String> {
    let channels = usize::from(supported.channels());
    let config: StreamConfig = supported.clone().into();
    macro_rules! build {
        ($sample:ty, $convert:expr) => {{
            let mut captured = Box::new([0i16; FRAME_SAMPLES]);
            let mut offset = 0usize;
            device.build_input_stream(
                config,
                move |data: &[$sample], _| {
                    let mut peak = 0u16;
                    for frame in data.chunks_exact(channels) {
                        let sample = (frame
                            .iter()
                            .copied()
                            .map($convert)
                            .map(i32::from)
                            .sum::<i32>()
                            / channels as i32)
                            .clamp(i32::from(i16::MIN), i32::from(i16::MAX))
                            as i16;
                        peak = peak.max(sample.unsigned_abs());
                        if muted.load(Ordering::Acquire) {
                            offset = 0;
                            continue;
                        }
                        captured[offset] = sample;
                        offset += 1;
                        if offset == FRAME_SAMPLES {
                            let frame =
                                std::mem::replace(&mut captured, Box::new([0i16; FRAME_SAMPLES]));
                            offset = 0;
                            let _ = outgoing.try_send(DriverOutgoing::Audio(frame));
                        }
                    }
                    level.store(((u32::from(peak) * 100) / 32_768) as u8, Ordering::Release);
                },
                audio_error,
                None,
            )
        }};
    }
    let stream = match supported.sample_format() {
        SampleFormat::I16 => build!(i16, |sample: i16| sample),
        SampleFormat::U16 => build!(u16, |sample: u16| (i32::from(sample) - 32_768) as i16),
        SampleFormat::F32 => build!(f32, |sample: f32| {
            (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
        }),
        format => return Err(format!("unsupported microphone sample format {format}")),
    }
    .map_err(|error| error.to_string())?;
    Ok(stream)
}

#[cfg(target_os = "windows")]
fn build_output_stream(
    device: &cpal::Device,
    supported: &SupportedStreamConfig,
    playback: Receiver<Box<[i16; FRAME_SAMPLES]>>,
) -> Result<Stream, String> {
    let channels = usize::from(supported.channels());
    let config: StreamConfig = supported.clone().into();
    macro_rules! build {
        ($sample:ty, $convert:expr) => {{
            let mut playing: Option<Box<[i16; FRAME_SAMPLES]>> = None;
            let mut offset = 0usize;
            device.build_output_stream(
                config,
                move |data: &mut [$sample], _| {
                    for frame in data.chunks_mut(channels) {
                        if playing.is_none() {
                            playing = playback.try_recv().ok();
                            offset = 0;
                        }
                        let sample = playing.as_ref().map_or(0, |samples| samples[offset]);
                        frame.fill($convert(sample));
                        if playing.is_some() {
                            offset += 1;
                            if offset == FRAME_SAMPLES {
                                playing = None;
                            }
                        }
                    }
                },
                audio_error,
                None,
            )
        }};
    }
    let stream = match supported.sample_format() {
        SampleFormat::I16 => build!(i16, |sample: i16| sample),
        SampleFormat::U16 => build!(u16, |sample: i16| (i32::from(sample) + 32_768) as u16),
        SampleFormat::F32 => build!(f32, |sample: i16| f32::from(sample) / 32_768.0),
        format => return Err(format!("unsupported speaker sample format {format}")),
    }
    .map_err(|error| error.to_string())?;
    Ok(stream)
}

#[cfg(target_os = "windows")]
fn audio_error(error: cpal::Error) {
    tracing::debug!(
        target: "ducktape::huddle",
        event = "audio_stream_error",
        reason = "device_stream_failed",
        detail = %error
    );
}

#[cfg(target_os = "linux")]
type PcmOpen = unsafe extern "C" fn(
    *mut *mut std::ffi::c_void,
    *const std::ffi::c_char,
    std::ffi::c_int,
    std::ffi::c_int,
) -> std::ffi::c_int;
#[cfg(target_os = "linux")]
type PcmClose = unsafe extern "C" fn(*mut std::ffi::c_void) -> std::ffi::c_int;
#[cfg(target_os = "linux")]
type PcmSetParams = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    std::ffi::c_int,
    std::ffi::c_int,
    std::ffi::c_uint,
    std::ffi::c_uint,
    std::ffi::c_int,
    std::ffi::c_uint,
) -> std::ffi::c_int;
#[cfg(target_os = "linux")]
type PcmIo = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    *mut std::ffi::c_void,
    libc::c_ulong,
) -> libc::c_long;
#[cfg(target_os = "linux")]
type PcmRecover = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    std::ffi::c_int,
    std::ffi::c_int,
) -> std::ffi::c_int;
#[cfg(target_os = "linux")]
type PcmDrop = unsafe extern "C" fn(*mut std::ffi::c_void) -> std::ffi::c_int;
#[cfg(target_os = "linux")]
type FormatValue = unsafe extern "C" fn(*const std::ffi::c_char) -> std::ffi::c_int;
#[cfg(target_os = "linux")]
type DeviceNameHint = unsafe extern "C" fn(
    std::ffi::c_int,
    *const std::ffi::c_char,
    *mut *mut *mut std::ffi::c_void,
) -> std::ffi::c_int;
#[cfg(target_os = "linux")]
type DeviceNameGetHint =
    unsafe extern "C" fn(*const std::ffi::c_void, *const std::ffi::c_char) -> *mut std::ffi::c_char;
#[cfg(target_os = "linux")]
type DeviceNameFreeHint = unsafe extern "C" fn(*mut *mut std::ffi::c_void) -> std::ffi::c_int;
#[cfg(target_os = "linux")]
type StrError = unsafe extern "C" fn(std::ffi::c_int) -> *const std::ffi::c_char;

#[cfg(target_os = "linux")]
struct AlsaLibrary(*mut std::ffi::c_void);

#[cfg(target_os = "linux")]
unsafe impl Send for AlsaLibrary {}
#[cfg(target_os = "linux")]
unsafe impl Sync for AlsaLibrary {}

#[cfg(target_os = "linux")]
impl AlsaLibrary {
    fn open() -> Result<Self, String> {
        let handle = unsafe {
            libc::dlopen(
                c"libasound.so.2".as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL,
            )
        };
        if handle.is_null() {
            Err(format!(
                "libasound.so.2 is unavailable: {}",
                dynamic_library_error()
            ))
        } else {
            Ok(Self(handle))
        }
    }

    fn symbol<T: Copy>(&self, name: &std::ffi::CStr) -> Result<T, String> {
        let symbol = unsafe { libc::dlsym(self.0, name.as_ptr()) };
        if symbol.is_null() {
            return Err(format!(
                "ALSA symbol {} is unavailable: {}",
                name.to_string_lossy(),
                dynamic_library_error()
            ));
        }
        debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&symbol));
        Ok(unsafe { std::mem::transmute_copy(&symbol) })
    }
}

#[cfg(target_os = "linux")]
impl Drop for AlsaLibrary {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.0);
        }
    }
}

#[cfg(target_os = "linux")]
fn dynamic_library_error() -> String {
    let error = unsafe { libc::dlerror() };
    if error.is_null() {
        "unknown dynamic loader error".into()
    } else {
        unsafe { std::ffi::CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(target_os = "linux")]
struct Alsa {
    _library: AlsaLibrary,
    pcm_open: PcmOpen,
    pcm_close: PcmClose,
    pcm_set_params: PcmSetParams,
    pcm_readi: PcmIo,
    pcm_writei: PcmIo,
    pcm_recover: PcmRecover,
    pcm_drop: PcmDrop,
    format_value: FormatValue,
    device_name_hint: DeviceNameHint,
    device_name_get_hint: DeviceNameGetHint,
    device_name_free_hint: DeviceNameFreeHint,
    strerror: StrError,
}

#[cfg(target_os = "linux")]
unsafe impl Send for Alsa {}
#[cfg(target_os = "linux")]
unsafe impl Sync for Alsa {}

#[cfg(target_os = "linux")]
impl Alsa {
    fn load() -> Result<Arc<Self>, String> {
        let library = AlsaLibrary::open()?;
        Ok(Arc::new(Self {
            pcm_open: library.symbol(c"snd_pcm_open")?,
            pcm_close: library.symbol(c"snd_pcm_close")?,
            pcm_set_params: library.symbol(c"snd_pcm_set_params")?,
            pcm_readi: library.symbol(c"snd_pcm_readi")?,
            pcm_writei: library.symbol(c"snd_pcm_writei")?,
            pcm_recover: library.symbol(c"snd_pcm_recover")?,
            pcm_drop: library.symbol(c"snd_pcm_drop")?,
            format_value: library.symbol(c"snd_pcm_format_value")?,
            device_name_hint: library.symbol(c"snd_device_name_hint")?,
            device_name_get_hint: library.symbol(c"snd_device_name_get_hint")?,
            device_name_free_hint: library.symbol(c"snd_device_name_free_hint")?,
            strerror: library.symbol(c"snd_strerror")?,
            _library: library,
        }))
    }

    fn error(&self, code: std::ffi::c_int) -> String {
        let detail = unsafe { (self.strerror)(code) };
        if detail.is_null() {
            format!("ALSA error {code}")
        } else {
            unsafe { std::ffi::CStr::from_ptr(detail) }
                .to_string_lossy()
                .into_owned()
        }
    }

    fn devices(&self, capture: bool) -> Vec<AlsaDevice> {
        let mut devices = vec![AlsaDevice {
            name: "default".into(),
            label: "System default".into(),
        }];
        let mut hints = std::ptr::null_mut();
        let result = unsafe { (self.device_name_hint)(-1, c"pcm".as_ptr(), &mut hints) };
        if result < 0 || hints.is_null() {
            return devices;
        }
        for index in 0..256 {
            let hint = unsafe { *hints.add(index) };
            if hint.is_null() {
                break;
            }
            let name = self.hint_string(hint, c"NAME");
            let direction = self.hint_string(hint, c"IOID");
            let description = self.hint_string(hint, c"DESC");
            let Some(name) = name.filter(|name| !name.is_empty() && name != "null") else {
                continue;
            };
            if (capture && direction.as_deref() == Some("Output"))
                || (!capture && direction.as_deref() == Some("Input"))
                || devices.iter().any(|device| device.name == name)
            {
                continue;
            }
            let label = description
                .map(|description| description.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|description| !description.is_empty())
                .unwrap_or_else(|| name.clone())
                .chars()
                .take(160)
                .collect();
            devices.push(AlsaDevice { name, label });
        }
        unsafe {
            (self.device_name_free_hint)(hints);
        }
        devices
    }

    fn hint_string(&self, hint: *mut std::ffi::c_void, key: &std::ffi::CStr) -> Option<String> {
        let value = unsafe { (self.device_name_get_hint)(hint, key.as_ptr()) };
        if value.is_null() {
            return None;
        }
        let owned = unsafe { std::ffi::CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned();
        unsafe {
            libc::free(value.cast());
        }
        Some(owned)
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct AlsaDevice {
    name: String,
    label: String,
}

#[cfg(target_os = "linux")]
struct Pcm {
    handle: *mut std::ffi::c_void,
    alsa: Arc<Alsa>,
}

#[cfg(target_os = "linux")]
unsafe impl Send for Pcm {}

#[cfg(target_os = "linux")]
impl Pcm {
    fn open(alsa: Arc<Alsa>, name: &str, stream: std::ffi::c_int) -> Result<Self, String> {
        let name = std::ffi::CString::new(name)
            .map_err(|_| "the selected ALSA device name is invalid".to_string())?;
        let mut handle = std::ptr::null_mut();
        let result = unsafe { (alsa.pcm_open)(&mut handle, name.as_ptr(), stream, 1) };
        if result < 0 {
            return Err(alsa.error(result));
        }
        let pcm = Self { handle, alsa };
        let format = unsafe { (pcm.alsa.format_value)(c"S16_LE".as_ptr()) };
        if format < 0 {
            return Err("ALSA does not support signed 16-bit PCM".into());
        }
        // SND_PCM_ACCESS_RW_INTERLEAVED is a stable ALSA ABI enum value.
        let result =
            unsafe { (pcm.alsa.pcm_set_params)(pcm.handle, format, 3, 1, SAMPLE_RATE, 1, 40_000) };
        if result < 0 {
            Err(pcm.alsa.error(result))
        } else {
            Ok(pcm)
        }
    }

    fn recover(&self, error: std::ffi::c_int) -> bool {
        unsafe { (self.alsa.pcm_recover)(self.handle, error, 1) >= 0 }
    }
}

#[cfg(target_os = "linux")]
impl Drop for Pcm {
    fn drop(&mut self) {
        unsafe {
            (self.alsa.pcm_drop)(self.handle);
            (self.alsa.pcm_close)(self.handle);
        }
    }
}

#[cfg(target_os = "linux")]
struct AudioStreams {
    running: Arc<AtomicBool>,
    capture: Option<JoinHandle<()>>,
    playback: Option<JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
impl Drop for AudioStreams {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(worker) = self.capture.take() {
            let _ = worker.join();
        }
        if let Some(worker) = self.playback.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(target_os = "linux")]
fn open_audio(
    muted: Arc<AtomicBool>,
    level: Arc<AtomicU8>,
    outgoing: tokio::sync::mpsc::Sender<DriverOutgoing>,
    microphone: Option<usize>,
    speaker: Option<usize>,
) -> Result<(AudioStreams, SyncSender<Box<[i16; FRAME_SAMPLES]>>), String> {
    let alsa = Alsa::load()?;
    let inputs = alsa.devices(true);
    let outputs = alsa.devices(false);
    let input = inputs
        .get(microphone.unwrap_or(0))
        .ok_or_else(|| "the selected microphone is no longer available".to_string())?;
    let output = outputs
        .get(speaker.unwrap_or(0))
        .ok_or_else(|| "the selected speaker is no longer available".to_string())?;
    let capture_pcm = Pcm::open(Arc::clone(&alsa), &input.name, 1)?;
    let playback_pcm = Pcm::open(alsa, &output.name, 0)?;
    let running = Arc::new(AtomicBool::new(true));
    let capture_running = Arc::clone(&running);
    let capture = std::thread::Builder::new()
        .name("ducktape-huddle-alsa-in".into())
        .spawn(move || {
            alsa_capture_loop(capture_pcm, capture_running, muted, level, outgoing);
        })
        .map_err(|error| format!("could not start ALSA capture: {error}"))?;
    let (playback_tx, playback_rx) = sync_channel(AUDIO_PLAYBACK_QUEUE);
    let playback_running = Arc::clone(&running);
    let playback = match std::thread::Builder::new()
        .name("ducktape-huddle-alsa-out".into())
        .spawn(move || alsa_playback_loop(playback_pcm, playback_running, playback_rx))
    {
        Ok(worker) => worker,
        Err(error) => {
            running.store(false, Ordering::Release);
            let _ = capture.join();
            return Err(format!("could not start ALSA playback: {error}"));
        }
    };
    Ok((
        AudioStreams {
            running,
            capture: Some(capture),
            playback: Some(playback),
        },
        playback_tx,
    ))
}

#[cfg(target_os = "linux")]
fn alsa_capture_loop(
    pcm: Pcm,
    running: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
    level: Arc<AtomicU8>,
    outgoing: tokio::sync::mpsc::Sender<DriverOutgoing>,
) {
    let mut frame = Box::new([0i16; FRAME_SAMPLES]);
    let mut offset = 0usize;
    while running.load(Ordering::Acquire) {
        let remaining = FRAME_SAMPLES - offset;
        let result = unsafe {
            (pcm.alsa.pcm_readi)(
                pcm.handle,
                frame[offset..].as_mut_ptr().cast(),
                remaining as libc::c_ulong,
            )
        };
        if result == -libc::EAGAIN as libc::c_long {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        if result < 0 {
            if pcm.recover(result as std::ffi::c_int) {
                continue;
            }
            tracing::debug!(
                target: "ducktape::huddle",
                event = "audio_stream_error",
                reason = "capture_failed",
                detail = %pcm.alsa.error(result as std::ffi::c_int)
            );
            break;
        }
        let captured = result as usize;
        if captured == 0 || captured > remaining {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        offset += captured;
        if offset == FRAME_SAMPLES {
            let peak = frame
                .iter()
                .map(|sample| sample.unsigned_abs())
                .max()
                .unwrap_or(0);
            level.store(((u32::from(peak) * 100) / 32_768) as u8, Ordering::Release);
            if !muted.load(Ordering::Acquire) {
                let complete = std::mem::replace(&mut frame, Box::new([0; FRAME_SAMPLES]));
                let _ = outgoing.try_send(DriverOutgoing::Audio(complete));
            }
            offset = 0;
        }
    }
    level.store(0, Ordering::Release);
}

#[cfg(target_os = "linux")]
fn alsa_playback_loop(
    pcm: Pcm,
    running: Arc<AtomicBool>,
    playback: Receiver<Box<[i16; FRAME_SAMPLES]>>,
) {
    let silence = [0i16; FRAME_SAMPLES];
    while running.load(Ordering::Acquire) {
        let frame = playback.try_recv().ok();
        let samples = frame.as_deref().unwrap_or(&silence);
        let mut offset = 0usize;
        while offset < FRAME_SAMPLES && running.load(Ordering::Acquire) {
            let remaining = FRAME_SAMPLES - offset;
            let result = unsafe {
                (pcm.alsa.pcm_writei)(
                    pcm.handle,
                    samples[offset..].as_ptr().cast_mut().cast(),
                    remaining as libc::c_ulong,
                )
            };
            if result == -libc::EAGAIN as libc::c_long {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            if result < 0 {
                if pcm.recover(result as std::ffi::c_int) {
                    continue;
                }
                tracing::debug!(
                    target: "ducktape::huddle",
                    event = "audio_stream_error",
                    reason = "playback_failed",
                    detail = %pcm.alsa.error(result as std::ffi::c_int)
                );
                return;
            }
            let written = result as usize;
            if written == 0 || written > remaining {
                std::thread::sleep(Duration::from_millis(2));
            } else {
                offset += written;
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn audio_device_names() -> (Vec<String>, Vec<String>) {
    Alsa::load().map_or_else(
        |_| (Vec::new(), Vec::new()),
        |alsa| {
            (
                alsa.devices(true)
                    .into_iter()
                    .map(|device| device.label)
                    .collect(),
                alsa.devices(false)
                    .into_iter()
                    .map(|device| device.label)
                    .collect(),
            )
        },
    )
}

struct RawFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

enum VideoSource {
    Off,
    Camera(CameraSource),
    Screen(ScreenSource),
}

impl VideoSource {
    fn frame(&mut self) -> Result<Option<RawFrame>, String> {
        match self {
            Self::Off => Ok(None),
            Self::Camera(camera) => camera.frame(),
            Self::Screen(screen) => screen.frame(),
        }
    }
}

fn stop_source(source: &mut VideoSource) {
    *source = VideoSource::Off;
}

fn enumerate_devices(
    microphone: Option<usize>,
    camera: Option<usize>,
    speaker: Option<usize>,
    screen_sources: &[ScreenChoice],
    screen_source: Option<usize>,
) -> DeviceOptions {
    let mut options = DeviceOptions {
        microphone,
        camera,
        speaker,
        screen_sources: screen_choice_labels(screen_sources),
        screen_source,
        cameras: camera_names(),
        ..DeviceOptions::default()
    };
    let (microphones, speakers) = audio_device_names();
    options.microphones = microphones;
    options.speakers = speakers;
    options
}

#[cfg(target_os = "windows")]
fn audio_device_names() -> (Vec<String>, Vec<String>) {
    let host = cpal::default_host();
    let speakers = host.output_devices().map_or_else(
        |_| Vec::new(),
        |devices| {
            devices
                .filter_map(|device| {
                    device
                        .description()
                        .ok()
                        .map(|description| description.to_string())
                })
                .collect()
        },
    );
    let microphones = host.input_devices().map_or_else(
        |_| Vec::new(),
        |devices| {
            devices
                .filter_map(|device| {
                    device
                        .description()
                        .ok()
                        .map(|description| description.to_string())
                })
                .collect()
        },
    );
    (microphones, speakers)
}

fn emit_devices(
    events: &SyncSender<Event>,
    microphone: Option<usize>,
    camera: Option<usize>,
    speaker: Option<usize>,
    screen_sources: &[ScreenChoice],
    screen_source: Option<usize>,
) {
    emit(
        events,
        Event::Devices(enumerate_devices(
            microphone,
            camera,
            speaker,
            screen_sources,
            screen_source,
        )),
    );
}

fn emit_failure(
    events: &SyncSender<Event>,
    detail: String,
    denied: FailureKind,
    unavailable: FailureKind,
) {
    let kind = classify_error(&detail, denied, unavailable);
    emit(events, Event::Failed { kind, detail });
}

fn classify_error(detail: &str, denied: FailureKind, unavailable: FailureKind) -> FailureKind {
    let detail = detail.to_ascii_lowercase();
    if detail.contains("permission")
        || detail.contains("denied")
        || detail.contains("not authorized")
        || detail.contains("not authorised")
        || detail.contains("cancel")
    {
        denied
    } else {
        unavailable
    }
}

fn valid_frame(width: u32, height: u32, bytes: usize) -> Result<(), String> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || pixels > MAX_CAPTURE_PIXELS
        || bytes < pixels.saturating_mul(4) as usize
    {
        Err("capture returned an invalid frame".into())
    } else {
        Ok(())
    }
}

fn hex_key(key: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in key {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(target_os = "windows")]
mod native {
    use nokhwa::Camera;
    use nokhwa::pixel_format::RgbFormat;
    use nokhwa::utils::{
        ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType,
        Resolution,
    };
    use windows_capture::capture::{
        CaptureControl, Context as CaptureContext, GraphicsCaptureApiHandler,
    };
    use windows_capture::frame::Frame;
    use windows_capture::graphics_capture_api::InternalCaptureControl;
    use windows_capture::monitor::Monitor;
    use windows_capture::settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        GraphicsCaptureItemType, MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    };
    use windows_capture::window::Window;

    use super::*;

    pub struct CameraSource(Camera);

    impl CameraSource {
        pub fn open(selection: Option<usize>) -> Result<Self, String> {
            let requested =
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(CameraFormat::new(
                    Resolution::new(VIDEO_WIDTH, VIDEO_HEIGHT),
                    FrameFormat::MJPEG,
                    VIDEO_FPS,
                )));
            let index = if let Some(index) = selection {
                nokhwa::query(ApiBackend::MediaFoundation)
                    .map_err(|error| error.to_string())?
                    .get(index)
                    .map(|camera| camera.index().clone())
                    .ok_or_else(|| "the selected camera is no longer available".to_string())?
            } else {
                CameraIndex::Index(0)
            };
            let mut camera = Camera::new(index, requested).map_err(|error| error.to_string())?;
            camera.open_stream().map_err(|error| error.to_string())?;
            Ok(Self(camera))
        }

        pub fn frame(&mut self) -> Result<Option<RawFrame>, String> {
            let image = self
                .0
                .frame()
                .and_then(|frame| frame.decode_image::<RgbFormat>())
                .map_err(|error| error.to_string())?;
            let (width, height) = image.dimensions();
            let rgb = image.into_raw();
            let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
            for pixel in rgb.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
            valid_frame(width, height, rgba.len())?;
            Ok(Some(RawFrame {
                width,
                height,
                rgba,
            }))
        }
    }

    pub fn camera_names() -> Vec<String> {
        nokhwa::query(ApiBackend::MediaFoundation).map_or_else(
            |_| Vec::new(),
            |cameras| {
                cameras
                    .into_iter()
                    .map(|camera| camera.human_name())
                    .collect()
            },
        )
    }

    struct CaptureHandler {
        frames: SyncSender<RawFrame>,
    }

    impl GraphicsCaptureApiHandler for CaptureHandler {
        type Flags = SyncSender<RawFrame>;
        type Error = String;

        fn new(context: CaptureContext<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self {
                frames: context.flags,
            })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            _control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            let width = frame.width();
            let height = frame.height();
            if u64::from(width) * u64::from(height) > MAX_CAPTURE_PIXELS {
                return Err("the selected screen is too large to capture safely".into());
            }
            let buffer = frame.buffer().map_err(|error| error.to_string())?;
            let mut contiguous = Vec::new();
            let rgba = buffer.as_nopadding_buffer(&mut contiguous).to_vec();
            valid_frame(width, height, rgba.len())?;
            let _ = self.frames.try_send(RawFrame {
                width,
                height,
                rgba,
            });
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    pub struct ScreenSource {
        control: Option<CaptureControl<CaptureHandler, String>>,
        frames: Receiver<RawFrame>,
    }

    impl ScreenSource {
        pub fn open_picker() -> Result<Self, String> {
            Err("Windows uses the in-app screen-source selector".into())
        }

        pub fn open_source(choice: &ScreenChoice) -> Result<Self, String> {
            match choice {
                ScreenChoice::Monitor(_, monitor) => Self::start(*monitor),
                ScreenChoice::Window(_, window) => Self::start(*window),
            }
        }

        fn start<T>(item: T) -> Result<Self, String>
        where
            T: TryInto<GraphicsCaptureItemType> + Send + 'static,
        {
            let (tx, frames) = sync_channel(SCREEN_FRAME_QUEUE);
            let settings = Settings::new(
                item,
                CursorCaptureSettings::WithCursor,
                DrawBorderSettings::Default,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Custom(VIDEO_INTERVAL),
                DirtyRegionSettings::Default,
                ColorFormat::Rgba8,
                tx,
            );
            let control =
                CaptureHandler::start_free_threaded(settings).map_err(|error| error.to_string())?;
            Ok(Self {
                control: Some(control),
                frames,
            })
        }

        pub fn frame(&self) -> Result<Option<RawFrame>, String> {
            let mut latest = self.frames.try_recv().ok();
            while let Ok(next) = self.frames.try_recv() {
                latest = Some(next);
            }
            if latest.is_none()
                && self
                    .control
                    .as_ref()
                    .is_none_or(CaptureControl::is_finished)
            {
                Err("the screen capture session ended".into())
            } else {
                Ok(latest)
            }
        }
    }

    impl Drop for ScreenSource {
        fn drop(&mut self) {
            if let Some(control) = self.control.take()
                && let Err(error) = control.stop()
            {
                tracing::debug!(
                    target: "ducktape::huddle",
                    event = "screen_capture_stop_failed",
                    reason = "capture_stop_failed",
                    detail = %error
                );
            }
        }
    }

    pub fn selectable_screen_sources() -> Result<Option<Vec<ScreenChoice>>, String> {
        Ok(Some(screen_items()?))
    }

    pub enum ScreenChoice {
        Monitor(String, Monitor),
        Window(String, Window),
    }

    pub fn screen_choice_labels(choices: &[ScreenChoice]) -> Vec<String> {
        choices
            .iter()
            .map(|choice| match choice {
                ScreenChoice::Monitor(label, _) | ScreenChoice::Window(label, _) => label.clone(),
            })
            .collect()
    }

    fn screen_items() -> Result<Vec<ScreenChoice>, String> {
        let monitors = Monitor::enumerate().map_err(|error| error.to_string())?;
        let windows = Window::enumerate().map_err(|error| error.to_string())?;
        let mut items = Vec::with_capacity(monitors.len() + windows.len().min(96));
        for (index, monitor) in monitors.into_iter().enumerate() {
            let name = monitor
                .name()
                .unwrap_or_else(|_| format!("Display {}", index + 1));
            let size = monitor.width().ok().zip(monitor.height().ok());
            let label = size.map_or(name.clone(), |(width, height)| {
                format!("{name} · {width}×{height}")
            });
            items.push(ScreenChoice::Monitor(label, monitor));
        }
        for window in windows.into_iter().take(96) {
            let title = window.title().unwrap_or_default();
            if title.trim().is_empty() {
                continue;
            }
            let process = window
                .process_name()
                .unwrap_or_else(|_| "Application".into());
            items.push(ScreenChoice::Window(format!("{process} · {title}"), window));
        }
        if items.is_empty() {
            Err("no windows or displays are available to share".into())
        } else {
            Ok(items)
        }
    }
}

#[cfg(target_os = "linux")]
mod native {
    use std::path::PathBuf;

    use image::ImageReader;
    use linuxvideo::format::{PixFormat, PixelFormat};
    use linuxvideo::stream::ReadStream;
    use linuxvideo::{BufType, CapabilityFlags, Device as VideoDevice, Fract};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder};
    use x11rb::rust_connection::RustConnection;

    use super::*;

    pub struct CameraSource {
        stream: ReadStream,
        format: PixelFormat,
        width: u32,
        height: u32,
    }

    impl CameraSource {
        pub fn open(selection: Option<usize>) -> Result<Self, String> {
            let devices = camera_devices();
            let (_, path) = devices
                .get(selection.unwrap_or(0))
                .ok_or_else(|| "no compatible V4L2 camera is available".to_string())?;
            let device = VideoDevice::open(path).map_err(|error| error.to_string())?;
            let formats = device
                .formats(BufType::VIDEO_CAPTURE)
                .filter_map(Result::ok)
                .map(|format| format.pixel_format())
                .collect::<Vec<_>>();
            let requested = [
                PixelFormat::MJPG,
                PixelFormat::JPEG,
                PixelFormat::YUYV,
                PixelFormat::RGB3,
                PixelFormat::BGR3,
            ]
            .into_iter()
            .find(|format| formats.contains(format))
            .ok_or_else(|| "the camera has no supported MJPEG, YUYV, or RGB format".to_string())?;
            let capture = device
                .video_capture(PixFormat::new(VIDEO_WIDTH, VIDEO_HEIGHT, requested))
                .map_err(|error| error.to_string())?;
            let format = capture.format().pixel_format();
            let width = capture.format().width();
            let height = capture.format().height();
            if !formats.contains(&format)
                || u64::from(width) * u64::from(height) > MAX_CAPTURE_PIXELS
            {
                return Err("the camera negotiated an unsupported capture format".into());
            }
            let _ = capture.set_frame_interval(Fract::new(1, VIDEO_FPS));
            let stream = capture.into_stream().map_err(|error| error.to_string())?;
            Ok(Self {
                stream,
                format,
                width,
                height,
            })
        }

        pub fn frame(&mut self) -> Result<Option<RawFrame>, String> {
            if self
                .stream
                .will_block()
                .map_err(|error| error.to_string())?
            {
                return Ok(None);
            }
            let bytes = self
                .stream
                .dequeue(|buffer| Ok(buffer.to_vec()))
                .map_err(|error| error.to_string())?;
            let (width, height, rgba) =
                if matches!(self.format, PixelFormat::MJPG | PixelFormat::JPEG) {
                    let image = ImageReader::new(std::io::Cursor::new(bytes))
                        .with_guessed_format()
                        .map_err(|error| error.to_string())?
                        .decode()
                        .map_err(|error| error.to_string())?
                        .to_rgba8();
                    let (width, height) = image.dimensions();
                    (width, height, image.into_raw())
                } else {
                    (
                        self.width,
                        self.height,
                        packed_camera_to_rgba(&bytes, self.width, self.height, self.format)?,
                    )
                };
            valid_frame(width, height, rgba.len())?;
            Ok(Some(RawFrame {
                width,
                height,
                rgba,
            }))
        }
    }

    fn camera_devices() -> Vec<(String, PathBuf)> {
        linuxvideo::list().map_or_else(
            |_| Vec::new(),
            |devices| {
                devices
                    .filter_map(Result::ok)
                    .filter_map(|device| {
                        let capabilities = device.capabilities().ok()?;
                        if !capabilities
                            .device_capabilities()
                            .contains(CapabilityFlags::VIDEO_CAPTURE)
                        {
                            return None;
                        }
                        let path = device.path().ok()?;
                        Some((capabilities.card().to_string(), path))
                    })
                    .collect()
            },
        )
    }

    pub fn camera_names() -> Vec<String> {
        camera_devices().into_iter().map(|(name, _)| name).collect()
    }

    fn packed_camera_to_rgba(
        bytes: &[u8],
        width: u32,
        height: u32,
        format: PixelFormat,
    ) -> Result<Vec<u8>, String> {
        let pixels = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| "camera dimensions overflow".to_string())?;
        if matches!(format, PixelFormat::RGB3 | PixelFormat::BGR3) {
            if bytes.len() < pixels.saturating_mul(3) {
                return Err("camera returned a truncated RGB frame".into());
            }
            let mut rgba = Vec::with_capacity(pixels * 4);
            for pixel in bytes[..pixels * 3].chunks_exact(3) {
                let (r, g, b) = if format == PixelFormat::RGB3 {
                    (pixel[0], pixel[1], pixel[2])
                } else {
                    (pixel[2], pixel[1], pixel[0])
                };
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
            return Ok(rgba);
        }
        if format != PixelFormat::YUYV || width % 2 != 0 || bytes.len() < pixels.saturating_mul(2) {
            return Err("camera returned an unsupported packed frame".into());
        }
        let mut rgba = Vec::with_capacity(pixels * 4);
        for pair in bytes[..pixels * 2].chunks_exact(4) {
            let u = pair[1];
            let v = pair[3];
            rgba.extend_from_slice(&yuv_pixel(pair[0], u, v));
            rgba.extend_from_slice(&yuv_pixel(pair[2], u, v));
        }
        Ok(rgba)
    }

    fn yuv_pixel(y: u8, u: u8, v: u8) -> [u8; 4] {
        let y = i32::from(y).saturating_sub(16);
        let u = i32::from(u) - 128;
        let v = i32::from(v) - 128;
        [
            super::super::clamp((298 * y + 409 * v + 128) >> 8),
            super::super::clamp((298 * y - 100 * u - 208 * v + 128) >> 8),
            super::super::clamp((298 * y + 516 * u + 128) >> 8),
            255,
        ]
    }

    struct PixelLayout {
        bits_per_pixel: u8,
        scanline_pad: u8,
        little_endian: bool,
        red_mask: u32,
        green_mask: u32,
        blue_mask: u32,
    }

    pub struct ScreenSource {
        connection: RustConnection,
        root: u32,
        width: u16,
        height: u16,
        layout: PixelLayout,
    }

    pub struct ScreenChoice {
        label: String,
    }

    pub fn screen_choice_labels(choices: &[ScreenChoice]) -> Vec<String> {
        choices.iter().map(|choice| choice.label.clone()).collect()
    }

    impl ScreenSource {
        pub fn open_picker() -> Result<Self, String> {
            Err("Linux uses the in-app X11 desktop source selector".into())
        }

        pub fn open_source(_choice: &ScreenChoice) -> Result<Self, String> {
            if wayland_session() {
                return Err(
                    "Wayland screen sharing requires the XDG ScreenCast portal and PipeWire".into(),
                );
            }
            let (connection, screen_index) =
                x11rb::connect(None).map_err(|error| error.to_string())?;
            let setup = connection.setup();
            let screen = setup
                .roots
                .get(screen_index)
                .ok_or_else(|| "the X11 desktop is unavailable".to_string())?;
            let visual = screen
                .allowed_depths
                .iter()
                .flat_map(|depth| &depth.visuals)
                .find(|visual| visual.visual_id == screen.root_visual)
                .ok_or_else(|| "the X11 root visual is unavailable".to_string())?;
            let format = setup
                .pixmap_formats
                .iter()
                .find(|format| format.depth == screen.root_depth)
                .ok_or_else(|| "the X11 root pixel format is unavailable".to_string())?;
            if !matches!(format.bits_per_pixel, 24 | 32) {
                return Err("the X11 root pixel format is unsupported".into());
            }
            let layout = PixelLayout {
                bits_per_pixel: format.bits_per_pixel,
                scanline_pad: format.scanline_pad,
                little_endian: setup.image_byte_order == ImageOrder::LSB_FIRST,
                red_mask: visual.red_mask,
                green_mask: visual.green_mask,
                blue_mask: visual.blue_mask,
            };
            let (root, width, height) =
                (screen.root, screen.width_in_pixels, screen.height_in_pixels);
            if u64::from(width) * u64::from(height) > MAX_CAPTURE_PIXELS {
                return Err("the X11 desktop is too large to capture safely".into());
            }
            Ok(Self {
                connection,
                root,
                width,
                height,
                layout,
            })
        }

        pub fn frame(&self) -> Result<Option<RawFrame>, String> {
            let reply = self
                .connection
                .get_image(
                    ImageFormat::Z_PIXMAP,
                    self.root,
                    0,
                    0,
                    self.width,
                    self.height,
                    u32::MAX,
                )
                .map_err(|error| error.to_string())?
                .reply()
                .map_err(|error| error.to_string())?;
            let bytes_per_pixel = usize::from(self.layout.bits_per_pixel / 8);
            let row_bits = usize::from(self.width) * usize::from(self.layout.bits_per_pixel);
            let pad = usize::from(self.layout.scanline_pad);
            let stride = row_bits.div_ceil(pad) * pad / 8;
            if reply.data.len() < stride.saturating_mul(usize::from(self.height)) {
                return Err("X11 returned a truncated desktop frame".into());
            }
            let mut rgba =
                Vec::with_capacity(usize::from(self.width) * usize::from(self.height) * 4);
            for row in reply.data.chunks(stride).take(usize::from(self.height)) {
                for pixel in
                    row[..usize::from(self.width) * bytes_per_pixel].chunks_exact(bytes_per_pixel)
                {
                    let value = if self.layout.little_endian {
                        let mut bytes = [0; 4];
                        bytes[..bytes_per_pixel].copy_from_slice(pixel);
                        u32::from_le_bytes(bytes)
                    } else {
                        let mut bytes = [0; 4];
                        bytes[4 - bytes_per_pixel..].copy_from_slice(pixel);
                        u32::from_be_bytes(bytes)
                    };
                    rgba.extend_from_slice(&[
                        channel(value, self.layout.red_mask),
                        channel(value, self.layout.green_mask),
                        channel(value, self.layout.blue_mask),
                        255,
                    ]);
                }
            }
            Ok(Some(RawFrame {
                width: u32::from(self.width),
                height: u32::from(self.height),
                rgba,
            }))
        }
    }

    fn channel(pixel: u32, mask: u32) -> u8 {
        if mask == 0 {
            return 0;
        }
        let value = (pixel & mask) >> mask.trailing_zeros();
        let max = mask >> mask.trailing_zeros();
        ((u64::from(value) * 255) / u64::from(max)) as u8
    }

    fn wayland_session() -> bool {
        std::env::var_os("WAYLAND_DISPLAY").is_some()
            && std::env::var("XDG_SESSION_TYPE").is_ok_and(|session| session != "x11")
    }

    pub fn selectable_screen_sources() -> Result<Option<Vec<ScreenChoice>>, String> {
        if wayland_session() {
            return Err(
                "Wayland screen sharing requires the XDG ScreenCast portal and PipeWire".into(),
            );
        }
        let (connection, screen_index) = x11rb::connect(None).map_err(|error| error.to_string())?;
        let screen = connection
            .setup()
            .roots
            .get(screen_index)
            .ok_or_else(|| "the X11 desktop is unavailable".to_string())?;
        Ok(Some(vec![ScreenChoice {
            label: format!(
                "X11 desktop · {}×{}",
                screen.width_in_pixels, screen.height_in_pixels
            ),
        }]))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn packed_yuyv_conversion_is_bounded() {
            let rgba =
                packed_camera_to_rgba(&[16, 128, 235, 128], 2, 1, PixelFormat::YUYV).unwrap();
            assert_eq!(rgba.len(), 8);
            assert_eq!(&rgba[..4], &[0, 0, 0, 255]);
            assert!(rgba[4] > 250 && rgba[5] > 250 && rgba[6] > 250);
            assert!(packed_camera_to_rgba(&[0; 3], 2, 1, PixelFormat::YUYV).is_err());
        }
    }
}

use native::{
    CameraSource, ScreenChoice, ScreenSource, camera_names, screen_choice_labels,
    selectable_screen_sources,
};
