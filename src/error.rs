#[cfg(not(feature="anyhow"))] mod anyhow {
    #[derive(Debug)] pub struct Error(Box<dyn std::error::Error>);
    impl<E:std::error::Error+'static/*Send+Sync*/> From<E> for Error { fn from(error: E) -> Self { Error(Box::new(error)) } }
    struct MessageError<M>(M);
    impl<M:std::fmt::Debug> std::fmt::Debug for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Debug::fmt(&self.0, f) } }
    impl<M:std::fmt::Display> std::fmt::Display for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Display::fmt(&self.0, f) } }
    impl<M:std::fmt::Debug+std::fmt::Display> std::error::Error for MessageError<M> {}
    impl Error { fn msg(msg: impl std::fmt::Debug+std::fmt::Display+'static) { MessageError(msg) } }
}
pub use anyhow::Error;

pub type Result<T=(), E=Error> = std::result::Result<T, E>;

pub trait OkOr<T> { fn ok_or(self, s: &'static str) -> Result<T, Error>; }
impl<T> OkOr<T> for Result<T, ()> { fn ok_or(self, s: &'static str) -> Result<T, Error> { self.ok().ok_or(Error::msg(s)) } }

pub trait Ok<T> { fn ok(self) -> Result<T, Error>; }
impl<T> Ok<T> for Option<T> { fn ok(self) -> Result<T, Error> { self.ok_or(()).ok_or("none") } }

//#[macro_export] macro_rules! throw { ($val:expr) => { fehler::throw!($crate::core::MessageError(format!("{:?}", $val))); } }
//#[macro_export] macro_rules! assert { ($cond:expr, $($val:expr),* ) => { std::assert!($cond,"{}. {:?}", stringify!($cond), ( $( format!("{} = {:?}", stringify!($val), $val), )* ) ); } }
//#[macro_export] macro_rules! ensure { ($cond:expr) => { (if !$cond { throw!($crate::core::MessageError(stringify!($cond))) } } }
