use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;
use common_infallible::{Condvar, Mutex};

struct WorkerNotify {
    waiting: Mutex<bool>,
    condvar: Condvar,
}

impl WorkerNotify {
    pub fn create() -> WorkerNotify {
        WorkerNotify {
            waiting: Mutex::new(false),
            condvar: Condvar::create(),
        }
    }
}

struct WorkersNotifyMutable {
    pub waiting_size: usize,
    pub workers_waiting: Vec<bool>,
}

pub struct WorkersNotify {
    mutable_state: Mutex<WorkersNotifyMutable>,
    workers_notify: Vec<WorkerNotify>,
}

impl WorkersNotify {
    pub fn create(workers: usize) -> Arc<WorkersNotify> {
        let mut workers_notify = Vec::with_capacity(workers);
        let mut workers_waiting = Vec::with_capacity(workers);

        for _index in 0..workers {
            workers_notify.push(WorkerNotify::create());
            workers_waiting.push(false);
        }

        Arc::new(WorkersNotify {
            workers_notify,
            mutable_state: Mutex::new(WorkersNotifyMutable { waiting_size: 0, workers_waiting }),
        })
    }

    pub fn is_empty(&self) -> bool {
        let mut mutable_state = self.mutable_state.lock();
        mutable_state.waiting_size == 0
    }

    pub fn wakeup(&self, worker_id: usize) {
        let mut mutable_state = self.mutable_state.lock();
        if mutable_state.waiting_size > 0 {
            mutable_state.waiting_size -= 1;

            if mutable_state.workers_waiting[worker_id] {
                mutable_state.workers_waiting[worker_id] = false;
                let mut waiting = self.workers_notify[worker_id].waiting.lock();

                *waiting = false;
                drop(mutable_state);
                self.workers_notify[worker_id].condvar.notify_one();
            } else {
                for (index, waiting) in mutable_state.workers_waiting.iter().enumerate() {
                    if *waiting {
                        mutable_state.workers_waiting[index] = false;
                        let mut waiting = self.workers_notify[index].waiting.lock();

                        *waiting = false;
                        drop(mutable_state);
                        self.workers_notify[index].condvar.notify_one();
                        return;
                    }
                }
            }
        }
    }

    pub fn wait(&self, worker_id: usize) {
        let mut mutable_state = self.mutable_state.lock();
        mutable_state.waiting_size += 1;
        mutable_state.workers_waiting[worker_id] = true;
        let mut waiting = self.workers_notify[worker_id].waiting.lock();

        *waiting = true;
        drop(mutable_state);
        self.workers_notify[worker_id].condvar.wait(&mut waiting);
    }
}

