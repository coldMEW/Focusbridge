mod bridge;
mod registry;

#[derive(Debug, PartialEq, Eq)]
enum Error {
    InvalidHandle,
    Capacity,
    InvalidInput,
    Engine(focusbridge_secure_channel::Error),
    Jni,
    Internal,
}

impl From<focusbridge_secure_channel::Error> for Error {
    fn from(error: focusbridge_secure_channel::Error) -> Self {
        Self::Engine(error)
    }
}

type Result<T> = std::result::Result<T, Error>;
