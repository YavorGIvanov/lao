use lao_optimize_api::{Optimize, Plan, Probe, Start, State};
use std::{
    io,
    panic::{self, AssertUnwindSafe},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread,
};

const IDLE: u8 = 0;
const WARMING: u8 = 1;
const READY: u8 = 2;
const FAILED: u8 = 3;

#[derive(Default)]
pub struct Optimizer {
    state: Arc<AtomicU8>,
}

impl Optimize for Optimizer {
    fn start(&self, plan: Plan) -> io::Result<Start> {
        let previous = loop {
            let state = self.state.load(Ordering::Acquire);
            if state == WARMING {
                return Ok(Start::Busy);
            }
            if self
                .state
                .compare_exchange(state, WARMING, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break state;
            }
        };

        let state = Arc::clone(&self.state);
        match thread::Builder::new()
            .name("lao-warm".into())
            .spawn(move || warm(state, plan))
        {
            Ok(_) => Ok(Start::Started),
            Err(error) => {
                self.state.store(previous, Ordering::Release);
                Err(error)
            }
        }
    }

    fn state(&self) -> State {
        match self.state.load(Ordering::Acquire) {
            IDLE => State::Idle,
            WARMING => State::Warming,
            READY => State::Ready,
            FAILED => State::Failed,
            _ => unreachable!(),
        }
    }
}

fn warm(state: Arc<AtomicU8>, plan: Plan) {
    let (claude, codex) = plan.into_probes();
    let claude = probe(claude);
    let codex = probe(codex);
    state.store(
        if claude && codex { READY } else { FAILED },
        Ordering::Release,
    );
}

fn probe(probe: Probe) -> bool {
    panic::catch_unwind(AssertUnwindSafe(probe)).is_ok_and(|result| result.is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Mutex, mpsc},
        time::{Duration, Instant},
    };

    #[test]
    fn warm_is_single_flight_and_claude_runs_first() {
        let optimizer = Optimizer::default();
        let order = Arc::new(Mutex::new(Vec::new()));
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let claude_order = Arc::clone(&order);
        let codex_order = Arc::clone(&order);
        let plan = Plan::new(
            move || {
                claude_order.lock().unwrap().push("claude");
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            },
            move || {
                codex_order.lock().unwrap().push("codex");
                Ok(())
            },
        );

        assert_eq!(optimizer.start(plan).unwrap(), Start::Started);
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(
            optimizer.start(Plan::new(|| Ok(()), || Ok(()))).unwrap(),
            Start::Busy
        );
        release_tx.send(()).unwrap();
        wait_for(&optimizer, State::Ready);
        assert_eq!(*order.lock().unwrap(), ["claude", "codex"]);
    }

    #[test]
    fn a_failed_probe_does_not_skip_codex_or_block_retry() {
        let optimizer = Optimizer::default();
        let codex_runs = Arc::new(AtomicU8::new(0));
        let runs = Arc::clone(&codex_runs);
        optimizer
            .start(Plan::new(
                || Err(io::Error::other("failed")),
                move || {
                    runs.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                },
            ))
            .unwrap();
        wait_for(&optimizer, State::Failed);
        assert_eq!(codex_runs.load(Ordering::Relaxed), 1);

        assert_eq!(
            optimizer.start(Plan::new(|| Ok(()), || Ok(()))).unwrap(),
            Start::Started
        );
        wait_for(&optimizer, State::Ready);
    }

    fn wait_for(optimizer: &Optimizer, state: State) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while optimizer.state() != state {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
    }
}
