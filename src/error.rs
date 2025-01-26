use thiserror::Error;

#[derive(Error, Debug)]
pub enum BBBotError {
    #[error("ESP Error")]
    ESPError(#[from] esp_idf_sys::EspError),
    #[error("Unit error")]
    UnitError,
}

impl From<()> for BBBotError {
    fn from(_: ()) -> BBBotError {
        BBBotError::UnitError
    }
}
