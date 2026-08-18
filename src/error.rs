use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Image(image::ImageError),
    Config(serde_json::Error),
    Window(String),
    Pet(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(err) => write!(f, "I/O error: {}", err),
            AppError::Image(err) => write!(f, "Image error: {}", err),
            AppError::Config(err) => write!(f, "Config error: {}", err),
            AppError::Window(msg) => write!(f, "Window error: {}", msg),
            AppError::Pet(msg) => write!(f, "Pet error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io(err) => Some(err),
            AppError::Image(err) => Some(err),
            AppError::Config(err) => Some(err),
            AppError::Window(_) => None,
            AppError::Pet(_) => None,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<image::ImageError> for AppError {
    fn from(err: image::ImageError) -> Self {
        AppError::Image(err)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Config(err)
    }
}
