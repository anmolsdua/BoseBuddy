//! How the controller reaches the headset. `WinRfcomm` is the real link
//! (Winsock AF_BTH RFCOMM, proven by the M0 spike); `fake::FakeHeadset` is a
//! simulated QC45 for tests and the CLI's `--fake` mode.

use std::time::Duration;

use crate::protocol::{parse_all, BmapError, Op, Packet};

pub trait Transport: Send {
    /// Send one encoded packet and collect the reply bytes.
    ///
    /// `expect_reply = false`: send, then return nothing (DISCONNECT, power off).
    /// `drain = false`: stop as soon as the buffer holds whole packets.
    /// `drain = true`: keep reading until 300 ms pass with no new data (GetAll).
    fn exchange(
        &mut self,
        request: &[u8],
        expect_reply: bool,
        drain: bool,
        timeout: Duration,
    ) -> Result<Vec<u8>, BmapError>;

    /// The RFCOMM channel in use (0 if not applicable).
    fn channel(&self) -> u8 {
        0
    }

    /// Wait between polls (sending CONNECT, then checking the device list).
    /// The fake headset skips the wait so tests run instantly.
    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// How long a freshly opened channel gets to answer the protocol-version probe.
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Open the first of `channels` that both connects and answers a BMAP
/// request (0.1, protocol version). A channel can accept the connection and
/// never answer, e.g. channel 9 while another QC Buddy holds channel 8.
pub fn open_first_answering<F>(channels: &[u8], mut connect: F) -> Result<Box<dyn Transport>, BmapError>
where
    F: FnMut(u8) -> Result<Box<dyn Transport>, BmapError>,
{
    let probe = Packet::new(0, 1, Op::Get, &[]).encode();
    let mut silent = Vec::new();
    let mut refused = Vec::new();
    for &channel in channels {
        match connect(channel) {
            Ok(mut link) => match link.exchange(&probe, true, false, PROBE_TIMEOUT) {
                Ok(bytes) if parse_all(&bytes).iter().any(|p| p.is(0, 1)) => return Ok(link),
                _ => silent.push(channel),
            },
            Err(e) => refused.push(format!("channel {channel}: {e}")),
        }
    }
    let list = |v: &[u8]| v.iter().map(u8::to_string).collect::<Vec<_>>().join(", ");
    if !silent.is_empty() {
        let opened = channels.iter().filter(|c| !silent.contains(c)).copied().collect::<Vec<_>>();
        let busy = if opened.is_empty() { String::new() } else { format!(" ({} refused)", list(&opened)) };
        return Err(BmapError::Transport(format!(
            "The headphones didn't answer on channel {}{busy}. Another app may be using them: close QC Buddy or the qcbuddy CLI elsewhere and try again.",
            list(&silent)
        )));
    }
    Err(BmapError::Transport(format!(
        "Couldn't open the Bluetooth control channel ({}). Make sure the headphones are on and connected to this PC.",
        refused.join("; ")
    )))
}

#[cfg(windows)]
pub use win::WinRfcomm;

#[cfg(windows)]
mod win {
    //! The socket is non-blocking and polled: Windows documents that after a
    //! SO_RCVTIMEO timeout "the socket state is indeterminate, and should not
    //! be used", so blocking reads with a timeout are avoided.

    use std::time::{Duration, Instant};

    use windows::Win32::Devices::Bluetooth::{AF_BTH, BTHPROTO_RFCOMM, SOCKADDR_BTH};
    use windows::Win32::Networking::WinSock::{
        closesocket, connect, ioctlsocket, recv, send, socket, WSACleanup, WSAGetLastError,
        WSAStartup, FIONBIO, INVALID_SOCKET, SEND_RECV_FLAGS, SOCKADDR, SOCKET, SOCK_STREAM,
        WSADATA, WSAEWOULDBLOCK,
    };

    use super::Transport;
    use crate::protocol::{is_complete, BmapError};

    pub struct WinRfcomm {
        sock: SOCKET,
        channel: u8,
        closed: bool,
    }

    impl WinRfcomm {
        /// Connect to `address` (e.g. 0x782B645B71EE), trying each channel in order.
        pub fn open(address: u64, channels: &[u8]) -> Result<WinRfcomm, BmapError> {
            let mut data = WSADATA::default();
            let rc = unsafe { WSAStartup(0x0202, &mut data) };
            if rc != 0 {
                return Err(BmapError::Transport(format!("Windows sockets failed to start ({rc}).")));
            }
            let mut errors = Vec::new();
            for &channel in channels {
                match open_channel(address, channel) {
                    Ok(sock) => return Ok(WinRfcomm { sock, channel, closed: false }),
                    Err(e) => errors.push(format!("channel {channel}: {e}")),
                }
            }
            unsafe { WSACleanup() };
            Err(BmapError::Transport(format!(
                "Couldn't open the Bluetooth control channel ({}). Make sure the headphones are on and connected to this PC.",
                errors.join("; ")
            )))
        }

        /// Bytes waiting right now, without blocking.
        fn read_available(&mut self, into: &mut Vec<u8>) -> Result<usize, BmapError> {
            if self.closed {
                return Err(BmapError::NotConnected);
            }
            let mut buf = [0u8; 1024];
            let mut total = 0;
            loop {
                let n = unsafe { recv(self.sock, &mut buf, SEND_RECV_FLAGS(0)) };
                if n > 0 {
                    into.extend_from_slice(&buf[..n as usize]);
                    total += n as usize;
                    continue;
                }
                if n == 0 {
                    self.closed = true;
                    return Err(BmapError::NotConnected);
                }
                let err = unsafe { WSAGetLastError() };
                if err == WSAEWOULDBLOCK {
                    return Ok(total);
                }
                self.closed = true;
                return Err(BmapError::NotConnected);
            }
        }

        fn send_all(&mut self, packet: &[u8]) -> Result<(), BmapError> {
            if self.closed {
                return Err(BmapError::NotConnected);
            }
            let sent = unsafe { send(self.sock, packet, SEND_RECV_FLAGS(0)) };
            if sent != packet.len() as i32 {
                let err = unsafe { WSAGetLastError() };
                self.closed = true;
                return Err(BmapError::Transport(format!("Bluetooth write failed ({err:?}).")));
            }
            Ok(())
        }
    }

    fn open_channel(address: u64, channel: u8) -> Result<SOCKET, String> {
        unsafe {
            let sock = socket(AF_BTH as i32, SOCK_STREAM, BTHPROTO_RFCOMM as i32)
                .map_err(|e| format!("socket: {e}"))?;
            if sock == INVALID_SOCKET {
                return Err(format!("socket: {:?}", WSAGetLastError()));
            }
            let addr = SOCKADDR_BTH {
                addressFamily: AF_BTH,
                btAddr: address,
                serviceClassId: Default::default(),
                port: channel as u32,
            };
            let rc = connect(
                sock,
                &addr as *const SOCKADDR_BTH as *const SOCKADDR,
                std::mem::size_of::<SOCKADDR_BTH>() as i32,
            );
            if rc != 0 {
                let err = WSAGetLastError();
                let _ = closesocket(sock);
                return Err(format!("connect {err:?}"));
            }
            let mut nonblocking: u32 = 1;
            if ioctlsocket(sock, FIONBIO, &mut nonblocking) != 0 {
                let err = WSAGetLastError();
                let _ = closesocket(sock);
                return Err(format!("ioctlsocket {err:?}"));
            }
            Ok(sock)
        }
    }

    impl Transport for WinRfcomm {
        fn exchange(
            &mut self,
            request: &[u8],
            expect_reply: bool,
            drain: bool,
            timeout: Duration,
        ) -> Result<Vec<u8>, BmapError> {
            // Drop anything left over from an earlier request (unsolicited
            // notifications, a late reply), so it can't be read as this reply.
            let mut stale = Vec::new();
            self.read_available(&mut stale)?;
            self.send_all(request)?;
            if !expect_reply {
                std::thread::sleep(Duration::from_millis(120));
                let _ = self.read_available(&mut stale);
                return Ok(Vec::new());
            }

            let deadline = Instant::now() + timeout;
            let mut buffer = Vec::new();
            let mut last_data = Instant::now();
            while Instant::now() < deadline {
                if self.read_available(&mut buffer)? > 0 {
                    last_data = Instant::now();
                    if !drain && is_complete(&buffer) {
                        break;
                    }
                }
                if drain && !buffer.is_empty() && last_data.elapsed() > Duration::from_millis(300) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(8));
            }
            if buffer.is_empty() {
                return Err(BmapError::Timeout);
            }
            Ok(buffer)
        }

        fn channel(&self) -> u8 {
            self.channel
        }
    }

    impl Drop for WinRfcomm {
        fn drop(&mut self) {
            unsafe {
                let _ = closesocket(self.sock);
                WSACleanup();
            }
        }
    }

    // The raw SOCKET handle is only used by the thread that owns WinRfcomm.
    unsafe impl Send for WinRfcomm {}
}
