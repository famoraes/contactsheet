use std::fmt;

#[derive(Debug)]
pub struct Error(pub String);

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn msg(s: impl Into<String>) -> Self {
        Error(s.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(format!("I/O error: {e}"))
    }
}

impl From<tiff::TiffError> for Error {
    fn from(e: tiff::TiffError) -> Self {
        Error(format!("TIFF error: {e}"))
    }
}

impl From<::image::ImageError> for Error {
    fn from(e: ::image::ImageError) -> Self {
        Error(format!("image error: {e}"))
    }
}
