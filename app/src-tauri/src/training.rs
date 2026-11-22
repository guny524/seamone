//! Automatic local work boundary. This build has no native optimizer, so jobs remain pending.
use crate::store::Store;
use seamone_core::review::EligibleExample;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
    mpsc,
};

pub trait LocalTrainer: Send {
    /// Produces a local candidate, never activates a production model or sends data.
    fn train(&mut self, example: &EligibleExample, audio: &[f32]) -> Result<String, String>;
}

#[derive(Default)]
pub struct Foreground {
    active: AtomicUsize,
    generation: AtomicU64,
}

pub struct ForegroundGuard(Arc<Foreground>);
impl Drop for ForegroundGuard {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Foreground {
    pub fn enter(self: &Arc<Self>) -> ForegroundGuard {
        self.active.fetch_add(1, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
        ForegroundGuard(Arc::clone(self))
    }
}

pub struct BackgroundWork {
    wake: Option<mpsc::Sender<()>>,
    pub foreground: Arc<Foreground>,
}

impl BackgroundWork {
    pub fn new(store: &Arc<Mutex<Store>>, trainer: Option<Box<dyn LocalTrainer>>) -> Self {
        let foreground = Arc::new(Foreground::default());
        let Some(mut trainer) = trainer else {
            return Self {
                wake: None,
                foreground,
            };
        };
        let (sender, receiver) = mpsc::channel();
        let store = Arc::downgrade(store);
        let activity = Arc::clone(&foreground);
        std::thread::spawn(move || {
            while let Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) =
                receiver.recv_timeout(std::time::Duration::from_secs(30))
            {
                let Some(store) = store.upgrade() else {
                    break;
                };
                // A failed or preempted job stays pending for a later wake/restart.
                while run_next(&store, &activity, trainer.as_mut()).unwrap_or(false) {}
            }
        });
        let work = Self {
            wake: Some(sender),
            foreground,
        };
        work.notify();
        work
    }

    pub fn notify(&self) {
        if let Some(wake) = &self.wake {
            let _ = wake.send(());
        }
    }
}

pub fn run_next(
    store: &Mutex<Store>,
    foreground: &Foreground,
    trainer: &mut dyn LocalTrainer,
) -> Result<bool, String> {
    if foreground.active.load(Ordering::SeqCst) != 0 {
        return Ok(false);
    }
    let generation = foreground.generation.load(Ordering::SeqCst);
    let (example, audio) = {
        let store = store.lock().map_err(|e| e.to_string())?;
        let Some(example) = store.next_job()? else {
            return Ok(false);
        };
        let s = example.segment();
        let info = store.recording(&s.recording_id)?;
        let bytes = match store
            .audio
            .range_wav(&s.recording_id, s.range, info.samples)
        {
            Ok(bytes) => bytes,
            Err(error) => {
                store.retry_job(&example, &error)?;
                return Err(error);
            }
        };
        let mut wav =
            hound::WavReader::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let audio = wav
            .samples::<i16>()
            .map(|s| {
                s.map(|v| f32::from(v) / 32_768.0)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        (example, audio)
    };
    let output = trainer.train(&example, &audio);
    let mut store = store.lock().map_err(|e| e.to_string())?;
    if foreground.active.load(Ordering::SeqCst) != 0
        || foreground.generation.load(Ordering::SeqCst) != generation
    {
        store.defer_job(&example)?;
        return Ok(false);
    }
    match output {
        Ok(candidate) => {
            if let Err(error) = store.publish(&example, &candidate) {
                store.retry_job(&example, &error)?;
                return Err(error);
            }
            Ok(true)
        }
        Err(error) => {
            store.defer_job(&example)?;
            Err(error)
        }
    }
}
