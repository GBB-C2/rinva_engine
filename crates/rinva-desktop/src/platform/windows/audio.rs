//! Native Windows MediaPlayer audio. File opening and decoding never run in the UI callback.
use rinva_core::MusicState;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
use windows::{
    core::HSTRING,
    Foundation::TypedEventHandler,
    Media::{
        Core::MediaSource,
        Playback::{MediaPlaybackState, MediaPlayer},
    },
    Storage::StorageFile,
    Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
};

#[derive(Clone, Debug)]
pub struct Request {
    pub root: PathBuf,
    pub music: Option<MusicState>,
    pub voice: Option<String>,
    pub effects: Vec<String>,
    pub volumes: [u8; 3],
    pub fade_ms: u32,
}
enum Command {
    Play(Request),
    Volumes([u8; 3]),
    Stop,
    Quit,
    Probe(mpsc::Sender<String>),
}
pub struct Audio {
    tx: mpsc::Sender<Command>,
    errors: mpsc::Receiver<String>,
    voice_busy: Arc<AtomicBool>,
}
impl Audio {
    pub fn probe(&self) -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(Command::Probe(tx))
            .map_err(|e| e.to_string())?;
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|e| e.to_string())
    }
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let (errors_tx, errors) = mpsc::channel();
        let busy = Arc::new(AtomicBool::new(false));
        let voice_busy = Arc::clone(&busy);
        std::thread::spawn(move || {
            if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
                let _ = errors_tx.send(format!("Звук недоступен: {error}"));
                return;
            }
            worker(rx, errors_tx, busy);
            unsafe {
                RoUninitialize();
            }
        });
        Self {
            tx,
            errors,
            voice_busy,
        }
    }
    pub fn play(&self, request: Request) {
        self.voice_busy
            .store(request.voice.is_some(), Ordering::Relaxed);
        let _ = self.tx.send(Command::Play(request));
    }
    pub fn volumes(&self, volumes: [u8; 3]) {
        let _ = self.tx.send(Command::Volumes(volumes));
    }
    pub fn stop(&self) {
        self.voice_busy.store(false, Ordering::Relaxed);
        let _ = self.tx.send(Command::Stop);
    }
    pub fn voice_busy(&self) -> bool {
        self.voice_busy.load(Ordering::Relaxed)
    }
    pub fn error(&self) -> Option<String> {
        self.errors.try_recv().ok()
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Quit);
    }
}

struct Channel {
    player: MediaPlayer,
    source: MediaSource,
    path: PathBuf,
    started: Instant,
    gain: f64,
    from: f64,
    target: f64,
    fade: Duration,
    fade_at: Instant,
    base: f64,
}
impl Channel {
    fn open(
        path: PathBuf,
        repeat: bool,
        gain: f64,
        errors: &mpsc::Sender<String>,
    ) -> Result<Self, String> {
        let create = || -> windows::core::Result<Self> {
            let value = path.to_string_lossy();
            let value = if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
                format!("\\\\{unc}")
            } else {
                value.strip_prefix("\\\\?\\").unwrap_or(&value).to_owned()
            };
            let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(value))?.join()?;
            let source = MediaSource::CreateFromStorageFile(&file)?;
            let player = MediaPlayer::new()?;
            player.SetIsLoopingEnabled(repeat)?;
            player.SetVolume(gain)?;
            player.SetIsVideoFrameServerEnabled(false)?;
            let sender = errors.clone();
            player.MediaFailed(&TypedEventHandler::new(
                move |_,
                      args: windows::core::Ref<
                    windows::Media::Playback::MediaPlayerFailedEventArgs,
                >| {
                    let message = args
                        .as_ref()
                        .and_then(|a| a.ErrorMessage().ok())
                        .map(|s| s.to_string_lossy())
                        .unwrap_or_default();
                    let _ = sender.send(format!("Не удалось воспроизвести звук: {message}"));
                    Ok(())
                },
            ))?;
            // Disable shell media transport controls: these independent channels belong to the game.
            player.CommandManager()?.SetIsEnabled(false)?;
            player.SetSource(&source)?;
            player.Play()?;
            Ok(Self {
                player,
                source,
                path: path.clone(),
                started: Instant::now(),
                gain,
                from: gain,
                target: gain,
                fade: Duration::ZERO,
                fade_at: Instant::now(),
                base: 1.,
            })
        };
        create().map_err(|e| format!("Не удалось воспроизвести звук: {} — {e}", path.display()))
    }
    fn fade_to(&mut self, target: f64, millis: u32) {
        self.from = self.gain;
        self.target = target;
        self.fade = Duration::from_millis(millis.into());
        self.fade_at = Instant::now();
    }
    fn tick(&mut self, volume: u8) {
        let fraction = if self.fade.is_zero() {
            1.
        } else {
            (self.fade_at.elapsed().as_secs_f64() / self.fade.as_secs_f64()).min(1.)
        };
        self.gain = self.from + (self.target - self.from) * fraction;
        let _ = self
            .player
            .SetVolume((self.gain * self.base * f64::from(volume.min(100)) / 100.).clamp(0., 1.));
    }
    fn active(&self) -> bool {
        self.started.elapsed() < Duration::from_millis(500)
            || self
                .player
                .PlaybackSession()
                .and_then(|s| s.PlaybackState())
                .is_ok_and(|s| {
                    matches!(
                        s,
                        MediaPlaybackState::Opening
                            | MediaPlaybackState::Buffering
                            | MediaPlaybackState::Playing
                    )
                })
    }
}
impl Drop for Channel {
    fn drop(&mut self) {
        let _ = self.player.Close();
        let _ = self.source.Close();
    }
}

fn worker(rx: mpsc::Receiver<Command>, errors: mpsc::Sender<String>, busy: Arc<AtomicBool>) {
    let mut music: Option<Channel> = None;
    let mut fading: Vec<Channel> = vec![];
    let mut voice: Option<Channel> = None;
    let mut effects: Vec<Channel> = vec![];
    let mut volumes = [80, 80, 100];
    loop {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(Command::Probe(reply)) => {
                let _ = reply.send(format!(
                    "music={} voice={} effects={} volumes={:?} music_gain={:?} music_duration={:?}",
                    music.is_some(),
                    voice.is_some(),
                    effects.len(),
                    volumes,
                    music.as_ref().and_then(|c| c.player.Volume().ok()),
                    music
                        .as_ref()
                        .and_then(|c| c
                            .player
                            .PlaybackSession()
                            .and_then(|s| s.NaturalDuration())
                            .ok())
                        .map(|t| t.Duration)
                ));
            }
            Ok(Command::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Stop) => {
                music = None;
                fading.clear();
                voice = None;
                effects.clear();
            }
            Ok(Command::Volumes(value)) => volumes = value,
            Ok(Command::Play(request)) => {
                volumes = request.volumes;
                let desired = request.music.as_ref().and_then(|m| {
                    match rinva_core::storage::resolve_asset(&request.root, &m.file) {
                        Ok(path) => Some((path, m)),
                        Err(e) => {
                            let _ = errors.send(e);
                            None
                        }
                    }
                });
                if music.as_ref().map(|c| &c.path) != desired.as_ref().map(|(p, _)| p) {
                    if let Some(mut old) = music.take() {
                        old.fade_to(0., request.fade_ms);
                        fading.push(old);
                    }
                    if let Some((path, state)) = desired {
                        match Channel::open(path, true, 0., &errors) {
                            Ok(mut c) => {
                                c.base = f64::from(state.volume) / 100.;
                                c.fade_to(1., state.fade_ms);
                                music = Some(c);
                            }
                            Err(e) => {
                                let _ = errors.send(e);
                            }
                        }
                    }
                } else if let (Some(channel), Some((_, state))) = (&mut music, desired) {
                    channel.base = f64::from(state.volume) / 100.;
                }
                voice = None;
                if let Some(file) = request.voice {
                    match rinva_core::storage::resolve_asset(&request.root, &file)
                        .and_then(|p| Channel::open(p, false, 0., &errors))
                    {
                        Ok(mut c) => {
                            c.fade_to(1., 0);
                            voice = Some(c);
                        }
                        Err(e) => {
                            let _ = errors.send(e);
                        }
                    }
                }
                // Effects from action nodes are launched together on arriving at the next visible frame.
                for file in request.effects.into_iter().take(16) {
                    if effects.len() == 16 {
                        effects.remove(0);
                    }
                    match rinva_core::storage::resolve_asset(&request.root, &file)
                        .and_then(|p| Channel::open(p, false, 0., &errors))
                    {
                        Ok(mut c) => {
                            c.fade_to(1., 0);
                            effects.push(c);
                        }
                        Err(e) => {
                            let _ = errors.send(e);
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Some(c) = &mut music {
            c.tick(volumes[0]);
        }
        for c in &mut fading {
            c.tick(volumes[0]);
        }
        fading.retain(|c| c.gain > 0.0001);
        if let Some(c) = &mut voice {
            c.tick(volumes[2]);
        }
        busy.store(
            voice.as_ref().is_some_and(Channel::active),
            Ordering::Relaxed,
        );
        for c in &mut effects {
            c.tick(volumes[1]);
        }
        effects.retain(Channel::active);
    }
    busy.store(false, Ordering::Relaxed);
}
