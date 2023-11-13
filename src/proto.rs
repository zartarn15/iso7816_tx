/// The Answer To Reset (ATR) ISO/IEC 7816-3 maximum length
const ATR_SIZE: usize = 32;

/// 3 bytes header + 254 bytes data + 2 bytes CRC
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
    LRC,
    CRC,
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
struct Tx<'a> {
    buf: Option<&'a [u8]>,
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
    atr: Option<[u8; ATR_SIZE]>,
    send: Tx<'a>,
    recv: Tx<'a>,
    recv_max: usize,
    recv_size: usize,
    buf: [u8; BUF_SIZE],
    n: usize,
}

impl<'a> T1Proto<'a> {
    pub fn set_nad(&mut self, card_nad: u8, dev_nad: u8) {
        self.nad.card = card_nad;
        self.nad.dev = dev_nad;
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        self.clear_states();
        self.need.reset = true;

        self.process()
    }

    pub fn atr(&mut self) -> Result<&[u8], Error> {
        let atr = self.atr.as_ref().ok_or(Error::NoAtr)?;

        Ok(atr)
    }

    pub fn transmit(&mut self, capdu: &[u8], rapdu: &mut [u8]) -> Result<(), Error> {
        self.clear_states();

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

    fn append_lrc8(&mut self, n: usize) -> usize {
        let mut c = 0u8;

        for it in self.buf[..n].iter() {
            c ^= it;
        }
        self.buf[n] = c;

        n + 1
    }

    fn do_chk(&mut self) -> usize {
        let n = 3 + usize::from(self.buf[2]);

        match self.chk_algo {
            ChkAlgo::LRC => self.append_lrc8(n),
            ChkAlgo::CRC => panic!("Unimplemented"),
        }
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

        self.n = self.do_chk();
    }

    fn request_init(&mut self) -> Result<(), Error> {
        if self.state.request {
            self.write_request(0x00);
        } else if self.state.reqresp {
            self.write_request(0x20);
            self.state.reqresp = false;
        } else {
            // TODO more
            return Err(Error::NoRespIBlock);
        }

        panic!("> req: {:02x?}", &self.buf[..self.n]);

        Ok(())
    }

    fn process(&mut self) -> Result<(), Error> {
        self.process_init();

        while !self.state.halt && self.retries > 0 {
            self.request_init()?;
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
            chk_algo: ChkAlgo::LRC,
            retries: MAX_RETRIES,
            request: 0xff,
            wtx: Wtx::default(),
            need: Need::default(),
            atr: None,
            send: Tx::default(),
            recv: Tx::default(),
            recv_max: RECV_MAX,
            recv_size: 0,
            buf: [0; BUF_SIZE],
            n: 0,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Error {
    CApduLen(usize),
    NoAtr,
    NoRespIBlock,
}
