use core::sync::atomic::AtomicU8;

use embassy_sync::blocking_mutex::raw::RawMutex;
use embedded_hal_async::digital::Wait;
#[cfg(feature = "interrupt")]
use embedded_hal_async::i2c::I2c;

use crate::{Address, Tca9554};

pub struct Interrupts<
    INT,
    const SUBS: usize = 8,
    M: RawMutex = embassy_sync::blocking_mutex::raw::NoopRawMutex,
> {
    int: embassy_sync::mutex::Mutex<M, INT>,
    int_subscribers: embassy_sync::pubsub::PubSubChannel<M, u8, 1, SUBS, 1>,
}

impl<I2C, INT, const SUBS: usize, M: RawMutex> Tca9554<I2C, Interrupts<INT, SUBS, M>> {
    /// Creates a new driver with the given I²C peripheral, address, and INT pin.
    pub fn with_int(i2c: I2C, address: Address, int: INT) -> Self {
        Self {
            i2c,
            address,
            interrupt_handler: Interrupts {
                int: embassy_sync::mutex::Mutex::new(int),
                int_subscribers: embassy_sync::pubsub::PubSubChannel::new(),
            },
            output_mask: AtomicU8::new(0),
            polarity_mask: AtomicU8::new(0),
            direction_mask: AtomicU8::new(0),
        }
    }

    /// Releases the driver, returning ownership of the I²C peripheral and INT pin.
    pub fn release_int(self) -> (I2C, INT) {
        (self.i2c, self.interrupt_handler.int.into_inner())
    }
}

#[cfg(feature = "interrupt")]
pub enum InterruptWaitError<INT, I2C> {
    InterruptError(INT),
    I2CError(I2C),
    TooManySubscribers,
}

#[cfg(feature = "interrupt")]
impl<I2C, INT, const SUBS: usize, M: RawMutex> Tca9554<I2C, Interrupts<INT, SUBS, M>>
where
    INT: Wait,
    I2C: I2c + Clone,
{
    /// Wait for any of the input pins to change state, and read the new pin state mask
    pub async fn wait_for_interrupt(
        &self,
    ) -> Result<u8, InterruptWaitError<INT::Error, I2C::Error>> {
        match self.interrupt_handler.int.try_lock() {
            Ok(mut int) => {
                // We are the first to wait for the interrupt, we should publish to everyone else

                use embassy_sync::pubsub::PubSubBehavior;

                use crate::driver::Register;
                int.wait_for_low()
                    .await
                    .map_err(InterruptWaitError::InterruptError)?;
                core::mem::drop(int);
                let mut read_buf = [0u8];
                self.i2c
                    .clone()
                    .write_read(self.address.into(), &[Register::Input as u8], &mut read_buf)
                    .await
                    .map_err(InterruptWaitError::I2CError)?;
                let pins = read_buf[0];
                self.interrupt_handler
                    .int_subscribers
                    .publish_immediate(pins);
                Ok(pins)
            }
            Err(_) => {
                let mut sub = self
                    .interrupt_handler
                    .int_subscribers
                    .subscriber()
                    .map_err(|_| InterruptWaitError::TooManySubscribers)?;
                Ok(sub.next_message_pure().await)
            }
        }
    }
}
