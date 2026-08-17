use core::fmt;

const MIN_CHANNEL_CAPACITY: usize = 1;
const MAX_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChannelCapacity(usize);

impl ChannelCapacity {
    pub const DEFAULT: Self = Self(128);
    pub const MIN: Self = Self(MIN_CHANNEL_CAPACITY);
    pub const MAX: Self = Self(MAX_CHANNEL_CAPACITY);

    pub const fn new(value: usize) -> Result<Self, InvalidChannelCapacity> {
        if value < MIN_CHANNEL_CAPACITY || value > MAX_CHANNEL_CAPACITY {
            Err(InvalidChannelCapacity { value })
        } else {
            Ok(Self(value))
        }
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidChannelCapacity {
    value: usize,
}

impl InvalidChannelCapacity {
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

#[cfg(feature = "std")]
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

// Unified error types
#[allow(dead_code)]
pub mod mpsc {
    use core::fmt;

    #[derive(Debug)]
    pub struct SendError<T>(pub T);

    impl<T> SendError<T> {
        pub fn into_inner(self) -> T {
            self.0
        }
    }

    impl<T> fmt::Display for SendError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("send failed: channel is disconnected")
        }
    }

    #[cfg(feature = "std")]
    impl<T: fmt::Debug> std::error::Error for SendError<T> {}

    #[derive(Debug)]
    pub enum TrySendError<T> {
        Full(T),
        Disconnected(T),
    }

    impl<T> TrySendError<T> {
        pub fn into_inner(self) -> T {
            match self {
                TrySendError::Full(v) => v,
                TrySendError::Disconnected(v) => v,
            }
        }

        pub fn is_full(&self) -> bool {
            matches!(self, TrySendError::Full(_))
        }

        pub fn is_disconnected(&self) -> bool {
            matches!(self, TrySendError::Disconnected(_))
        }
    }

    impl<T> fmt::Display for TrySendError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                TrySendError::Full(_) => f.write_str("send failed: channel is full"),
                TrySendError::Disconnected(_) => {
                    f.write_str("send failed: channel is disconnected")
                }
            }
        }
    }

    #[cfg(feature = "std")]
    impl<T: fmt::Debug> std::error::Error for TrySendError<T> {}
}

#[allow(dead_code)]
pub mod oneshot {
    use core::fmt;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RecvError;

    impl fmt::Display for RecvError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("oneshot receiver closed")
        }
    }

    #[cfg(feature = "std")]
    impl std::error::Error for RecvError {}
}

#[allow(dead_code)]
pub mod watch {
    use core::fmt;

    #[derive(Debug)]
    pub struct SendError<T>(pub T);

    impl<T> SendError<T> {
        pub fn into_inner(self) -> T {
            self.0
        }
    }

    impl<T> fmt::Display for SendError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("watch channel has no receivers")
        }
    }

    #[cfg(feature = "std")]
    impl<T: fmt::Debug> std::error::Error for SendError<T> {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RecvError;

    impl fmt::Display for RecvError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("watch channel closed")
        }
    }

    #[cfg(feature = "std")]
    impl std::error::Error for RecvError {}
}

#[derive(Debug)]
pub struct JoinError {
    kind: JoinErrorKind,
}

#[derive(Debug)]
enum JoinErrorKind {
    Cancelled,
    Panic,
}

impl JoinError {
    pub fn is_cancelled(&self) -> bool {
        matches!(self.kind, JoinErrorKind::Cancelled)
    }

    pub fn is_panic(&self) -> bool {
        matches!(self.kind, JoinErrorKind::Panic)
    }

    pub fn cancelled() -> Self {
        JoinError {
            kind: JoinErrorKind::Cancelled,
        }
    }

    pub fn panic() -> Self {
        JoinError {
            kind: JoinErrorKind::Panic,
        }
    }
}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            JoinErrorKind::Cancelled => f.write_str("task was cancelled"),
            JoinErrorKind::Panic => f.write_str("task panicked"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for JoinError {}
