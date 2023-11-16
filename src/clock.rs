#[derive(Default)]
pub struct Clock {}

impl Clock {
    pub fn start(&mut self, timeout: u32) {}

    pub fn sleep(&self, ms: u32) {}

    pub fn timeout(&self) -> bool {
        false
    }
}
