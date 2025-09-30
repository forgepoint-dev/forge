use async_graphql::{Error, ErrorExtensions};

pub fn bad_user_input(message: impl Into<String>) -> Error {
    Error::new(message.into()).extend_with(|_, e| e.set("code", "BAD_USER_INPUT"))
}

pub fn internal_error(err: impl std::fmt::Display) -> Error {
    Error::new(err.to_string())
}
