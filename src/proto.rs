//! ISO7816 Smart Card T=1 Transmission protocol Implementation
//!
//! This module based on open source Gemalto ISO7816 T1 library:
//! https://android.googlesource.com/platform/hardware/st/secure_element2/+/refs/tags/platform-tools-30.0.4/esehal/src/iso7816_t1.c
//!

use crate::clock::Clock;

/// The Answer To Reset (ATR) ISO/IEC 7816-3 maximum length
const ATR_SIZE: usize = 32;

/// 3 bytes header + 254 bytes data + 2 bytes Crc
const BUF_SIZE: usize = 3 + 255 + 2;

const MAX_RETRIES: u8 = 3;
const MAX_WTX_ROUNDS: i32 = 200; // wtx_max_rounds == MAX_WTX_ROUNDS
const WTX_MAX_VALUE: u8 = 1;

/// Maximum for extended APDU response
const RECV_MAX: usize = 65536 + 2;

const REQUEST_RESYNC: u8 = 0x00;
const REQUEST_IFS: u8 = 0x01;
const REQUEST_ABORT: u8 = 0x02;
const REQUEST_WTX: u8 = 0x03;
const REQUEST_RESET: u8 = 0x05;

pub enum ChkAlgo {
    Lrc,
}

#[derive(PartialEq)]
enum Block {
    I,
    R,
    S,
}

#[derive(Default)]
struct State {
    halt: bool,
    request: bool,
    reqresp: bool,
    badcrc: bool,
    timeout: bool,
    aborted: bool,
}

struct Ifs {
    card: u8,
    dev: u8,
}

impl Default for Ifs {
    fn default() -> Self {
        Self { card: 32, dev: 32 }
    }
}

#[derive(Default)]
struct Nad {
    card: u8,
    dev: u8,
}

struct Wtx {
    wtx: u8,
    rounds: i32,
}

impl Default for Wtx {
    fn default() -> Self {
        Self {
            wtx: 1,
            rounds: MAX_WTX_ROUNDS,
        }
    }
}

struct Need {
    reset: bool,
    resync: bool,
    ifsd_sync: bool,
}

impl Default for Need {
    fn default() -> Self {
        Self {
            reset: true,
            resync: false,
            ifsd_sync: false,
        }
    }
}

#[derive(Default)]
struct Atr {
    buf: [u8; ATR_SIZE],
    len: usize,
}

#[derive(Default)]
struct Snd<'a> {
    buf: &'a [u8],
    len: usize,
    next: u8,
}

#[derive(Default)]
struct Recv<'a> {
    buf: &'a mut [u8],
    len: usize,
    next: u8,
    size: usize,
}

pub struct T1Proto<'a, E> {
    state: State,
    ifs: Ifs,
    nad: Nad,
    bwt: u32,
    chk_algo: ChkAlgo,
    retries: u8,
    request: u8,
    wtx: Wtx,
    need: Need,
    atr: Atr,
    send: Snd<'a>,
    recv: Recv<'a>,
    recv_max: usize,
    recv_size: usize,
    buf: [u8; BUF_SIZE],
    n: usize,
    sleep_cb: fn(u32),
    err: Result<(), Error<E>>,
}

impl<'a, E> T1Proto<'a, E> {
    pub fn set_nad(&mut self, card_nad: u8, dev_nad: u8) {
        self.nad.card = card_nad;
        self.nad.dev = dev_nad;
    }

    pub fn set_sleep_cb(&mut self, cb: fn(u32)) {
        self.sleep_cb = cb;
    }

    pub fn reset<R, W>(&mut self, read: R, write: W) -> Result<(), Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
        W: Fn(&[u8]) -> Result<usize, E>,
    {
        self.clear_states();
        self.need.reset = true;

        self.process(read, write)
    }

    pub fn atr<R, W>(&mut self, read: R, write: W) -> Result<&[u8], Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
        W: Fn(&[u8]) -> Result<usize, E>,
    {
        if self.need.reset {
            self.reset(read, write)?;
        }

        Ok(&self.atr.buf[..self.atr.len])
    }

    pub fn transmit<R, W>(
        &mut self,
        capdu: &'a [u8],
        rapdu: &'a mut [u8],
        read: R,
        write: W,
    ) -> Result<&[u8], Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
        W: Fn(&[u8]) -> Result<usize, E>,
    {
        self.clear_states();

        self.send.buf = capdu;
        self.send.len = capdu.len();
        self.recv.buf = rapdu;
        self.recv.len = 0;
        self.recv.size = self.recv.buf.len();

        self.process(read, write)?;

        Ok(&self.recv.buf[..self.recv.len])
    }

    fn clear_states(&mut self) {
        self.state = State::default();
        self.wtx = Wtx::default();
        self.retries = MAX_RETRIES;
        self.request = 0xff;
        self.send.len = 0;
        self.recv.len = 0;
        self.recv.size = 0;
        self.recv_size = 0;
        self.n = 0;
        self.err = Ok(());
    }

    fn process_init(&mut self) {
        if self.need.reset {
            self.state.request = true;
            self.request = REQUEST_RESET;
        } else if self.need.resync {
            self.state.request = true;
            self.request = REQUEST_RESYNC;
        } else if self.need.ifsd_sync {
            self.state.request = true;
            self.request = REQUEST_IFS;
            self.ifs.dev = 254;
        }
    }

    fn lrc8(&mut self, n: usize) -> u8 {
        let mut c = 0u8;

        for it in self.buf[..n].iter() {
            c ^= it;
        }

        c
    }

    fn append_lrc8(&mut self, n: usize) -> usize {
        self.buf[n] = self.lrc8(n);

        n + 1
    }

    fn do_chk(&mut self) {
        let n = 3 + usize::from(self.buf[2]);

        self.n = match self.chk_algo {
            ChkAlgo::Lrc => self.append_lrc8(n),
        };
    }

    fn write_request(&mut self, mask: u8) {
        let mut request = self.request | mask;

        self.buf[0] = self.nad.dev;
        self.buf[1] = 0xc0 | request;

        request &= 0x1f;
        if request == REQUEST_IFS {
            self.buf[2] = 1;
            if self.buf[1] & 0x20 != 0 {
                self.buf[3] = self.ifs.card;
            } else {
                self.buf[3] = self.ifs.dev;
            }
        } else if request == REQUEST_WTX {
            self.buf[2] = 1;
            self.buf[3] = self.wtx.wtx;
        } else {
            self.buf[2] = 0;
        }

        self.do_chk();
    }

    fn write_rblock(&mut self, n: u8) {
        self.buf[0] = self.nad.dev;
        self.buf[1] = 0x80 | (n & 3);
        if self.recv.next != 0 {
            self.buf[1] |= 0x10;
        }
        self.buf[2] = 0;

        self.do_chk();
    }

    fn write_iblock(&mut self) {
        let mut n = self.send_window_size();
        let mut pcb: u8;

        if n > self.ifs.card.into() {
            n = self.ifs.card.into();
            pcb = 0x20;
        } else {
            pcb = 0;
        }

        if self.send.next != 0 {
            pcb |= 0x40;
        }

        self.buf[0] = self.nad.dev;
        self.buf[1] = pcb;
        self.buf[2] = n.try_into().unwrap();
        self.buf[3..n + 3].copy_from_slice(self.send.buf);

        self.do_chk();
    }

    fn request_init(&mut self) -> Result<(), Error<E>> {
        if self.state.request {
            self.write_request(0x00);
        } else if self.state.reqresp {
            self.write_request(0x20);
            self.state.reqresp = false;
        } else if self.state.badcrc {
            self.write_rblock(1);
        } else if self.state.timeout {
            self.write_rblock(0);
        } else if self.send_window_size() != 0 {
            self.write_iblock();
        } else if self.state.aborted {
            return Err(Error::Aborted);
        } else if self.recv_window_size() > 0 {
            self.write_rblock(0);
        } else {
            return Err(Error::NoRespIBlock);
        }

        Ok(())
    }

    fn chk_algo_len(&self) -> usize {
        match self.chk_algo {
            ChkAlgo::Lrc => 1,
        }
    }

    fn block_recv<R>(&mut self, read: R) -> Result<(), Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
    {
        self.n = 0;

        let bwt = self.bwt
            * if self.wtx.wtx != 0 {
                self.wtx.wtx.into()
            } else {
                1
            };
        self.wtx.wtx = 1;

        let mut clock = Clock::new(bwt, self.sleep_cb);

        loop {
            clock.sleep(2);

            let n = read(&mut self.buf[..1]).map_err(Error::ReadNad)?;
            if n != 1 {
                return Err(Error::ReadNadLen(n, 1));
            }
            self.n = n;

            if self.buf[0] == self.nad.card {
                break;
            }

            if clock.timeout() {
                return Err(Error::Timeout(bwt));
            }
        }

        let mut max = 2 + self.chk_algo_len();
        let n = read(&mut self.buf[self.n..self.n + max]).map_err(Error::ReadHdr)?;
        if n != max {
            return Err(Error::ReadHdrLen(n, max));
        }
        self.n += n;

        let len = usize::from(self.buf[2]);
        max += len;
        if max > BUF_SIZE {
            return Err(Error::RecvLen(max, len));
        }

        if len != 0 {
            let n = read(&mut self.buf[self.n..self.n + len]).map_err(Error::ReadData)?;
            if n != len {
                return Err(Error::ReadDataLen(n, len));
            }
            self.n += n;
        }

        Ok(())
    }

    fn chk_is_good(&mut self) -> Result<(), Error<E>> {
        let n = 3 + usize::from(self.buf[2]);

        match self.chk_algo {
            ChkAlgo::Lrc => {
                let chk = self.lrc8(n);
                if chk != self.buf[n] {
                    return Err(Error::BadCrc(chk, self.buf[n]));
                }
            }
        }

        Ok(())
    }

    fn read_block<R>(&mut self, read: R) -> Result<(), Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
    {
        self.block_recv(read)?;

        if self.n < 3 {
            return Err(Error::ReadLen(self.n));
        } else if self.buf[0] != self.nad.card {
            return Err(Error::ReadNadVal(self.buf[0]));
        } else if self.buf[2] == 255 {
            return Err(Error::ReadLen255);
        }

        self.chk_is_good()
    }

    fn block_kind(&mut self) -> Block {
        if self.buf[1] & 0x80 == 0 {
            return Block::I;
        } else if self.buf[1] & 0x40 == 0 {
            return Block::R;
        }

        Block::S
    }

    fn parse_atr(&mut self) {
        let mut y = match self.atr.len {
            0 => 0i32,
            _ => self.atr.buf[0].into(),
        };
        let mut tck = y as u8;
        let mut proto = 0;
        let mut ifsc = -1i32;

        for it in self.atr.buf.iter() {
            let c = *it;
            tck ^= c;

            if y & 0xf0 == 0x80 {
                y = c.into();
                proto |= 1 << (c & 15);
            } else if y >= 16 {
                if ifsc < 0 && y & 0x1f == 0x11 {
                    ifsc = c.into();
                }
                y &= y - 16;
            } else {
                y = -1;
            }
        }

        if proto & 2 != 0 && tck == 0 {
            self.ifs.card = ifsc as u8;
        }
    }

    fn parse_response(&mut self) -> Result<bool, Error<E>> {
        let mut pcb = self.buf[1];

        if pcb & 0x20 == 0 {
            return Ok(false);
        }

        pcb &= 0x1f;
        if pcb != self.request {
            return Ok(false);
        }

        match pcb {
            REQUEST_IFS => {
                self.need.ifsd_sync = false;
                if (self.buf[2] != 1) && (self.buf[3] != self.ifs.dev) {
                    return Err(Error::BadMsgIfs);
                }
            }
            REQUEST_RESET => {
                self.need.reset = false;
                if usize::from(self.buf[2]) <= ATR_SIZE {
                    self.atr.len = self.buf[2].into();
                    self.atr.buf[..self.atr.len].copy_from_slice(&self.buf[3..self.atr.len + 3]);
                    self.parse_atr();
                } else {
                    return Err(Error::BadMsgRst);
                }
            }
            REQUEST_RESYNC => {
                self.need.resync = false;
                self.send.next = 0;
                self.recv.next = 0;
            }
            _ => return Err(Error::NeverReq),
        }

        Ok(true)
    }

    fn recv_window_size(&mut self) -> usize {
        self.recv.len
    }

    fn recv_window_free_size(&mut self) -> isize {
        isize::try_from(self.recv.size).unwrap() - isize::try_from(self.recv.len).unwrap()
    }

    fn send_window_size(&mut self) -> usize {
        self.send.len
    }

    fn recv_window_append(&mut self) {
        let free = self.recv_window_free_size();
        let mut n = isize::from(self.buf[2]);

        if n > free {
            n = free;
        }

        if n > 0 {
            let un = usize::try_from(n).unwrap();
            self.recv.buf[self.recv.len..un].copy_from_slice(&self.buf[3..un + 3]);
            self.recv.len += un;
        }
    }

    fn close_send_window(&mut self) {
        self.send.buf = &[];
        self.send.len = 0;
    }

    fn close_recv_window(&mut self) {
        self.recv.buf = &mut [];
        self.recv.len = 0;
        self.recv.size = 0;
    }

    fn ack_iblock(&mut self) {
        let mut n = self.send_window_size();

        if n > self.ifs.card.into() {
            n = self.ifs.card.into();
        }
        self.send.buf = &self.send.buf[n..];
        self.send.len -= n;

        self.send.next ^= 1;
    }

    fn parse_iblock(&mut self) -> usize {
        let pcb = self.buf[1];
        let next = Self::zero(pcb & 0x40);

        if self.recv.next == next {
            self.recv.next ^= 1;
            self.recv_window_append();
            self.recv_size += usize::from(self.buf[2]);
        }

        usize::from(Self::zero(pcb & 0x20))
    }

    fn parse_rblock(&mut self) -> Result<(), Error<E>> {
        let pcb = self.buf[1];
        let next = Self::zero(pcb & 0x10);

        match pcb & 0x2f {
            0 => {
                if self.send.next ^ next != 0 {
                    self.retries = MAX_RETRIES;
                    self.ack_iblock();
                } else {
                    self.retries -= 1;
                    if self.retries == 0 {
                        return Err(Error::RbTimeout);
                    }
                }
            }
            1 => {
                self.retries -= 1;
                self.send.next = next;
                return Err(Error::PrevBlkCrc);
            }
            2 => {
                if self.state.halt {
                    return Err(Error::RbHalt);
                }
            }
            3 => {
                self.retries -= 1;
                self.state.request = true;
                self.request = REQUEST_RESYNC;
                return Err(Error::RbResync);
            }
            _ => {
                self.state.halt = true;
                return Err(Error::RbNotSupported);
            }
        }

        Ok(())
    }

    fn parse_request(&mut self) -> Result<(), Error<E>> {
        let request = self.buf[1] & 0x3f;

        self.request = request;
        match request {
            REQUEST_RESYNC => {
                return Err(Error::ReqResync);
            }
            REQUEST_IFS => {
                if self.buf[2] != 1 {
                    return Err(Error::ErrorBadMsg1(self.buf[2]));
                } else if self.buf[3] == 0 || self.buf[3] == 0xFF {
                    return Err(Error::ErrorBadMsg2(self.buf[2]));
                } else {
                    self.ifs.card = self.buf[3];
                }
            }
            REQUEST_ABORT => {
                if self.buf[2] == 0 {
                    self.state.aborted = true;
                    self.close_send_window();
                    self.close_recv_window();
                } else {
                    return Err(Error::ErrorBadMsg3(self.buf[2]));
                }
            }
            REQUEST_WTX => {
                match self.buf[2] {
                    2.. => return Err(Error::ErrorBadMsg4(self.buf[2])),
                    1 => {
                        self.wtx.wtx = self.buf[3];

                        // if (t1->wtx_max_value) - is always true
                        if self.wtx.wtx > WTX_MAX_VALUE {
                            self.wtx.wtx = WTX_MAX_VALUE;
                        }

                        // if (t1->wtx_max_rounds) - is always true
                        self.wtx.rounds -= 1;
                        if self.wtx.rounds <= 0 {
                            self.retries = 0;
                            return Err(Error::NoRoundsLeft);
                        }
                    }
                    0 => (),
                }
            }
            _ => {}
        }

        self.state.reqresp = true;

        Ok(())
    }

    fn process<R, W>(&mut self, read: R, write: W) -> Result<(), Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
        W: Fn(&[u8]) -> Result<usize, E>,
    {
        let mut ret: Result<(), Error<E>> = Ok(());

        self.process_init();

        while !self.state.halt && self.retries > 0 {
            self.request_init()?;
            let n = write(&self.buf[..self.n]).map_err(Error::Write)?;
            if n != self.n {
                return Err(Error::WriteLen(self.n, n));
            }

            if let Err(e) = self.read_block(&read) {
                self.retries -= 1;
                match e {
                    Error::BadCrc(_, _) => self.state.badcrc = true,
                    Error::Timeout(_) => self.state.timeout = true,
                    _ => self.retries = 0,
                }
                ret = Err(e);

                continue;
            }

            if self.state.badcrc && self.buf[1] & 0xef == 0x81 {
                self.retries -= 1;
                ret = Err(Error::StateBadCrc);
                continue;
            }

            self.state.badcrc = false;
            self.state.timeout = false;

            if self.state.request {
                if self.block_kind() == Block::S {
                    match self.parse_response() {
                        Ok(false) => (),

                        Ok(true) => {
                            self.state.request = false;
                            if self.recv_window_free_size() == 0 {
                                self.state.halt = true;
                            }
                            self.retries = MAX_RETRIES;
                            if self.request == REQUEST_RESET {
                                self.state.request = true;
                                self.request = REQUEST_IFS;
                                self.ifs.dev = 254;
                                self.need.ifsd_sync = true;
                            }
                            continue;
                        }

                        Err(e) => {
                            ret = Err(e);
                            self.state.halt = true;
                            continue;
                        }
                    }
                }

                self.retries -= 1;
                ret = Err(Error::Ebade);
            } else {
                match self.block_kind() {
                    Block::I => {
                        self.retries = MAX_RETRIES;
                        if self.send_window_size() != 0 {
                            self.ack_iblock();
                        }

                        let n = self.parse_iblock();
                        if self.state.aborted {
                            continue;
                        }
                        if self.recv_size > self.recv_max {
                            ret = Err(Error::RecvMsgSize(self.recv_size, self.recv_max));
                            self.state.halt = true;
                            continue;
                        }
                        if n == 0 && self.send_window_size() == 0 {
                            self.state.halt = true;
                        }
                        self.wtx.rounds = MAX_WTX_ROUNDS;
                    }
                    Block::R => {
                        ret = self.parse_rblock();
                        self.wtx.rounds = MAX_WTX_ROUNDS;
                    }
                    Block::S => {
                        ret = self.parse_request();
                        match ret {
                            Ok(()) => self.state.reqresp = true,
                            Err(Error::NoRoundsLeft) => (),
                            Err(_) => self.state.halt = true,
                        }
                    }
                }
            }
        }

        ret
    }

    fn zero(val: u8) -> u8 {
        if val == 0 {
            0
        } else {
            1
        }
    }
}

impl<E> Default for T1Proto<'_, E> {
    fn default() -> Self {
        Self {
            state: State::default(),
            ifs: Ifs::default(),
            nad: Nad::default(),
            bwt: 300,
            chk_algo: ChkAlgo::Lrc,
            retries: MAX_RETRIES,
            request: 0xff,
            wtx: Wtx::default(),
            need: Need::default(),
            atr: Atr::default(),
            send: Snd::default(),
            recv: Recv::default(),
            recv_max: RECV_MAX,
            recv_size: 0,
            buf: [0; BUF_SIZE],
            n: 0,
            sleep_cb: |_| (),
            err: Ok(()),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    CApduLen(usize),
    NoAtr,
    NoRespIBlock,
    ReadNad(E),
    ReadHdr(E),
    ReadData(E),
    Write(E),
    ReadLen(usize),
    ReadNadVal(u8),
    ReadLen255,
    BadCrc(u8, u8),
    Timeout(u32),
    WriteLen(usize, usize),
    ReadNadLen(usize, usize),
    ReadHdrLen(usize, usize),
    ReadDataLen(usize, usize),
    RecvLen(usize, usize),
    Aborted,
    BadMsgIfs,
    BadMsgRst,
    NeverReq,
    RbTimeout,
    PrevBlkCrc,
    RbHalt,
    RbResync,
    RbNotSupported,
    ReqResync,
    ErrorBadMsg1(u8),
    ErrorBadMsg2(u8),
    ErrorBadMsg3(u8),
    ErrorBadMsg4(u8),
    NoRoundsLeft,
    StateBadCrc,
    Ebade,
    RecvMsgSize(usize, usize),
}
