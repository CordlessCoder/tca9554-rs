use embassy_sync::blocking_mutex::raw::RawMutex;
use embedded_hal_async::i2c::I2c;
use futures::TryFutureExt;

use crate::driver::{ExioPin, Register, write_register};

pub(crate) fn read_bit(mask: u8, bit: u8) -> bool {
    (mask >> bit) & 1 != 0
}

fn with_bit(mask: u8, bit: u8, value: bool) -> u8 {
    let bit = (value as u8) << bit;
    mask & !bit | bit
}

impl<I2C, INT, M: RawMutex> ExioPin<'_, I2C, INT, M>
where
    I2C: I2c + Clone,
{
    /// Reads the value of the polarity inversion register for this pin.
    /// Returns: Whether this input's polarity is inverted.
    pub fn read_polarity(&self) -> impl Future<Output = Result<bool, I2C::Error>> {
        self.0
            .read_polarity_ref()
            .map_ok(move |mask| read_bit(mask, self.1))
    }

    /// Reads the value of the input register for this pin.
    /// Returns: Whether the incoming logic level of this pin is high, if it's defined as an input.
    pub fn read_input(&self) -> impl Future<Output = Result<bool, I2C::Error>> {
        self.0
            .read_input_ref()
            .map_ok(move |mask| read_bit(mask, self.1))
    }

    /// Reads the value of the output register for this pin.
    /// Returns: The outgoing logic level of this pin, if it's defined as an output.
    pub fn read_output(&self) -> impl Future<Output = Result<bool, I2C::Error>> {
        self.0
            .read_output_ref()
            .map_ok(move |mask| read_bit(mask, self.1))
    }

    /// Reads the value of the direction register for this pin.
    /// Returns: Whether this pin is configured as an input
    pub fn read_is_input(&self) -> impl Future<Output = Result<bool, I2C::Error>> {
        self.0
            .read_direction_ref()
            .map_ok(move |mask| (mask >> self.1) & 1 != 0)
    }

    /// Sets this output pin low or high.
    /// Updates the output register atomically with other pin updates on this driver.
    pub async fn set_output_state(&self, high: bool) -> Result<(), I2C::Error> {
        let mut cache = self.0.register_cache.lock().await;
        let state = with_bit(cache.output, self.1, high);
        write_register(
            &mut self.0.i2c.clone(),
            self.0.address,
            Register::Output,
            state,
        )
        .await?;
        cache.output = state;
        Ok(())
    }

    /// Sets this output pin low or high.
    /// Updates the polarity register atomically with other pin updates on this driver.
    pub async fn set_polarity(&self, inverted: bool) -> Result<(), I2C::Error> {
        let mut cache = self.0.register_cache.lock().await;
        let state = with_bit(cache.polarity, self.1, inverted);
        write_register(
            &mut self.0.i2c.clone(),
            self.0.address,
            Register::Polarity,
            state,
        )
        .await?;
        cache.polarity = state;
        Ok(())
    }

    /// Configures this pin as an input or output.
    /// Updates the direction register atomically with other pin updates on this driver.
    pub async fn set_input(&self, input: bool) -> Result<(), I2C::Error> {
        let mut cache = self.0.register_cache.lock().await;
        let state = with_bit(cache.direction, self.1, input);
        write_register(
            &mut self.0.i2c.clone(),
            self.0.address,
            Register::Direction,
            state,
        )
        .await?;
        cache.direction = state;
        Ok(())
    }
}
