# `iso7816_tx`

> Implement **ISO7816** Smart Card **T=1** Transmission protocol

The T=1 protocol are commonly called the ISO protocols. They are primarily
based on the provisions of the ISO/IEC 7816 family of standards

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Usage

Basic APDU Transmission

```rust
fn main() {
    use iso7816_tx::TransmissionBuilder;

    let mut buf = [0u8; 258];
    let mut t = TransmissionBuilder::<(), ()>::new()
        .set_init_cb(open)
        .set_release_cb(close)
        .set_reset_cb(reset)
        .set_read_cb(read)
        .set_write_cb(write)
        .set_sleep_cb(sleep)
        .set_nad(15, 51)
        .build();

    let atr = t.atr().expect("Failed to get ATR");

    let capdu = &[0x80, 0xca, 0x9f, 0x7f];
    let rapdu = t.transmit(capdu, &mut buf).expect("Failed to transmit");
}

fn open() -> Result<Option<()>, ()> {
    // Initialize connection interface
    // ...

    Ok(Some(()))
}

fn close(_interface: Option<&()>) -> Result<Option<()>, ()> {
    // Release connection interface
    // ...

    Ok(None)
}

fn reset(_interface: Option<&()>) -> Result<(), ()> {
    // Cold reset implementation
    // ...

    Ok(())
}

fn read(_interface: Option<&()>, buf: &mut [u8]) -> Result<usize, ()> {
    // Read data from connection interface
    // ...

    Ok(buf.len())
}

fn write(_interface: Option<&()>, buf: &[u8]) -> Result<usize, ()> {
    // Write data to connection interface
    // ...

    Ok(buf.len())
}

fn sleep(_ms: u32) {
    // Sleep implementation
    // ...
}
```
