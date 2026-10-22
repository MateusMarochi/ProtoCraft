use eda_core::Project;
use eda_simulation::{SimulationControl, SimulationEngine, SimulationResult, TransientSession};
use std::{sync::mpsc, time::Duration};

pub enum Message {
    Update(Box<Project>, u64),
    Pause(bool),
    Step,
}
pub struct LiveWorker {
    pub sender: mpsc::Sender<Message>,
    pub receiver: mpsc::Receiver<Result<SimulationResult, String>>,
    pub control: SimulationControl,
    pub paused: bool,
    pub revision: u64,
}
impl Drop for LiveWorker {
    fn drop(&mut self) {
        self.control.cancel();
    }
}
pub fn start(
    project: Project,
    revision: u64,
    engine: SimulationEngine,
    step: f64,
    chunk: usize,
) -> Result<LiveWorker, String> {
    let mut session =
        TransientSession::new(project, revision, engine, step).map_err(|e| e.to_string())?;
    let (sender, commands) = mpsc::channel();
    let (results, receiver) = mpsc::sync_channel(1);
    let control = SimulationControl::default();
    let worker_control = control.clone();
    std::thread::Builder::new()
        .name("protocraft-continuous".into())
        .spawn(move || {
            let mut paused = false;
            let mut pending = None;
            loop {
                let mut one_step = false;
                for command in pending.take().into_iter().chain(commands.try_iter()) {
                    match command {
                        Message::Update(project, revision) => {
                            if let Err(e) = session.update_project(*project, revision) {
                                match results.send(Err(e.to_string())) {
                                    Ok(()) | Err(_) => return,
                                }
                            }
                        }
                        Message::Pause(value) => paused = value,
                        Message::Step => one_step = true,
                    }
                }
                // O cancelamento também é observado durante uma pausa.
                if worker_control.is_cancelled() {
                    return;
                }
                if !paused || one_step {
                    match session.advance(if one_step { 1 } else { chunk }, &worker_control) {
                        Ok(result) => match results.try_send(Ok(result)) {
                            Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
                            Err(mpsc::TrySendError::Disconnected(_)) => return,
                        },
                        Err(e) => match results.send(Err(e.to_string())) {
                            Ok(()) | Err(_) => return,
                        },
                    }
                }
                match commands.recv_timeout(Duration::from_millis(16)) {
                    Ok(command) => pending = Some(command),
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(LiveWorker {
        sender,
        receiver,
        control,
        paused: false,
        revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_pauses_steps_updates_revision_and_cancels_on_drop() {
        let project = eda_core::examples::rc_filter();
        let worker = start(project.clone(), 0, SimulationEngine::Dynamic, 1e-5, 8).unwrap();
        worker
            .receiver
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        worker.sender.send(Message::Pause(true)).unwrap();
        while worker
            .receiver
            .recv_timeout(Duration::from_millis(100))
            .is_ok()
        {}
        worker.sender.send(Message::Step).unwrap();
        let first = worker
            .receiver
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        worker.sender.send(Message::Step).unwrap();
        let second = worker
            .receiver
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        let time = |r: &SimulationResult| r.traces[0].samples.last().unwrap()[0];
        assert!((time(&second) - time(&first) - 1e-5).abs() < 1e-12);
        let mut edited = project;
        edited.name = "Editado ao vivo".into();
        worker
            .sender
            .send(Message::Update(Box::new(edited), 3))
            .unwrap();
        worker.sender.send(Message::Step).unwrap();
        let third = worker
            .receiver
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        assert_eq!(third.revision, 3);
        assert!(time(&third) > time(&second));
        let control = worker.control.clone();
        drop(worker);
        assert!(control.is_cancelled());
    }
}
