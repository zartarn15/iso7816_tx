use crate::clock::Clock;

/// The Answer To Reset (ATR) ISO/IEC 7816-3 maximum length
const ATR_SIZE: usize = 32;

/// 3 bytes header + 254 bytes data + 2 bytes Crc
const BUF_SIZE: usize = 3 + 255 + 2;

const MAX_RETRIES: u8 = 3;
const MAX_WTX_ROUNDS: i32 = 200;
const MAX_WTX_VALUE: i32 = 1;
const WTX_MAX_VALUE: u8 = 1;

/// Maximum for extended APDU response
const RECV_MAX: usize = 65536 + 2;

const REQUEST_RESYNC: u8 = 0x00;
const REQUEST_IFS: u8 = 0x01;
const REQUEST_ABORT: u8 = 0x02;
const REQUEST_WTX: u8 = 0x03;
const REQUEST_RESET: u8 = 0x05;

enum ChkAlgo {
    Lrc,
    Crc,
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
struct Tx<'a> {
    buf: &'a [u8],
    next: u8,
    size: usize,
}

pub struct T1Proto<'a> {
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
    send: Tx<'a>,
    recv: Tx<'a>,
    recv_max: usize,
    recv_size: usize,
    buf: [u8; BUF_SIZE],
    n: usize,
    clock: Clock,
}

impl<'a> T1Proto<'a> {
    pub fn set_nad(&mut self, card_nad: u8, dev_nad: u8) {
        self.nad.card = card_nad;
        self.nad.dev = dev_nad;
    }

    pub fn reset<R, W, E>(&mut self, read: R, write: W) -> Result<(), Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
        W: Fn(&[u8]) -> Result<usize, E>,
    {
        self.clear_states();
        self.need.reset = true;

        self.process(read, write)
    }

    pub fn atr<R, W, E>(&mut self, read: R, write: W) -> Result<&[u8], Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
        W: Fn(&[u8]) -> Result<usize, E>,
    {
        Ok(&[])
    }

    pub fn transmit<R, W, E>(
        &mut self,
        capdu: &'a [u8],
        rapdu: &'a mut [u8],
        read: R,
        write: W,
    ) -> Result<(), Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
        W: Fn(&[u8]) -> Result<usize, E>,
    {
        self.clear_states();

        self.send.buf = capdu;
        self.recv.buf = rapdu;
        self.recv.size = 0;

        self.process(read, write)?;

        Ok(())
    }

    fn clear_states(&mut self) {
        self.state = State::default();
        self.wtx = Wtx::default();
        self.retries = MAX_RETRIES;
        self.request = 0xff;
        self.send = Tx::default();
        self.recv = Tx::default();
        self.recv_size = 0;
        self.n = 0;
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
            ChkAlgo::Crc => todo!(),
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
        let mut n = self.send.buf.len();
        let mut pcb = 0u8;

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
        self.buf[3..self.send.buf.len() + 3].copy_from_slice(self.send.buf);

        self.do_chk();
    }

    fn request_init<E>(&mut self) -> Result<(), Error<E>> {
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
            ChkAlgo::Crc => 2,
        }
    }

    fn block_recv<R, E>(&mut self, read: R) -> Result<(), Error<E>>
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

        self.clock.start(bwt);
        loop {
            self.clock.sleep(2);

            let n = read(&mut self.buf[..1]).map_err(Error::ReadNad)?;
            if n != 1 {
                return Err(Error::ReadNadLen(n, 1));
            }
            self.n = n;

            if self.buf[0] == self.nad.card {
                break;
            }

            if self.clock.timeout() {
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

    fn chk_is_good<E>(&mut self) -> Result<(), Error<E>> {
        let n = 3 + usize::from(self.buf[2]);

        match self.chk_algo {
            ChkAlgo::Lrc => {
                let chk = self.lrc8(n);
                if chk != self.buf[n] {
                    return Err(Error::BadCrc(chk, self.buf[n]));
                }
            }
            ChkAlgo::Crc => todo!(),
        }

        Ok(())
    }

    fn read_block<R, E>(&mut self, read: R) -> Result<(), Error<E>>
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
        } else if self.buf[1] & 0x80 == 0 {
            return Block::R;
        }

        return Block::S;
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

    fn parse_response<E>(&mut self) -> Result<bool, Error<E>> {
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
        self.recv.buf.len()
    }

    fn recv_window_free_size(&mut self) -> isize {
        isize::try_from(self.recv.size).unwrap() - isize::try_from(self.recv.buf.len()).unwrap()
    }

    fn send_window_size(&mut self) -> usize {
        self.send.buf.len()
    }

    fn process<R, W, E>(&mut self, read: R, write: W) -> Result<(), Error<E>>
    where
        R: Fn(&mut [u8]) -> Result<usize, E>,
        W: Fn(&[u8]) -> Result<usize, E>,
    {
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

                continue;
            }

            if self.state.badcrc {
                if self.buf[1] & 0xef == 0x81 {
                    self.retries -= 1;
                    continue;
                }
            }

            self.state.badcrc = false;
            self.state.timeout = false;

            if self.state.request {
                if self.block_kind() == Block::S {
                    match self.parse_response::<E>() {
                        Ok(false) => break,

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
                        }

                        Err(_) => self.state.halt = true,
                    }

                    continue;
                }

                self.retries -= 1;
            } else {
                match self.block_kind() {
                    // TODO
                    Block::I => (),
                    Block::R => (),
                    Block::S => (),
                }
            }
        }

        Ok(())
    }
}

impl<'a> Default for T1Proto<'a> {
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
            send: Tx::default(),
            recv: Tx::default(),
            recv_max: RECV_MAX,
            recv_size: 0,
            buf: [0; BUF_SIZE],
            n: 0,
            clock: Clock::default(),
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
}
