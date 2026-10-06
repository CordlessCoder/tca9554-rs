use embedded_hal_async::i2c::{ErrorKind, NoAcknowledgeSource};
use embedded_hal_mock::eh1::i2c::{Mock, Transaction};
use tca9554::{Address, Tca9554};

#[cfg(feature = "interrupt")]
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
#[cfg(feature = "interrupt")]
use embedded_hal_async::digital::Wait;
#[cfg(feature = "interrupt")]
use embedded_hal_mock::eh1::digital::{Mock as PinMock, State, Transaction as PinTransaction};

/// Builds a driver whose type is named without its interrupt and mutex parameters.
fn driver(i2c: Mock) -> Tca9554<Mock> {
    Tca9554::new(i2c, Address::standard())
}

#[pollster::test]
async fn test_driver_type_names_its_defaults() {
    let i2c = Mock::new(&[Transaction::write(0x20, vec![0x03, 0x55])]);
    let mut driver = driver(i2c);
    driver.write_direction(0x55).await.unwrap();
    driver.release().done();
}

#[pollster::test]
async fn test_write_direction() {
    let i2c = Mock::new(&[Transaction::write(0x20, vec![0x03, 0xAA])]);
    let mut driver = Tca9554::new(i2c, Address::standard());
    driver.write_direction(0xAA).await.unwrap();
    driver.release().done();
}

#[pollster::test]
async fn test_write_output() {
    let i2c = Mock::new(&[
        Transaction::write(0x20, vec![0x01, 0xAA]),
        Transaction::write(0x20, vec![0x01, 0xAB]),
    ]);
    let mut driver = Tca9554::new(i2c, Address::standard());
    driver.write_output(0xAA).await.unwrap();
    driver.pin(0).unwrap().set_output_state(true).await.unwrap();
    driver.release().done();
}

#[pollster::test]
async fn test_write_polarity() {
    let i2c = Mock::new(&[
        Transaction::write(0x20, vec![0x02, 0xAA]),
        Transaction::write(0x20, vec![0x02, 0xAB]),
    ]);
    let mut driver = Tca9554::new(i2c, Address::standard());
    driver.write_polarity(0xAA).await.unwrap();
    driver.pin(0).unwrap().set_polarity(true).await.unwrap();
    driver.release().done();
}

#[pollster::test]
async fn test_write_error() {
    let no_ack = ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address);
    let i2c = Mock::new(&[Transaction::write(0x20, vec![0x03, 0xAA]).with_error(no_ack)]);
    let mut driver = Tca9554::new(i2c, Address::standard());
    assert!(driver.write_direction(0xAA).await.is_err());
    driver.release().done();
}

#[pollster::test]
async fn test_read_input() {
    let i2c = Mock::new(&[Transaction::write_read(0x20, vec![0x00], vec![0xAA])]);
    let mut driver = Tca9554::new(i2c, Address::standard());
    assert_eq!(driver.read_input().await.unwrap(), 0xAA);
    driver.release().done();
}

#[pollster::test]
async fn test_reset() {
    let i2c = Mock::new(&[
        Transaction::write(0x20, vec![0x03, 0xFF]),
        Transaction::write(0x20, vec![0x02, 0x00]),
        Transaction::write(0x20, vec![0x01, 0xFF]),
    ]);
    let mut driver = Tca9554::new(i2c, Address::standard());
    driver.reset().await.unwrap();
    driver.release().done();
}

#[pollster::test]
async fn test_pin_bounds() {
    let i2c = Mock::new(&[]);
    let driver = Tca9554::new(i2c, Address::standard());
    assert!(driver.pin(7).is_some());
    assert!(driver.pin(8).is_none());
    driver.release().done();
}

#[cfg(feature = "interrupt")]
#[pollster::test]
async fn test_interrupt_wait_ignores_unrelated_first_event() {
    let i2c = Mock::new(&[
        Transaction::write_read(0x20, vec![0x00], vec![0x00]),
        Transaction::write_read(0x20, vec![0x00], vec![0x02]),
        Transaction::write_read(0x20, vec![0x00], vec![0x01]),
    ]);
    let int = PinMock::new(&[
        PinTransaction::wait_for_state(State::Low),
        PinTransaction::wait_for_state(State::Low),
    ]);
    let driver = Tca9554::new(i2c, Address::standard()).with_int::<_, 8, NoopRawMutex>(int);
    driver.pin(0).unwrap().wait_for_high().await.unwrap();
    let (mut i2c, mut int) = driver.release_int();
    i2c.done();
    int.done();
}
