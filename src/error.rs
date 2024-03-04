use std::fmt::{self, Debug};
use std::error::Error;

pub type Result<T> = std::result::Result<T, StateError>;

// Определение перечисления для пользовательской ошибки
#[derive(Debug)]
pub enum StateError
{
    MismatchedTypes(),
    Default(String),
}

// Реализация трейта Display для пользовательской ошибки
impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StateError::MismatchedTypes() => {
                write!(f, "Given object has different type")
            },
            StateError::Default(s) => write!(f, "{}", s)
        }
    }
}

// Реализация трейта Error для пользовательской ошибки
impl Error for StateError {}

