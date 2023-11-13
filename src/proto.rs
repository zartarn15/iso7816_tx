/// The Answer To Reset (ATR) ISO/IEC 7816-3 maximum length
const ATR_SIZE: usize = 33;

/// Minimum length for an APDU command
const CAPDU_MIN: usize = 4;

pub struct T1Proto {
    atr: Option<[u8; ATR_SIZE]>,
}

impl T1Proto {
    pub fn new() -> Self {
        Self { atr: None }
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        Ok(())
    }

    pub fn atr(&mut self) -> Result<&[u8], Error> {
        let atr = self.atr.as_ref().ok_or(Error::NoAtr)?;

        Ok(atr)
    }

    pub fn transmit(&mut self, capdu: &[u8]) -> Result<&[u8], Error> {
        if capdu.len() < CAPDU_MIN {
            return Err(Error::CApduLen(capdu.len()));
        }

        Ok(&[])
    }
}

impl Default for T1Proto {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq)]
pub enum Error {
    CApduLen(usize),
    NoAtr,
}
