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
//! let mut t = TransmissionBuilder::<(), ()>::new()
//!     .build();
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

mod proto;

use crate::proto::T1Proto;

type InitCb<T, E> = fn() -> Result<Option<T>, E>;
type ReleaseCb<T, E> = fn(Option<&T>) -> Result<Option<T>, E>;
type ResetCb<T, E> = fn(Option<&T>) -> Result<(), E>;

/// Main ISO7816 Transmission API structure
pub struct Transmission<T, E> {
    /// ISO/IEC 7816 T=1 transmission protocol context
    t1: T1Proto,

    /// Smart Card communication interface context
    interface: Option<T>,

    /// Connection interface initialization callback
    init_cb: Option<InitCb<T, E>>,

    /// Connection interface release callback
    release_cb: Option<ReleaseCb<T, E>>,

    /// Connection interface reset callback
    reset_cb: Option<ResetCb<T, E>>,
}

impl<T, E> Transmission<T, E> {
    /// Initialize Transmission context
    pub fn init(&mut self) -> Result<(), Error<E>> {
        self.interface = match self.init_cb {
            Some(cb) => cb().map_err(Error::InitCbErr)?,
            None => None,
        };

        Ok(())
    }

    /// Reset Transmission protocol states
    pub fn reset(&mut self) -> Result<(), Error<E>> {
        if let Some(cb) = self.reset_cb {
            cb(self.interface.as_ref()).map_err(Error::ResetCbErr)?
        }

        self.t1.reset().map_err(Error::T1)
    }

    /// Get Answer To Reset (ATR)
    pub fn atr(&mut self) -> Result<&[u8], Error<E>> {
        self.t1.atr().map_err(Error::T1)
    }

    /// Transmit APDU data and get the response
    pub fn transmit(&mut self, capdu: &[u8]) -> Result<&[u8], Error<E>> {
        self.t1.transmit(capdu).map_err(Error::T1)
    }

    /// Release Transmission context
    pub fn release(&mut self) -> Result<(), Error<E>> {
        self.interface = match self.release_cb {
            Some(cb) => cb(self.interface.as_ref()).map_err(Error::ReleaseCbErr)?,
            None => None,
        };

        Ok(())
    }
}

impl<T, E> Drop for Transmission<T, E> {
    fn drop(&mut self) {
        self.release().unwrap_or(())
    }
}

/// ISO7816 Transmission context Builder
pub struct TransmissionBuilder<T, E> {
    init_cb: Option<InitCb<T, E>>,
    release_cb: Option<ReleaseCb<T, E>>,
    reset_cb: Option<ResetCb<T, E>>,
}

impl<T, E> TransmissionBuilder<T, E> {
    /// Create new TransmissionBuilder structure
    pub fn new() -> Self {
        Self {
            init_cb: None,
            release_cb: None,
            reset_cb: None,
        }
    }

    /// Set connection interface initialization callback
    pub fn set_init_cb(mut self, cb: InitCb<T, E>) -> Self {
        self.init_cb = Some(cb);

        self
    }

    /// Set connection interface release callback
    pub fn set_release_cb(mut self, cb: ReleaseCb<T, E>) -> Self {
        self.release_cb = Some(cb);

        self
    }

    /// Set connection interface reset callback
    pub fn set_reset_cb(mut self, cb: ResetCb<T, E>) -> Self {
        self.reset_cb = Some(cb);

        self
    }

    /// Build Transmission structure from setuped TransmissionBuilder
    pub fn build(self) -> Transmission<T, E> {
        Transmission {
            t1: T1Proto::new(),
            interface: None,
            init_cb: self.init_cb,
            release_cb: self.release_cb,
            reset_cb: self.reset_cb,
        }
    }
}

impl<T, E> Default for TransmissionBuilder<T, E> {
    fn default() -> Self {
        Self::new()
    }
}

/// ISO7816 Transmission errors
#[derive(Debug, PartialEq)]
pub enum Error<E> {
    /// ISO/IEC 7816 T=1 transmission protocol context
    T1(crate::proto::Error),

    /// Connection interface initialization callback error
    InitCbErr(E),

    /// Connection interface release callback error
    ReleaseCbErr(E),

    /// Connection interface reset callback error
    ResetCbErr(E),
}
