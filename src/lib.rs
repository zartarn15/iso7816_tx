//! ISO7816 Smart Card T=1 Transmission protocol
//!
//! This Library implements ISO/IEC 7816 T=1 transmission protocol in 'no_std'
//! environment.
//!
//! The T=1 protocol are commonly called the ISO protocols. They are primarily
//! based on the provisions of the ISO/IEC 7816 family of standards
//!
//! # Examples
//! ```
//! use iso7816_tx::TransmissionBuilder;
//!
//! let mut t = TransmissionBuilder::new().build();
//!
//! t.init().expect("Failed to init");
//! t.reset().expect("Failed to reset");
//!
//! let atr = t.atr().expect("Failed to get ATR");
//!
//! let capdu = [0x80, 0xca, 0x9f, 0x7f];
//! let rapdu = t.transmit(&capdu).expect("Failed to transmit");
//!
//! ```

#![no_std]

/// The Answer To Reset (ATR) ISO/IEC 7816-3 maximum length
const ATR_SIZE: usize = 33;

/// Minimum length for an APDU command
const CAPDU_MIN: usize = 4;

/// Main ISO7816 Transmission API structure
pub struct Transmission {
    /// Answer To Reset
    atr: [u8; ATR_SIZE],
}

impl Transmission {
    /// Initialize Transmission context
    pub fn init(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Reset Transmission protocol states
    pub fn reset(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Get Answer To Reset (ATR)
    pub fn atr(&mut self) -> Result<&[u8], Error> {
        Ok(&self.atr)
    }

    /// Transmit APDU data and get the response
    pub fn transmit(&mut self, capdu: &[u8]) -> Result<&[u8], Error> {
        if capdu.len() < CAPDU_MIN {
            return Err(Error::CApduLen(capdu.len()));
        }

        Ok(&[0x90, 0x00])
    }
}

/// ISO7816 Transmission context Builder
pub struct TransmissionBuilder {}

impl TransmissionBuilder {
    /// Create new TransmissionBuilder structure
    pub fn new() -> TransmissionBuilder {
        TransmissionBuilder {}
    }

    /// Build Transmission structure from setuped TransmissionBuilder
    pub fn build(self) -> Transmission {
        Transmission {
            atr: [0u8; ATR_SIZE],
        }
    }
}

/// ISO7816 Transmission errors
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Incorrect APDU command length
    CApduLen(usize),
}
