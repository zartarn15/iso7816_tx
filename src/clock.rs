#[derive(Default)]
pub struct Clock {}

impl Clock {
    pub fn start(&mut self, _timeout: u32) {}

    pub fn sleep(&self, _ms: u32) {}

    pub fn timeout(&self) -> bool {
        false
    }
}
