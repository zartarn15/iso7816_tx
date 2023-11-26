use hex_literal::hex;
use iso7816_tx::{Error, Transmission, TransmissionBuilder};

#[test]
fn test_init() {
    let mut t = transmission();
    assert_eq!(t.init(), Ok(()));
}

#[test]
fn test_reset() {
    let mut t = transmission();
    assert_eq!(t.reset(), Ok(()));
}

#[test]
fn test_transmit_apdu() {
    let mut buf = [0u8; 258];
    let capdu = &hex!["80ca9f7f"];
    set_resp(&hex!["1500059f7f55900035"]);

    let mut t = transmission();
    let rapdu = t.transmit(capdu, &mut buf).expect("Transmit failed");

    assert_eq!(rapdu, &hex!["9f7f559000"]);
}

#[test]
fn test_transmit_wrong_card_crc() {
    let mut buf = [0u8; 258];
    let capdu = &hex!["80ca9f7f"];
    set_resp(&hex!["1500059f7f55900000"]);

    let mut t = transmission();
    let ret = t.transmit(capdu, &mut buf);

    assert!(matches!(ret, Err(Error::T1(_))));
}

#[test]
fn test_transmit_wrong_card_nad() {
    let mut buf = [0u8; 258];
    let capdu = &hex!["80ca9f7f"];
    set_resp(&hex!["0000079f7f55900035"]);

    let mut t = transmission();
    let ret = t.transmit(capdu, &mut buf);

    assert!(matches!(ret, Err(Error::T1(_))));
}

#[test]
fn test_transmit_empty() {
    let mut buf = [0u8; 258];
    let capdu = &[];

    let mut t = transmission();
    let ret = t.transmit(capdu, &mut buf);

    assert!(matches!(ret, Err(Error::T1(_))));
}

#[test]
fn test_transmit_too_long() {
    let mut buf = [0u8; 258];
    let capdu = &[0u8; 1024];

    let mut t = transmission();
    let ret = t.transmit(capdu, &mut buf);

    assert!(matches!(ret, Err(Error::T1(_))));
}

const NAD_CARD: u8 = 0x15;
const NAD_DEV: u8 = 0x51;

fn transmission<'a>() -> Transmission<'a, (), ()> {
    TransmissionBuilder::new()
        .set_init_cb(open)
        .set_release_cb(close)
        .set_reset_cb(reset)
        .set_read_cb(read)
        .set_write_cb(write)
        .set_sleep_cb(sleep)
        .set_nad(NAD_CARD, NAD_DEV)
        .build()
}

fn open() -> Result<Option<()>, ()> {
    set_cnt(0);
    Ok(Some(()))
}

fn close(_interface: Option<&()>) -> Result<Option<()>, ()> {
    Ok(None)
}

fn reset(_interface: Option<&()>) -> Result<(), ()> {
    set_cnt(0);
    Ok(())
}

fn read(_interface: Option<&()>, buf: &mut [u8]) -> Result<usize, ()> {
    let resp = get_resp();
    let cnt = get_cnt();
    let mut read_len = buf.len();
    let mut reset_resp = false;

    if read_len >= resp.len() - cnt {
        read_len = resp.len() - cnt;
        reset_resp = true;
    }

    buf[..read_len].copy_from_slice(&resp[cnt..cnt + read_len]);
    set_cnt(cnt + read_len);

    if reset_resp {
        set_cnt(0);
        set_resp(&[]);
    }

    Ok(read_len)
}

fn write(_interface: Option<&()>, buf: &[u8]) -> Result<usize, ()> {
    set_cnt(0);
    if buf[0] != NAD_DEV {
        return Ok(0);
    }

    Ok(buf.len())
}

fn sleep(_ms: u32) {}

static mut RESP: &[u8] = &[];
static mut READ_CNT: usize = 0;

fn set_resp(resp: &'static [u8]) {
    unsafe { RESP = resp };
}

fn set_cnt(cnt: usize) {
    unsafe { READ_CNT = cnt };
}

fn get_resp() -> &'static [u8] {
    unsafe { RESP }
}

fn get_cnt() -> usize {
    unsafe { READ_CNT }
}
