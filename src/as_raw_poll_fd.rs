mod nix {
	pub type RawPollFd = std::os::unix::io::RawFd;
	pub trait AsRawPollFd { fn as_raw_poll_fd(&self) -> RawPollFd; }
	impl AsRawPollFd for std::os::unix::io::RawFd { fn as_raw_poll_fd(&self) -> RawPollFd { *self } }
}
pub struct Async<T>(T);
pub struct AsRawFd<T>(T);
impl<T:nix::AsRawPollFd> std::os::unix::io::AsRawFd for AsRawFd<T> { fn as_raw_fd(&self) -> std::os::unix::io::RawFd { self.0.as_raw_poll_fd() /*->smol::Reactor*/ } }
impl<T> std::ops::Deref for AsRawFd<T> { type Target = T; fn deref(&self) -> &Self::Target { &self.0 } }
impl<T> std::ops::DerefMut for AsRawFd<T> { fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 } }
impl<T:nix::AsRawPollFd> Async<T> { pub fn new(io: T) -> Result<smol::Async<AsRawFd<T>>, std::io::Error> { smol::Async::new(AsRawFd(io)) } }
impl nix::AsRawPollFd for client_toolkit::reexports::client::EventQueue { fn as_raw_poll_fd(&self) -> nix::RawPollFd { self.display().get_connection_fd() } }
