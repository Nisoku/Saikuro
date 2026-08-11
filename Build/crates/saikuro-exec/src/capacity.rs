use core::fmt;

const MIN_CHANNEL_CAPACITY: usize = 1;
const MAX_CHANNEL_CAPACITY: usize = 256;

/// A validated capacity for a bounded execution channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChannelCapacity(usize);

impl ChannelCapacity {
    /// The default channel capacity used by the execution facade.
    pub const DEFAULT: Self = Self(128);

    /// The smallest supported channel capacity.
    pub const MIN: Self = Self(MIN_CHANNEL_CAPACITY);

    /// The largest supported channel capacity.
    pub const MAX: Self = Self(MAX_CHANNEL_CAPACITY);

    /// Construct a capacity after checking the shared backend bounds.
    pub const fn new(value: usize) -> Result<Self, InvalidChannelCapacity> {
        if value < MIN_CHANNEL_CAPACITY || value > MAX_CHANNEL_CAPACITY {
            Err(InvalidChannelCapacity { value })
        } else {
            Ok(Self(value))
        }
    }

    /// Return the validated capacity as a `usize`.
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Error returned when a channel capacity is outside the supported bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidChannelCapacity {
    value: usize,
}

impl InvalidChannelCapacity {
    /// Return the rejected capacity.
    pub const fn value(self) -> usize {
        self.value
    }
}

impl fmt::Display for InvalidChannelCapacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "channel capacity {} is outside the range {}..={}",
            self.value, MIN_CHANNEL_CAPACITY, MAX_CHANNEL_CAPACITY
        )
    }
}

#[cfg(any(feature = "tokio-runtime", feature = "wasm-runtime"))]
impl std::error::Error for InvalidChannelCapacity {}

impl TryFrom<usize> for ChannelCapacity {
    type Error = InvalidChannelCapacity;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ChannelCapacity> for usize {
    fn from(value: ChannelCapacity) -> Self {
        value.get()
    }
}
