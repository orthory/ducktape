use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, Stream, StreamConfig};
use nokhwa::Camera;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
};
use objc2_foundation::{NSOperatingSystemVersion, NSProcessInfo};
use screencapturekit::content_sharing_picker::{
    SCContentSharingPicker, SCContentSharingPickerConfiguration, SCContentSharingPickerMode,
    SCPickerFilterOutcome,
};
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;
use tokio::sync::mpsc::error::TryRecvError;

use super::*;
use crate::huddle_session::{DriverIncoming, DriverOutgoing, FRAME_SAMPLES, SAMPLE_RATE};

const AUDIO_PLAYBACK_QUEUE: usize = 8;
const SCREEN_FRAME_QUEUE: usize = 2;
const LOOP_INTERVAL: Duration = Duration::from_millis(5);
const VIDEO_INTERVAL: Duration = Duration::from_millis(1000 / VIDEO_FPS as u64);

pub struct Backend;

impl MediaBackend for Backend {
    fn run(
        mut port: MediaDriverPort,
        commands: Receiver<Command>,
        events: SyncSender<Event>,
        running: Arc<AtomicBool>,
        level: Arc<AtomicU8>,
    ) {
        // A fresh huddle must never be a hot-mic moment. The shell can unmute
        // deliberately after capture is ready, but the callback starts closed.
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
                let kind = classify_error(
                    &error,
                    FailureKind::MicrophoneDenied,
                    FailureKind::MicrophoneUnavailable,
                );
                emit(
                    &events,
                    Event::Failed {
                        kind,
                        detail: error,
                    },
                );
                emit(&events, Event::Stopped);
                return;
            }
        };
        let mut audio = Some(audio);
        let mut playback_tx = playback_tx;
        let mut screen_sources = Vec::<(String, SCContentFilter)>::new();
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
                        match CameraSource::open(camera_device, &running) {
                            Ok(camera) => {
                                source = VideoSource::Camera(camera);
                                camera_on = true;
                                force_keyframe = true;
                            }
                            Err(error) => {
                                if !running.load(Ordering::Acquire) {
                                    break;
                                }
                                emit_video_failure(
                                    &events,
                                    error,
                                    FailureKind::CameraDenied,
                                    FailureKind::CameraUnavailable,
                                );
                            }
                        }
                    }
                    emit(&events, Event::VideoState { camera_on, sharing });
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
                }
                Ok(Command::SetScreenShare(value)) => {
                    if value == sharing && !camera_on {
                        continue;
                    }
                    if value && !macos_14_or_newer() {
                        match enumerate_screen_sources() {
                            Ok(sources) => {
                                screen_sources = sources;
                                screen_source = None;
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
                            }
                            Err(error) => emit_video_failure(
                                &events,
                                error,
                                FailureKind::ScreenDenied,
                                FailureKind::ScreenUnavailable,
                            ),
                        }
                        continue;
                    }
                    if value {
                        resume_camera_after_share = camera_on;
                    }
                    stop_source(&mut source);
                    camera_on = false;
                    sharing = false;
                    if value {
                        match ScreenSource::open_picker(&running) {
                            Ok(screen) => {
                                source = VideoSource::Screen(screen);
                                sharing = true;
                                force_keyframe = true;
                            }
                            Err(error) => {
                                if !running.load(Ordering::Acquire) {
                                    break;
                                }
                                emit_video_failure(
                                    &events,
                                    error,
                                    FailureKind::ScreenDenied,
                                    FailureKind::ScreenUnavailable,
                                );
                                if resume_camera_after_share
                                    && let Ok(camera) = CameraSource::open(camera_device, &running)
                                {
                                    source = VideoSource::Camera(camera);
                                    camera_on = true;
                                    force_keyframe = true;
                                }
                                resume_camera_after_share = false;
                            }
                        }
                    } else if resume_camera_after_share {
                        if let Ok(camera) = CameraSource::open(camera_device, &running) {
                            source = VideoSource::Camera(camera);
                            camera_on = true;
                            force_keyframe = true;
                        }
                        resume_camera_after_share = false;
                    }
                    screen_sources.clear();
                    screen_source = None;
                    emit(&events, Event::VideoState { camera_on, sharing });
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
                }
                Ok(Command::RefreshDevices) => emit(
                    &events,
                    Event::Devices(enumerate_devices(
                        microphone,
                        camera_device,
                        speaker,
                        &screen_sources,
                        screen_source,
                    )),
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
                            emit_video_failure(
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
                            } else {
                                emit(
                                    &events,
                                    Event::Failed {
                                        kind: FailureKind::MicrophoneUnavailable,
                                        detail: "the previous microphone could not be restored"
                                            .into(),
                                    },
                                );
                                running.store(false, Ordering::Release);
                            }
                        }
                    }
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
                            emit_video_failure(
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
                            } else {
                                emit(
                                    &events,
                                    Event::Failed {
                                        kind: FailureKind::MicrophoneUnavailable,
                                        detail: "the previous speaker could not be restored".into(),
                                    },
                                );
                                running.store(false, Ordering::Release);
                            }
                        }
                    }
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
                }
                Ok(Command::SetCameraDevice(selection)) => {
                    let old = camera_device;
                    camera_device = selection;
                    if camera_on {
                        stop_source(&mut source);
                        match CameraSource::open(camera_device, &running) {
                            Ok(camera) => {
                                source = VideoSource::Camera(camera);
                                force_keyframe = true;
                            }
                            Err(error) => {
                                if !running.load(Ordering::Acquire) {
                                    break;
                                }
                                camera_device = old;
                                emit_video_failure(
                                    &events,
                                    error,
                                    FailureKind::DeviceSelection,
                                    FailureKind::DeviceSelection,
                                );
                                match CameraSource::open(camera_device, &running) {
                                    Ok(camera) => {
                                        source = VideoSource::Camera(camera);
                                        camera_on = true;
                                        force_keyframe = true;
                                    }
                                    Err(error) => {
                                        camera_on = false;
                                        emit_video_failure(
                                            &events,
                                            error,
                                            FailureKind::CameraDenied,
                                            FailureKind::CameraUnavailable,
                                        );
                                    }
                                }
                                emit(&events, Event::VideoState { camera_on, sharing });
                            }
                        }
                    }
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
                }
                Ok(Command::SetScreenSource(selection)) => {
                    let Some(index) = selection else {
                        continue;
                    };
                    let Some((_, filter)) = screen_sources.get(index) else {
                        emit_video_failure(
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
                    match ScreenSource::open_filter(filter) {
                        Ok(screen) => {
                            source = VideoSource::Screen(screen);
                            sharing = true;
                            screen_source = Some(index);
                            force_keyframe = true;
                        }
                        Err(error) => {
                            emit_video_failure(
                                &events,
                                error,
                                FailureKind::ScreenDenied,
                                FailureKind::ScreenUnavailable,
                            );
                            if resume_camera_after_share
                                && let Ok(camera) = CameraSource::open(camera_device, &running)
                            {
                                source = VideoSource::Camera(camera);
                                camera_on = true;
                                force_keyframe = true;
                            }
                            resume_camera_after_share = false;
                        }
                    }
                    emit(&events, Event::VideoState { camera_on, sharing });
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
                            let request = keyframe_requests.get(&peer).is_none_or(|last| {
                                now.duration_since(*last) >= Duration::from_secs(2)
                            });
                            if request {
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
                        emit_video_failure(&events, error, denied, unavailable);
                        stop_source(&mut source);
                        camera_on = false;
                        sharing = false;
                        if was_sharing
                            && resume_camera_after_share
                            && let Ok(camera) = CameraSource::open(camera_device, &running)
                        {
                            source = VideoSource::Camera(camera);
                            camera_on = true;
                            force_keyframe = true;
                        }
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

type OpenAudioResult = Result<(AudioStreams, SyncSender<Box<[i16; FRAME_SAMPLES]>>), String>;

fn open_audio(
    muted: Arc<AtomicBool>,
    level: Arc<AtomicU8>,
    outgoing: tokio::sync::mpsc::Sender<DriverOutgoing>,
    microphone: Option<usize>,
    speaker: Option<usize>,
) -> OpenAudioResult {
    let host = cpal::default_host();
    let input_device = match microphone {
        Some(index) => host
            .input_devices()
            .map_err(|error| error.to_string())?
            .nth(index),
        None => host.default_input_device(),
    }
    .ok_or_else(|| "the selected microphone is no longer available".to_string())?;
    let output_device = match speaker {
        Some(index) => host
            .output_devices()
            .map_err(|error| error.to_string())?
            .nth(index),
        None => host.default_output_device(),
    }
    .ok_or_else(|| "the selected speaker is no longer available".to_string())?;
    let input_config = audio_config(&input_device, true)?;
    let output_config = audio_config(&output_device, false)?;
    let input_channels = usize::from(input_config.channels);
    let output_channels = usize::from(output_config.channels);
    let (playback_tx, playback_rx) = sync_channel(AUDIO_PLAYBACK_QUEUE);
    let mut captured = Box::new([0i16; FRAME_SAMPLES]);
    let mut capture_offset = 0usize;
    let input_stream = input_device
        .build_input_stream::<f32, _, _>(
            input_config,
            move |input, _| {
                let mut square_sum = 0.0f64;
                let mut frame_count = 0usize;
                for frame in input.chunks(input_channels) {
                    let mono = frame.iter().copied().sum::<f32>() / frame.len().max(1) as f32;
                    square_sum += f64::from(mono) * f64::from(mono);
                    frame_count += 1;
                    if muted.load(Ordering::Acquire) {
                        continue;
                    }
                    captured[capture_offset] =
                        (mono.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
                    capture_offset += 1;
                    if capture_offset == FRAME_SAMPLES {
                        let frame =
                            std::mem::replace(&mut captured, Box::new([0i16; FRAME_SAMPLES]));
                        capture_offset = 0;
                        let _ = outgoing.try_send(DriverOutgoing::Audio(frame));
                    }
                }
                let rms = (square_sum / frame_count.max(1) as f64).sqrt();
                // Match the web huddle: 0.25 RMS fills the meter and 8%
                // corresponds to its -34 dBFS speaking threshold.
                level.store((rms * 400.0).min(100.0).round() as u8, Ordering::Release);
                if muted.load(Ordering::Acquire) {
                    capture_offset = 0;
                }
            },
            |error| {
                tracing::debug!(
                    target: "ducktape::huddle",
                    event = "audio_capture_failed",
                    reason = "stream_error",
                    detail = %error
                );
            },
            None,
        )
        .map_err(|error| error.to_string())?;

    let mut playing: Option<Box<[i16; FRAME_SAMPLES]>> = None;
    let mut play_offset = 0usize;
    let output_stream = output_device
        .build_output_stream::<f32, _, _>(
            output_config,
            move |output, _| {
                for output_frame in output.chunks_mut(output_channels) {
                    let sample = loop {
                        if let Some(frame) = &playing {
                            let sample = frame[play_offset];
                            play_offset += 1;
                            if play_offset == FRAME_SAMPLES {
                                playing = None;
                            }
                            break sample;
                        }
                        playing = playback_rx.try_recv().ok();
                        play_offset = 0;
                        if playing.is_none() {
                            break 0;
                        }
                    };
                    output_frame.fill(f32::from(sample) / 32_768.0);
                }
            },
            |error| {
                tracing::debug!(
                    target: "ducktape::huddle",
                    event = "audio_playback_failed",
                    reason = "stream_error",
                    detail = %error
                );
            },
            None,
        )
        .map_err(|error| error.to_string())?;
    input_stream.play().map_err(|error| error.to_string())?;
    output_stream.play().map_err(|error| error.to_string())?;
    Ok((
        AudioStreams {
            _input: input_stream,
            _output: output_stream,
        },
        playback_tx,
    ))
}

struct AudioStreams {
    _input: Stream,
    _output: Stream,
}

fn audio_config(device: &cpal::Device, input: bool) -> Result<StreamConfig, String> {
    let configs = if input {
        device
            .supported_input_configs()
            .map(|configs| configs.collect::<Vec<_>>())
    } else {
        device
            .supported_output_configs()
            .map(|configs| configs.collect::<Vec<_>>())
    }
    .map_err(|error| error.to_string())?;
    let supported = configs
        .into_iter()
        .filter(|config| {
            config.sample_format() == SampleFormat::F32
                && config.min_sample_rate() <= SAMPLE_RATE
                && config.max_sample_rate() >= SAMPLE_RATE
        })
        .max_by_key(|config| (config.channels() == 1, std::cmp::Reverse(config.channels())))
        .ok_or_else(|| "the audio device does not support 48 kHz PCM".to_string())?;
    let mut config: StreamConfig = supported.with_sample_rate(SAMPLE_RATE).into();
    // Device callbacks may choose any practical quantum; the accumulator above
    // still emits exact protocol frames of 960 mono samples.
    config.buffer_size = BufferSize::Default;
    Ok(config)
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
            Self::Camera(camera) => camera.frame().map(Some),
            Self::Screen(screen) => Ok(screen.frame()),
        }
    }
}

fn stop_source(source: &mut VideoSource) {
    *source = VideoSource::Off;
}

struct CameraSource(Camera);

impl CameraSource {
    fn open(selection: Option<usize>, running: &AtomicBool) -> Result<Self, String> {
        if !nokhwa::nokhwa_check() {
            let (tx, rx) = sync_channel(1);
            nokhwa::nokhwa_initialize(move |allowed| {
                let _ = tx.try_send(allowed);
            });
            let deadline = Instant::now() + Duration::from_secs(120);
            loop {
                if !running.load(Ordering::Acquire) {
                    return Err("camera permission request was cancelled".into());
                }
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(true) => break,
                    Ok(false) => return Err("camera access was not authorized".into()),
                    Err(RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                    Err(RecvTimeoutError::Timeout) => {
                        return Err("camera permission request timed out".into());
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err("camera permission request was interrupted".into());
                    }
                }
            }
        }
        let requested =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(CameraFormat::new(
                Resolution::new(VIDEO_WIDTH, VIDEO_HEIGHT),
                FrameFormat::MJPEG,
                VIDEO_FPS,
            )));
        let index = if let Some(index) = selection {
            nokhwa::query(nokhwa::utils::ApiBackend::AVFoundation)
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

    fn frame(&mut self) -> Result<RawFrame, String> {
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
        Ok(RawFrame {
            width,
            height,
            rgba,
        })
    }
}

fn enumerate_devices(
    microphone: Option<usize>,
    camera: Option<usize>,
    speaker: Option<usize>,
    screen_sources: &[(String, SCContentFilter)],
    screen_source: Option<usize>,
) -> DeviceOptions {
    let mut options = DeviceOptions {
        microphone,
        camera,
        speaker,
        screen_sources: screen_sources
            .iter()
            .map(|(label, _)| label.clone())
            .collect(),
        screen_source,
        ..DeviceOptions::default()
    };
    let host = cpal::default_host();
    options.speakers = host
        .output_devices()
        .map(|devices| devices.map(|device| device.to_string()).collect())
        .unwrap_or_default();
    options.microphones = host
        .input_devices()
        .map(|devices| devices.map(|device| device.to_string()).collect())
        .unwrap_or_default();
    if let Ok(cameras) = nokhwa::query(nokhwa::utils::ApiBackend::AVFoundation) {
        options.cameras = cameras
            .into_iter()
            .map(|camera| camera.human_name())
            .collect();
    }
    options
}

fn macos_14_or_newer() -> bool {
    NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
        majorVersion: 14,
        minorVersion: 0,
        patchVersion: 0,
    })
}

fn enumerate_screen_sources() -> Result<Vec<(String, SCContentFilter)>, String> {
    let content = SCShareableContent::get().map_err(|error| error.to_string())?;
    let mut sources = Vec::new();
    for (position, display) in content.displays().into_iter().enumerate() {
        let frame = display.frame();
        sources.push((
            format!(
                "Display {} · {:.0}×{:.0}",
                position + 1,
                frame.size.width,
                frame.size.height
            ),
            SCContentFilter::create()
                .with_display(&display)
                .with_excluding_windows(&[])
                .build(),
        ));
    }
    for window in content
        .windows()
        .into_iter()
        .filter(|window| window.is_on_screen() && window.window_layer() == 0)
        .take(96)
    {
        let title = window
            .title()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Untitled window".into());
        let app = window
            .owning_application()
            .map_or_else(|| "Application".into(), |app| app.application_name());
        sources.push((
            format!("{app} · {title}"),
            SCContentFilter::create().with_window(&window).build(),
        ));
    }
    if sources.is_empty() {
        Err("no windows or displays are available to share".into())
    } else {
        Ok(sources)
    }
}

struct ScreenSource {
    stream: SCStream,
    frames: Receiver<RawFrame>,
}

impl ScreenSource {
    fn open_picker(running: &AtomicBool) -> Result<Self, String> {
        let mut config = SCContentSharingPickerConfiguration::new();
        config.set_allowed_picker_modes(&[
            SCContentSharingPickerMode::SingleWindow,
            SCContentSharingPickerMode::SingleDisplay,
            SCContentSharingPickerMode::SingleApplication,
        ]);
        let (tx, rx) = sync_channel(1);
        SCContentSharingPicker::show_filter(&config, move |outcome| {
            let selected = match outcome {
                SCPickerFilterOutcome::Filter(filter) => Ok(filter),
                SCPickerFilterOutcome::Cancelled => Err("screen sharing was cancelled".into()),
                SCPickerFilterOutcome::Error(error) => Err(error),
            };
            let _ = tx.try_send(selected);
        });
        let deadline = Instant::now() + Duration::from_secs(120);
        let outcome = loop {
            if !running.load(Ordering::Acquire) {
                break Err("screen source selection was cancelled".into());
            }
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(outcome) => break outcome,
                Err(RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                Err(RecvTimeoutError::Timeout) => {
                    break Err("screen source selection timed out".into());
                }
                Err(RecvTimeoutError::Disconnected) => {
                    break Err("screen source selection was interrupted".into());
                }
            }
        };
        // The picker is process-global. Leaving it active keeps the system
        // sharing UI in a ready state even after this huddle has gone away.
        SCContentSharingPicker::set_active(false);
        let filter = outcome?;
        Self::open_filter(&filter)
    }

    fn open_filter(filter: &SCContentFilter) -> Result<Self, String> {
        let interval = CMTime::new(1, VIDEO_FPS as i32);
        let config = SCStreamConfiguration::new()
            .with_width(VIDEO_WIDTH)
            .with_height(VIDEO_HEIGHT)
            .with_pixel_format(PixelFormat::BGRA)
            .with_shows_cursor(true)
            .with_queue_depth(2)
            .with_minimum_frame_interval(&interval);
        let (tx, frames) = sync_channel(SCREEN_FRAME_QUEUE);
        let mut stream = SCStream::new(filter, &config);
        stream.add_output_handler(
            move |sample: CMSampleBuffer, output_type: SCStreamOutputType| {
                if output_type != SCStreamOutputType::Screen {
                    return;
                }
                let Some(buffer) = sample.image_buffer() else {
                    return;
                };
                let Ok(guard) = buffer.lock(CVPixelBufferLockFlags::READ_ONLY) else {
                    return;
                };
                let width = guard.width();
                let height = guard.height();
                let stride = guard.bytes_per_row();
                let source = guard.as_slice();
                if width == 0 || height == 0 || stride < width * 4 || source.len() < stride * height
                {
                    return;
                }
                let mut rgba = Vec::with_capacity(width * height * 4);
                for row in source.chunks(stride).take(height) {
                    for pixel in row[..width * 4].chunks_exact(4) {
                        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
                    }
                }
                let _ = tx.try_send(RawFrame {
                    width: width as u32,
                    height: height as u32,
                    rgba,
                });
            },
            SCStreamOutputType::Screen,
        );
        stream.start_capture().map_err(|error| error.to_string())?;
        Ok(Self { stream, frames })
    }

    fn frame(&self) -> Option<RawFrame> {
        let mut latest = self.frames.try_recv().ok()?;
        while let Ok(next) = self.frames.try_recv() {
            latest = next;
        }
        Some(latest)
    }
}

impl Drop for ScreenSource {
    fn drop(&mut self) {
        if let Err(error) = self.stream.stop_capture() {
            tracing::debug!(
                target: "ducktape::huddle",
                event = "screen_capture_stop_failed",
                reason = "capture_stop_failed",
                detail = %error
            );
        }
    }
}

fn emit_video_failure(
    events: &SyncSender<Event>,
    detail: String,
    denied: FailureKind,
    unavailable: FailureKind,
) {
    emit(
        events,
        Event::Failed {
            kind: classify_error(&detail, denied, unavailable),
            detail,
        },
    );
}

fn classify_error(detail: &str, denied: FailureKind, unavailable: FailureKind) -> FailureKind {
    let detail = detail.to_ascii_lowercase();
    if detail.contains("permission")
        || detail.contains("denied")
        || detail.contains("not authorized")
        || detail.contains("not authorised")
    {
        denied
    } else {
        unavailable
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
