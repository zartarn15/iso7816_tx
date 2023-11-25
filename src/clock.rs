type SleepCb = fn(u32);

pub struct Clock {
    timeout: u32,
    time: u32,
    sleep_cb: SleepCb,
}

impl Clock {
    pub fn new(timeout: u32, sleep_cb: SleepCb) -> Self {
        Self {
            timeout,
            time: 0,
            sleep_cb,
        }
    }

    pub fn sleep(&mut self, time: u32) {
        (self.sleep_cb)(time);
        self.time += time;
    }

    pub fn timeout(&self) -> bool {
        self.time > self.timeout
    }
}
