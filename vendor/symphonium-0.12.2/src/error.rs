use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum LoadError {
    FileNotFound(std::io::Error),
    UnkownFormat(symphonia::core::errors::Error),
    NoAudioTrackFound,
    TrackIndexOutOfBounds {
        index: usize,
        num_tracks: usize,
    },
    NoAudioChannelsFound,
    NoAudioCodecFound,
    UnkownSampleType,
    UnkownChannelFormat(usize),
    FileTooLarge(usize),
    CouldNotCreateDecoder(symphonia::core::errors::Error),
    ErrorWhileDecoding(symphonia::core::errors::Error),
    #[cfg(feature = "resampler")]
    InvalidResampler {
        needed_channels: usize,
        got_channels: usize,
    },
    #[cfg(feature = "resampler")]
    ErrorWhileResampling(fixed_resample::rubato::ResampleError),
}

impl Error for LoadError {}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use LoadError::*;

        match self {
            FileNotFound(e) => write!(f, "File not found: {}", e),
            UnkownFormat(e) => write!(f, "Format not supported: {}", e),
            NoAudioTrackFound => write!(f, "No default audio track found"),
            TrackIndexOutOfBounds { index, num_tracks } => write!(
                f,
                "The track index {index} is out of bounds for media with {num_tracks} tracks"
            ),
            NoAudioChannelsFound => write!(f, "No audio channels found"),
            NoAudioCodecFound => write!(f, "No audio codec found"),
            UnkownSampleType => write!(f, "Unknown sample format"),
            UnkownChannelFormat(num_channels) => {
                write!(f, "Unkown channel format: {} channels found", num_channels)
            }
            FileTooLarge(max_bytes) => {
                write!(f, "File is too large: maximum is {} bytes", max_bytes)
            }
            CouldNotCreateDecoder(e) => write!(f, "Failed to create decoder: {}", e),
            ErrorWhileDecoding(e) => write!(f, "Error while decoding: {}", e),
            #[cfg(feature = "resampler")]
            InvalidResampler {
                got_channels,
                needed_channels,
            } => {
                write!(
                    f,
                    "Invalid custom resampler: Expected {} channels, got {} channels",
                    needed_channels, got_channels
                )
            }
            #[cfg(feature = "resampler")]
            ErrorWhileResampling(e) => write!(f, "Error while resampling: {}", e),
        }
    }
}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        Self::FileNotFound(e)
    }
}

#[cfg(feature = "resampler")]
impl From<fixed_resample::rubato::ResampleError> for LoadError {
    fn from(e: fixed_resample::rubato::ResampleError) -> Self {
        Self::ErrorWhileResampling(e)
    }
}
