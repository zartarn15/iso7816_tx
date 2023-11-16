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
//! let mut rapdu = [0u8, 258];
//! let rapdu = t.transmit(&capdu, &mut rapdu).expect("Failed to transmit");
//!
//! ```

#![no_std]

mod proto;

use proto::T1Proto;

type InitCb<T, E> = fn() -> Result<Option<T>, E>;
type ReleaseCb<T, E> = fn(Option<&T>) -> Result<Option<T>, E>;
type ResetCb<T, E> = fn(Option<&T>) -> Result<(), E>;
type ReadCb<T, E> = fn(Option<&T>, &mut [u8]) -> Result<usize, E>;
type WriteCb<T, E> = fn(Option<&T>, &[u8]) -> Result<usize, E>;

/// Main ISO7816 Transmission API structure
pub struct Transmission<'a, T, E> {
    /// ISO/IEC 7816 T=1 transmission protocol context
    t1: T1Proto<'a>,

    /// Smart Card communication interface context
    interface: Option<T>,

    /// Connection interface initialization callback
    init_cb: Option<InitCb<T, E>>,

    /// Connection interface release callback
    release_cb: Option<ReleaseCb<T, E>>,

    /// Connection interface reset callback
    reset_cb: Option<ResetCb<T, E>>,

    /// Connection interface read callback
    read_cb: Option<ReadCb<T, E>>,

    /// Connection interface write callback
    write_cb: Option<WriteCb<T, E>>,

    /// NAD byte for Smart Card
    card_nad: Option<u8>,

    /// NAD byte for device
    dev_nad: Option<u8>,
}

impl<'a, T, E> Transmission<'a, T, E> {
    /// Initialize Transmission context
    pub fn init(&mut self) -> Result<(), Error<E>> {
        self.interface = match self.init_cb {
            Some(cb) => cb().map_err(Error::InitCbErr)?,
            None => None,
        };

        let card_nad = self.card_nad.ok_or(Error::NadNotSet)?;
        let dev_nad = self.dev_nad.ok_or(Error::NadNotSet)?;
        self.t1.set_nad(card_nad, dev_nad);

        Ok(())
    }

    /// Reset Transmission protocol states
    pub fn reset(&mut self) -> Result<(), Error<E>> {
        // Cold reset
        if let Some(cb) = self.reset_cb {
            cb(self.interface.as_ref()).map_err(Error::ResetCbErr)?
        }

        // Soft reset
        let ifc = self.interface.as_ref(); // TODO: use tuple
        let read = self.read_cb.as_ref().ok_or(Error::NoReadCb)?;
        let write = self.write_cb.as_ref().ok_or(Error::NoWriteCb)?;
        self.t1
            .reset(|b| read(ifc, b), |b| write(ifc, b))
            .map_err(Error::T1)
    }

    /// Get Answer To Reset (ATR)
    pub fn atr(&mut self) -> Result<&[u8], Error<E>> {
        let ifc = self.interface.as_ref(); // TODO: use tuple
        let read = self.read_cb.as_ref().ok_or(Error::NoReadCb)?;
        let write = self.write_cb.as_ref().ok_or(Error::NoWriteCb)?;
        self.t1
            .atr(|b| read(ifc, b), |b| write(ifc, b))
            .map_err(Error::T1)
    }

    /// Transmit APDU data and get the response
    pub fn transmit(&mut self, capdu: &[u8], rapdu: &mut [u8]) -> Result<(), Error<E>> {
        let ifc = self.interface.as_ref(); // TODO: use tuple
        let read = self.read_cb.as_ref().ok_or(Error::NoReadCb)?;
        let write = self.write_cb.as_ref().ok_or(Error::NoWriteCb)?;
        self.t1
            .transmit(capdu, rapdu, |b| read(ifc, b), |b| write(ifc, b))
            .map_err(Error::T1)
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

impl<'a, T, E> Drop for Transmission<'a, T, E> {
    fn drop(&mut self) {
        self.release().unwrap_or(())
    }
}

/// ISO7816 Transmission context Builder
pub struct TransmissionBuilder<T, E> {
    init_cb: Option<InitCb<T, E>>,
    release_cb: Option<ReleaseCb<T, E>>,
    reset_cb: Option<ResetCb<T, E>>,
    read_cb: Option<ReadCb<T, E>>,
    write_cb: Option<WriteCb<T, E>>,
    card_nad: Option<u8>,
    dev_nad: Option<u8>,
}

impl<'a, T, E> TransmissionBuilder<T, E> {
    /// Create new TransmissionBuilder structure
    pub fn new() -> Self {
        Self {
            init_cb: None,
            release_cb: None,
            reset_cb: None,
            read_cb: None,
            write_cb: None,
            card_nad: None,
            dev_nad: None,
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

    /// Set connection interface read callback
    pub fn set_read_cb(mut self, cb: ReadCb<T, E>) -> Self {
        self.read_cb = Some(cb);

        self
    }

    /// Set connection interface write callback
    pub fn set_write_cb(mut self, cb: WriteCb<T, E>) -> Self {
        self.write_cb = Some(cb);

        self
    }

    /// Set NAD bytes for Smart Card and Device
    pub fn set_nad(mut self, card_nad: u8, dev_nad: u8) -> Self {
        self.card_nad = Some(card_nad);
        self.dev_nad = Some(dev_nad);

        self
    }

    /// Build Transmission structure from setuped TransmissionBuilder
    pub fn build(self) -> Transmission<'a, T, E> {
        Transmission {
            t1: T1Proto::default(),
            interface: None,
            init_cb: self.init_cb,
            release_cb: self.release_cb,
            reset_cb: self.reset_cb,
            read_cb: self.read_cb,
            write_cb: self.write_cb,
            card_nad: self.card_nad,
            dev_nad: self.dev_nad,
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
    T1(proto::Error<E>),

    /// Connection interface initialization callback error
    InitCbErr(E),

    /// Connection interface release callback error
    ReleaseCbErr(E),

    /// Connection interface reset callback error
    ResetCbErr(E),

    /// NAD byte is not set
    NadNotSet,

    NoReadCb,
    NoWriteCb,
}
