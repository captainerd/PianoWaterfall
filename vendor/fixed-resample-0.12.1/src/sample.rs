/// The trait governing a single sample.
///
/// There are two types which implements this trait:
/// * [u8]
/// * [i8]
/// * [u16]
/// * [i16]
/// * [u32]
/// * [i32]
/// * [u64]
/// * [i64]
/// * [f32]
/// * [f64]
/// * `[u8; N]`
pub trait Sample
where
    Self: Copy + Send + 'static,
{
    fn zero() -> Self;
}

impl Sample for u8 {
    fn zero() -> Self {
        0
    }
}

impl Sample for i8 {
    fn zero() -> Self {
        0
    }
}

impl Sample for u16 {
    fn zero() -> Self {
        0
    }
}

impl Sample for i16 {
    fn zero() -> Self {
        0
    }
}

impl Sample for u32 {
    fn zero() -> Self {
        0
    }
}

impl Sample for i32 {
    fn zero() -> Self {
        0
    }
}

impl Sample for u64 {
    fn zero() -> Self {
        0
    }
}

impl Sample for i64 {
    fn zero() -> Self {
        0
    }
}

impl Sample for f32 {
    fn zero() -> Self {
        0.0
    }
}

impl Sample for f64 {
    fn zero() -> Self {
        0.0
    }
}

impl<const N: usize> Sample for [u8; N] {
    fn zero() -> Self {
        [0; N]
    }
}
