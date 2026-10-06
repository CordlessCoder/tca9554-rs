use crate::Address;
use embassy_sync::blocking_mutex::raw::{NoopRawMutex, RawMutex};
use embassy_sync::mutex::Mutex;
use embedded_hal_async::i2c::I2c;

/// The driver's interrupt handling when no interrupt pin is bound.
pub struct NoInterrupts;

/// Driver for a TCA9554(A) I/O expander.
pub struct Tca9554<I2C, Int = NoInterrupts, M: RawMutex = NoopRawMutex> {
    pub(crate) i2c: I2C,
    pub(crate) address: Address,
    #[cfg_attr(not(feature = "interrupt"), allow(unused))]
    pub(crate) interrupt_handler: Int,
    pub(crate) register_cache: Mutex<M, RegisterCache>,
}

impl<I2C> Tca9554<I2C, NoInterrupts, NoopRawMutex> {
    /// Creates a new driver with the given I²C peripheral and address.
    #[must_use]
    pub fn new(i2c: I2C, address: Address) -> Self {
        Self {
            i2c,
            address,
            interrupt_handler: NoInterrupts,
            register_cache: Mutex::new(RegisterCache::default()),
        }
    }

    /// Binds an interrupt pin to this peripheral.
    #[must_use]
    #[cfg(feature = "interrupt")]
    pub fn with_int<INT, const SUBS: usize, M: embassy_sync::blocking_mutex::raw::RawMutex>(
        self,
        int: INT,
    ) -> Tca9554<I2C, crate::interrupt::Interrupts<INT, SUBS, M>, M> {
        let Self {
            i2c,
            address,
            register_cache,
            interrupt_handler: _,
        } = self;
        Tca9554 {
            i2c,
            address,
            interrupt_handler: crate::interrupt::Interrupts {
                int: embassy_sync::mutex::Mutex::new(int),
                int_subscribers: embassy_sync::pubsub::PubSubChannel::new(),
            },
            register_cache: Mutex::new(register_cache.into_inner()),
        }
    }
}

impl<I2C, INT, M: RawMutex> Tca9554<I2C, INT, M> {
    /// Gets the I²C used by the driver.
    pub fn address(&self) -> Address {
        self.address
    }

    /// Releases the driver, returning ownership of the I²C peripheral.
    pub fn release(self) -> I2C {
        self.i2c
    }
}

/// Device register address.
#[repr(u8)]
pub(crate) enum Register {
    Input = 0x00,
    Output = 0x01,
    Polarity = 0x02,
    Direction = 0x03,
}

// Power-on defaults
const OUTPUT_REGISTER_DEFAULT: u8 = 0xFF;
const POLARITY_REGISTER_DEFAULT: u8 = 0x00;
const DIRECTION_REGISTER_DEFAULT: u8 = 0xFF;

pub(crate) struct RegisterCache {
    pub(crate) output: u8,
    pub(crate) polarity: u8,
    pub(crate) direction: u8,
}

impl Default for RegisterCache {
    fn default() -> Self {
        Self {
            output: OUTPUT_REGISTER_DEFAULT,
            polarity: POLARITY_REGISTER_DEFAULT,
            direction: DIRECTION_REGISTER_DEFAULT,
        }
    }
}

/// Writes a value to a register.
pub(crate) async fn write_register<I: I2c>(
    i2c: &mut I,
    address: Address,
    register: Register,
    value: u8,
) -> Result<(), I::Error> {
    i2c.write(address.into(), &[register as u8, value]).await?;
    Ok(())
}

/// Reads the value from a register.
pub(crate) async fn read_register<I: I2c>(
    i2c: &mut I,
    address: Address,
    register: Register,
) -> Result<u8, I::Error> {
    let mut read_buf = [0u8];
    i2c.write_read(address.into(), &[register as u8], &mut read_buf)
        .await?;
    Ok(read_buf[0])
}

/// One pin of the I/O expander, from [`Tca9554::pin`].
#[derive(Clone)]
pub struct ExioPin<'io, I2C, INT, M: RawMutex = NoopRawMutex>(
    pub(crate) &'io Tca9554<I2C, INT, M>,
    pub(crate) u8,
);

impl<I2C, INT, M: RawMutex> Tca9554<I2C, INT, M>
where
    I2C: I2c,
{
    /// Performs reads of all registers to ensure the device is functioning correctly.
    pub async fn init(&mut self) -> Result<(), I2C::Error> {
        self.read_output().await?;
        self.read_polarity().await?;
        self.read_direction().await?;
        Ok(())
    }

    /// Reads the value of the input register.
    pub fn read_input(&mut self) -> impl Future<Output = Result<u8, I2C::Error>> {
        read_register(&mut self.i2c, self.address, Register::Input)
    }

    /// Reads the value of the output register.
    pub async fn read_output(&mut self) -> Result<u8, I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        let value = read_register(&mut self.i2c, self.address, Register::Output).await?;
        cache.output = value;
        Ok(value)
    }

    /// Writes the value of the output register.
    pub async fn write_output(&mut self, state: u8) -> Result<(), I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        write_register(&mut self.i2c, self.address, Register::Output, state).await?;
        cache.output = state;
        Ok(())
    }

    /// Reads the value of the polarity inversion register.
    pub async fn read_polarity(&mut self) -> Result<u8, I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        let value = read_register(&mut self.i2c, self.address, Register::Polarity).await?;
        cache.polarity = value;
        Ok(value)
    }

    /// Writes the value of polarity inversion register.
    pub async fn write_polarity(&mut self, state: u8) -> Result<(), I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        write_register(&mut self.i2c, self.address, Register::Polarity, state).await?;
        cache.polarity = state;
        Ok(())
    }

    /// Reads the value of the direction register.
    pub async fn read_direction(&mut self) -> Result<u8, I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        let value = read_register(&mut self.i2c, self.address, Register::Direction).await?;
        cache.direction = value;
        Ok(value)
    }

    /// Writes the value of the direction register.
    pub async fn write_direction(&mut self, state: u8) -> Result<(), I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        write_register(&mut self.i2c, self.address, Register::Direction, state).await?;
        cache.direction = state;
        Ok(())
    }

    /// Writes the state of the registers to the chip's power-on defaults.
    pub async fn reset(&mut self) -> Result<(), I2C::Error> {
        self.write_direction(DIRECTION_REGISTER_DEFAULT).await?;
        self.write_polarity(POLARITY_REGISTER_DEFAULT).await?;
        self.write_output(OUTPUT_REGISTER_DEFAULT).await?;
        Ok(())
    }

    /// Returns whether the cached register state matches the chip's power-on defaults.
    ///
    /// When the chip is functioning properly, the registers will match
    /// the power-on defaults after power has been applied or after
    /// a call to [`Self::reset()`].
    ///
    pub async fn is_in_default_state(&mut self) -> Result<bool, I2C::Error> {
        Ok(self.read_direction().await? == DIRECTION_REGISTER_DEFAULT
            && self.read_polarity().await? == POLARITY_REGISTER_DEFAULT
            && self.read_output().await? == OUTPUT_REGISTER_DEFAULT)
    }
}

impl<I2C, INT, M: RawMutex> Tca9554<I2C, INT, M>
where
    I2C: I2c + Clone,
{
    /// Reads the value of the input register.
    pub async fn read_input_ref(&self) -> Result<u8, I2C::Error> {
        read_register(&mut self.i2c.clone(), self.address, Register::Input).await
    }

    /// Reads the value of the output register.
    pub async fn read_output_ref(&self) -> Result<u8, I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        let value = read_register(&mut self.i2c.clone(), self.address, Register::Output).await?;
        cache.output = value;
        Ok(value)
    }

    /// Writes the value of the output register.
    pub async fn write_output_ref(&self, state: u8) -> Result<(), I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        write_register(&mut self.i2c.clone(), self.address, Register::Output, state).await?;
        cache.output = state;
        Ok(())
    }

    /// Reads the value of the polarity inversion register.
    pub async fn read_polarity_ref(&self) -> Result<u8, I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        let value = read_register(&mut self.i2c.clone(), self.address, Register::Polarity).await?;
        cache.polarity = value;
        Ok(value)
    }

    /// Writes the value of polarity inversion register.
    pub async fn write_polarity_ref(&self, state: u8) -> Result<(), I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        write_register(
            &mut self.i2c.clone(),
            self.address,
            Register::Polarity,
            state,
        )
        .await?;
        cache.polarity = state;
        Ok(())
    }

    /// Reads the value of the direction register.
    pub async fn read_direction_ref(&self) -> Result<u8, I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        let value = read_register(&mut self.i2c.clone(), self.address, Register::Direction).await?;
        cache.direction = value;
        Ok(value)
    }

    /// Writes the value of the direction register.
    pub async fn write_direction_ref(&self, state: u8) -> Result<(), I2C::Error> {
        let mut cache = self.register_cache.lock().await;
        write_register(
            &mut self.i2c.clone(),
            self.address,
            Register::Direction,
            state,
        )
        .await?;
        cache.direction = state;
        Ok(())
    }

    /// Writes the state of the registers to the chip's power-on defaults.
    pub async fn reset_ref(&self) -> Result<(), I2C::Error> {
        self.write_direction_ref(DIRECTION_REGISTER_DEFAULT).await?;
        self.write_polarity_ref(POLARITY_REGISTER_DEFAULT).await?;
        self.write_output_ref(OUTPUT_REGISTER_DEFAULT).await?;
        Ok(())
    }

    /// Returns whether the cached register state matches the chip's power-on defaults.
    ///
    /// When the chip is functioning properly, the registers will match
    /// the power-on defaults after power has been applied or after
    /// a call to [`Self::reset_ref()`].
    pub async fn is_in_default_state_ref(&self) -> bool {
        let cache = self.register_cache.lock().await;
        cache.direction == DIRECTION_REGISTER_DEFAULT
            && cache.polarity == POLARITY_REGISTER_DEFAULT
            && cache.output == OUTPUT_REGISTER_DEFAULT
    }

    /// Creates a view restricted to a specific pin of the I/O expander.
    ///
    /// Returns `None` when `pin` is greater than 7.
    pub fn pin<'io>(&'io self, pin: u8) -> Option<ExioPin<'io, I2C, INT, M>> {
        (pin < 8).then_some(ExioPin(self, pin))
    }
}
