#[derive(Debug)] pub struct Status<T=std::process::ExitStatus, E=crate::core::Error>(pub Result<T, E>);
impl<T,E> std::ops::Try for Status<T,E> {
    type Ok = T;
    type Error = E;
    fn into_result(self) -> Result<Self::Ok, Self::Error> { self.0.into_result() }
    fn from_error(o: Self::Error) -> Self { Status(Result::from_error(o)) }
    fn from_ok(o: Self::Ok) -> Self { Status(Result::from_ok(o)) }
}
impl<E:std::fmt::Debug> std::process::Termination for Status<i32,E> { fn report(self) -> i32 { match self.0 { Ok(code) => code, Err(err) => {eprintln!("{:?}", err); 1} } } }
impl<E:std::fmt::Debug> std::process::Termination for Status<std::process::ExitStatus,E> {
    fn report(self) -> i32 {
        Status::<_,E>(try {
            let status = self?;
            use std::os::unix::process::ExitStatusExt;
            status.code().unwrap_or_else(||status.signal().unwrap())
        }).report()
    }
}
impl<T,E> From<T> for Status<T,E> { fn from(o: T) -> Self { use std::ops::Try; Self::from_ok(o) } }
