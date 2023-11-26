use iso7816_tx::{Transmission, TransmissionBuilder};

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
fn test_transmit() {
    let capdu = &[0x80, 0xca, 0x9f, 0x7f];
    let mut buf = [0u8; 258];

    let mut t = transmission();
    let rapdu = t.transmit(capdu, &mut buf).expect("Transmit failed");

    assert_eq!(rapdu, &[0x9f, 0x7f, 0x55, 0x90, 0x00]);
}

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
    unsafe { READ_CNT = 0 };
    Ok(Some(()))
}

fn close(_interface: Option<&()>) -> Result<Option<()>, ()> {
    Ok(None)
}

fn reset(_interface: Option<&()>) -> Result<(), ()> {
    unsafe { READ_CNT = 0 };
    Ok(())
}

fn read(_interface: Option<&()>, buf: &mut [u8]) -> Result<usize, ()> {
    let cnt = unsafe { &mut READ_CNT };
    let resp = &[NAD_CARD, 0x00, 0x05, 0x9f, 0x7f, 0x55, 0x90, 0x00, 0x35];

    assert!(buf.len() < resp.len());
    buf.copy_from_slice(&resp[*cnt..*cnt + buf.len()]);
    *cnt += buf.len();

    Ok(buf.len())
}

fn write(_interface: Option<&()>, buf: &[u8]) -> Result<usize, ()> {
    println!("> WRITE: {:02x?}", buf);
    assert_eq!(buf[0], NAD_DEV);
    unsafe { READ_CNT = 0 };

    Ok(buf.len())
}

fn sleep(_ms: u32) {}

const NAD_CARD: u8 = 0x15;
const NAD_DEV: u8 = 0x51;
static mut READ_CNT: usize = 0;
